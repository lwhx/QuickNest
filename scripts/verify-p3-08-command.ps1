#requires -Version 5.1
# P3-08 acceptance (baseline 22 自定义命令管理 / chapter 8 命令执行安全).
#
# Seeded command items (manual grid order c1..c4, all unverified):
#   c1 direct     : cmd.exe /c echo direct-ok >> probe1.txt  (workdir=probeDir)
#   c2 cmd        : run.cmd   (echo cmd-ok >> probe2.txt)
#   c3 powershell : run.ps1   (Add-Content probe3.txt 'ps-ok')
#   c4 direct     : cmd.exe /c echo batch-skip >> probe4.txt  (stays unverified)
#
# Flow: click c1 -> first-run confirm dialog -> 确认执行 -> probe1 written and
# the unverified flag clears; clicking again starts immediately (no dialog).
# c2 is cancelled once (probe2 must stay absent, flag stays true), then
# confirmed. c3 exercises the PowerShell mode. A batch with verified c1 and
# unverified c4 starts c1 and SKIPS c4 (probe4 absent, flag true).
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
using System;using System.Runtime.InteropServices;using System.Text;
public static class P38Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h, out Rect r);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 public struct Point{public int X,Y;}
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-08-command.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Invoke-ClickAt([int]$X, [int]$Y, [int]$DelayMs = 450) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P005Desktop]::MouseButton($true)
    Start-Sleep -Milliseconds 70
    [P005Desktop]::MouseButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 600) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 150
    [P005Desktop]::RightButton($true)
    Start-Sleep -Milliseconds 90
    [P005Desktop]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}
function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P38Window+EnumProc]{
            param($h, $l)
            if (-not [P38Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P38Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P38Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
# Opens the card menu and clicks the Nth row (posted arrows, 1-based).
function Invoke-CardMenuRow([int]$X, [int]$Y, [int]$Row) {
    Focus-P117Window $script:hwnd
    Start-Sleep -Milliseconds 250
    Invoke-RightClickAt $X $Y
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'card menu did not open'
    $hmenu = [P38Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P38Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 13) "card menu items: $count (expected 13)"
    for ($index = 0; $index -lt $Row; $index++) {
        [P005Keyboard]::Key(0x28)
        Start-Sleep -Milliseconds 60
    }
    [P005Keyboard]::Key(0x0D)
    Start-Sleep -Milliseconds 800
}
# Single click on a command card: the first-run dialog must appear (or, when
# $ExpectDialog is false, the launch must run without one).
function Invoke-ConfirmDialog([bool]$ExpectDialog, [bool]$Confirm, [string]$Screenshot) {
    if ($ExpectDialog) {
        if ($Screenshot) {
            Save-P005Screenshot -Path $Screenshot -WindowHwnd $script:hwnd
        }
        if ($Confirm) {
            Invoke-ClickAt ($script:left + [int](($script:width - 470) / 2) + 470 - 16 - 40) ($script:top + [int](($script:height - 270) / 2) + 270 - 16 - 15) 700
        } else {
            Invoke-ClickAt ($script:left + [int](($script:width - 470) / 2) + 470 - 16 - 80 - 10 - 26) ($script:top + [int](($script:height - 270) / 2) + 270 - 16 - 15) 700
        }
    } else {
        Start-Sleep -Milliseconds 1200
    }
}
function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 25; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json } catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}
function Wait-Probe([string]$Path, [int]$TimeoutSeconds) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path) { return $true }
        Start-Sleep -Milliseconds 200
    }
    return $false
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$probeDir = Join-Path $results 'probes'
if (Test-Path $probeDir) { Remove-QnSandboxItem $probeDir -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDir)
$runCmd = Join-Path $probeDir 'run.cmd'
[IO.File]::WriteAllText($runCmd, "@echo off`r`necho cmd-ok >> probe2.txt`r`n", [Text.Encoding]::ASCII)
$runPs1 = Join-Path $probeDir 'run.ps1'
[IO.File]::WriteAllText($runPs1, "Add-Content -Path probe3.txt -Value 'ps-ok'`r`n", (New-Object Text.UTF8Encoding($false)))
$app = $null

