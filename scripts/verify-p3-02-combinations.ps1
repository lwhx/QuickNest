#requires -Version 5.1
# P3-02 acceptance (baseline 22 启动组合): a batch selection can be saved as
# a named launch combination, persisted with the configuration, launched as
# a unit and deleted.
#
# Flow: two cards are picked through the card context menu, "保存批量选择为
# 组合…" opens a name dialog (pre-filled), confirming saves the combination;
# "启动组合…" opens the combination dialog whose 启动组合 button launches the
# saved items and 删除 removes the combination.
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
public static class P302Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
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
 public struct Point{public int X,Y;}
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-02-combinations.tsv'
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
        $cb = [P302Window+EnumProc]{
            param($h, $l)
            if (-not [P302Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P302Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P302Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
function Open-CardMenu([int]$X, [int]$Y) {
    Focus-P117Window $script:hwnd
    Start-Sleep -Milliseconds 250
    Invoke-RightClickAt $X $Y
    $point = New-Object P302Window+Point
    $point.X = $X
    $point.Y = $Y
    $candidate = [P302Window]::WindowFromPoint($point)
    $menu = 0
    if ($candidate -ne [IntPtr]::Zero) {
        $sb = New-Object System.Text.StringBuilder 64
        [void][P302Window]::GetClassName($candidate, $sb, 64)
        if ($sb.ToString() -eq '#32768') { $menu = [long]$candidate }
    }
    if ($menu -eq 0) { $menu = Wait-MenuWindow 1500 }
    Assert-True ($menu -ne 0) 'card menu did not open'
    $hmenu = [P302Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P302Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 13) "card menu items: $count (expected the 8 baseline entries plus the favorite and 4 batch/combination entries)"
    # The row is picked with posted arrow keys (the menu is owner-drawn, so
    # its text cannot be read back); row N needs N presses.
    return @{ Window = $menu; Handle = $hmenu; Count = $count }
}
function Click-MenuRow([hashtable]$Menu, [int]$Row) {
    for ($index = 0; $index -lt $Row; $index++) {
        [P005Keyboard]::Key(0x28)      # Down
        Start-Sleep -Milliseconds 60
    }
    [P005Keyboard]::Key(0x0D)          # Enter
    Start-Sleep -Milliseconds 800
}
function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 25; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json } catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}

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
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
        if ($window.Count -ge 1) { $window = $window[0] }
        $left = $window.rect.Left
        $top = $window.rect.Top
        # The grid starts after the 160px category bar and the 12px grid
        # padding; a card is inset by 4px and measures 88x100, so the first
        # card's centre is at (left + 220, top + 150) and the second's at
        # (left + 316, top + 150).
        $cardX = $left + 220
        $cardY = $top + 150
        $card2X = $left + 316
        $card2Y = $cardY
        $probe1 = Join-Path $probeDir 'probe-1.txt'
        $probe2 = Join-Path $probeDir 'probe-2.txt'
        $probe3 = Join-Path $probeDir 'probe-3.txt'

        # b) Pick the first two cards through the context menu.
        foreach ($card in @(@($cardX, $cardY), @($card2X, $card2Y))) {
            $menu = Open-CardMenu $card[0] $card[1]
            Click-MenuRow $menu 9          # 加入批量选择
            Start-Sleep -Milliseconds 600
            Invoke-ClickAt ($left + 700) ($top + 430) 300
        }
        Record 'selected' 'two cards picked'

        # c) Save the batch selection as a combination through the name dialog
        #    (pre-filled with "组合 1", so confirming without typing works).
        $menu = Open-CardMenu $cardX $cardY
        Click-MenuRow $menu 11          # 保存批量选择为组合…
        Start-Sleep -Milliseconds 900
        Save-P005Screenshot -Path (Join-Path $results 'p3-02-name-dialog.png') -WindowHwnd $hwnd
        # The name dialog's 保存 button sits bottom-right of the 320x140 dialog
        # centered at (left+220, top+190).
        Invoke-ClickAt ($left + 508) ($top + 314) 1200
        $config = Read-Config $configPath
        Assert-True (@($config.combinations).Count -eq 1) "combinations in config: $(@($config.combinations).Count)"
        $combination = @($config.combinations)[0]
        Assert-True (@($combination.item_ids).Count -eq 2) "combination items: $(@($combination.item_ids).Count)"
        Record 'combination_saved' ("name=$($combination.name) items=$(@($combination.item_ids).Count)")

        # d) Launch the combination through the combination dialog: the dialog
        #    is already open after saving; select the row and press 启动组合.
        # The name-dialog interaction can shift focus; bring the panel back to
        # the topmost band before driving the combination dialog.
        Focus-P117Window $hwnd
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), $left, $top, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 500
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
        if ($window.Count -ge 1) { $window = $window[0] }
        $left = $window.rect.Left
        $top = $window.rect.Top
        Record 'panel_rect' ("left=$left top=$top")
        Invoke-ClickAt ($left + 355) ($top + 286) 400      # select row 1
        Invoke-ClickAt ($left + 195) ($top + 389) 2000     # 启动组合
        $deadline = [DateTime]::UtcNow.AddSeconds(10)
        while ([DateTime]::UtcNow -lt $deadline -and (-not ((Test-Path -LiteralPath $probe1) -and (Test-Path -LiteralPath $probe2)))) {
            Start-Sleep -Milliseconds 200
        }
        Assert-True ((Test-Path -LiteralPath $probe1) -and (Test-Path -LiteralPath $probe2)) 'the combination did not start both of its items'
        Assert-True ((Get-Content -LiteralPath $probe1 -Raw).Contains('picked-1')) 'probe-1 content mismatch'
        Assert-True ((Get-Content -LiteralPath $probe2 -Raw).Contains('picked-2')) 'probe-2 content mismatch'
        Record 'combination_launched' 'probes 1+2 written by the combination items'
        Save-P005Screenshot -Path (Join-Path $results 'p3-02-launched.png') -WindowHwnd $hwnd

        # e) Delete the combination through the dialog's 删除 button.
        Invoke-ClickAt ($left + 355) ($top + 286) 400      # select row 1
        Save-P005Screenshot -Path (Join-Path $results 'p3-02-before-delete.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($left + 270) ($top + 389) 1200     # 删除
        Start-Sleep -Milliseconds 1500
        Save-P005Screenshot -Path (Join-Path $results 'p3-02-tmp-after-delete.png')
        $config = Read-Config $configPath
        Assert-True (@($config.combinations).Count -eq 0) "combinations after delete: $(@($config.combinations).Count)"
        Record 'combination_deleted' 'config no longer holds the combination'

        # f) Cancel closes the dialog; the deletion persists after exit.
        Invoke-ClickAt ($left + 570) ($top + 389) 500      # 取消
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "exit code $($app.ExitCode)"
        Record 'app_exit' ("exit=$($app.ExitCode)")

        Record 'completed' 'all P3-02 combination phases passed'
        Write-Host 'All P3-02 combination phases passed.'
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
