#requires -Version 5.1
param([Parameter(Mandatory=$true)][string]$ReleaseDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/release-metadata.ps1')
$context = Read-QnReleaseContext $root $ReleaseDirectory -ProbeExecutable
$version = Get-QnCargoVersion $root
$zipPath = Assert-QnReleasePath (Join-Path $context.directory ('QuickNest-' + $version + '-portable.zip')) $context.directory
if (Test-Path -LiteralPath $zipPath) { throw 'Portable output already exists; allocate a new release run' }
$stageManifestHash = Get-QnReleaseHash (Join-Path $context.directory 'staging-manifest.json')
$locks = Open-QnReleaseStageLocks $context.directory $context.stage
try {
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $stream = [IO.File]::Open($zipPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        $archive = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
        try {
            foreach ($record in $context.stage.files) {
                [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, (Join-Path $context.staging_directory $record.path), $record.path, [IO.Compression.CompressionLevel]::Optimal)
            }
        } finally { $archive.Dispose() }
    } finally { $stream.Dispose() }
    [void](Assert-QnReleaseStage $context.directory $context.stage)
    if ((Get-QnReleaseHash (Join-Path $context.directory 'staging-manifest.json')) -cne $stageManifestHash) { throw 'Staging manifest was replaced during portable packaging' }
    [void](Assert-QnReleaseSourceUnchanged $root $context.source)
    Assert-QnPortableArchive $zipPath $context.stage
    Write-Host ('Portable archive verified: ' + $zipPath)
} finally { foreach ($lock in $locks) { $lock.Dispose() } }
