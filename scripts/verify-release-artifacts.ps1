#requires -Version 5.1
# Verification entry for local release runs.
#   -ReleaseDirectory: strict in-place verification against the current source
#                      (unrelaxed: Cargo/Git/current HEAD stay in the contract).
#   -Archive:          frozen-evidence verification of a schema-2 run only;
#                      no Cargo, no Git, no current source; the archive may
#                      have been moved or renamed after the build.
#   -Legacy:           read-only internal-consistency pass for schema-1 runs.
#                      They lack the source-files snapshot, so archive-grade
#                      verification is impossible; nothing is ever written.
param([string]$ReleaseDirectory = '', [string]$Archive = '', [string]$Legacy = '', [switch]$Fixture)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/release-metadata.ps1')
if ($Fixture) {
    if ($ReleaseDirectory -or $Archive -or $Legacy) { throw '-Fixture cannot be combined with a release directory' }
    & (Join-Path $root 'scripts/fixtures/verify-release-artifacts-fixtures.ps1')
    return
}
if ($Archive -and ($ReleaseDirectory -or $Legacy)) { throw 'Choose exactly one of -ReleaseDirectory, -Archive or -Legacy' }
if ($Legacy -and ($ReleaseDirectory -or $Archive)) { throw 'Choose exactly one of -ReleaseDirectory, -Archive or -Legacy' }
if (-not $ReleaseDirectory -and -not $Archive -and -not $Legacy) { throw 'Pass the allocated -ReleaseDirectory printed by scripts/build-release.ps1, or -Archive, or -Legacy' }

if ($Archive) {
    $context = Assert-QnReleaseFrozenArchive (Assert-QnReleasePath $Archive) -ProbeExecutable
    $recipeText = @('opt-level','debug','debug-assertions','overflow-checks','lto','codegen-units','incremental','panic','strip') | ForEach-Object { $_ + '=' + (Get-QnReleaseRecipeValue $context.build.build_recipe $_) }
    $subsystem = Get-QnReleasePESubsystem (Join-Path $context.staging_directory 'quicknest.exe')
    Write-Host ('Verified frozen archive without Cargo/Git/current source: ' + $context.directory)
    Write-Host ('version=' + $context.release.version + '; source=' + $context.release.source_fingerprint + '; build=' + $context.release.build_fingerprint)
    Write-Host ('pinned recipe: ' + ($recipeText -join ' '))
    Write-Host ('PE subsystem probe (record-only, not a gate): ' + $subsystem)
    Write-Host 'Source fingerprint, manifest chain, staging, ZIP, setup evidence and compiler arguments were re-derived from the frozen snapshot and recorded context only.'
    return
}

