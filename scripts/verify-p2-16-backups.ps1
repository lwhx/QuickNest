#requires -Version 5.1
# P2-16 acceptance (baseline 15.7/15.8/17.4): historical backups.
#
# The title-bar settings menu opens the backup dialog, which lists every
# backup whose content parses (newest first), backs the configuration up on
# demand and restores the selected backup. The restore keeps the replaced
# configuration as a backup, so it can itself be undone, and the retention
# window stays at the last 10 valid backups.
throw 'BLOCKED: obsolete flat settings-menu fixture; current UI uses a settings panel and right-click submenus. Core control/callback regression coverage is maintained in src/ui_regression_tests.rs and tests/ui_layout.rs; this script cannot certify desktop behavior until migrated.'
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
public static class P216Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h, out Rect r);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p2-16-backups.tsv'
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
function Wait-MenuWindow([int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $menus = New-Object System.Collections.ArrayList
        $cb = [P216Window+EnumProc]{
            param($h, $l)
            if (-not [P216Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P216Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P216Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
# Opens the title-bar settings menu (18 rows) and clicks the 1-based row.
function Invoke-SettingsMenu([long]$Hwnd, [int]$ItemIndex) {
    Invoke-ClickAt ($script:left + $script:width - 60) ($script:top + 22) 500
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'settings menu did not open'
    $rect = New-Object P216Window+Rect
    [void][P216Window]::GetWindowRect([IntPtr]$menu, [ref]$rect)
    $hmenu = [P216Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P216Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -ge 18) "settings menu items: $count (expected at least 18)"
    $height = $rect.Bottom - $rect.Top
    $itemHeight = $height / $count
    Invoke-ClickAt ($rect.Left + 50) ($rect.Top + [int]($itemHeight * ($ItemIndex - 0.5))) 800
}
function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        if (Test-Path -LiteralPath $Path) {
            try { return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json } catch { }
        }
        Start-Sleep -Milliseconds 100
    }
    throw 'config.json missing'
}
function Get-GroupName([string]$Path) {
    $config = Read-Config $Path
    return [string]$config.groups[0].name
}
function Get-Backups([string]$Directory) {
    return @(Get-ChildItem -LiteralPath $Directory -Filter 'config.backup.*.json' -File | Sort-Object Name -Descending)
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$script:left = 60
$script:top = 80
$script:width = 760
$app = $null

Stop-QnTestProcesses
Start-Sleep -Milliseconds 400
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
Get-ChildItem -LiteralPath $configDir -Filter 'config.corrupted.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

function Write-Library([string]$GroupName, [string]$Path) {
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none' }
        groups = @(@{ id='g1'; name=$GroupName; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=@() }) })
    }
    [IO.File]::WriteAllText($Path, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
}

try {
    # a) Seed the live config plus two historical backups: one valid (newest
    #    by name so it is listed first) and one that does not parse.
    Write-Library 'Current' $configPath
    $validBackup = Join-Path $configDir 'config.backup.20261231-120000-000.json'
    $brokenBackup = Join-Path $configDir 'config.backup.20270101-000000-000.json'
    Write-Library 'Restored' $validBackup
    [IO.File]::WriteAllText($brokenBackup, 'not json at all', (New-Object Text.UTF8Encoding($false)))
    Record 'seeded' ("live=Current valid=2026-12-31 broken=2027-01-01")

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
        [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), $script:left, $script:top, 0, 0, 0x0001)
        Start-Sleep -Milliseconds 400
        Focus-P117Window $hwnd
        Start-Sleep -Milliseconds 700
        $window = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $app.Id -and $_.title -like 'QuickNest*' })[0]
        $script:left = $window.rect.Left
        $script:top = $window.rect.Top
        $script:width = $window.rect.Right - $window.rect.Left
        Save-P005Screenshot -Path (Join-Path $results 'p2-16-before.png') -WindowHwnd $hwnd

        # b) The settings menu opens the backup dialog (18th row).
        Invoke-SettingsMenu $hwnd 18
        Save-P005Screenshot -Path (Join-Path $results 'p2-16-dialog.png') -WindowHwnd $hwnd
        Record 'dialog_opened' 'p2-16-dialog.png'

        # c) The first row is the newest valid backup (the broken 2027 entry is
        #    not offered) and is selected by default; restore it.
        $before = Get-Backups $configDir
        Invoke-ClickAt ($script:left + 571) ($script:top + 421) 1200
        $group = Get-GroupName $configPath
        Assert-True ($group -eq 'Restored') "restored group: $group (expected Restored; the broken 2027 backup must not be offered)"
        Save-P005Screenshot -Path (Join-Path $results 'p2-16-restored.png') -WindowHwnd $hwnd
        Record 'restored' ("group=$group")

        # d) The replaced configuration is kept as a backup (the restore is
        #    reversible) and the retention window stays at 10.
        $after = Get-Backups $configDir
        $kept = $null
        foreach ($backup in $after) {
            $content = Get-Content -LiteralPath $backup.FullName -Raw -Encoding UTF8
            if ($content -match '"name":\s*"Current"') { $kept = $backup; break }
        }
        Assert-True ($null -ne $kept) 'the replaced configuration was not kept as a backup'
        Assert-True ($after.Count -le 10) "backups after restore: $($after.Count) (retention is 10)"
        Record 'restore_kept_previous' ("backup=$($kept.Name) count=$($after.Count)")

        # e) 立即备份 creates another backup of the current configuration.
        $beforeNow = (Get-Backups $configDir).Count
        Invoke-ClickAt ($script:left + 199) ($script:top + 421) 1200
        $afterNow = Get-Backups $configDir
        Assert-True ($afterNow.Count -gt $beforeNow) "immediate backup did not add a file ($beforeNow -> $($afterNow.Count))"
        # The seeded backup carries a future timestamp, so pick the file that
        # was written last rather than the first name.
        $newest = @(Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -File | Sort-Object LastWriteTime -Descending)[0]
        $newestContent = Get-Content -LiteralPath $newest.FullName -Raw -Encoding UTF8
        Assert-True ($newestContent -match '"name":\s*"Restored"') "newest backup does not hold the current config: $($newest.Name)"
        Record 'backup_now' ("file=$($newest.Name) count=$($afterNow.Count)")

        # f) 取消 closes the dialog.
        Invoke-ClickAt ($script:left + 478) ($script:top + 421) 600
        Save-P005Screenshot -Path (Join-Path $results 'p2-16-closed.png') -WindowHwnd $hwnd
        Record 'dialog_closed' 'p2-16-closed.png'

        # g) The restored library is the live library: it survives a restart.
        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "exit code $($app.ExitCode)"
        $group = Get-GroupName $configPath
        Assert-True ($group -eq 'Restored') "group after exit: $group"
        Record 'app_exit' ("exit=$($app.ExitCode) group=$group")

        Record 'completed' 'all P2-16 backup phases passed'
        Write-Host 'All P2-16 backup phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Get-ChildItem -LiteralPath $configDir -Filter 'config.corrupted.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
