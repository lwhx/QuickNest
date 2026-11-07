#requires -Version 5.1
# Shared regression safety boundary. Dot-sourcing only defines functions.
function Get-QnEnvironmentSnapshot {
    $values = @{}
    foreach ($name in @('QUICKNEST_TEST_SANDBOX_ROOT','QUICKNEST_TEST_RUN_ROOT','QUICKNEST_TEST_CASE_ROOT','QUICKNEST_DATA_DIR','QUICKNEST_INSTANCE_NAMESPACE','QUICKNEST_TEST_ISOLATED_DESKTOP','QUICKNEST_TEST_DISPOSABLE_ACCOUNT','TEMP','TMP')) {
        $values[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    }
    return $values
}
function Restore-QnEnvironment($Snapshot) {
    foreach ($name in $Snapshot.Keys) { [Environment]::SetEnvironmentVariable($name, $Snapshot[$name], 'Process') }
}
function Assert-QnContainedPath([string]$Path, [string]$Root) {
    if ([string]::IsNullOrWhiteSpace($Path) -or [string]::IsNullOrWhiteSpace($Root)) { throw 'sandbox path/root is empty' }
    if ($Path -match '::|^[^:]+:[^\\/]' -or $Path -match '[*?\[\]]') { throw "non-literal sandbox path refused: $Path" }
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\','/')
    $base = [IO.Path]::GetFullPath($Root).TrimEnd('\','/')
    $protected = @()
    if ($env:APPDATA) { $protected += Join-Path $env:APPDATA 'QuickNest' }
    if ($env:LOCALAPPDATA) { $protected += Join-Path $env:LOCALAPPDATA 'Packages/Microsoft.WindowsNotepad_8wekyb3d8bbwe/LocalState/TabState' }
    foreach ($directory in $protected) {
        $protectedRoot = [IO.Path]::GetFullPath($directory).TrimEnd('\','/')
        if ($full.Equals($protectedRoot, [StringComparison]::OrdinalIgnoreCase) -or $full.StartsWith($protectedRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw "protected user data path refused: $full" }
    }
    if (-not $full.StartsWith($base + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw "path escapes sandbox: $full (root $base)" }
    # Refuse junctions/symlinks in the full ancestry, including the root itself.
    $cursor = $full
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force -ErrorAction Stop
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "reparse point refused: $cursor" }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if ($parent -eq $cursor) { break }
        $cursor = $parent
    }
    return $full
}
function Assert-QnDisposableDesktop {
    if ($env:QUICKNEST_TEST_ISOLATED_DESKTOP -ne '1') { throw 'Desktop acceptance refused. Use an independent disposable Windows account/VM and explicitly enable isolated desktop testing.' }
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent().Name
    if ($env:QUICKNEST_TEST_DISPOSABLE_ACCOUNT -ne $identity) { throw "Disposable account acknowledgement must exactly match current identity: $identity" }
}
function New-QnSandboxRun([string]$Parent) {
    if ([string]::IsNullOrWhiteSpace($Parent)) { $Parent = Join-Path ([IO.Path]::GetTempPath()) 'QuickNest-regression' }
    $run = Join-Path ([IO.Path]::GetFullPath($Parent)) ('run-' + [Guid]::NewGuid().ToString('N'))
    [void](Assert-QnContainedPath $run $Parent)
    [void][IO.Directory]::CreateDirectory($run)
    [IO.File]::WriteAllText((Join-Path $run '.quicknest-test-run'), 'QuickNest regression sandbox')
    return $run
}
function Initialize-QnTestSandbox([string]$ScriptPath, [switch]$FixtureOnly) {
    if (-not $FixtureOnly) {
        Assert-QnDisposableDesktop
        # ShellExecute can reuse a user's existing app and has no reliable process
        # ownership contract. These cases must first be converted to file probes.
        $source = [IO.File]::ReadAllText($ScriptPath)
        if ($source -match '(?i)notepad|explorer\.exe|Start-Process[^\r\n]*mshta') { throw 'Acceptance refused: shared Shell/Notepad/Explorer target requires an owned file probe before desktop execution.' }
    }
    if ([string]::IsNullOrWhiteSpace($env:QUICKNEST_TEST_RUN_ROOT)) {
        $env:QUICKNEST_TEST_RUN_ROOT = New-QnSandboxRun $env:QUICKNEST_TEST_SANDBOX_ROOT
    }
    $script:QnRunRoot = [IO.Path]::GetFullPath($env:QUICKNEST_TEST_RUN_ROOT)
    if (-not (Test-Path -LiteralPath (Join-Path $script:QnRunRoot '.quicknest-test-run'))) { throw 'run root is not an allocated QuickNest sandbox' }
    $caseName = [IO.Path]::GetFileNameWithoutExtension($ScriptPath) + '-' + [Guid]::NewGuid().ToString('N')
    $script:QnCaseRoot = Assert-QnContainedPath (Join-Path $script:QnRunRoot $caseName) $script:QnRunRoot
    [void][IO.Directory]::CreateDirectory($script:QnCaseRoot)
    $env:QUICKNEST_TEST_CASE_ROOT = $script:QnCaseRoot
    if ([string]::IsNullOrWhiteSpace($env:QUICKNEST_DATA_DIR)) { $env:QUICKNEST_DATA_DIR = Join-Path $script:QnCaseRoot 'data' }
    [void](Assert-QnContainedPath $env:QUICKNEST_DATA_DIR $script:QnRunRoot)
    [void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
    if ([string]::IsNullOrWhiteSpace($env:QUICKNEST_INSTANCE_NAMESPACE)) { $env:QUICKNEST_INSTANCE_NAMESPACE = 'regression-' + [Guid]::NewGuid().ToString('N') }
    $env:TEMP = Join-Path $script:QnCaseRoot 'temp'
    $env:TMP = $env:TEMP
    [void][IO.Directory]::CreateDirectory($env:TEMP)
    $script:QnOwnedProcesses = @{}
    return $script:QnCaseRoot
}
function Register-QnOwnedProcess([Diagnostics.Process]$Process) {
    if ($Process.Id -eq $PID) { throw 'cannot register the harness itself' }
    try {
        $Process.Refresh()
        if ($Process.HasExited) { return }
        # A Process instance returned by Start() is registered immediately, before
        # any PID lookup; StartTime protects against subsequent PID recycling.
        $ticks = $Process.StartTime.ToUniversalTime().Ticks
        $script:QnOwnedProcesses[$Process.Id] = @{ Process = $Process; Ticks = $ticks }
    } catch { if (-not $Process.HasExited) { throw } }
}
function Start-QnOwnedProcess {
    [CmdletBinding()]
    param([Parameter(Position=0,Mandatory=$true)][string]$FilePath, [string[]]$ArgumentList, [string]$WorkingDirectory, [switch]$PassThru, [switch]$Wait, [string]$WindowStyle)
    $arguments = @{ FilePath=$FilePath; PassThru=$true; ErrorAction='Stop' }
    foreach ($name in @('ArgumentList','WorkingDirectory','WindowStyle')) { if ($PSBoundParameters.ContainsKey($name)) { $arguments[$name] = $PSBoundParameters[$name] } }
    if ([IO.Path]::GetFileNameWithoutExtension($FilePath) -match '^(notepad|explorer)$') { throw 'shared shell application has no cleanup ownership contract' }
    $process = Microsoft.PowerShell.Management\Start-Process @arguments
    Register-QnOwnedProcess $process
    if ($Wait) { $process.WaitForExit() }
    if ($PassThru) { return $process }
}
function Stop-QnOwnedProcess {
    [CmdletBinding()]
    param([Parameter(ValueFromPipeline=$true)][Diagnostics.Process]$InputObject, [Diagnostics.Process]$Process, [int[]]$Id, [switch]$Force)
    process {
        $targets = @()
        if ($Process) { $targets += $Process }
        if ($InputObject) { $targets += $InputObject }
        foreach ($processId in $Id) { if ($script:QnOwnedProcesses.ContainsKey($processId)) { $targets += $script:QnOwnedProcesses[$processId].Process } }
        foreach ($target in $targets) {
            if (-not $script:QnOwnedProcesses.ContainsKey($target.Id)) { continue }
            $entry = $script:QnOwnedProcesses[$target.Id]
            $owned = $entry.Process
            $owned.Refresh()
            if (-not $owned.HasExited -and $owned.StartTime.ToUniversalTime().Ticks -eq $entry.Ticks) { $owned.Kill(); [void]$owned.WaitForExit(5000) }
        }
    }
}
function Close-QnOwnedWindow([IntPtr]$Hwnd) {
    if (-not ('QnSandboxWindow' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class QnSandboxWindow {
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr w, IntPtr l);
}
'@
    }
    $windowPid = [uint32]0
    [void][QnSandboxWindow]::GetWindowThreadProcessId($Hwnd, [ref]$windowPid)
    if (-not $script:QnOwnedProcesses.ContainsKey([int]$windowPid)) { throw 'refusing to close a window without test process ownership' }
    $entry = $script:QnOwnedProcesses[[int]$windowPid]
    $entry.Process.Refresh()
    if ($entry.Process.HasExited -or $entry.Process.StartTime.ToUniversalTime().Ticks -ne $entry.Ticks) { throw 'window owner is no longer the registered test process' }
    [void][QnSandboxWindow]::PostMessage($Hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
}
function Stop-QnTestProcesses {
    if (Get-Variable QnOwnedProcesses -Scope Script -ErrorAction SilentlyContinue) {
        foreach ($entry in @($script:QnOwnedProcesses.Values)) { Stop-QnOwnedProcess -Process $entry.Process }
    }
}
function Remove-QnSandboxItem {
    [CmdletBinding()]
    param([Parameter(Position=0)][string[]]$Path, [string[]]$LiteralPath, [Parameter(ValueFromPipeline=$true)]$InputObject, [switch]$Recurse, [switch]$Force)
    process {
        $paths = @($Path) + @($LiteralPath)
        if ($null -ne $InputObject) { $paths += $InputObject.FullName }
        foreach ($candidate in $paths) {
            if ($null -eq $candidate) { continue }
            $checked = Assert-QnContainedPath $candidate $script:QnRunRoot
            if (Test-Path -LiteralPath $checked) {
                if ($Recurse) {
                    foreach ($child in Get-ChildItem -LiteralPath $checked -Recurse -Force -ErrorAction Stop) { [void](Assert-QnContainedPath $child.FullName $script:QnRunRoot) }
                }
                Microsoft.PowerShell.Management\Remove-Item -LiteralPath $checked -Recurse:$Recurse -Force:$Force -ErrorAction Stop
            }
        }
    }
}
