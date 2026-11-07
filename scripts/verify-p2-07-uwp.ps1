#requires -Version 5.1
# P2-07 acceptance (baseline 4.2): importing a UWP shortcut detects it and
# persists the AUMID read from its target AppsFolder item (never guessed);
# launching the item starts the app through the original LNK.
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
public static class P207Input {
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
public static class P207Window {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-07-uwp.tsv'
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
    [P207Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P207Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Get-OpenMenuRect([int]$X, [int]$Y) {
    $point = New-Object P207Window+Point
    $point.X = $X
    $point.Y = $Y
    $menuHwnd = [P207Window]::WindowFromPoint($point)
    $rect2 = New-Object P207Window+Rect
    [void][P207Window]::GetWindowRect($menuHwnd, [ref]$rect2)
    return $rect2
}

function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
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

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP207-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$fixture = Join-Path $run 'uwp-calc.lnk'
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$aumid = 'Microsoft.WindowsCalculator_8wekyb3d8bbwe!App'
$ws = New-Object -ComObject WScript.Shell
$shortcut = $ws.CreateShortcut($fixture)
$shortcut.TargetPath = "shell:AppsFolder\$aumid"
$shortcut.Save()
Assert-True (Test-Path -LiteralPath $fixture) 'UWP fixture shortcut was not created'
$fixtureBytes = [IO.File]::ReadAllBytes($fixture).Length
Record 'fixture' ("lnk=$fixture bytes=$fixtureBytes aumid=$aumid")

$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true; auto_hide_on_launch = $false }
    groups = @(
        @{
            id = 'g1'; name = 'UwpGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{ id = 'c1'; name = 'UwpCat'; icon = ''; items = @() }
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

    # a) Import the UWP shortcut through the add dialog: the target is the
    #    fixture LNK; save-time detection persists the AUMID.
    Invoke-RightClickAt ($left + 620) ($top + 420) 600
    Invoke-ClickAt ($left + 660) ($top + 429) 600
    Invoke-ClickAt ($left + 370) ($top + 113) 300   # 名称
    [P005Keyboard]::TypeText('uwpcalc')
    Invoke-ClickAt ($left + 370) ($top + 192) 300   # 目标
    [P005Keyboard]::TypeText($fixture)
    # 选择类型：Windows 快捷方式（下拉第二项）
    Invoke-ClickAt ($left + 333) ($top + 159) 500   # 类型下拉
    [P005Keyboard]::Key(0x28)                          # Down
    [P005Keyboard]::Key(0x0D)                          # Enter
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p2-07-import-uwp.png') -WindowHwnd $hwnd
    Invoke-ClickAt ($left + 581) ($top + 432) 700   # 保存
    $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $items = @($config.groups[0].categories[0].items)
    if ($items.Count -gt 0) {
        Record 'import_debug' ("item0=$($items[0].name)|$($items[0].item_type)|aumid=[$($items[0].launch.aumid)]")
    } else {
        Record 'import_debug' 'item0=<none>'
    }
    Assert-True ($items.Count -eq 1) "imported item count: $($items.Count)"
    Assert-True ($items[0].name -eq 'uwpcalc') "imported name: [$($items[0].name)]"
    Assert-True ($items[0].item_type -eq 'lnk') "imported type: $($items[0].item_type)"
    Assert-True ($items[0].launch.path -eq $fixture) "imported path: $($items[0].launch.path)"
    Assert-True ($items[0].launch.aumid -eq $aumid) "persisted AUMID: [$($items[0].launch.aumid)]"
    Record 'import_aumid' ("aumid=$($items[0].launch.aumid) path kept=original lnk")

    # b) Launch through the original LNK via the item menu (打开): the same
    #    launch_item path as card clicks, without the card gesture race.
    Show-QuickNest $hwnd
    Invoke-RightClickAt ($left + 208) ($top + 150) 700
    $menu = Get-OpenMenuRect ($left + 218) ($top + 160)
    Record 'menu_rect' ("$($menu.Left),$($menu.Top),$($menu.Right),$($menu.Bottom)")
    Save-P005Screenshot -Path (Join-Path $results 'p2-07-menu.png') -WindowHwnd $hwnd
    Invoke-ClickAt ($menu.Left + 40) ($menu.Top + 15) 300
    $calcDeadline = [DateTime]::UtcNow.AddSeconds(25)
    $calc = $null
    while ($null -eq $calc -and [DateTime]::UtcNow -lt $calcDeadline) {
        Start-Sleep -Milliseconds 250
        $calcWindows = @([P005Desktop]::Windows() | Where-Object { $_.title -like '*计算器*' })
        if ($calcWindows.Count -gt 0) { $calc = $calcWindows[0] }
    }
    Save-P005Screenshot -Path (Join-Path $results 'p2-07-after-launch.png') -WindowHwnd $hwnd
    Assert-True ($null -ne $calc) "launching the UWP item did not open Calculator"
    Record 'launch_original_lnk' ("calculator window=$($calc.hwnd) title=$($calc.title)")
    Save-P005Screenshot -Path (Join-Path $results 'p2-07-calculator.png') -WindowHwnd $hwnd
    Close-QnOwnedWindow -Hwnd ([IntPtr]$calc.hwnd)
    Start-Sleep -Milliseconds 700

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-07 UWP phases passed'
    Write-Host 'All P2-07 UWP phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
