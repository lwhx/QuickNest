#requires -Version 5.1
# P2-12 acceptance (baseline 16.1/21): light, dark and follow-system themes
# switch without restart, persist to settings.theme, and the accent color
# updates the semantic palette.
throw 'BLOCKED: obsolete flat settings-menu fixture; current UI uses a settings panel and right-click submenus. Core control/callback regression coverage is maintained in src/ui_regression_tests.rs and tests/ui_layout.rs; this script cannot certify desktop behavior until migrated.'
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
Add-Type -AssemblyName System.Drawing

Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class P212Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void Key(ushort vk){
  var down=new Input{type=1,u=new Union{key=new Keyboard{vk=vk}}};
  var up=new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=0x0002}}};
  if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
 public static class P212Window {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,System.Text.StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-12-theme.tsv'
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

function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P212Window+EnumProc]{
            param($h, $l)
            if (-not [P212Window]::IsWindowVisible($h)) { return $true }
            $class = New-Object System.Text.StringBuilder 64
            [void][P212Window]::GetClassName($h, $class, 64)
            if ($class.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P212Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}

function Invoke-ThemeMenu([long]$Hwnd, [int]$ItemIndex) {
    Show-QuickNest $Hwnd
    Invoke-ClickAt ($script:left + 700) ($script:top + 22) 500
    # The native popup is a #32768 window; its screen position can differ from
    # the logical show() point (DPI), so locate it by class and click the row.
    $menuHwnd = Wait-MenuWindow 5000
    Assert-True ($menuHwnd -ne 0) 'theme menu did not open'
    $rect = New-Object P212Window+Rect
    [void][P212Window]::GetWindowRect($menuHwnd, [ref]$rect)
    $height = $rect.Bottom - $rect.Top
    Assert-True ($height -gt 100) "theme menu too small: $height"
    # P2-13 extended the menu to 14 rows; derive the row height from the real
    # item count instead of a hardcoded 7.
    $hmenu = [P212Window]::SendMessage([IntPtr]$menuHwnd, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P212Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -ge 7) "theme menu items: $count"
    $itemHeight = $height / $count
    $clickX = $rect.Left + 50
    $clickY = $rect.Top + [int]($itemHeight * ($ItemIndex - 0.5))
    Invoke-ClickAt $clickX $clickY 800
}

function Get-Pixel([int]$X, [int]$Y) {
    $bitmap = New-Object Drawing.Bitmap(1, 1)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($X, $Y, 0, 0, $bitmap.Size) } finally { $graphics.Dispose() }
        $pixel = $bitmap.GetPixel(0, 0)
        return [pscustomobject]@{ R = [int]$pixel.R; G = [int]$pixel.G; B = [int]$pixel.B }
    } finally {
        $bitmap.Dispose()
    }
}

function Test-Light([object]$Pixel) {
    return ($Pixel.R -gt 190 -and $Pixel.G -gt 190 -and $Pixel.B -gt 190)
}

function Test-Dark([object]$Pixel) {
    return ($Pixel.R -lt 90 -and $Pixel.G -lt 90 -and $Pixel.B -lt 90)
}

function Sample-Background([long]$Hwnd) {
    [void][P005Desktop]::SetCursorPos($script:left + 650, $script:top + 300)
    Start-Sleep -Milliseconds 350
    return Get-Pixel ($script:left + 650) ($script:top + 300)
}

