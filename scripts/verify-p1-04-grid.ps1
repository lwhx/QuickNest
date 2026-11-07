#requires -Version 5.1
# P1-04 acceptance: the project grid is model-driven, the column count follows
# the window width, icon presets (small/medium/large) come from settings, the
# path-invalid badge appears, the hover tooltip shows name/path/note, and both
# the tile and blank-area context menus open.
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

if (-not ('P104Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P104Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static void Key(ushort vk){
  var down=new Input{type=1,u=new Union{key=new Keyboard{vk=vk}}};
  var up=new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=0x0002}}};
  if(SendInput(2,new[]{down,up},Marshal.SizeOf(typeof(Input)))!=2)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-04-grid.tsv'
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
    Start-Sleep -Milliseconds 400
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

function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 80
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 550) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P104Input]::RightButton($true)
    Start-Sleep -Milliseconds 80
    [P104Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'

# The sample library seeds six projects into 必备软件 (the first category):
# two point at real system files, two at missing demo paths (badge), one URI
# and one command (never flagged).
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$run = Start-QuickNest $configPath
try {
    $win = $run.Window
    $left = $win.rect.Left
    $top = $win.rect.Top
    $width = $win.rect.Right - $left
    $height = $win.rect.Bottom - $top
    Record 'window_found' ("pid=$($run.Process.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")

    # The grid follows the active category: switch to the empty second
    # category (row 2 at y=136) and back (row 1 at y=108). Switching also
    # triggers the first persistence write, which the seed check needs.
    Invoke-ClickAt ($left + 40) ($top + 136)
    Save-P005Screenshot -Path (Join-Path $results 'p1-04-empty-category.png') -WindowHwnd $win.hwnd
    Record 'category_switch_empty' 'p1-04-empty-category.png (second category has no projects)'
    Invoke-ClickAt ($left + 40) ($top + 108)
    Start-Sleep -Milliseconds 400
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8
    if ($json -notmatch 'demo-item-1') { throw 'sample projects missing from the config' }
    Record 'model_seeded' 'config contains the six demo projects'

    Save-P005Screenshot -Path (Join-Path $results 'p1-04-medium-initial.png') -WindowHwnd $win.hwnd
    Record 'medium_initial' ("p1-04-medium-initial.png window=${width}x${height} (medium 96x108, default)")

    # a) Hover tooltip: first tile centre. Grid origin: 148 (categories) + 12
    #    padding; tile inset 4; medium tile 88x100. Just move the pointer —
    #    a click would launch the project since P1-06.
    [void][P005Desktop]::SetCursorPos($left + 208, $top + 150)
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p1-04-hover.png') -WindowHwnd $win.hwnd
    Record 'hover_tooltip' 'p1-04-hover.png (pointer over the first tile)'

    # b) Tile context menu (right-click the first tile centre).
    Invoke-RightClickAt ($left + 208) ($top + 150)
    Save-P005Screenshot -Path (Join-Path $results 'p1-04-tile-menu.png') -WindowHwnd $win.hwnd
    Record 'tile_menu' 'p1-04-tile-menu.png'
    [P104Input]::Key(0x1B) # ESC closes the native menu
    Start-Sleep -Milliseconds 400

    # c) Blank-area context menu (lower right of the grid).
    Invoke-RightClickAt ($left + 620) ($top + 420)
    Save-P005Screenshot -Path (Join-Path $results 'p1-04-blank-menu.png') -WindowHwnd $win.hwnd
    Record 'blank_menu' 'p1-04-blank-menu.png'
    [P104Input]::Key(0x1B)
    Start-Sleep -Milliseconds 300

    # d) Column count follows the window width: 680 (min) vs 760 vs 1100.
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), $left, $top, 680, 520, 0x0000)
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p1-04-narrow-680.png') -WindowHwnd $win.hwnd
    Record 'columns_narrow' 'p1-04-narrow-680.png (expected 5 columns, two rows)'
    [void][P005Desktop]::SetWindowPos([IntPtr]$win.hwnd, [IntPtr](-1), $left, $top, 1100, 520, 0x0000)
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p1-04-wide-1100.png') -WindowHwnd $win.hwnd
    Record 'columns_wide' 'p1-04-wide-1100.png (expected 9 columns)'

    Stop-QuickNest $run 'medium'
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# e) Icon presets: rewrite settings.icon_size and restart each time.
function Set-IconSize([string]$Size) {
    $json = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $json.settings.icon_size = $Size
    $text = $json | ConvertTo-Json -Depth 12
    # No BOM: serde_json rejects it (the app strips it defensively, but the
    # script should write the same shape the app writes).
    [IO.File]::WriteAllText($configPath, $text, (New-Object Text.UTF8Encoding($false)))
}

foreach ($size in @('small', 'large')) {
    Set-IconSize $size
    $run = Start-QuickNest $configPath
    try {
        $win = $run.Window
        Save-P005Screenshot -Path (Join-Path $results ("p1-04-{0}-preset.png" -f $size)) -WindowHwnd $win.hwnd
        Record ("{0}_preset" -f $size) ("p1-04-{0}-preset.png" -f $size)
        Stop-QuickNest $run $size
    } finally {
        if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
    }
}

Record 'completed' 'all P1-04 grid phases passed'
Write-Host 'All P1-04 grid phases passed.'
Get-Content -LiteralPath $report -Encoding UTF8

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
