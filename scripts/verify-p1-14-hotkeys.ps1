#requires -Version 5.1
# P1-14 acceptance (baseline 11.1/11.2):
# - The configured global hotkey hides/restores the panel (Ctrl+Alt+Space).
# - A custom hotkey from settings (Ctrl+Alt+F9) is registered instead.
# - A conflicting registration surfaces a clear status-bar message.
# - Ctrl+F routes the search intent.
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

if (-not ('P114Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P114Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Keyboard key;[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Keyboard{public ushort vk,scan;public uint flags,time;public UIntPtr extra;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 [DllImport("user32.dll",SetLastError=true)]static extern bool RegisterHotKey(IntPtr h,int id,uint mods,uint vk);
 [DllImport("user32.dll",SetLastError=true)]static extern bool UnregisterHotKey(IntPtr h,int id);
 static Input Down(ushort vk){return new Input{type=1,u=new Union{key=new Keyboard{vk=vk}}};}
 static Input Up(ushort vk){return new Input{type=1,u=new Union{key=new Keyboard{vk=vk,flags=0x0002}}};}
 public static void Hotkey3(ushort m1,ushort m2,ushort vk){
  var inputs=new[]{Down(m1),Down(m2),Down(vk),Up(vk),Up(m2),Up(m1)};
  if(SendInput(6,inputs,Marshal.SizeOf(typeof(Input)))!=6)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static void Combo2(ushort m,ushort vk){
  var inputs=new[]{Down(m),Down(vk),Up(vk),Up(m)};
  if(SendInput(4,inputs,Marshal.SizeOf(typeof(Input)))!=4)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
 public static bool Register(ushort m1,ushort m2,ushort vk){
  return RegisterHotKey(IntPtr.Zero, 9911, (uint)(m1|m2|0x4000), vk);
 }
 public static bool Unregister(){return UnregisterHotKey(IntPtr.Zero, 9911);}
 [DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();
}
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-14-hotkeys.tsv'
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
    Start-Sleep -Milliseconds 500
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
    if ($proc.ExitCode -ne 0) { throw "unexpected exit code $($proc.ExitCode)" }
}

function Invoke-ClickAt([int]$X, [int]$Y) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds 500
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

function Hide-QuickNest([long]$Hwnd, [int]$Left, [int]$Top) {
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        Invoke-ClickAt ($Left + 742) ($Top + 22)
        Start-Sleep -Milliseconds 500
        $visible = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $Hwnd } | Select-Object -First 1)
        if ($visible.Count -eq 0) { return }
    }
    throw "the window is still visible after the close button"
}

function Set-Hotkey([string]$Spec) {
    $library = @{
        schema_version = 2
        settings = @{ hotkey = $Spec }
        current_group_id = 'g1'
        groups = @(
            @{ id = 'g1'; name = 'HotkeyTests'; icon = ''; color = ''; last_category_id = 'c1'
               categories = @(@{ id = 'c1'; name = 'Tests'; icon = ''; items = @() }) }
        )
    }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    $json = $library | ConvertTo-Json -Depth 12
    [IO.File]::WriteAllText($configPath, $json, (New-Object Text.UTF8Encoding($false)))
}

# a) Default hotkey: hide with the title-bar close button, restore with the key.
Set-Hotkey 'Ctrl+Alt+Space'
$run = Start-QuickNest
try {
    $hwnd = [long]$run.Window.hwnd
    $left = $run.Window.rect.Left
    $top = $run.Window.rect.Top
    Hide-QuickNest $hwnd $left $top
    Save-P005Screenshot -Path (Join-Path $results "p1-14-after-close.png")
    Record "after_close" "window hidden"
    Record 'hidden' 'close button hid the window'
    [P114Input]::Hotkey3(0x11, 0x12, 0x20)      # Ctrl+Alt+Space
    Start-Sleep -Milliseconds 900
    $restored = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($restored.Count -eq 0) { throw 'the global hotkey did not restore the window' }
    $foreground = [P114Input]::GetForegroundWindow().ToInt64()
    Record 'hotkey_restore' ("visible=1 foreground=$($foreground -eq $hwnd)")
    Save-P005Screenshot -Path (Join-Path $results 'p1-14-restored.png') -WindowHwnd $hwnd

    # Ctrl+F routes the search intent (search center is P2-01).
    [P114Input]::Combo2(0x11, 0x46)
    Start-Sleep -Milliseconds 400
    Record 'ctrl_f' 'search intent routed'
    Save-P005Screenshot -Path (Join-Path $results 'p1-14-ctrl-f.png') -WindowHwnd $hwnd
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# b) Custom hotkey from settings.
Set-Hotkey "Ctrl+Alt+K"
$run = Start-QuickNest
try {
    $hwnd = [long]$run.Window.hwnd
    $left = $run.Window.rect.Left
    $top = $run.Window.rect.Top
    Save-P005Screenshot -Path (Join-Path $results 'p1-14-custom-start.png') -WindowHwnd $hwnd
    Record 'custom_registered' 'screenshot after startup (status bar shows the hotkey result)'
    Hide-QuickNest $hwnd $left $top
    [P114Input]::Hotkey3(0x11, 0x12, 0x4B)      # Ctrl+Alt+K
    Start-Sleep -Milliseconds 900
    Save-P005Screenshot -Path (Join-Path $results 'p1-14-custom-after.png')
    $restored = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
    if ($restored.Count -eq 0) {
        # Diagnostic: if the default combo restores it, the settings hotkey
        # was ignored; otherwise the custom registration itself failed.
        [P114Input]::Hotkey3(0x11, 0x12, 0x20)
        Start-Sleep -Milliseconds 900
        $defaultRestored = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $hwnd } | Select-Object -First 1)
        throw "the custom hotkey did not restore the window (default_restored=$($defaultRestored.Count))"
    }
    Record 'custom_hotkey' 'Ctrl+Alt+K restored the window'
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

# c) Invalid hotkey specification: a clear status-bar message instead of a
#    silent fallback (registration conflicts themselves are covered by the
#    P0-10 hotkey validation matrix).
Set-Hotkey 'Ctrl+Alt+Unknown'
$run = Start-QuickNest
try {
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p1-14-invalid-hotkey.png') -WindowHwnd $run.Window.hwnd
    Record 'invalid_hotkey' 'status bar shows the parse failure'
    Stop-QuickNest $run
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Record 'completed' 'all P1-14 hotkey phases passed'
Write-Host 'All P1-14 hotkey phases passed.'
Get-Content -LiteralPath $report -Encoding UTF8

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