function Read-ConfigTheme([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try {
                $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
                return $config
            } catch {
                # A concurrent atomic replace can briefly expose a missing file.
            }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$app = $null
$script:left = 0
$script:top = 0

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

try {
    $library = @{
        schema_version = 2
        current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb' }
        groups = @(
            @{ id='g1'; name='ThemeGroup'; icon=''; color=''; last_category_id='c1'; categories=@(
                @{ id='c1'; name='ThemeCat'; icon=''; items=@(
                    @{ id='i1'; name='Notepad'; item_type='exe'; icon=''; window_state='normal'; note='demo'
                       launch=@{ path='C:\Windows\System32\notepad.exe'; args=@(); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } }
                ) }) }
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
        $pixel = Sample-Background $hwnd
        Assert-True (Test-Light $pixel) "light startup background: $($pixel.R),$($pixel.G),$($pixel.B)"
        Save-P005Screenshot -Path (Join-Path $results 'p2-12-light.png') -WindowHwnd $hwnd
        Record 'light_start' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")

        # a) Switch to dark through the title-bar settings menu (2nd item).
        Invoke-ThemeMenu $hwnd 2
        Save-P005Screenshot -Path (Join-Path $results 'p2-12-dark.png') -WindowHwnd $hwnd
        $pixel = Sample-Background $hwnd
        $config = Read-ConfigTheme $configPath
        Assert-True (Test-Dark $pixel) "dark background: $($pixel.R),$($pixel.G),$($pixel.B)"
        Assert-True ($config.settings.theme -eq 'dark') "persisted theme: [$($config.settings.theme)]"
        Record 'dark_switch' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B) theme=$($config.settings.theme)")

        # b) Back to light (1st item).
        Invoke-ThemeMenu $hwnd 1
        $pixel = Sample-Background $hwnd
        $config = Read-ConfigTheme $configPath
        Assert-True (Test-Light $pixel) "light after switch: $($pixel.R),$($pixel.G),$($pixel.B)"
        Assert-True ($config.settings.theme -eq 'light') "persisted theme: [$($config.settings.theme)]"
        Record 'light_switch' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B) theme=$($config.settings.theme)")

        # c) Follow system (3rd item) must match AppsUseLightTheme.
        $systemLight = [int](Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name AppsUseLightTheme -ErrorAction Stop).AppsUseLightTheme
        Invoke-ThemeMenu $hwnd 3
        Save-P005Screenshot -Path (Join-Path $results 'p2-12-system.png') -WindowHwnd $hwnd
        $pixel = Sample-Background $hwnd
        $config = Read-ConfigTheme $configPath
        Assert-True ($config.settings.theme -eq 'system') "persisted theme: [$($config.settings.theme)]"
        if ($systemLight -eq 1) {
            Assert-True (Test-Light $pixel) "system=light background: $($pixel.R),$($pixel.G),$($pixel.B)"
        } else {
            Assert-True (Test-Dark $pixel) "system=dark background: $($pixel.R),$($pixel.G),$($pixel.B)"
        }
        Record 'system_follow' ("AppsUseLightTheme=$systemLight rgb=$($pixel.R),$($pixel.G),$($pixel.B) theme=$($config.settings.theme)")

        # d) Accent green (5th item): the selected category row uses the
        #    accent-soft tint, so its green channel must exceed blue.
        Invoke-ThemeMenu $hwnd 5
        $config = Read-ConfigTheme $configPath
        Assert-True ($config.settings.accent_color -eq '#16a34a') "persisted accent: [$($config.settings.accent_color)]"
        Save-P005Screenshot -Path (Join-Path $results 'p2-12-accent-green.png') -WindowHwnd $hwnd
        $accentPixel = Get-Pixel ($script:left + 130) ($script:top + 108)
        Assert-True ($accentPixel.G -gt $accentPixel.B) "accent tint not green: $($accentPixel.R),$($accentPixel.G),$($accentPixel.B)"
        Record 'accent_green' ("accent=$($config.settings.accent_color) category_rgb=$($accentPixel.R),$($accentPixel.G),$($accentPixel.B)")

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
        Record 'app_exit' ("exit=$($app.ExitCode)")

        # e) Persistence: rewrite theme=dark and restart; the first paint is dark.
        $config = Read-ConfigTheme $configPath
        $config.settings.theme = 'dark'
        [IO.File]::WriteAllText($configPath, ($config | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
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
            $pixel = Sample-Background $hwnd2
            Assert-True (Test-Dark $pixel) "dark persisted background: $($pixel.R),$($pixel.G),$($pixel.B)"
            Save-P005Screenshot -Path (Join-Path $results 'p2-12-dark-restart.png') -WindowHwnd $hwnd2
            Record 'dark_restart' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")
            Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd2)
            if (-not $app2.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app2); throw 'QuickNest did not exit on WM_CLOSE' }
            $app2.WaitForExit()
            Assert-True ($app2.ExitCode -eq 0) 'QuickNest exit code after restart is not zero'
            Record 'app_exit_after_restart' ("exit=$($app2.ExitCode)")
        } finally {
            if ($null -ne $app2 -and -not $app2.HasExited) { try { (Stop-QnOwnedProcess -Process $app2) } catch { } }
        }

        Record 'completed' 'all P2-12 theme phases passed'
        Write-Host 'All P2-12 theme phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if ($null -ne $app -and -not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
