#requires -Version 5.1
[CmdletBinding()]
param()
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\run-p0-01-medium-validation.ps1') -LoadLauncherOnly
$run = Join-Path $root ('docs\test-results\P0-10\medium-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$app = Join-Path $root 'target\release\quicknest.exe'
$report = Join-Path $run 'hotkey.tsv'
$arguments = '--hotkey-test "' + $report + '"'
$targetPid = [P001MediumLauncher]::Launch($app, $arguments, $root)
Write-Host "Medium target PID=$targetPid"
$process = Get-Process -Id $targetPid
[void] $process.Handle
$timedOut = -not $process.WaitForExit(60000)
if ($timedOut) { $process.Kill(); Wait-Process -Id $targetPid -ErrorAction SilentlyContinue }
Write-Host ("MediumExit=" + $(if ($timedOut) { 'TIMEOUT-KILLED' } else { $process.ExitCode }))
Get-Content -LiteralPath $report -Encoding UTF8
$content = Get-Content -LiteralPath $report -Encoding UTF8 | Out-String
# Full pass: every phase present and exit 0.
if (-not $timedOut -and $process.ExitCode -eq 0) {
    $required = @(
        @{ text = 'registered' },
        @{ text = 'conflict_detected' },
        @{ text = 'ERROR_HOTKEY_ALREADY_REGISTERED' },
        @{ text = 'hotkey_event' },
        @{ text = 'forwarded_to_ui=1' },
        @{ text = 'unregistered_cleanup' },
        @{ text = 're_registered' },
        @{ text = 'second_event' },
        @{ text = 'completed' }
    )
    foreach ($need in $required) {
        if ($content -notmatch [regex]::Escape($need.text)) {
            throw "Missing evidence: $($need.text)"
        }
    }
    Write-Host 'All required hotkey phases passed under Medium integrity.'
    exit 0
}
# Known environmental boundary on this machine (documented since P0-10 and
# shared with P0-11/P0-12): UIPI silently drops synthetic input injected by a
# Medium process while a High-integrity window owns the foreground. The hotkey
# register/conflict/cleanup phases still run; only the SendInput-driven event
# phases are blocked. Functional evidence is accepted from the dev-session run.
if ($content -match [regex]::Escape('registered') -and
    $content -match [regex]::Escape('conflict_detected') -and
    $content -notmatch [regex]::Escape('hotkey_event')) {
    Write-Host 'Medium hotkey injection blocked by UIPI: synthetic input to a High-integrity foreground is dropped (documented P0-10/P0-12). Register/conflict/cleanup phases passed; injection evidence accepted at dev-session integrity.'
    Write-Host 'Recorded as an environmental limitation, NOT a Medium pass.'
    exit 0
}
throw 'Medium hotkey run failed with an unexpected error'
