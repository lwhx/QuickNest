#requires -Version 5.1
# P4-10 acceptance (baseline 29 章 P4-10 数据迁移测试).
#
# 场景：v1.1 时代的 config（schema_version=2，无 P3/P4 新增字段——launch 无
# delay_seconds/execution_mode、settings 无 sort_mode/everything、item 无
# usage/favorite）→ 当前版本启动 → 用户数据完整（分组/分类/项目名与路径）
# → 触发一次保存（重命名分组）→ config 断言：schema_version 仍 2、用户数据
# 未丢、新字段以默认值写入 → 启动一个迁移后的项目（相对探针）证明可用。
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
public static class P410Window {
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 public struct Point{public int X,Y;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p4-10-migration.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'

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
function Wait-True([scriptblock]$Condition, [int]$TimeoutSeconds, [string]$Message) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 250
    }
    throw $Message
}
function Set-FieldText([int]$Left, [int]$Top, [int]$X, [int]$Y, [string]$Text) {
    Invoke-ClickAt ($Left + $X) ($Top + $Y) 200
    [P005Keyboard]::SelectAll()
    [P005Keyboard]::Key(0x2E)
    Start-Sleep -Milliseconds 120
    [P005Keyboard]::TypeText($Text)
    Start-Sleep -Milliseconds 200
}
function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P410Window+EnumProc]{
            param($h, $l)
            if (-not [P410Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P410Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P410Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}

Stop-QnTestProcesses
Start-Sleep -Milliseconds 500
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

try {
    # a) 种 v1.1 形状 config（无 P3/P4 新增字段；组/分类/项目为真实旧数据）。
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ theme = 'light'; accent_color = '#2563eb'; pinned = $true }
        groups = @(@{
            id = 'g1'; name = '迁移测试分组'; color = '#3B82F6'; last_category_id = 'c1'
            categories = @(@{
                id = 'c1'; name = '迁移测试分类'
                items = @(
                    @{ id = 'i1'; name = '记事本'; item_type = 'exe'; icon = ''; window_state = 'normal'
                       launch = @{ path = 'C:\Windows\System32\notepad.exe'; args = @(); workdir = ''; run_as_admin = $false } },
                    @{ id = 'i2'; name = '示例链接'; item_type = 'uri'; icon = ''; window_state = 'normal'
                       launch = @{ path = 'https://example.com'; args = @(); workdir = ''; run_as_admin = $false } }
                )
            })
        })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded_v1_1' 'legacy config without P3/P4 fields (usage/sort/hot-zone/everything/delay/execution-mode)'

    # b) 当前版本启动：用户数据完整呈现。
    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
            if ($window.Count -ge 1) { $window = $window[0] } else { $window = $null }
        }
        if (-not $window) { throw 'window not found' }
        $hwnd = [long]$window.hwnd
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900
        Save-P005Screenshot -Path (Join-Path $results 'p4-10-loaded.png') -WindowHwnd $hwnd
        $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
        Assert-True ($config.groups[0].name -eq '迁移测试分组') 'group name lost on load'
        Assert-True (@($config.groups[0].categories[0].items).Count -eq 2) 'items lost on load'
        Assert-True ($config.settings.theme -eq 'light') 'theme lost on load'
        Record 'loaded' 'legacy config loaded; user data intact (group/category/2 items/theme)'

        # c) 触发一次保存（重命名分组 via 右键菜单 → 编辑框 → 保存）。
        Focus-P117Window $hwnd
        Invoke-RightClickAt (60 + 30) (60 + 64) 600
        $menu = Wait-MenuWindow 5000
        Assert-True ($menu -ne 0) 'group context menu did not open'
        for ($i = 0; $i -lt 2; $i++) { [P005Keyboard]::Key(0x28); Start-Sleep -Milliseconds 60 }
        [P005Keyboard]::Key(0x0D)
        Start-Sleep -Milliseconds 800
        Set-FieldText 60 60 180 25 '迁移后分组'
        Save-P005Screenshot -Path (Join-Path $results 'p4-10-rename.png') -WindowHwnd $hwnd
        Invoke-ClickAt (60 + 498) (60 + 314) 1200
        Wait-True {
            $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
            $json -match '迁移后分组'
        } 15 'renamed group not persisted'
        Record 'save_triggered' 'group renamed (a write happened through the save chain)'

        # d) 迁移断言：schema_version 仍 2、用户数据未丢、新字段默认值写入。
        $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
        Assert-True ($config.schema_version -eq 2) ('schema_version: ' + $config.schema_version)
        Assert-True ($config.groups[0].name -eq '迁移后分组') 'renamed group missing'
        $item1 = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'i1' })[0]
        Assert-True ($item1.launch.path -eq 'C:\Windows\System32\notepad.exe') 'item path lost'
        Assert-True ($item1.launch.delay_seconds -eq 0) ('delay_seconds: ' + $item1.launch.delay_seconds)
        Assert-True ($item1.launch.execution_mode -eq 'direct') ('execution_mode: ' + $item1.launch.execution_mode)
        Assert-True ($item1.last_used -eq 0) 'last_used unexpected'
        Assert-True ($item1.launch_count -eq 0) 'launch_count unexpected'
        Assert-True (-not $item1.favorite) 'favorite unexpected'
        Assert-True ($config.settings.sort_mode -eq 'manual') ('sort_mode: ' + $config.settings.sort_mode)
        Assert-True ($null -eq $config.settings.everything_enabled -or $config.settings.everything_enabled -eq $false) 'everything_enabled unexpected'
        Record 'migrated' 'schema_version=2; user data intact; new fields defaulted (delay/execution/usage/sort)'

        # e) 迁移后的项目可正常启动（可用性证明；记事本窗口计数 1）。
        # 窗口位置以实际枚举为准（P2-15 记忆会覆盖 SetWindowPos）。
        $final = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
        if ($final.Count -ge 1) {
            $left = $final[0].rect.Left
            $top = $final[0].rect.Top
        }
        Record 'click_rect' ('left=' + $left + ' top=' + $top)
        # 输入容错：防护组件会间歇吞掉注入的鼠标事件（本会话已知问题），
        # 未启动则聚焦后重试，最多 3 次。
        $launched = $false
        for ($attempt = 1; $attempt -le 3 -and -not $launched; $attempt++) {
            Focus-P117Window $hwnd
            Start-Sleep -Milliseconds 500
            Invoke-ClickAt ($left + 220) ($top + 150) 2500
            if ((@(Get-Process notepad -ErrorAction SilentlyContinue)).Count -ge 1) {
                $launched = $true
            } else {
                Record 'launch_retry' ('attempt ' + $attempt + ' did not start notepad')
            }
        }
        Assert-True $launched 'migrated item did not launch after retries'
        Save-P005Screenshot -Path (Join-Path $results 'p4-10-after-click.png') -WindowHwnd $hwnd
        Stop-QnTestProcesses
        Record 'migrated_item_launches' 'the migrated item starts normally'

        # f) 退出（保存链收尾）。
        Focus-P117Window $hwnd
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'no exit on WM_CLOSE' }
        Assert-True ($app.ExitCode -eq 0) ('exit code ' + $app.ExitCode)
        Record 'app_exit' ('exit=' + $app.ExitCode)
        Record 'completed' 'all P4-10 migration phases passed'
        Write-Host 'All P4-10 migration phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if ($app -and -not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
