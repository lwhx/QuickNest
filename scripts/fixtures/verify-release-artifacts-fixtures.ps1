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
function New-QnFixtureArchive([string]$Name, [object[]]$Entries) {
    $path = Assert-QnReleasePath (Join-Path $fixtureDirectory $Name) $fixtureDirectory
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
Write-Host ('Release fixtures passed: ' + $script:fixturePasses + ' negative cases; version=' + $version + '; no application/installer/desktop was run')
Write-Host ('Fixture artifacts: ' + $fixtureDirectory)
