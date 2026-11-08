#requires -Version 5.1
param([string]$ReleaseDirectory = '', [switch]$Fixture)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/release-metadata.ps1')
if ($Fixture) {
    if ($ReleaseDirectory) { throw '-Fixture cannot be combined with -ReleaseDirectory' }
    & (Join-Path $root 'scripts/fixtures/verify-release-artifacts-fixtures.ps1')
    return
}
if (-not $ReleaseDirectory) { throw 'Pass the allocated -ReleaseDirectory printed by scripts/build-release.ps1' }
$context = Read-QnReleaseContext $root $ReleaseDirectory -ProbeExecutable
$run = $context.directory
$release = Read-QnReleaseJson (Join-Path $run 'release-manifest.json')
if ($release.schema_version -ne 1 -or $release.version -cne (Get-QnCargoVersion $root) -or $release.source_fingerprint -cne $context.source.source_fingerprint -or $release.build_fingerprint -cne $context.build.build_info.build_fingerprint) { throw 'Release/source/build provenance mismatch' }
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
