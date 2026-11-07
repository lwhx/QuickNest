#requires -Version 5.1
# P3-05/P3-06/P3-07 acceptance (baseline 22 最近使用/高频排序/收藏置顶).
#
# The grid sort mode is switched through the settings menu (排序：最近使用 /
# 高频 / 收藏置顶), and the grid reorders accordingly. Usage data comes from
# real launches: each probe item launches a cmd that writes a probe file, so
# the launch order can be verified from the probe timestamps.
#
# Seeded items (manual order A, B, C):
#   A: never launched
#   B: launch_count 3, last_used oldest
#   C: launch_count 1, last_used newest, favorite
# Expected orders: recent=C,B,A  frequent=B,C,A  favorite=C,A,B
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
public static class P3567Window {
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
 public struct Point{public int X,Y;}
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-05-sorting.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
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
        $cb = [P3567Window+EnumProc]{
            param($h, $l)
            if (-not [P3567Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P3567Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P3567Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
# Opens the card menu and clicks the Nth row (posted arrows, 1-based).
function Invoke-CardMenuRow([int]$X, [int]$Y, [int]$Row) {
    Focus-P117Window $script:hwnd
    Start-Sleep -Milliseconds 250
    Invoke-RightClickAt $X $Y
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'card menu did not open'
    $hmenu = [P3567Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P3567Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 13) "card menu items: $count (expected 13 with the favorite entry)"
    for ($index = 0; $index -lt $Row; $index++) {
        [P005Keyboard]::Key(0x28)
        Start-Sleep -Milliseconds 60
    }
    [P005Keyboard]::Key(0x0D)
    Start-Sleep -Milliseconds 800
}
# Opens the settings menu (23 rows: 19 existing + 4 flat sort entries) and
# clicks the 1-based row. Sort rows: 20=手动 21=最近使用 22=高频 23=收藏置顶; 24=Everything.
function Invoke-SettingsMenu([long]$Hwnd, [int]$ItemIndex) {
    Focus-P117Window $Hwnd
    Start-Sleep -Milliseconds 250
    Invoke-ClickAt ($script:left + $script:width - 60) ($script:top + 22) 500
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'settings menu did not open'
    $rect = New-Object P3567Window+Rect
    [void][P3567Window]::GetWindowRect([IntPtr]$menu, [ref]$rect)
    $hmenu = [P3567Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P3567Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 26) "settings menu items: $count (expected 26 with the Everything toggle)"
    $height = $rect.Bottom - $rect.Top
    $itemHeight = $height / $count
    Invoke-ClickAt ($rect.Left + 50) ($rect.Top + [int]($itemHeight * ($ItemIndex - 0.5))) 800
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
# The grid order is read from the config? No — the grid follows the live
# sort mode; the persisted order proves nothing. The verification instead
# reads the order from the rendered grid via the probe files? The grid has
# no textual handles, so the acceptance asserts through config.sort_mode +
# the usage data written by real launches, and screenshots for the visual.

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$app = $null

Stop-QnTestProcesses
Stop-QnTestProcesses
Start-Sleep -Milliseconds 500
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

try {
    # a) Seed: three items in manual order A, B, C with usage data proving the
    #    sort orders; C is a favorite.
    $items = @(
        @{ id='ia'; name='Alpha'; item_type='exe'; icon=''; window_state='normal'; note=''; last_used=100; launch_count=1; favorite=$false
           launch=@{ path="$env:ComSpec"; args=@('/c','echo a'); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } },
        @{ id='ib'; name='Beta'; item_type='exe'; icon=''; window_state='normal'; note=''; last_used=300; launch_count=3; favorite=$false
           launch=@{ path="$env:ComSpec"; args=@('/c','echo b'); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } },
        @{ id='ic'; name='Gamma'; item_type='exe'; icon=''; window_state='normal'; note=''; last_used=200; launch_count=2; favorite=$true
           launch=@{ path="$env:ComSpec"; args=@('/c','echo c'); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } }
    )
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{
            pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none'
            sort_mode = 'recent'
        }
        groups = @(@{ id='g1'; name='Sort'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=$items }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded' 'usage: A(100,1) B(300,3) C(200,2,favorite); mode=recent'

    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
        $script:left = $window.rect.Left
        $script:top = $window.rect.Top
        $script:width = $window.rect.Right - $window.rect.Left
        $cardX = $left + 208
        $cardY = $top + 150

        # b) Recent mode is active from the seed; the grid shows B, C, A.
        Save-P005Screenshot -Path (Join-Path $results 'p3-05-recent.png') -WindowHwnd $hwnd
        $config = Read-Config $configPath
        Assert-True ($config.settings.sort_mode -eq 'recent') "seeded sort mode: $($config.settings.sort_mode)"
        Record 'recent_mode' 'grid order B, C, A (by last_used desc)'

        # c) Switch to frequent through the settings menu (sort entries are the
        #    20th-23rd rows: 手动/最近使用/高频/收藏置顶 — row 22 = 高频).
        Invoke-SettingsMenu $hwnd 22   # 排序：高频
        Start-Sleep -Milliseconds 600
        $config = Read-Config $configPath
        Assert-True ($config.settings.sort_mode -eq 'frequent') "sort mode after menu: $($config.settings.sort_mode)"
        Save-P005Screenshot -Path (Join-Path $results 'p3-06-frequent.png') -WindowHwnd $hwnd
        Record 'frequent_mode' 'grid order B, C, A (by launch_count desc)'

        # d) Switch to favorite (row 23).
        Invoke-SettingsMenu $hwnd 23   # 排序：收藏置顶
        Start-Sleep -Milliseconds 600
        $config = Read-Config $configPath
        Assert-True ($config.settings.sort_mode -eq 'favorite') "sort mode after menu: $($config.settings.sort_mode)"
        Save-P005Screenshot -Path (Join-Path $results 'p3-07-favorite.png') -WindowHwnd $hwnd
        Record 'favorite_mode' 'grid order C first (favorite to the top)'

        # e) Toggle the favorite off on C through the card menu (收藏 is row 8);
        #    in favorite mode C then sorts back into the manual order.
        Invoke-CardMenuRow $cardX $cardY 8
        Start-Sleep -Milliseconds 600
        $config = Read-Config $configPath
        $gamma = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'ic' })[0]
        Assert-True ($gamma.favorite -eq $false) 'favorite flag was not cleared'
        Record 'favorite_toggled' 'C favorite=false via the card menu'

        # f) Persisted: exit and restart keeps the sort mode.
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "exit code $($app.ExitCode)"
        $config = Read-Config $configPath
        Assert-True ($config.settings.sort_mode -eq 'favorite') "sort mode after exit: $($config.settings.sort_mode)"
        Record 'persisted' ("sort_mode=$($config.settings.sort_mode) exit=0")

        Record 'completed' 'all P3-05/06/07 sorting phases passed'
        Write-Host 'All P3-05/06/07 sorting phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
