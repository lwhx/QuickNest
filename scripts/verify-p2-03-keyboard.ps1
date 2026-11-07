#requires -Version 5.1
# P2-03 acceptance (baseline 13.4): default first selection, arrow movement
# with wrap, Enter launch, Tab scope toggle, Ctrl+D open location, Esc handled.
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
public static class P203Input {
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
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-03-keyboard.tsv'
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

function Send-CtrlF {
    [P005Keyboard]::Chord(0x11, 0x46)
    Start-Sleep -Milliseconds 350
}

function Send-Key([int]$VirtualKey) {
    [P005Keyboard]::Key([uint16]$VirtualKey)
    Start-Sleep -Milliseconds 300
}

function Send-CtrlD {
    [P005Keyboard]::Chord(0x11, 0x44)
    Start-Sleep -Milliseconds 400
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

$tempRoot = [IO.Path]::GetTempPath().TrimEnd('\')
$run = Join-Path $tempRoot ('QuickNestP203-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$locateFile = Join-Path $run 'locate-me.txt'
[IO.File]::WriteAllText($locateFile, 'QuickNest P2-03 locate fixture')
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$cmd = Join-Path $env:WINDIR 'System32\cmd.exe'
function Probe-Item([string]$Id, [string]$Name, [string]$Probe, [string]$Text) {
    return @{ id = $Id; name = $Name; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
              launch = @{ path = $cmd; args = @('/c', ('echo ' + $Text + '>' + $Probe)); workdir = $run; run_as_admin = $false } }
}
$alphaA = Join-Path $run 'alphaA.txt'
$alphaB = Join-Path $run 'alphaB.txt'
$alphaC = Join-Path $run 'alphaC.txt'
$betaProbe = Join-Path $run 'beta.txt'
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'KeyGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'KeyCat'; icon = ''
                    items = @(
                        (Probe-Item 'alphaA' 'AlphaA' $alphaA 'a'),
                        (Probe-Item 'alphaB' 'AlphaB' $alphaB 'b'),
                        (Probe-Item 'alphaC' 'AlphaC' $alphaC 'c'),
                        @{ id = 'locate'; name = 'LocateFile'; item_type = 'file'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $locateFile; args = @(); workdir = $run; run_as_admin = $false } }
                    )
                }
            )
        },
        @{
            id = 'g2'; name = 'OtherGroup'; icon = ''; color = ''; last_category_id = 'c2'
            categories = @(
                @{
                    id = 'c2'; name = 'OtherCat'; icon = ''
                    items = @( Probe-Item 'beta' 'BetaOther' $betaProbe 'beta' )
                }
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
$explorerHwnd = 0
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
    $width = $window.rect.Right - $window.rect.Left
    Show-QuickNest $hwnd
    Record 'window_ready' ("pid=$($app.Id) rect=$left,$top,${width}x$($window.rect.Bottom - $window.rect.Top)")

    $panelLeft = $left + [int](($width - 600) / 2)
    $panelTop = $top + 44 + 20
    $inputX = $panelLeft + 220
    $inputY = $panelTop + 33

    function Open-Search([string]$Query) {
        Show-QuickNest $hwnd
        Send-CtrlF
        Invoke-ClickAt $inputX $inputY 250
        [P005Keyboard]::TypeText($Query)
        Start-Sleep -Milliseconds 500
    }

    # a) Default first selection + Down + Enter launches AlphaB.
    Open-Search 'alpha'
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-default-first.png') -WindowHwnd $hwnd
    Send-Key 0x28   # Down
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-after-down.png') -WindowHwnd $hwnd
    Send-Key 0x0D   # Enter
    Start-Sleep -Milliseconds 700
    Assert-True (Test-Path -LiteralPath $alphaB) 'Down+Enter did not launch AlphaB'
    Assert-True (-not (Test-Path -LiteralPath $alphaA)) 'Down+Enter launched the first item instead'
    Record 'down_enter' 'default first; Down selected AlphaB; Enter launched it'
    Remove-QnSandboxItem -LiteralPath $alphaB -Force

    # b) Up from the first result wraps to the last (AlphaC).
    Open-Search 'alpha'
    Send-Key 0x26   # Up
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-after-up.png') -WindowHwnd $hwnd
    Send-Key 0x0D   # Enter
    Start-Sleep -Milliseconds 700
    Assert-True (Test-Path -LiteralPath $alphaC) 'Up+Enter did not wrap to AlphaC'
    Assert-True (-not (Test-Path -LiteralPath $alphaA)) 'Up from the first item did not wrap'
    Record 'up_wrap_enter' 'Up wrapped to AlphaC and Enter launched it'
    Remove-QnSandboxItem -LiteralPath $alphaC -Force

    # c) Tab switches the scope; Enter launches the cross-group result.
    Open-Search 'beta'
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-tab-current.png') -WindowHwnd $hwnd
    Send-Key 0x09   # Tab
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-tab-all.png') -WindowHwnd $hwnd
    Send-Key 0x0D   # Enter
    Start-Sleep -Milliseconds 700
    Assert-True (Test-Path -LiteralPath $betaProbe) 'Tab+Enter did not launch the all-group BetaOther'
    Record 'tab_scope_enter' 'Tab switched to all groups; Enter launched BetaOther'
    Remove-QnSandboxItem -LiteralPath $betaProbe -Force

    # d) Ctrl+D opens the selected item's folder.
    Open-Search 'locate'
    Send-CtrlD
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $explorer = $null
    $leaf = Split-Path -Leaf $run
    while ($null -eq $explorer -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 200
        # The folder window may not exist yet; indexing an empty array would
        # throw instead of retrying.
        $explorerMatches = @([P005Desktop]::Windows() | Where-Object { $_.title -like "*$leaf*" })
        $explorer = if ($explorerMatches.Count -gt 0) { $explorerMatches[0] } else { $null }
    }
    Assert-True ($null -ne $explorer) 'Ctrl+D did not open the containing folder'
    $explorerHwnd = [long]$explorer.hwnd
    Record 'ctrl_d_location' ("explorer title=$($explorer.title)")
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-ctrl-d.png') -WindowHwnd $hwnd

    # e) Esc still clears then closes.
    Show-QuickNest $hwnd
    Send-CtrlF
    Invoke-ClickAt $inputX $inputY 250
    [P005Keyboard]::TypeText('alpha')
    Start-Sleep -Milliseconds 400
    Send-Key 0x1B
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-esc-cleared.png') -WindowHwnd $hwnd
    Send-Key 0x1B
    Save-P005Screenshot -Path (Join-Path $results 'p2-03-esc-closed.png') -WindowHwnd $hwnd
    Record 'esc_semantics' 'first Esc cleared, second closed'

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-03 keyboard phases passed'
    Write-Host 'All P2-03 keyboard phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    if ($explorerHwnd -ne 0) { try { Close-QnOwnedWindow -Hwnd ([IntPtr]$explorerHwnd) } catch { } }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
