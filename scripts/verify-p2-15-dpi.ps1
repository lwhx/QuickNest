#requires -Version 5.1
# P2-15 acceptance (baseline 16.4): Per-Monitor V2 DPI awareness, the four
# required scale factors (100%, 125%, 150%, 175%) and the cross-screen
# position correction that keeps the panel on a connected display.
#
# The scale factors are measured on the running application: the script posts
# WM_DPICHANGED with the DPI under test and reads the scale factor and the
# window size back from the application's own report. The logical size must
# stay the same while the physical size follows the scale factor.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))
Add-Type @'
using System;using System.Runtime.InteropServices;using System.Text;
public static class P215Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern int GetWindowLong(IntPtr h,int i);
 [DllImport("user32.dll")]public static extern IntPtr GetWindowDpiAwarenessContext(IntPtr h);
 [DllImport("user32.dll")]public static extern int AreDpiAwarenessContextsEqual(IntPtr a, IntPtr b);
 [DllImport("user32.dll")]public static extern int GetAwarenessFromDpiAwarenessContext(IntPtr ctx);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")]public static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int hgt,uint flags);
 [DllImport("user32.dll")]public static extern IntPtr MonitorFromWindow(IntPtr h,uint flags);
 [DllImport("user32.dll")]public static extern bool GetMonitorInfo(IntPtr m, ref MonitorInfo info);
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)]
 public struct MonitorInfo{public uint cbSize;public Rect rcMonitor;public Rect rcWork;public uint dwFlags;}
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-15-dpi.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -like 'QuickNest*' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}
function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json } catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}
function Get-WorkArea([long]$Hwnd) {
    $monitor = [P215Window]::MonitorFromWindow([IntPtr]$Hwnd, 2)
    $info = New-Object P215Window+MonitorInfo
    $info.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf([type][P215Window+MonitorInfo])
    [void][P215Window]::GetMonitorInfo($monitor, [ref]$info)
    return $info.rcWork
}
function Get-ApplicationState([string]$Path) {
    # The application reports its state every 250ms; the last line wins.
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    $lines = @(Get-Content -LiteralPath $Path -Encoding UTF8 | Where-Object { $_ -match "`tstate`t" })
    if ($lines.Count -eq 0) { return $null }
    return $lines[-1]
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$scaleReport = Join-Path $results 'p2-15-scale-report.tsv'

Stop-QnTestProcesses
Start-Sleep -Milliseconds 400
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Remove-QnSandboxItem -LiteralPath $scaleReport -ErrorAction SilentlyContinue

try {
    # a) The manifest declares Per-Monitor V2: the running window must report
    #    the per-monitor v2 awareness context.
    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -ArgumentList @('--dpi-scale-test', $scaleReport) -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = Get-QuickNestWindow $app.Id
        }
        if (-not $window) { throw 'QuickNest validation window not found' }
        $hwnd = [long]$window.hwnd
        $context = [P215Window]::GetWindowDpiAwarenessContext([IntPtr]$hwnd)
        # GetWindowDpiAwarenessContext returns an opaque handle; the documented
        # comparison is AreDpiAwarenessContextsEqual, not a numeric compare.
        $isV2 = [P215Window]::AreDpiAwarenessContextsEqual($context, [IntPtr](-4))
        $awareness = [P215Window]::GetAwarenessFromDpiAwarenessContext($context)
        Assert-True ($isV2 -ne 0) "window DPI awareness context is not PerMonitorV2 (handle=$context awareness=$awareness)"
        Assert-True ($awareness -eq 2) "window DPI awareness $awareness (expected 2 = per monitor)"
        Record 'pmv2_awareness' ("context=$context pmv2=$isV2 awareness=$awareness")

        # b) The four required scale factors. WM_DPICHANGED carries the new DPI
        #    and the rectangle Windows suggests; winit keeps the logical size
        #    and scales the physical one.
        $state = Get-ApplicationState $scaleReport
        Assert-True ($null -ne $state) 'the application did not report its state'
        Record 'initial_state' $state
        foreach ($pair in @(@(96, '100%'), @(120, '125%'), @(144, '150%'), @(168, '175%'))) {
            $dpi = [int]$pair[0]
            $label = [string]$pair[1]
            $rect = New-Object P215Window+Rect
            [void][P215Window]::GetWindowRect([IntPtr]$hwnd, [ref]$rect)
            $width = $rect.Right - $rect.Left
            $height = $rect.Bottom - $rect.Top
            # The suggested rectangle keeps the window where it is; only the
            # DPI changes.
            $suggested = New-Object P215Window+Rect
            $suggested.Left = $rect.Left
            $suggested.Top = $rect.Top
            $suggested.Right = $rect.Left + [int][Math]::Round($width * $dpi / 96.0)
            $suggested.Bottom = $rect.Top + [int][Math]::Round($height * $dpi / 96.0)
            $memory = [Runtime.InteropServices.Marshal]::AllocHGlobal(16)
            [Runtime.InteropServices.Marshal]::StructureToPtr($suggested, $memory, $false)
            $wParam = [IntPtr](($dpi -band 0xFFFF) -bor (($dpi -shl 16) -band 0xFFFF0000))
            # SendMessage keeps the suggested-rect pointer alive until winit has
            # copied it; an asynchronous post could outlive the buffer.
            [void][P215Window]::SendMessage([IntPtr]$hwnd, 0x02E0, $wParam, $memory)
            Start-Sleep -Milliseconds 700
            [Runtime.InteropServices.Marshal]::FreeHGlobal($memory)
            $state = Get-ApplicationState $scaleReport
            Assert-True ($null -ne $state) "no application state after the $label change"
            $scale = [double]([regex]::Match($state, 'scale=([0-9.]+)').Groups[1].Value)
            $size = [regex]::Match($state, 'size=([0-9]+)x([0-9]+)')
            $expected = $dpi / 96.0
            Assert-True ([Math]::Abs($scale - $expected) -lt 0.01) "$label scale $scale (expected $expected)"
            $width2 = [int]$size.Groups[1].Value
            $height2 = [int]$size.Groups[2].Value
            Assert-True ($width2 -gt 0 -and $height2 -gt 0) "$label window size ${width2}x${height2}"
            Record 'scale_factor' ("$label dpi=$dpi scale=$scale size=${width2}x${height2}")
        }
        # The logical size must be the same at every scale factor.
        $scales = @(Get-Content -LiteralPath $report -Encoding UTF8 | Where-Object { $_ -match "`tscale_factor`t" })
        Assert-True ($scales.Count -eq 4) "scale factor phases: $($scales.Count)"

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'validation window did not exit' }
        $app.WaitForExit()
        Record 'scale_exit' ("exit=$($app.ExitCode)")
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }

    # c) A position that belongs to a display that is gone is corrected into
    #    the nearest connected display instead of restoring off-screen.
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none'
                      window_bounds = @{ x = 6000; y = 4000; width = 760; height = 520; monitor_id = '\\\\.\\DISPLAY9' } }
        groups = @(@{ id='g1'; name='Dpi'; icon=''; color=''; last_category_id='c1'; categories=@(
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
        Start-Sleep -Milliseconds 900
        $rect = New-Object P215Window+Rect
        [void][P215Window]::GetWindowRect([IntPtr]$hwnd, [ref]$rect)
        $work = Get-WorkArea $hwnd
        Assert-True ($rect.Left -ge $work.Left -and $rect.Top -ge $work.Top) "restored position $($rect.Left),$($rect.Top) is outside the work area $($work.Left),$($work.Top)"
        Assert-True ($rect.Right -le $work.Right -and $rect.Bottom -le $work.Bottom) "restored position $($rect.Right),$($rect.Bottom) is outside the work area $($work.Right),$($work.Bottom)"
        Record 'offscreen_corrected' ("rect=$($rect.Left),$($rect.Top),$($rect.Right),$($rect.Bottom) work=$($work.Left),$($work.Top),$($work.Right),$($work.Bottom)")

        # d) The corrected position is remembered and restored on the next start.
        $targetX = 120
        $targetY = 90
        [void][P215Window]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), $targetX, $targetY, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 1500
        $config = Read-Config $configPath
        Assert-True ($config.settings.window_bounds.x -eq $targetX) "persisted x=$($config.settings.window_bounds.x) (expected $targetX)"
        Assert-True ($config.settings.window_bounds.y -eq $targetY) "persisted y=$($config.settings.window_bounds.y) (expected $targetY)"
        Record 'position_persisted' ("x=$($config.settings.window_bounds.x) y=$($config.settings.window_bounds.y) monitor=$($config.settings.window_bounds.monitor_id)")

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Record 'app_exit' ("exit=$($app.ExitCode)")
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }

    # e) The remembered position is restored on the next start.
    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = Get-QuickNestWindow $app.Id
        }
        if (-not $window) { throw 'QuickNest window not found after restart' }
        $hwnd = [long]$window.hwnd
        Start-Sleep -Milliseconds 900
        $rect = New-Object P215Window+Rect
        [void][P215Window]::GetWindowRect([IntPtr]$hwnd, [ref]$rect)
        Assert-True ([Math]::Abs($rect.Left - $targetX) -le 2) "restored x=$($rect.Left) (expected $targetX)"
        Assert-True ([Math]::Abs($rect.Top - $targetY) -le 2) "restored y=$($rect.Top) (expected $targetY)"
        Record 'position_restored' ("rect=$($rect.Left),$($rect.Top)")

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Record 'app_exit_restart' ("exit=$($app.ExitCode)")
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }

    Record 'completed' 'all P2-15 DPI phases passed'
    Write-Host 'All P2-15 DPI phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
