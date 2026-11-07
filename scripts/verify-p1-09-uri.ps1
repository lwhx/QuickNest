#requires -Version 5.1
# P1-09 acceptance: URIs launch through the system protocol handler.
# A temporary HKCU protocol (quicknest-p109) points at cmd writing a probe
# file, so the launch result is observable; the registration is removed in
# finally. An invalid URI must report a structured error.
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

if (-not ('P109Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P109Input {
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
$report = Join-Path $results 'p1-09-uri.tsv'
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

function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$probeDirectory = Join-Path $env:TEMP 'quicknest-p109'
if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDirectory)
$uriProbeFile = Join-Path $probeDirectory 'uri-probe.txt'
$protocolRoot = 'HKCU:\Software\Classes\quicknest-p109'

# Temporary custom protocol so the launch has an observable effect.
New-Item -Path $protocolRoot -Force | Out-Null
New-ItemProperty -Path $protocolRoot -Name 'URL Protocol' -Value '' -PropertyType String -Force | Out-Null
New-Item -Path "$protocolRoot\shell\open\command" -Force | Out-Null
$command = '"C:\Windows\System32\cmd.exe" /c echo uri-launched>"' + $uriProbeFile + '"'
Set-Item -Path "$protocolRoot\shell\open\command" -Value $command
Record 'protocol_registered' $command

$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'UriTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'Tests'; icon = ''
                    items = @(
                        @{ id = 'uri-probe'; name = 'UriProbe'; item_type = 'uri'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'quicknest-p109:probe'; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'uri-invalid'; name = 'UriInvalid'; item_type = 'uri'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'not a uri'; args = @(); workdir = ''; run_as_admin = $false } }
                    )
                }
            )
        }
    )
}
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$json = $library | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($configPath, $json, (New-Object Text.UTF8Encoding($false)))

$run = Start-QuickNest
try {
    $win = $run.Window
    $left = $win.rect.Left
    $top = $win.rect.Top
    Record 'window_found' ("pid=$($run.Process.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-09-initial.png') -WindowHwnd $win.hwnd

    $tileY = 150
    # a) Valid URI: the registered protocol writes the probe file.
    Invoke-ClickAt ($left + 208) ($top + $tileY) 1800
    if (-not (Test-Path $uriProbeFile)) { throw 'uri probe file was not created' }
    $content = Get-Content -LiteralPath $uriProbeFile -Raw -Encoding UTF8
    if ($content -notmatch 'uri-launched') { throw "uri probe content mismatch: $content" }
    Record 'uri_launch' 'custom protocol handled by the Shell'
    Save-P005Screenshot -Path (Join-Path $results 'p1-09-after-click.png') -WindowHwnd $win.hwnd

    # b) Invalid URI: structured error in the status bar.
    Invoke-ClickAt ($left + 304) ($top + $tileY) 800
    Record 'invalid_uri' 'status bar error expected'
    Save-P005Screenshot -Path (Join-Path $results 'p1-09-invalid.png') -WindowHwnd $win.hwnd

    Stop-QuickNest $run
    Record 'completed' 'all P1-09 URI phases passed'
    Write-Host 'All P1-09 URI phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
    if (Test-Path $protocolRoot) { Remove-QnSandboxItem -LiteralPath $protocolRoot -Recurse -Force -ErrorAction SilentlyContinue }
    if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force -ErrorAction SilentlyContinue }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
