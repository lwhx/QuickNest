#requires -Version 5.1
# P1-15 acceptance (baseline 11.3): tray icon registration, left-click toggle,
# menu "显示 QuickNest" and menu "退出程序" (config saved on exit).
# The tray host window is driven by posting the same callback messages that
# Explorer posts (P0-11 pattern), so no notification-area coordinates are
# needed.
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

if (-not ('P115Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P115Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr FindWindowW(string cls,string title);
 [DllImport("user32.dll")]public static extern bool PostMessageW(IntPtr hwnd,uint msg,IntPtr w,IntPtr l);
 public static void Key(ushort vk){
  var down=new Input{type=1,u=new Union{key=new Keyboard{vk=vk}}};
  var up=new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=0x0002}}};
  if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static IntPtr FindTray(){return FindWindowW("QuickNestTrayHost","QuickNest Tray Host");}
 public static bool PostCallback(IntPtr hwnd,uint mouse){
  return PostMessageW(hwnd, 0x8000u+0x5452u, new IntPtr(1), new IntPtr(mouse));
 }
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")]static extern bool GetWindowRect(IntPtr h,out Rect r);
 public static Rect MenuRect(int x,int y){
  var p=new Point{X=x,Y=y};
  var h=WindowFromPoint(p);
  Rect r;
  GetWindowRect(h,out r);
  return r;
 }
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-15-tray.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -eq 'QuickNest' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

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
try {
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
    $hwnd = [long]$win.hwnd
    Record 'window_found' ("pid=$($proc.Id) rect=$($win.rect.Left),$($win.rect.Top)")

    # a) The tray host window exists: registration succeeded.
    $trayHwnd = [P115Input]::FindTray()
    if ($trayHwnd -eq [IntPtr]::Zero) { throw 'tray host window not found (registration failed)' }
    Record 'tray_registered' ("tray_hwnd=0x{0:X}" -f $trayHwnd.ToInt64())
    Save-P005Screenshot -Path (Join-Path $results 'p1-15-initial.png') -WindowHwnd $hwnd

    # b) Left click toggles visibility: hide then show.
    if (-not [P115Input]::PostCallback($trayHwnd, 0x0202)) { throw 'failed to post the left click' }
    Start-Sleep -Milliseconds 1000
    Save-P005Screenshot -Path (Join-Path $results "p1-15-after-left.png")
    $hidden = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($hidden.Count -ne 0) {
        throw 'left click did not hide the window'
    }
    Record 'toggle_hide' 'left click hid the panel'
    if (-not [P115Input]::PostCallback($trayHwnd, 0x0202)) { throw 'failed to post the second left click' }
    Start-Sleep -Milliseconds 1100
    $visible = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($visible.Count -eq 0) { throw 'the second left click did not restore the window' }
    Record 'toggle_show' 'left click restored the panel'
    Save-P005Screenshot -Path (Join-Path $results 'p1-15-restored.png') -WindowHwnd $hwnd

    # c) Menu "隐藏 QuickNest": second entry (popup at 500,300; entries are
    #    ~19px tall starting ~5px below the popup point).
    [void][P005Desktop]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 300
    [void][P005Desktop]::SetCursorPos(500, 300)
    Start-Sleep -Milliseconds 150
    if (-not [P115Input]::PostCallback($trayHwnd, 0x0205)) { throw 'failed to post the right click' }
    Start-Sleep -Milliseconds 900
    Save-P005Screenshot -Path (Join-Path $results "p1-15-menu.png")
    $menuRect = [P115Input]::MenuRect(550, 250)
    Record 'menu_rect' ("$($menuRect.Left),$($menuRect.Top),$($menuRect.Right),$($menuRect.Bottom)")
    $menuItemX = $menuRect.Left + 45
    $menuItemY = $menuRect.Top + [int](($menuRect.Bottom - $menuRect.Top) * 0.25)   # second entry
    [void][P005Desktop]::SetCursorPos($menuItemX, $menuItemY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 80
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 1000
    Save-P005Screenshot -Path (Join-Path $results "p1-15-after-menu-click.png")
    $hidden = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($hidden.Count -ne 0) { throw 'menu hide did not hide the window' }
    Record 'menu_hide' 'menu entry hid the panel'

    # d) Restore through the left click so the exit menu runs with the panel
    #    visible and focused.
    if (-not [P115Input]::PostCallback($trayHwnd, 0x0202)) { throw 'failed to restore before exit' }
    Start-Sleep -Milliseconds 1000
    [void][P005Desktop]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 300
    Record 'restored_before_exit' 'left click restored the panel'

    # e) Menu "退出程序": sixth entry at popup + 5 + 5*19 px.
    [void][P005Desktop]::SetCursorPos(500, 300)
    Start-Sleep -Milliseconds 150
    if (-not [P115Input]::PostCallback($trayHwnd, 0x0205)) { throw 'failed to post the exit right click' }
    Start-Sleep -Milliseconds 900
    $menuRect = [P115Input]::MenuRect(550, 250)
    $exitX = $menuRect.Left + 45
    $exitY = $menuRect.Top + [int](($menuRect.Bottom - $menuRect.Top) * 0.917)   # sixth entry
    [void][P005Desktop]::SetCursorPos($exitX, $exitY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 80
    [P005Desktop]::MouseButton($false)
    if (-not $proc.WaitForExit(10000)) { (Stop-QnOwnedProcess -Process $proc); throw 'menu exit did not terminate the process' }
    Record 'menu_exit' ("exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }
    if (-not (Test-Path $configPath)) { throw 'config was not saved on exit' }
    Record 'config_saved' 'config.json exists after the tray exit'
    $err = $proc.StandardError.ReadToEnd()
    if ($err) { Set-Content -LiteralPath (Join-Path $results 'stderr.log') -Value $err -Encoding UTF8 }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Record 'completed' 'all P1-15 tray phases passed'
    Write-Host 'All P1-15 tray phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $proc.HasExited) { try { (Stop-QnOwnedProcess -Process $proc) } catch {} }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