Stop-QnTestProcesses
Stop-QnTestProcesses
Start-Sleep -Milliseconds 500
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

try {
    # a) Seed four unverified command items in manual order c1..c4.
    $items = @(
        @{ id='c1'; name='DirectCmd'; item_type='command'; icon=''; window_state='normal'; note=''; last_used=0; launch_count=0; favorite=$false
           launch=@{ path=$env:ComSpec; args=@('/c','echo','direct-ok','>>','probe1.txt'); workdir=$probeDir; run_as_admin=$false; aumid=''; msi_advertised=$false; confirm_on_first_run=$true; delay_seconds=0; execution_mode='direct' } },
        @{ id='c2'; name='CmdScript'; item_type='command'; icon=''; window_state='normal'; note=''; last_used=0; launch_count=0; favorite=$false
           launch=@{ path=$runCmd; args=@(); workdir=$probeDir; run_as_admin=$false; aumid=''; msi_advertised=$false; confirm_on_first_run=$true; delay_seconds=0; execution_mode='cmd' } },
        @{ id='c3'; name='PsScript'; item_type='command'; icon=''; window_state='normal'; note=''; last_used=0; launch_count=0; favorite=$false
           launch=@{ path=$runPs1; args=@(); workdir=$probeDir; run_as_admin=$false; aumid=''; msi_advertised=$false; confirm_on_first_run=$true; delay_seconds=0; execution_mode='powershell' } },
        @{ id='c4'; name='BatchSkip'; item_type='command'; icon=''; window_state='normal'; note=''; last_used=0; launch_count=0; favorite=$false
           launch=@{ path=$env:ComSpec; args=@('/c','echo','batch-skip','>>','probe4.txt'); workdir=$probeDir; run_as_admin=$false; aumid=''; msi_advertised=$false; confirm_on_first_run=$true; delay_seconds=0; execution_mode='direct' } }
    )
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{
            pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none'
            sort_mode = 'manual'
        }
        groups = @(@{ id='g1'; name='Cmd'; icon=''; color=''; last_category_id='c10'; categories=@(
            @{ id='c10'; name='Cat'; icon=''; items=$items }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded' '4 unverified command items: c1 direct, c2 cmd, c3 powershell, c4 direct (batch skip)'

    $app = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        $window = $null
        while ($null -eq $window -and [DateTime]::UtcNow -lt $deadline) {
            Start-Sleep -Milliseconds 200
            $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
        }
        if (-not $window) { throw 'QuickNest window not found' }
        $hwnd = [long]$window.hwnd
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 900
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
        $script:left = $window.rect.Left
        $script:top = $window.rect.Top
        $script:width = $window.rect.Right - $window.rect.Left
        $script:height = $window.rect.Bottom - $window.rect.Top
        $script:hwnd = $hwnd

        # b) c1 (direct): the first-run dialog shows, confirming runs the
        #    command, writes the probe and clears the unverified flag.
        $cardBaseX = $script:left + 208
        $cardY = $script:top + 150
        Invoke-ClickAt $cardBaseX $cardY 800
        Invoke-ConfirmDialog $true $true (Join-Path $results 'p3-08-confirm.png')
        $probe1 = Join-Path $probeDir 'probe1.txt'
        Assert-True (Wait-Probe $probe1 10) 'probe1 missing after confirmed direct run'
        Assert-True ((Get-Content $probe1 -Raw).Contains('direct-ok')) 'probe1 content wrong'
        $config = Read-Config $configPath
        $c1 = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'c1' })[0]
        Assert-True ($c1.launch.confirm_on_first_run -eq $false) 'c1 flag not cleared'
        Record 'direct_confirmed' 'c1 direct: dialog confirmed, probe1 written, flag cleared'

        # c) The verified item starts immediately: no dialog, second probe line.
        Invoke-ClickAt $cardBaseX $cardY 100
        Invoke-ConfirmDialog $false $false $null
        $lines1 = (Get-Content $probe1).Count
        Assert-True ($lines1 -ge 2) "verified c1 did not relaunch (probe1 lines: $lines1)"
        Record 'direct_verified_no_dialog' "c1 relaunched without the dialog (probe1 lines: $lines1)"

        # d) c2 (cmd mode): cancelling keeps the probe absent and the flag
        #    true; confirming then runs the script via cmd.exe /d /s /c.
        Invoke-ClickAt ($cardBaseX + 96) $cardY 800
        Invoke-ConfirmDialog $true $false $null
        Start-Sleep -Milliseconds 500
        Assert-True (-not (Test-Path (Join-Path $probeDir 'probe2.txt'))) 'c2 ran after cancel'
        $config = Read-Config $configPath
        $c2 = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'c2' })[0]
        Assert-True ($c2.launch.confirm_on_first_run -eq $true) 'c2 flag cleared by cancel'
        Record 'cmd_cancelled' 'c2 cancel: probe2 absent, flag still true'
        Invoke-ClickAt ($cardBaseX + 96) $cardY 800
        Invoke-ConfirmDialog $true $true $null
        $probe2 = Join-Path $probeDir 'probe2.txt'
        Assert-True (Wait-Probe $probe2 10) 'probe2 missing after confirmed cmd run'
        Assert-True ((Get-Content $probe2 -Raw).Contains('cmd-ok')) 'probe2 content wrong'
        $config = Read-Config $configPath
        $c2 = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'c2' })[0]
        Assert-True ($c2.launch.confirm_on_first_run -eq $false) 'c2 flag not cleared'
        Assert-True ($c2.launch.execution_mode -eq 'cmd') "c2 mode: $($c2.launch.execution_mode)"
        Record 'cmd_confirmed' 'c2 cmd mode: confirmed run wrote probe2, mode persisted'

        # e) c3 (powershell mode): confirmed run executes the script through
        #    powershell.exe -NoProfile -File.
        Invoke-ClickAt ($cardBaseX + 192) $cardY 800
        Invoke-ConfirmDialog $true $true $null
        $probe3 = Join-Path $probeDir 'probe3.txt'
        Assert-True (Wait-Probe $probe3 15) 'probe3 missing after confirmed powershell run'
        Assert-True ((Get-Content $probe3 -Raw).Contains('ps-ok')) 'probe3 content wrong'
        $config = Read-Config $configPath
        $c3 = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'c3' })[0]
        Assert-True ($c3.launch.confirm_on_first_run -eq $false) 'c3 flag not cleared'
        Assert-True ($c3.launch.execution_mode -eq 'powershell') "c3 mode: $($c3.launch.execution_mode)"
        Record 'powershell_confirmed' 'c3 powershell mode: confirmed run wrote probe3'

        # f) Batch skip: verified c1 + unverified c4 in one batch — c1 runs
        #    (third line), c4 is skipped (probe4 absent, flag stays true).
        Invoke-CardMenuRow ($cardBaseX + 288) $cardY 9      # c4 加入批量选择
        Invoke-CardMenuRow $cardBaseX $cardY 9              # c1 加入批量选择
        Invoke-CardMenuRow $cardBaseX $cardY 10             # 批量启动所选
        Start-Sleep -Milliseconds 2500
        $lines1b = (Get-Content $probe1).Count
        Assert-True ($lines1b -ge $lines1 + 1) "batch did not start c1 (probe1 lines: $lines1b)"
        Assert-True (-not (Test-Path (Join-Path $probeDir 'probe4.txt'))) 'unverified c4 ran inside the batch'
        $config = Read-Config $configPath
        $c4 = @($config.groups[0].categories[0].items | Where-Object { $_.id -eq 'c4' })[0]
        Assert-True ($c4.launch.confirm_on_first_run -eq $true) 'c4 flag changed by the batch'
        Save-P005Screenshot -Path (Join-Path $results 'p3-08-final.png') -WindowHwnd $hwnd
        Record 'batch_skips_unverified' "c1 started (probe1 lines: $lines1b); c4 skipped, flag true"

        # g) Persisted state survives an orderly exit.
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "exit code $($app.ExitCode)"
        Record 'persisted' ("exit=0 c1/c2/c3 verified, c4 unverified")
        Record 'completed' 'all P3-08 command phases passed'
        Write-Host 'All P3-08 command phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
