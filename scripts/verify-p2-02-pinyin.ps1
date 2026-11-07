#requires -Version 5.1
# P2-02 acceptance (baseline 13.2/13.3): full pinyin, initials, and common
# polyphone candidates find Chinese-named items; clicking launches them.
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
public static class P202Input {
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
$report = Join-Path $results 'p2-02-pinyin.tsv'
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
$run = Join-Path $tempRoot ('QuickNestP202-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($run)
$weixinProbe = Join-Path $run 'weixin.txt'
$neteaseProbe = Join-Path $run 'netease.txt'
$musicProbe = Join-Path $run 'music.txt'
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
            id = 'g1'; name = 'PinyinGroup'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'PinyinCat'; icon = ''
                    items = @(
                        @{ id = 'weixin'; name = '微信测试'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo weixin>' + $weixinProbe)); workdir = $run; run_as_admin = $false } },
                        @{ id = 'netease'; name = '网易云音乐'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo netease>' + $neteaseProbe)); workdir = $run; run_as_admin = $false } },
                        @{ id = 'music'; name = '音乐盒'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = $cmd; args = @('/c', ('echo music>' + $musicProbe)); workdir = $run; run_as_admin = $false } }
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
    Show-QuickNest $hwnd
    Record 'window_ready' ("pid=$($app.Id) rect=$left,$top,${width}x$($window.rect.Bottom - $window.rect.Top)")

    $panelLeft = $left + [int](($width - 600) / 2)
    $panelTop = $top + 44 + 20
    $inputX = $panelLeft + 220
    $inputY = $panelTop + 33
    $resultX = $panelLeft + 300
    $firstResultY = $panelTop + 109

    function Search-And-Launch([string]$Query, [string]$Probe, [string]$Expected, [string]$Capture) {
        Show-QuickNest $hwnd
        Send-CtrlF
        Invoke-ClickAt $inputX $inputY 250
        [P005Keyboard]::TypeText($Query)
        Start-Sleep -Milliseconds 500
        Save-P005Screenshot -Path (Join-Path $results $Capture) -WindowHwnd $hwnd
        Invoke-ClickAt $resultX $firstResultY 900
        Assert-True (Test-Path -LiteralPath $Probe) "query '$Query' did not launch the expected item"
        Assert-True ((Get-Content -LiteralPath $Probe -Raw).Trim() -eq $Expected) "query '$Query' launched the wrong item"
        Remove-QnSandboxItem -LiteralPath $Probe -Force
        Record 'query_launch' ("query=$Query launched $Expected")
    }

    # a) full pinyin: weixin -> 微信测试
    Search-And-Launch 'weixin' $weixinProbe 'weixin' 'p2-02-weixin.png'
    # b) initials: wx -> 微信测试
    Search-And-Launch 'wx' $weixinProbe 'weixin' 'p2-02-wx.png'
    # c) initials prefix: wyy -> 网易云音乐
    Search-And-Launch 'wyy' $neteaseProbe 'netease' 'p2-02-wyy.png'
    # d) polyphone: yinyue -> 音乐盒 (乐 read as yue)
    Search-And-Launch 'yinyue' $musicProbe 'music' 'p2-02-yinyue.png'

    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P2-02 pinyin phases passed'
    Write-Host 'All P2-02 pinyin phases passed.'
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
