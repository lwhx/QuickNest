#requires -Version 5.1
# P2-06 acceptance (baseline 5.6/2.2): cross-group drag — hover a group tab
# for 400 ms spring-loads it; a fast drop targets the group; ESC cancels.
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
$report = Join-Path $results 'p2-06-cross-group.tsv'
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

function Read-Groups([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
            $groupA = @($config.groups | Where-Object { $_.name -eq 'GroupA' })[0]
            $groupB = @($config.groups | Where-Object { $_.name -eq 'GroupB' })[0]
            return [pscustomobject]@{
                Current = [string]$config.current_group_id
                CatA = (@($groupA.categories[0].items) | ForEach-Object { $_.name }) -join ','
                CatB = (@($groupB.categories[0].items) | ForEach-Object { $_.name }) -join ','
            }
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
$run = Join-Path $tempRoot ('QuickNestP206-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

function Make-Item([string]$Id, [string]$Name) {
    return @{ id = $Id; name = $Name; item_type = 'file'; icon = ''; window_state = 'normal'; note = ''
              launch = @{ path = (Join-Path $run "$Id.txt"); args = @(); workdir = $run; run_as_admin = $false } }
}
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'GroupA'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'CatA1'; icon = ''
                    items = @(
                        (Make-Item 'cross' 'CrossItem'),
                        (Make-Item 'fast' 'FastItem'),
                        (Make-Item 'keep' 'KeepItem')
                    )
                }
            )
        },
        @{
            id = 'g2'; name = 'GroupB'; icon = ''; color = ''; last_category_id = 'c2'
            categories = @(
                @{ id = 'c2'; name = 'CatB1'; icon = ''; items = @() }
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
    [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 40, 120, 760, 520, 0x0050)
    Show-QuickNest $hwnd
    Start-Sleep -Milliseconds 400
    $window = Get-QuickNestWindow $app.Id
    $left = $window.rect.Left
    $top = $window.rect.Top
    Record 'window_ready' ("pid=$($app.Id) rect=$left,$top")
    $initial = Read-Groups $configPath
    Assert-True ($initial.CatA -eq 'CrossItem,FastItem,KeepItem') "initial CatA: $($initial.CatA)"
    Assert-True ($initial.CatB -eq '') "initial CatB: $($initial.CatB)"
    Assert-True ($initial.Current -eq 'g1') "initial group: $($initial.Current)"

    $cardX = $left + 208
    $cardY = $top + 150
    $tabBX = $left + 134
    $tabBY = $top + 64
    $tabAX = $left + 47

    function Invoke-CardDragTo([int]$ToX, [int]$ToY, [int]$HoldMs, [string]$Capture) {
        [void][P005Desktop]::SetCursorPos($cardX, $cardY)
        Start-Sleep -Milliseconds 150
        [P005Desktop]::MouseButton($true)
        Start-Sleep -Milliseconds 150
        $steps = 15
        for ($i = 1; $i -le $steps; $i++) {
            [void][P005Desktop]::SetCursorPos(($cardX + [int](($ToX - $cardX) * $i / $steps)), ($cardY + [int](($ToY - $cardY) * $i / $steps)))
            Start-Sleep -Milliseconds 20
        }
        if ($HoldMs -gt 0) {
            Start-Sleep -Milliseconds $HoldMs
            if ($Capture) { Save-P005Screenshot -Path (Join-Path $results $Capture) -WindowHwnd $hwnd }
        }
        [P005Desktop]::MouseButton($false)
        Start-Sleep -Milliseconds 800
    }

    # a) Spring-load: hover GroupB for 700 ms (switches at 400 ms), release on
    #    the tab -> CrossItem moves to GroupB's active category.
    Invoke-CardDragTo $tabBX $tabBY 700 'p2-06-spring-hover.png'
    $state = Read-Groups $configPath
    Assert-True ($state.CatA -eq 'FastItem,KeepItem') "spring CatA: $($state.CatA)"
    Assert-True ($state.CatB -eq 'CrossItem') "spring CatB: $($state.CatB)"
    Assert-True ($state.Current -eq 'g2') "spring group: $($state.Current)"
    Record 'spring_load' ("CatA=$($state.CatA) CatB=$($state.CatB) current=g2")
    Save-P005Screenshot -Path (Join-Path $results 'p2-06-after-spring.png') -WindowHwnd $hwnd

    # b) Fast drop: drag FastItem to the tab and release within 400 ms.
    Show-QuickNest $hwnd
    [void][P005Desktop]::SetCursorPos($tabAX, $tabBY)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p2-06-back-to-a.png') -WindowHwnd $hwnd
    [void][P005Desktop]::SetCursorPos($cardX, $cardY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 120
    [void][P005Desktop]::SetCursorPos($tabBX, $tabBY)
    Start-Sleep -Milliseconds 100
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p2-06-fast-drop-moment.png') -WindowHwnd $hwnd
    Start-Sleep -Milliseconds 500
    $state = Read-Groups $configPath
    Assert-True ($state.CatA -eq 'KeepItem') "fast CatA: $($state.CatA)"
    Assert-True ($state.CatB -eq 'CrossItem,FastItem') "fast CatB: $($state.CatB)"
    Record 'fast_drop' ("CatA=$($state.CatA) CatB=$($state.CatB)")
    Save-P005Screenshot -Path (Join-Path $results 'p2-06-after-fast-drop.png') -WindowHwnd $hwnd

    # c) ESC during the hover cancels the move (the spring switch persists).
    #    The button stays down across the ESC so the release commits nothing.
    Show-QuickNest $hwnd
    [void][P005Desktop]::SetCursorPos($tabAX, $tabBY)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 500
    [void][P005Desktop]::SetCursorPos($cardX, $cardY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 150
    $steps = 15
    for ($i = 1; $i -le $steps; $i++) {
        [void][P005Desktop]::SetCursorPos(($cardX + [int](($tabBX - $cardX) * $i / $steps)), ($cardY + [int](($tabBY - $cardY) * $i / $steps)))
        Start-Sleep -Milliseconds 20
    }
    Start-Sleep -Milliseconds 600
    Save-P005Screenshot -Path (Join-Path $results 'p2-06-esc-cancel.png') -WindowHwnd $hwnd
    [P005Desktop]::Key(0x1B, $true)
    Start-Sleep -Milliseconds 100
    [P005Desktop]::Key(0x1B, $false)
    Start-Sleep -Milliseconds 250
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 800
    $state = Read-Groups $configPath
    Assert-True ($state.CatA -eq 'KeepItem') "esc CatA: $($state.CatA)"
    Assert-True ($state.CatB -eq 'CrossItem,FastItem') "esc CatB: $($state.CatB)"
    Assert-True ($state.Current -eq 'g2') "esc group: $($state.Current)"
    Record 'esc_cancel' ("CatA=$($state.CatA) CatB=$($state.CatB) current=g2")

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-06 cross-group phases passed'
    Write-Host 'All P2-06 cross-group phases passed.'
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
