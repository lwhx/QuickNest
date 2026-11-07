#requires -Version 5.1
[CmdletBinding()]
param()
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\run-p0-01-medium-validation.ps1') -LoadLauncherOnly
$run = Join-Path $root ('docs\test-results\P0-08\medium-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$app = Join-Path $root 'target\release\quicknest.exe'
$report = Join-Path $run 'mutex.tsv'
$arguments = '--mutex-test "' + $report + '"'
$targetPid = [P001MediumLauncher]::Launch($app, $arguments, $root)
Write-Host "Medium target PID=$targetPid"
$process = Get-Process -Id $targetPid
[void] $process.Handle
if (-not $process.WaitForExit(45000)) {
    $process.Kill()
    throw 'Mutex test exceeded 45 seconds'
}
Write-Host ("MediumExit=" + $process.ExitCode + " (70=simulated crash)")
Get-Content -LiteralPath (Join-Path $run 'mutex.tsv') -Encoding UTF8
Get-Content -LiteralPath (Join-Path $run 'mutex.child.tsv') -Encoding UTF8
Get-Content -LiteralPath (Join-Path $run 'mutex.keeper.tsv') -Encoding UTF8
if ($process.ExitCode -ne 70) { throw 'Medium run did not follow the crash scenario' }
$child = Get-Content -LiteralPath (Join-Path $run 'mutex.child.tsv') -Encoding UTF8 | Out-String
$keeper = Get-Content -LiteralPath (Join-Path $run 'mutex.keeper.tsv') -Encoding UTF8 | Out-String
$parent = Get-Content -LiteralPath $report -Encoding UTF8 | Out-String
foreach ($need in @(
    @{ text = 'FIRST_INSTANCE'; where = 'parent' },
    @{ text = 'ALREADY_RUNNING'; where = 'child' },
    @{ text = 'WAIT_TIMEOUT'; where = 'child' },
    @{ text = 'WAIT_ABANDONED'; where = 'keeper' },
    @{ text = 'abandoned_takeover'; where = 'keeper' }
)) {
    $content = switch ($need.where) { 'parent' { $parent } 'child' { $child } 'keeper' { $keeper } }
    if ($content -notmatch [regex]::Escape($need.text)) {
        throw "Missing evidence in $($need.where): $($need.text)"
    }
}
Write-Host 'All required Mutex single-instance phases passed under Medium integrity.'
