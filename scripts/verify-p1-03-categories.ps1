#requires -Version 5.1
# P1-03 acceptance: category create (dialog + typed name), right-click menu,
# rename, delete, reorder, and config persistence.
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

if (-not ('P103Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P103Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
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
 public static void SelectAll(){
  const ushort ctrl=0x11, a=0x41;
  var inputs=new[]{
   new Input{type=1,u=new Union{key=new Keyboard{vk=ctrl}}},
   new Input{type=1,u=new Union{key=new Keyboard{vk=a}}},
   new Input{type=1,u=new Union{key=new Keyboard{vk=a,flags=0x0002}}},
   new Input{type=1,u=new Union{key=new Keyboard{vk=ctrl,flags=0x0002}}}};
  if(SendInput(4,inputs,Marshal.SizeOf(typeof(Input)))!=4)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-03-categories.tsv'
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

function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 80
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P103Input]::RightButton($true)
    Start-Sleep -Milliseconds 80
    [P103Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

$app = Join-Path $root 'target\release\quicknest.exe'
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $app
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardError = $true
$psi.RedirectStandardOutput = $true
$proc = New-Object System.Diagnostics.Process
$proc.StartInfo = $psi
if (-not $proc.Start()) { throw 'failed to start QuickNest' }
    Register-QnOwnedProcess $proc
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $win = $null
    while ($null -eq $win -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $win = Get-QuickNestWindow $proc.Id
    }
    if ($null -eq $win) { throw 'QuickNest window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), 40, 120, 0, 0, 0x0001)
    Start-Sleep -Milliseconds 400
    $win = Get-QuickNestWindow $proc.Id
    if ($null -eq $win) { throw 'window lost after move' }
    $top = $win.rect.Top
    $left = $win.rect.Left
    $bottom = $win.rect.Bottom
    Record 'window_found' ("pid=$($proc.Id) rect=$left,$top,$($win.rect.Right),$bottom")

    Save-P005Screenshot -Path (Join-Path $results 'p1-03-initial.png') -WindowHwnd $win.hwnd
    Record 'initial_screenshot' 'p1-03-initial.png'

    # a) Create a category through the dialog: click "+ 新建分类", type the
    # name, confirm.
    # new-category row sits above the 32px status bar
    $newRowY = $bottom - 32 - 24
    Invoke-ClickAt ($left + 40) $newRowY
    Save-P005Screenshot -Path (Join-Path $results 'p1-03-dialog.png') -WindowHwnd $win.hwnd
    Record 'dialog_opened' 'p1-03-dialog.png'
    [P005Keyboard]::TypeText('TestCat')
    Start-Sleep -Milliseconds 200
    # Dialog is 320x140 centred; the confirm button sits bottom-right.
    Invoke-ClickAt ($left + 497) ($top + 315)
    Save-P005Screenshot -Path (Join-Path $results 'p1-03-created.png') -WindowHwnd $win.hwnd
    Record 'created' 'typed TestCat and confirmed'
    if (-not (Test-Path $configPath)) { throw 'config was not written after category creation' }
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch 'TestCat') { throw 'created category missing from the persisted config' }
    Record 'persisted' 'config.json contains TestCat'

    # b) Right-click the newly created TestCat row (last row, index 7):
    #    the context menu appears. Row geometry: category bar starts at y=84
    #    (title 44 + group bar 40), scroll starts +10, rows are 36px tall
    #    with a 2px gap (38px pitch).
    $row7Y = $top + 84 + 10 + 7 * 38 + 18
    Invoke-RightClickAt ($left + 40) $row7Y
    Save-P005Screenshot -Path (Join-Path $results 'p1-03-menu.png') -WindowHwnd $win.hwnd
    Record 'menu_opened' 'right-clicked TestCat (last row)'

    # c) Rename TestCat through the menu: 重命名 is the first menu entry and
    #    its centre sits 9px below the popup point. The dialog pre-fills the
    #    current name, so select all and delete before typing.
    Invoke-ClickAt ($left + 105) ($row7Y + 9)
    Start-Sleep -Milliseconds 600
    Invoke-ClickAt ($left + 380) ($top + 251)   # focus the line edit
    [P005Keyboard]::SelectAll()
    [P005Keyboard]::Key(0x2E)                      # Delete
    Start-Sleep -Milliseconds 150
    [P005Keyboard]::TypeText('RenamedCat')
    Start-Sleep -Milliseconds 200
    Invoke-ClickAt ($left + 497) ($top + 315)
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch 'RenamedCat') { throw 'rename did not reach the config' }
    if ($json -notmatch 'tool-category-1') { throw 'rename touched the wrong category' }
    Record 'renamed' 'last category renamed; tool-category-1 untouched'

    # d) Move RenamedCat up via the menu (上移 is the second entry, +34).
    Invoke-RightClickAt ($left + 40) $row7Y
    Invoke-ClickAt ($left + 105) ($row7Y + 34)
    Start-Sleep -Milliseconds 600
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch '(?s)"name"\s*:\s*"RenamedCat".*?"name"\s*:\s*"AI"') {
        throw 'move up did not place RenamedCat before AI'
    }
    Record 'moved_up' 'RenamedCat now before AI in the config order'

    # e) Move it back down (row is now index 6, 下移 is +58), then delete it.
    $row6Y = $top + 84 + 10 + 6 * 38 + 18
    Invoke-RightClickAt ($left + 40) $row6Y
    Invoke-ClickAt ($left + 105) ($row6Y + 58)
    Start-Sleep -Milliseconds 600
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch '(?s)"name"\s*:\s*"AI".*?"name"\s*:\s*"RenamedCat"') {
        throw 'move down did not restore RenamedCat after AI'
    }
    Record 'moved_down' 'RenamedCat back after AI in the config order'

    if ($proc.HasExited) { throw "process exited before the delete phase (exit=$($proc.ExitCode))" }
    $enumerated = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $proc.Id })
    Record "pre_delete_probe" ("enumerated=" + $enumerated.Count + " hwnd=" + $win.hwnd)
    Save-P005Screenshot -Path (Join-Path $results "p1-03-before-delete.png") -WindowHwnd $win.hwnd
    Invoke-RightClickAt ($left + 40) $row7Y
    Invoke-ClickAt ($left + 105) ($row7Y + 84)  # 删除
    Start-Sleep -Milliseconds 400
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -match 'RenamedCat') { throw 'deleted category still in the config' }
    if ($json -notmatch 'tool-category-1') { throw 'delete removed the wrong category' }
    Record 'deleted' 'RenamedCat gone, tool-category-1 intact'
    Save-P005Screenshot -Path (Join-Path $results 'p1-03-after-delete.png') -WindowHwnd $win.hwnd

    # f) Clean exit.
    Close-QnOwnedWindow -Hwnd ([IntPtr]$win.hwnd)
    if (-not $proc.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $proc); throw 'no exit on WM_CLOSE' }
    # With redirected streams the parameterless overload must run before
    # ExitCode is populated.
    $proc.WaitForExit()
    Record 'wm_close_exit' ("exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }

    Record 'completed' 'all P1-03 category phases passed'
    Write-Host 'All P1-03 category phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $proc.HasExited) { try { (Stop-QnOwnedProcess -Process $proc) } catch {}; try { [void]$proc.WaitForExit(5000) } catch {} }
    try {
        $errText = $proc.StandardError.ReadToEnd()
        if ($errText) { Set-Content -LiteralPath (Join-Path $results 'p1-03-stderr.log') -Value $errText -Encoding UTF8 }
    } catch {}
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
