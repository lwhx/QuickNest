#requires -Version 5.1
# P2-04 acceptance (baseline 2.4/5.6): grid internal drag-sorting, insertion
# marker, persisted order, ESC cancel, and click-to-launch still working.
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
$report = Join-Path $results 'p2-04-sorting.tsv'
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

function Read-Order([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
            return (@($config.groups[0].categories[0].items) | ForEach-Object { $_.name }) -join ','
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
$run = Join-Path $tempRoot ('QuickNestP204-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$probeB = Join-Path $run 'sortB.txt'
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$cmd = Join-Path $env:WINDIR 'System32\cmd.exe'
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'SortGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'SortCat'; icon = ''
                    items = @(
                        @{ id = 'sortA'; name = 'SortA'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo a>' + (Join-Path $run 'sortA.txt'))); workdir = $run; run_as_admin = $false } },
                        @{ id = 'sortB'; name = 'SortB'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo b>' + $probeB)); workdir = $run; run_as_admin = $false } },
                        @{ id = 'sortC'; name = 'SortC'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo c>' + (Join-Path $run 'sortC.txt'))); workdir = $run; run_as_admin = $false } }
                    )
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
    Assert-True ((Read-Order $configPath) -eq 'SortA,SortB,SortC') 'unexpected initial order'
    Save-P005Screenshot -Path (Join-Path $results 'p2-04-initial.png') -WindowHwnd $hwnd

    $firstX = $left + 208
    $thirdX = $left + 400
    $rowY = $top + 150

    # a) Drag SortA onto the third slot; order becomes B, C, A.
    [void][P005Desktop]::SetCursorPos($firstX, $rowY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le 15; $i++) {
        [void][P005Desktop]::SetCursorPos(($firstX + [int](($thirdX - $firstX) * $i / 15)), $rowY)
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p2-04-drag-hover.png') -WindowHwnd $hwnd
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 800
    $order = Read-Order $configPath
    Assert-True ($order -eq 'SortB,SortC,SortA') "drag reorder failed: $order"
    Record 'drag_reorder' ("order=$order")
    Save-P005Screenshot -Path (Join-Path $results 'p2-04-after-drop.png') -WindowHwnd $hwnd

    # b) ESC during a drag cancels and leaves the order unchanged.
    [void][P005Desktop]::SetCursorPos($firstX, $rowY)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le 15; $i++) {
        [void][P005Desktop]::SetCursorPos(($firstX + [int](($thirdX - $firstX) * $i / 15)), $rowY)
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 300
    [P005Desktop]::Key(0x1B, $true)
    Start-Sleep -Milliseconds 100
    [P005Desktop]::Key(0x1B, $false)
    Start-Sleep -Milliseconds 250
    Save-P005Screenshot -Path (Join-Path $results 'p2-04-esc-cancel.png') -WindowHwnd $hwnd
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 700
    $order = Read-Order $configPath
    Assert-True ($order -eq 'SortB,SortC,SortA') "ESC cancel changed the order: $order"
    Record 'esc_cancel' 'order unchanged after ESC during drag'

    # c) A normal click (no drag) still launches the first item (SortB).
    $launched = $false
    for ($attempt = 0; $attempt -lt 3 -and -not $launched; $attempt++) {
        Focus-P117Window $hwnd
        [void][P005Desktop]::SetCursorPos($firstX, $rowY)
        Start-Sleep -Milliseconds 120
        [P005Desktop]::MouseButton($true)
        Start-Sleep -Milliseconds 70
        [P005Desktop]::MouseButton($false)
        for ($wait = 0; $wait -lt 20; $wait++) {
            if (Test-Path -LiteralPath $probeB) { $launched = $true; break }
            Start-Sleep -Milliseconds 100
        }
    }
    Assert-True $launched 'click-to-launch stopped working after the drag handler'
    Record 'click_launch' 'SortB launched after the sorting changes'
    Remove-QnSandboxItem -LiteralPath $probeB -Force

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-04 internal-sort phases passed'
    Write-Host 'All P2-04 internal-sort phases passed.'
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
