# P2-09 MSI test fixture. Builds a minimal per-user MSI with one advertised
# shortcut (Shortcut.Target = feature name, the shape Windows Installer uses
# for install-on-demand), installs it, and cleans it up. Test-only helper:
# the product/package/component codes are supplied by the caller.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function New-P209MsiFixture {
    param(
        [Parameter(Mandatory = $true)][string]$WorkDir,
        [Parameter(Mandatory = $true)][string]$ProductCode,
        [Parameter(Mandatory = $true)][string]$PackageCode,
        [Parameter(Mandatory = $true)][string]$ComponentId,
        [Parameter(Mandatory = $true)][string]$UpgradeCode
    )
    New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null
    $msiPath = Join-Path $WorkDir 'QuickNestMsiTest.msi'
    # Uncompressed source files are resolved relative to the package's
    # directory using the INSTALLDIR source name.
    $sourceDir = Join-Path $WorkDir 'QuickNestMsiTest'
    New-Item -ItemType Directory -Path $sourceDir -Force | Out-Null
    $payloadPath = Join-Path $sourceDir 'payload.exe'
    if (Test-Path -LiteralPath $msiPath) { Remove-Item -LiteralPath $msiPath -Force }

    $source = Join-Path $WorkDir 'payload.rs'
    @'
fn main() {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = std::fs::write(dir.join("payload-ran.txt"), b"msi payload ran");
        }
    }
}
'@ | Set-Content -LiteralPath $source -Encoding ASCII
    & rustc -O $source -o $payloadPath
    if ($LASTEXITCODE -ne 0) { throw 'rustc payload build failed' }
    $payloadSize = (Get-Item -LiteralPath $payloadPath).Length

    $installer = New-Object -ComObject WindowsInstaller.Installer
    $db = $installer.OpenDatabase($msiPath, 3)
    function ExecSql([string]$sql) {
        $view = $db.OpenView($sql)
        [void]$view.Execute()
        [void]$view.Close()
    }

    ExecSql 'CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(0) NOT NULL PRIMARY KEY `Property`)'
    ExecSql 'CREATE TABLE `Directory` (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72), `DefaultDir` CHAR(255) NOT NULL PRIMARY KEY `Directory`)'
    ExecSql 'CREATE TABLE `Component` (`Component` CHAR(72) NOT NULL, `ComponentId` CHAR(38), `Directory_` CHAR(72) NOT NULL, `Attributes` SHORT NOT NULL, `Condition` CHAR(255), `KeyPath` CHAR(72) PRIMARY KEY `Component`)'
    ExecSql 'CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL, `Feature_Parent` CHAR(38), `Title` CHAR(64), `Description` CHAR(255), `Display` SHORT, `Level` SHORT NOT NULL, `Directory_` CHAR(72), `Attributes` SHORT NOT NULL PRIMARY KEY `Feature`)'
    ExecSql 'CREATE TABLE `FeatureComponents` (`Feature_` CHAR(38) NOT NULL, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Feature_`, `Component_`)'
    ExecSql 'CREATE TABLE `File` (`File` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL, `FileName` CHAR(255) NOT NULL, `FileSize` LONG NOT NULL, `Version` CHAR(72), `Language` CHAR(20), `Attributes` SHORT, `Sequence` LONG NOT NULL PRIMARY KEY `File`)'
    ExecSql 'CREATE TABLE `Media` (`DiskId` SHORT NOT NULL, `LastSequence` LONG NOT NULL, `DiskPrompt` CHAR(64), `Cabinet` CHAR(255), `VolumeLabel` CHAR(32), `Source` CHAR(72) PRIMARY KEY `DiskId`)'
    ExecSql 'CREATE TABLE `Shortcut` (`Shortcut` CHAR(72) NOT NULL, `Directory_` CHAR(72) NOT NULL, `Name` CHAR(128) NOT NULL, `Component_` CHAR(72) NOT NULL, `Target` CHAR(72) NOT NULL, `Arguments` CHAR(255), `Description` CHAR(255), `Hotkey` SHORT, `Icon_` CHAR(72), `IconIndex` SHORT, `ShowCmd` SHORT, `WkDir` CHAR(72) PRIMARY KEY `Shortcut`)'
    ExecSql 'CREATE TABLE `InstallExecuteSequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)'

    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductCode', '$ProductCode')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductName', 'QuickNest MSI Test')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductVersion', '1.0.0')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('Manufacturer', 'QuickNest')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ProductLanguage', '1033')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('UpgradeCode', '$UpgradeCode')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('ALLUSERS', '2')"
    ExecSql "INSERT INTO ``Property`` (``Property``, ``Value``) VALUES ('MSIINSTALLPERUSER', '1')"

    ExecSql "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('TARGETDIR', '', 'SourceDir')"
    ExecSql "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('LocalAppDataFolder', 'TARGETDIR', '.')"
    ExecSql "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('INSTALLDIR', 'LocalAppDataFolder', 'QuickNestMsiTest')"
    ExecSql "INSERT INTO ``Directory`` (``Directory``, ``Directory_Parent``, ``DefaultDir``) VALUES ('ProgramMenuFolder', 'TARGETDIR', '.')"

    ExecSql "INSERT INTO ``Component`` (``Component``, ``ComponentId``, ``Directory_``, ``Attributes``, ``Condition``, ``KeyPath``) VALUES ('MainComponent', '$ComponentId', 'INSTALLDIR', 0, '', 'payload.exe')"
    ExecSql "INSERT INTO ``Feature`` (``Feature``, ``Feature_Parent``, ``Title``, ``Description``, ``Display``, ``Level``, ``Directory_``, ``Attributes``) VALUES ('MainFeature', '', 'QuickNest MSI Test', 'Advertised shortcut fixture', 1, 1, 'INSTALLDIR', 0)"
    ExecSql "INSERT INTO ``FeatureComponents`` (``Feature_``, ``Component_``) VALUES ('MainFeature', 'MainComponent')"
    ExecSql "INSERT INTO ``File`` (``File``, ``Component_``, ``FileName``, ``FileSize``, ``Version``, ``Language``, ``Attributes``, ``Sequence``) VALUES ('payload.exe', 'MainComponent', 'payload.exe', $payloadSize, '', '', 0, 1)"
    ExecSql "INSERT INTO ``Media`` (``DiskId``, ``LastSequence``, ``DiskPrompt``, ``Cabinet``, ``VolumeLabel``, ``Source``) VALUES (1, 1, '', '', '', '')"
    ExecSql "INSERT INTO ``Shortcut`` (``Shortcut``, ``Directory_``, ``Name``, ``Component_``, ``Target``, ``Arguments``, ``Description``, ``Hotkey``, ``Icon_``, ``IconIndex``, ``ShowCmd``, ``WkDir``) VALUES ('MainShortcut', 'ProgramMenuFolder', 'QuickNest MSI Test.lnk', 'MainComponent', 'MainFeature', '', 'QuickNest MSI advertised test', 0, '', 0, 1, '')"

    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('CostInitialize', '', 800)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('FileCost', '', 900)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('CostFinalize', '', 1000)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('InstallValidate', '', 1400)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('InstallInitialize', '', 1500)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('ProcessComponents', '', 1600)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('UnpublishFeatures', 'REMOVE~=`"ALL`"', 1800)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('RemoveShortcuts', 'REMOVE~=`"ALL`"', 3200)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('RemoveFiles', 'REMOVE~=`"ALL`"', 3500)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('InstallFiles', '', 4000)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('CreateShortcuts', '', 4500)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('RegisterUser', '', 6000)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('RegisterProduct', '', 6100)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('PublishFeatures', '', 6300)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('PublishProduct', '', 6400)"
    ExecSql "INSERT INTO ``InstallExecuteSequence`` (``Action``, ``Condition``, ``Sequence``) VALUES ('InstallFinalize', '', 6600)"

    [void]$db.Commit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($db)
    $db = $null
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()

    # The Windows Installer automation Property(Pid) setter is broken on this
    # host (Msi API Error: Property,Pid), so the mandatory summary stream is
    # written by the SDK's MsiInfo tool. PID 15 (Words) carries the source
    # type/elevation prompt: 0 = uncompressed source, and Security = 2.
    $msiInfo = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter 'MsiInfo.exe' -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
    if (-not $msiInfo) { throw 'MsiInfo.exe not found under Windows Kits' }
    & $msiInfo $msiPath /nologo /C 1252 /T 'QuickNest MSI Test' /J 'QuickNest P2-09 fixture' /A 'QuickNest' /P 'Intel;1033' /V $PackageCode /G 400 /W 0 /U 2 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "MsiInfo failed with $LASTEXITCODE" }

    return [pscustomobject]@{
        MsiPath      = $msiPath
        PayloadPath  = $payloadPath
        ProductCode  = $ProductCode
        PackageCode  = $PackageCode
        InstallDir   = Join-Path $env:LOCALAPPDATA 'QuickNestMsiTest'
        ShortcutName = 'QuickNest MSI Test.lnk'
    }
}

