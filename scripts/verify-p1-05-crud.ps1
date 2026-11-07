#requires -Version 5.1
# P1-05 acceptance: add (dialog), edit (prefilled dialog), duplicate, copy path,
# move to another category and delete with confirmation. The config file is
# asserted after every mutation.
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

if (-not ('P105Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P105Input {
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

if (-not ('P105Menu' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public static class P105Menu {
 public struct Point{public int X,Y;}
 public struct Rect{public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out Rect r);
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-05-crud.tsv'
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

function Start-QuickNest([string]$ConfigPath) {
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

function Stop-QuickNest($Run, [string]$Phase) {
    $proc = $Run.Process
    $win = $Run.Window
    if ($proc.HasExited) { throw "process exited before $Phase (exit=$($proc.ExitCode))" }
    Close-QnOwnedWindow -Hwnd ([IntPtr]$win.hwnd)
    if (-not $proc.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $proc); throw "no exit on WM_CLOSE at $Phase" }
    $proc.WaitForExit()
    $err = $proc.StandardError.ReadToEnd()
    if ($err) { Set-Content -LiteralPath (Join-Path $results "stderr-$Phase.log") -Value $err -Encoding UTF8 }
    Record 'wm_close_exit' ("$Phase exit=$($proc.ExitCode)")
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode) at $Phase" }
}

function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 400) {
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
    [P105Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P105Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Set-FieldText([int]$Left, [int]$Top, [int]$X, [int]$Y, [string]$Text, [bool]$Clear) {
    Invoke-ClickAt ($Left + $X) ($Top + $Y) 200
    if ($Clear) {
        [P005Keyboard]::SelectAll()
        [P005Keyboard]::Key(0x2E)
        Start-Sleep -Milliseconds 120
    }
    [P005Keyboard]::TypeText($Text)
    Start-Sleep -Milliseconds 200
}

function Save-WindowOrScreen([string]$Path, [long]$Hwnd) {
    try {
        Save-P005Screenshot -Path $Path -WindowHwnd $Hwnd
    } catch {
        # Window enumeration can briefly miss the window while a native menu
        # is closing; the full-screen capture is still valid evidence.
        Save-P005Screenshot -Path $Path
    }
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$run = Start-QuickNest $configPath
try {
    $win = $run.Window
    $left = $win.rect.Left
    $top = $win.rect.Top
    Record 'window_found' ("pid=$($run.Process.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")

    # Dialog geometry (window-relative): the editor panel is 470x396 centred.
    $nameY = 113
    $targetY = 192
    $argsY = 230
    $saveX = 581
    $saveY = 432

    # a) Add a project through the blank-area menu.
    Invoke-ClickAt ($left + 400) ($top + 400) 300   # activate the window
    Invoke-RightClickAt ($left + 620) ($top + 420)
    Invoke-ClickAt ($left + 660) ($top + 429)
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-dialog-add.png') $win.hwnd
    Record 'dialog_opened' 'p1-05-dialog-add.png'
    Set-FieldText $left $top 370 $nameY 'TestItem' $false
    Set-FieldText $left $top 370 $targetY 'C:\Windows\System32\notepad.exe' $false
    Set-FieldText $left $top 370 $argsY '-a "b c"' $false
    Invoke-ClickAt ($left + $saveX) ($top + $saveY)
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch 'TestItem') { throw 'added project missing from the config' }
    if ($json -notmatch 'b c') { throw 'quoted argument missing from the config' }
    Record 'added' 'TestItem persisted'
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-after-add.png') $win.hwnd

    # b) Edit it: right-click the new tile (index 6, second row first cell).
    Invoke-RightClickAt ($left + 208) ($top + 258)
    Invoke-ClickAt ($left + 273) ($top + 258 + 55)   # 编辑
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-dialog-edit.png') $win.hwnd
    Record 'edit_dialog' 'p1-05-dialog-edit.png'
    Set-FieldText $left $top 370 $nameY 'EditedItem' $true
    Invoke-ClickAt ($left + $saveX) ($top + $saveY)
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch 'EditedItem') { throw 'edited name missing from the config' }
    if ($json -match 'TestItem') { throw 'old name still in the config' }
    Record 'edited' 'EditedItem persisted'

    # c) Duplicate it: the copy lands at index 7 (second row second cell).
    Invoke-RightClickAt ($left + 208) ($top + 258)
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-copy-menu.png') $win.hwnd
    Invoke-ClickAt ($left + 273) ($top + 258 + 135)  # 复制项目
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-after-copy.png') $win.hwnd
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    # "副本" as code points: this script is read as ANSI by PowerShell 5.1
    # without a BOM, so literal CJK would be corrupted.
    $copySuffix = [string][char]0x526F + [string][char]0x672C
    if ($json -notmatch ('EditedItem ' + $copySuffix)) { throw 'duplicate missing from the config' }
    Record 'duplicated' 'duplicate suffix persisted'

    # d) Copy the path of the copy; read the clipboard back.
    Invoke-RightClickAt ($left + 304) ($top + 258)
    Invoke-ClickAt ($left + 369) ($top + 258 + 95)  # 复制路径
    Start-Sleep -Milliseconds 300
    $clip = Get-Clipboard -Raw
    if ($clip -ne 'C:\Windows\System32\notepad.exe') { throw "clipboard mismatch: $clip" }
    Record 'copied_path' $clip

    # e) Move the copy to the second category (系统工具) via the combo.
    Invoke-RightClickAt ($left + 304) ($top + 258)
    Invoke-ClickAt ($left + 369) ($top + 258 + 115)  # 移动到其他分类
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-move-dialog.png') $win.hwnd
    Record 'move_dialog' 'p1-05-move-dialog.png'
    Invoke-ClickAt ($left + 380) ($top + 268) 600    # open the category combo
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-move-combo.png') $win.hwnd
    [P005Keyboard]::Key(0x28)                            # Down -> second entry
    Start-Sleep -Milliseconds 150
    [P005Keyboard]::Key(0x0D)                            # Enter confirms
    Start-Sleep -Milliseconds 300
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-move-selected.png') $win.hwnd
    Invoke-ClickAt ($left + 515) ($top + 311)          # 移动
    $cfg = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $secondItems = @($cfg.groups[0].categories[1].items)
    if ($secondItems.Count -ne 1) { throw "move failed: second category items=$($secondItems.Count)" }
    Record 'moved' 'copy moved into the second category'
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-after-move.png') $win.hwnd

    # f) Delete the moved copy with confirmation.
    Invoke-ClickAt ($left + 40) ($top + 150) 500       # select 系统工具
    $cfg = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($cfg.groups[0].last_category_id -ne 'tool-category-2') {
        # The first click after a modal overlay closes can be swallowed;
        # retry once and require the persisted selection to change.
        Invoke-ClickAt ($left + 40) ($top + 150) 500
        $cfg = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($cfg.groups[0].last_category_id -ne 'tool-category-2') {
            throw "category switch failed: last=$($cfg.groups[0].last_category_id)"
        }
    }
    Record 'category_switched' 'second category selected'
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-second-category.png') $win.hwnd
    Invoke-RightClickAt ($left + 208) ($top + 150)
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-delete-menu.png') $win.hwnd
    # Locate the native menu window and click its last entry (删除). This is
    # robust against the popup offset guessing the fixed offsets required.
    $probe = New-Object P105Menu+Point
    $probe.X = $left + 218
    $probe.Y = $top + 160
    $menuHwnd = [P105Menu]::WindowFromPoint($probe)
    $menuRect = New-Object P105Menu+Rect
    [void][P105Menu]::GetWindowRect($menuHwnd, [ref]$menuRect)
    Record 'delete_menu_rect' ("$($menuRect.Left),$($menuRect.Top),$($menuRect.Right),$($menuRect.Bottom)")
    Invoke-ClickAt ($menuRect.Left + 40) ($menuRect.Bottom - 10)
    Start-Sleep -Milliseconds 400
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-delete-confirm.png') $win.hwnd
    Record 'delete_confirm' 'p1-05-delete-confirm.png'
    Invoke-ClickAt ($left + 500) ($top + 311)           # 删除
    $cfg = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $secondItems = @($cfg.groups[0].categories[1].items)
    if ($secondItems.Count -ne 0) { throw "delete failed: second category items=$($secondItems.Count)" }
    Record 'deleted' 'copy deleted after confirmation'

    # g) The original survives in the first category.
    Invoke-ClickAt ($left + 40) ($top + 108) 400
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch 'EditedItem') { throw 'original removed with the copy' }
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-final.png') $win.hwnd

    # h) The native file picker fills target, name and workdir (baseline 3.2/3.3).
    Invoke-ClickAt ($left + 400) ($top + 400) 300
    Invoke-RightClickAt ($left + 620) ($top + 420)
    Invoke-ClickAt ($left + 660) ($top + 429)
    Start-Sleep -Milliseconds 600
    Invoke-ClickAt ($left + 585) ($top + 192) 800   # 浏览…
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-browse-dialog.png') $win.hwnd
    # The native picker keeps the file name in a plain EDIT child; pasting
    # into the dialog window itself would land in the address bar.
    [void][P005Keyboard]::SetClipboard('C:\Windows\System32\notepad.exe')
    Start-Sleep -Milliseconds 150
    $fileNameEdit = [P005Keyboard]::FindFileNameEditByPid($run.Process.Id)
    if ($fileNameEdit -eq [IntPtr]::Zero) { throw 'file name edit of the picker not found' }
    [P005Keyboard]::PasteInto($fileNameEdit)
    Start-Sleep -Milliseconds 300
    [P005Keyboard]::Key(0x0D)
    Start-Sleep -Milliseconds 800
    Save-WindowOrScreen -Path (Join-Path $results 'p1-05-browse-filled.png') $win.hwnd
    Invoke-ClickAt ($left + $saveX) ($top + $saveY)
    $cfg = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $picked = @($cfg.groups[0].categories[0].items | Where-Object { $_.name -eq 'notepad' })
    if ($picked.Count -ne 1) { throw "browse did not add the picked file (count=$($picked.Count))" }
    if ($picked[0].item_type -ne 'exe') { throw "browse type inference failed: $($picked[0].item_type)" }
    if ($picked[0].launch.workdir -ne 'C:\Windows\System32') {
        throw "browse workdir inference failed: $($picked[0].launch.workdir)"
    }
    Record 'browsed' 'picker filled name/type/workdir'

    Stop-QuickNest $run 'final'
    Record 'completed' 'all P1-05 CRUD phases passed'
    Write-Host 'All P1-05 CRUD phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
    return
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
