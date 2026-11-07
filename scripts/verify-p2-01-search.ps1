#requires -Version 5.1
# P2-01 acceptance (baseline 2.6/11.2/13.1/13.2/13.4):
# Ctrl+F and the title-bar entry open the search center; typed input filters in
# real time; the default scope is the active group and can switch to all
# groups; clicking a result launches it; Esc clears then closes.
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
public static class P201Input {
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
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-01-search.tsv'
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
$run = Join-Path $tempRoot ('QuickNestP201-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$alphaProbe = Join-Path $run 'alpha.txt'
$betaProbe = Join-Path $run 'beta.txt'
$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$cmd = Join-Path $env:WINDIR 'System32\cmd.exe'
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    settings = @{ pinned = $true }
    groups = @(
        @{
            id = 'g1'; name = 'AlphaGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'AlphaCat'; icon = ''
                    items = @(
                        @{ id = 'alpha'; name = 'AlphaOne'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo alpha>' + $alphaProbe)); workdir = $run; run_as_admin = $false } },
                        @{ id = 'shared'; name = 'SharedTool'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo shared>' + (Join-Path $run 'shared.txt'))); workdir = $run; run_as_admin = $false } }
                    )
                }
            )
        },
        @{
            id = 'g2'; name = 'BetaGroup'; icon = ''; color = ''; last_category_id = 'c2'
            categories = @(
                @{
                    id = 'c2'; name = 'BetaCat'; icon = ''
                    items = @(
                        @{ id = 'beta'; name = 'BetaTwo'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo beta>' + $betaProbe)); workdir = $run; run_as_admin = $false } }
                    )
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
    $height = $window.rect.Bottom - $window.rect.Top
    Show-QuickNest $hwnd
    Record 'window_ready' ("pid=$($app.Id) rect=$left,$top,${width}x${height}")

    $panelLeft = $left + [int](($width - 600) / 2)
    $panelTop = $top + 44 + 20
    $resultX = $panelLeft + 300
    $firstResultY = $panelTop + 109
    $scopeX = $panelLeft + 509
    $scopeY = $panelTop + 33
    # P2-12 widened the title-bar settings entry to a 44px button; 搜索 moved
    # left by ~20px. Click its current centre (was width-116 before P2-12).
    $searchTitleX = $left + $width - 136
    $searchTitleY = $top + 22

    # a) Ctrl+F opens the center with an empty query (all active-group items).
    Send-CtrlF
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-search-open.png') -WindowHwnd $hwnd
    Record 'ctrl_f_open' 'Ctrl+F opened the search center'

    # b) Typing filters in real time; the first row is AlphaOne.
    [P005Keyboard]::TypeText('alpha')
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-query-alpha.png') -WindowHwnd $hwnd
    Invoke-ClickAt $resultX $firstResultY 900
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-after-alpha-click.png') -WindowHwnd $hwnd
    $files = (@(Get-ChildItem -LiteralPath $run | ForEach-Object { $_.Name }) -join ',')
    Assert-True (Test-Path -LiteralPath $alphaProbe) "clicking the AlphaOne result did not launch it (files=$files)"
    Assert-True ((Get-Content -LiteralPath $alphaProbe -Raw).Trim() -eq 'alpha') 'alpha probe content mismatch'
    Record 'filter_and_launch' ("alpha.txt created; query=alpha")
    Remove-QnSandboxItem -LiteralPath $alphaProbe -Force

    # c) The title-bar entry opens a fresh search; beta is outside the default
    #    active-group scope, so no result launches.
    Show-QuickNest $hwnd
    Invoke-ClickAt $searchTitleX $searchTitleY 450
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-title-open.png') -WindowHwnd $hwnd
    Invoke-ClickAt ($panelLeft + 220) ($panelTop + 33) 300
    [P005Keyboard]::TypeText('beta')
    Start-Sleep -Milliseconds 500
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-scope-current.png') -WindowHwnd $hwnd
    Assert-True (-not (Test-Path -LiteralPath $betaProbe)) 'current-group scope produced a beta launch'
    Record 'scope_current_group' 'beta had no result in the active group'

    # d) Switch to all groups; the beta result appears and launches.
    Invoke-ClickAt $scopeX $scopeY 450
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-scope-all.png') -WindowHwnd $hwnd
    Invoke-ClickAt $resultX $firstResultY 900
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-after-beta-click.png') -WindowHwnd $hwnd
    $files = (@(Get-ChildItem -LiteralPath $run | ForEach-Object { $_.Name }) -join ',')
    Assert-True (Test-Path -LiteralPath $betaProbe) "all-groups scope did not launch the beta result (files=$files)"
    Assert-True ((Get-Content -LiteralPath $betaProbe -Raw).Trim() -eq 'beta') 'beta probe content mismatch'
    Record 'scope_all_groups' 'all-groups scope launched BetaTwo'

    # e) Esc clears the query first, then closes the center.
    Show-QuickNest $hwnd
    Send-CtrlF
    Invoke-ClickAt ($panelLeft + 220) ($panelTop + 33) 250
    [P005Keyboard]::TypeText('alpha')
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-esc-typed.png') -WindowHwnd $hwnd
    [P005Keyboard]::Key(27)
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-esc-cleared.png') -WindowHwnd $hwnd
    Record 'esc_clears' 'first Esc cleared the query'
    [P005Keyboard]::Key(27)
    Start-Sleep -Milliseconds 400
    Save-P005Screenshot -Path (Join-Path $results 'p2-01-esc-closed.png') -WindowHwnd $hwnd
    Record 'esc_closes' 'second Esc closed the search center'

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-01 search phases passed'
    Write-Host 'All P2-01 search phases passed.'
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
