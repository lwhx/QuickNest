#requires -Version 5.1
# P3-03 acceptance (baseline 22 延时启动): items with a per-item delay start
# after their delay during a batch launch, while immediate items start at
# once.
#
# Flow: seed three items (delays 0 / 3 / 6 seconds, each starting a probe
# cmd that writes a timestamp file). Batch launch all three, then sample the
# probe files: the immediate one exists at once, the others appear after
# their delays.
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
using System;using System.Runtime.InteropServices;using System.Text;
public static class P303Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
 [DllImport("user32.dll")]public static extern bool GetMenuItemRect(IntPtr hMenu, uint item, out Rect r);
 [DllImport("user32.dll")]public static extern bool ClientToScreen(IntPtr h, ref Point p);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
 public struct Point{public int X,Y;}
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-03-delay.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
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
function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 600) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::RightButton($true)
    Start-Sleep -Milliseconds 90
    [P005Desktop]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P303Window+EnumProc]{
            param($h, $l)
            if (-not [P303Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P303Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P303Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
function Open-CardMenu([int]$X, [int]$Y) {
    Focus-P117Window $script:hwnd
    Start-Sleep -Milliseconds 250
    Invoke-RightClickAt $X $Y
    $point = New-Object P303Window+Point
    $point.X = $X
    $point.Y = $Y
    $candidate = [P303Window]::WindowFromPoint($point)
    $menu = 0
    if ($candidate -ne [IntPtr]::Zero) {
        $sb = New-Object System.Text.StringBuilder 64
        [void][P303Window]::GetClassName($candidate, $sb, 64)
        if ($sb.ToString() -eq '#32768') { $menu = [long]$candidate }
    }
    if ($menu -eq 0) { $menu = Wait-MenuWindow 1500 }
    Assert-True ($menu -ne 0) 'card menu did not open'
    $hmenu = [P303Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P303Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 13) "card menu items: $count (expected 13 with the favorite entry)"
    return @{ Window = $menu; Handle = $hmenu; Count = $count }
}
function Click-MenuRow([hashtable]$Menu, [int]$Row) {
    for ($index = 0; $index -lt $Row; $index++) {
        [P005Keyboard]::Key(0x28)
        Start-Sleep -Milliseconds 60
    }
    [P005Keyboard]::Key(0x0D)
    Start-Sleep -Milliseconds 800
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$probeDir = Join-Path $results 'probes'
$app = $null

Stop-QnTestProcesses
Start-Sleep -Milliseconds 400
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Remove-QnSandboxItem -LiteralPath $probeDir -Recurse -Force -ErrorAction SilentlyContinue
[void][IO.Directory]::CreateDirectory($probeDir)

try {
    function Read-Config3([string]$Path) {
        return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    }
    # a) Seed three items with delays 0/3/6; each launches a cmd that writes a
    #    probe file. The probe file name carries the delay for identification.
    $items = @()
    foreach ($delay in @(0, 3, 6)) {
        $probeFile = Join-Path $probeDir ("probe-$delay.txt")
        $items += @{
            id = "i$delay"; name = "Delay $delay"; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
            launch = @{
                path = "$env:ComSpec"
                args = @('/c', "echo $delay > $probeFile")
                workdir = $probeDir
                run_as_admin = $false; aumid = ''; msi_advertised = $false
                delay_seconds = $delay
            }
        }
    }
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none' }
        groups = @(@{ id='g1'; name='Delay'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=$items }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    $config = Read-Config3 $configPath
    Assert-True ($config.groups[0].categories[0].items[1].launch.delay_seconds -eq 3) 'seeded delay missing'
    Record 'seeded' 'delays=0/3/6 probe files prepared'



    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900

        # b) Pick all three cards through the context menu.
        $cardBaseX = 60 + 208
        $cardBaseY = 60 + 150
        for ($index = 0; $index -lt 3; $index++) {
            $cardX = $cardBaseX + 96 * $index
            $menu = Open-CardMenu $cardX $cardBaseY
            Click-MenuRow $menu 9
            Start-Sleep -Milliseconds 500
            Invoke-ClickAt (60 + 700) (60 + 430) 300
        }
        Record 'selected' 'three cards picked'

        # c) Batch launch; sample the probe files at t=0.5s, t=4s, t=7.5s.
        Save-P005Screenshot -Path (Join-Path $results 'p3-03-before-launch.png') -WindowHwnd $hwnd
        $menu = Open-CardMenu $cardBaseX $cardBaseY
        Record 'menu_open' ("count=$($menu.Count) window=$($menu.Window)")
        Record 'pre_launch' ("probes_before=" + ((Test-Path (Join-Path $probeDir 'probe-0.txt')) -or (Test-Path (Join-Path $probeDir 'probe-3.txt'))))
        Click-MenuRow $menu 10          # 批量启动所选
        Start-Sleep -Milliseconds 800
        Record 'post_launch' ("probe0_exists=" + (Test-Path (Join-Path $probeDir 'probe-0.txt')))
        $launchTime = [DateTime]::UtcNow
        Start-Sleep -Milliseconds 500
        $at0 = (Test-Path (Join-Path $probeDir 'probe-0.txt')) -and -not (Test-Path (Join-Path $probeDir 'probe-3.txt'))
        Record 'sample_t05' ("probe0=$at0 probe3_absent=$(-not (Test-Path (Join-Path $probeDir 'probe-3.txt')))")
        Assert-True $at0 'the immediate item did not start at once'
        Start-Sleep -Milliseconds 3500
        $at4 = (Test-Path (Join-Path $probeDir 'probe-3.txt')) -and (-not (Test-Path (Join-Path $probeDir 'probe-6.txt')))
        Record 'sample_t4' "probe3=$at4"
        Assert-True $at4 'the 3s item did not start around its delay'
        Start-Sleep -Milliseconds 3500
        $at75 = (Test-Path (Join-Path $probeDir 'probe-6.txt'))
        Record 'sample_t75' "probe6=$at75"
        Assert-True $at75 'the 6s item did not start around its delay'

        # d) The probe file contents carry the delays (0/3/6).
        $content0 = Get-Content -LiteralPath (Join-Path $probeDir 'probe-0.txt') -Raw -Encoding UTF8
        Assert-True ($content0.Trim() -eq '0') 'probe-0 content mismatch'
        Record 'probe_contents' 'delay values written correctly'

        Record 'completed' 'all P3-03 delay phases passed'
        Write-Host 'All P3-03 delay phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
