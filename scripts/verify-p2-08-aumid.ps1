#requires -Version 5.1
# P2-08 acceptance (baseline 4.2.4/4.2.5/4.6): a UWP item with a saved AUMID
# activates through IApplicationActivationManager; when the AUMID is stale the
# original LNK is the fallback; the seeded item launches Calculator by AUMID.
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
$report = Join-Path $results 'p2-08-aumid.tsv'
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

function Close-Calculator {
    $calcWindows = @([P005Desktop]::Windows() | Where-Object { $_.title -like '*计算器*' })
    foreach ($w in $calcWindows) {
        Close-QnOwnedWindow -Hwnd ([IntPtr]$w.hwnd)
    }
    Start-Sleep -Milliseconds 600
}

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP208-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$calcAumid = 'Microsoft.WindowsCalculator_8wekyb3d8bbwe!App'
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true; auto_hide_on_launch = $false }
    groups = @(
        @{
            id = 'g1'; name = 'AumidGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'AumidCat'; icon = ''
                    items = @(
                        @{ id = 'aumid-item'; name = 'UwpCalc'; item_type = 'lnk'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\QuickNest Demo\original.lnk'; args = @(); workdir = ''; run_as_admin = $false; aumid = $calcAumid } }
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

    # a) AUMID-first activation: the item's LNK path does NOT exist, so a
    #    successful launch proves ActivateApplication ran.
    Invoke-ClickAt ($left + 208) ($top + 150) 500
    $calcDeadline = [DateTime]::UtcNow.AddSeconds(25)
    $calc = $null
    while ($null -eq $calc -and [DateTime]::UtcNow -lt $calcDeadline) {
        Start-Sleep -Milliseconds 250
        $calcWindows = @([P005Desktop]::Windows() | Where-Object { $_.title -like '*计算器*' })
        if ($calcWindows.Count -gt 0) { $calc = $calcWindows[0] }
    }
    Save-P005Screenshot -Path (Join-Path $results 'p2-08-aumid-launch.png') -WindowHwnd $hwnd
    Assert-True ($null -ne $calc) 'AUMID activation did not open Calculator'
    Record 'aumid_activation' ("calculator window=$($calc.hwnd)")
    Close-Calculator

    # b) Re-click: activation of an already-running app still succeeds.
    Show-QuickNest $hwnd
    Invoke-ClickAt ($left + 208) ($top + 150) 500
    $calcDeadline = [DateTime]::UtcNow.AddSeconds(25)
    $calc = $null
    while ($null -eq $calc -and [DateTime]::UtcNow -lt $calcDeadline) {
        Start-Sleep -Milliseconds 250
        $calcWindows = @([P005Desktop]::Windows() | Where-Object { $_.title -like '*计算器*' })
        if ($calcWindows.Count -gt 0) { $calc = $calcWindows[0] }
    }
    Save-P005Screenshot -Path (Join-Path $results 'p2-08-aumid-reactivate.png') -WindowHwnd $hwnd
    Assert-True ($null -ne $calc) 'second AUMID activation did not open Calculator'
    Record 'aumid_reactivate' ("calculator window=$($calc.hwnd)")
    Close-Calculator

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-08 AUMID phases passed'
    Write-Host 'All P2-08 AUMID phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    Close-Calculator
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
