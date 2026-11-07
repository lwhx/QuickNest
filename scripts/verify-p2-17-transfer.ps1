#requires -Version 5.1
# P2-17 acceptance (baseline 17.4/15.7/8.4): configuration export and import.
#
# 1) The transfer dialog opens from the settings menu and every button is
#    wired to a native file dialog.
# 2) The import buttons are driven end to end through the native open dialog:
#    the file is applied, the live configuration is backed up first and
#    imported command items are marked unverified (baseline 8.4).
# 3) The export documents are verified by the --transfer-test validation mode,
#    which runs the same controller calls against real files. The native save
#    dialog cancels under automation (its OK path stalls on a hidden shell
#    confirmation), so the live script verifies that it opens and cancels.
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
public static class P217Window {
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
$report = Join-Path $results 'p2-17-transfer.tsv'
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
        $cb = [P217Window+EnumProc]{
            param($h, $l)
            if (-not [P217Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P217Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P217Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
# Opens the title-bar settings menu (19 rows) and clicks the 1-based row.
function Invoke-SettingsMenu([long]$Hwnd, [int]$ItemIndex) {
    Invoke-ClickAt ($script:left + $script:width - 60) ($script:top + 22) 500
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'settings menu did not open'
    $rect = New-Object P217Window+Rect
    [void][P217Window]::GetWindowRect([IntPtr]$menu, [ref]$rect)
    $hmenu = [P217Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P217Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 26) "settings menu items: $count (expected 26 with the sort and Everything entries)"
    $height = $rect.Bottom - $rect.Top
    $itemHeight = $height / $count
    Invoke-ClickAt ($rect.Left + 50) ($rect.Top + [int]($itemHeight * ($ItemIndex - 0.5))) 800
}
# Waits for the app's native file dialog and returns its dialog and file name
# edit handles.
function Find-NativeDialog([int]$ProcessId, [int]$TimeoutMs = 8000) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $dialog = [P005Keyboard]::FindFileDialogByPid($ProcessId)
        $edit = [P005Keyboard]::FindFileNameEditByPid($ProcessId)
        if ($dialog -ne [IntPtr]::Zero -and $edit -ne [IntPtr]::Zero) {
            return @{ Dialog = $dialog; Edit = $edit }
        }
        Start-Sleep -Milliseconds 150
    }
    return $null
}
# Fills the file name box and presses OK through the dialog itself.
function Complete-NativeDialog([int]$ProcessId, [string]$Path) {
    $found = Find-NativeDialog $ProcessId
    if ($null -eq $found) { return $false }
    [P005Keyboard]::SetText($found.Edit, $Path)
    Start-Sleep -Milliseconds 150
    $written = [P005Keyboard]::GetText($found.Edit)
    if ($written -ne $Path) { throw "file name box holds '$written' instead of '$Path'" }
    [P005Keyboard]::PressDialogButton($found.Dialog, 1)   # IDOK
    Start-Sleep -Milliseconds 1200
    return $true
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
function Backup-Count {
    return @(Get-ChildItem -LiteralPath $script:configDir -Filter 'config.backup.*.json' -File -ErrorAction SilentlyContinue).Count
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$script:configDir = Split-Path -Parent $configPath
$exePath = Join-Path $root 'target\release\quicknest.exe'
$mergeFile = Join-Path $results 'import-merge.json'
$overwriteFile = Join-Path $results 'import-overwrite.json'
$validationReport = Join-Path $results 'p2-17-transfer-validation.tsv'
$script:left = 60
$script:top = 80
$script:width = 760
$app = $null

Stop-QnTestProcesses
Start-Sleep -Milliseconds 400
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $script:configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force

function Write-Library([string]$GroupName, [string]$Path) {
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none' }
        groups = @(@{ id='g1'; name=$GroupName; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=@(
                @{ id='i1'; name='Notepad'; item_type='exe'; icon=''; window_state='normal'; note=''
                   launch=@{ path='C:\Windows\System32\notepad.exe'; args=@(); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } }
            ) }) })
    }
    [IO.File]::WriteAllText($Path, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
}

try {
    # a) Export documents and the full import pipeline are verified by the
    #    validation mode (real files, real controller).
    Remove-QnSandboxItem -LiteralPath $validationReport -ErrorAction SilentlyContinue
    $validation = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -ArgumentList @('--transfer-test', $validationReport) -PassThru -Wait
    Assert-True ($validation.ExitCode -eq 0) "transfer validation exit=$($validation.ExitCode)"
    $lines = @(Get-Content -LiteralPath $validationReport -Encoding UTF8)
    foreach ($phase in @('export_all', 'export_group', 'merge_import', 'overwrite_import', 'broken_rejected', 'completed')) {
        Assert-True ([bool](@($lines | Where-Object { $_ -match "`t$phase`t" }))) "validation phase missing: $phase"
    }
    Record 'validation_mode' 'export_all/export_group/merge/overwrite/broken pass'

    # b) Seed the live configuration and the two import documents.
    Write-Library 'Alpha' $configPath
    $merge = @{
        format = 'quicknest-export'; version = 1; schema_version = 2; scope = 'group'
        groups = @(@{ id='xg'; name='Imported'; icon=''; color=''; last_category_id='xc'; categories=@(
            @{ id='xc'; name='Cat'; icon=''; items=@(
                @{ id='xi'; name='Run Thing'; item_type='command'; icon=''; window_state='normal'; note=''
                   launch=@{ path='cmd.exe'; args=@('/c','echo hi'); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false } }
            ) }) })
    }
    [IO.File]::WriteAllText($mergeFile, ($merge | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    $overwrite = @{
        format = 'quicknest-export'; version = 1; schema_version = 2; scope = 'all'
        settings = @{ pinned = $true; auto_hide_on_launch = $false; theme = 'dark'; accent_color = '#16a34a'; backdrop = 'none' }
        groups = @(@{ id='og'; name='Replaced'; icon=''; color=''; last_category_id='oc'; categories=@(
            @{ id='oc'; name='Cat'; icon=''; items=@() }) })
    }
    [IO.File]::WriteAllText($overwriteFile, ($overwrite | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded' 'live=Alpha merge=Imported overwrite=Replaced/dark'

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

        # c) The settings menu opens the transfer dialog.
        Invoke-SettingsMenu $hwnd 19
        Save-P005Screenshot -Path (Join-Path $results 'p2-17-dialog.png') -WindowHwnd $hwnd
        Record 'dialog_opened' 'p2-17-dialog.png'

        # d) The export buttons open the native save dialog; it is cancelled
        #    so no file is written (the export itself is covered above).
        Invoke-ClickAt ($script:left + 268) ($script:top + 232) 900
        $found = Find-NativeDialog $app.Id
        Assert-True ($null -ne $found) 'save dialog did not open for export all'
        [P005Keyboard]::PressDialogButton($found.Dialog, 2)   # IDCANCEL
        Start-Sleep -Milliseconds 700
        Record 'export_all_dialog' 'save dialog opened and cancelled'

        Invoke-ClickAt ($script:left + 497) ($script:top + 232) 900
        $found = Find-NativeDialog $app.Id
        Assert-True ($null -ne $found) 'save dialog did not open for export group'
        [P005Keyboard]::PressDialogButton($found.Dialog, 2)
        Start-Sleep -Milliseconds 700
        Record 'export_group_dialog' 'save dialog opened and cancelled'

        # e) Merge import through the native open dialog, end to end.
        $backupsBefore = Backup-Count
        Invoke-ClickAt ($script:left + 268) ($script:top + 276) 900
        Assert-True (Complete-NativeDialog $app.Id $mergeFile) 'merge import dialog did not complete'
        $config = Read-Config $configPath
        $names = @($config.groups | ForEach-Object { $_.name })
        Assert-True ($names -contains 'Alpha' -and $names -contains 'Imported') "groups after merge: $($names -join ',')"
        $imported = @($config.groups | Where-Object { $_.name -eq 'Imported' })[0]
        $command = @($imported.categories[0].items)[0]
        Assert-True ($command.item_type -eq 'command') "imported item type: $($command.item_type)"
        Assert-True ($command.launch.confirm_on_first_run -eq $true) 'imported command item is not marked unverified'
        $backupsAfter = Backup-Count
        Assert-True ($backupsAfter -gt $backupsBefore) "no pre-import backup ($backupsBefore -> $backupsAfter)"
        Record 'merge_import' ("groups=$($names -join ',') unverified=$($command.launch.confirm_on_first_run) backups=$backupsBefore->$backupsAfter")

        # f) Overwrite import through the native open dialog.
        Invoke-ClickAt ($script:left + 497) ($script:top + 276) 900
        Assert-True (Complete-NativeDialog $app.Id $overwriteFile) 'overwrite import dialog did not complete'
        $config = Read-Config $configPath
        $names = @($config.groups | ForEach-Object { $_.name })
        Assert-True (@($names).Count -eq 1 -and $names[0] -eq 'Replaced') "groups after overwrite: $($names -join ',')"
        Assert-True ($config.settings.theme -eq 'dark') "theme after overwrite: $($config.settings.theme)"
        Record 'overwrite_import' ("groups=$($names -join ',') theme=$($config.settings.theme)")

        # g) The dialog closes and the import result persists after exit.
        Invoke-ClickAt ($script:left + 581) ($script:top + 359) 500
        Save-P005Screenshot -Path (Join-Path $results 'p2-17-closed.png') -WindowHwnd $hwnd
        Record 'dialog_closed' 'p2-17-closed.png'

        Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
        if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
        $app.WaitForExit()
        Assert-True ($app.ExitCode -eq 0) "exit code $($app.ExitCode)"
        $config = Read-Config $configPath
        Assert-True ($config.groups[0].name -eq 'Replaced') 'import result did not persist'
        Record 'app_exit' ("exit=$($app.ExitCode) group=$($config.groups[0].name)")

        Record 'completed' 'all P2-17 transfer phases passed'
        Write-Host 'All P2-17 transfer phases passed.'
        Get-Content -LiteralPath $report -Encoding UTF8
    } finally {
        if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    }
} finally {
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $script:configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
