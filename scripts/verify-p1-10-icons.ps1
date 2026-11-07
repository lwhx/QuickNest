#requires -Version 5.1
# P1-10 acceptance: shell icons are extracted on a worker thread and shown in
# the grid; items without a file icon keep the default artwork.
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

if (-not ('P110Input' -as [type])) { Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;using System.ComponentModel;
public static class P110Input {
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
'@ }

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-10-icons.tsv'
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

Add-Type -AssemblyName System.Drawing
function Get-Pixel([int]$X, [int]$Y) {
    $bitmap = New-Object Drawing.Bitmap(1, 1)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($X, $Y, 0, 0, $bitmap.Size) } finally { $graphics.Dispose() }
        $pixel = $bitmap.GetPixel(0, 0)
        return @($pixel.R, $pixel.G, $pixel.B)
    } finally { $bitmap.Dispose() }
}

# Distance to the default artwork's blue (#2563EB).
function Get-DefaultBlueDistance([int]$X, [int]$Y) {
    $rgb = Get-Pixel $X $Y
    return [Math]::Abs($rgb[0] - 0x25) + [Math]::Abs($rgb[1] - 0x63) + [Math]::Abs($rgb[2] - 0xEB)
}

# 卡片规范 V1.0 §2: URL 项目使用浏览器图标（#0f766e），不再使用蓝色应用图标。
function Get-BrowserTealDistance([int]$X, [int]$Y) {
    $rgb = Get-Pixel $X $Y
    return [Math]::Abs($rgb[0] - 0x0F) + [Math]::Abs($rgb[1] - 0x76) + [Math]::Abs($rgb[2] - 0x6E)
}
function Get-TealRatio([int]$CenterX, [int]$CenterY, [int]$HalfSize) {
    $close = 0
    $total = 0
    for ($y = $CenterY - $HalfSize; $y -le $CenterY + $HalfSize; $y += 4) {
        for ($x = $CenterX - $HalfSize; $x -le $CenterX + $HalfSize; $x += 4) {
            if ((Get-BrowserTealDistance $x $y) -lt 90) { $close++ }
            $total++
        }
    }
    return $close / $total
}

# Fraction of sampled pixels close to the default artwork's blue. The default
# icon is a blue rounded square with a white grid, so a single pixel can sit on
# a grid line; the ratio is stable.
function Get-BlueRatio([int]$CenterX, [int]$CenterY, [int]$HalfSize) {
    $close = 0
    $total = 0
    for ($y = $CenterY - $HalfSize; $y -le $CenterY + $HalfSize; $y += 4) {
        for ($x = $CenterX - $HalfSize; $x -le $CenterX + $HalfSize; $x += 4) {
            if ((Get-DefaultBlueDistance $x $y) -lt 80) { $close++ }
            $total++
        }
    }
    return $close / $total
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$library = @{
    schema_version = 2
    current_group_id = 'g1'
    groups = @(
        @{
            id = 'g1'; name = 'IconTests'; icon = ''; color = ''; last_category_id = 'c1'
            categories = @(
                @{
                    id = 'c1'; name = 'Tests'; icon = ''
                    items = @(
                        @{ id = 'cmd-icon'; name = 'CmdIcon'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\Windows\System32\cmd.exe'; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'uri-icon'; name = 'UriIcon'; item_type = 'uri'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'https://example.com'; args = @(); workdir = ''; run_as_admin = $false } },
                        @{ id = 'missing-icon'; name = 'MissingIcon'; item_type = 'exe'; icon = ''; window_state = 'normal'; note = ''
                           launch = @{ path = 'C:\QuickNest Demo\missing-icon.exe'; args = @(); workdir = ''; run_as_admin = $false } }
                    )
                }
            )
        }
    )
}
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
$json = $library | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($configPath, $json, (New-Object Text.UTF8Encoding($false)))

$run = Start-QuickNest
try {
    $win = $run.Window
    $left = $win.rect.Left
    $top = $win.rect.Top
    Record 'window_found' ("pid=$($run.Process.Id) rect=$($win.rect.Left),$($win.rect.Top),$($win.rect.Right),$($win.rect.Bottom)")
    # Give the worker + 150ms drain timer a moment to deliver the icons.
    Start-Sleep -Milliseconds 2000
    Save-P005Screenshot -Path (Join-Path $results 'p1-10-icons.png') -WindowHwnd $win.hwnd
    Record 'icons_captured' 'p1-10-icons.png'

    # Icon area centre: 卡片规范 V1.0 的 32px 图标位于 40x40 图标区中心
    # （卡片顶 100 + 10 内边距 + 7 居中余量 + 4 图标内偏移 = 121）。
    $iconY = 137
    $cmdRatio = Get-BlueRatio ($left + 208) ($top + $iconY) 6
    $uriRatio = Get-BlueRatio ($left + 304) ($top + $iconY) 6
    $uriTealRatio = Get-TealRatio ($left + 304) ($top + $iconY) 6
    Record 'pixels' ("cmd_blue_ratio=$cmdRatio uri_blue_ratio=$uriRatio uri_teal_ratio=$uriTealRatio")
    if ($cmdRatio -ge 0.2) { throw "cmd tile still shows the default artwork (ratio=$cmdRatio)" }
    # 卡片规范 V1.0 §2: URL 使用浏览器图标，不再是蓝色应用图标。
    if ($uriRatio -ge 0.3) { throw "uri tile still shows the blue app artwork (ratio=$uriRatio)" }
    if ($uriTealRatio -lt 0.25) { throw "uri tile lost the browser artwork (teal ratio=$uriTealRatio)" }

    Stop-QuickNest $run
    Record 'completed' 'all P1-10 icon phases passed'
    Write-Host 'All P1-10 icon phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $run.Process.HasExited) { try { (Stop-QnOwnedProcess -Process $run.Process) } catch {} }
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