if ($Legacy) {
    $run = Assert-QnReleasePath $Legacy
    if ([IO.File]::ReadAllText((Join-Path $run '.quicknest-release-run'), [Text.Encoding]::UTF8) -cne 'QuickNest local release v1') { throw 'Legacy verification expects a schema-1 release run' }
    $release = Read-QnReleaseJson (Join-Path $run 'release-manifest.json')
    if ($release.schema_version -ne 1) { throw 'Release manifest is not schema version 1; use -Archive for schema-2 runs' }
    $references = @{
        source_manifest='source-manifest.json'; build_manifest='build-manifest.json'
        staging_manifest='staging-manifest.json'; setup_evidence='setup-evidence.json'
        portable=('QuickNest-' + $release.version + '-portable.zip')
        setup=('QuickNest-' + $release.version + '-setup.exe')
    }
    foreach ($name in $references.Keys) {
        if ($release.$name.path -cne $references[$name]) { throw ('Incorrect release reference: ' + $name) }
        [void](Assert-QnReleaseFileRecord $run $release.$name)
    }
    if ($release.installation_executed -ne $false -or $release.desktop_validation_executed -ne $false) { throw 'Release manifest makes unsupported runtime claims' }
    $source = Read-QnReleaseJson (Join-Path $run 'source-manifest.json')
    [void](Assert-QnReleaseSourceManifest $source)
    if ($release.version -cne $source.version -or $release.source_fingerprint -cne $source.source_fingerprint) { throw 'Release/source provenance mismatch' }
    $build = Read-QnReleaseJson (Join-Path $run 'build-manifest.json')
    if ($build.schema_version -ne 1) { throw 'Build manifest is not schema version 1' }
    [void](Assert-QnReleaseFileRecord $run $build.source_manifest)
    if ($build.source_manifest.path -cne 'source-manifest.json' -or $build.source_manifest.sha256 -cne $release.source_manifest.sha256) { throw 'Build manifest points at a different source manifest' }
    $info = $build.build_info
    if ($info.version -cne $source.version -or $info.source_fingerprint -cne $source.source_fingerprint -or $info.base_head -cne $source.base_head -or $info.dirty -cne $source.dirty) { throw 'Build/source provenance mismatch' }
    Assert-QnReleaseBuildInfo $info $info
    if ($release.build_fingerprint -cne $info.build_fingerprint) { throw 'Release/build fingerprint mismatch' }
    $stage = Read-QnReleaseJson (Join-Path $run 'staging-manifest.json')
    if ($stage.schema_version -ne 1) { throw 'Unsupported staging manifest' }
    [void](Assert-QnReleaseFileRecord $run $stage.build_manifest)
    if ($stage.build_manifest.path -cne 'build-manifest.json' -or $stage.build_manifest.sha256 -cne $release.build_manifest.sha256) { throw 'Staging manifest points at a different build manifest' }
    if ($stage.version -cne $source.version -or $stage.build_fingerprint -cne $info.build_fingerprint) { throw 'Staging/build provenance mismatch' }
    $directory = Assert-QnReleaseStage $run $stage
    $exeRecord = @($stage.files | Where-Object { $_.path -ceq 'quicknest.exe' })[0]
    if ($build.executable.path -cne 'quicknest.exe' -or $exeRecord.sha256 -cne $build.executable.sha256 -or $exeRecord.bytes -ne $build.executable.bytes) { throw 'Staging executable differs from recorded build' }
    [void](Assert-QnReleaseDistributedFiles $source $stage)
    [void](Assert-QnReleaseFileRecord $run $build.cargo_log)
    if ($build.cargo_exit_code -ne 0 -or $build.update_health_protocol_verified -ne $true -or $build.desktop_validation_executed -ne $false) { throw 'Invalid offline build evidence' }
    $evidence = Read-QnReleaseJson (Join-Path $run 'setup-evidence.json')
    if ($evidence.schema_version -ne 1 -or $evidence.evidence_level -cne 'frozen-input-and-compiler' -or $evidence.compiler_exit_code -ne 0 -or $evidence.installation_executed -ne $false -or $evidence.unpacking_verified -ne $false -or $evidence.desktop_validation_executed -ne $false) { throw 'Invalid installer compilation evidence or unsupported runtime claim' }
    [void](Assert-QnReleaseFileRecord $run $evidence.staging_manifest)
    if ($evidence.staging_manifest.path -cne 'staging-manifest.json' -or $evidence.staging_manifest.sha256 -cne $release.staging_manifest.sha256) { throw 'Installer used different frozen inputs' }
    # v1 records no frozen installer snapshot: only the recorded hash shape and
    # the recorded compiler argument shape can be checked here.
    if ($evidence.installer_source_sha256 -cnotmatch '^[a-f0-9]{64}$') { throw 'Invalid recorded installer script hash' }
    [void](Assert-QnReleaseFileRecord $run $evidence.compiler_log)
    if ($evidence.compiler_log.path -cne 'inno-compile.log') { throw 'Incorrect compiler log reference' }
    if ($evidence.output.path -cne $release.setup.path -or $evidence.output.sha256 -cne $release.setup.sha256 -or $evidence.output.bytes -ne $release.setup.bytes) { throw 'Installer output differs from compiler evidence' }
    if ($evidence.compiler_arguments.Count -ne 4 -or $evidence.compiler_arguments[0] -cne ('/DMyAppVersion=' + $release.version) -or $evidence.compiler_arguments[1] -cnotmatch '^/DReleaseStageDir=.+\\staging$' -or $evidence.compiler_arguments[2] -cnotmatch '^/DReleaseOutputDir=' -or $evidence.compiler_arguments[3] -cnotmatch 'installer[/\\]QuickNest\.iss$') { throw 'Recorded installer compiler arguments have an unexpected shape' }
    if ($evidence.compiler.path -cne 'ISCC.exe' -or $evidence.compiler.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $evidence.compiler.bytes -le 0) { throw 'Invalid compiler identity record' }
    $locks = Open-QnReleaseStageLocks $run $stage
    try {
        Assert-QnPortableArchive (Join-Path $run $release.portable.path) $stage
        Assert-QnReleaseBuildInfo $info (Get-QnExecutableBuildInfo (Join-Path $directory 'quicknest.exe'))
        $nonce = [Guid]::NewGuid().ToString('N')
        $health = Invoke-QnReleaseProbe (Join-Path $directory 'quicknest.exe') ('--quicknest-update-health ' + $release.version + ' ' + $nonce)
        if ($health -cne ("QuickNestHealth`t" + $release.version + "`t" + $nonce)) { throw 'Existing update-health protocol failed' }
    } finally { foreach ($lock in $locks) { $lock.Dispose() } }
    Write-Host ('Legacy v1 run verified for internal consistency only: ' + $run)
    Write-Host 'Limitation: schema-1 runs lack the source-files snapshot, so relocated-archive verification is unavailable and the evidence cannot be rebound to actual source bytes. No manifest was written or modified.'
    return
}

