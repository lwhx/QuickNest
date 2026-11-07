#requires -Version 5.1
# P2-11 acceptance (baseline 2.2/2.3/3.5): group context menu (rename, accent,
# reorder, delete with migrate/drop confirm), category "new project" acting on
# the right-clicked category, blank-area create entries, and screen-edge menu
# placement.
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
Add-Type -AssemblyName System.Windows.Forms

Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class P211Input {
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
 public static void CtrlA(){
  var ctrlDown=new Input{type=1,u=new Union{key=new Keyboard{vk=0x11}}};
  var aDown=new Input{type=1,u=new Union{key=new Keyboard{vk=0x41}}};
  var aUp=new Input{type=1,u=new Union{key=new Keyboard{vk=0x41,flags=0x0002}}};
  var ctrlUp=new Input{type=1,u=new Union{key=new Keyboard{vk=0x11,flags=0x0002}}};
  if(SendInput(4,new[]{ctrlDown,aDown,aUp,ctrlUp},Marshal.SizeOf(typeof(Input)))!=4)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
public static class P211Window {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,System.Text.StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-11-menus.tsv'
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
    [P211Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P211Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Invoke-Menu([long]$Hwnd, [int]$X, [int]$Y, [int]$ItemIndex) {
    Show-QuickNest $Hwnd
    Invoke-RightClickAt $X $Y 600
    for ($i = 0; $i -lt $ItemIndex; $i++) {
        [P005Keyboard]::Key(0x28) # Down selects the first item, then walks
        Start-Sleep -Milliseconds 40
    }
    [P005Keyboard]::Key(0x0D) # Enter
    Start-Sleep -Milliseconds 700
}

function Get-OpenMenuRect([int]$X, [int]$Y) {
    $point = New-Object P211Window+Point
    $point.X = $X
    $point.Y = $Y
    $menuHwnd = [P211Window]::WindowFromPoint($point)
    $rect2 = New-Object P211Window+Rect
    [void][P211Window]::GetWindowRect($menuHwnd, [ref]$rect2)
    return [pscustomobject]@{ Hwnd = [long]$menuHwnd; Rect = $rect2 }
}

function Get-TopWindows {
    $list = New-Object System.Collections.ArrayList
    $callback = [P211Window+EnumProc]{
        param($h, $l)
        [void]$list.Add([long]$h)
        return $true
    }
    [void][P211Window]::EnumWindows($callback, [IntPtr]::Zero)
    return @($list)
}

function Find-MenuWindow([long[]]$Before, [long]$Exclude) {
    $script:foundMenu = 0
    $callback = [P211Window+EnumProc]{
        param($h, $l)
        if (-not [P211Window]::IsWindowVisible($h)) { return $true }
        if ([long]$h -eq $Exclude) { return $true }
        if ($Before -contains [long]$h) { return $true }
        $rect2 = New-Object P211Window+Rect
        [void][P211Window]::GetWindowRect($h, [ref]$rect2)
        if (($rect2.Right - $rect2.Left) -lt 20 -or ($rect2.Bottom - $rect2.Top) -lt 20) { return $true }
        $script:foundMenu = [long]$h
        return $false
    }
    [void][P211Window]::EnumWindows($callback, [IntPtr]::Zero)
    return [long]$script:foundMenu
}

function Get-WindowRect([long]$Hwnd) {
    $rect2 = New-Object P211Window+Rect
    [void][P211Window]::GetWindowRect([IntPtr]$Hwnd, [ref]$rect2)
    return $rect2
}

function Read-ConfigGroups {
    param([string]$Path)
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try {
                $config = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
                return @($config.groups)
            } catch {
                # A concurrent atomic replace can briefly expose a missing file.
            }
        }
        Start-Sleep -Milliseconds 100
    }
    return @()
}

function Group-Center([int]$Index, [string[]]$Names) {
    $x = 14
    for ($i = 0; $i -lt $Names.Count; $i++) {
        $width = 16
        foreach ($character in $Names[$i].ToCharArray()) {
            if ([int][char]$character -gt 0x2E80) { $width += 15 } else { $width += 8 }
        }
        if ($i -eq $Index) { return $x + [int]($width / 2) }
        $x += $width + 24
    }
    return $x
}

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP211-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
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
        settings = @{ pinned = $true; auto_hide_on_launch = $false }
        groups = @(
            @{ id='g1'; name='Alpha'; icon=''; color=''; last_category_id='c1'; categories=@(
                @{ id='c1'; name='CatA1'; icon=''; items=@() },
                @{ id='c2'; name='CatA2'; icon=''; items=@() }) },
            @{ id='g2'; name='Beta'; icon=''; color=''; last_category_id='c3'; categories=@(
                @{ id='c3'; name='CatB1'; icon=''; items=@() }) },
            @{ id='g3'; name='Gamma'; icon=''; color=''; last_category_id='c4'; categories=@(
                @{ id='c4'; name='CatG1'; icon=''; items=@() }) }
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

        # a) Group rename (2nd menu item).
        $alphaX = $script:left + (Group-Center 0 @('Alpha','Beta','Gamma'))
        $groupY = $script:top + 64
        Invoke-Menu $hwnd $alphaX $groupY 2
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-group-rename-dialog.png') -WindowHwnd $hwnd
        [P005Keyboard]::SelectAll()
        Start-Sleep -Milliseconds 120
        [P005Keyboard]::TypeText('AlphA')
        Start-Sleep -Milliseconds 200
        Invoke-ClickAt ($script:left + 498) ($script:top + 314) 700
        $groups = @(Read-ConfigGroups -Path $configPath)
        Assert-True ($groups.Count -eq 3) "group count after rename: $($groups.Count)"
        Assert-True ($groups[0].name -eq 'AlphA') "renamed group: [$($groups[0].name)]"
        Record 'group_rename' ("name=$($groups[0].name)")

        # b) Accent color (5th menu item = blue).
        Invoke-Menu $hwnd $alphaX $groupY 5
        $groups = @(Read-ConfigGroups -Path $configPath)
        Assert-True ($groups[0].color -eq '#2563eb') "group color: [$($groups[0].color)]"
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-group-accent.png') -WindowHwnd $hwnd
        Record 'group_color' ("color=$($groups[0].color)")

        # c) Category menu "new project" (5th item) targets CatA1, not CatA2.
        # 侧边栏行高 36px + 10px 内边距 → 第一行中心 = top + 112
        $categoryY = $script:top + 112
        Invoke-Menu $hwnd ($script:left + 40) $categoryY 5
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-category-new-project.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($script:left + 370) ($script:top + 113) 250
        [P005Keyboard]::TypeText('CatItem')
        Invoke-ClickAt ($script:left + 370) ($script:top + 192) 250
        [P005Keyboard]::TypeText('C:\Windows\System32\notepad.exe')
        Invoke-ClickAt ($script:left + 581) ($script:top + 432) 800
        $config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
        $catA1 = @($config.groups[0].categories[0].items)
        $catA2 = @($config.groups[0].categories[1].items)
        Assert-True ($catA1.Count -eq 1) "CatA1 item count: $($catA1.Count)"
        Assert-True ($catA1[0].name -eq 'CatItem') "CatA1 item name: [$($catA1[0].name)]"
        Assert-True ($catA2.Count -eq 0) "CatA2 must stay empty: $($catA2.Count)"
        Record 'category_new_project' ("CatA1 items=$($catA1.Count) CatA2 items=$($catA2.Count)")

        # d) Blank-area menu "new group" (3rd item).
        Invoke-Menu $hwnd ($script:left + 620) ($script:top + 420) 3
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-new-group-dialog.png') -WindowHwnd $hwnd
        [P005Keyboard]::SelectAll()
        Start-Sleep -Milliseconds 120
        [P005Keyboard]::TypeText('Delta')
        Start-Sleep -Milliseconds 200
        Invoke-ClickAt ($script:left + 498) ($script:top + 314) 800
        $groups = @(Read-ConfigGroups -Path $configPath)
        Assert-True ($groups.Count -eq 4) "group count after create: $($groups.Count)"
        Assert-True ($groups[3].name -eq 'Delta') "created group: [$($groups[3].name)]"
        Record 'blank_menu_new_group' ("groups=$($groups.Count) last=$($groups[3].name)")

        # e) Delete the non-empty Gamma group by migrating it into AlphA.
        $gammaX = $script:left + (Group-Center 2 @('AlphA','Beta','Gamma','Delta'))
        Invoke-Menu $hwnd $gammaX $groupY 10
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-group-delete-confirm.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($script:left + 432) ($script:top + 290) 800
        $groups = @(Read-ConfigGroups -Path $configPath)
        Assert-True ($groups.Count -eq 3) "group count after migrate: $($groups.Count)"
        $names = @($groups | ForEach-Object { $_.name })
        Assert-True (-not ($names -contains 'Gamma')) "Gamma still present: $($names -join ',')"
        $alphaCategories = @($groups[0].categories | ForEach-Object { $_.name })
        Assert-True ($alphaCategories -contains 'CatG1') "CatG1 was not migrated: $($alphaCategories -join ',')"
        Record 'group_migrate' ("groups=$($names -join ',') AlphA categories=$($alphaCategories -join ',')")

        # f) Delete the non-empty Beta group with "delete all".
        $betaX = $script:left + (Group-Center 1 @('AlphA','Beta','Delta'))
        Invoke-Menu $hwnd $betaX $groupY 10
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-group-delete-all.png') -WindowHwnd $hwnd
        Invoke-ClickAt ($script:left + 518) ($script:top + 290) 800
        $groups = @(Read-ConfigGroups -Path $configPath)
        $names = @($groups | ForEach-Object { $_.name })
        Assert-True ($groups.Count -eq 2) "group count after delete all: $($groups.Count)"
        Assert-True (-not ($names -contains 'Beta')) "Beta still present: $($names -join ',')"
        Record 'group_delete_all' ("groups=$($names -join ',')")

        # g) Move AlphA right (4th menu item).
        Invoke-Menu $hwnd $alphaX $groupY 4
        $groups = @(Read-ConfigGroups -Path $configPath)
        $names = @($groups | ForEach-Object { $_.name })
        Assert-True ($names[0] -eq 'Delta' -and $names[1] -eq 'AlphA') "order after move: $($names -join ',')"
        Record 'group_move_right' ("order=$($names -join ',')")

        # h) Screen-edge placement: put the first card 20 px from the
        #    work-area corner, open the 8-item menu, and require the whole
        #    native menu inside the work area.
        $alphaTabX = $script:left + (Group-Center 1 @('Delta','AlphA'))
        Invoke-ClickAt $alphaTabX $groupY 500
        $work = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr]::Zero, ($work.Right - 228), ($work.Bottom - 170), 760, 520, 0x0004)
        Start-Sleep -Milliseconds 600
        $moved = Get-QuickNestWindow $app.Id
        $script:left = $moved.rect.Left
        $script:top = $moved.rect.Top
        Record 'edge_window' ("rect=$($script:left),$($script:top) work=$($work.Left),$($work.Top),$($work.Right),$($work.Bottom)")
        Show-QuickNest $hwnd
        $cardX = $script:left + 208
        $cardY = $script:top + 150
        $beforeMenus = @(Get-TopWindows)
        Invoke-RightClickAt $cardX $cardY 700
        $menuHwnd = 0
        for ($attempt = 0; $attempt -lt 10 -and $menuHwnd -eq 0; $attempt++) {
            $menuHwnd = Find-MenuWindow $beforeMenus $hwnd
            if ($menuHwnd -eq 0) { Start-Sleep -Milliseconds 100 }
        }
        Assert-True ($menuHwnd -ne 0) 'the native menu window was not found'
        $rect2 = Get-WindowRect $menuHwnd
        # The native popup is not part of P005Desktop's visible-window list
        # (no title/process filter match), so capture the virtual screen.
        Save-P005Screenshot -Path (Join-Path $results 'p2-11-edge-menu.png')
        Assert-True ($rect2.Left -ge $work.Left) "menu left $($rect2.Left) < $($work.Left)"
        Assert-True ($rect2.Top -ge $work.Top) "menu top $($rect2.Top) < $($work.Top)"
        Assert-True ($rect2.Right -le $work.Right) "menu right $($rect2.Right) > $($work.Right)"
        Assert-True ($rect2.Bottom -le $work.Bottom) "menu bottom $($rect2.Bottom) > $($work.Bottom)"
        Assert-True (($rect2.Bottom - $rect2.Top) -ge 160) "menu too small: $($rect2.Bottom - $rect2.Top)"
        Record 'edge_menu' ("rect=$($rect2.Left),$($rect2.Top),$($rect2.Right),$($rect2.Bottom) work=$($work.Left),$($work.Top),$($work.Right),$($work.Bottom)")
        [P005Keyboard]::Key(0x1B)
        Start-Sleep -Milliseconds 300

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
        Record 'app_exit' ("exit=$($app.ExitCode)")
        Record 'completed' 'all P2-11 menu phases passed'
        Write-Host 'All P2-11 menu phases passed.'
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
