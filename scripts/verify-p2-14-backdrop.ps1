#requires -Version 5.1
# P2-14 acceptance (baseline 16.3): the DWM system backdrop (Mica / Acrylic /
# none) is selected from the title-bar settings menu, applied to the window
# (read back via DwmGetWindowAttribute), persisted, and survives restart.
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
using System.Runtime.InteropServices;
using System.Text;
public static class P214ScriptWindow {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,System.Text.StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
 [DllImport("dwmapi.dll")]public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out int value, int size);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-14-backdrop.tsv'
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
        $cb = [P214ScriptWindow+EnumProc]{
            param($h, $l)
            if (-not [P214ScriptWindow]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P214ScriptWindow]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P214ScriptWindow]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}

# Opens the title-bar settings menu (17 rows) and clicks the 1-based row.
function Invoke-SettingsMenu([long]$Hwnd, [int]$ItemIndex) {
    Show-QuickNest $Hwnd
    Invoke-ClickAt ($script:left + $script:width - 60) ($script:top + 22) 500
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'settings menu did not open'
    $rect = New-Object P214ScriptWindow+Rect
    [void][P214ScriptWindow]::GetWindowRect([IntPtr]$menu, [ref]$rect)
    $hmenu = [P214ScriptWindow]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P214ScriptWindow]::GetMenuItemCount($hmenu)
    Assert-True ($count -ge 17) "settings menu items: $count"
    $height = $rect.Bottom - $rect.Top
    $itemHeight = $height / $count
    $clickX = $rect.Left + 50
    $clickY = $rect.Top + [int]($itemHeight * ($ItemIndex - 0.5))
    Invoke-ClickAt $clickX $clickY 800
}

function Read-BackdropValue([long]$Hwnd) {
    $value = [int]0
    $hr = [P214ScriptWindow]::DwmGetWindowAttribute([IntPtr]$Hwnd, 38, [ref]$value, 4)
    Assert-True ($hr -eq 0) "DwmGetWindowAttribute failed: $hr"
    return $value
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

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$script:left = 0
$script:top = 0
$script:width = 760
$app = $null

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

# Menu rows (1-based): 1 浅色, 2 深色, 3 跟随系统, 4-7 强调色, 8-12 透明度,
# 13-14 圆角, 15 背景纯色, 16 背景 Mica, 17 背景 Acrylic.
try {
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none' }
        groups = @(@{ id='g1'; name='Backdrop'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=@() }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = Get-QuickNestWindow $app.Id
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        $script:left = $window.rect.Left
        $script:top = $window.rect.Top
        $script:width = $window.rect.Right - $window.rect.Left
        Show-QuickNest $hwnd
        $noneValue = Read-BackdropValue $hwnd
        Assert-True ($noneValue -eq 1) "backdrop readback with none: $noneValue"
        Record 'start_none' ("dwm_readback=$noneValue")

        # a) Menu: Mica (16th row).
        Invoke-SettingsMenu $hwnd 16
        $config = Read-Config $configPath
        Assert-True ($config.settings.backdrop -eq 'mica') "config backdrop: $($config.settings.backdrop)"
        $applied = Read-BackdropValue $hwnd
        Assert-True ($applied -eq 2) "DWM readback after mica: $applied"
        Save-P005Screenshot -Path (Join-Path $results 'p2-14-backdrop-mica.png') -WindowHwnd $hwnd
        Record 'mica_selected' ("config=mica dwm=$applied")

        # b) Menu: Acrylic (17th row).
        Invoke-SettingsMenu $hwnd 17
        $config = Read-Config $configPath
        Assert-True ($config.settings.backdrop -eq 'acrylic') "config backdrop: $($config.settings.backdrop)"
        $applied = Read-BackdropValue $hwnd
        Assert-True ($applied -eq 3) "DWM readback after acrylic: $applied"
        Save-P005Screenshot -Path (Join-Path $results 'p2-14-backdrop-acrylic.png') -WindowHwnd $hwnd
        Record 'acrylic_selected' ("config=acrylic dwm=$applied")

        # c) Menu: plain color (15th row).
        Invoke-SettingsMenu $hwnd 15
        $config = Read-Config $configPath
        Assert-True ($config.settings.backdrop -eq 'none') "config backdrop: $($config.settings.backdrop)"
        $applied = Read-BackdropValue $hwnd
        Assert-True ($applied -eq 1) "DWM readback after none: $applied"
        Save-P005Screenshot -Path (Join-Path $results 'p2-14-backdrop-none.png') -WindowHwnd $hwnd
        Record 'none_selected' ("config=none dwm=$applied")

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
        Record 'app_exit' ("exit=$($app.ExitCode)")

        # d) Persistence: seed mica, restart, readback must stay applied.
        $config = Read-Config $configPath
        $config.settings.backdrop = 'mica'
        [IO.File]::WriteAllText($configPath, ($config | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
        $app2 = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(20)
            $window2 = $null
            while ($null -eq $window2 -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 200
                $window2 = Get-QuickNestWindow $app2.Id
            }
            if (-not $window2) { throw 'window after restart not found' }
            $hwnd2 = [long]$window2.hwnd
            Show-QuickNest $hwnd2
            $applied = Read-BackdropValue $hwnd2
            Assert-True ($applied -eq 2) "DWM readback after restart: $applied"
            $config = Read-Config $configPath
            Assert-True ($config.settings.backdrop -eq 'mica') "config after restart: $($config.settings.backdrop)"
            Save-P005Screenshot -Path (Join-Path $results 'p2-14-restart-mica.png') -WindowHwnd $hwnd2
            Record 'restart_mica' ("dwm=$applied config=$($config.settings.backdrop)")
            Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd2)
            if (-not $app2.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app2); throw 'no exit after restart' }
            Record 'app_exit_restart' ("exit=$($app2.ExitCode)")
        } finally {
            if (-not $app2.HasExited) { try { (Stop-QnOwnedProcess -Process $app2) } catch { } }
        }

        Record 'completed' 'all P2-14 backdrop phases passed'
        Write-Host 'All P2-14 backdrop phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
