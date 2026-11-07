#requires -Version 5.1
# P4-02 acceptance (baseline 23 章 便携版 / 29 章 P4-02 绿色便携版).
#
# 全链路：构建便携包 → 解压到根 A（隔离目录）→ 首次启动验证数据目录落在
# <根A>\data（而非 %APPDATA%）→ 预置相对路径项目（cmd 脚本以 %~dp0 写探针
# 到自身目录）→ 单击卡片启动（相对路径按根 A 解析）→ 关闭后把整个文件夹
# 移动到根 B（同一隔离根的新目录）→ 重启验证数据随文件夹迁移 → 再单击启动
# （相对路径按新根 B 解析，探针落在根 B）→ 全程 %APPDATA% 不受影响。
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
using System;using System.Runtime.InteropServices;
public static class P42Window {
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern IntPtr FindWindow(string cls, string title);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p4-02-portable.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$zip = Join-Path $root 'installer\output\QuickNest-1.2.0-portable.zip'
$rootA = Join-Path $results 'root-a'
$rootB = Join-Path $results 'root-b'
$roamingConfig = Join-Path $env:APPDATA 'QuickNest/config.json'
$roamingHashBefore = if (Test-Path -LiteralPath $roamingConfig) { (Get-FileHash -LiteralPath $roamingConfig).Hash } else { $null }

function Assert-RoamingUnchanged {
    $after = if (Test-Path -LiteralPath $roamingConfig) { (Get-FileHash -LiteralPath $roamingConfig).Hash } else { $null }
    Assert-True ($after -eq $roamingHashBefore) 'portable run changed the real roaming configuration'
}
function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Wait-True([scriptblock]$Condition, [int]$TimeoutSeconds, [string]$Message) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 250
    }
    throw $Message
}
function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
function Start-Portable([string]$ExePath) {
    Stop-QnTestProcesses
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $ExePath
    $psi.UseShellExecute = $false
    # Portable resolution is the subject under test. Remove the override only
    # from this child's environment, retaining the caller's data-root unchanged.
    $psi.EnvironmentVariables.Remove('QUICKNEST_DATA_DIR')
    $app = New-Object Diagnostics.Process
    $app.StartInfo = $psi
    if (-not $app.Start()) { throw 'portable child failed to start' }
    Register-QnOwnedProcess $app
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    $window = $null
    while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 200
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
        if ($window.Count -ge 1) { $window = $window[0] } else { $window = $null }
    }
    if (-not $window) { throw 'portable app window not found' }
    [void][P005Desktop]::SetWindowPos([IntPtr]$window.hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
    Start-Sleep -Milliseconds 400
    Focus-P117Window $window.hwnd
    Start-Sleep -Milliseconds 900
    # 应用会异步应用窗口位置记忆（P2-15），SetWindowPos 后必须二次枚举，
    # 否则卡片坐标错位（2026-11-06：rect 60,60 -> 记忆位置 26,26）。
    $final = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })
    if ($final.Count -ge 1) { $window = $final[0] }
    return @{ App = $app; Hwnd = [long]$window.hwnd; Left = $window.rect.Left; Top = $window.rect.Top }
}
function Stop-Portable($Run) {
    Close-QnOwnedWindow -Hwnd ([IntPtr]$Run.Hwnd)
    if (-not $Run.App.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $Run.App); throw 'portable app did not exit on WM_CLOSE' }
    Assert-True ($Run.App.ExitCode -eq 0) ("portable app exit code " + $Run.App.ExitCode)
}
function Write-ProbeScript([string]$Path, [string]$Marker) {
    # 探针脚本把标记写到自己所在目录（%~dp0），与工作目录无关。
    [IO.File]::WriteAllText($Path, "@echo off`r`necho $Marker > `"%~dp0probe-rel-$Marker.txt`"`r`n", [Text.Encoding]::ASCII)
}

Stop-QnTestProcesses
Stop-QnTestProcesses
Start-Sleep -Milliseconds 500

foreach ($dir in @($rootA, $rootB)) {
    if (Test-Path -LiteralPath $dir) { Remove-QnSandboxItem $dir -Recurse -Force }
}

try {
    # a) 预检：便携包存在；记录哈希。
    Assert-True (Test-Path -LiteralPath $zip) 'portable zip missing; run installer/build-portable.ps1 first'
    $sha = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
    Record 'preflight' ('zip_sha256=' + $sha.Substring(0, 16) + '...')

    # b) 解压即用：解压到根 A（E: 盘）。
    Expand-Archive -LiteralPath $zip -DestinationPath $rootA -Force
    foreach ($f in @('quicknest.exe', 'QuickNest.ico', 'README.md', 'QuickNest.portable')) {
        Assert-True (Test-Path -LiteralPath (Join-Path $rootA $f)) "package file missing after extract: $f"
    }
    Record 'extracted' ('rootA=' + $rootA)

    # c) 首次启动：数据目录必须落在 <根A>\data，而不是 %APPDATA%。
    $run = Start-Portable (Join-Path $rootA 'quicknest.exe')
    $configA = Join-Path $rootA 'data\config.json'
    Wait-True { Test-Path -LiteralPath $configA } 15 'portable data config not created under <rootA>\data'
    Assert-RoamingUnchanged
    Stop-Portable $run
    Record 'first_launch_data_dir' 'data\config.json created inside the folder; %APPDATA% untouched'

    # d) 预置相对路径项目：cmd 探针脚本放在根 A，项目目标为相对路径。
    for ($index = 1; $index -le 3; $index++) {
        Write-ProbeScript (Join-Path $rootA "probe-$index.cmd") "$index"
    }
    $probeItems = @()
    for ($index = 1; $index -le 3; $index++) {
        $probeItems += @{
            id = "i$index"; name = "Relative $index"; item_type = 'command'; icon = ''; window_state = 'normal'; note = ''
            last_used = 0; launch_count = 0; favorite = $false
            launch = @{
                path = "probe-$index.cmd"; args = @(); workdir = ''
                run_as_admin = $false; aumid = ''; msi_advertised = $false
                confirm_on_first_run = $false; delay_seconds = 0; execution_mode = 'cmd'
            }
        }
    }
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none'; sort_mode = 'manual' }
        groups = @(@{ id='g1'; name='Portable'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=$probeItems }) })
    }
    [IO.File]::WriteAllText($configA, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded_relatives' 'three relative-path command items (probe scripts use %~dp0)'

    # e) 根 A 内单击启动：相对路径按根 A 解析（探针落在根 A）。
    $run = Start-Portable (Join-Path $rootA 'quicknest.exe')
    Invoke-ClickAt ($run.Left + 220) ($run.Top + 150) 1500
    Save-P005Screenshot -Path (Join-Path $results 'p4-02-diag.png') -WindowHwnd $run.Hwnd
    $probeA1 = Join-Path $rootA 'probe-rel-1.txt'
    Wait-True { Test-Path -LiteralPath $probeA1 } 10 'relative item did not launch from root A'
    Assert-True ((Get-Content -LiteralPath $probeA1 -Raw).Contains('1')) 'probe-rel-1 content mismatch'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $rootA 'probe-rel-2.txt'))) 'unpicked item started'
    Save-P005Screenshot -Path (Join-Path $results 'p4-02-root-a.png') -WindowHwnd $run.Hwnd
    Stop-Portable $run
    Record 'relative_launch_root_a' 'click launched probe-1.cmd; probe written inside root A'

    # f) 关闭后把整个文件夹移动到根 B（隔离目录 B）。
    # A copied probe is not evidence of a launch at the destination.
    Remove-QnSandboxItem -LiteralPath $probeA1 -Force
    Copy-Item -LiteralPath $rootA -Destination $rootB -Recurse -Force
    $moveNonce = [Guid]::NewGuid().ToString('N')
    Write-ProbeScript (Join-Path $rootB 'probe-1.cmd') $moveNonce
    $freshMovedProbe = Join-Path $rootB ("probe-rel-$moveNonce.txt")
    Assert-True (-not (Test-Path -LiteralPath $freshMovedProbe)) 'fresh move probe already exists'
    Remove-QnSandboxItem -LiteralPath $rootA -Recurse -Force
    Assert-True (Test-Path -LiteralPath (Join-Path $rootB 'data\config.json')) 'data did not move with the folder'
    Assert-True (Test-Path -LiteralPath (Join-Path $rootB 'QuickNest.portable')) 'marker did not move with the folder'
    Record 'moved_to_root_b' ('rootB=' + $rootB + ' (sandbox directory relocation)')

    # g) 根 B 重启：数据随文件夹（配置仍含 3 个项目），相对路径按新根解析。
    $run = Start-Portable (Join-Path $rootB 'quicknest.exe')
    $configB = Join-Path $rootB 'data\config.json'
    $cfg = Get-Content -LiteralPath $configB -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert-True (@($cfg.groups[0].categories[0].items).Count -eq 3) 'items lost after the move'
    Invoke-ClickAt ($run.Left + 220) ($run.Top + 150) 1500
    $probeB1 = $freshMovedProbe
    Wait-True { Test-Path -LiteralPath $probeB1 } 10 'relative item did not launch from root B'
    Assert-True ((Get-Content -LiteralPath $probeB1 -Raw).Trim() -eq $moveNonce) 'moved launch nonce mismatch'
    Assert-True (-not (Test-Path -LiteralPath (Join-Path $rootB 'probe-rel-2.txt'))) 'unpicked item started'
    Assert-True (-not (Test-Path -LiteralPath $probeA1)) 'launch wrote to the OLD root (resolution not portable)'
    Save-P005Screenshot -Path (Join-Path $results 'p4-02-root-b.png') -WindowHwnd $run.Hwnd
    Stop-Portable $run
    Record 'relative_launch_root_b' 'config survived the move; relative resolution follows the new root'

    # h) 全程漫游目录不受影响。
    Assert-RoamingUnchanged
    Record 'roaming_untouched' '%APPDATA%\QuickNest never used by the portable copy'

    # i) 清理。
    Remove-QnSandboxItem -LiteralPath $rootB -Recurse -Force
    Stop-QnTestProcesses
    Record 'cleanup' 'roots removed'
    Record 'completed' 'all P4-02 portable phases passed'
    Write-Host 'All P4-02 portable phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    Stop-QnTestProcesses
    if (Test-Path -LiteralPath $rootA) { Remove-QnSandboxItem $rootA -Recurse -Force -ErrorAction SilentlyContinue }
    if (Test-Path -LiteralPath $rootB) { Remove-QnSandboxItem $rootB -Recurse -Force -ErrorAction SilentlyContinue }

}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
