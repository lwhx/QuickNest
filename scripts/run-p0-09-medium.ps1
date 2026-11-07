#requires -Version 5.1
[CmdletBinding()]
param()
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\run-p0-01-medium-validation.ps1') -LoadLauncherOnly
$run = Join-Path $root ('docs\test-results\P0-09\medium-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$app = Join-Path $root 'target\release\quicknest.exe'
$report = Join-Path $run 'ipc.tsv'
$arguments = '--ipc-test "' + $report + '"'
$targetPid = [P001MediumLauncher]::Launch($app, $arguments, $root)
Write-Host "Medium target PID=$targetPid"
$process = Get-Process -Id $targetPid
[void] $process.Handle
if (-not $process.WaitForExit(60000)) {
    $process.Kill()
    throw 'IPC test exceeded 60 seconds'
}
Write-Host ("MediumExit=" + $process.ExitCode)
Get-Content -LiteralPath $report -Encoding UTF8
if ($process.ExitCode -ne 0) { throw 'Medium IPC run failed' }
$required = @(
    @{ text = 'acquired before pipe' },
    @{ text = 'show_and_focus' },
    @{ text = 'rejected_frame' },
    @{ text = 'completed' }
)
$content = Get-Content -LiteralPath $report -Encoding UTF8 | Out-String
foreach ($need in $required) {
    if ($content -notmatch [regex]::Escape($need.text)) {
        throw "Missing evidence: $($need.text)"
    }
}
Write-Host 'All required IPC phases passed under Medium integrity.'
