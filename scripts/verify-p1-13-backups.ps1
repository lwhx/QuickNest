#requires -Version 5.1
# P1-13 acceptance: backup retention keeps the ten newest valid backups and the
# status-bar "备份" entry creates a manual backup (also pruned).
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

if (-not ('P113Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P113Input {
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
$report = Join-Path $results 'p1-13-backups.tsv'
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
    return @{ Process = $proc; Window = $win }
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
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Get-ChildItem -LiteralPath $configDir -Filter 'config.corrupted.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

# a) Many saves: every replacement creates a backup, retention caps them at 10.
$run = Start-QuickNest
try {
    $left = $run.Window.rect.Left
    $top = $run.Window.rect.Top
    for ($i = 0; $i -lt 14; $i++) {
        $rowY = if ($i % 2 -eq 0) { 136 } else { 108 }
        Invoke-ClickAt ($left + 40) ($top + $rowY)
    }
    Start-Sleep -Milliseconds 500
    $backups = @(Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue)
    if ($backups.Count -ne 10) { throw "retention failed: $($backups.Count) backups kept" }
    Record 'retention' ("$($backups.Count) backups kept after 14 saves")
    Save-P005Screenshot -Path (Join-Path $results 'p1-13-status-bar.png') -WindowHwnd $run.Window.hwnd
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# b) Manual backup from the status bar; retention still applies.
$latestBefore = @(Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' | Sort-Object Name -Descending)[0].Name
$run = Start-QuickNest
try {
    $left = $run.Window.rect.Left
    $top = $run.Window.rect.Top
    Invoke-ClickAt ($left + 734) ($top + 504)   # 备份 entry at the end of the status bar
    Start-Sleep -Milliseconds 700
    $backups = @(Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue)
    if ($backups.Count -ne 10) { throw "manual backup broke retention: $($backups.Count)" }
    $latest = @($backups | Sort-Object Name -Descending)[0].Name
    if ($latest -eq $latestBefore) { throw 'manual backup did not create a newer file' }
    Record 'manual_backup' ("newest=$latest (before=$latestBefore)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-13-manual-backup.png') -WindowHwnd $run.Window.hwnd
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Record 'completed' 'all P1-13 backup phases passed'
Write-Host 'All P1-13 backup phases passed.'
Get-Content -LiteralPath $report -Encoding UTF8

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
