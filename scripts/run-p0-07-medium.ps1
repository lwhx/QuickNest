#requires -Version 5.1
[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\run-p0-01-medium-validation.ps1') -LoadLauncherOnly
$run = Join-Path $root ('docs\test-results\P0-07\medium-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$app = Join-Path $root 'target\release\quicknest.exe'
$report = Join-Path $run 'shell.tsv'
$arguments = '--shell-test "' + $report + '"'
$targetPid = [P001MediumLauncher]::Launch($app, $arguments, $root)
Write-Host "Medium target PID=$targetPid"
$process = Get-Process -Id $targetPid
[void] $process.Handle
if (-not $process.WaitForExit(30000)) {
    $process.Kill()
    throw 'Shell STA validation exceeded 30 seconds'
}
$rows = Get-Content -LiteralPath $report -Encoding UTF8
$rows | ForEach-Object { Write-Host $_ }
Write-Host ("Exit=" + $process.ExitCode)
if ($process.ExitCode -ne 0) { throw 'Medium run failed' }
$required = @('service_started', 'create_link', 'generated_resolve', 'real_link_resolve', 'execute_folder', 'service_restarted', 'completed')
foreach ($phase in $required) {
    if (-not @($rows | Where-Object { $_ -match "`t$phase`t" }).Count) {
        throw "Missing required phase: $phase"
    }
}
Write-Host 'All required Shell STA phases present in Medium run.'
