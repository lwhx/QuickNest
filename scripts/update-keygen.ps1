#requires -Version 5.1
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path))).TrimEnd('\')
$output = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\')
if ($output.Equals($repo, [StringComparison]::OrdinalIgnoreCase) -or $output.StartsWith($repo + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Signing identities must be generated outside the source checkout.'
}
if (Test-Path -LiteralPath $output) { throw 'OutputDirectory must be a new directory; nothing is overwritten.' }
[void][IO.Directory]::CreateDirectory($output)
$ecdsa = $null
try {
    $ecdsa = [Security.Cryptography.ECDsaCng]::new([Security.Cryptography.ECCurve]::CreateFromFriendlyName('nistP256'))
    $ecdsa.HashAlgorithm = [Security.Cryptography.CngAlgorithm]::Sha256
    $public = $ecdsa.Key.Export([Security.Cryptography.CngKeyBlobFormat]::EccPublicBlob)
    $private = $ecdsa.Key.Export([Security.Cryptography.CngKeyBlobFormat]::EccPrivateBlob)
    foreach ($entry in @(@('public.hex', $public), @('private.hex', $private))) {
        $stream = [IO.File]::Open((Join-Path $output $entry[0]), [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try {
            $bytes = [Text.Encoding]::ASCII.GetBytes(([BitConverter]::ToString($entry[1])).Replace('-', '').ToLowerInvariant())
            $stream.Write($bytes, 0, $bytes.Length)
            $stream.Flush($true)
        } finally { $stream.Dispose() }
    }
    Write-Host ('Identity generated in external directory: ' + $output)
} finally { if ($ecdsa) { $ecdsa.Dispose() } }
