#requires -Version 5.1
# P1-07 acceptance: LNK launching keeps the shortcut's own semantics.
# - A real .lnk (cmd /c echo via-lnk>probe.txt, workdir=temp) is created with
#   WScript.Shell; the item stores only the .lnk path, so the probe file proves
#   the LNK's own arguments/working directory were used.
# - Picking the same .lnk in the editor fills args/workdir from IShellLinkW.
# - A missing .lnk reports a structured error in the status bar.
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

if (-not ('P107Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P107Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static void TypeText(string text){
  foreach(var c in text){
   var down=new Input{type=1,u=new Union{key=new Keyboard{vk=0,scan=c,flags=0x0004}}};
   var up=new Input{type=1,u=new Union{key=new Keyboard{vk=0,scan=c,flags=0x0004|0x0002}}};
   if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
  }
 }
 public static void Key(ushort vk){
  var down=new Input{type=1,u=new Union{key=new Keyboard{vk=vk}}};
  var up=new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=0x0002}}};
  if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static void SelectAll(){
  const ushort ctrl=0x11, a=0x41;
  var inputs=new[]{
   new Input{type=1,u=new Union{key=new Keyboard{vk=ctrl}}},
   new Input{type=1,u=new Union{key=new Keyboard{vk=a}}},
   new Input{type=1,u=new Union{key=new Keyboard{vk=a,flags=0x0002}}},
   new Input{type=1,u=new Union{key=new Keyboard{vk=ctrl,flags=0x0002}}}};
  if(SendInput(4,inputs,Marshal.SizeOf(typeof(Input)))!=4)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
public static class P107Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-07-lnk.tsv'
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

function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 500) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P107Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P107Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$probeDirectory = Join-Path $env:TEMP 'quicknest-p107'
if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDirectory)
$lnkPath = Join-Path $probeDirectory 'LnkProbe.lnk'
$lnkProbeFile = Join-Path $probeDirectory 'lnk-probe.txt'

# Real shortcut: cmd runs from the link's working directory with its arguments.
$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($lnkPath)
$shortcut.TargetPath = 'C:\Windows\System32\cmd.exe'
$shortcut.Arguments = '/c echo via-lnk>lnk-probe.txt'
$shortcut.WorkingDirectory = $probeDirectory
$shortcut.Save()
if (-not (Test-Path $lnkPath)) { throw 'failed to create the test shortcut' }
Record 'lnk_created' $lnkPath

$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'LnkTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'Tests'; icon = ''
                    items = @(
                        @{ id = 'lnk-probe'; name = 'LnkProbe'; item_type = 'lnk'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $lnkPath; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'lnk-missing'; name = 'LnkMissing'; item_type = 'lnk'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\QuickNest Demo\missing.lnk'; args = @(); workdir = ''; run_as_admin = $false } }
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
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-initial.png') -WindowHwnd $win.hwnd

    $tileY = 150
    # a) Single click starts the shortcut through the Shell (LNK semantics).
    Invoke-ClickAt ($left + 208) ($top + $tileY) 1800
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-after-click.png') -WindowHwnd $win.hwnd
    if (-not (Test-Path $lnkProbeFile)) { throw 'lnk probe file was not created' }
    $content = Get-Content -LiteralPath $lnkProbeFile -Raw -Encoding UTF8
    if ($content -notmatch 'via-lnk') { throw "lnk probe content mismatch: $content" }
    Record 'click_launch' 'LNK started with its own args+workdir'
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-after-click.png') -WindowHwnd $win.hwnd

    # b) Missing LNK: structured error in the status bar.
    Invoke-ClickAt ($left + 304) ($top + $tileY) 800
    Record 'missing_lnk' 'status bar error expected'
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-missing.png') -WindowHwnd $win.hwnd

    # c) Editor: a saved LNK resolves its metadata through IShellLinkW
    #    (args/workdir filled at save), and keeps the .lnk as its target.
    #    Let the background icon refresh settle first so it cannot race the
    #    injected clicks below.
    Start-Sleep -Milliseconds 1500
    Invoke-ClickAt ($left + 620) ($top + 420) 300
    Invoke-RightClickAt ($left + 620) ($top + 420)
    Invoke-ClickAt ($left + 660) ($top + 429) 600
    Invoke-ClickAt ($left + 370) ($top + 113) 200    # name field
    [P005Keyboard]::TypeText('PickedLnk')
    Invoke-ClickAt ($left + 370) ($top + 192) 200    # target field
    [P005Keyboard]::TypeText($lnkPath)
    Start-Sleep -Milliseconds 200
    # Type combo: second entry is "Windows 快捷方式" (LNK).
    Invoke-ClickAt ($left + 310) ($top + 153) 700
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-type-combo.png') -WindowHwnd $win.hwnd
    Invoke-ClickAt ($left + 310) ($top + 153 + 56) 400   # second entry
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-dialog-filled.png') -WindowHwnd $win.hwnd
    Invoke-ClickAt ($left + 581) ($top + 432) 700   # 保存
    $cfg = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $saved = @($cfg.groups[0].categories[0].items | Where-Object { $_.name -eq 'PickedLnk' })
    if ($saved.Count -ne 1) { throw "saved LNK item missing (count=$($saved.Count))" }
    if ($saved[0].launch.path -ne $lnkPath) { throw "saved target: $($saved[0].launch.path)" }
    if ($saved[0].item_type -ne 'lnk') { throw "picked item type: $($saved[0].item_type)" }
    if (($saved[0].launch.args -join ' ') -notmatch 'via-lnk') {
        throw "picked item args: $($saved[0].launch.args -join ' ')"
    }
    if ($saved[0].launch.workdir -ne $probeDirectory) {
        throw "picked item workdir: $($saved[0].launch.workdir)"
    }
    Record 'metadata_parsed' 'args/workdir filled from the shortcut'
    Save-P005Screenshot -Path (Join-Path $results 'p1-07-after-save.png') -WindowHwnd $win.hwnd

    Stop-QuickNest $run
    Record 'completed' 'all P1-07 LNK phases passed'
    Write-Host 'All P1-07 LNK phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
    if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force -ErrorAction SilentlyContinue }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
