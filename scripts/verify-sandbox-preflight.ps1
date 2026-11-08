#requires -Version 5.1
param([switch]$Fixture)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\test-sandbox.ps1')
function Get-QnScriptSafetyClass([string]$Name, [Management.Automation.Language.ScriptBlockAst]$Ast, [string]$Source) {
    if (Get-QnScriptBlockReason $Ast) { return 'BLOCKED' }
    $Name = $Name.Replace('\','/')
    if ($Name -in @('test-sandbox.ps1','verify-sandbox-preflight.ps1')) { return 'infrastructure' }
    $support = @('p0-05-desktop.ps1','p0-06-native.ps1','p1-17-focus.ps1')
    $utilities = @('run-regression.ps1','assert-p0-03-report.ps1','assert-p0-04-report.ps1','assert-p0-05-interactive.ps1','summarize-p0-06.ps1','gen-icon.ps1','update-keygen.ps1','update-sign.ps1','update-server.ps1')
    # Exact audited paths only; verify-* or an infrastructure basename grants no exemption.
    $releaseScripts = @('release-metadata.ps1','build-release.ps1','verify-release-artifacts.ps1','fixtures/verify-release-artifacts-fixtures.ps1','installer/build-portable.ps1')
    $isRelease = $Name -in $releaseScripts
    if ($Name -notlike 'verify-p*.ps1' -and $Name -notin $support -and $Name -notin $utilities -and -not $isRelease) { throw ('unclassified executable script requires migration or a first-statement BLOCKED guard: ' + $Name) }
    $commands = @($Ast.FindAll({ param($node) $node -is [Management.Automation.Language.CommandAst] }, $true))
    foreach ($command in $commands) {
        if ($command.GetCommandName() -match '(^|\\)(Remove-Item|Stop-Process|taskkill)(\.exe)?$') { throw ('unguarded mutation in ' + $Name + ': ' + $command.Extent.Text) }
        if ($isRelease -and $command.GetCommandName() -match '(^|\\)(Start-Process|Invoke-WebRequest|Invoke-RestMethod|Set-ItemProperty|New-ItemProperty|Remove-ItemProperty|schtasks|reg|msiexec)(\.exe)?$') { throw ('release entry has an unsupported external/user-state operation: ' + $Name) }
    }
    $dangerousMembers = @($Ast.FindAll({ param($node)
        $node -is [Management.Automation.Language.InvokeMemberExpressionAst] -and $node.Member.Value -in @('Kill','Delete','DeleteFile','RemoveDirectory')
    }, $true))
    foreach ($member in $dangerousMembers) {
        $owner = $member.Parent
        while ($owner -and $owner -isnot [Management.Automation.Language.FunctionDefinitionAst]) { $owner = $owner.Parent }
        # The sole exception owns a freshly created Process object, starts it,
        # and kills that object only after timeout. No PID/name lookup is used.
        $ownedProbe = $Name -eq 'release-metadata.ps1' -and $member.Member.Value -eq 'Kill' -and $owner -and $owner.Name -in @('Invoke-QnReleaseProbe','Invoke-QnReleaseTool') -and $member.Expression.Extent.Text -ceq '$process' -and $owner.Extent.Text -match '\$startedHere = \$process.Start\(\)' -and $owner.Extent.Text -match '\$startedHere -and -not \$process.HasExited'
        if (-not $ownedProbe) { throw ('unguarded member mutation in ' + $Name) }
    }
    if ($isRelease) {
        if ($Name -eq 'release-metadata.ps1') {
            foreach ($statement in $Ast.EndBlock.Statements) { if ($statement -isnot [Management.Automation.Language.FunctionDefinitionAst]) { throw 'release metadata library must only define functions' } }
            return 'release-support-library'
        }
        if ($Name -eq 'fixtures/verify-release-artifacts-fixtures.ps1') {
            if ($Source -notmatch 'GetTempPath' -or $Source -notmatch 'Assert-QnReleasePath' -or $Source -match 'Invoke-QnReleaseProbe|Get-QnExecutableBuildInfo|Read-QnReleaseContext|Diagnostics.Process') { throw 'release fixtures must stay in newly allocated temp roots without executable probes' }
            if ($Source -notmatch 'Assert-QnReleaseFrozenArchive' -or $Source -notmatch 'Assert-QnReleaseRecipe' -or $Source -notmatch 'Assert-QnReleaseSnapshot' -or $Source -notmatch 'CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS' -or $Source -notmatch 'quicknest-recipe-v2' -or $Source -notmatch 'WizardIsTaskSelected') { throw 'release fixtures must exercise the recipe, snapshot, frozen-archive and installer contracts as pure text-double functions' }
            return 'release-headless-fixtures'
        }
        if ($Name -eq 'build-release.ps1') {
            # Current-source contract: the build entry freezes and re-checks the
            # live worktree, pins the release profile and records the snapshot.
            if ($Source -notmatch "'--locked','--offline'" -or $Source -notmatch 'savedEnvironment' -or $Source -notmatch 'QUICKNEST_UPDATE_PUBLIC_KEY_HEX') { throw 'release build lacks offline/key/environment controls' }
            if ($Source -notmatch 'Assert-QnReleaseSourceUnchanged' -or $Source -notmatch '\^CARGO_PROFILE_' -or $Source -notmatch 'Assert-QnReleaseRecipe' -or $Source -notmatch 'Assert-QnReleaseSnapshot' -or $Source -notmatch 'build_recipe' -or $Source -notmatch 'original_context' -or $Source -notmatch "'source-files'") { throw 'release build lacks current-source, pinned-profile and frozen snapshot assertions' }
            return 'release-current-source-contract'
        }
        if ($Name -eq 'verify-release-artifacts.ps1') {
            # Frozen-evidence contract: -Archive verifies without Cargo/Git or
            # the current tree, -Legacy stays read-only, -ReleaseDirectory keeps
            # the strict current-source assertions.
            if ($Source -notmatch '\-Archive' -or $Source -notmatch '\-Legacy' -or $Source -notmatch 'Assert-QnReleaseFrozenArchive' -or $Source -notmatch 'Assert-QnReleaseSourceManifest' -or $Source -notmatch 'Assert-QnReleaseSnapshot' -or $Source -notmatch 'original_context' -or $Source -notmatch 'Read-QnReleaseContext' -or $Source -notmatch 'Open-QnReleaseStageLocks' -or $Source -notmatch 'Assert-QnReleaseSourceUnchanged' -or $Source -notmatch 'Assert-QnReleaseStage') { throw 'release verifier lacks frozen-evidence archive mode or the strict current-source mode' }
            return 'release-frozen-evidence-contract'
        }
        if ($Source -notmatch 'Read-QnReleaseContext' -or $Source -notmatch 'Open-QnReleaseStageLocks' -or $Source -notmatch 'Assert-QnReleaseSourceUnchanged' -or $Source -notmatch 'Assert-QnReleaseStage') { throw ('release entry lacks the frozen-input contract: ' + $Name) }
        return 'release-frozen-input-contract'
    }
    if ($Name -like 'verify-p*.ps1') {
        if ($Source -notmatch 'Initialize-QnTestSandbox' -or $Source -notmatch 'Restore-QnEnvironment') { throw ('missing sandbox/env restoration in ' + $Name) }
        if ($Source -match '\$env:QUICKNEST_DATA_DIR\s*=') { throw ('child overwrites caller data root: ' + $Name) }
        if ($Source -match 'Join-Path \$env:APPDATA ''QuickNest[\\/](config|icons|data)' -and $Name -ne 'verify-p4-02-portable.ps1') { throw ('real user data path in ' + $Name) }
        return 'verification-contract'
    }
    if ($Name -in $support) { return 'support-library' }
    return 'utility'
}
$scriptsDirectory = Join-Path $root 'scripts'
# Filesystem inventory deliberately includes ignored/hidden temporary scripts and
# nested helpers. Git tracking and a verify-* filename are not safety boundaries.
$files = @(Get-ChildItem -LiteralPath $scriptsDirectory -Filter '*.ps1' -File -Recurse -Force | Sort-Object FullName)
$files += Get-Item -LiteralPath (Join-Path $root 'installer/build-portable.ps1')
$checked = 0
$classes = @{}
$blockedNames = @()
foreach ($file in $files) {
    $tokens = $null; $errors = $null
    $ast = [Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors)
    $name = if ($file.FullName.StartsWith($scriptsDirectory + '\', [StringComparison]::OrdinalIgnoreCase)) { $file.FullName.Substring($scriptsDirectory.Length + 1) } else { 'installer/build-portable.ps1' }
    if ($errors.Count) { throw ("{0}: {1}" -f $name, ($errors -join '; ')) }
    $class = Get-QnScriptSafetyClass $name $ast ([IO.File]::ReadAllText($file.FullName))
    if (-not $classes.ContainsKey($class)) { $classes[$class] = 0 }
    $classes[$class]++
    if ($class -eq 'BLOCKED') { $blockedNames += $name }
    $checked++
}
$counts = @($classes.Keys | Sort-Object | ForEach-Object { $_ + '=' + $classes[$_] }) -join ', '
Write-Host "preflight static inventory: $checked scripts parsed; $counts; no desktop script executed"
Write-Host ('preflight BLOCKED entries (not acceptance passes): ' + ($blockedNames -join ', '))
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
        # Parse synthetic unsafe examples; never execute their bodies. Only a
        # literal, unconditional first throw may classify an entry as BLOCKED.
        $refusal = "throw 'BLOCKED: fixture';"
        $guardCases = @(
            @{ Source=$refusal + " [IO.File]::WriteAllText('unused','never');"; Blocked=$true },
            @{ Source="param([string]`$Name='fixture')`n" + $refusal; Blocked=$true },
            @{ Source="# throw 'BLOCKED: comment'`nWrite-Output 'not blocked'"; Blocked=$false },
            @{ Source="if (`$true) { $refusal }"; Blocked=$false },
            @{ Source="function Test-Refusal { $refusal }; Test-Refusal"; Blocked=$false },
            @{ Source="Write-Output 'before'; $refusal"; Blocked=$false },
            @{ Source="trap { continue }; $refusal"; Blocked=$false },
            @{ Source="param([string]`$Value=`$(Write-Output 'before'))`n" + $refusal; Blocked=$false },
            @{ Source="param([ValidateScript({ Write-Output 'before' })][string]`$Value)`n" + $refusal; Blocked=$false },
            @{ Source="using namespace System`n" + $refusal; Blocked=$false }
        )
        foreach ($guardCase in $guardCases) {
            $fixtureTokens = $null; $fixtureErrors = $null
            $fixtureAst = [Management.Automation.Language.Parser]::ParseInput($guardCase.Source, [ref]$fixtureTokens, [ref]$fixtureErrors)
            if ($fixtureErrors.Count) { throw ('invalid guard fixture: ' + ($fixtureErrors -join '; ')) }
            $isBlocked = [bool](Get-QnScriptBlockReason $fixtureAst)
            if ($isBlocked -ne $guardCase.Blocked) { throw ('incorrect BLOCKED classification: ' + $guardCase.Source) }
            $rejected = $false
            try { $kind = Get-QnScriptSafetyClass 'tmp-omitted-fixture.ps1' $fixtureAst $guardCase.Source } catch { $rejected = $true }
            if ($guardCase.Blocked) {
                if ($rejected -or $kind -ne 'BLOCKED') { throw 'unconditional compatibility refusal was not inventoried as BLOCKED' }
            } elseif (-not $rejected) { throw 'unclassified supplemental entry was accepted' }
        }
        $nestedRejected = $false
        try { [void](Get-QnScriptSafetyClass 'nested/test-sandbox.ps1' $fixtureAst '') } catch { $nestedRejected = $true }
        if (-not $nestedRejected) { throw 'nested script impersonated audited infrastructure' }
        if ($files.Name -notcontains 'p2-12-theme-headless.ps1' -or $blockedNames -notcontains 'p2-12-theme-headless.ps1') { throw 'headless compatibility entry escaped the filesystem inventory' }
        Write-Host 'preflight guard fixtures: unconditional refusal, unsafe initializer/trap rejection and supplemental-entry inventory passed (no legacy body executed)'
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
        $noAllocation = Join-Path $fixtureRun 'blocked-runner-must-not-allocate'
        $blockedVerifications = @($blockedNames | Where-Object { $_ -like 'verify-p*.ps1' })
        if ($blockedVerifications.Count -eq 0) { throw 'blocked-menu runner fixtures were not selected' }
        foreach ($blockedName in $blockedVerifications) {
            $psi.Arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + (Join-Path $root 'scripts\run-regression.ps1') + '" -Only ' + $blockedName
            $psi.EnvironmentVariables['QUICKNEST_TEST_SANDBOX_ROOT'] = $noAllocation
            $child = New-Object Diagnostics.Process
            $child.StartInfo = $psi
            [void]$child.Start()
            Register-QnOwnedProcess $child
            $errorTask = $child.StandardError.ReadToEndAsync()
            if (-not $child.WaitForExit(15000)) { throw ('blocked runner did not exit: ' + $blockedName) }
            if (-not $errorTask.Wait(5000)) { throw 'blocked runner stderr pipe remained open' }
            if ($child.ExitCode -eq 0 -or $errorTask.Result -notmatch 'BLOCKED:' -or $errorTask.Result -notmatch [regex]::Escape($blockedName)) { throw ('obsolete verification was not explicitly BLOCKED: ' + $blockedName) }
            if (Test-Path -LiteralPath $noAllocation) { throw 'blocked selection allocated a run before refusal' }
        }
        Write-Host ("preflight runner fixtures: {0} obsolete selections returned nonzero BLOCKED before sandbox allocation or desktop execution" -f $blockedVerifications.Count)
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