$context = Read-QnReleaseContext $root $ReleaseDirectory -ProbeExecutable
$run = $context.directory
$release = Read-QnReleaseJson (Join-Path $run 'release-manifest.json')
if (($release.schema_version -ne 1 -and $release.schema_version -ne 2) -or $release.version -cne (Get-QnCargoVersion $root) -or $release.source_fingerprint -cne $context.source.source_fingerprint -or $release.build_fingerprint -cne $context.build.build_info.build_fingerprint) { throw 'Release/source/build provenance mismatch' }
if ($release.schema_version -ge 2) {
    # In-place schema 2 additionally requires the frozen snapshot to match the
    # manifest and the recorded build-time context to equal the live paths.
    [void](Assert-QnReleaseSnapshot $run $context.source)
    if ((Assert-QnReleaseRecipe $release.build_recipe) -cne (Assert-QnReleaseRecipe ($context.build.build_info.PSObject.Properties['recipe'].Value))) { throw 'Release manifest recipe differs from the compiled recipe' }
    foreach ($pair in @(
        @('repository_root', $root),
        @('release_directory', $run),
        @('staging_directory', $context.staging_directory),
        @('source_snapshot_directory', (Join-Path $run 'source-files'))
    )) {
        $recorded = $context.build.original_context.PSObject.Properties[$pair[0]]
        if ($null -eq $recorded -or [IO.Path]::GetFullPath($recorded.Value).TrimEnd('\','/') -cne [IO.Path]::GetFullPath($pair[1]).TrimEnd('\','/')) { throw ('Recorded original context does not match the in-place run: ' + $pair[0]) }
    }
}
$references = @{
    source_manifest='source-manifest.json'; build_manifest='build-manifest.json'
    staging_manifest='staging-manifest.json'; setup_evidence='setup-evidence.json'
    portable=('QuickNest-' + $release.version + '-portable.zip')
    setup=('QuickNest-' + $release.version + '-setup.exe')
}
foreach ($name in $references.Keys) {
    if ($release.$name.path -cne $references[$name]) { throw ('Incorrect release reference: ' + $name) }
    [void](Assert-QnReleaseFileRecord $run $release.$name)
}
$locks = Open-QnReleaseStageLocks $run $context.stage
try {
    Assert-QnPortableArchive (Join-Path $run $release.portable.path) $context.stage
    $evidence = Read-QnReleaseJson (Join-Path $run 'setup-evidence.json')
    if ($evidence.schema_version -ne 1 -or $evidence.evidence_level -cne 'frozen-input-and-compiler' -or $evidence.compiler_exit_code -ne 0 -or $evidence.installation_executed -ne $false -or $evidence.unpacking_verified -ne $false -or $evidence.desktop_validation_executed -ne $false) { throw 'Invalid installer compilation evidence or unsupported runtime claim' }
    [void](Assert-QnReleaseFileRecord $run $evidence.staging_manifest)
    if ($evidence.staging_manifest.path -cne 'staging-manifest.json' -or $evidence.staging_manifest.sha256 -cne $release.staging_manifest.sha256) { throw 'Installer used different frozen inputs' }
    if ($evidence.installer_source_sha256 -cne (Get-QnReleaseHash (Join-Path $root 'installer/QuickNest.iss'))) { throw 'Installer script differs from frozen evidence' }
    [void](Assert-QnReleaseFileRecord $run $evidence.compiler_log)
    if ($evidence.compiler_log.path -cne 'inno-compile.log') { throw 'Incorrect compiler log reference' }
    if ($evidence.output.path -cne $release.setup.path -or $evidence.output.sha256 -cne $release.setup.sha256 -or $evidence.output.bytes -ne $release.setup.bytes) { throw 'Installer output differs from compiler evidence' }
    $expectedArguments = @(
        ('/DMyAppVersion=' + $release.version), ('/DReleaseStageDir=' + $context.staging_directory),
        ('/DReleaseOutputDir=' + $run), (Join-Path $root 'installer/QuickNest.iss')
    )
    if (($evidence.compiler_arguments -join "`n") -cne ($expectedArguments -join "`n")) { throw 'Installer compiler arguments did not bind the frozen staging/version' }
    if ($evidence.compiler.path -cne 'ISCC.exe' -or $evidence.compiler.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $evidence.compiler.bytes -le 0) { throw 'Invalid compiler identity record' }
    if ($release.installation_executed -ne $false -or $release.desktop_validation_executed -ne $false) { throw 'Release manifest makes unsupported runtime claims' }
    [void](Assert-QnReleaseFileRecord $run $context.build.cargo_log)
    if ($context.build.cargo_exit_code -ne 0 -or $context.build.update_health_protocol_verified -ne $true -or $context.build.desktop_validation_executed -ne $false) { throw 'Invalid offline build evidence' }
    $nonce = [Guid]::NewGuid().ToString('N')
    $health = Invoke-QnReleaseProbe (Join-Path $context.staging_directory 'quicknest.exe') ('--quicknest-update-health ' + $release.version + ' ' + $nonce)
    if ($health -cne ("QuickNestHealth`t" + $release.version + "`t" + $nonce)) { throw 'Existing update-health protocol failed' }
    [void](Assert-QnReleaseStage $run $context.stage)
    [void](Assert-QnReleaseSourceUnchanged $root $context.source)
    foreach ($name in $references.Keys) { [void](Assert-QnReleaseFileRecord $run $release.$name) }
    Write-Host ('Verified source -> embedded EXE -> portable ZIP and setup compiler evidence: ' + $run)
    Write-Host ('version=' + $release.version + '; source=' + $release.source_fingerprint + '; build=' + $release.build_fingerprint)
    Write-Host 'ZIP entries/duplicates/marker/byte hashes verified. Setup was compiled only; unpacking, installation and desktop acceptance are not claimed.'
} finally { foreach ($lock in $locks) { $lock.Dispose() } }
