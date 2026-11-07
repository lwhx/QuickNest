#requires -Version 5.1
# P1-11 acceptance: BLAKE3-keyed icon snapshots are written on first launch,
# reused (without rewrite) on the next launch, and pruned once no item
# references them anymore.
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

if (-not ('P111Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P111Input {
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
$report = Join-Path $results 'p1-11-icon-cache.tsv'
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
    Start-Sleep -Milliseconds 450
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
    Record 'wm_close_exit' ("exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$iconsDir = Join-Path $env:QUICKNEST_DATA_DIR 'icons'
if (Test-Path $iconsDir) { Remove-QnSandboxItem -LiteralPath $iconsDir -Recurse -Force }

$library = @{
    schema_version = 2
    current_group_id = 'g1'
    groups = @(
        @{
            id = 'g1'; name = 'CacheTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'Tests'; icon = ''
                    items = @(
                        @{ id = 'cmd-icon'; name = 'CmdIcon'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\Windows\System32\cmd.exe'; args = @(); workdir = ''; run_as_admin = $false } }
                    )
                }
            )
        }
    )
}
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$json = $library | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($configPath, $json, (New-Object Text.UTF8Encoding($false)))

# a) First launch writes exactly one BLAKE3-keyed snapshot.
$run = Start-QuickNest
try {
    Start-Sleep -Milliseconds 2200
    $files = @(Get-ChildItem -LiteralPath $iconsDir -Filter '*.qni' -ErrorAction SilentlyContinue)
    if ($files.Count -ne 1) { throw "expected one cache file, found $($files.Count)" }
    $first = $files[0]
    Record 'cache_written' ("$($first.Name) bytes=$($first.Length)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-11-first-launch.png') -WindowHwnd $run.Window.hwnd
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# b) Second launch reuses the snapshot without rewriting it.
$run = Start-QuickNest
try {
    Start-Sleep -Milliseconds 2200
    $files = @(Get-ChildItem -LiteralPath $iconsDir -Filter '*.qni' -ErrorAction SilentlyContinue)
    if ($files.Count -ne 1) { throw "cache count changed on second launch: $($files.Count)" }
    if ($files[0].LastWriteTimeUtc -ne $first.LastWriteTimeUtc) {
        throw "cache hit rewrote the snapshot: $($files[0].LastWriteTimeUtc) != $($first.LastWriteTimeUtc)"
    }
    Record 'cache_reused' ("mtime=$($files[0].LastWriteTimeUtc.ToString('o'))")
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# c) Removing the only item prunes the now-unreferenced snapshot on startup.
$emptyLibrary = @{
    schema_version = 2
    current_group_id = 'g1'
    groups = @(
        @{
            id = 'g1'; name = 'CacheTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{ id = 'c1'; name = 'Tests'; icon = ''; items = @() }
            )
        }
    )
}
$json = $emptyLibrary | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($configPath, $json, (New-Object Text.UTF8Encoding($false)))
$run = Start-QuickNest
try {
    Start-Sleep -Milliseconds 1200
    $remaining = @(Get-ChildItem -LiteralPath $iconsDir -Filter '*.qni' -ErrorAction SilentlyContinue)
    if ($remaining.Count -ne 0) { throw "unreferenced cache entries survived: $($remaining.Count)" }
    Record 'cache_pruned' 'no unreferenced snapshots remain'
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

Record 'completed' 'all P1-11 icon-cache phases passed'
Write-Host 'All P1-11 icon-cache phases passed.'
Get-Content -LiteralPath $report -Encoding UTF8

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
