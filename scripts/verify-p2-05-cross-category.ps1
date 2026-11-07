#requires -Version 5.1
# P2-05 acceptance (baseline 5.6/3.5): drag a grid item onto another category
# row, highlight the target, move and persist; ESC cancels.
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

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-05-cross-category.tsv'
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

function Show-QuickNest([long]$Hwnd) {
    for ($attempt = 0; $attempt -lt 6; $attempt++) {
        if ([P117Focus]::GetForegroundWindow() -eq [IntPtr]$Hwnd) { return }
        [P005Desktop]::Key(0x11, $true)
        [P005Desktop]::Key(0x12, $true)
        [P005Desktop]::Key(0x20, $true)
        [P005Desktop]::Key(0x20, $false)
        [P005Desktop]::Key(0x12, $false)
        [P005Desktop]::Key(0x11, $false)
        Start-Sleep -Milliseconds 300
        Focus-P117Window $Hwnd
    }
    throw 'QuickNest did not take the foreground'
}

function Read-Categories([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
            $categories = @($config.groups[0].categories)
            $catA = (@($categories[0].items) | ForEach-Object { $_.name }) -join ','
            $catB = (@($categories[1].items) | ForEach-Object { $_.name }) -join ','
            return [pscustomobject]@{ CatA = $catA; CatB = $catB }
        } catch {
            if ($attempt -eq 19) { throw }
            Start-Sleep -Milliseconds 100
        }
    }
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP205-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
[IO.File]::WriteAllText((Join-Path $run 'move.txt'), 'move fixture')
[IO.File]::WriteAllText((Join-Path $run 'stay.txt'), 'stay fixture')
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'MoveGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'CatA'; icon = ''
                    items = @(
                        @{ id = 'move-me'; name = 'MoveMe'; item_type = 'file'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = (Join-Path $run 'move.txt'); args = @(); workdir = $run; run_as_admin = $false } },
                        @{ id = 'stay-here'; name = 'StayHere'; item_type = 'file'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = (Join-Path $run 'stay.txt'); args = @(); workdir = $run; run_as_admin = $false } }
                    )
                },
                @{
                    id = 'c2'; name = 'CatB'; icon = ''
                    items = @()
                }
            )
        }
    )
}
[IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exePath
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardError = $true
$psi.RedirectStandardOutput = $true
$app = New-Object System.Diagnostics.Process
$app.StartInfo = $psi
if (-not $app.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $app
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $window = $null
    while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $window = Get-QuickNestWindow $app.Id
    }
    if ($null -eq $window) { throw 'QuickNest window not found' }
    $hwnd = [long]$window.hwnd
    $left = $window.rect.Left
    $top = $window.rect.Top
    Show-QuickNest $hwnd
    Record 'window_ready' ("pid=$($app.Id) rect=$left,$top")

    $initial = Read-Categories $configPath
    Assert-True ($initial.CatA -eq 'MoveMe,StayHere') "unexpected initial CatA: $($initial.CatA)"
    Assert-True ($initial.CatB -eq '') "unexpected initial CatB: $($initial.CatB)"
    Save-P005Screenshot -Path (Join-Path $results 'p2-05-initial.png') -WindowHwnd $hwnd

    $cardX = $left + 208
    $cardY = $top + 150
    # UI 优化后侧边栏几何：行高 36px + 2px 间距；标题栏+分组标签条
    # 约 84px，列表起点 ≈ top+94，第二分类行（CatB）中心 = 94+38+18 = 152。
    $catBX = $left + 70
    $catBY = $top + 150

    # a) Drag MoveMe onto the CatB row; CatB highlights and receives the item.
    [void][P005Desktop]::SetCursorPos($cardX, $cardY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le 20; $i++) {
        [void][P005Desktop]::SetCursorPos(($cardX + [int](($catBX - $cardX) * $i / 20)), ($cardY + [int](($catBY - $cardY) * $i / 20)))
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p2-05-drag-over-category.png') -WindowHwnd $hwnd
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 900
    $moved = Read-Categories $configPath
    Assert-True ($moved.CatA -eq 'StayHere') "source category still has the item: $($moved.CatA)"
    Assert-True ($moved.CatB -eq 'MoveMe') "target category missing the item: $($moved.CatB)"
    Record 'cross_category_move' ("CatA=$($moved.CatA) CatB=$($moved.CatB)")
    Save-P005Screenshot -Path (Join-Path $results 'p2-05-after-move.png') -WindowHwnd $hwnd

    # b) ESC during the same drag keeps both categories unchanged.
    $cardX = $left + 208
    [void][P005Desktop]::SetCursorPos($cardX, $cardY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le 20; $i++) {
        [void][P005Desktop]::SetCursorPos(($cardX + [int](($catBX - $cardX) * $i / 20)), ($cardY + [int](($catBY - $cardY) * $i / 20)))
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 350
    [P005Desktop]::Key(0x1B, $true)
    Start-Sleep -Milliseconds 100
    [P005Desktop]::Key(0x1B, $false)
    Start-Sleep -Milliseconds 250
    Save-P005Screenshot -Path (Join-Path $results 'p2-05-esc-cancel.png') -WindowHwnd $hwnd
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 700
    $unchanged = Read-Categories $configPath
    Assert-True ($unchanged.CatA -eq 'StayHere') "ESC moved the source item: $($unchanged.CatA)"
    Assert-True ($unchanged.CatB -eq 'MoveMe') "ESC changed the target category: $($unchanged.CatB)"
    Record 'esc_cancel' 'categories unchanged after ESC during drag'

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-05 cross-category phases passed'
    Write-Host 'All P2-05 cross-category phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
