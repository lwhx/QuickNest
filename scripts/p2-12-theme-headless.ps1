#requires -Version 5.1
# P2-12 supplementary acceptance for locked workstations: the real native
# settings menu is opened/selected with window messages (no interactive
# input desktop) and the rendered window is sampled with PrintWindow.
# The primary script verify-p2-12-theme.ps1 uses SendInput/CopyFromScreen.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class P212Headless {
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
 [DllImport("user32.dll")] public static extern bool RedrawWindow(IntPtr hwnd, IntPtr rect, IntPtr region, uint flags);
 public struct RECT { public int Left, Top, Right, Bottom; }
}
'@

$results = Join-Path $root 'docs\test-results\P2-12'
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-12-theme-headless.tsv'
if (Test-Path $report) { Remove-Item $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -eq 'QuickNest' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P212Headless+EnumProc]{
            param($h, $l)
            if (-not [P212Headless]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P212Headless]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P212Headless]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}

function Open-MenuAndSelect([long]$Hwnd, [int]$ItemIndex) {
    $helper = Start-Process -FilePath 'powershell.exe' -ArgumentList @(
        '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
        "`"$(Join-Path $root 'scripts\p2-12-menu-click.ps1')`"", $Hwnd, '700', '22'
    ) -PassThru
    try {
        $menu = Wait-MenuWindow 8000
        Assert-True ($menu -ne 0) 'the settings menu did not open'
        for ($i = 0; $i -lt $ItemIndex; $i++) {
            [void][P212Headless]::PostMessage([IntPtr]$menu, 0x0100, [IntPtr]0x28, [IntPtr]::Zero)
            Start-Sleep -Milliseconds 100
        }
        [void][P212Headless]::PostMessage([IntPtr]$menu, 0x0100, [IntPtr]0x0D, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 900
        if (-not $helper.WaitForExit(5000)) { $helper.Kill() }
    } finally {
        if (-not $helper.HasExited) { try { $helper.Kill() } catch { } }
    }
}

function Capture-Window([long]$Hwnd, [string]$Path) {
    # Force a repaint even though the window lives on the inactive desktop.
    [void][P212Headless]::RedrawWindow([IntPtr]$Hwnd, [IntPtr]::Zero, [IntPtr]::Zero, 0x0185)
    Start-Sleep -Milliseconds 250
    $rect = New-Object P212Headless+RECT
    [void][P212Headless]::GetWindowRect([IntPtr]$Hwnd, [ref]$rect)
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    $bitmap = New-Object Drawing.Bitmap($width, $height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $hdc = $graphics.GetHdc()
    try { $printed = [P212Headless]::PrintWindow([IntPtr]$Hwnd, $hdc, 2) } finally { $graphics.ReleaseHdc($hdc); $graphics.Dispose() }
    Assert-True $printed 'PrintWindow failed'
    $bitmap.Save($Path, [Drawing.Imaging.ImageFormat]::Png)
    $bg = $bitmap.GetPixel(650, 300)
    $category = $bitmap.GetPixel(130, 108)
    $bitmap.Dispose()
    return [pscustomobject]@{ R = [int]$bg.R; G = [int]$bg.G; B = [int]$bg.B; CatR = [int]$category.R; CatG = [int]$category.G; CatB = [int]$category.B }
}

function Test-Light([object]$Pixel) { return ($Pixel.R -gt 190 -and $Pixel.G -gt 190 -and $Pixel.B -gt 190) }
function Test-Dark([object]$Pixel) { return ($Pixel.R -lt 90 -and $Pixel.G -lt 90 -and $Pixel.B -lt 90) }

function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json } catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}

$configPath = Join-Path $env:APPDATA 'QuickNest\config.json'
$configDir = Split-Path -Parent $configPath
$exe = Join-Path $root 'target\release\quicknest.exe'
$systemLight = [int](Get-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize' -Name AppsUseLightTheme -ErrorAction Stop).AppsUseLightTheme
Remove-Item -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-Item -Force
Get-Process quicknest -ErrorAction SilentlyContinue | Where-Object { try { $_.Path -eq $exe } catch { $false } } | Stop-Process -Force
Start-Sleep -Milliseconds 400
$app = $null
try {
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb' }
        groups = @(@{ id='g1'; name='Theme'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=@(
                @{ id='i1'; name='Notepad'; item_type='exe'; icon=''; window_state='normal'; note=''
                   launch=@{ path='C:\Windows\System32\notepad.exe'; args=@(); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } }
            ) }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    $app = Start-Process -FilePath $exe -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = Get-QuickNestWindow $app.Id
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        Start-Sleep -Milliseconds 600

        $pixel = Capture-Window $hwnd (Join-Path $results 'p2-12-hl-light.png')
        Assert-True (Test-Light $pixel) "light startup: $($pixel.R),$($pixel.G),$($pixel.B)"
        Record 'light_start' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")

        Open-MenuAndSelect $hwnd 2
        $pixel = Capture-Window $hwnd (Join-Path $results 'p2-12-hl-dark.png')
        $config = Read-Config $configPath
        Assert-True (Test-Dark $pixel) "dark menu switch: $($pixel.R),$($pixel.G),$($pixel.B)"
        Assert-True ($config.settings.theme -eq 'dark') "theme=$($config.settings.theme)"
        Record 'dark_switch' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B) theme=$($config.settings.theme)")

        Open-MenuAndSelect $hwnd 1
        $pixel = Capture-Window $hwnd (Join-Path $results 'p2-12-hl-light2.png')
        $config = Read-Config $configPath
        Assert-True (Test-Light $pixel) "light menu switch: $($pixel.R),$($pixel.G),$($pixel.B)"
        Assert-True ($config.settings.theme -eq 'light') "theme=$($config.settings.theme)"
        Record 'light_switch' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B) theme=$($config.settings.theme)")

        Open-MenuAndSelect $hwnd 3
        $pixel = Capture-Window $hwnd (Join-Path $results 'p2-12-hl-system.png')
        $config = Read-Config $configPath
        Assert-True ($config.settings.theme -eq 'system') "theme=$($config.settings.theme)"
        if ($systemLight -eq 1) { Assert-True (Test-Light $pixel) "system light: $($pixel.R),$($pixel.G),$($pixel.B)" }
        else { Assert-True (Test-Dark $pixel) "system dark: $($pixel.R),$($pixel.G),$($pixel.B)" }
        Record 'system_follow' ("AppsUseLightTheme=$systemLight rgb=$($pixel.R),$($pixel.G),$($pixel.B) theme=$($config.settings.theme)")

        Open-MenuAndSelect $hwnd 5
        $pixel = Capture-Window $hwnd (Join-Path $results 'p2-12-hl-accent-green.png')
        $config = Read-Config $configPath
        Assert-True ($config.settings.accent_color -eq '#16a34a') "accent=$($config.settings.accent_color)"
        Assert-True ($pixel.CatG -gt $pixel.CatB) "accent tint not green: $($pixel.CatR),$($pixel.CatG),$($pixel.CatB)"
        Record 'accent_green' ("accent=$($config.settings.accent_color) category=$($pixel.CatR),$($pixel.CatG),$($pixel.CatB)")

        # Persist dark, restart, and verify the first painted frame.
        Open-MenuAndSelect $hwnd 2
        $config = Read-Config $configPath
        Assert-True ($config.settings.theme -eq 'dark') "theme before restart=$($config.settings.theme)"
        [void][P005Desktop]::PostMessage([IntPtr]$hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
        if (-not $app.WaitForExit(15000)) { $app.Kill(); throw 'no exit on WM_CLOSE' }
        Assert-True ($app.ExitCode -eq 0) "exit=$($app.ExitCode)"
        Record 'app_exit' ("exit=$($app.ExitCode)")

        $app2 = Start-Process -FilePath $exe -WorkingDirectory $root -PassThru
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(20)
            $window2 = $null
            while ($null -eq $window2 -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 200
                $window2 = Get-QuickNestWindow $app2.Id
            }
            if (-not $window2) { throw 'window after restart not found' }
            $hwnd2 = [long]$window2.hwnd
            Start-Sleep -Milliseconds 600
            $pixel = Capture-Window $hwnd2 (Join-Path $results 'p2-12-hl-dark-restart.png')
            Assert-True (Test-Dark $pixel) "dark restart: $($pixel.R),$($pixel.G),$($pixel.B)"
            Record 'dark_restart' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")
            [void][P005Desktop]::PostMessage([IntPtr]$hwnd2, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)
            if (-not $app2.WaitForExit(15000)) { $app2.Kill(); throw 'no exit after restart' }
            Record 'app_exit_restart' ("exit=$($app2.ExitCode)")
        } finally {
            if ($null -ne $app2 -and -not $app2.HasExited) { try { $app2.Kill() } catch { } }
        }

        Record 'completed' 'all P2-12 headless menu/palette phases passed'
        Write-Host 'All P2-12 headless phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if ($null -ne $app -and -not $app.HasExited) { try { $app.Kill() } catch { } }
    }
} finally {
    Remove-Item -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-Item -Force
}
