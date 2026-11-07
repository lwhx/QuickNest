#requires -Version 5.1
param(
    [string]$ResultsDir = '',
    [string[]]$Only = @(),
    [switch]$AllowIsolatedDesktop,
    [string]$DisposableAccount = '',
    [switch]$PreflightOnly
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\test-sandbox.ps1')
$savedEnvironment = Get-QnEnvironmentSnapshot
$failed = 0
$report = $null
try {
    $scripts = @(Get-ChildItem -LiteralPath (Join-Path $root 'scripts') -Filter 'verify-p*.ps1' | Sort-Object Name)
    if ($Only.Count -gt 0) {
        $unknown = @($Only | Where-Object { $_ -notin $scripts.Name })
        if ($unknown.Count) { throw ('unknown verify scripts: ' + ($unknown -join ', ')) }
        $scripts = @($scripts | Where-Object { $Only -contains $_.Name })
    }
    if ($scripts.Count -eq 0) { throw 'no verify scripts found' }
    if ($PreflightOnly) {
        & (Join-Path $root 'scripts\verify-sandbox-preflight.ps1')
    } else {
        # Refuse obsolete selections before allocating a run, acknowledging a
        # desktop, or launching any child. BLOCKED must never become an exit-0 row.
        $blocked = @()
        foreach ($candidate in $scripts) {
            $tokens = $null; $errors = $null
            $ast = [Management.Automation.Language.Parser]::ParseFile($candidate.FullName, [ref]$tokens, [ref]$errors)
            if ($errors.Count) { throw ('script parse failed: ' + $candidate.Name) }
            $reason = Get-QnScriptBlockReason $ast
            if ($reason) { $blocked += ($candidate.Name + ': ' + $reason) }
        }
        if ($blocked.Count) { throw ($blocked -join "`r`n") }
        if ($AllowIsolatedDesktop) { $env:QUICKNEST_TEST_ISOLATED_DESKTOP = '1' }
        if ($DisposableAccount) { $env:QUICKNEST_TEST_DISPOSABLE_ACCOUNT = $DisposableAccount }
        Assert-QnDisposableDesktop
        # The caller's SANDBOX_ROOT is a container, never replaced or removed.
        # Every invocation allocates a new run, even if a previous run was supplied.
        $env:QUICKNEST_TEST_RUN_ROOT = New-QnSandboxRun $env:QUICKNEST_TEST_SANDBOX_ROOT
        $script:QnRunRoot = $env:QUICKNEST_TEST_RUN_ROOT
        $script:QnOwnedProcesses = @{}
        if ([string]::IsNullOrWhiteSpace($ResultsDir)) { $ResultsDir = Join-Path $script:QnRunRoot 'results' }
        [void](Assert-QnContainedPath $ResultsDir $script:QnRunRoot)
        [void][IO.Directory]::CreateDirectory($ResultsDir)
        $logDir = Join-Path $ResultsDir 'logs'
        [void][IO.Directory]::CreateDirectory($logDir)
        $report = Join-Path $ResultsDir 'regression.tsv'
        if (Test-Path -LiteralPath $report) { Remove-QnSandboxItem -LiteralPath $report -Force }
        foreach ($script in $scripts) {
            $name = $script.Name
            $code = 1
            $process = $null
            $stopwatch = [Diagnostics.Stopwatch]::StartNew()
            $logPath = Join-Path $logDir ($name + '.log')
            try {
                # A verify script outside this contract must first be migrated;
                # notably update scripts are maintained by a separate workstream.
                $source = [IO.File]::ReadAllText($script.FullName)
                if ($source -notmatch 'Initialize-QnTestSandbox') { throw 'script has no audited sandbox initialization; refusing execution' }
                $tokens = $null; $errors = $null
                [void][Management.Automation.Language.Parser]::ParseFile($script.FullName, [ref]$tokens, [ref]$errors)
                if ($errors.Count) { throw ('script parse failed: ' + ($errors -join '; ')) }
                $startInfo = New-Object Diagnostics.ProcessStartInfo
                $startInfo.FileName = 'powershell.exe'
                $startInfo.Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $script.FullName + '"'
                $startInfo.UseShellExecute = $false
                $startInfo.RedirectStandardOutput = $true
                $startInfo.RedirectStandardError = $true
                $startInfo.CreateNoWindow = $true
                # Per-child environment avoids leaking a test root into the host.
                $startInfo.EnvironmentVariables['QUICKNEST_TEST_RUN_ROOT'] = $script:QnRunRoot
                $startInfo.EnvironmentVariables['QUICKNEST_INSTANCE_NAMESPACE'] = 'regression-' + [Guid]::NewGuid().ToString('N')
                if (-not [string]::IsNullOrWhiteSpace($env:QUICKNEST_DATA_DIR)) {
                    [void](Assert-QnContainedPath $env:QUICKNEST_DATA_DIR $script:QnRunRoot)
                }
                $process = New-Object Diagnostics.Process
                $process.StartInfo = $startInfo
                if (-not $process.Start()) { throw 'could not start verification process' }
                Register-QnOwnedProcess $process
                $stdout = $process.StandardOutput.ReadToEndAsync()
                $stderr = $process.StandardError.ReadToEndAsync()
                if (-not $process.WaitForExit(300000)) {
                    Stop-QnOwnedProcess -Process $process
                    $code = 124
                } else { $code = $process.ExitCode }
                if (-not [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout, $stderr), 5000)) { throw 'output pipe remained open after child exit; refusing an unbounded wait' }
                $output = $stdout.Result + $stderr.Result
                if ($code -eq 124) { $output += "`r`nregression: script timed out after 300s; no unrelated processes were cleaned" }
                Set-Content -LiteralPath $logPath -Value $output -Encoding UTF8
            } catch {
                $code = 1
                $_ | Out-String | Set-Content -LiteralPath $logPath -Encoding UTF8
            } finally {
                if ($null -ne $process) {
                    try { Stop-QnOwnedProcess -Process $process } finally {
                        if ($script:QnOwnedProcesses.Count -gt 0) {
                            foreach ($ownedId in @($script:QnOwnedProcesses.Keys)) {
                                if ([object]::ReferenceEquals($script:QnOwnedProcesses[$ownedId].Process, $process)) { $script:QnOwnedProcesses.Remove($ownedId) }
                            }
                        }
                        $process.Dispose()
                    }
                }
            }
            $stopwatch.Stop()
            if ($code -ne 0) { $failed++ }
            Add-Content -LiteralPath $report -Encoding UTF8 -Value "$name`t$code`t$([int]$stopwatch.Elapsed.TotalSeconds)"
            Write-Host ("{0}`t{1}" -f $name, $code)
        }
        Write-Host ("regression: {0} scripts, {1} failed; report={2}" -f $scripts.Count, $failed, $report)
    }
} catch {
    $failed++
    [Console]::Error.WriteLine('regression preparation/execution failed: ' + $_.Exception.Message)
    if ($report) { Add-Content -LiteralPath $report -Encoding UTF8 -Value "__preparation__`t1`t0" -ErrorAction Continue }
} finally {
    Restore-QnEnvironment $savedEnvironment
}
if ($failed -gt 0) { exit 1 }
