#requires -Version 5.1
# P1-16 acceptance (baseline 9.1-9.5):
# - A second launch exits immediately (Mutex already held) and creates no
#   second window.
# - Launching again while the first instance is hidden wakes it through the
#   Named Pipe (show_and_focus).
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
$report = Join-Path $results 'p1-16-single-instance.tsv'
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

function Start-QuickNest {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = (Join-Path $root 'target\release\quicknest.exe')
    $psi.WorkingDirectory = $root
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    if (-not $proc.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $proc
    return $proc
}

function Invoke-ClickAt([int]$X, [int]$Y) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 500
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

# a) First instance owns the mutex and opens its window.
$first = Start-QuickNest
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $win = $null
    while ($null -eq $win -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $win = Get-QuickNestWindow $first.Id
    }
    if ($null -eq $win) { throw 'first instance window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 40, 120, 0, 0, 0x0001)
    [void][P005Desktop]::SetForegroundWindow($win.hwnd)
    Start-Sleep -Milliseconds 500
    $win = Get-QuickNestWindow $first.Id
    $hwnd = [long]$win.hwnd
    $left = $win.rect.Left
    $top = $win.rect.Top
    Record 'first_instance' ("pid=$($first.Id) rect=$left,$top")

    # b) A second launch exits immediately without a second window.
    $second = Start-QuickNest
    if (-not $second.WaitForExit(10000)) { (Stop-QnOwnedProcess -Process $second); throw 'the second instance did not exit' }
    $second.WaitForExit()
    if ($second.ExitCode -ne 0) { throw "unexpected second-instance exit code $($second.ExitCode)" }
    $windows = @([P005Desktop]::Windows() | Where-Object { $_.title -eq 'QuickNest' })
    if ($windows.Count -ne 1) { throw "expected exactly one QuickNest window, found $($windows.Count)" }
    Record 'second_exit' ("exit=$($second.ExitCode) windows=$($windows.Count)")

    # c) Hide the first instance, launch again, and it must be woken.
    Invoke-ClickAt ($left + 742) ($top + 22)
    Start-Sleep -Milliseconds 600
    $hidden = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($hidden.Count -ne 0) { throw 'the close button did not hide the first instance' }
    Record 'hidden' 'first instance hidden'
    $third = Start-QuickNest
    if (-not $third.WaitForExit(10000)) { (Stop-QnOwnedProcess -Process $third); throw 'the wake-up launch did not exit' }
    $third.WaitForExit()
    if ($third.ExitCode -ne 0) { throw "unexpected wake-up exit code $($third.ExitCode)" }
    Start-Sleep -Milliseconds 900
    $restored = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($restored.Count -eq 0) { throw 'the second launch did not wake the first instance' }
    Record 'wake_up' ("visible=1 second_exit=$($third.ExitCode)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-16-wake-up.png') -WindowHwnd $hwnd

    # d) Clean shutdown of the first instance.
    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $first.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $first); throw 'first instance did not exit on WM_CLOSE' }
    $first.WaitForExit()
    Record 'first_exit' ("exit=$($first.ExitCode)")
    if ($first.ExitCode -ne 0) { throw "unexpected first-instance exit code $($first.ExitCode)" }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Record 'completed' 'all P1-16 single-instance phases passed'
    Write-Host 'All P1-16 single-instance phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $first.HasExited) { try { (Stop-QnOwnedProcess -Process $first) } catch {} }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
