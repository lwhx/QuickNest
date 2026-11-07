#requires -Version 5.1
# P3-04 acceptance (baseline 22 鼠标热区): hovering the configured screen
# edge toggles the panel.
#
# Flow (hot zone enabled on the left edge, pinned so blur never hides it):
# 1. Panel visible at startup; hovering the left edge for ~1s hides it.
# 2. Hover again after moving away -> panel shows.
# 3. Hot zone disabled -> hovering the edge does nothing.
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

function Get-PanelVisible([int]$ProcessId) {
    return @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -like 'QuickNest*' }).Count -gt 0
}
function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 300) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
# Parks the cursor on the left edge long enough for the dwell (600ms) plus
# the polling interval (200ms).
function Hover-LeftEdge {
    [void][P005Desktop]::SetCursorPos(1, 540)
    Start-Sleep -Milliseconds 1400
}
# Parks the cursor far from the edge so the dwell resets.
function Park-Cursor {
    [void][P005Desktop]::SetCursorPos(900, 540)
    Start-Sleep -Milliseconds 700
}

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-04-hotzone.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$app = $null

Stop-QnTestProcesses
Start-Sleep -Milliseconds 400
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

try {
    # a) Hot zone enabled on the left edge; pinned so blur never hides the
    #    panel during the cursor moves.
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{
            pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none'
            hot_zone_enabled = $true; hot_zone_edge = 'left'
        }
        groups = @(@{ id='g1'; name='HotZone'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=@() }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))

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
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 200, 200, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900

        Park-Cursor
        Assert-True (Get-PanelVisible $app.Id) 'panel is not visible at startup'
        Record 'startup_visible' 'panel visible with the hot zone enabled'

        # b) Hover the left edge: the panel hides.
        Hover-LeftEdge
        Assert-True (-not (Get-PanelVisible $app.Id)) 'hovering the left edge did not hide the panel'
        Record 'edge_hide' 'left-edge hover hid the panel'
        Save-P005Screenshot -Path (Join-Path $results 'p3-04-hidden.png')

        # c) Park away, hover again: the panel shows.
        Park-Cursor
        Hover-LeftEdge
        Assert-True (Get-PanelVisible $app.Id) 'hovering the left edge again did not show the panel'
        Record 'edge_show' 'left-edge hover showed the panel'
        Park-Cursor

        # d) Dwell required: right after the cursor touches the edge the panel
        # is still visible; it hides only after the ~600ms dwell.
        Park-Cursor
        [void][P005Desktop]::SetCursorPos(1, 540)
        Start-Sleep -Milliseconds 200
        Assert-True (Get-PanelVisible $app.Id) 'the panel hid without the dwell'
        Start-Sleep -Milliseconds 1200
        Assert-True (-not (Get-PanelVisible $app.Id)) 'the panel did not hide after the dwell'
        Record 'dwell_required' 'the toggle happens only after the dwell'
        Park-Cursor

        # e) Disable the hot zone: hovering the edge does nothing.
        $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $config.settings.hot_zone_enabled = $false
        [IO.File]::WriteAllText($configPath, ($config | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
        # The settings are read per tick; a restart applies them cleanly.
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "first exit code $($app.ExitCode)"

        $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(20)
            $window = $null
            while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 200
                $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
            }
            if (-not $window) { throw 'QuickNest window not found after restart' }
            Focus-P117Window ([long]$window.hwnd)
            Start-Sleep -Milliseconds 900
            Hover-LeftEdge
            Assert-True (Get-PanelVisible $app.Id) 'the hot zone fired while disabled'
            Record 'disabled_noop' 'hover does nothing when disabled'

            Close-QnOwnedWindow -Hwnd ([IntPtr]$window.hwnd)
            if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
            $app.WaitForExit()
            Assert-True ($app.ExitCode -eq 0) "second exit code $($app.ExitCode)"
            Record 'app_exit' "exit=$($app.ExitCode)"

            Record 'completed' 'all P3-04 hot-zone phases passed'
            Write-Host 'All P3-04 hot-zone phases passed.'
            Get-Content -LiteralPath $report -Encoding UTF8
        } finally {
            if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
        }
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    # Park the cursor away from the edge so later runs are not affected.
    [void][P005Desktop]::SetCursorPos(900, 540)
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
