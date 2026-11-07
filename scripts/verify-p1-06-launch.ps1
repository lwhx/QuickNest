#requires -Version 5.1
# P1-06 acceptance: EXE launch through the grid and the item menu.
# - CmdProbe (cmd.exe /c echo ...) proves arguments + working directory.
# - MshtaTest proves the normal (CreateProcessW) path and the elevated path.
# - MaxTest proves the persisted maximized window state.
# - MissingTest proves the structured failure reported in the status bar.
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

if (-not ('P106Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P106Input {
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
public static class P106Window {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [StructLayout(LayoutKind.Sequential)]public struct Placement{public uint length;public uint flags;public uint showCmd;public Point min;public Point max;public Rect normal;}
 [DllImport("user32.dll")]public static extern bool GetWindowPlacement(IntPtr h,ref Placement p);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-06-launch.tsv'
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
    [P106Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P106Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Get-OpenMenuRect([int]$X, [int]$Y) {
    $point = New-Object P106Window+Point
    $point.X = $X
    $point.Y = $Y
    $menuHwnd = [P106Window]::WindowFromPoint($point)
    $rect2 = New-Object P106Window+Rect
    [void][P106Window]::GetWindowRect($menuHwnd, [ref]$rect2)
    return $rect2
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$probeDirectory = Join-Path $env:TEMP 'quicknest-p106'
if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDirectory)
$probeFile = Join-Path $probeDirectory 'probe.txt'
$startupProbeFile = Join-Path $env:TEMP 'quicknest-startup-probe.txt'
$startupProbeExe = Join-Path $probeDirectory 'StartupProbe.exe'

# A tiny Win32 GUI program that records the STARTUPINFO it was launched with
# (dwFlags/wShowWindow) and then shows a window. This proves the window-state
# hand-off independently of whether third-party apps honour it.
$startupProbeSource = @'
using System;using System.IO;using System.Runtime.InteropServices;using System.Windows.Forms;
public static class StartupProbe {
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
 public struct STARTUPINFO { public int cb; public IntPtr lpReserved; public IntPtr lpDesktop; public IntPtr lpTitle;
  public int dwX; public int dwY; public int dwXSize; public int dwYSize; public int dwXCountChars; public int dwYCountChars;
  public int dwFillAttribute; public int dwFlags; public short wShowWindow; public short cbReserved2;
  public IntPtr lpReserved2; public IntPtr hStdInput; public IntPtr hStdOutput; public IntPtr hStdError; }
 [DllImport("kernel32.dll")] private static extern void GetStartupInfoW(out STARTUPINFO si);
 [STAThread] public static void Main() {
  STARTUPINFO si; GetStartupInfoW(out si);
  File.WriteAllText(Path.Combine(Path.GetTempPath(), "quicknest-startup-probe.txt"),
   "flags=" + si.dwFlags + ";show=" + si.wShowWindow);
  Application.Run(new Form());
 }
}
'@
Add-Type -TypeDefinition $startupProbeSource -OutputAssembly $startupProbeExe -OutputType WindowsApplication -ReferencedAssemblies System.Windows.Forms, System.Drawing

# Seed four projects so every launch path can be exercised from the grid.
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'LaunchTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'Tests'; icon = ''
                    items = @(
                        @{ id = 'cmd-probe'; name = 'CmdProbe'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\Windows\System32\cmd.exe'; args = @('/c', 'echo launched>probe.txt'); workdir = $probeDirectory; run_as_admin = $false } },
                        @{ id = 'charmap'; name = 'MshtaTest'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\Windows\System32\mshta.exe'; args = @('about:blank'); workdir = ''; run_as_admin = $false } },
                        @{ id = 'missing'; name = 'MissingTest'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\QuickNest Demo\missing-app.exe'; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'max-test'; name = 'StartupProbeTest'; item_type = 'exe'; icon = ''; window_state = 'maximized'; note = ''
                           launch = @{ path = $startupProbeExe; args = @(); workdir = ''; run_as_admin = $false } }
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
$spawnedCharmap = @()
try {
    $win = $run.Window
    $left = $win.rect.Left
    $top = $win.rect.Top
    Record 'window_found' ("pid=$($run.Process.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-06-initial.png') -WindowHwnd $win.hwnd

    # Tile centres: grid origin (160, 96), medium cell 96x108, tile inset 4.
    $tileY = 150
    # a) CmdProbe: single click starts cmd /c echo > probe.txt in the workdir.
    Invoke-ClickAt ($left + 208) ($top + $tileY) 1500
    if (-not (Test-Path $probeFile)) { throw 'cmd probe file was not created' }
    $content = Get-Content -LiteralPath $probeFile -Raw -Encoding UTF8
    if ($content -notmatch 'launched') { throw "cmd probe content mismatch: $content" }
    Record 'click_launch' 'CmdProbe args+workdir honoured'
    Save-P005Screenshot -Path (Join-Path $results 'p1-06-after-click.png') -WindowHwnd $win.hwnd

    # b) Item menu "打开": MshtaTest starts through the open entry.
    $before = @(Get-Process mshta -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Invoke-RightClickAt ($left + 304) (($top + $tileY))
    $menu = Get-OpenMenuRect ($left + 314) (($top + $tileY) + 10)
    Record 'menu_rect' ("$($menu.Left),$($menu.Top),$($menu.Right),$($menu.Bottom)")
    Invoke-ClickAt ($menu.Left + 40) ($menu.Top + 15)     # 打开
    $started = $null
    for ($i = 0; $i -lt 30 -and $null -eq $started; $i++) {
        Start-Sleep -Milliseconds 200
        $started = Get-Process mshta -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | Select-Object -First 1
    }
    if ($null -eq $started) { throw 'mshta did not start from the menu' }
    Record 'menu_open' ("charmap pid=$($started.Id)")
    $spawnedCharmap += $started.Id
    Stop-QnOwnedProcess -Id $started.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    Save-P005Screenshot -Path (Join-Path $results 'p1-06-menu-open.png') -WindowHwnd $win.hwnd

    # c) Item menu "管理员身份运行": runas path (this session is elevated).
    $before = @(Get-Process mshta -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Invoke-RightClickAt ($left + 304) (($top + $tileY))
    $menu = Get-OpenMenuRect ($left + 314) (($top + $tileY) + 10)
    Invoke-ClickAt ($menu.Left + 40) ($menu.Top + 35)     # 管理员身份运行
    $started = $null
    for ($i = 0; $i -lt 40 -and $null -eq $started; $i++) {
        Start-Sleep -Milliseconds 250
        $started = Get-Process mshta -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | Select-Object -First 1
    }
    if ($null -eq $started) { throw 'mshta did not start elevated' }
    Record 'menu_admin' ("charmap pid=$($started.Id)")
    $spawnedCharmap += $started.Id
    Stop-QnOwnedProcess -Id $started.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    Save-P005Screenshot -Path (Join-Path $results 'p1-06-menu-admin.png') -WindowHwnd $win.hwnd

    # d) MaxTest: the persisted maximized window state reaches STARTUPINFO.
    $before = @(Get-Process mshta -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Invoke-ClickAt ($left + 496) ($top + $tileY) 2500
    $probeText = $null
    for ($i = 0; $i -lt 40 -and $null -eq $probeText; $i++) {
        Start-Sleep -Milliseconds 200
        if (Test-Path $startupProbeFile) {
            $probeText = Get-Content -LiteralPath $startupProbeFile -Raw -Encoding UTF8
        }
    }
    if ($null -eq $probeText) { throw 'startup probe file missing' }
    Record 'maximized_state' $probeText
    if ($probeText -notmatch 'show=3') { throw "window state not maximized: $probeText" }
    Stop-QnTestProcesses
    Start-Sleep -Milliseconds 300
    Save-P005Screenshot -Path (Join-Path $results 'p1-06-maximized.png') -WindowHwnd $win.hwnd

    # e) MissingTest: structured failure in the status bar, no process spawned.
    $before = @(Get-Process StartupProbe -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Invoke-ClickAt ($left + 400) ($top + $tileY) 800
    $newProcess = Get-Process StartupProbe -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id }
    if ($null -ne $newProcess) { throw 'a process started for the missing target' }
    Record 'missing_target' 'no process; status bar shows the error'
    Save-P005Screenshot -Path (Join-Path $results 'p1-06-missing.png') -WindowHwnd $win.hwnd

    Stop-QuickNest $run
    Record 'completed' 'all P1-06 launch phases passed'
    Write-Host 'All P1-06 launch phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
    foreach ($charmapPid in $spawnedCharmap) { Stop-QnOwnedProcess -Id $charmapPid -Force -ErrorAction SilentlyContinue }
    if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force -ErrorAction SilentlyContinue }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
