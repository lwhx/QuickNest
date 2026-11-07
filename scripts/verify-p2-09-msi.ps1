#requires -Version 5.1
# P2-09 acceptance (baseline 4.3/4.6): an MSI advertised shortcut is imported
# as the original LNK (the advertised flag is persisted, no EXE is resolved),
# launches through the Shell/Windows Installer, and a broken entry reports
# repair/re-add guidance instead of failing silently.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
. (Join-Path $root 'scripts\p1-17-focus.ps1')
. (Join-Path $root 'scripts\p2-09-msi-fixture.ps1')
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))

Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class P209Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void TypeText(string text){
  foreach(var c in text){
   var down=new Input{type=1,u=new Union{key=new Keyboard{vk=0,scan=c,flags=0x0004}}};
   var up=new Input{type=1,u=new Union{key=new Keyboard{vk=0,scan=c,flags=0x0004|0x0002}}};
   if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
  }
 }
 public static void Key(ushort vk){
  var down=new Input{type=1,u=new Union{key=new Keyboard{vk=vk}}};
  var up=new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=0x0002}}};
  if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
public static class P209Window {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-09-msi.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -eq 'QuickNest' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

function Show-QuickNest([long]$Hwnd) {
    for ($attempt = 0; $attempt -lt 6; $attempt++) {
        if ([P117Focus]::GetForegroundWindow() -eq [IntPtr]$Hwnd) { return }
        [P005Desktop]::Key(0x11, $true)
        [P005Desktop]::Key(0x12, $true)
        [P005Desktop]::Key(0x20, $true)
        [P005Desktop]::Key(0x20, $false)
        [P005Desktop]::Key(0x12, $false)
        [P005Desktop]::Key(0x11, $false)
        Start-Sleep -Milliseconds 300
        Focus-P117Window $Hwnd
    }
    throw 'QuickNest did not take the foreground'
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 500) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P209Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P209Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Get-OpenMenuRect([int]$X, [int]$Y) {
    $point = New-Object P209Window+Point
    $point.X = $X
    $point.Y = $Y
    $menuHwnd = [P209Window]::WindowFromPoint($point)
    $rect2 = New-Object P209Window+Rect
    [void][P209Window]::GetWindowRect($menuHwnd, [ref]$rect2)
    return $rect2
}

function Read-ConfigItems {
    param([string]$Path)
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try {
                $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
                $items = @($config.groups[0].categories[0].items)
                return $items
            } catch {
                # A concurrent atomic replace can briefly expose a missing file.
            }
        }
        Start-Sleep -Milliseconds 100
    }
    return @()
}

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP209-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$installLog = Join-Path $run 'msi-install.log'
$uninstallLog = Join-Path $run 'msi-uninstall.log'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$markerName = 'payload-ran.txt'
$productCode = '{' + [Guid]::NewGuid().ToString().ToUpper() + '}'
$packageCode = '{' + [Guid]::NewGuid().ToString().ToUpper() + '}'
$componentId = '{' + [Guid]::NewGuid().ToString().ToUpper() + '}'
$upgradeCode = '{' + [Guid]::NewGuid().ToString().ToUpper() + '}'
$fixture = $null
$installed = $false
$app = $null

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

