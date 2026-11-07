#requires -Version 5.1
[CmdletBinding()]
param()
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\run-p0-01-medium-validation.ps1') -LoadLauncherOnly
$run = Join-Path $root ('docs\test-results\P0-13\medium-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$app = Join-Path $root 'target\release\quicknest.exe'
$report = Join-Path $run 'save.tsv'
$arguments = '--save-test "' + $report + '"'
$targetPid = [P001MediumLauncher]::Launch($app, $arguments, $root)
Write-Host "Medium target PID=$targetPid"
$process = Get-Process -Id $targetPid
[void] $process.Handle
if (-not $process.WaitForExit(30000)) {
    $process.Kill()
    throw 'Save test exceeded 30 seconds'
}
Write-Host ("MediumExit=" + $process.ExitCode)
Get-Content -LiteralPath $report -Encoding UTF8
if ($process.ExitCode -ne 0) { throw 'Medium save run failed' }
$required = @(
    @{ text = 'first_create' },
    @{ text = 'atomic_replace' },
    @{ text = 'failure_protection' },
    @{ text = 'temp_lifecycle' },
    @{ text = 'backup_recovery' },
    @{ text = 'safe_mode' },
    @{ text = 'completed' }
)
$content = Get-Content -LiteralPath $report -Encoding UTF8 | Out-String
foreach ($need in $required) {
    if ($content -notmatch [regex]::Escape($need.text)) {
        throw "Missing evidence: $($need.text)"
    }
}
Write-Host 'All required save phases passed under Medium integrity.'
