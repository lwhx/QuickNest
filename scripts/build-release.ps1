#requires -Version 5.1
# Offline local build and packaging only. Never installs, publishes or starts UI.
param([string]$OutputParent = '', [string]$InnoCompiler = '', [string]$UpdatePublicKeyHex = '')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/release-metadata.ps1')
$version = Get-QnCargoVersion $root
if (-not $OutputParent) { $OutputParent = Join-Path $root 'installer/output' }
$OutputParent = Assert-QnReleaseOutputParent $root $OutputParent
if (-not $InnoCompiler) {
    $candidates = @((Join-Path $root 'tools/InnoSetup/ISCC.exe'), (Join-Path $root 'tools/Inno Setup 6/ISCC.exe'))
    if (${env:ProgramFiles(x86)}) { $candidates += Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe' }
    if (Test-Path -LiteralPath (Join-Path $root 'tools')) { $candidates += @(Get-ChildItem -LiteralPath (Join-Path $root 'tools') -Filter 'ISCC.exe' -File -Recurse | ForEach-Object { $_.FullName }) }
    $available = @($candidates | Where-Object { Test-Path -LiteralPath $_ })
    if ($available.Count) { $InnoCompiler = $available[0] } else { throw 'Local ISCC.exe is required; pass -InnoCompiler. This entry does not download/install tools.' }
}
$InnoCompiler = Assert-QnReleasePath $InnoCompiler
if ([IO.Path]::GetFileName($InnoCompiler) -ine 'ISCC.exe') { throw 'The installer compiler must be ISCC.exe' }
$key = Get-QnReleaseKeyInfo $root $UpdatePublicKeyHex
# The optional argument is the sole key input. An inherited environment key is
# deliberately cleared for the default build, then the caller's env is restored.
$environmentNames = @('QUICKNEST_UPDATE_PUBLIC_KEY_HEX','QUICKNEST_BUILD_SOURCE_FINGERPRINT','QUICKNEST_BUILD_BASE_HEAD','QUICKNEST_BUILD_DIRTY','QUICKNEST_BUILD_RUSTC_SHA256','QUICKNEST_BUILD_CARGO_SHA256','QUICKNEST_BUILD_FINGERPRINT','QUICKNEST_BUILD_RECIPE','CARGO_TARGET_DIR','CARGO_BUILD_TARGET','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC','RUSTDOC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTC_WRAPPER','CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER','CARGO_BUILD_RUSTFLAGS','CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS','CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER','RUSTC_BOOTSTRAP')
$savedEnvironment = @{}
foreach ($name in $environmentNames) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
# B11: every inherited Cargo profile override is cleared by full prefix, which
# covers CARGO_PROFILE_RELEASE_* plus _PACKAGE_* and _BUILD_OVERRIDE_* forms,
# then the nine canonical release keys are pinned explicitly and recorded.
$clearedProfileEnvironment = @([Environment]::GetEnvironmentVariables('Process').Keys | Where-Object { $_ -match '^CARGO_PROFILE_' } | Sort-Object -Unique)
foreach ($name in $clearedProfileEnvironment) {
    if (-not $savedEnvironment.ContainsKey($name)) { $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
}
# The pinned values equal the effective release defaults so the compiled output
# stays equivalent to an unpinned build while becoming explicit and auditable.
$recipe = [pscustomobject][ordered]@{
    'opt-level'='3'; 'debug'='false'; 'debug-assertions'='false'; 'overflow-checks'='false'
    'lto'='false'; 'codegen-units'='16'; 'incremental'='false'; 'panic'='unwind'; 'strip'='none'
}
[void](Assert-QnReleaseRecipe $recipe)
$recipeKeys = @('opt-level','debug','debug-assertions','overflow-checks','lto','codegen-units','incremental','panic','strip')
$recipeEnvironmentNames = @{
    'opt-level'='CARGO_PROFILE_RELEASE_OPT_LEVEL'; 'debug'='CARGO_PROFILE_RELEASE_DEBUG'
    'debug-assertions'='CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS'; 'overflow-checks'='CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS'
    'lto'='CARGO_PROFILE_RELEASE_LTO'; 'codegen-units'='CARGO_PROFILE_RELEASE_CODEGEN_UNITS'
    'incremental'='CARGO_PROFILE_RELEASE_INCREMENTAL'; 'panic'='CARGO_PROFILE_RELEASE_PANIC'
    'strip'='CARGO_PROFILE_RELEASE_STRIP'
}
$pinnedProfileEnvironment = [ordered]@{}
foreach ($recipeKey in $recipeKeys) { $pinnedProfileEnvironment[$recipeEnvironmentNames[$recipeKey]] = Get-QnReleaseRecipeValue $recipe $recipeKey }
$locks = $null
try {
    foreach ($name in $savedEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $null, 'Process') }
    [Environment]::SetEnvironmentVariable('QUICKNEST_UPDATE_PUBLIC_KEY_HEX', $key.normalized_hex, 'Process')
    foreach ($recipeKey in $recipeKeys) { [Environment]::SetEnvironmentVariable($recipeEnvironmentNames[$recipeKey], (Get-QnReleaseRecipeValue $recipe $recipeKey), 'Process') }
    # Transport form without control characters; build.rs reconstructs the
    # canonical recipe string from these pairs before hashing it.
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_RECIPE', (($recipeKeys | ForEach-Object { $_ + '=' + (Get-QnReleaseRecipeValue $recipe $_) }) -join ';'), 'Process')
    $target = 'x86_64-pc-windows-msvc'
    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', (Join-Path $root 'target'), 'Process')
    Push-Location -LiteralPath $root
    try {
        $rustc = Get-QnReleaseToolVersion 'rustc'
        $cargo = Get-QnReleaseToolVersion 'cargo'
    } finally { Pop-Location }
    $source = Get-QnReleaseSourceSnapshot $root
    $info = [pscustomobject][ordered]@{
        schema_version = 1; version = $version; provenance = 'recorded'
        source_fingerprint = $source.source_fingerprint; build_fingerprint = ''
        base_head = $source.base_head; dirty = $source.dirty
        target = $target; profile = 'release'; rustc = $rustc; cargo = $cargo
        update_key = [pscustomobject][ordered]@{ status = $key.status; blob_sha256 = $key.blob_sha256 }
        recipe = $recipe
    }
    $info.build_fingerprint = Get-QnReleaseBuildFingerprint $info
    $run = Assert-QnReleasePath (Join-Path $OutputParent ('release-' + $version + '-' + [Guid]::NewGuid().ToString('N'))) $OutputParent
    [void][IO.Directory]::CreateDirectory($run)
    [IO.File]::WriteAllText((Join-Path $run '.quicknest-release-run'), 'QuickNest local release v2', (New-Object Text.UTF8Encoding($false)))
    Write-QnReleaseJson (Join-Path $run 'source-manifest.json') $source
    # D6: full frozen source snapshot inside the run, including untracked input,
    # so the finished archive can be re-verified without this repository.
    $snapshotRoot = Assert-QnReleasePath (Join-Path $run 'source-files') $run
    [void][IO.Directory]::CreateDirectory($snapshotRoot)
    foreach ($record in $source.files) {
        $relativePath = [string]$record.path
        $snapshotDestination = Assert-QnReleasePath (Join-Path $snapshotRoot ($relativePath.Replace('/', '\'))) $run
        $snapshotParent = Split-Path -Parent $snapshotDestination
        if (-not (Test-Path -LiteralPath $snapshotParent)) { [void][IO.Directory]::CreateDirectory($snapshotParent) }
        [IO.File]::Copy((Assert-QnReleasePath (Join-Path $root ($relativePath.Replace('/', '\'))) $root), $snapshotDestination, $false)
        $copied = Get-QnReleaseFileRecord $snapshotDestination $relativePath
        if ($copied.bytes -ne $record.bytes -or $copied.sha256 -cne $record.sha256) { throw ('Frozen snapshot copy differs from the source manifest: ' + $relativePath) }
    }
    [void](Assert-QnReleaseSnapshot $run $source)
    $stageDirectory = Assert-QnReleasePath (Join-Path $run 'staging') $run
    $buildArguments = @('build','--manifest-path',(Join-Path $root 'Cargo.toml'),'--locked','--offline','--release','--target',$target,'--bin','quicknest')
    $buildLog = Join-Path $run 'cargo-build.log'
    Write-Host ('Source frozen: ' + $source.source_fingerprint + '; base HEAD=' + $source.base_head + '; dirty=' + $source.dirty)
    Write-Host ('Release profile pinned: ' + (($recipeKeys | ForEach-Object { $_ + '=' + (Get-QnReleaseRecipeValue $recipe $_) }) -join ' '))
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_SOURCE_FINGERPRINT', $source.source_fingerprint, 'Process')
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_BASE_HEAD', $source.base_head, 'Process')
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_DIRTY', $source.dirty.ToString().ToLowerInvariant(), 'Process')
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_RUSTC_SHA256', (Get-QnReleaseTextHash $rustc), 'Process')
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_CARGO_SHA256', (Get-QnReleaseTextHash $cargo), 'Process')
    [Environment]::SetEnvironmentVariable('QUICKNEST_BUILD_FINGERPRINT', $info.build_fingerprint, 'Process')
    # An explicit process avoids PS 5.1 treating successful native stderr as a
    # terminating NativeCommandError. Logs are written with explicit UTF-8.
    $buildExit = Invoke-QnReleaseTool 'cargo.exe' $buildArguments $root $buildLog
    [void](Assert-QnReleaseSourceUnchanged $root $source)
    if ($buildExit -ne 0) { throw ('Offline Cargo build failed; see ' + $buildLog) }
    $exe = Assert-QnReleasePath (Join-Path $root ('target/' + $target + '/release/quicknest.exe')) $root
    $exeRecord = Get-QnReleaseFileRecord $exe 'quicknest.exe'
    Assert-QnReleaseBuildInfo $info (Get-QnExecutableBuildInfo $exe)
    $nonce = [Guid]::NewGuid().ToString('N')
    $health = Invoke-QnReleaseProbe $exe ('--quicknest-update-health ' + $version + ' ' + $nonce)
    if ($health -cne ("QuickNestHealth`t" + $version + "`t" + $nonce)) { throw 'Existing update-health protocol did not match' }
    if ((Get-QnReleaseHash $exe) -cne $exeRecord.sha256) { throw 'Executable changed during early-entry probes' }
    $build = [pscustomobject][ordered]@{
        schema_version = 2; version = $version
        source_manifest = Get-QnReleaseFileRecord (Join-Path $run 'source-manifest.json') 'source-manifest.json'
        build_info = $info; executable = $exeRecord
        build_recipe = $recipe
        original_context = [pscustomobject][ordered]@{
            repository_root = $root; release_directory = $run
            staging_directory = $stageDirectory; source_snapshot_directory = $snapshotRoot
        }
        cleared_build_environment = $environmentNames
        cleared_profile_environment = $clearedProfileEnvironment
        pinned_profile_environment = $pinnedProfileEnvironment
        cargo_arguments = $buildArguments; cargo_exit_code = $buildExit
        cargo_log = Get-QnReleaseFileRecord $buildLog 'cargo-build.log'
        update_health_protocol_verified = $true; desktop_validation_executed = $false
    }
    Write-QnReleaseJson (Join-Path $run 'build-manifest.json') $build
    [void][IO.Directory]::CreateDirectory($stageDirectory)
    [IO.File]::Copy($exe, (Join-Path $stageDirectory 'quicknest.exe'), $false)
    foreach ($distributedFile in @(@{ source='assets/QuickNest.ico'; destination='QuickNest.ico' }, @{ source='README.md'; destination='README.md' })) {
        [IO.File]::Copy((Join-Path $root $distributedFile.source), (Join-Path $stageDirectory $distributedFile.destination), $false)
    }
    [IO.File]::WriteAllText((Join-Path $stageDirectory 'QuickNest.portable'), '', (New-Object Text.UTF8Encoding($false)))
    $stageFiles = @()
    foreach ($name in @('QuickNest.ico','QuickNest.portable','README.md','quicknest.exe')) {
        $path = Join-Path $stageDirectory $name
        [IO.File]::SetAttributes($path, ([IO.File]::GetAttributes($path) -bor [IO.FileAttributes]::ReadOnly))
        $stageFiles += Get-QnReleaseFileRecord $path $name
    }
    $stage = [pscustomobject][ordered]@{
        schema_version = 1; version = $version; build_fingerprint = $info.build_fingerprint
        build_manifest = Get-QnReleaseFileRecord (Join-Path $run 'build-manifest.json') 'build-manifest.json'
        files = $stageFiles
    }
    Write-QnReleaseJson (Join-Path $run 'staging-manifest.json') $stage
    $context = Read-QnReleaseContext $root $run -ProbeExecutable
    $stageManifest = Get-QnReleaseFileRecord (Join-Path $run 'staging-manifest.json') 'staging-manifest.json'
    $locks = Open-QnReleaseStageLocks $run $stage
    # Both packaging paths consume this exact, read-locked staging tree.
    & (Join-Path $root 'installer/build-portable.ps1') -ReleaseDirectory $run
    [void](Assert-QnReleaseStage $run $stage)
    [void](Assert-QnReleaseFileRecord $run $stageManifest)
    [void](Assert-QnReleaseSourceUnchanged $root $source)
    $setupName = 'QuickNest-' + $version + '-setup.exe'
    $setupPath = Join-Path $run $setupName
    if (Test-Path -LiteralPath $setupPath) { throw 'Installer output already exists' }
    $iss = Join-Path $root 'installer/QuickNest.iss'
    $issHash = Get-QnReleaseHash $iss
    $compilerRecord = Get-QnReleaseFileRecord $InnoCompiler 'ISCC.exe'
    $compilerArguments = @(('/DMyAppVersion=' + $version), ('/DReleaseStageDir=' + $stageDirectory), ('/DReleaseOutputDir=' + $run), $iss)
    $compilerLog = Join-Path $run 'inno-compile.log'
    $compilerExit = Invoke-QnReleaseTool $InnoCompiler $compilerArguments $root $compilerLog
    [void](Assert-QnReleaseStage $run $stage)
    [void](Assert-QnReleaseFileRecord $run $stageManifest)
    [void](Assert-QnReleaseSourceUnchanged $root $source)
    if ((Get-QnReleaseHash $iss) -cne $issHash -or (Get-QnReleaseHash $InnoCompiler) -cne $compilerRecord.sha256) { throw 'Installer source/compiler changed during packaging' }
    if ($compilerExit -ne 0) { throw ('Inno compilation failed; see ' + $compilerLog) }
    $setupRecord = Get-QnReleaseFileRecord $setupPath $setupName
    $setupEvidence = [pscustomobject][ordered]@{
        schema_version = 1; evidence_level = 'frozen-input-and-compiler'
        staging_manifest = $stageManifest; installer_source_sha256 = $issHash
        compiler_path = $InnoCompiler; compiler = $compilerRecord
        compiler_version = (Get-Item -LiteralPath $InnoCompiler).VersionInfo.FileVersion
        compiler_arguments = $compilerArguments; compiler_exit_code = $compilerExit
        compiler_log = Get-QnReleaseFileRecord $compilerLog 'inno-compile.log'
        output = $setupRecord
        installation_executed = $false; unpacking_verified = $false; desktop_validation_executed = $false
    }
    Write-QnReleaseJson (Join-Path $run 'setup-evidence.json') $setupEvidence
    $release = [pscustomobject][ordered]@{
        schema_version = 2; version = $version; source_fingerprint = $source.source_fingerprint; build_fingerprint = $info.build_fingerprint
        source_manifest = $build.source_manifest
        build_manifest = $stage.build_manifest; staging_manifest = $stageManifest
        setup_evidence = Get-QnReleaseFileRecord (Join-Path $run 'setup-evidence.json') 'setup-evidence.json'
        portable = Get-QnReleaseFileRecord (Join-Path $run ('QuickNest-' + $version + '-portable.zip')) ('QuickNest-' + $version + '-portable.zip')
        setup = $setupRecord
        build_recipe = $recipe
        installation_executed = $false; desktop_validation_executed = $false
    }
    Write-QnReleaseJson (Join-Path $run 'release-manifest.json') $release
    & (Join-Path $root 'scripts/verify-release-artifacts.ps1') -ReleaseDirectory $run
    Write-Host ('Local release completed: ' + $run)
} finally {
    if ($null -ne $locks) { foreach ($lock in $locks) { $lock.Dispose() } }
    foreach ($name in $savedEnvironment.Keys) { [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process') }
}