try {
    # a) Build and install a real MSI with an advertised shortcut.
    $fixture = New-P209MsiFixture -WorkDir $run -ProductCode $productCode -PackageCode $packageCode -ComponentId $componentId -UpgradeCode $upgradeCode
    Record 'fixture_built' ("msi=$($fixture.MsiPath) payloadBytes=$((Get-Item -LiteralPath $fixture.PayloadPath).Length)")
    $installExit = Install-P209Msi -MsiPath $fixture.MsiPath -LogPath $installLog
    Assert-True ($installExit -eq 0) "MSI install failed with exit code $installExit"
    $installed = $true
    $shortcut = Find-P209Shortcut -Name $fixture.ShortcutName
    Assert-True ($null -ne $shortcut) 'the advertised shortcut was not created'
    Assert-True (Test-P209AdvertisedFile -Path $shortcut) 'the shortcut has no Darwin advertised data'
    Record 'fixture_installed' ("exit=$installExit shortcut=$shortcut advertised=true")

    $library = @{
        schema_version = 2
        current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false }
        groups = @(
            @{
                id = 'g1'; name = 'MsiGroup'; icon = ''; color = ''; last_category_id = 'c1'
                categories = @(
                    @{ id = 'c1'; name = 'MsiCat'; icon = ''; items = @() }
                )
            }
        )
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exePath
    $psi.WorkingDirectory = $root
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    $app = New-Object System.Diagnostics.Process
    $app.StartInfo = $psi
    if (-not $app.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $app
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 150
            $window = Get-QuickNestWindow $app.Id
        }
        if ($null -eq $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        $left = $window.rect.Left
        $top = $window.rect.Top
        Show-QuickNest $hwnd
        Record 'window_ready' ("pid=$($app.Id) rect=$left,$top")

        # b) Import through the add dialog: the save-time detector must keep
        #    the original LNK and persist the advertised flag without
        #    resolving an EXE target.
        Invoke-RightClickAt ($left + 620) ($top + 420) 600
        Invoke-ClickAt ($left + 660) ($top + 429) 600
        Invoke-ClickAt ($left + 370) ($top + 113) 300
        [P005Keyboard]::TypeText('msiitem')
        Invoke-ClickAt ($left + 370) ($top + 192) 300
        [P005Keyboard]::TypeText($shortcut)
        Invoke-ClickAt ($left + 333) ($top + 159) 500
        [P005Keyboard]::Key(0x28)
        [P005Keyboard]::Key(0x0D)
        Start-Sleep -Milliseconds 400
        Save-P005Screenshot -Path (Join-Path $results 'p2-09-import-msi.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($left + 581) ($top + 432) 700

        $items = @(Read-ConfigItems -Path $configPath)
        if ($items.Count -gt 0) {
            Record 'import_debug' ("item0=$($items[0].name)|$($items[0].item_type)|msi=$($items[0].launch.msi_advertised)|path=$($items[0].launch.path)")
        } else {
            Record 'import_debug' 'item0=<none>'
        }
        Assert-True ($items.Count -eq 1) "imported item count: $($items.Count)"
        Assert-True ($items[0].name -eq 'msiitem') "imported name: [$($items[0].name)]"
        Assert-True ($items[0].item_type -eq 'lnk') "imported type: $($items[0].item_type)"
        Assert-True ($items[0].launch.path -eq $shortcut) "imported path: [$($items[0].launch.path)]"
        Assert-True ($items[0].launch.msi_advertised -eq $true) 'the advertised flag was not persisted'
        Assert-True ([string]::IsNullOrEmpty($items[0].launch.aumid)) "MSI item must not get an AUMID: [$($items[0].launch.aumid)]"
        Assert-True (@($items[0].launch.args).Count -eq 0) "MSI item must not resolve arguments: [$($items[0].launch.args)]"
        Assert-True ([string]::IsNullOrEmpty($items[0].launch.workdir)) "MSI item must not resolve a workdir: [$($items[0].launch.workdir)]"
        Record 'import_advertised' ("flag=true path kept=original lnk aumid=[] args=[] workdir=[]")

        # c) Launch through the item menu (same launch_item path as a card
        #    click). The payload only appears if Windows Installer/Shell
        #    resolved the advertised shortcut.
        Show-QuickNest $hwnd
        $marker = Join-Path $fixture.InstallDir $markerName
        Remove-QnSandboxItem -LiteralPath $marker -ErrorAction SilentlyContinue
        Invoke-RightClickAt ($left + 208) ($top + 150) 700
        $menu = Get-OpenMenuRect ($left + 218) ($top + 160)
        Record 'menu_rect' ("$($menu.Left),$($menu.Top),$($menu.Right),$($menu.Bottom)")
        Save-P005Screenshot -Path (Join-Path $results 'p2-09-menu.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($menu.Left + 40) ($menu.Top + 15) 300
        $launchDeadline = [DateTime]::UtcNow.AddSeconds(20)
        while (-not (Test-Path -LiteralPath $marker) -and [DateTime]::UtcNow -lt $launchDeadline) {
            Start-Sleep -Milliseconds 250
        }
        Save-P005Screenshot -Path (Join-Path $results 'p2-09-msi-launched.png') -WindowHwnd $hwnd
        Assert-True (Test-Path -LiteralPath $marker) 'the advertised shortcut did not launch the MSI payload'
        Record 'launch_advertised' ("payload marker=$marker")

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
        Record 'app_exit' ("exit=$($app.ExitCode)")

        # d) Uninstall the product; Windows Installer removes the advertised
        #    shortcut, so the saved item now points at a missing LNK.
        $uninstallExit = Uninstall-P209Msi -ProductCode $productCode -LogPath $uninstallLog
        Assert-True ($uninstallExit -eq 0) "MSI uninstall failed with exit code $uninstallExit"
        $installed = $false
        Assert-True (-not (Test-Path -LiteralPath $shortcut)) 'uninstall did not remove the advertised shortcut'
        Record 'fixture_uninstalled' ("exit=$uninstallExit shortcut_removed=true")

        # e) Broken entry: launching must stay alive and show the repair /
        #    re-add guidance (the exact string is unit-tested; the screenshot
        #    captures the status bar).
        Remove-QnSandboxItem -LiteralPath $marker -ErrorAction SilentlyContinue
        $app2 = New-Object System.Diagnostics.Process
        $app2.StartInfo = $psi
        if (-not $app2.Start()) { throw 'failed to restart QuickNest' }
    Register-QnOwnedProcess $app2
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(20)
            $window2 = $null
            while ($null -eq $window2 -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 150
                $window2 = Get-QuickNestWindow $app2.Id
            }
            if ($null -eq $window2) { throw 'QuickNest window not found after restart' }
            $hwnd2 = [long]$window2.hwnd
            $left2 = $window2.rect.Left
            $top2 = $window2.rect.Top
            Show-QuickNest $hwnd2
            Invoke-RightClickAt ($left2 + 208) ($top2 + 150) 700
            $menu2 = Get-OpenMenuRect ($left2 + 218) ($top2 + 160)
            Invoke-ClickAt ($menu2.Left + 40) ($menu2.Top + 15) 900
            Save-P005Screenshot -Path (Join-Path $results 'p2-09-msi-missing.png') -WindowHwnd $hwnd2
            Assert-True (-not $app2.HasExited) 'QuickNest exited when the MSI shortcut was missing'
            Assert-True (-not (Test-Path -LiteralPath $marker)) 'the missing MSI shortcut still launched a payload'
            $items = @(Read-ConfigItems -Path $configPath)
            Assert-True ($items.Count -eq 1) 'the item disappeared after the failed launch'
            Assert-True ($items[0].launch.msi_advertised -eq $true) 'the advertised flag was lost'
            Assert-True ($items[0].launch.path -eq $shortcut) 'the original LNK path was rewritten'
            Record 'launch_missing' ("alive=true marker=false path_kept=true screenshot=p2-09-msi-missing.png")

            Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd2)
            if (-not $app2.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app2); throw 'QuickNest did not exit on WM_CLOSE' }
            $app2.WaitForExit()
            Assert-True ($app2.ExitCode -eq 0) 'QuickNest exit code after the missing-shortcut phase is not zero'
            Record 'app_exit_after_missing' ("exit=$($app2.ExitCode)")
        } finally {
            if ($null -ne $app2 -and -not $app2.HasExited) { try { (Stop-QnOwnedProcess -Process $app2) } catch { } }
        }

        Record 'completed' 'all P2-09 MSI phases passed'
        Write-Host 'All P2-09 MSI phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if ($null -ne $app -and -not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    if ($installed) {
        [void](Uninstall-P209Msi -ProductCode $productCode -LogPath $uninstallLog)
    }
    $leftover = Find-P209Shortcut -Name 'QuickNest MSI Test.lnk'
    if ($leftover) { Remove-QnSandboxItem -LiteralPath $leftover -Force -ErrorAction SilentlyContinue }
    if ($fixture) {
        Remove-QnSandboxItem -LiteralPath $fixture.InstallDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
