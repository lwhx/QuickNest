#requires -Version 5.1
# P3-01 acceptance (baseline 22 批量启动): items can be picked for a batch
# launch and started together.
#
# The selection is driven through the card context menu and the batch runs
# from the same menu: injected key presses never reach this machine (a
# third-party hook swallows them), so the mouse path is the one that can be
# verified end to end.
#
# 启动探针：每个项目启动 cmd 向自己的探针文件写入标记（与启动次数严格 1:1，
# 同 P3-03 的确定性模式）。不用记事本做探针——Win11 的打包版记事本由存根
# 进程拉起且会预创建宿主窗口，进程数与窗口数都与启动次数不成比例（实测
# 两次启动 = 3 个进程 / 7 个窗口）。
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
. (Join-Path $root 'scripts\p1-17-focus.ps1')
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))
Add-Type @'
using System;using System.Runtime.InteropServices;using System.Text;
public static class P301Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public struct Point{public int X,Y;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h, out Rect r);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
 [DllImport("user32.dll")]public static extern bool GetMenuItemRect(IntPtr hMenu, uint item, out Rect r);
 [DllImport("user32.dll")]public static extern bool ClientToScreen(IntPtr h, ref Point p);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-01-batch.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$probeDir = Join-Path $results 'probes'
if (Test-Path $probeDir) { Remove-QnSandboxItem $probeDir -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDir)

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 600) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::RightButton($true)
    Start-Sleep -Milliseconds 90
    [P005Desktop]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P301Window+EnumProc]{
            param($h, $l)
            if (-not [P301Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P301Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P301Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
# Opens the card context menu and returns the menu window.
function Open-CardMenu([int]$X, [int]$Y) {
    # The panel has the focus before the right click (the way the P2-11 script
    # does it); the popup is the window under the cursor afterwards.
    Focus-P117Window $script:hwnd
    Start-Sleep -Milliseconds 250
    Invoke-RightClickAt $X $Y
    Start-Sleep -Milliseconds 300
    $point = New-Object P301Window+Point
    $point.X = $X
    $point.Y = $Y
    $candidate = [P301Window]::WindowFromPoint($point)
    $menu = 0
    if ($candidate -ne [IntPtr]::Zero) {
        $sb = New-Object System.Text.StringBuilder 64
        [void][P301Window]::GetClassName($candidate, $sb, 64)
        if ($sb.ToString() -eq '#32768') { $menu = [long]$candidate }
    }
    if ($menu -eq 0) { $menu = Wait-MenuWindow 1500 }
    Assert-True ($menu -ne 0) 'card menu did not open'
    return $menu
}
# Clicks the 1-based row of the open menu. The menu is owner-drawn, so the
# row is picked with posted arrow keys; the first arrow selects the first
# entry, so row N needs N presses.
function Click-MenuRow([int]$Row) {
    for ($index = 0; $index -lt $Row; $index++) {
        [P005Keyboard]::Key(0x28)      # Down
        Start-Sleep -Milliseconds 60
    }
    [P005Keyboard]::Key(0x0D)          # Enter
    Start-Sleep -Milliseconds 800
}

$script:hwnd = 0
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$app = $null

Stop-QnTestProcesses
Start-Sleep -Milliseconds 500
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

try {
    # a) Seed three probe items: launching item K writes probe-K.txt.
    $items = @()
    for ($index = 1; $index -le 3; $index++) {
        $probeFile = Join-Path $probeDir "probe-$index.txt"
        $items += @{
            id = "i$index"; name = "Probe $index"; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
            launch = @{
                path = "$env:ComSpec"
                args = @('/c', "echo picked-$index > $probeFile")
                workdir = $probeDir
                run_as_admin = $false; aumid = ''; msi_advertised = $false
            }
        }
    }
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none' }
        groups = @(@{ id='g1'; name='Batch'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=$items }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded' 'three probe items prepared (probes 1-3 absent)'

    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
            if ($window.Count -ge 1) { $window = $window[0] } else { $window = $null }
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        $script:hwnd = $hwnd
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
        if ($window.Count -ge 1) { $window = $window[0] }
        $left = $window.rect.Left
        $top = $window.rect.Top
        Save-P005Screenshot -Path (Join-Path $results 'p3-01-grid.png') -WindowHwnd $hwnd

        # The grid starts after the 160px category bar and the 12px grid
        # padding; a card is inset by 4px and measures 88x100 (medium icons),
        # so the first card spans left+176..left+264 (centre left+220) and
        # the second left+272..left+360 (centre left+316).
        $cardX = $left + 220
        $cardY = $top + 150
        $card2X = $left + 316
        $card2Y = $cardY
        $probe1 = Join-Path $probeDir 'probe-1.txt'
        $probe2 = Join-Path $probeDir 'probe-2.txt'
        $probe3 = Join-Path $probeDir 'probe-3.txt'

        # b) Pick the first and the second card through the context menu.
        # The card menu has thirteen entries (the favorite entry sits at row
        # 8), so row 9 toggles the selection and row 10 starts the batch.
        foreach ($card in @(@($cardX, $cardY), @($card2X, $card2Y))) {
            Open-CardMenu $card[0] $card[1] | Out-Null
            Click-MenuRow 9
            Start-Sleep -Milliseconds 600
            # Close the menu without activating an entry: a click on an empty
            # part of the panel dismisses it.
            Invoke-ClickAt ($left + 700) ($top + 430) 300
        }
        Save-P005Screenshot -Path (Join-Path $results 'p3-01-selected.png') -WindowHwnd $hwnd
        Record 'selected' 'two cards picked through the context menu'

        # c) The batch entry starts every picked item: probes 1+2 appear.
        Open-CardMenu $cardX $cardY | Out-Null
        Click-MenuRow 10
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while ([DateTime]::UtcNow -lt $deadline -and (-not ((Test-Path -LiteralPath $probe1) -and (Test-Path -LiteralPath $probe2)))) {
            Start-Sleep -Milliseconds 200
        }
        Assert-True ((Test-Path -LiteralPath $probe1) -and (Test-Path -LiteralPath $probe2)) 'the batch did not start both picked items'
        Assert-True (-not (Test-Path -LiteralPath $probe3)) 'the unpicked item started'
        Assert-True ((Get-Content -LiteralPath $probe1 -Raw).Contains('picked-1')) 'probe-1 content mismatch'
        Assert-True ((Get-Content -LiteralPath $probe2 -Raw).Contains('picked-2')) 'probe-2 content mismatch'
        Record 'batch_launch' 'probes 1+2 written by the picked items; probe 3 absent'
        Save-P005Screenshot -Path (Join-Path $results 'p3-01-launched.png') -WindowHwnd $hwnd
        $stamp1 = (Get-Item -LiteralPath $probe1).LastWriteTimeUtc
        $stamp2 = (Get-Item -LiteralPath $probe2).LastWriteTimeUtc

        # d) The selection is cleared after the batch, so a second batch
        #    starts nothing (no new probes, existing probes untouched).
        Open-CardMenu $cardX $cardY | Out-Null
        Click-MenuRow 10
        Start-Sleep -Milliseconds 1500
        Assert-True (-not (Test-Path -LiteralPath $probe3)) 'the empty batch started the unpicked item'
        Assert-True ((Get-Item -LiteralPath $probe1).LastWriteTimeUtc -eq $stamp1) 'the empty batch relaunched picked item 1'
        Assert-True ((Get-Item -LiteralPath $probe2).LastWriteTimeUtc -eq $stamp2) 'the empty batch relaunched picked item 2'
        Record 'selection_cleared' 'a second batch starts nothing'

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "exit code $($app.ExitCode)"
        Record 'app_exit' ("exit=$($app.ExitCode)")

        Record 'completed' 'all P3-01 batch phases passed'
        Write-Host 'All P3-01 batch phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
