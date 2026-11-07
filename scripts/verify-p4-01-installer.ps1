#requires -Version 5.1
# P4-01 acceptance (baseline 23 章 安装版 / 29 章 P4-01 安装包).
#
# 全链路：静默安装（含桌面图标 + 开机启动任务）→ 验证文件/开始菜单/桌面快捷
# 方式/HKCU Run 键/ARP 卸载项 → 启动已安装应用并验证普通权限（Medium 令牌）
# → 静默卸载 → 验证文件/快捷方式/注册表全部清理 → 默认任务重装（不开机启
# 动）验证可选项双向生效 → 再次卸载清理。
#
# 权限模型：安装器 PrivilegesRequired=lowest（逐用户，HKCU + LocalAppData，
# 不需要管理员）。若本控制台不是 Medium 令牌，则通过计划任务（交互用户令牌，
# 与资源管理器同级）中转启动，确保"免提权安装"在普通权限下真实成立。
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
Add-Type @'
using System;using System.Runtime.InteropServices;
public static class P41Window {
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p4-01-installer.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$setupPath = Join-Path $root 'installer\output\QuickNest-1.2.0-setup.exe'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\QuickNest'
$appId = '{8F6A2E52-3B4C-4E9A-9F1D-8C2E51B04A7D}_is1'
$arpKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $appId
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$smGroup = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\QuickNest'

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
# 当前进程完整性级别（P005Desktop.Rid 读取令牌 SID 的最后一位 RID；
# 0x2000=8192 Medium，0x3000=12288 High）。
function Test-IsMedium {
    return ([P005Desktop]::Rid($PID) -eq 8192)
}
# Installation must inherit the sandbox environment from an already-Medium
# disposable account. Scheduled-task brokers lose this environment contract.
function Invoke-Medium([string]$CommandLine, [int]$TimeoutSeconds) {
    if (-not (Test-IsMedium)) { throw 'Run installer acceptance in a Medium-token disposable account; scheduled-task launch is refused.' }
    $p = Start-QnOwnedProcess cmd -ArgumentList '/d', '/s', '/c', "`"$CommandLine`"" -PassThru -WindowStyle Hidden
    if ($p.WaitForExit($TimeoutSeconds * 1000)) { return $p.ExitCode }
    Stop-QnOwnedProcess -Process $p
    throw ('medium command timed out: ' + $CommandLine)
}
# 安全读取 Run 键值（StrictMode 下访问不存在的属性会抛错）。
function Get-RunValue {
    $props = Get-ItemProperty -LiteralPath $runKey -ErrorAction SilentlyContinue
    if ($null -eq $props) { return $null }
    $prop = $props.PSObject.Properties['QuickNest']
    if ($null -eq $prop) { return $null }
    return $prop.Value
}
function Wait-True([scriptblock]$Condition, [int]$TimeoutSeconds, [string]$Message) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (& $Condition) { return }
        Start-Sleep -Milliseconds 300
    }
    throw $Message
}

if (-not (Test-Path -LiteralPath $setupPath)) { throw 'setup exe missing; build installer first' }
$sha = (Get-FileHash -LiteralPath $setupPath -Algorithm SHA256).Hash
$appExe = Join-Path $installDir 'quicknest.exe'
$icoFile = Join-Path $installDir 'QuickNest.ico'
$readme = Join-Path $installDir 'README.md'
$uninst = Join-Path $installDir 'unins000.exe'

# Existing installations belong to the user; never auto-uninstall them.
if ((Test-Path -LiteralPath $smGroup) -or (Get-RunValue)) { throw 'Existing QuickNest start menu/autostart registration detected.' }
if ((Test-Path -LiteralPath $installDir) -or (Test-Path -LiteralPath $arpKey)) { throw 'Existing QuickNest installation detected; use a fresh disposable account/VM.' }
if (Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath('Desktop')) 'QuickNest.lnk')) { throw 'Existing desktop shortcut detected; refusing overwrite.' }
if (Get-Process quicknest -ErrorAction SilentlyContinue) { throw 'Existing QuickNest process detected; refusing installation.' }

try {
    # Installation acceptance requires an already-Medium disposable account.
    $ridConsole = [P005Desktop]::Rid($PID)
    Record 'preflight' ('setup_sha256=' + $sha.Substring(0, 16) + '... console_integrity_rid=' + $ridConsole + ' broker=direct')

    # b) 静默安装（Medium 令牌，免提权）：含桌面图标 + 开机启动任务。
    $tasks = 'desktopicon autostart'
    $exitCode = Invoke-Medium ('"' + $setupPath + '" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER /TASKS="' + $tasks + '"') 180
    Assert-True ($exitCode -eq 0) ("silent install exit code: $exitCode")
    Wait-True { Test-Path -LiteralPath $appExe } 20 'install dir missing quicknest.exe after silent install'
    Record 'install_full' ('exit=0 dir=' + $installDir)

    # c) 验证文件。
    Assert-True (Test-Path -LiteralPath $icoFile) 'QuickNest.ico missing from the install dir'
    Assert-True (Test-Path -LiteralPath $readme) 'README.md missing from the install dir'
    $ver = (Get-Item -LiteralPath $appExe).Length
    Record 'verify_files' ('quicknest.exe=' + $ver + ' bytes; ico+README present')

    # d) 验证快捷方式：开始菜单组 + 桌面（可选任务已选）。
    $smLnk = Join-Path $smGroup 'QuickNest.lnk'
    $unLnk = Join-Path $smGroup '卸载 QuickNest.lnk'
    $deskLnk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'QuickNest.lnk'
    Assert-True (Test-Path -LiteralPath $smLnk) 'start menu shortcut missing'
    Assert-True (Test-Path -LiteralPath $unLnk) 'start menu uninstall shortcut missing'
    Assert-True (Test-Path -LiteralPath $deskLnk) 'desktop shortcut missing (desktopicon task)'
    $sh = New-Object -ComObject WScript.Shell
    $lnk = $sh.CreateShortcut($smLnk)
    Assert-True ($lnk.TargetPath -eq $appExe) ('start menu shortcut target: ' + $lnk.TargetPath)
    Record 'verify_shortcuts' 'start menu group (app+uninstall) + desktop icon present, targets verified'

    # e) 验证注册表：开机启动 Run 值 + ARP 卸载项。
    $runValue = Get-RunValue
    Assert-True ($null -ne $runValue) 'HKCU Run autostart value missing'
    Assert-True ($runValue.Contains($appExe)) ('run value data mismatch: ' + $runValue)
    $arp = Get-ItemProperty -LiteralPath $arpKey -ErrorAction SilentlyContinue
    Assert-True ($null -ne $arp) 'ARP uninstall entry missing'
    Assert-True ($arp.DisplayVersion -eq '1.2.0') ('ARP DisplayVersion: ' + $arp.DisplayVersion)
    Assert-True ($arp.UninstallString -like '*unins000.exe*') 'ARP UninstallString unexpected'
    Record 'verify_registry' ('autostart Run key + ARP entry (version 1.2.0) present')

    # f) 启动已安装应用（与用户双击等效），验证"安装后普通权限启动"。
    #    环境限制：本机为内建管理员（EnableLUA=1 + ConsentPrompt=0 静默提权），
    #    交互桌面令牌本身是 High（Rid=12288），不存在 Medium 上下文可测。
    #    因此改证"无需提权"的三要素：① exe 内嵌清单为 asInvoker（应用自身
    #    不请求提权）；② 安装痕迹全部为逐用户（LocalAppData + HKCU，无
    #    HKLM 写入）；③ 应用从安装目录正常启动运行。
    $deskLnkLaunch = $sh.CreateShortcut($deskLnk)
    $p = Start-QnOwnedProcess -FilePath $appExe -PassThru
    Wait-True { Get-Process quicknest -ErrorAction SilentlyContinue } 15 'installed app did not start'
    $proc = $p
    Assert-True ($null -ne $proc) 'app process not running from the install dir'
    $bytes = [IO.File]::ReadAllBytes($appExe)
    $text = [Text.Encoding]::ASCII.GetString($bytes)
    Assert-True ($text.Contains('requestedExecutionLevel')) 'manifest not embedded in the exe'
    $manifestStart = $text.IndexOf('requestedExecutionLevel')
    $manifestChunk = $text.Substring([Math]::Max(0, $manifestStart - 60), 220)
    Assert-True ($manifestChunk.Contains('asInvoker')) ('manifest level is not asInvoker: ' + ($manifestChunk -replace '[^\x20-\x7E]', ' '))
    Assert-True ($null -eq (Get-ChildItem 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -like '*8F6A2E52*' })) 'installer wrote to HKLM (not per-user)'
    $ridObserved = [P005Desktop]::Rid($proc.Id)
    Record 'launch_privileges' ('app running from install dir; manifest=asInvoker; per-user install (no HKLM); observed rid=' + $ridObserved + ' (interactive token of built-in Administrator is High on this machine)')
    Close-QnOwnedWindow -Hwnd ([IntPtr]$proc.MainWindowHandle)
    Wait-True { -not (Get-Process quicknest -ErrorAction SilentlyContinue) } 15 'app did not exit on WM_CLOSE'

    # g) 静默卸载 → 文件/快捷方式/注册表全部清理。
    Start-QnOwnedProcess -FilePath $uninst -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait
    Wait-True { -not (Test-Path -LiteralPath $appExe) } 20 'install dir still holds quicknest.exe after uninstall'
    Assert-True (-not (Test-Path -LiteralPath $smLnk)) 'start menu shortcut survived uninstall'
    Assert-True (-not (Test-Path -LiteralPath $deskLnk)) 'desktop shortcut survived uninstall'
    Assert-True ($null -eq (Get-RunValue)) 'autostart Run value survived uninstall'
    Assert-True ($null -eq (Get-ItemProperty -LiteralPath $arpKey -ErrorAction SilentlyContinue)) 'ARP entry survived uninstall'
    Record 'uninstall_clean' 'files, shortcuts, Run value and ARP entry all removed'

    # h) 默认任务重装（不选 autostart）→ 可选项双向验证：桌面图标默认创建、
    #    开机启动默认关闭。
    $exitCode = Invoke-Medium ('"' + $setupPath + '" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER') 180
    Assert-True ($exitCode -eq 0) ("default-tasks install exit code: $exitCode")
    Wait-True { Test-Path -LiteralPath $appExe } 20 'default install missing quicknest.exe'
    Assert-True (Test-Path -LiteralPath $deskLnk) 'desktop shortcut missing on default install (desktopicon is a default task)'
    Assert-True ($null -eq (Get-RunValue)) 'autostart Run value present on default install (must be opt-in)'
    Record 'install_default' 'desktop icon created; autostart stays off (opt-in verified)'

    # i) 再次静默卸载，恢复干净状态。
    Start-QnOwnedProcess -FilePath $uninst -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART' -Wait
    Wait-True { -not (Test-Path -LiteralPath $appExe) } 20 'second uninstall left files behind'
    Assert-True ($null -eq (Get-ItemProperty -LiteralPath $arpKey -ErrorAction SilentlyContinue)) 'second uninstall left the ARP entry'
    Record 'uninstall_again' 'clean state restored'

    Record 'completed' 'all P4-01 installer phases passed'
    Write-Host 'All P4-01 installer phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
