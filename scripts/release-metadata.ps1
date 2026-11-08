#requires -Version 5.1
# Shared, side-effect-free definitions for the local release entry and verifier.
# Source fingerprints cover actual bytes, including untracked/ignored source.
function Assert-QnReleasePath([string]$Path, [string]$Root = '') {
    if ([string]::IsNullOrWhiteSpace($Path) -or -not [IO.Path]::IsPathRooted($Path) -or $Path -match '[*?\[\]\r\n\t]' -or $Path -match '::') { throw 'Release path must be an absolute literal path' }
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\','/')
    if ($Root) {
        $base = [IO.Path]::GetFullPath($Root).TrimEnd('\','/')
        if (-not $full.StartsWith($base + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw ('Release path escapes its allocated root: ' + $full) }
    }
    $cursor = $full
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            if (((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw ('Release reparse point refused: ' + $cursor) }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if ($parent -eq $cursor) { break }
        $cursor = $parent
    }
    return $full
}
function Assert-QnReleaseOutputParent([string]$Root, [string]$Parent) {
    $Root = Assert-QnReleasePath $Root
    $Parent = Assert-QnReleasePath $Parent
    $inside = $Parent.Equals($Root, [StringComparison]::OrdinalIgnoreCase) -or $Parent.StartsWith($Root + '\', [StringComparison]::OrdinalIgnoreCase)
    if ($inside) {
        $allowed = [IO.Path]::GetFullPath((Join-Path $Root 'installer/output'))
        if (-not $Parent.Equals($allowed, [StringComparison]::OrdinalIgnoreCase) -and -not $Parent.StartsWith($allowed + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'In-repository output must be below installer/output' }
    }
    return $Parent
}
function Get-QnReleaseBytesHash([byte[]]$Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose() }
}
function Get-QnReleaseTextHash([string]$Text) {
    return Get-QnReleaseBytesHash ([Text.Encoding]::UTF8.GetBytes($Text))
}
function Get-QnReleaseHash([string]$Path) {
    [void](Assert-QnReleasePath $Path)
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose(); $stream.Dispose() }
}
function Get-QnReleaseFileRecord([string]$Path, [string]$Name) {
    $file = Get-Item -LiteralPath (Assert-QnReleasePath $Path) -Force
    if ($file.PSIsContainer) { throw 'A release input must be a file' }
    return [pscustomobject][ordered]@{ path = $Name; bytes = [long]$file.Length; sha256 = Get-QnReleaseHash $file.FullName }
}
function Write-QnReleaseJson([string]$Path, $Value) {
    [void](Assert-QnReleasePath $Path)
    $json = ($Value | ConvertTo-Json -Depth 32) + "`n"
    $bytes = (New-Object Text.UTF8Encoding($false)).GetBytes($json)
    # Never replace an earlier run, manifest, archive or evidence file.
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try { $stream.Write($bytes, 0, $bytes.Length) } finally { $stream.Dispose() }
}
function Read-QnReleaseJson([string]$Path) {
    [void](Assert-QnReleasePath $Path)
    return [IO.File]::ReadAllText($Path, [Text.Encoding]::UTF8) | ConvertFrom-Json
}
function Invoke-QnReleaseGit([string]$Root, [string[]]$Arguments) {
    $output = @(& git -C $Root @Arguments)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read Git provenance' }
    return $output
}
function Get-QnCargoVersion([string]$Root) {
    Push-Location -LiteralPath $Root
    try {
        $output = @(& cargo metadata --manifest-path (Join-Path $Root 'Cargo.toml') --locked --offline --no-deps --format-version 1)
        $metadataExit = $LASTEXITCODE
    } finally { Pop-Location }
    if ($metadataExit -ne 0) { throw 'cargo metadata --locked --offline --no-deps failed' }
    $metadata = ($output -join "`n") | ConvertFrom-Json
    $manifest = [IO.Path]::GetFullPath((Join-Path $Root 'Cargo.toml'))
    $packages = @($metadata.packages | Where-Object { [IO.Path]::GetFullPath($_.manifest_path) -eq $manifest })
    if ($packages.Count -ne 1 -or $packages[0].name -cne 'quicknest') { throw 'Cargo metadata did not identify the QuickNest package' }
    $version = [string]$packages[0].version
    if ($version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') { throw 'Release version must be canonical major.minor.patch' }
    return $version
}
function Get-QnReleaseSourceSnapshot([string]$Root) {
    $Root = Assert-QnReleasePath $Root
    $version = Get-QnCargoVersion $Root
    $baseHead = ((Invoke-QnReleaseGit $Root @('rev-parse','--verify','HEAD')) -join '').Trim()
    if ($baseHead -cnotmatch '^([a-f0-9]{40}|[a-f0-9]{64})$') { throw 'Invalid base HEAD' }
    $status = @(Invoke-QnReleaseGit $Root @('-c','core.quotepath=false','status','--porcelain=v1','--untracked-files=all','--no-renames'))
    $tracked = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    foreach ($path in (Invoke-QnReleaseGit $Root @('-c','core.quotepath=false','ls-files','--cached'))) { [void]$tracked.Add($path) }
    $excludedDirectories = @('.git','.zcode','.vs','.idea','node_modules','target','tools','installer/output','docs/test-results')
    $paths = New-Object 'Collections.Generic.List[string]'
    $queue = New-Object 'Collections.Generic.Queue[string]'
    $queue.Enqueue($Root)
    while ($queue.Count) {
        $directory = $queue.Dequeue()
        foreach ($item in (Get-ChildItem -LiteralPath $directory -Force)) {
            $relative = $item.FullName.Substring($Root.Length + 1).Replace('\','/')
            if ($item.PSIsContainer -and ($relative -in $excludedDirectories -or $item.Name -in @('.git','node_modules','target'))) { continue }
            # Only compilation, verification, packaging and distributed-document
            # inputs belong to the byte identity. Ordinary docs/results do not.
            if ($directory -eq $Root) {
                if ($item.PSIsContainer -and $item.Name -notin @('src','ui','assets','tests','examples','scripts','installer','.cargo')) { continue }
                if (-not $item.PSIsContainer -and $item.Name -notin @('Cargo.toml','Cargo.lock','build.rs','rust-toolchain.toml','README.md','RELEASE_NOTES.md','.gitignore','LICENSE','LICENSE.md','LICENSE.txt','NOTICE','NOTICE.md')) { continue }
            }
            [void](Assert-QnReleasePath $item.FullName $Root)
            if ($item.PSIsContainer) { $queue.Enqueue($item.FullName); continue }
            if ($item.Name -in @('Thumbs.db','desktop.ini','.DS_Store') -or $item.Name -like '*.swp' -or $item.Name -like '*.probe') { continue }
            if ($relative -match '[\r\n\t]') { throw 'Source filenames cannot contain line separators' }
            $paths.Add($relative)
        }
    }
    $paths.Sort([StringComparer]::Ordinal)
    $canonical = New-Object Text.StringBuilder
    [void]$canonical.Append("quicknest-source-v1`n")
    $files = @()
    $untracked = @()
    foreach ($relative in $paths) {
        $record = Get-QnReleaseFileRecord (Join-Path $Root $relative) $relative
        $files += $record
        [void]$canonical.Append($relative).Append("`t").Append($record.bytes.ToString([Globalization.CultureInfo]::InvariantCulture)).Append("`t").Append($record.sha256).Append("`n")
        if (-not $tracked.Contains($relative)) { $untracked += $relative }
    }
    foreach ($required in @('Cargo.toml','Cargo.lock','build.rs','src/main.rs','src/lib.rs','src/build_info.rs','ui/main_window.slint','assets/QuickNest.ico','README.md','installer/QuickNest.iss','installer/build-portable.ps1','scripts/build-release.ps1','scripts/release-metadata.ps1','scripts/verify-release-artifacts.ps1')) {
        if (-not $paths.Contains($required)) { throw ('Required release source is missing: ' + $required) }
    }
    return [pscustomobject][ordered]@{
        schema_version = 1; version = $version; base_head = $baseHead
        dirty = [bool]($status.Count -gt 0 -or $untracked.Count -gt 0)
        git_status = $status; untracked_source = $untracked
        fingerprint_algorithm = 'sha256/quicknest-source-v1-path-tab-bytes-tab-sha256-lf'
        excluded_directories = $excludedDirectories
        source_fingerprint = Get-QnReleaseTextHash $canonical.ToString()
        files = $files
    }
}
function Assert-QnReleaseSourceManifest($Source) {
    if ($Source.schema_version -ne 1 -or $Source.source_fingerprint -cnotmatch '^[a-f0-9]{64}$') { throw 'Invalid source manifest' }
    $canonical = New-Object Text.StringBuilder
    [void]$canonical.Append("quicknest-source-v1`n")
    $previous = $null
    $seen = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($record in $Source.files) {
        if ($record.path -match '(^/|\\|[\r\n\t]|(^|/)\.\.?(/|$))' -or [string]::IsNullOrWhiteSpace($record.path) -or $record.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $record.bytes -lt 0 -or -not $seen.Add($record.path)) { throw 'Invalid or duplicate source file record' }
        if ($null -ne $previous -and [StringComparer]::Ordinal.Compare($previous, $record.path) -ge 0) { throw 'Source input list must be sorted with ordinal comparison' }
        [void]$canonical.Append($record.path).Append("`t").Append(([long]$record.bytes).ToString([Globalization.CultureInfo]::InvariantCulture)).Append("`t").Append($record.sha256).Append("`n")
        $previous = $record.path
    }
    if ((Get-QnReleaseTextHash $canonical.ToString()) -cne $Source.source_fingerprint) { throw 'Source file list does not match its byte fingerprint' }
}
function Assert-QnReleaseSourceUnchanged([string]$Root, $Frozen) {
    Assert-QnReleaseSourceManifest $Frozen
    $current = Get-QnReleaseSourceSnapshot $Root
    foreach ($name in @('version','base_head','source_fingerprint')) {
        if ($current.$name -cne $Frozen.$name) { throw ('Frozen source changed: ' + $name) }
    }
    # dirty/git_status describe the real worktree at freeze time. Later audit
    # documents or generated evidence cannot alter the compiled byte identity.
    return $current
}
function Get-QnReleaseKeyInfo([string]$Root, [string]$PublicKeyHex) {
    if ([string]::IsNullOrWhiteSpace($PublicKeyHex)) { return [pscustomobject]@{ status = 'disabled'; blob_sha256 = $null; normalized_hex = '' } }
    $hex = $PublicKeyHex.Trim().ToLowerInvariant()
    if ($hex -cnotmatch '^[0-9a-f]{144}$') { throw 'Only a 72-byte BCrypt ECDSA P-256 PUBLIC blob is accepted' }
    if ($hex -ceq [IO.File]::ReadAllText((Join-Path $Root 'src/update_key.pub')).Trim().ToLowerInvariant()) { throw 'The disclosed fixture public key is refused' }
    $bytes = New-Object byte[] 72
    for ($i = 0; $i -lt $bytes.Length; $i++) { $bytes[$i] = [Convert]::ToByte($hex.Substring($i * 2, 2), 16) }
    if ([BitConverter]::ToString($bytes,0,8) -cne '45-43-53-31-20-00-00-00') { throw 'Only a BCrypt ECDSA P-256 PUBLIC blob is accepted' }
    $key = [Security.Cryptography.CngKey]::Import($bytes, [Security.Cryptography.CngKeyBlobFormat]::EccPublicBlob)
    try { if ($key.KeySize -ne 256 -or $key.AlgorithmGroup -ne [Security.Cryptography.CngAlgorithmGroup]::ECDsa) { throw 'Release key is not ECDSA P-256' } } finally { $key.Dispose() }
    return [pscustomobject]@{ status = 'configured'; blob_sha256 = Get-QnReleaseBytesHash $bytes; normalized_hex = $hex }
}
function Get-QnReleaseToolVersion([string]$Tool) {
    $text = @(& $Tool -Vv)
    if ($LASTEXITCODE -ne 0) { throw ('Cannot query tool version: ' + $Tool) }
    return ($text -join "`n").Replace("`r`n", "`n").Trim()
}
function Get-QnReleaseBuildFingerprint($Info) {
    $dirtyText = ([bool]$Info.dirty).ToString().ToLowerInvariant()
    $recipe = "quicknest-build-v1`nversion`t$($Info.version)`nsource_fingerprint`t$($Info.source_fingerprint)`nbase_head`t$($Info.base_head)`ndirty`t$dirtyText`ntarget`t$($Info.target)`nprofile`t$($Info.profile)`nrustc`t$(Get-QnReleaseTextHash $Info.rustc)`ncargo`t$(Get-QnReleaseTextHash $Info.cargo)`nupdate_key_status`t$($Info.update_key.status)`nupdate_key_blob_sha256`t$($Info.update_key.blob_sha256)`n"
    return Get-QnReleaseTextHash $recipe
}
function Assert-QnReleaseBuildInfo($Expected, $Actual) {
    if ($null -eq $Actual -or $Actual.provenance -cne 'recorded' -or $Actual.schema_version -ne 1) { throw 'Executable has unrecorded or unsupported provenance' }
    if ($Actual.version -cnotmatch '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' -or $Actual.source_fingerprint -cnotmatch '^[a-f0-9]{64}$' -or $Actual.build_fingerprint -cnotmatch '^[a-f0-9]{64}$' -or $Actual.base_head -cnotmatch '^([a-f0-9]{40}|[a-f0-9]{64})$' -or $Actual.dirty -isnot [bool] -or $Actual.target -cne 'x86_64-pc-windows-msvc' -or $Actual.profile -cne 'release') { throw 'Executable has invalid release provenance fields' }
    if ($Actual.update_key.status -cnotin @('disabled','configured') -or ($Actual.update_key.status -ceq 'disabled' -and $null -ne $Actual.update_key.blob_sha256) -or ($Actual.update_key.status -ceq 'configured' -and $Actual.update_key.blob_sha256 -cnotmatch '^[a-f0-9]{64}$')) { throw 'Invalid compiled update key status/fingerprint' }
    foreach ($name in @('version','source_fingerprint','base_head','dirty','target','profile','rustc','cargo','build_fingerprint')) {
        if ($Actual.$name -cne $Expected.$name) { throw ('Executable build information mismatch: ' + $name) }
    }
    foreach ($name in @('status','blob_sha256')) {
        if ($Actual.update_key.$name -cne $Expected.update_key.$name) { throw ('Executable update key mismatch: ' + $name) }
    }
    if ((Get-QnReleaseBuildFingerprint $Actual) -cne $Actual.build_fingerprint) { throw 'Executable build fingerprint does not match its recipe' }
}
function Invoke-QnReleaseProbe([string]$Exe, [string]$Arguments, [int]$TimeoutMilliseconds = 10000) {
    [void](Assert-QnReleasePath $Exe)
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $Exe; $start.Arguments = $Arguments
    $start.UseShellExecute = $false; $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $start.EnvironmentVariables['QUICKNEST_DATA_DIR'] = 'relative-root-must-not-be-used'
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $start
    $startedHere = $false
    try {
        $startedHere = $process.Start()
        if (-not $startedHere) { throw 'Release probe did not start' }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($TimeoutMilliseconds)) { throw 'Release early-entry probe timed out' }
        if (-not $stdoutTask.Wait(5000) -or -not $stderrTask.Wait(5000)) { throw 'Release probe output did not close' }
        if ($process.ExitCode -ne 0) { throw ('Release early-entry probe failed: ' + $stderrTask.Result) }
        return $stdoutTask.Result.Trim()
    } finally {
        # Only the Process object created and started in this function is eligible.
        if ($startedHere -and -not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(5000) }
        $process.Dispose()
    }
}
function ConvertTo-QnReleaseArgument([string]$Argument) {
    if ($Argument.Length -gt 0 -and $Argument -notmatch '[\s"]') { return $Argument }
    $escaped = [regex]::Replace($Argument, '(\\*)"', '$1$1\"')
    $escaped = [regex]::Replace($escaped, '(\\+)$', '$1$1')
    return '"' + $escaped + '"'
}
function Invoke-QnReleaseTool([string]$Tool, [string[]]$Arguments, [string]$WorkingDirectory, [string]$LogPath) {
    [void](Assert-QnReleasePath $WorkingDirectory)
    [void](Assert-QnReleasePath $LogPath)
    if (Test-Path -LiteralPath $LogPath) { throw 'Release tool log already exists' }
    $start = New-Object Diagnostics.ProcessStartInfo
    $start.FileName = $Tool
    $start.Arguments = (@($Arguments | ForEach-Object { ConvertTo-QnReleaseArgument $_ }) -join ' ')
    $start.WorkingDirectory = $WorkingDirectory
    $start.UseShellExecute = $false; $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $start
    $startedHere = $false
    try {
        $startedHere = $process.Start()
        if (-not $startedHere) { throw 'Release tool did not start' }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(1800000)) { throw 'Release build/compiler timed out' }
        if (-not $stdoutTask.Wait(5000) -or -not $stderrTask.Wait(5000)) { throw 'Release tool output did not close' }
        $text = "[stdout]`n" + $stdoutTask.Result + "`n[stderr]`n" + $stderrTask.Result
        $bytes = (New-Object Text.UTF8Encoding($false)).GetBytes($text)
        $stream = [IO.File]::Open($LogPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        try { $stream.Write($bytes,0,$bytes.Length) } finally { $stream.Dispose() }
        return $process.ExitCode
    } finally {
        if ($startedHere -and -not $process.HasExited) { $process.Kill(); [void]$process.WaitForExit(5000) }
        $process.Dispose()
    }
}
function Get-QnExecutableBuildInfo([string]$Exe) {
    return (Invoke-QnReleaseProbe $Exe '--quicknest-build-info') | ConvertFrom-Json
}
function Assert-QnReleaseFileRecord([string]$Root, $Record) {
    if ($Record.path -cnotmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$' -or $Record.sha256 -cnotmatch '^[a-f0-9]{64}$' -or $Record.bytes -lt 0) { throw 'Invalid release file record' }
    $path = Assert-QnReleasePath (Join-Path $Root $Record.path) $Root
    $actual = Get-QnReleaseFileRecord $path $Record.path
    if ($actual.bytes -ne $Record.bytes -or $actual.sha256 -cne $Record.sha256) { throw ('Release input hash/length mismatch: ' + $Record.path) }
    return $path
}
function Assert-QnReleaseStage([string]$ReleaseDirectory, $Stage) {
    $stageDirectory = Assert-QnReleasePath (Join-Path $ReleaseDirectory 'staging') $ReleaseDirectory
    if ($Stage.schema_version -ne 1) { throw 'Unsupported staging manifest' }
    $expected = @('QuickNest.ico','QuickNest.portable','README.md','quicknest.exe')
    $entries = @(Get-ChildItem -LiteralPath $stageDirectory -Force)
    if ($entries.Count -ne 4 -or @($Stage.files).Count -ne 4) { throw 'Staging has unexpected file count' }
    $seen = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($record in $Stage.files) {
        if ($record.path -cnotin $expected -or -not $seen.Add($record.path)) { throw 'Staging contains duplicate or extra entries' }
        $path = Assert-QnReleaseFileRecord $stageDirectory $record
        if (((Get-Item -LiteralPath $path -Force).Attributes -band [IO.FileAttributes]::ReadOnly) -eq 0) { throw ('Staging input is not read-only: ' + $record.path) }
        if ($record.path -ceq 'QuickNest.portable' -and $record.bytes -ne 0) { throw 'Portable marker is not empty' }
    }
    foreach ($entry in $entries) { if ($entry.PSIsContainer -or -not $seen.Contains($entry.Name)) { throw 'Staging contains unexpected entries' } }
    return $stageDirectory
}
function Open-QnReleaseStageLocks([string]$ReleaseDirectory, $Stage) {
    $directory = Assert-QnReleaseStage $ReleaseDirectory $Stage
    $streams = New-Object 'Collections.Generic.List[IO.FileStream]'
    try {
        foreach ($record in $Stage.files) { $streams.Add([IO.File]::Open((Join-Path $directory $record.path), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)) }
        return ,$streams
    } catch { foreach ($stream in $streams) { $stream.Dispose() }; throw }
}
function Assert-QnPortableArchive([string]$ZipPath, $Stage) {
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead((Assert-QnReleasePath $ZipPath))
    try {
        if ($archive.Entries.Count -ne 4 -or @($Stage.files).Count -ne 4) { throw 'Unexpected portable archive entry count' }
        $records = @{}
        foreach ($record in $Stage.files) {
            if ($record.path -cnotin @('QuickNest.ico','QuickNest.portable','README.md','quicknest.exe') -or $records.ContainsKey($record.path)) { throw 'Invalid expected portable entries' }
            $records[$record.path] = $record
        }
        $seen = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $archive.Entries) {
            $name = $entry.FullName
            if (-not $records.ContainsKey($name) -or $name -cne $records[$name].path -or -not $seen.Add($name)) { throw ('Duplicate, extra or noncanonical portable entry: ' + $name) }
            $record = $records[$name]
            if ($entry.Length -ne $record.bytes) { throw ('Portable entry length mismatch: ' + $name) }
            if ($name -ceq 'QuickNest.portable' -and $entry.Length -ne 0) { throw 'Portable marker is not empty' }
            $stream = $entry.Open(); $sha = [Security.Cryptography.SHA256]::Create()
            try { $hash = ([BitConverter]::ToString($sha.ComputeHash($stream))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose(); $stream.Dispose() }
            if ($hash -cne $record.sha256) { throw ('Portable entry hash mismatch: ' + $name) }
        }
    } finally { $archive.Dispose() }
}
function Assert-QnReleaseDistributedFiles($Source, $Stage) {
    foreach ($distributedFile in @(@{ source='assets/QuickNest.ico'; destination='QuickNest.ico' }, @{ source='README.md'; destination='README.md' })) {
        $original = @($Source.files | Where-Object { $_.path -ceq $distributedFile.source })
        $staged = @($Stage.files | Where-Object { $_.path -ceq $distributedFile.destination })
        if ($original.Count -ne 1 -or $staged.Count -ne 1 -or $original[0].sha256 -cne $staged[0].sha256 -or $original[0].bytes -ne $staged[0].bytes) { throw 'Staging document/icon differs from frozen source' }
    }
}
function Read-QnReleaseContext([string]$Root, [string]$ReleaseDirectory, [switch]$ProbeExecutable) {
    $run = Assert-QnReleasePath $ReleaseDirectory
    if ([IO.File]::ReadAllText((Join-Path $run '.quicknest-release-run'), [Text.Encoding]::UTF8) -cne 'QuickNest local release v1') { throw 'Not an allocated release directory' }
    $source = Read-QnReleaseJson (Join-Path $run 'source-manifest.json')
    $build = Read-QnReleaseJson (Join-Path $run 'build-manifest.json')
    $stage = Read-QnReleaseJson (Join-Path $run 'staging-manifest.json')
    [void](Assert-QnReleaseSourceUnchanged $Root $source)
    if ($build.schema_version -ne 1 -or $stage.schema_version -ne 1) { throw 'Unsupported release metadata' }
    [void](Assert-QnReleaseFileRecord $run $build.source_manifest)
    if ($build.source_manifest.path -cne 'source-manifest.json') { throw 'Incorrect source manifest reference' }
    [void](Assert-QnReleaseFileRecord $run $stage.build_manifest)
    if ($stage.build_manifest.path -cne 'build-manifest.json') { throw 'Incorrect build manifest reference' }
    $info = $build.build_info
    if ($info.version -cne $source.version -or $info.source_fingerprint -cne $source.source_fingerprint -or $info.base_head -cne $source.base_head -or $info.dirty -cne $source.dirty) { throw 'Build/source provenance mismatch' }
    Assert-QnReleaseBuildInfo $info $info
    if ($stage.version -cne $source.version -or $stage.build_fingerprint -cne $info.build_fingerprint) { throw 'Staging/build provenance mismatch' }
    $directory = Assert-QnReleaseStage $run $stage
    $exeRecord = @($stage.files | Where-Object { $_.path -ceq 'quicknest.exe' })[0]
    if ($build.executable.path -cne 'quicknest.exe' -or $exeRecord.sha256 -cne $build.executable.sha256 -or $exeRecord.bytes -ne $build.executable.bytes) { throw 'Staging executable differs from recorded build' }
    Assert-QnReleaseDistributedFiles $source $stage
    if ($ProbeExecutable) { Assert-QnReleaseBuildInfo $info (Get-QnExecutableBuildInfo (Join-Path $directory 'quicknest.exe')) }
    return [pscustomobject]@{ directory = $run; source = $source; build = $build; stage = $stage; staging_directory = $directory }
}