function Install-P209Msi {
    param([Parameter(Mandatory = $true)][string]$MsiPath, [Parameter(Mandatory = $true)][string]$LogPath)
    Remove-Item -LiteralPath $LogPath -ErrorAction SilentlyContinue
    $process = Start-Process msiexec.exe -ArgumentList @('/i', "`"$MsiPath`"", '/qn', '/l*v', "`"$LogPath`"") -Wait -PassThru
    return $process.ExitCode
}

function Uninstall-P209Msi {
    param([Parameter(Mandatory = $true)][string]$ProductCode, [Parameter(Mandatory = $true)][string]$LogPath)
    Remove-Item -LiteralPath $LogPath -ErrorAction SilentlyContinue
    $process = Start-Process msiexec.exe -ArgumentList @('/x', "`"$ProductCode`"", '/qn', '/l*v', "`"$LogPath`"") -Wait -PassThru
    return $process.ExitCode
}

function Find-P209Shortcut {
    param([Parameter(Mandatory = $true)][string]$Name)
    $roots = @(
        (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'),
        (Join-Path $env:ProgramData 'Microsoft\Windows\Start Menu\Programs')
    )
    foreach ($root in $roots) {
        $found = Get-ChildItem -LiteralPath $root -Recurse -Filter $Name -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) { return $found.FullName }
    }
    return $null
}

function Test-P209AdvertisedFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 0x18) { return $false }
    $flags = [BitConverter]::ToUInt32($bytes, 0x14)
    if (($flags -band 0x1000) -eq 0) { return $false }
    for ($i = 0; $i -lt ($bytes.Length - 8); $i++) {
        if ($bytes[$i] -eq 0x06 -and $bytes[$i + 1] -eq 0x00 -and $bytes[$i + 2] -eq 0x00 -and $bytes[$i + 3] -eq 0xA0) {
            return $true
        }
    }
    return $false
}
