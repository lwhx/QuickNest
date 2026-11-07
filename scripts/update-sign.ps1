#requires -Version 5.1
param(
    [Parameter(Mandatory = $true)][string]$PayloadFile,
    [Parameter(Mandatory = $true)][string]$OutFile,
    [Parameter(Mandatory = $true)][string]$PrivateKeyFile
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = [IO.Path]::GetFullPath((Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path))).TrimEnd('\')
$keyPath = [IO.Path]::GetFullPath($PrivateKeyFile)
if ($keyPath.StartsWith($repo + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing a signing private key inside the checkout.' }
$privateHex = [IO.File]::ReadAllText($keyPath)
if ($privateHex -cnotmatch '^[0-9a-fA-F]{208}$') { throw 'Expected a 104-byte BCrypt EccPrivateBlob, without BOM or whitespace.' }
$privateBytes = [byte[]]::new(104)
for ($i = 0; $i -lt 104; $i++) { $privateBytes[$i] = [Convert]::ToByte($privateHex.Substring($i * 2, 2), 16) }
$ecdsa = $null
try {
    $ecdsa = [Security.Cryptography.ECDsaCng]::new([Security.Cryptography.CngKey]::Import($privateBytes, [Security.Cryptography.CngKeyBlobFormat]::EccPrivateBlob))
    $ecdsa.HashAlgorithm = [Security.Cryptography.CngAlgorithm]::Sha256
    $payload = [IO.File]::ReadAllBytes($PayloadFile)
    if ($payload.Length -gt 65536 -or ($payload.Length -ge 3 -and $payload[0] -eq 239 -and $payload[1] -eq 187 -and $payload[2] -eq 191)) { throw 'Payload must be bounded UTF-8 without BOM.' }
    $signature = $ecdsa.SignData($payload, [Security.Cryptography.HashAlgorithmName]::SHA256)
    if ($signature.Length -ne 64) { throw 'Expected IEEE P1363 ECDSA r||s signature.' }
    $bytes = [Text.Encoding]::ASCII.GetBytes(([BitConverter]::ToString($signature)).Replace('-', '').ToLowerInvariant())
    $stream = [IO.File]::Open($OutFile, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
} finally {
    if ($ecdsa) { $ecdsa.Dispose() }
    [Array]::Clear($privateBytes, 0, $privateBytes.Length)
}
