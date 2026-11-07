#requires -Version 5.1
# P2-10 acceptance (baseline 4.4/4.6): special Shell targets - a typed
# `shell:` moniker is recognized, keeps its original string and gets a Shell
# display name; `::{CLSID}`, a Control Panel Shell Link, `ms-settings:` and a
# UNC share all launch through Windows Shell without a local exists() probe.
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
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))

Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class P210Input {
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
public static class P210Window {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-10-shell.tsv'
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
    [P210Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P210Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Get-OpenMenuRect([int]$X, [int]$Y) {
    $point = New-Object P210Window+Point
    $point.X = $X
    $point.Y = $Y
    $menuHwnd = [P210Window]::WindowFromPoint($point)
    $rect2 = New-Object P210Window+Rect
    [void][P210Window]::GetWindowRect($menuHwnd, [ref]$rect2)
    return $rect2
}

function Get-WindowSnapshot {
    return @([P005Desktop]::Windows() | ForEach-Object { [pscustomobject]@{ Hwnd = [long]$_.hwnd; Pid = [int]$_.pid; Title = $_.title } })
}

function Wait-NewWindowForProcess([object[]]$Before, [string[]]$ProcessNames, [int]$TimeoutMs) {
    $previous = @($Before | ForEach-Object { [long]$_.Hwnd })
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $pids = @(Get-Process $ProcessNames -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
        if ($pids.Count -gt 0) {
            foreach ($window in @([P005Desktop]::Windows())) {
                if ($previous -notcontains [long]$window.hwnd -and $pids -contains [int]$window.pid) {
                    return $window
                }
            }
        }
        Start-Sleep -Milliseconds 100
    }
    return $null
}

function Close-TestWindow([long]$Hwnd) {
    try { Close-QnOwnedWindow -Hwnd ([IntPtr]$Hwnd) } catch { }
    Start-Sleep -Milliseconds 500
}

function Close-SettingsWindows {
    $pids = @(Get-Process SystemSettings,ApplicationFrameHost -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    if ($pids.Count -eq 0) { return }
    $windows = @([P005Desktop]::Windows() | Where-Object { $pids -contains [int]$_.pid })
    foreach ($window in $windows) { Close-TestWindow ([long]$window.hwnd) }
    Start-Sleep -Milliseconds 1200
}

function Open-Item([long]$Hwnd, [int]$Index) {
    Show-QuickNest $Hwnd
    $cardX = $script:left + 208 + (96 * $Index)
    $cardY = $script:top + 150
    Invoke-RightClickAt $cardX $cardY 700
    $menu = Get-OpenMenuRect ($cardX + 10) ($cardY + 10)
    Invoke-ClickAt ($menu.Left + 40) ($menu.Top + 15) 600
}

function Read-ConfigItems {
    param([string]$Path)
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try {
                $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
                return @($config.groups[0].categories[0].items)
            } catch {
                # A concurrent atomic replace can briefly expose a missing file.
            }
        }
        Start-Sleep -Milliseconds 100
    }
    return @()
}

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP210-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$panelLnk = Join-Path $run 'control-panel.lnk'
$settingsNeedles = @('SystemSettings', 'ApplicationFrameHost')
$script:left = 0
$script:top = 0
$app = $null

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

try {
    # A real special Shell Link: WScript saves the `shell:ControlPanelFolder`
    # target; the LNK stays the item target (never resolved to an EXE).
    $ws = New-Object -ComObject WScript.Shell
    $shortcut = $ws.CreateShortcut($panelLnk)
    $shortcut.TargetPath = 'shell:ControlPanelFolder'
    $shortcut.Save()
    Assert-True (Test-Path -LiteralPath $panelLnk) 'control panel LNK was not created'
    Record 'fixture' ("panel_lnk=$panelLnk bytes=$((Get-Item -LiteralPath $panelLnk).Length)")

    $library = @{
        schema_version = 2
        current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false }
        groups = @(
            @{
                id = 'g1'; name = 'ShellGroup'; icon = ''; color = ''; last_category_id = 'c1'
                categories = @(
                    @{ id = 'c1'; name = 'ShellCat'; icon = ''; items = @() }
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
        $script:left = $window.rect.Left
        $script:top = $window.rect.Top
        Show-QuickNest $hwnd
        Record 'window_ready' ("pid=$($app.Id) rect=$($script:left),$($script:top)")

        # a) Type a `shell:` target with an empty name. Save-time recognition
        #    must infer `uri`, keep the original string and fill the Shell
        #    display name (never the raw path).
        Invoke-RightClickAt ($script:left + 620) ($script:top + 420) 600
        Invoke-ClickAt ($script:left + 660) ($script:top + 429) 600
        Invoke-ClickAt ($script:left + 370) ($script:top + 192) 300
        [P005Keyboard]::TypeText('shell:MyComputerFolder')
        Save-P005Screenshot -Path (Join-Path $results 'p2-10-import-shell.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($script:left + 581) ($script:top + 432) 800

        $items = @(Read-ConfigItems -Path $configPath)
        Assert-True ($items.Count -eq 1) "imported item count: $($items.Count)"
        Assert-True ($items[0].item_type -eq 'uri') "imported type: $($items[0].item_type)"
        Assert-True ($items[0].launch.path -eq 'shell:MyComputerFolder') "imported path: $($items[0].launch.path)"
        Assert-True (-not [string]::IsNullOrWhiteSpace($items[0].name)) 'the Shell display name was not filled'
        Assert-True ($items[0].name -ne 'shell:MyComputerFolder') 'the raw path was used as the name'
        Assert-True ($items[0].launch.msi_advertised -eq $false) 'the URI item must not be MSI'
        Record 'import_shell' ("name=$($items[0].name) type=$($items[0].item_type) path=$($items[0].launch.path)")

        # b) Launch it through the item menu; a new Explorer window must appear.
        $before = @(Get-WindowSnapshot)
        Open-Item $hwnd 0
        $target = Wait-NewWindowForProcess $before @('explorer') 15000
        Save-P005Screenshot -Path (Join-Path $results 'p2-10-thispc.png') -WindowHwnd $hwnd
        Assert-True ($null -ne $target) 'shell:MyComputerFolder did not open an Explorer window'
        Focus-P117Window ([long]$target.hwnd)
        Start-Sleep -Milliseconds 900
        Save-P005Screenshot -Path (Join-Path $results 'p2-10-thispc-window.png') -WindowHwnd ([long]$target.hwnd)
        Record 'launch_shell' ("window=$($target.hwnd) title=$($target.title)")
        Close-TestWindow ([long]$target.hwnd)

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
        Record 'app_exit_after_import' ("exit=$($app.ExitCode)")

        # c) Append the remaining special targets to the same category.
        $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $extra = @(
            @{ id = 'p210-clsid'; name = 'CLSID'; item_type = 'folder'; icon = ''; window_state = 'normal'; note = ''
               launch = @{ path = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}'; args = @(); workdir = ''; run_as_admin = $false; aumid = ''; msi_advertised = $false } },
            @{ id = 'p210-panel'; name = 'ControlPanel'; item_type = 'lnk'; icon = ''; window_state = 'normal'; note = ''
               launch = @{ path = $panelLnk; args = @(); workdir = ''; run_as_admin = $false; aumid = ''; msi_advertised = $false } },
            @{ id = 'p210-settings'; name = 'Settings'; item_type = 'uri'; icon = ''; window_state = 'normal'; note = ''
               launch = @{ path = 'ms-settings:display'; args = @(); workdir = ''; run_as_admin = $false; aumid = ''; msi_advertised = $false } },
            @{ id = 'p210-share'; name = 'Share'; item_type = 'folder'; icon = ''; window_state = 'normal'; note = ''
               launch = @{ path = '\\localhost\C$'; args = @(); workdir = ''; run_as_admin = $false; aumid = ''; msi_advertised = $false } }
        )
        $config.groups[0].categories[0].items = @($config.groups[0].categories[0].items) + $extra
        [IO.File]::WriteAllText($configPath, ($config | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
        Record 'extras_written' 'clsid/panel/settings/share appended'

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
            $script:left = $window2.rect.Left
            $script:top = $window2.rect.Top
            Show-QuickNest $hwnd2
            Record 'window_ready_2' ("pid=$($app2.Id) rect=$($script:left),$($script:top)")

            # d) ::{CLSID} namespace folder.
            $before = @(Get-WindowSnapshot)
            Open-Item $hwnd2 1
            $target = Wait-NewWindowForProcess $before @('explorer') 15000
            Assert-True ($null -ne $target) '::{CLSID} did not open an Explorer window'
            Focus-P117Window ([long]$target.hwnd)
            Start-Sleep -Milliseconds 900
            Save-P005Screenshot -Path (Join-Path $results 'p2-10-clsid-window.png') -WindowHwnd ([long]$target.hwnd)
            Record 'launch_clsid' ("window=$($target.hwnd) title=$($target.title)")
            Close-TestWindow ([long]$target.hwnd)

            # e) Special Shell Link (Control Panel).
            $before = @(Get-WindowSnapshot)
            Open-Item $hwnd2 2
            $target = Wait-NewWindowForProcess $before @('explorer') 15000
            Assert-True ($null -ne $target) 'the Control Panel LNK did not open an Explorer window'
            Focus-P117Window ([long]$target.hwnd)
            Start-Sleep -Milliseconds 1400
            Save-P005Screenshot -Path (Join-Path $results 'p2-10-control-panel-window.png') -WindowHwnd ([long]$target.hwnd)
            Record 'launch_control_panel' ("window=$($target.hwnd) title=$($target.title)")
            Close-TestWindow ([long]$target.hwnd)

            # f) ms-settings:display. Close any existing Settings window first
            #    so a new window proves the URI launched it.
            Close-SettingsWindows
            $before = @(Get-WindowSnapshot)
            Open-Item $hwnd2 3
            $target = Wait-NewWindowForProcess $before $settingsNeedles 20000
            Assert-True ($null -ne $target) 'ms-settings:display did not open the Settings window'
            Focus-P117Window ([long]$target.hwnd)
            Start-Sleep -Milliseconds 2200
            Save-P005Screenshot -Path (Join-Path $results 'p2-10-settings-window.png') -WindowHwnd ([long]$target.hwnd)
            Record 'launch_settings' ("window=$($target.hwnd) title=$($target.title) pid=$($target.pid)")
            Close-TestWindow ([long]$target.hwnd)

            # g) UNC network location (no local exists() probe).
            $before = @(Get-WindowSnapshot)
            Open-Item $hwnd2 4
            $target = Wait-NewWindowForProcess $before @('explorer') 15000
            Assert-True ($null -ne $target) '\\localhost\C$ did not open an Explorer window'
            Focus-P117Window ([long]$target.hwnd)
            Start-Sleep -Milliseconds 900
            Save-P005Screenshot -Path (Join-Path $results 'p2-10-unc-window.png') -WindowHwnd ([long]$target.hwnd)
            Record 'launch_unc' ("window=$($target.hwnd) title=$($target.title)")
            Close-TestWindow ([long]$target.hwnd)

            # h) The stored targets/types are never rewritten by launching.
            $items = @(Read-ConfigItems -Path $configPath)
            Assert-True ($items.Count -eq 5) "final item count: $($items.Count)"
            Assert-True ($items[0].launch.path -eq 'shell:MyComputerFolder' -and $items[0].item_type -eq 'uri') 'shell: item was rewritten'
            Assert-True ($items[1].launch.path -eq '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}' -and $items[1].item_type -eq 'folder') 'CLSID item was rewritten'
            Assert-True ($items[2].launch.path -eq $panelLnk -and $items[2].item_type -eq 'lnk') 'Control Panel LNK was rewritten'
            Assert-True ($items[3].launch.path -eq 'ms-settings:display' -and $items[3].item_type -eq 'uri') 'settings item was rewritten'
            Assert-True ($items[4].launch.path -eq '\\localhost\C$' -and $items[4].item_type -eq 'folder') 'UNC item was rewritten'
            Record 'targets_kept' 'all five original targets and kinds unchanged'

            Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd2)
            if (-not $app2.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app2); throw 'QuickNest did not exit on WM_CLOSE' }
            $app2.WaitForExit()
            Assert-True ($app2.ExitCode -eq 0) 'QuickNest exit code after the specials is not zero'
            Record 'app_exit' ("exit=$($app2.ExitCode)")
        } finally {
            if ($null -ne $app2 -and -not $app2.HasExited) { try { (Stop-QnOwnedProcess -Process $app2) } catch { } }
        }

        Record 'completed' 'all P2-10 special Shell phases passed'
        Write-Host 'All P2-10 special Shell phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if ($null -ne $app -and -not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
