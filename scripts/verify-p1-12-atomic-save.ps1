#requires -Version 5.1
# P1-12 acceptance: production recovery path (baseline 15.8/15.9).
# - A corrupt config with a valid backup is restored and the corrupt file is
#   renamed to config.corrupted.<stamp>.json.
# - A corrupt config without backups enters safe mode and is never overwritten.
# - A newer schema_version enters safe mode and is never overwritten.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))

if (-not ('P112Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P112Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-12-atomic-save.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -like 'QuickNest*' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

function Start-QuickNest {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = (Join-Path $root 'target\release\quicknest.exe')
    $psi.WorkingDirectory = $root
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    if (-not $proc.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $proc
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $win = $null
    while ($null -eq $win -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $win = Get-QuickNestWindow $proc.Id
    }
    if ($null -eq $win) { throw 'QuickNest window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 40, 120, 0, 0, 0x0001)
    [void][P005Desktop]::SetForegroundWindow($win.hwnd)
    Start-Sleep -Milliseconds 500
    $win = Get-QuickNestWindow $proc.Id
    if ($null -eq $win) { throw 'window lost after move' }
    $shot = Join-Path $results ("p1-12-{0}.png" -f $Label)
    Save-P005Screenshot -Path $shot -WindowHwnd $win.hwnd
    return @{ Process = $proc; Window = $win; Shot = $shot }
}

function Stop-QuickNest($Run) {
    $proc = $Run.Process
    $win = $Run.Window
    if ($proc.HasExited) { throw "process exited before close (exit=$($proc.ExitCode))" }
    Close-QnOwnedWindow -Hwnd ([IntPtr]$win.hwnd)
    if (-not $proc.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $proc); throw 'no exit on WM_CLOSE' }
    $proc.WaitForExit()
    $err = $proc.StandardError.ReadToEnd()
    if ($err) { Set-Content -LiteralPath (Join-Path $results 'stderr.log') -Value $err -Encoding UTF8 }
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }
}

function Invoke-ClickAt([int]$X, [int]$Y) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 450
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Get-ChildItem -LiteralPath $configDir -Filter 'config.corrupted.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

# a) Produce a config and a backup through normal saves.
$Label = 'backup-seed'
$run = Start-QuickNest
try {
    $left = $run.Window.rect.Left
    $top = $run.Window.rect.Top
    Invoke-ClickAt ($left + 40) ($top + 136)   # second category -> first save
    Invoke-ClickAt ($left + 40) ($top + 108)   # first category -> replacement + backup
    Start-Sleep -Milliseconds 400
    if (-not (Test-Path $configPath)) { throw 'config was not created' }
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}
$backups = @(Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue)
if ($backups.Count -lt 1) { throw 'no backup was produced by the atomic save' }
Record 'backup_seeded' ("$($backups.Count) backup(s), first=$($backups[0].Name)")

# b) Corrupt the config: startup restores the newest valid backup.
[IO.File]::WriteAllText($configPath, 'garbage-not-json', (New-Object Text.UTF8Encoding($false)))
$Label = 'recovered'
$run = Start-QuickNest
try {
    Start-Sleep -Milliseconds 600
    $content = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    $null = $content | ConvertFrom-Json   # throws when recovery failed
    $corrupted = @(Get-ChildItem -LiteralPath $configDir -Filter 'config.corrupted.*.json' -ErrorAction SilentlyContinue)
    if ($corrupted.Count -lt 1) { throw 'the corrupt file was not renamed out of the way' }
    Record 'recovered' ("config valid; corrupted=$($corrupted[0].Name)")
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# c) No backups left: safe mode must not overwrite the corrupt file.
$backups | Remove-QnSandboxItem -Force
Get-ChildItem -LiteralPath $configDir -Filter 'config.corrupted.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
[IO.File]::WriteAllText($configPath, 'still-garbage', (New-Object Text.UTF8Encoding($false)))
$Label = 'safe-mode'
$run = Start-QuickNest
try {
    $left = $run.Window.rect.Left
    $top = $run.Window.rect.Top
    Invoke-ClickAt ($left + 40) ($top + 136)   # attempt a mutating action
    $content = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($content -ne 'still-garbage') { throw "safe mode overwrote the corrupt file: $content" }
    Record 'safe_mode' 'corrupt config untouched after a mutating action'
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# d) Newer schema version: safe mode again, file untouched.
[IO.File]::WriteAllText($configPath, '{"schema_version":3,"current_group_id":null,"groups":[]}', (New-Object Text.UTF8Encoding($false)))
$before = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
$Label = 'newer-schema'
$run = Start-QuickNest
try {
    Start-Sleep -Milliseconds 400
    $after = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($after -ne $before) { throw 'a newer schema config was overwritten' }
    Record 'newer_schema' 'v3 config rejected without modification'
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Record 'completed' 'all P1-12 atomic-save phases passed'
Write-Host 'All P1-12 atomic-save phases passed.'
Get-Content -LiteralPath $report -Encoding UTF8

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
