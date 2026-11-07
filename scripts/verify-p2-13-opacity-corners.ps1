#requires -Version 5.1
# P2-13 acceptance (baseline 16.2): window opacity (50%-100%, default 95%)
# and rounded corners. A solid red reference form sits behind the panel so
# the composited pixel proves the layered alpha; corner pixels prove the DWM
# rounded/square shape; menu switching, persistence and clamping are covered.
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
Add-Type -AssemblyName System.Windows.Forms

Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class P213Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
public static class P213Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,System.Text.StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-13-opacity-corners.tsv'
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
        $cb = [P213Window+EnumProc]{
            param($h, $l)
            if (-not [P213Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P213Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P213Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}

# Opens the title-bar settings menu and clicks the 1-based menu row.
function Invoke-SettingsMenu([long]$Hwnd, [int]$ItemIndex) {
    Show-QuickNest $Hwnd
    Invoke-ClickAt ($script:left + $script:width - 60) ($script:top + 22) 500
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'settings menu did not open'
    $rect = New-Object P213Window+Rect
    [void][P213Window]::GetWindowRect([IntPtr]$menu, [ref]$rect)
    $hmenu = [P213Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P213Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -ge 14) "settings menu items: $count"
    $height = $rect.Bottom - $rect.Top
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

# The blend assertion only means something when the red reference really is
# behind the panel. That is verified before the first sample, so a half
# painted screen (a visible desktop colour where the reference should be)
# cannot be mistaken for a compositing result.
function Wait-ReferenceReady([int]$ProbeX, [int]$ProbeY) {
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        $probe = Get-Pixel $ProbeX $ProbeY
        if ($probe.R -gt 240 -and $probe.G -lt 20 -and $probe.B -lt 20) { return $true }
        Start-Sleep -Milliseconds 150
    }
    return $false
}

# Takes the median of a few samples: the compositor can be mid flight right
# after a menu change, and a single torn frame must not fail the assertion.
function Get-PixelStable([int]$X, [int]$Y) {
    $samples = @()
    for ($attempt = 0; $attempt -lt 5; $attempt++) {
        $samples += ,(Get-Pixel $X $Y)
        Start-Sleep -Milliseconds 120
    }
    $greens = @($samples | ForEach-Object { $_.G } | Sort-Object)
    $median = $greens[[int]($greens.Count / 2)]
    return $samples | Where-Object { $_.G -eq $median } | Select-Object -First 1
}

function Assert-Blend([object]$Pixel, [int]$AppR, [int]$AppG, [int]$AppB, [double]$Alpha) {
    # Composited = app*alpha + red*(1-alpha); the reference form is pure red.
    $expectedR = [int][Math]::Round($AppR * $Alpha + 255 * (1 - $Alpha))
    $expectedG = [int][Math]::Round($AppG * $Alpha + 0 * (1 - $Alpha))
    $expectedB = [int][Math]::Round($AppB * $Alpha + 0 * (1 - $Alpha))
    Assert-True ([Math]::Abs($Pixel.R - $expectedR) -le 4) "R $($Pixel.R) vs $expectedR"
    Assert-True ([Math]::Abs($Pixel.G - $expectedG) -le 4) "G $($Pixel.G) vs $expectedG"
    Assert-True ([Math]::Abs($Pixel.B - $expectedB) -le 4) "B $($Pixel.B) vs $expectedB"
}

function Assert-CornerRed([object]$Pixel, [string]$Label) {
    Assert-True ($Pixel.R -gt 170 -and $Pixel.G -lt 80 -and $Pixel.B -lt 80) "$Label corner not red: $($pixel.R),$($pixel.G),$($pixel.B)"
}

function Assert-CornerApp([object]$Pixel, [string]$Label) {
    Assert-True ($Pixel.G -gt 200 -and $Pixel.B -gt 200) "$Label corner not app background: $($Pixel.R),$($Pixel.G),$($Pixel.B)"
}

function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json } catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP213-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$script:left = 0
$script:top = 0
$script:width = 760
$form = $null
$app = $null

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

function Write-Config([double]$Opacity, [bool]$Rounded) {
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; opacity = $Opacity; rounded_corners = $Rounded }
        groups = @(@{ id='g1'; name='Opacity'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=@(
                @{ id='i1'; name='Notepad'; item_type='exe'; icon=''; window_state='normal'; note=''
                   launch=@{ path='C:\Windows\System32\notepad.exe'; args=@(); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } }
            ) }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
}

try {
    Write-Config 0.95 $true
    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 150
            $window = Get-QuickNestWindow $app.Id
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        $script:left = $window.rect.Left
        $script:top = $window.rect.Top
        $script:width = $window.rect.Right - $window.rect.Left
        Record 'window_ready' ("pid=$($app.Id) rect=$($script:left),$($script:top) size=$($script:width)")

        # Red reference form behind the panel: composited pixels become
        # app*alpha + red*(1-alpha).
        $form = New-Object System.Windows.Forms.Form
        $form.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
        $form.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
        $form.BackColor = [Drawing.Color]::Red
        $form.Bounds = New-Object Drawing.Rectangle -ArgumentList ($script:left - 60), ($script:top - 60), ($script:width + 120), (($window.rect.Bottom - $window.rect.Top) + 120)
        # A topmost reference form cannot be hidden behind unrelated windows
        # (the panel is raised above it right after).
        $form.TopMost = $true
        $form.Show()
        [System.Windows.Forms.Application]::DoEvents()
        # Raise the panel into the topmost band as well; it is ordered after the
        # form, so the panel stays visible above the red reference.
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0013)
        Show-QuickNest $hwnd
        Start-Sleep -Milliseconds 400
        $probe = Get-Pixel ($script:left - 30) ($script:top + 300)
        Record 'reference_probe' ("window=$($script:left),$($script:top) form=$($form.Left),$($form.Top),$($form.Width)x$($form.Height) probe=$($script:left - 30),$($script:top + 300) rgb=$($probe.R),$($probe.G),$($probe.B)")
        if (-not ($probe.R -gt 240 -and $probe.G -lt 20 -and $probe.B -lt 20)) {
            Save-P005Screenshot -Path (Join-Path $results ("p2-13-reference-missing-" + [DateTime]::UtcNow.ToString("HHmmss") + ".png"))
        }
        Assert-True (Wait-ReferenceReady ($script:left - 30) ($script:top + 300)) 'red reference form is not visible behind the panel'
        Record 'reference_form' ("red form bounds=$($form.Left),$($form.Top),$($form.Width)x$($form.Height)")

        # a) Default 95%.
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 0.95
        Save-P005Screenshot -Path (Join-Path $results 'p2-13-opacity-95.png') -WindowHwnd $hwnd
        Record 'opacity_95' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")

        # b) Menu: opacity 100% (8th row); corner stays red (rounded on).
        Invoke-SettingsMenu $hwnd 8
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 1.0
        $corner = Get-PixelStable ($script:left + 2) ($script:top + 2)
        Assert-CornerRed $corner 'rounded'
        Save-P005Screenshot -Path (Join-Path $results 'p2-13-opacity-100.png') -WindowHwnd $hwnd
        Record 'opacity_100' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B) corner=$($corner.R),$($corner.G),$($corner.B)")

        # c) Menu: corners off (14th row); the corner becomes the app surface.
        Invoke-SettingsMenu $hwnd 14
        $corner = Get-PixelStable ($script:left + 2) ($script:top + 2)
        Assert-CornerApp $corner 'square'
        Save-P005Screenshot -Path (Join-Path $results 'p2-13-corners-off.png') -WindowHwnd $hwnd
        Record 'corners_off' ("corner=$($corner.R),$($corner.G),$($corner.B)")

        # d) Menu: corners on (13th row); the corner turns red again.
        Invoke-SettingsMenu $hwnd 13
        $corner = Get-PixelStable ($script:left + 2) ($script:top + 2)
        Assert-CornerRed $corner 'rounded again'
        Record 'corners_on' ("corner=$($corner.R),$($corner.G),$($corner.B)")

        # e) Menu: opacity 70% (11th row) and 50% (12th row).
        Invoke-SettingsMenu $hwnd 11
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 0.7
        Record 'opacity_70' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")
        Invoke-SettingsMenu $hwnd 12
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 0.5
        Save-P005Screenshot -Path (Join-Path $results 'p2-13-opacity-50.png') -WindowHwnd $hwnd
        Record 'opacity_50' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")

        # f) Persistence: config carries the menu-driven values.
        $config = Read-Config $configPath
        Assert-True ([Math]::Abs($config.settings.opacity - 0.5) -lt 0.001) "opacity=$($config.settings.opacity)"
        Assert-True ($config.settings.rounded_corners -eq $true) 'rounded_corners lost'
        Record 'persisted' ("opacity=$($config.settings.opacity) rounded=$($config.settings.rounded_corners)")

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
        Record 'app_exit' ("exit=$($app.ExitCode)")
    } finally {
        if ($null -ne $form) { $form.Close(); $form.Dispose() }
        if ($null -ne $app -and -not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }

    # g) Restart with persisted 50% + rounded on; then clamp checks.
    Write-Config 0.7 $false
    $app2 = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window2 = $null
        while ($null -eq $window2 -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 150
            $window2 = Get-QuickNestWindow $app2.Id
        }
        if (-not $window2) { throw 'window after restart not found' }
        $hwnd2 = [long]$window2.hwnd
        $script:left = $window2.rect.Left
        $script:top = $window2.rect.Top
        $form2 = New-Object System.Windows.Forms.Form
        $form2.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
        $form2.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
        $form2.BackColor = [Drawing.Color]::Red
        $form2.Bounds = New-Object Drawing.Rectangle -ArgumentList ($script:left - 60), ($script:top - 60), (($window2.rect.Right - $window2.rect.Left) + 120), (($window2.rect.Bottom - $window2.rect.Top) + 120)
        $form2.Show()
        [System.Windows.Forms.Application]::DoEvents()
        Show-QuickNest $hwnd2
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 0.7
        $corner = Get-PixelStable ($script:left + 2) ($script:top + 2)
        # Square corner: the title-bar white surface blends over the red form.
        Assert-Blend $corner 255 255 255 0.7
        Save-P005Screenshot -Path (Join-Path $results 'p2-13-restart-70-square.png') -WindowHwnd $hwnd2
        Record 'restart_70_square' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B) corner=$($corner.R),$($corner.G),$($corner.B)")
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd2)
        if (-not $app2.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app2); throw 'no exit after restart' }
        Record 'app_exit_restart' ("exit=$($app2.ExitCode)")
    } finally {
        if ($null -ne $form2) { $form2.Close(); $form2.Dispose() }
        if ($null -ne $app2 -and -not $app2.HasExited) { try { (Stop-QnOwnedProcess -Process $app2) } catch { } }
    }

    # h) Clamp: opacity 1.5 behaves as 100%; opacity 0.2 behaves as 50%.
    Write-Config 1.5 $true
    $app3 = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window3 = $null
        while ($null -eq $window3 -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 150
            $window3 = Get-QuickNestWindow $app3.Id
        }
        if (-not $window3) { throw 'window (clamp) not found' }
        $hwnd3 = [long]$window3.hwnd
        $script:left = $window3.rect.Left
        $script:top = $window3.rect.Top
        $form3 = New-Object System.Windows.Forms.Form
        $form3.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
        $form3.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
        $form3.BackColor = [Drawing.Color]::Red
        $form3.Bounds = New-Object Drawing.Rectangle -ArgumentList ($script:left - 60), ($script:top - 60), (($window3.rect.Right - $window3.rect.Left) + 120), (($window3.rect.Bottom - $window3.rect.Top) + 120)
        $form3.Show()
        [System.Windows.Forms.Application]::DoEvents()
        Show-QuickNest $hwnd3
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 1.0
        Record 'clamp_high' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd3)
        if (-not $app3.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app3); throw 'no exit (clamp high)' }
    } finally {
        if ($null -ne $form3) { $form3.Close(); $form3.Dispose() }
        if ($null -ne $app3 -and -not $app3.HasExited) { try { (Stop-QnOwnedProcess -Process $app3) } catch { } }
    }
    Write-Config 0.2 $true
    $app4 = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window4 = $null
        while ($null -eq $window4 -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 150
            $window4 = Get-QuickNestWindow $app4.Id
        }
        if (-not $window4) { throw 'window (clamp low) not found' }
        $hwnd4 = [long]$window4.hwnd
        $script:left = $window4.rect.Left
        $script:top = $window4.rect.Top
        $form4 = New-Object System.Windows.Forms.Form
        $form4.FormBorderStyle = [System.Windows.Forms.FormBorderStyle]::None
        $form4.StartPosition = [System.Windows.Forms.FormStartPosition]::Manual
        $form4.BackColor = [Drawing.Color]::Red
        $form4.Bounds = New-Object Drawing.Rectangle -ArgumentList ($script:left - 60), ($script:top - 60), (($window4.rect.Right - $window4.rect.Left) + 120), (($window4.rect.Bottom - $window4.rect.Top) + 120)
        $form4.Show()
        [System.Windows.Forms.Application]::DoEvents()
        Show-QuickNest $hwnd4
        $pixel = Get-PixelStable ($script:left + 650) ($script:top + 300)
        Assert-Blend $pixel 244 246 250 0.5
        Record 'clamp_low' ("rgb=$($pixel.R),$($pixel.G),$($pixel.B)")
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd4)
        if (-not $app4.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app4); throw 'no exit (clamp low)' }
        Record 'completed' 'all P2-13 opacity/corner phases passed'
        Write-Host 'All P2-13 opacity/corner phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if ($null -ne $form4) { $form4.Close(); $form4.Dispose() }
        if ($null -ne $app4 -and -not $app4.HasExited) { try { (Stop-QnOwnedProcess -Process $app4) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
