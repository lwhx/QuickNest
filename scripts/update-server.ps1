#requires -Version 5.1
# Ephemeral localhost TLS server. CertificateRequest creates an in-memory key;
# no Windows certificate store, netsh binding, or system trust is modified.
param(
    [Parameter(Mandatory = $true)][int]$Port,
    [Parameter(Mandatory = $true)][string]$ManifestFile,
    [Parameter(Mandatory = $true)][string]$PackageFile,
    [Parameter(Mandatory = $true)][string]$ReadyFile,
    [Parameter(Mandatory = $true)][string]$StopFile,
    [Parameter(Mandatory = $true)][string]$LogFile,
    [Parameter(Mandatory = $true)][string]$CertificateFile
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$listener = $null; $cert = $null; $rsa = $null; $tlsKey = $null
function Write-Log([string]$message) { [IO.File]::AppendAllText($LogFile, ([DateTime]::UtcNow.ToString('o') + "`t$message`n"), [Text.UTF8Encoding]::new($false)) }
try {
    $rsa = [Security.Cryptography.RSA]::Create(2048)
    $request = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=localhost', $rsa, [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $san = [Security.Cryptography.X509Certificates.SubjectAlternativeNameBuilder]::new()
    $san.AddDnsName('localhost'); $san.AddIpAddress([Net.IPAddress]::Loopback)
    $request.CertificateExtensions.Add($san.Build())
    $cert = $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddMinutes(-1), [DateTimeOffset]::UtcNow.AddHours(1))
    # .NET Framework Schannel cannot use CertificateRequest's ephemeral RSA
    # handle directly. A UNIQUE TEMPORARY CNG key is persisted for Schannel,
    # associated with the in-memory certificate, and deleted in finally.
    # No certificate store or trusted root is involved.
    $keyParameters = [Security.Cryptography.CspParameters]::new(24)
    $keyParameters.KeyContainerName = 'QuickNest-TLS-fixture-' + [Guid]::NewGuid().ToString('N')
    $tlsKey = [Security.Cryptography.RSACryptoServiceProvider]::new(2048, $keyParameters)
    $tlsKey.PersistKeyInCsp = $true
    $tlsKey.ImportParameters($rsa.ExportParameters($true))
    $cert.PrivateKey = $tlsKey
    [IO.File]::WriteAllBytes($CertificateFile, $cert.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert))
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
    $listener.Start()
    [IO.File]::WriteAllText($ReadyFile, 'ready', [Text.Encoding]::ASCII)
    while (-not (Test-Path -LiteralPath $StopFile)) {
        if (-not $listener.Pending()) { Start-Sleep -Milliseconds 50; continue }
        $client = $listener.AcceptTcpClient()
        $client.ReceiveTimeout = 5000; $client.SendTimeout = 5000
        $ssl = $null
        try {
            $ssl = [Net.Security.SslStream]::new($client.GetStream(), $false)
            $ssl.ReadTimeout = 5000; $ssl.WriteTimeout = 5000
            $ssl.AuthenticateAsServer($cert, $false, [Security.Authentication.SslProtocols]::Tls12, $false)
            $reader = [IO.StreamReader]::new($ssl, [Text.Encoding]::ASCII, $false, 8192, $true)
            try {
                $line = $reader.ReadLine(); $path = '/'
                if ($line -match '^GET (\S+) HTTP/1\.[01]$') { $path = ($Matches[1] -split '\?')[0] }
                $headerBytes = 0
                do { $line = $reader.ReadLine(); if ($null -ne $line) { $headerBytes += $line.Length }; if ($headerBytes -gt 16384) { throw 'oversized headers' } } while ($line)
            } finally { $reader.Dispose() }
            Write-Log ('request ' + $path)
            $status = '200 OK'; $extra = ''
            $bytes = switch ($path) {
                '/manifest.json' { [IO.File]::ReadAllBytes($ManifestFile) }
                '/package' { [IO.File]::ReadAllBytes($PackageFile) }
                '/redirect-http' { $status = '302 Found'; $extra = "Location: http://localhost:$Port/package`r`n"; [Text.Encoding]::ASCII.GetBytes('redirect') }
                default { $status = '404 Not Found'; [Text.Encoding]::ASCII.GetBytes('missing') }
            }
            $header = [Text.Encoding]::ASCII.GetBytes("HTTP/1.1 $status`r`n${extra}Content-Length: $($bytes.Length)`r`nConnection: close`r`n`r`n")
            $ssl.Write($header, 0, $header.Length); $ssl.Write($bytes, 0, $bytes.Length); $ssl.Flush()
        } catch { Write-Log ('client error: ' + $_.Exception.Message) }
        finally { if ($ssl) { $ssl.Dispose() }; $client.Dispose() }
    }
} catch {
    Write-Log ('server error: ' + $_.Exception.Message)
    [IO.File]::WriteAllText($ReadyFile, ('error: ' + $_.Exception.Message), [Text.Encoding]::ASCII)
    throw
} finally {
    if ($listener) { $listener.Stop() }
    if ($cert) { $cert.Dispose() }; if ($rsa) { $rsa.Dispose() }
    if ($tlsKey) { $tlsKey.PersistKeyInCsp = $false; $tlsKey.Clear(); $tlsKey.Dispose() }
}
