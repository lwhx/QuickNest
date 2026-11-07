#requires -Version 5.1
# Writes only a fresh OS-temp fixture. No recursive deletion or certificate store changes.
param([switch]$SkipHttps)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $repo 'scripts/test-sandbox.ps1')
$sandboxSnapshot = Get-QnEnvironmentSnapshot
$originalFixture = $env:QUICKNEST_UPDATE_TEST_FIXTURE
$work = $null
$server = $null
$utf8 = [Text.UTF8Encoding]::new($false)
try {
    $case = Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path -FixtureOnly
    $work = Join-Path $case 'updater'
    [void][IO.Directory]::CreateDirectory($work)
    Write-Host ('Isolated non-desktop fixture: ' + $work)
    & (Join-Path $repo 'scripts/update-keygen.ps1') -OutputDirectory (Join-Path $work 'keys')
    [IO.File]::Copy((Join-Path $work 'keys/public.hex'), (Join-Path $work 'public.hex'))
    [IO.File]::WriteAllBytes((Join-Path $work 'package.exe'), [Text.Encoding]::ASCII.GetBytes('new-version-1.2.1-fixture'))
    [IO.File]::WriteAllBytes((Join-Path $work 'old.exe'), [Text.Encoding]::ASCII.GetBytes('old-version-1.2.0-fixture'))
    if ((Get-FileHash (Join-Path $work 'old.exe')).Hash -eq (Get-FileHash (Join-Path $work 'package.exe')).Hash) { throw 'Old/new fixture must differ' }
    $probe = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $probe.Start(); $port = ([Net.IPEndPoint]$probe.LocalEndpoint).Port; $probe.Stop()
    $payload = @{ version = '1.2.1'; package_url = "https://localhost:$port/package"; package_sha256 = (Get-FileHash (Join-Path $work 'package.exe') -Algorithm SHA256).Hash.ToLowerInvariant(); package_kind = 'raw'; notes = 'isolated fixture' } | ConvertTo-Json -Compress
    [IO.File]::WriteAllText((Join-Path $work 'payload.json'), $payload, $utf8)
    & (Join-Path $repo 'scripts/update-sign.ps1') -PayloadFile (Join-Path $work 'payload.json') -OutFile (Join-Path $work 'signature.hex') -PrivateKeyFile (Join-Path $work 'keys/private.hex')
    $signature = [IO.File]::ReadAllText((Join-Path $work 'signature.hex'))
    [IO.File]::WriteAllText((Join-Path $work 'manifest.json'), (@{ payload = $payload; signature = $signature } | ConvertTo-Json -Compress), $utf8)
    $data = Join-Path $work 'data'; [void][IO.Directory]::CreateDirectory($data)
    [IO.File]::WriteAllText((Join-Path $data 'sentinel'), 'never touch user data', $utf8)
    $before = (Get-FileHash (Join-Path $data 'sentinel') -Algorithm SHA256).Hash
    $env:QUICKNEST_UPDATE_TEST_FIXTURE = $work
    & cargo test --manifest-path (Join-Path $repo 'Cargo.toml') --lib updater -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Updater unit/fixture tests failed' }
    & cargo test --manifest-path (Join-Path $repo 'Cargo.toml') --lib updater_validation::tests::signed_fixture_acceptance -- --ignored --exact
    if ($LASTEXITCODE -ne 0) { throw 'Ephemeral signed fixture test failed' }
    & cargo test --manifest-path (Join-Path $repo 'Cargo.toml') --lib update_net -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Transport unit tests failed' }
    if (-not $SkipHttps) {
        $ready = Join-Path $work 'ready'; $stop = Join-Path $work 'stop'; $certificate = Join-Path $work 'localhost.cer'
        $serverScript = Join-Path $repo 'scripts/update-server.ps1'
        $server = Start-QnOwnedProcess -FilePath (Get-Process -Id $PID).Path -ArgumentList @('-NoProfile', '-File', ('"' + $serverScript + '"'), '-Port', $port, '-ManifestFile', ('"' + (Join-Path $work 'manifest.json') + '"'), '-PackageFile', ('"' + (Join-Path $work 'package.exe') + '"'), '-ReadyFile', ('"' + $ready + '"'), '-StopFile', ('"' + $stop + '"'), '-LogFile', ('"' + (Join-Path $work 'server.log') + '"'), '-CertificateFile', ('"' + $certificate + '"')) -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds(15)
        while (-not (Test-Path -LiteralPath $ready) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
        if (-not (Test-Path -LiteralPath $ready) -or [IO.File]::ReadAllText($ready) -ne 'ready') { throw 'TLS fixture did not start' }
        # Trust only this certificate inside this handler. No global callback/store mutation.
        Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Net;
using System.Security.Cryptography.X509Certificates;
public static class UpdateFixtureClient {
    public static HttpWebResponse Fetch(string url, string certificate) {
        string expected;
        using (var cert = new X509Certificate2(certificate)) { expected = cert.GetCertHashString(); }
        var request = (HttpWebRequest)WebRequest.Create(url);
        request.AllowAutoRedirect = false;
        request.Timeout = 5000;
        request.ReadWriteTimeout = 5000;
        request.Proxy = null;
        request.ServerCertificateValidationCallback = (sender, remote, chain, errors) =>
            remote != null && remote.GetCertHashString() == expected && request.RequestUri.Host == "localhost";
        try { return (HttpWebResponse)request.GetResponse(); }
        catch (WebException e) { if (e.Response != null) { return (HttpWebResponse)e.Response; } throw; }
    }
    public static string Text(string url, string certificate) {
        using (var response = Fetch(url, certificate)) {
            if ((int)response.StatusCode != 200) { throw new Exception("Expected HTTP 200"); }
            using (var reader = new StreamReader(response.GetResponseStream())) { return reader.ReadToEnd(); }
        }
    }
    public static int Status(string url, string certificate) {
        using (var response = Fetch(url, certificate)) { return (int)response.StatusCode; }
    }
}
'@
        $received = [UpdateFixtureClient]::Text("https://localhost:$port/manifest.json", $certificate)
        if ($received -ne [IO.File]::ReadAllText((Join-Path $work 'manifest.json'))) { throw 'TLS manifest mismatch' }
        if ([UpdateFixtureClient]::Status("https://localhost:$port/missing", $certificate) -ne 404) { throw 'Expected real 404' }
        if ([UpdateFixtureClient]::Status("https://localhost:$port/redirect-http", $certificate) -ne 302) { throw 'Redirect followed unexpectedly' }
        Write-Host 'Temporary TLS fixture passed with request-local pinning; production WinInet certificate validation is not bypassed or claimed tested by this fixture.'
    }
    $after = (Get-FileHash (Join-Path $data 'sentinel') -Algorithm SHA256).Hash
    if ($before -ne $after -or @(Get-ChildItem -LiteralPath $data).Count -ne 1) { throw 'Isolated data snapshot changed during actual operations' }
    Write-Host 'Updater isolated validation passed. Production process-exit/relaunch integration requires distinct release binaries and is not claimed by this fixture.'
} finally {
    try {
        if ($server) {
            [IO.File]::WriteAllText((Join-Path $work 'stop'), 'stop', [Text.Encoding]::ASCII)
            if (-not $server.WaitForExit(10000)) { Stop-QnOwnedProcess -Process $server -Force }
        }
        Stop-QnTestProcesses
        if ($work) {
            $private = Join-Path $work 'keys/private.hex'
            if (Test-Path -LiteralPath $private) { Remove-QnSandboxItem -LiteralPath $private -Force }
            Write-Host ('Evidence retained at ' + $work)
        }
    } finally {
        $env:QUICKNEST_UPDATE_TEST_FIXTURE = $originalFixture
        Restore-QnEnvironment $sandboxSnapshot
    }
}
