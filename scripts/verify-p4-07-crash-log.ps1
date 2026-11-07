#requires -Version 5.1
# P4-07 acceptance (baseline 24 章 稳定性 / 29 章 P4-07 崩溃日志).
#
# 安装版场景实测：%APPDATA%\QuickNest\data\crash-log.txt。应用带
# --crash-test 参数启动 → 内部触发真实 panic（panic hook 记录后进程退出）
# → 验证崩溃日志存在、包含 panic 消息与位置、且时间戳递增（追加语义）。
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

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p4-07-crash-log.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$exePath = Join-Path $root 'target\release\quicknest.exe'
$crashLog = Join-Path $env:QUICKNEST_DATA_DIR 'crash-log.txt'

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

Stop-QnTestProcesses
Start-Sleep -Milliseconds 500
# 基线：每次验收从空日志开始。
Remove-QnSandboxItem -LiteralPath $crashLog -ErrorAction SilentlyContinue

try {
    # a) 预检：exe 存在；崩溃日志初始不存在。
    Assert-True (Test-Path -LiteralPath $exePath) 'release exe missing'
    Record 'preflight' 'release exe present; crash log cleared'

    # b) 触发真实崩溃（--crash-test：panic hook 记录后退出，非零码）。
    $p = Start-QnOwnedProcess -FilePath $exePath -ArgumentList '--crash-test' -PassThru -WindowStyle Hidden
    Wait-True { $p.HasExited } 30 'crash-test process did not exit'
    Assert-True ($p.ExitCode -ne 0) ("crash-test exit code should be non-zero: " + $p.ExitCode)
    Record 'crash_triggered' ('exit=' + $p.ExitCode + ' (hook wrote the log, then the process aborted)')

    # c) 崩溃日志落盘且内容完整。
    Wait-True { Test-Path -LiteralPath $crashLog } 15 'crash log missing after the crash'
    $content = Get-Content -LiteralPath $crashLog -Raw -Encoding UTF8
    Assert-True ($content.Contains('QuickNest crash-test panic')) 'panic message missing from the log'
    Assert-True ($content.Contains('message: QuickNest crash-test panic')) 'message line missing'
    Assert-True ($content -match 'panic in thread .+ at src') 'location line missing'
    Assert-True ($content -match '\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC') 'timestamp missing'
    Record 'crash_log' 'log exists with message, location and timestamp'

    # d) 追加语义：二次崩溃在日志末尾追加（不覆盖）。
    $first = (Get-Content -LiteralPath $crashLog).Count
    $p2 = Start-QnOwnedProcess -FilePath $exePath -ArgumentList '--crash-test' -PassThru -WindowStyle Hidden
    Wait-True { $p2.HasExited } 30 'second crash-test did not exit'
    Wait-True { (Get-Content -LiteralPath $crashLog).Count -gt $first } 15 'second crash did not append'
    $content2 = Get-Content -LiteralPath $crashLog -Raw -Encoding UTF8
    Assert-True (([regex]::Matches($content2, 'QuickNest crash-test panic')).Count -ge 2) 'second entry missing'
    Record 'append_semantics' ('entries grew from ' + $first + ' lines; two crash entries present')

    # e) 清理崩溃日志（恢复无残留状态）。
    Remove-QnSandboxItem -LiteralPath $crashLog -Force
    Record 'cleanup' 'crash log removed'
    Record 'completed' 'all P4-07 crash-log phases passed'
    Write-Host 'All P4-07 crash-log phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    Stop-QnTestProcesses
    Remove-QnSandboxItem -LiteralPath $crashLog -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
