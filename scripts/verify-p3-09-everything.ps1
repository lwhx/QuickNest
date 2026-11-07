#requires -Version 5.1
# P3-09 acceptance (baseline 22 Everything 可选集成).
#
# The integration is opt-in: settings.everything_enabled=true seeds the
# search center with a handoff row ("在 Everything 中搜索…") that launches
# Everything.exe -search <query>. The configured path here is a probe .cmd
# that appends its arguments to a probe file (CreateProcessW cannot exec a
# .cmd directly; the engine's documented Shell fallback runs it, which is
# exactly the production path for batch scripts).
#
# Phases: handoff probe → settings-menu toggle off (no handoff row, Enter on
# an empty result list does nothing) → restart with a missing configured path
# (resolve falls back / reports 未找到, app unaffected) → exit=0.
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
public static class P39Window {
 public struct Rect{public int Left,Top,Right,Bottom;}
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")]public static extern bool EnumWindows(EnumProc cb, IntPtr l);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
 [DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h, out Rect r);
 [DllImport("user32.dll")]public static extern int GetMenuItemCount(IntPtr hMenu);
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p3-09-everything.tsv'
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
        $cb = [P39Window+EnumProc]{
            param($h, $l)
            if (-not [P39Window]::IsWindowVisible($h)) { return $true }
            $sb = New-Object System.Text.StringBuilder 64
            [void][P39Window]::GetClassName($h, $sb, 64)
            if ($sb.ToString() -eq '#32768') { [void]$menus.Add([long]$h) }
            return $true
        }
        [void][P39Window]::EnumWindows($cb, [IntPtr]::Zero)
        if ($menus.Count -gt 0) { return [long]$menus[0] }
        Start-Sleep -Milliseconds 100
    }
    return 0
}
# Opens the settings menu (24 rows incl. the Everything toggle) and clicks
# the 1-based row (24 = Everything 搜索：开/关).
function Invoke-SettingsMenu([long]$Hwnd, [int]$ItemIndex) {
    Focus-P117Window $Hwnd
    Start-Sleep -Milliseconds 250
    Invoke-ClickAt ($script:left + $script:width - 60) ($script:top + 22) 500
    $menu = Wait-MenuWindow 5000
    Assert-True ($menu -ne 0) 'settings menu did not open'
    $rect = New-Object P39Window+Rect
    [void][P39Window]::GetWindowRect([IntPtr]$menu, [ref]$rect)
    $hmenu = [P39Window]::SendMessage([IntPtr]$menu, 0x01E1, [IntPtr]::Zero, [IntPtr]::Zero)
    $count = [P39Window]::GetMenuItemCount($hmenu)
    Assert-True ($count -eq 26) "settings menu items: $count (expected 26 with the Everything toggle)"
    $height = $rect.Bottom - $rect.Top
    $itemHeight = $height / $count
    Invoke-ClickAt ($rect.Left + 50) ($rect.Top + [int]($itemHeight * ($ItemIndex - 0.5))) 800
}
# Opens the search center with Ctrl+F and pastes the query.
function Invoke-Search([long]$Hwnd, [string]$Query) {
    Focus-P117Window $Hwnd
    Start-Sleep -Milliseconds 300
    [P005Keyboard]::Chord(0x11, 0x46)   # Ctrl+F
    Start-Sleep -Milliseconds 500
    [P005Keyboard]::TypeText($Query)
    Start-Sleep -Milliseconds 700
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
function Wait-Window([object]$Process, [int]$TimeoutSeconds) {
    $deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    while ([DateTime]::UtcNow -lt $deadline) {
        $matches = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $Process.Id -and $_.title -like 'QuickNest*' })
        if ($matches.Count -ge 1) { return $matches[0] }
        Start-Sleep -Milliseconds 200
    }
    return $null
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$exePath = Join-Path $root 'target\release\quicknest.exe'
$probeDir = Join-Path $results 'probes'
if (Test-Path $probeDir) { Remove-QnSandboxItem $probeDir -Recurse -Force }
[void][IO.Directory]::CreateDirectory($probeDir)
# The probe must carry a .cmd extension: the handoff launches the configured
# path like any EXE (CreateProcessW first, then the documented Shell
# fallback), and Windows executes batch files by extension only.
$probeEverything = Join-Path $probeDir 'Everything.cmd'
# The redirect target must be absolute: the launched process inherits the
# app's working directory, not the probe directory.
$probeFile = Join-Path $probeDir 'everything-probe.txt'
[IO.File]::WriteAllText($probeEverything, "@echo off`r`necho %* >> `"$probeFile`"`r`n", [Text.Encoding]::ASCII)
$missingPath = Join-Path $probeDir 'no-such\Everything.exe'

# Fallback install locations the engine searches after the configured path.
$fallbacks = @(
    (Join-Path $env:ProgramFiles 'Everything\Everything.exe'),
    (Join-Path ${env:ProgramFiles(x86)} 'Everything\Everything.exe'),
    (Join-Path $env:LOCALAPPDATA 'Everything\Everything.exe')
) | Where-Object { $_ -and (Test-Path -LiteralPath $_) }
$hasRealEverything = @($fallbacks).Count -gt 0

function Start-App {
    $process = Start-QnOwnedProcess -FilePath $exePath -WorkingDirectory $root -PassThru
    $window = Wait-Window $process 20
    if (-not $window) { throw 'QuickNest window not found' }
    $hwnd = [long]$window.hwnd
    [void][P005Desktop]::SetWindowPos([IntPtr]$hwnd, [IntPtr](-1), 60, 60, 0, 0, 0x0001)
    Start-Sleep -Milliseconds 400
    Focus-P117Window $hwnd
    Start-Sleep -Milliseconds 900
    $window = Wait-Window $process 5
    $script:app = $process
    $script:hwnd = $hwnd
    $script:left = $window.rect.Left
    $script:top = $window.rect.Top
    $script:width = $window.rect.Right - $window.rect.Left
    $script:height = $window.rect.Bottom - $window.rect.Top
}
function Stop-AppClean {
    if ($script:app -and -not $script:app.HasExited) {
        Close-QnOwnedWindow -Hwnd ([IntPtr]$script:hwnd)
        if (-not $script:app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $script:app); throw 'QuickNest did not exit on WM_CLOSE' }
        Assert-True ($script:app.ExitCode -eq 0) "exit code $($script:app.ExitCode)"
    }
}

$script:app = $null
Stop-QnTestProcesses
Stop-QnTestProcesses
Start-Sleep -Milliseconds 500
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue

try {
    # a) Seed one local item and the opt-in integration pointing at the probe.
    $items = @(
        @{ id='i1'; name='AlphaTool'; item_type='exe'; icon=''; window_state='normal'; note=''; last_used=0; launch_count=0; favorite=$false
           launch=@{ path="$env:ComSpec"; args=@('/c','echo','unused'); workdir=''; run_as_admin=$false; aumid=''; msi_advertised=$false; confirm_on_first_run=$true; delay_seconds=0; execution_mode='direct' } }
    )
    $library = @{
        schema_version = 2; current_group_id = 'g1'
        settings = @{
            pinned = $true; auto_hide_on_launch = $false; theme = 'light'; accent_color = '#2563eb'; backdrop = 'none'
            sort_mode = 'manual'; everything_enabled = $true; everything_path = $probeEverything
        }
        groups = @(@{ id='g1'; name='Ev'; icon=''; color=''; last_category_id='c1'; categories=@(
            @{ id='c1'; name='Cat'; icon=''; items=$items }) })
    }
    [IO.File]::WriteAllText($configPath, ($library | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Record 'seeded' "everything_enabled=true path=probe.cmd realEverything=$hasRealEverything"

    Start-App

    # b) Enabled + query: the search center shows the handoff row; activating
    #    it runs Everything.exe -search <query> (probe appends its args).
    Invoke-Search $script:hwnd 'qnprobe'
    Save-P005Screenshot -Path (Join-Path $results 'p3-09-search.png') -WindowHwnd $script:hwnd
    [P005Keyboard]::Key(0x0D)   # Enter activates the selected (only) row
    $probeReady = $false
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ([DateTime]::UtcNow -lt $deadline -and -not $probeReady) {
        $probeReady = (Test-Path -LiteralPath $probeFile) -and ((Get-Content $probeFile -Raw).Contains('-search qnprobe'))
        if (-not $probeReady) { Start-Sleep -Milliseconds 200 }
    }
    Assert-True $probeReady 'handoff probe missing; the Everything row did not run'
    Record 'handoff_probe' 'Enter on the handoff row ran Everything.exe -search qnprobe'

    # c) Toggle off through the settings menu (row 24): the flag persists and
    #    the handoff row disappears (Enter on an empty result list is a no-op).
    Invoke-SettingsMenu $script:hwnd 24
    Start-Sleep -Milliseconds 600
    [P005Keyboard]::Key(0x1B)   # close any popup
    Start-Sleep -Milliseconds 400
    $config = Read-Config $configPath
    Assert-True ($config.settings.everything_enabled -eq $false) "toggle off did not persist: $($config.settings.everything_enabled)"
    Invoke-Search $script:hwnd 'qnprobe2'
    Save-P005Screenshot -Path (Join-Path $results 'p3-09-disabled.png') -WindowHwnd $script:hwnd
    $linesBefore = if (Test-Path $probeFile) { @(Get-Content $probeFile).Count } else { 0 }
    [P005Keyboard]::Key(0x0D)
    Start-Sleep -Milliseconds 1200
    $linesAfter = if (Test-Path $probeFile) { @(Get-Content $probeFile).Count } else { 0 }
    Assert-True ($linesAfter -eq $linesBefore) "disabled mode still handed off (probe lines: $linesBefore -> $linesAfter)"
    Record 'toggle_off' "disabled: config flag false, no handoff row (probe lines: $linesAfter)"

    # d) Missing configured path: enabled again with a nonexistent path —
    #    resolution falls back (or reports 未找到) without touching the probe.
    Stop-AppClean
    $config = Read-Config $configPath
    $config.settings.everything_enabled = $true
    $config.settings.everything_path = $missingPath
    [IO.File]::WriteAllText($configPath, ($config | ConvertTo-Json -Depth 12), (New-Object Text.UTF8Encoding($false)))
    Start-App
    if ($hasRealEverything) {
        Invoke-Search $script:hwnd 'qnmiss'
        [P005Keyboard]::Key(0x0D)
        Start-Sleep -Milliseconds 1200
        Record 'missing_path_fallback' 'configured path missing; the fallback install location was used (real Everything present)'
    } else {
        Invoke-Search $script:hwnd 'qnmiss'
        [P005Keyboard]::Key(0x0D)
        Start-Sleep -Milliseconds 1200
        $linesMissing = if (Test-Path $probeFile) { @(Get-Content $probeFile).Count } else { 0 }
        Assert-True ($linesMissing -eq $linesAfter) "missing path unexpectedly launched something (probe lines: $linesMissing)"
        Assert-True (-not $script:app.HasExited) 'app died on the missing-path activation'
        Record 'missing_path_reported' 'configured path missing and no fallback: status reports 未找到, app alive, probe untouched'
    }

    # e) Orderly exit.
    Stop-AppClean
    Record 'persisted' 'exit=0; integration state survived the restart'
    Record 'completed' 'all P3-09 Everything phases passed'
    Write-Host 'All P3-09 Everything phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if ($script:app -and -not $script:app.HasExited) {
        try { (Stop-QnOwnedProcess -Process $script:app) } catch { }
    }
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Stop-QnTestProcesses
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
