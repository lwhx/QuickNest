#requires -Version 5.1
# P1-08 acceptance: plain files and folders start through the Shell, and the
# "打开文件所在位置" entry opens Explorer with the item selected.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
. (Join-Path $root 'scripts/test-sandbox.ps1')
$qnSavedEnvironment = Get-QnEnvironmentSnapshot
try {
[void](Initialize-QnTestSandbox -ScriptPath $MyInvocation.MyCommand.Path)
# P4 审核：数据目录隔离——本脚本的写入绝不触碰真实用户配置。
. (Join-Path $root 'scripts\p0-05-desktop.ps1')
. (Join-Path $root ('scripts' + [IO.Path]::DirectorySeparatorChar + 'p1-17-focus.ps1'))
[void][P005Desktop]::SetThreadDpiAwarenessContext([IntPtr](-4))

if (-not ('P108Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P108Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
public static class P108Menu {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")]public static extern bool GetMenuItemRect(IntPtr hMenu, uint item, out Rect r);
 [DllImport("user32.dll")]public static extern bool ClientToScreen(IntPtr h, ref Point p);
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-08-shell-targets.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -like 'QuickNest*' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

function Start-QuickNest {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = (Join-Path $root 'target\release\quicknest.exe')
    $psi.WorkingDirectory = $root
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    $proc = New-Object System.Diagnostics.Process
    $proc.StartInfo = $psi
    if (-not $proc.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $proc
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $win = $null
    while ($null -eq $win -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $win = Get-QuickNestWindow $proc.Id
    }
    if ($null -eq $win) { throw 'QuickNest window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 40, 120, 0, 0, 0x0001)
    [void][P005Desktop]::SetForegroundWindow($win.hwnd)
    Start-Sleep -Milliseconds 450
    $win = Get-QuickNestWindow $proc.Id
    if ($null -eq $win) { throw 'window lost after move' }
    return @{ Process = $proc; Window = $win }
}

function Stop-QuickNest($Run) {
    $proc = $Run.Process
    $win = $Run.Window
    if ($proc.HasExited) { throw "process exited before close (exit=$($proc.ExitCode))" }
    Close-QnOwnedWindow -Hwnd ([IntPtr]$win.hwnd)
    if (-not $proc.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $proc); throw 'no exit on WM_CLOSE' }
    $proc.WaitForExit()
    $err = $proc.StandardError.ReadToEnd()
    if ($err) { Set-Content -LiteralPath (Join-Path $results 'stderr.log') -Value $err -Encoding UTF8 }
    Record 'wm_close_exit' ("exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }
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
    [P108Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P108Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Wait-WindowWithTitle([string]$Pattern, [int]$TimeoutMs = 8000) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $match = @([P005Desktop]::Windows() | Where-Object { $_.title -like $Pattern })
        if ($match.Count -gt 0) { return $match[0] }
        Start-Sleep -Milliseconds 250
    }
    return $null
}

function Close-Window([long]$Hwnd) {
    Close-QnOwnedWindow -Hwnd ([IntPtr]$Hwnd)
    Start-Sleep -Milliseconds 600
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$probeDirectory = Join-Path $env:TEMP 'quicknest-p108'
if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDirectory)
$folderPath = Join-Path $probeDirectory 'FolderProbe'
[void][IO.Directory]::CreateDirectory($folderPath)
$filePath = Join-Path $probeDirectory 'file-probe.txt'
Set-Content -LiteralPath $filePath -Value 'probe' -Encoding UTF8

$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'ShellTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'Tests'; icon = ''
                    items = @(
                        @{ id = 'folder-probe'; name = 'FolderProbe'; item_type = 'folder'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $folderPath; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'file-probe'; name = 'FileProbe'; item_type = 'file'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $filePath; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'file-missing'; name = 'FileMissing'; item_type = 'file'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\QuickNest Demo\missing-file.txt'; args = @(); workdir = ''; run_as_admin = $false } }
                    )
                }
            )
        }
    )
}
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$json = $library | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($configPath, $json, (New-Object Text.UTF8Encoding($false)))

# Test hygiene: stacked notepad windows from earlier runs cover the panel
# and the popup menus, so the run starts from an empty desktop.
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400
$notepadBefore = @(Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
$run = Start-QuickNest
try {
    $win = $run.Window
    $left = $win.rect.Left
    $top = $win.rect.Top
    Record 'window_found' ("pid=$($run.Process.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-08-initial.png') -WindowHwnd $win.hwnd

    $tileY = 150
    # a) Folder: opens in Explorer (window titled FolderProbe).
    Invoke-ClickAt ($left + 208) ($top + $tileY) 1200
    $explorerWindow = Wait-WindowWithTitle '*FolderProbe*'
    if ($null -eq $explorerWindow) { throw 'Explorer window for the folder not found' }
    Record 'folder_launch' ("window=$($explorerWindow.title)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-08-folder-opened.png') -WindowHwnd $win.hwnd
    Close-Window $explorerWindow.hwnd

    # b) File: system association opens the text file; the status bar confirms.
    Invoke-ClickAt ($left + 304) ($top + $tileY) 1500
    Record 'file_launch' 'status bar confirms the file launch'
    Save-P005Screenshot -Path (Join-Path $results 'p1-08-file-launched.png') -WindowHwnd $win.hwnd

    # c) Open containing folder (explorer /select) from the item menu.
    # The launched notepad covers the panel, so it is closed and the panel is
    # brought back to the topmost band before the next right click.
    Stop-QnTestProcesses
    Start-Sleep -Milliseconds 400
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0013)
    Focus-P117Window $win.hwnd
    Start-Sleep -Milliseconds 400
    Invoke-RightClickAt ($left + 304) ($top + $tileY)
    $probePoint = New-Object P108Menu+Point
    $probePoint.X = $left + 314
    $probePoint.Y = $top + $tileY + 10
    $menuHwnd = [P108Menu]::WindowFromPoint($probePoint)
    $menuRect = New-Object P108Menu+Rect
    [void][P108Menu]::GetWindowRect($menuHwnd, [ref]$menuRect)
    Record 'menu_rect' ("$($menuRect.Left),$($menuRect.Top),$($menuRect.Right),$($menuRect.Bottom)")
    # 打开文件所在位置 (4th). Rows 1-4 sit at the same offset from the popup
    # top as before P3-01 added the two batch entries, so the tuned offset
    # still lands on row 4.
    Invoke-ClickAt ($menuRect.Left + 40) ($menuRect.Top + 75)
    Start-Sleep -Milliseconds 900
    Save-P005Screenshot -Path (Join-Path $results 'p1-08-after-location-click.png') -WindowHwnd $win.hwnd
    $parentWindow = Wait-WindowWithTitle '*quicknest-p108*'
    if ($null -eq $parentWindow) { throw 'Explorer window for the containing folder not found' }
    Record 'open_location' ("window=$($parentWindow.title)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-08-open-location.png') -WindowHwnd $win.hwnd
    Close-Window $parentWindow.hwnd

    # d) Missing file: structured error in the status bar.
    Invoke-ClickAt ($left + 400) ($top + $tileY) 800
    Record 'missing_file' 'status bar error expected'
    Save-P005Screenshot -Path (Join-Path $results 'p1-08-missing.png') -WindowHwnd $win.hwnd

    Stop-QuickNest $run
    Record 'completed' 'all P1-08 shell-target phases passed'
    Write-Host 'All P1-08 shell-target phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
    Stop-QnTestProcesses
    if (Test-Path $probeDirectory) { Remove-QnSandboxItem -LiteralPath $probeDirectory -Recurse -Force -ErrorAction SilentlyContinue }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
