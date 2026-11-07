#requires -Version 5.1
param([switch]$Fixture)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\test-sandbox.ps1')
$files = @(Get-ChildItem -LiteralPath (Join-Path $root 'scripts') -Filter 'verify-*.ps1')
$files += Get-Item -LiteralPath (Join-Path $root 'scripts\run-regression.ps1')
$files += Get-Item -LiteralPath (Join-Path $root 'scripts\test-sandbox.ps1')
$checked = 0
foreach ($file in $files) {
    $tokens = $null; $errors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw ("{0}: {1}" -f $file.Name, ($errors -join '; ')) }
    if ($file.Name -notin @('test-sandbox.ps1','verify-sandbox-preflight.ps1')) {
        $commands = @($ast.FindAll({ param($node) $node -is [Management.Automation.Language.CommandAst] }, $true))
        foreach ($command in $commands) {
            if ($command.GetCommandName() -match '(^|\\)(Remove-Item|Stop-Process|taskkill)(\.exe)?$') { throw ('unguarded mutation in ' + $file.Name + ': ' + $command.Extent.Text) }
        }
        $dangerousMembers = @($ast.FindAll({ param($node)
            $node -is [Management.Automation.Language.InvokeMemberExpressionAst] -and $node.Member.Value -in @('Kill','Delete','DeleteFile','RemoveDirectory')
        }, $true))
        if ($dangerousMembers.Count) { throw ('unguarded member mutation in ' + $file.Name) }
        if ($file.Name -like 'verify-p*.ps1') {
            $source = [IO.File]::ReadAllText($file.FullName)
            if ($source -notmatch 'Initialize-QnTestSandbox' -or $source -notmatch 'Restore-QnEnvironment') { throw ('missing sandbox/env restoration in ' + $file.Name) }
            if ($source -match '\$env:QUICKNEST_DATA_DIR\s*=') { throw ('child overwrites caller data root: ' + $file.Name) }
            if ($source -match 'Join-Path \$env:APPDATA ''QuickNest\\(config|icons|data)' -and $file.Name -ne 'verify-p4-02-portable.ps1') { throw ('real user data path in ' + $file.Name) }
        }
    }
    $checked++
}
Write-Host "preflight: $checked scripts parsed and safety-checked; no desktop script executed"
if ($Fixture) {
    $snapshot = Get-QnEnvironmentSnapshot
    $fixtureRun = $null
    try {
        # The fixture never opts into desktop mode and never reads user config.
        $env:QUICKNEST_TEST_RUN_ROOT = New-QnSandboxRun ''
        $fixtureRun = $env:QUICKNEST_TEST_RUN_ROOT
        $env:QUICKNEST_DATA_DIR = Join-Path $fixtureRun 'caller-data'
        $env:QUICKNEST_INSTANCE_NAMESPACE = 'fixture-caller-namespace'
        $callerRoot = $env:QUICKNEST_TEST_SANDBOX_ROOT
        $callerData = $env:QUICKNEST_DATA_DIR
        [void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path -FixtureOnly)
        if ($env:QUICKNEST_DATA_DIR -ne $callerData -or $env:QUICKNEST_TEST_SANDBOX_ROOT -ne $callerRoot -or $env:QUICKNEST_INSTANCE_NAMESPACE -ne 'fixture-caller-namespace') { throw 'caller environment was overwritten' }
        $next = New-QnSandboxRun ''
        if ($next -eq $fixtureRun) { throw 'sandbox roots are not unique' }
        # Only an allocated, asserted fixture root is eligible for cleanup.
        $nextParent = Split-Path -Parent $next
        [void](Assert-QnContainedPath $next $nextParent)
        $oldRoot = $script:QnRunRoot
        $script:QnRunRoot = $nextParent
        Remove-QnSandboxItem -LiteralPath $next -Recurse -Force
        $script:QnRunRoot = $oldRoot
        foreach ($bad in @($fixtureRun, ($fixtureRun + '-sibling\sentinel'), (Join-Path $fixtureRun '..\outside'), (Join-Path $fixtureRun '*.json'))) {
            $rejected = $false
            try { Remove-QnSandboxItem -LiteralPath $bad -Recurse -Force -ErrorAction SilentlyContinue } catch { $rejected = $true }
            if (-not $rejected) { throw "path assertion failed to reject $bad" }
        }
        $env:QUICKNEST_TEST_ISOLATED_DESKTOP = $null
        $refused = $false
        try { Assert-QnDisposableDesktop } catch { $refused = $true }
        if (-not $refused) { throw 'desktop gate did not refuse the default environment' }
        # Explicitly declaring a real data directory as the sandbox cannot grant
        # deletion rights. This exercises assertions only, without reading it.
        foreach ($protected in @((Join-Path $env:APPDATA 'QuickNest/config.json'), (Join-Path $env:LOCALAPPDATA 'Packages/Microsoft.WindowsNotepad_8wekyb3d8bbwe/LocalState/TabState/fixture'))) {
            $refused = $false
            try { [void](Assert-QnContainedPath $protected (Split-Path -Parent $protected)) } catch { $refused = $true }
            if (-not $refused) { throw 'protected user data root was accepted' }
        }
        $probe = Join-Path $callerData 'owned-probe.txt'
        [IO.File]::WriteAllText($probe, 'owned')
        Remove-QnSandboxItem -LiteralPath $probe -Force
        if (Test-Path -LiteralPath $probe) { throw 'owned fixture deletion failed' }
        # A junction inside the sandbox must never grant access to its target.
        $target = Join-Path $fixtureRun 'junction-target'
        [void][IO.Directory]::CreateDirectory($target)
        $sentinel = Join-Path $target 'sentinel.txt'
        [IO.File]::WriteAllText($sentinel, 'keep')
        $junction = Join-Path $script:QnCaseRoot 'junction'
        [void](New-Item -ItemType Junction -Path $junction -Target $target)
        $rejected = $false
        try { Remove-QnSandboxItem -LiteralPath $script:QnCaseRoot -Recurse -Force } catch { $rejected = $true }
        if (-not $rejected -or -not (Test-Path -LiteralPath $sentinel)) { throw 'junction deletion guard failed' }
        # Unlink this exact, newly created fixture junction without traversal.
        [void](Assert-QnContainedPath (Split-Path -Parent $junction) $script:QnRunRoot)
        if ((Get-Item -LiteralPath $junction -Force).LinkType -ne 'Junction') { throw 'fixture junction identity changed' }
        [IO.Directory]::Delete($junction)
        Stop-QnOwnedProcess -Process (Get-Process -Id $PID) -Force
        $child = Start-QnOwnedProcess -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-Command','Start-Sleep -Seconds 30') -WindowStyle Hidden -PassThru
        Stop-QnOwnedProcess -Process $child -Force
        if (-not $child.HasExited) { throw 'owned child was not cleaned' }
        # A preparation failure must be visible as a nonzero runner result.
        $psi = New-Object Diagnostics.ProcessStartInfo
        $psi.FileName = 'powershell.exe'
        $psi.Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + (Join-Path $root 'scripts\run-regression.ps1') + '" -Only nonexistent-fixture.ps1'
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $psi.RedirectStandardError = $true
        $child = New-Object Diagnostics.Process
        $child.StartInfo = $psi
        [void]$child.Start()
        Register-QnOwnedProcess $child
        $errorText = $child.StandardError.ReadToEnd()
        if (-not $child.WaitForExit(15000) -or $child.ExitCode -eq 0) { throw 'runner preparation failure did not fail' }
        if ($errorText -notmatch 'preparation/execution failed') { throw 'runner preparation error not reported' }
        Write-Host 'preflight fixtures: root preservation, uniqueness, traversal/junction refusal, owned cleanup and runner preparation failure passed'
    } finally {
        try {
            Stop-QnTestProcesses
            if ($fixtureRun) {
                $script:QnRunRoot = Split-Path -Parent $fixtureRun
                Remove-QnSandboxItem -LiteralPath $fixtureRun -Recurse -Force
            }
        } finally { Restore-QnEnvironment $snapshot }
    }
}
