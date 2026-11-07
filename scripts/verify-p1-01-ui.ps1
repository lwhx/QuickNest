#requires -Version 5.1
# P1-01 acceptance: frameless window style, title-bar drag moves the window,
# and a screenshot of the five-region layout. Uses the P0-05 desktop helper
# for trusted SendInput/observation; adds GetWindowLong locally.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')

if (-not ('P101Style' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
 public static class P101Style {
 public struct Rect {public int Left,Top,Right,Bottom;}
 public struct Point {public int X,Y;}
 [DllImport("user32.dll",EntryPoint="GetWindowLongW")]public static extern int GetWindowLong(IntPtr h,int index);
 [DllImport("user32.dll")]public static extern bool GetClientRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern bool ClientToScreen(IntPtr h,ref Point p);
 [DllImport("user32.dll")]public static extern bool GetCursorPos(out Point p);
 [DllImport("user32.dll")]public static extern short GetAsyncKeyState(int vkey);
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-01-ui.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

$app = Join-Path $root 'target\release\quicknest.exe'
# Make the whole script thread per-monitor-aware so GetWindowRect / SetCursorPos /
# WindowFromPoint all use physical pixels (the defaults virtualize coordinates and
# misplace the injected drag).
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))
# Start from a clean library so the sample groups match this script.
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$proc = Start-QnOwnedProcess -FilePath $app -WorkingDirectory $root -PassThru
try {
    # Wait for the QuickNest window of this exact process.
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $win = $null
    while ($null -eq $win -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $win = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $proc.Id -and $_.title -like 'QuickNest*' })
        if ($win.Count -gt 0) { $win = $win[0] } else { $win = $null }
    }
    if ($null -eq $win) { throw 'QuickNest window not found in 20s' }
    [void][P005Desktop]::SetForegroundWindow($win.hwnd)
    Start-Sleep -Milliseconds 400
    Record 'window_found' ("pid=$($proc.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")

    # a) Frameless (winit Windows mechanism): WS_CAPTION stays set by design
    #    (aero-snap support); the non-client area is removed via WM_NCCALCSIZE.
    #    The observable proof is the client area covering the window top.
    $style = [P101Style]::GetWindowLong([IntPtr]$win.hwnd, -16)
    $clientRect = New-Object P101Style+Rect
    [void][P101Style]::GetClientRect([IntPtr]$win.hwnd, [ref]$clientRect)
    $clientOrigin = New-Object P101Style+Point
    [void][P101Style]::ClientToScreen([IntPtr]$win.hwnd, [ref]$clientOrigin)
    $nonClientTop = $clientOrigin.Y - $win.rect.Top
    Record "style" ("GWL_STYLE=0x{0:X8}" -f $style)
    Record 'frameless' ("client_top_offset=$nonClientTop (0 means no title bar strip)")
    if ($nonClientTop -gt 2) {
        throw "client area does not cover the window top (offset=$nonClientTop px): decorations still drawn"
    }

    # b) Screenshot of the five-region layout for visual acceptance.
    $shot = Join-Path $results 'p1-01-window.png'
    Save-P005Screenshot -Path $shot -WindowHwnd $win.hwnd
    Record 'screenshot' (Split-Path $shot -Leaf)

    # c) Title-bar drag must move the window (WindowMoveArea). Move QuickNest
    #    to a quiet screen corner first: the invoking terminal often covers the
    #    default centered position, which would swallow the injected drag.
    Add-Type -AssemblyName System.Windows.Forms
    $screenW = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Width
    $screenH = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Height
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 40, ($screenH - 620), 0, 0, 0x0001)
    Start-Sleep -Milliseconds 300
    $before = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $win.hwnd })[0].rect
    $grabX = $before.Left + 300   # blank title-bar area between the name and the buttons
    $grabY = $before.Top + 22     # middle of the 44px title bar
    $hit = [P005Desktop]::WindowFromPoint([P005Desktop+Point]@{ X = $grabX; Y = $grabY })
    $hitRoot = [P005Desktop]::GetAncestor($hit, 2)
    $hitInfo = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hitRoot.ToInt64() })
    $hitTitle = if ($hitInfo.Count -gt 0) { $hitInfo[0].title } else { '<unknown>' }
    Record 'drag_probe' ("hit_root=0x{0:X} title={1}" -f $hitRoot.ToInt64(), $hitTitle)
    [void][P005Desktop]::SetCursorPos($grabX, $grabY)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 120
    $cursorNow = New-Object P101Style+Point
    [void][P101Style]::GetCursorPos([ref]$cursorNow)
    $lbutton = [P101Style]::GetAsyncKeyState(0x01) # VK_LBUTTON
    $lbuttonDown = (($lbutton -band 0x8000) -ne 0)
    Record 'drag_down' ("cursor=($($cursorNow.X),$($cursorNow.Y)) want=($grabX,$grabY) lbutton_down=$lbuttonDown")
    foreach ($step in 1..10) {
        [void][P005Desktop]::SetCursorPos($grabX + 20 * $step, $grabY + 10 * $step)
        Start-Sleep -Milliseconds 30
    }
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 400
    $after = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $win.hwnd })[0].rect
    $dx = $after.Left - $before.Left
    $dy = $after.Top - $before.Top
    Record 'drag' ("dx=$dx dy=$dy (cursor +200,+100)")
    if ([Math]::Abs($dx) -lt 150 -or [Math]::Abs($dy) -lt 60) {
        throw "window did not follow the title-bar drag (dx=$dx dy=$dy)"
    }

    # d) Close via WM_CLOSE for a clean exit.
    Close-QnOwnedWindow -Hwnd ([IntPtr]$win.hwnd)
    if (-not $proc.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $proc); throw 'process did not exit on WM_CLOSE' }
    Record 'wm_close_exit' ("exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }

    Record 'completed' 'all P1-01 UI phases passed'
    Write-Host 'All P1-01 UI acceptance phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $proc.HasExited) { try { (Stop-QnOwnedProcess -Process $proc) } catch {} }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
