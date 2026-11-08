#requires -Version 5.1
# P4-03 acceptance (baseline 29 章 P4-03 安装后降权启动).
#
# Required behavior: an elevated installer's completion-page launch must create
# the application under the original interactive user's Medium token.
# Source flags, installer artifacts and an asInvoker manifest are prerequisite
# evidence only. This script does not drive that completion-page interaction or
# establish its token provenance; acceptance therefore ends BLOCKED/nonzero.
param([string]$ReleaseDirectory = '')
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\p0-05-desktop.ps1')

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p4-03-deelevate.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$issPath = Join-Path $root 'installer\QuickNest.iss'
. (Join-Path $root 'scripts/release-metadata.ps1')
$packageVersion = Get-QnCargoVersion $root
$packageDirectory = if ($ReleaseDirectory) { (Read-QnReleaseContext $root $ReleaseDirectory -ProbeExecutable).directory } else { Join-Path $root 'installer/output' }
$setupPath = Join-Path $packageDirectory ('QuickNest-' + $packageVersion + '-setup.exe')
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\QuickNest'
$appExe = Join-Path $installDir 'quicknest.exe'
$uninst = Join-Path $installDir 'unins000.exe'
$arpKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{8F6A2E52-3B4C-4E9A-9F1D-8C2E51B04A7D}_is1'

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Wait-True([scriptblock]$Condition, [int]$TimeoutSeconds, [string]$Message) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 250
    }
    throw $Message
}

# Existing installations belong to the user; never auto-uninstall them.
if (Test-Path -LiteralPath (Join-Path $env:APPDATA 'Microsoft/Windows/Start Menu/Programs/QuickNest')) { throw 'Existing start menu registration detected.' }
if ((Test-Path -LiteralPath $installDir) -or (Test-Path -LiteralPath $arpKey)) { throw 'Existing QuickNest installation detected; use a fresh disposable account/VM.' }
if (Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath('Desktop')) 'QuickNest.lnk')) { throw 'Existing desktop shortcut detected; refusing overwrite.' }
if (Get-Process quicknest -ErrorAction SilentlyContinue) { throw 'Existing QuickNest process detected; refusing installation.' }

try {
    # a) Source flag and artifact freshness checks do not establish which source
    # was compiled into the artifact, nor completion-page runtime behavior.
    $iss = Get-Content -LiteralPath $issPath -Raw -Encoding UTF8
    Assert-True ($iss -match 'Flags: nowait postinstall skipifsilent runasoriginaluser') '[Run] entry missing the runasoriginaluser flag'
    Assert-True (Test-Path -LiteralPath $setupPath) 'setup exe missing'
    $setupTime = (Get-Item -LiteralPath $setupPath).LastWriteTimeUtc
    $issTime = (Get-Item -LiteralPath $issPath).LastWriteTimeUtc
    Assert-True ($setupTime -ge $issTime) 'setup exe is older than the iss (stale build)'
    $ridConsole = [P005Desktop]::Rid($PID)
    Record 'mechanism' ('source flag present; setup timestamp not older than source; caller integrity rid=' + $ridConsole + '; completion-page behavior untested')

    # b) Silent installation exercises artifacts only. skipifsilent prevents
    # this invocation from exercising the completion-page launch action.
    $p = Start-QnOwnedProcess -FilePath $setupPath -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', '/TASKS=desktopicon' -Wait -PassThru
    Assert-True ($p.ExitCode -eq 0) ('silent install exit code: ' + $p.ExitCode)
    Wait-True { Test-Path -LiteralPath $appExe } 20 'install dir missing quicknest.exe'
    Record 'silent_install_artifacts' ('exit=0; caller integrity rid=' + $ridConsole + '; per-user dir=' + $installDir)

    # c) Inspect per-user registration artifacts.
    Assert-True ($null -eq (Get-ChildItem 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -like '*8F6A2E52*' })) 'matching HKLM uninstall entry found'
    $arp = Get-ItemProperty -LiteralPath $arpKey -ErrorAction SilentlyContinue
    Assert-True ($null -ne $arp) 'ARP entry missing'
    Record 'per_user_install_artifacts' 'HKCU ARP present; no matching HKLM uninstall entry observed'

    # d) asInvoker requests the caller's token; its presence does not demonstrate
    # a Medium token or the installer's runasoriginaluser launch path.
    $bytes = [IO.File]::ReadAllBytes($appExe)
    $text = [Text.Encoding]::ASCII.GetString($bytes)
    Assert-True ($text.Contains('requestedExecutionLevel')) 'manifest not embedded'
    $idx = $text.IndexOf('requestedExecutionLevel')
    $chunk = $text.Substring([Math]::Max(0, $idx - 60), 220)
    Assert-True ($chunk.Contains('asInvoker')) 'manifest level is not asInvoker'
    Record 'manifest_asInvoker' 'asInvoker text observed in installed executable; runtime token provenance untested'

    # e) Remove only the installation created after the existing-install guards.
    Start-QnOwnedProcess -FilePath $uninst -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait
    Wait-True { -not (Test-Path -LiteralPath $appExe) } 20 'files survived uninstall'
    Assert-True ($null -eq (Get-ItemProperty -LiteralPath $arpKey -ErrorAction SilentlyContinue)) 'ARP entry survived uninstall'
    Record 'uninstall_clean' 'test installation artifacts removed'

    $blocked = 'BLOCKED: actual elevated-installer completion-page runasoriginaluser launch was not exercised. Acceptance requires observing that launch under the original interactive user with Medium integrity (RID 8192), distinct from the elevated installer token.'
    Record 'blocked' $blocked
    Get-Content -LiteralPath $report -Encoding UTF8
    throw $blocked
} finally {
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
