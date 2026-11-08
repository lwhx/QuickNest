#requires -Version 5.1
# No application, installer, UI, user config, network, key generation or cleanup.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path))
. (Join-Path $root 'scripts/release-metadata.ps1')
$parent = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) 'QuickNest-release-fixtures'))
$fixtureDirectory = Assert-QnReleasePath (Join-Path $parent ('run-' + [Guid]::NewGuid().ToString('N'))) $parent
[void][IO.Directory]::CreateDirectory($fixtureDirectory)
$script:fixturePasses = 0
function Assert-QnFixtureRefuses([string]$Name, [scriptblock]$Action) {
    $refused = $false
    try { & $Action } catch { $refused = $true }
    if (-not $refused) { throw ('Negative release fixture was accepted: ' + $Name) }
    $script:fixturePasses++
    Write-Host ('PASS refusal: ' + $Name)
}
function Copy-QnFixtureInfo($Info) { return ($Info | ConvertTo-Json -Depth 8) | ConvertFrom-Json }
function New-QnFixtureArchive([string]$Name, [object[]]$Entries, [string]$Directory = '') {
    if (-not $Directory) { $Directory = $fixtureDirectory }
    $path = Assert-QnReleasePath (Join-Path $Directory $Name) $Directory
    $stream = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        $zip = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            foreach ($item in $Entries) {
                $entry = $zip.CreateEntry($item.name)
                $output = $entry.Open()
                try { $bytes = [Text.Encoding]::UTF8.GetBytes([string]$item.text); $output.Write($bytes,0,$bytes.Length) } finally { $output.Dispose() }
            }
        } finally { $zip.Dispose() }
    } finally { $stream.Dispose() }
    return $path
}
if ((Get-QnReleaseTextHash 'abc') -cne 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad') { throw 'SHA-256 known vector failed' }
$version = Get-QnCargoVersion $root
$info = [pscustomobject][ordered]@{
    schema_version=1; version=$version; provenance='recorded'
    source_fingerprint=('a' * 64); build_fingerprint=''; base_head=('b' * 40); dirty=$true
    target='x86_64-pc-windows-msvc'; profile='release'; rustc='fixture rustc'; cargo='fixture cargo'
    update_key=[pscustomobject]@{ status='disabled'; blob_sha256=$null }
}
$info.build_fingerprint = Get-QnReleaseBuildFingerprint $info
Assert-QnReleaseBuildInfo $info $info
foreach ($change in @(
    @{ name='version mismatch'; field='version'; value='0.0.0' },
    @{ name='source fingerprint mismatch'; field='source_fingerprint'; value=('c' * 64) },
    @{ name='build fingerprint mismatch'; field='build_fingerprint'; value=('d' * 64) },
    @{ name='base HEAD mismatch'; field='base_head'; value=('e' * 40) },
    @{ name='dirty declaration mismatch'; field='dirty'; value=$false },
    @{ name='unrecorded manual executable'; field='provenance'; value='unrecorded' },
    @{ name='wrong target'; field='target'; value='aarch64-pc-windows-msvc' },
    @{ name='debug executable'; field='profile'; value='debug' }
)) {
    $actual = Copy-QnFixtureInfo $info
    $actual.($change.field) = $change.value
    Assert-QnFixtureRefuses $change.name { Assert-QnReleaseBuildInfo $info $actual }
}
$stale = Copy-QnFixtureInfo $info
$stale.source_fingerprint = '0' * 64
$stale.build_fingerprint = Get-QnReleaseBuildFingerprint $stale
Assert-QnFixtureRefuses 'same-version old executable with internally consistent older provenance' { Assert-QnReleaseBuildInfo $info $stale }
$actual = Copy-QnFixtureInfo $info
$actual.update_key = [pscustomobject]@{ status='configured'; blob_sha256=('f' * 64) }
$actual.build_fingerprint = Get-QnReleaseBuildFingerprint $actual
Assert-QnFixtureRefuses 'different compiled key at the same version' { Assert-QnReleaseBuildInfo $info $actual }
$disclosed = [IO.File]::ReadAllText((Join-Path $root 'src/update_key.pub')).Trim()
Assert-QnFixtureRefuses 'disclosed fixture public key' { [void](Get-QnReleaseKeyInfo $root $disclosed.ToUpperInvariant()) }
$privateHeader = ([BitConverter]::ToString([Text.Encoding]::ASCII.GetBytes('ECS2'))).Replace('-','').ToLowerInvariant() + ('20' + ('00' * 3))
Assert-QnFixtureRefuses 'private blob header refused' { [void](Get-QnReleaseKeyInfo $root ($privateHeader + ('00' * 64))) }
$savedKey = [Environment]::GetEnvironmentVariable('QUICKNEST_UPDATE_PUBLIC_KEY_HEX', 'Process')
try {
    [Environment]::SetEnvironmentVariable('QUICKNEST_UPDATE_PUBLIC_KEY_HEX', $disclosed, 'Process')
    $disabled = Get-QnReleaseKeyInfo $root ''
    if ($disabled.status -cne 'disabled' -or $null -ne $disabled.blob_sha256 -or $disabled.normalized_hex -cne '') { throw 'Default key input inherited ambient trust' }
} finally { [Environment]::SetEnvironmentVariable('QUICKNEST_UPDATE_PUBLIC_KEY_HEX', $savedKey, 'Process') }
$sourcePath = Assert-QnReleasePath (Join-Path $fixtureDirectory 'source.bin') $fixtureDirectory
[IO.File]::WriteAllText($sourcePath, 'source-bytes', (New-Object Text.UTF8Encoding($false)))
$record = Get-QnReleaseFileRecord $sourcePath 'source.bin'
[void](Assert-QnReleaseFileRecord $fixtureDirectory $record)
$canonical = "quicknest-source-v1`nsource.bin`t$($record.bytes)`t$($record.sha256)`n"
$source = [pscustomobject]@{ schema_version=1; source_fingerprint=(Get-QnReleaseTextHash $canonical); files=@($record) }
Assert-QnReleaseSourceManifest $source
$badSource = ($source | ConvertTo-Json -Depth 8) | ConvertFrom-Json
$badSource.files[0].sha256 = '0' * 64
Assert-QnFixtureRefuses 'source manifest byte hash changed under frozen fingerprint' { Assert-QnReleaseSourceManifest $badSource }
$badSource = ($source | ConvertTo-Json -Depth 8) | ConvertFrom-Json
$badSource.files = @($badSource.files[0], $badSource.files[0])
Assert-QnFixtureRefuses 'duplicate source manifest paths' { Assert-QnReleaseSourceManifest $badSource }
[IO.File]::WriteAllText($sourcePath, 'source-CHANGED', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'frozen source input changed' { [void](Assert-QnReleaseFileRecord $fixtureDirectory $record) }
Assert-QnFixtureRefuses 'sibling path containment' { [void](Assert-QnReleasePath ($fixtureDirectory + '-sibling/file.bin') $fixtureDirectory) }
Assert-QnFixtureRefuses 'parent traversal' { [void](Assert-QnReleasePath (Join-Path $fixtureDirectory '../outside.bin') $fixtureDirectory) }
[void](Assert-QnReleaseOutputParent $root (Join-Path $root 'installer/output'))
Assert-QnFixtureRefuses 'output parent equals repository root' { [void](Assert-QnReleaseOutputParent $root $root) }
Assert-QnFixtureRefuses 'output parent equals root with trailing separator' { [void](Assert-QnReleaseOutputParent $root ($root + '\')) }
Assert-QnFixtureRefuses 'output parent inside source tree' { [void](Assert-QnReleaseOutputParent $root (Join-Path $root 'src')) }
Assert-QnFixtureRefuses 'output parent prefix sibling of allowed output' { [void](Assert-QnReleaseOutputParent $root (Join-Path $root 'installer/output-sibling')) }
$stageDirectory = Assert-QnReleasePath (Join-Path $fixtureDirectory 'staging') $fixtureDirectory
[void][IO.Directory]::CreateDirectory($stageDirectory)
$contents = [ordered]@{ 'QuickNest.ico'='icon'; 'QuickNest.portable'=''; 'README.md'='portable instructions without package hashes'; 'quicknest.exe'='fixture current exe bytes' }
$records = @(); $entries = @()
foreach ($name in $contents.Keys) {
    $path = Assert-QnReleasePath (Join-Path $stageDirectory $name) $stageDirectory
    [IO.File]::WriteAllText($path, $contents[$name], (New-Object Text.UTF8Encoding($false)))
    [IO.File]::SetAttributes($path, [IO.FileAttributes]::ReadOnly)
    $records += Get-QnReleaseFileRecord $path $name
    $entries += [pscustomobject]@{ name=$name; text=$contents[$name] }
}
$stage = [pscustomobject]@{ schema_version=1; files=$records }
[void](Assert-QnReleaseStage $fixtureDirectory $stage)
$distributedRecords = @()
foreach ($fixtureMapping in @(@{ source='assets/QuickNest.ico'; destination='QuickNest.ico' }, @{ source='README.md'; destination='README.md' })) {
    $stagedRecord = @($records | Where-Object { $_.path -ceq $fixtureMapping.destination })[0]
    $distributedRecords += [pscustomobject]@{ path=$fixtureMapping.source; bytes=$stagedRecord.bytes; sha256=$stagedRecord.sha256 }
}
$distributedSource = [pscustomobject]@{ files=$distributedRecords }
Assert-QnReleaseDistributedFiles $distributedSource $stage
Write-Host 'PASS frozen document/icon source mapping through release-context validator'
foreach ($sourceName in @('assets/QuickNest.ico','README.md')) {
    $changedSource = ($distributedSource | ConvertTo-Json -Depth 8) | ConvertFrom-Json
    ($changedSource.files | Where-Object { $_.path -ceq $sourceName }).sha256 = '0' * 64
    Assert-QnFixtureRefuses ('staging source mismatch for ' + $sourceName) { Assert-QnReleaseDistributedFiles $changedSource $stage }
}
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$goodZip = New-QnFixtureArchive 'good.zip' $entries
Assert-QnPortableArchive $goodZip $stage
$goodZipRecord = Get-QnReleaseFileRecord $goodZip 'good.zip'
# Every archive fixture is newly allocated; no release or earlier fixture is overwritten.
$changed = @($entries | ForEach-Object { [pscustomobject]@{ name=$_.name; text=$_.text } })
($changed | Where-Object { $_.name -ceq 'quicknest.exe' }).text = 'fixture old exe bytes'
$zip = New-QnFixtureArchive 'same-version-old-exe.zip' $changed
Assert-QnFixtureRefuses 'same-version old EXE bytes in ZIP' { Assert-QnPortableArchive $zip $stage }
$changed = @($entries | ForEach-Object { [pscustomobject]@{ name=$_.name; text=$_.text } })
($changed | Where-Object { $_.name -ceq 'README.md' }).text = 'tampered README'
$zip = New-QnFixtureArchive 'tampered-readme.zip' $changed
Assert-QnFixtureRefuses 'ZIP document tampering' { Assert-QnPortableArchive $zip $stage }
$changed = @($entries | ForEach-Object { [pscustomobject]@{ name=$_.name; text=$_.text } })
($changed | Where-Object { $_.name -ceq 'QuickNest.portable' }).text = 'not empty'
$zip = New-QnFixtureArchive 'nonempty-marker.zip' $changed
Assert-QnFixtureRefuses 'nonempty portable marker' { Assert-QnPortableArchive $zip $stage }
$zip = New-QnFixtureArchive 'duplicate-five.zip' ($entries + $entries[3])
Assert-QnFixtureRefuses 'duplicate ZIP entries' { Assert-QnPortableArchive $zip $stage }
$changed = @($entries[0], $entries[1], $entries[3], $entries[3])
$zip = New-QnFixtureArchive 'duplicate-four.zip' $changed
Assert-QnFixtureRefuses 'duplicate ZIP replacing a required entry at exact entry count' { Assert-QnPortableArchive $zip $stage }
foreach ($name in @('extra.txt','sub/README.md','../README.md','readme.md','sub\README.md')) {
    $changed = @($entries | ForEach-Object { [pscustomobject]@{ name=$_.name; text=$_.text } })
    ($changed | Where-Object { $_.name -ceq 'README.md' }).name = $name
    $zip = New-QnFixtureArchive ('extra-' + [Guid]::NewGuid().ToString('N') + '.zip') $changed
    Assert-QnFixtureRefuses ('extra/noncanonical ZIP entry ' + $name) { Assert-QnPortableArchive $zip $stage }
}
$zip = New-QnFixtureArchive 'missing-marker.zip' @($entries | Where-Object { $_.name -cne 'QuickNest.portable' })
Assert-QnFixtureRefuses 'missing marker' { Assert-QnPortableArchive $zip $stage }
$append = [IO.File]::Open($goodZip, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::None)
try { $append.WriteByte(42) } finally { $append.Dispose() }
Assert-QnFixtureRefuses 'package byte tampering outside ZIP entries' { [void](Assert-QnReleaseFileRecord $fixtureDirectory $goodZipRecord) }
$setup = Assert-QnReleasePath (Join-Path $fixtureDirectory 'setup.exe') $fixtureDirectory
[IO.File]::WriteAllText($setup, 'compiler output placeholder', (New-Object Text.UTF8Encoding($false)))
$setupRecord = Get-QnReleaseFileRecord $setup 'setup.exe'
[IO.File]::WriteAllText($setup, 'modified compiler output', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'setup output hash tampering (without executing setup)' { [void](Assert-QnReleaseFileRecord $fixtureDirectory $setupRecord) }
$exePath = Assert-QnReleasePath (Join-Path $stageDirectory 'quicknest.exe') $stageDirectory
$locks = Open-QnReleaseStageLocks $fixtureDirectory $stage
try {
    [IO.File]::SetAttributes($exePath, [IO.FileAttributes]::Normal)
    Assert-QnFixtureRefuses 'staging replacement while shared read locks are held' { [IO.File]::WriteAllText($exePath, 'replacement', (New-Object Text.UTF8Encoding($false))) }
    [IO.File]::SetAttributes($exePath, [IO.FileAttributes]::ReadOnly)
} finally { foreach ($lock in $locks) { $lock.Dispose() } }
[IO.File]::SetAttributes($exePath, [IO.FileAttributes]::Normal)
Assert-QnFixtureRefuses 'writable staging input' { [void](Assert-QnReleaseStage $fixtureDirectory $stage) }
[IO.File]::WriteAllText($exePath, 'changed-stage-executable', (New-Object Text.UTF8Encoding($false)))
[IO.File]::SetAttributes($exePath, [IO.FileAttributes]::ReadOnly)
Assert-QnFixtureRefuses 'staging input changed after packaging' { [void](Assert-QnReleaseStage $fixtureDirectory $stage) }
# --- B11/D6: pinned release recipe, frozen source snapshot, schema-2 archives ---
$recipeFixture = [pscustomobject][ordered]@{
    'opt-level'='3'; 'debug'='false'; 'debug-assertions'='false'; 'overflow-checks'='false'
    'lto'='false'; 'codegen-units'='16'; 'incremental'='false'; 'panic'='unwind'; 'strip'='none'
}
$recipeCanonicalFixture = Assert-QnReleaseRecipe $recipeFixture
if (-not $recipeCanonicalFixture.StartsWith("quicknest-recipe-v2`nopt-level`t3`n")) { throw 'Canonical recipe header is unexpected' }
$infoV2 = Copy-QnFixtureInfo $info
$infoV2 | Add-Member -MemberType NoteProperty -Name 'recipe' -Value $recipeFixture
$infoV2.build_fingerprint = Get-QnReleaseBuildFingerprint $infoV2
Assert-QnReleaseBuildInfo $infoV2 $infoV2
if ($infoV2.build_fingerprint -ceq $info.build_fingerprint) { throw 'The recipe did not change the v2 build fingerprint' }
Write-Host 'PASS schema-2 build info with pinned recipe is consistent; fingerprint v2 differs from the still-recomputable v1 fingerprint'
foreach ($badRecipe in @(
    @{ name='invalid opt-level'; key='opt-level'; value='9' },
    @{ name='invalid debug'; key='debug'; value='sometimes' },
    @{ name='invalid debug-assertions'; key='debug-assertions'; value='maybe' },
    @{ name='invalid overflow-checks'; key='overflow-checks'; value='1' },
    @{ name='invalid lto'; key='lto'; value='fatter' },
    @{ name='invalid codegen-units'; key='codegen-units'; value='0' },
    @{ name='invalid incremental'; key='incremental'; value='TRUE' },
    @{ name='invalid panic'; key='panic'; value='crash' },
    @{ name='invalid strip'; key='strip'; value='everything' }
)) {
    $brokenRecipe = Copy-QnFixtureInfo $recipeFixture
    $brokenRecipe.($badRecipe.key) = $badRecipe.value
    Assert-QnFixtureRefuses $badRecipe.name { Assert-QnReleaseRecipe $brokenRecipe }
}
$extraKeyRecipe = Copy-QnFixtureInfo $recipeFixture
$extraKeyRecipe | Add-Member -MemberType NoteProperty -Name 'rustflags' -Value '-Ctarget-feature'
Assert-QnFixtureRefuses 'unexpected recipe key' { Assert-QnReleaseRecipe $extraKeyRecipe }
$missingKeyRecipe = Copy-QnFixtureInfo $recipeFixture
$missingKeyRecipe.PSObject.Properties.Remove('strip')
Assert-QnFixtureRefuses 'missing recipe key' { Assert-QnReleaseRecipe $missingKeyRecipe }
# B11 negative: an environment-injected profile override that disagrees with the
# recorded recipe is refused; a consistently recorded override is accepted.
$interferenceRecipe = Copy-QnFixtureInfo $recipeFixture
$interferenceRecipe.'debug-assertions' = 'true'
$interferenceInfo = Copy-QnFixtureInfo $infoV2
$interferenceInfo.recipe = $interferenceRecipe
$interferenceInfo.build_fingerprint = Get-QnReleaseBuildFingerprint $interferenceInfo
Assert-QnFixtureRefuses 'CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true refused against the false-assertions recipe' { Assert-QnReleaseBuildInfo $infoV2 $interferenceInfo }
Assert-QnReleaseBuildInfo $interferenceInfo $interferenceInfo
Write-Host 'PASS CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true recorded consistently is accepted with its own fingerprint'
$recipelessActual = Copy-QnFixtureInfo $info
Assert-QnFixtureRefuses 'recipe recorded only on the expected side' { Assert-QnReleaseBuildInfo $infoV2 $recipelessActual }
Assert-QnFixtureRefuses 'recipe recorded only on the executable side' { Assert-QnReleaseBuildInfo $info $infoV2 }
$tamperedRecipeInfo = Copy-QnFixtureInfo $infoV2
$tamperedRecipeInfo.recipe.'strip' = 'symbols'
Assert-QnFixtureRefuses 'recipe value changed under the recorded fingerprint' { Assert-QnReleaseBuildInfo $infoV2 $tamperedRecipeInfo }
# B10 static contract: both autostart registry branches must coexist in the
# installer script (selected task writes; deselected task removes an old Run
# value; a cancelled wizard runs no install step and never touches it).
$installerScript = [IO.File]::ReadAllText((Join-Path $root 'installer/QuickNest.iss'), [Text.Encoding]::UTF8)
if ($installerScript -cnotmatch 'ValueType:\s*string;\s*ValueName:\s*"QuickNest"[\s\S]{0,200}Flags:\s*uninsdeletevalue;\s*Tasks:\s*autostart') { throw 'Installer autostart selection branch is missing' }
if ($installerScript -cnotmatch 'ValueType:\s*none;\s*ValueName:\s*"QuickNest"[\s\S]{0,200}Flags:\s*deletevalue;\s*Check:\s*not WizardIsTaskSelected\(''autostart''\)') { throw 'Installer autostart removal branch is missing' }
Write-Host 'PASS installer registry contract keeps the autostart write branch and adds the deselected deletevalue branch'
$utf8Fixture = New-Object Text.UTF8Encoding($false)
function Repair-QnFixtureManifestChain([string]$Run) {
    # Re-links every manifest record after an out-of-band rewrite so that a
    # refusal exercises deeper checks than the record-hash layer.
    $sourceRecord = Get-QnReleaseFileRecord (Join-Path $Run 'source-manifest.json') 'source-manifest.json'
    $buildPath = Join-Path $Run 'build-manifest.json'
    $build = Read-QnReleaseJson $buildPath
    $build.source_manifest = $sourceRecord
    [IO.File]::WriteAllText($buildPath, (($build | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
    $buildRecord = Get-QnReleaseFileRecord $buildPath 'build-manifest.json'
    $stagingPath = Join-Path $Run 'staging-manifest.json'
    $staging = Read-QnReleaseJson $stagingPath
    $staging.build_manifest = $buildRecord
    [IO.File]::WriteAllText($stagingPath, (($staging | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
    $stagingRecord = Get-QnReleaseFileRecord $stagingPath 'staging-manifest.json'
    $evidencePath = Join-Path $Run 'setup-evidence.json'
    $evidence = Read-QnReleaseJson $evidencePath
    $evidence.staging_manifest = $stagingRecord
    [IO.File]::WriteAllText($evidencePath, (($evidence | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
    $evidenceRecord = Get-QnReleaseFileRecord $evidencePath 'setup-evidence.json'
    $releasePath = Join-Path $Run 'release-manifest.json'
    $release = Read-QnReleaseJson $releasePath
    $release.source_manifest = $sourceRecord
    $release.build_manifest = $buildRecord
    $release.staging_manifest = $stagingRecord
    $release.setup_evidence = $evidenceRecord
    [IO.File]::WriteAllText($releasePath, (($release | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
}
function New-QnFixtureArchiveRun {
    # Pure text doubles: a complete synthetic schema-2 release run that never
    # existed in a real build (version 9.9.9, synthetic base HEAD).
    $run = Assert-QnReleasePath (Join-Path $fixtureDirectory ('v2run-' + [Guid]::NewGuid().ToString('N'))) $fixtureDirectory
    [void][IO.Directory]::CreateDirectory($run)
    [IO.File]::WriteAllText((Join-Path $run '.quicknest-release-run'), 'QuickNest local release v2', (New-Object Text.UTF8Encoding($false)))
    $snapshotRoot = Assert-QnReleasePath (Join-Path $run 'source-files') $run
    [void][IO.Directory]::CreateDirectory($snapshotRoot)
    $snapshotTexts = [ordered]@{
        'README.md' = 'fixture readme instructions'
        'assets/QuickNest.ico' = 'icon-v2'
        'installer/QuickNest.iss' = 'fixture frozen installer script'
        'src/main.rs' = 'fn main() {} fixture source'
    }
    $sourceRecords = @()
    foreach ($relative in $snapshotTexts.Keys) {
        $destination = Assert-QnReleasePath (Join-Path $snapshotRoot ($relative.Replace('/', '\'))) $run
        $parent = Split-Path -Parent $destination
        if (-not (Test-Path -LiteralPath $parent)) { [void][IO.Directory]::CreateDirectory($parent) }
        [IO.File]::WriteAllText($destination, $snapshotTexts[$relative], (New-Object Text.UTF8Encoding($false)))
        $sourceRecords += Get-QnReleaseFileRecord $destination $relative
    }
    $canonical = New-Object Text.StringBuilder
    [void]$canonical.Append("quicknest-source-v1`n")
    foreach ($record in $sourceRecords) {
        [void]$canonical.Append($record.path).Append("`t").Append($record.bytes.ToString([Globalization.CultureInfo]::InvariantCulture)).Append("`t").Append($record.sha256).Append("`n")
    }
    $sourceFingerprint = Get-QnReleaseTextHash $canonical.ToString()
    $source = [pscustomobject][ordered]@{
        schema_version = 1; version = '9.9.9'; base_head = ('f' * 40); dirty = $false
        git_status = @(); untracked_source = @()
        fingerprint_algorithm = 'sha256/quicknest-source-v1-path-tab-bytes-tab-sha256-lf'
        excluded_directories = @(); source_fingerprint = $sourceFingerprint; files = $sourceRecords
    }
    Write-QnReleaseJson (Join-Path $run 'source-manifest.json') $source
    $stageDirectory = Assert-QnReleasePath (Join-Path $run 'staging') $run
    [void][IO.Directory]::CreateDirectory($stageDirectory)
    $stageTexts = [ordered]@{ 'QuickNest.ico'='icon-v2'; 'QuickNest.portable'=''; 'README.md'='fixture readme instructions'; 'quicknest.exe'='fixture archive exe bytes' }
    $stageRecords = @(); $zipEntries = @()
    foreach ($name in $stageTexts.Keys) {
        $path = Assert-QnReleasePath (Join-Path $stageDirectory $name) $run
        [IO.File]::WriteAllText($path, $stageTexts[$name], (New-Object Text.UTF8Encoding($false)))
        [IO.File]::SetAttributes($path, [IO.FileAttributes]::ReadOnly)
        $stageRecords += Get-QnReleaseFileRecord $path $name
        $zipEntries += [pscustomobject]@{ name=$name; text=$stageTexts[$name] }
    }
    [IO.File]::WriteAllText((Join-Path $run 'cargo-build.log'), '[stdout]' + "`n" + 'fixture build log', (New-Object Text.UTF8Encoding($false)))
    [IO.File]::WriteAllText((Join-Path $run 'inno-compile.log'), '[stdout]' + "`n" + 'fixture compile log', (New-Object Text.UTF8Encoding($false)))
    $setupName = 'QuickNest-9.9.9-setup.exe'
    [IO.File]::WriteAllText((Join-Path $run $setupName), 'fixture compiler output', (New-Object Text.UTF8Encoding($false)))
    [void](New-QnFixtureArchive 'QuickNest-9.9.9-portable.zip' $zipEntries $run)
    $recipe = Copy-QnFixtureInfo $recipeFixture
    $info = [pscustomobject][ordered]@{
        schema_version=1; version='9.9.9'; provenance='recorded'
        source_fingerprint=$sourceFingerprint; build_fingerprint=''; base_head=('f' * 40); dirty=$false
        target='x86_64-pc-windows-msvc'; profile='release'; rustc='fixture archive rustc'; cargo='fixture archive cargo'
        update_key=[pscustomobject]@{ status='disabled'; blob_sha256=$null }
        recipe=$recipe
    }
    $info.build_fingerprint = Get-QnReleaseBuildFingerprint $info
    $originalRoot = Assert-QnReleasePath (Join-Path $fixtureDirectory ('original-root-' + [Guid]::NewGuid().ToString('N')))
    $originalContext = [pscustomobject][ordered]@{
        repository_root = $originalRoot; release_directory = $run
        staging_directory = $stageDirectory; source_snapshot_directory = $snapshotRoot
    }
    $pinned = [ordered]@{
        CARGO_PROFILE_RELEASE_OPT_LEVEL='3'; CARGO_PROFILE_RELEASE_DEBUG='false'
        CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS='false'; CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS='false'
        CARGO_PROFILE_RELEASE_LTO='false'; CARGO_PROFILE_RELEASE_CODEGEN_UNITS='16'
        CARGO_PROFILE_RELEASE_INCREMENTAL='false'; CARGO_PROFILE_RELEASE_PANIC='unwind'
        CARGO_PROFILE_RELEASE_STRIP='none'
    }
    $compilerArguments = @('/DMyAppVersion=9.9.9', ('/DReleaseStageDir=' + $stageDirectory), ('/DReleaseOutputDir=' + $run), (Join-Path $originalRoot 'installer/QuickNest.iss'))
    $build = [pscustomobject][ordered]@{
        schema_version = 2; version = '9.9.9'
        source_manifest = Get-QnReleaseFileRecord (Join-Path $run 'source-manifest.json') 'source-manifest.json'
        build_info = $info
        executable = @($stageRecords | Where-Object { $_.path -ceq 'quicknest.exe' })[0]
        build_recipe = $recipe
        original_context = $originalContext
        cleared_build_environment = @('QUICKNEST_UPDATE_PUBLIC_KEY_HEX')
        cleared_profile_environment = @()
        pinned_profile_environment = $pinned
        cargo_arguments = @('build','--locked','--offline','--release','--bin','quicknest'); cargo_exit_code = 0
        cargo_log = Get-QnReleaseFileRecord (Join-Path $run 'cargo-build.log') 'cargo-build.log'
        update_health_protocol_verified = $true; desktop_validation_executed = $false
    }
    Write-QnReleaseJson (Join-Path $run 'build-manifest.json') $build
    $stage = [pscustomobject][ordered]@{
        schema_version = 1; version = '9.9.9'; build_fingerprint = $info.build_fingerprint
        build_manifest = Get-QnReleaseFileRecord (Join-Path $run 'build-manifest.json') 'build-manifest.json'
        files = $stageRecords
    }
    Write-QnReleaseJson (Join-Path $run 'staging-manifest.json') $stage
    $evidence = [pscustomobject][ordered]@{
        schema_version = 1; evidence_level = 'frozen-input-and-compiler'
        staging_manifest = Get-QnReleaseFileRecord (Join-Path $run 'staging-manifest.json') 'staging-manifest.json'
        installer_source_sha256 = (@($sourceRecords | Where-Object { $_.path -ceq 'installer/QuickNest.iss' })[0]).sha256
        compiler_path = 'C:\fixture-tools\ISCC.exe'
        compiler = [pscustomobject]@{ path='ISCC.exe'; bytes=1234; sha256=('c' * 64) }
        compiler_version = '6.5.1 fixture'
        compiler_arguments = $compilerArguments; compiler_exit_code = 0
        compiler_log = Get-QnReleaseFileRecord (Join-Path $run 'inno-compile.log') 'inno-compile.log'
        output = Get-QnReleaseFileRecord (Join-Path $run $setupName) $setupName
        installation_executed = $false; unpacking_verified = $false; desktop_validation_executed = $false
    }
    Write-QnReleaseJson (Join-Path $run 'setup-evidence.json') $evidence
    $release = [pscustomobject][ordered]@{
        schema_version = 2; version = '9.9.9'
        source_fingerprint = $sourceFingerprint; build_fingerprint = $info.build_fingerprint
        source_manifest = $build.source_manifest
        build_manifest = $stage.build_manifest; staging_manifest = $evidence.staging_manifest
        setup_evidence = Get-QnReleaseFileRecord (Join-Path $run 'setup-evidence.json') 'setup-evidence.json'
        portable = Get-QnReleaseFileRecord (Join-Path $run 'QuickNest-9.9.9-portable.zip') 'QuickNest-9.9.9-portable.zip'
        setup = $evidence.output
        build_recipe = $recipe
        installation_executed = $false; desktop_validation_executed = $false
    }
    Write-QnReleaseJson (Join-Path $run 'release-manifest.json') $release
    return $run
}
$freshArchiveRun = New-QnFixtureArchiveRun
[void](Assert-QnReleaseFrozenArchive $freshArchiveRun)
Write-Host 'PASS schema-2 archive verifies purely from frozen evidence (synthetic 9.9.9 version and HEAD, no Cargo/Git/current source; HEAD or version advances cannot cause false reports)'
$movedName = (Split-Path -Leaf $freshArchiveRun) + '-moved'
Rename-Item -LiteralPath $freshArchiveRun -NewName $movedName
[void](Assert-QnReleaseFrozenArchive (Join-Path $fixtureDirectory $movedName))
Write-Host 'PASS renamed/moved archive still verifies; the recorded original_context stays authoritative for compiler arguments'
$mutateRun = New-QnFixtureArchiveRun
[IO.File]::Move((Join-Path $mutateRun 'source-files\README.md'), (Join-Path $fixtureDirectory ('orphan-' + [Guid]::NewGuid().ToString('N') + '.txt')))
Assert-QnFixtureRefuses 'snapshot missing a recorded source file' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
[IO.File]::WriteAllText((Join-Path $mutateRun 'source-files\extra.txt'), 'unrecorded', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'snapshot contains an extra unrecorded file' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
[IO.File]::WriteAllText((Join-Path $mutateRun 'source-files\src\main.rs'), 'tampered source bytes', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'snapshot source bytes tampered' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$manifestPath = Join-Path $mutateRun 'source-manifest.json'
$manifestObject = Read-QnReleaseJson $manifestPath
$manifestObject.source_fingerprint = ('0' * 64)
[IO.File]::WriteAllText($manifestPath, (($manifestObject | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
Repair-QnFixtureManifestChain $mutateRun
Assert-QnFixtureRefuses 'source manifest fingerprint tampered under an intact snapshot' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$exePath = Join-Path $mutateRun 'staging\quicknest.exe'
[IO.File]::SetAttributes($exePath, [IO.FileAttributes]::Normal)
[IO.File]::WriteAllText($exePath, 'tampered archive exe', (New-Object Text.UTF8Encoding($false)))
[IO.File]::SetAttributes($exePath, [IO.FileAttributes]::ReadOnly)
Assert-QnFixtureRefuses 'archive EXE bytes tampered' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$zipPath = Join-Path $mutateRun 'QuickNest-9.9.9-portable.zip'
$append = [IO.File]::Open($zipPath, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::None)
try { $append.WriteByte(42) } finally { $append.Dispose() }
Assert-QnFixtureRefuses 'archive ZIP bytes tampered' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
[IO.File]::WriteAllText((Join-Path $mutateRun 'QuickNest-9.9.9-setup.exe'), 'tampered compiler output', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'archive setup output tampered' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
[IO.File]::WriteAllText((Join-Path $mutateRun 'cargo-build.log'), '[stdout]' + "`n" + 'tampered build log', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'archive build log tampered' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$releasePath = Join-Path $mutateRun 'release-manifest.json'
$releaseObject = Read-QnReleaseJson $releasePath
$releaseObject.schema_version = 1
[IO.File]::WriteAllText($releasePath, (($releaseObject | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
Assert-QnFixtureRefuses 'release manifest downgraded to schema 1 inside a v2 archive' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$buildPath = Join-Path $mutateRun 'build-manifest.json'
$buildObject = Read-QnReleaseJson $buildPath
$buildObject.schema_version = 1
$buildObject.PSObject.Properties.Remove('build_recipe')
$buildObject.PSObject.Properties.Remove('original_context')
[IO.File]::WriteAllText($buildPath, (($buildObject | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
Repair-QnFixtureManifestChain $mutateRun
Assert-QnFixtureRefuses 'build manifest downgraded to schema 1 (v1/v2 mixing) refused' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$buildPath = Join-Path $mutateRun 'build-manifest.json'
$buildObject = Read-QnReleaseJson $buildPath
$buildObject.build_recipe.'opt-level' = '2'
[IO.File]::WriteAllText($buildPath, (($buildObject | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
Repair-QnFixtureManifestChain $mutateRun
Assert-QnFixtureRefuses 'manifest recipe differing from the compiled recipe refused' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
$evidencePath = Join-Path $mutateRun 'setup-evidence.json'
$evidenceObject = Read-QnReleaseJson $evidencePath
$evidenceObject.compiler_arguments[1] = '/DReleaseStageDir=C:\somewhere-else\staging'
[IO.File]::WriteAllText($evidencePath, (($evidenceObject | ConvertTo-Json -Depth 32) + "`n"), $utf8Fixture)
Repair-QnFixtureManifestChain $mutateRun
Assert-QnFixtureRefuses 'compiler arguments unbound from the recorded frozen context' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
[IO.File]::WriteAllText((Join-Path $mutateRun '.quicknest-release-run'), 'QuickNest local release v1', (New-Object Text.UTF8Encoding($false)))
Assert-QnFixtureRefuses 'schema-2 archive with a v1 run marker' { Assert-QnReleaseFrozenArchive $mutateRun }
$mutateRun = New-QnFixtureArchiveRun
[IO.Directory]::Move((Join-Path $mutateRun 'source-files'), (Join-Path $fixtureDirectory ('orphan-snapshot-' + [Guid]::NewGuid().ToString('N'))))
Assert-QnFixtureRefuses 'archive without its source snapshot' { Assert-QnReleaseFrozenArchive $mutateRun }
Assert-QnFixtureRefuses 'snapshot validator itself refuses a missing snapshot root' { [void](Assert-QnReleaseSnapshot (Join-Path $mutateRun 'no-such-subdir') (Read-QnReleaseJson (Join-Path $mutateRun 'source-manifest.json'))) }
Write-Host ('Release fixtures passed: ' + $script:fixturePasses + ' negative cases; version=' + $version + '; no application/installer/desktop was run')
Write-Host ('Fixture artifacts: ' + $fixtureDirectory)
