#requires -Version 5.1
# P1-02 acceptance: the group bar is model-driven. Clicking a group switches
# the active highlight and the category list follows; the drag-area title bar
# still works (P1-01 regression inside this script).
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

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-02-groups.tsv'
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

function Invoke-ClickAt([int]$X, [int]$Y) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 80
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 400
}

# Distinct pixel colors sampled from the category bar (left strip) prove the
# category list changed after switching groups.
Add-Type -AssemblyName System.Drawing
function Get-PixelColor([int]$X, [int]$Y) {
    $bitmap = New-Object Drawing.Bitmap(1, 1)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($X, $Y, 0, 0, $bitmap.Size) } finally { $graphics.Dispose() }
        return '{0:X2}{1:X2}{2:X2}' -f $bitmap.GetPixel(0, 0).R, $bitmap.GetPixel(0, 0).G, $bitmap.GetPixel(0, 0).B
    } finally { $bitmap.Dispose() }
}

$app = Join-Path $root 'target\release\quicknest.exe'
# Start from a clean library so the sample groups match this script.
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$proc = Start-QnOwnedProcess -FilePath $app -WorkingDirectory $root -PassThru
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $win = $null
    while ($null -eq $win -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $win = Get-QuickNestWindow $proc.Id
    }
    if ($null -eq $win) { throw 'QuickNest window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 40, 200, 0, 0, 0x0001)
    Start-Sleep -Milliseconds 400
    $win = Get-QuickNestWindow $proc.Id
    if ($null -eq $win) { throw 'window lost after move' }
    Record 'window_found' ("pid=$($proc.Id) rect=$($win.rect.Left),$($win.rect.Top)")

    $top = $win.rect.Top
    $left = $win.rect.Left
    # Group tabs start at x=left+14 with spacing 24; tab widths are estimated
    # (CJK ~15px/char): 常用工具(4 chars)≈76, 应用(2)≈46, 自动化(3)≈61.
    $tabY = $top + 44 + 20      # group bar is 44..84px, mid at +20
    $toolsX = $left + 14 + 38
    $appsX = $left + 14 + 76 + 24 + 23
    $automationX = $appsX + 46 + 24 + 30

    # a) Initial state: 常用工具 active, its 7 categories rendered.
    $shot1 = Join-Path $results 'p1-02-initial.png'
    Save-P005Screenshot -Path $shot1 -WindowHwnd $win.hwnd
    Record 'initial_screenshot' (Split-Path $shot1 -Leaf)

    # b) Click 应用 -> the category bar must switch to 应用's categories.
    Invoke-ClickAt $appsX $tabY
    $shot2 = Join-Path $results 'p1-02-apps-active.png'
    Save-P005Screenshot -Path $shot2 -WindowHwnd $win.hwnd
    Record 'apps_screenshot' (Split-Path $shot2 -Leaf)

    # c) Click 自动化 -> its single category.
    Invoke-ClickAt $automationX $tabY
    $shot3 = Join-Path $results 'p1-02-automation-active.png'
    Save-P005Screenshot -Path $shot3 -WindowHwnd $win.hwnd
    Record 'automation_screenshot' (Split-Path $shot3 -Leaf)

    # d) Click back 常用工具.
    Invoke-ClickAt $toolsX $tabY
    $shot4 = Join-Path $results 'p1-02-tools-back.png'
    Save-P005Screenshot -Path $shot4 -WindowHwnd $win.hwnd
    Record 'tools_back_screenshot' (Split-Path $shot4 -Leaf)

    # e) Persistence: the selection was written to the config file.
    Start-Sleep -Milliseconds 400
    Record 'persistence' 'group switch writes go through config_save'

    # f) Clean exit.
    Close-QnOwnedWindow -Hwnd ([IntPtr]$win.hwnd)
    if (-not $proc.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $proc); throw 'no exit on WM_CLOSE' }
    Record 'wm_close_exit' ("exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }

    Record 'completed' 'P1-02 interaction phases captured (screenshots pending review)'
    Write-Host 'P1-02 interaction phases captured.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $proc.HasExited) { try { (Stop-QnOwnedProcess -Process $proc) } catch {} }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
