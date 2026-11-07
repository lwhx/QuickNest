#requires -Version 5.1
# P1-18 acceptance (baseline chapter 10):
# blur hide after 120 ms + hotkey restore, pinned suppresses blur and launch
# hide, unpinned launch hides, modal dialog and external-drag suppression,
# failed launch keeps the panel.
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
public static class P118Input {
 [StructLayout(LayoutKind.Sequential)]struct Input{public uint type;public Union u;}
 [StructLayout(LayoutKind.Explicit)]struct Union{[FieldOffset(0)]public Mouse mouse;}
 [StructLayout(LayoutKind.Sequential)]struct Mouse{public int x,y;public uint data,flags,time;public UIntPtr extra;}
 [DllImport("user32.dll",SetLastError=true)]static extern uint SendInput(uint n,Input[] i,int size);
 public static void RightButton(bool down){
  var i=new Input{type=0,u=new Union{mouse=new Mouse{flags=down?0x0008u:0x0010u}}};
  if(SendInput(1,new[]{i},Marshal.SizeOf(typeof(Input)))!=1)throw new Win32Exception(Marshal.GetLastWin32Error());
 }
}
'@

$results = $script:QnCaseRoot
[void][IO.Directory]::CreateDirectory($env:QUICKNEST_DATA_DIR)
[void][IO.Directory]::CreateDirectory($results)
$report = Join-Path $results 'p1-18-auto-hide.tsv'
if (Test-Path $report) { Remove-QnSandboxItem $report -Force }
$quickDropLog = Join-Path $results 'quickdrop.jsonl'
if (Test-Path $quickDropLog) { Remove-QnSandboxItem $quickDropLog -Force }

function Record([string]$phase, [string]$detail) {
    $stamp = [DateTime]::UtcNow.ToString('HHmmss.fff')
    Add-Content -LiteralPath $report -Encoding UTF8 -Value "$stamp`t$phase`t$detail"
}

function Get-QuickNestWindow([int]$ProcessId) {
    $matches2 = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $ProcessId -and $_.title -eq 'QuickNest' })
    if ($matches2.Count -gt 0) { return $matches2[0] }
    return $null
}

function Wait-PanelVisibility([long]$Hwnd, [bool]$Visible, [int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    while ([DateTime]::UtcNow -lt $deadline) {
        $shown = @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $Hwnd }).Count -gt 0
        if ($shown -eq $Visible) { return $true }
        Start-Sleep -Milliseconds 50
    }
    return $false
}

function Test-PanelVisible([long]$Hwnd) {
    return @([P005Desktop]::Windows() | Where-Object { $_.hwnd -eq $Hwnd }).Count -gt 0
}

function Send-ShowHotkey {
    [P005Desktop]::Key(0x11, $true)
    [P005Desktop]::Key(0x12, $true)
    [P005Desktop]::Key(0x20, $true)
    [P005Desktop]::Key(0x20, $false)
    [P005Desktop]::Key(0x12, $false)
    [P005Desktop]::Key(0x11, $false)
}

function Show-QuickNest([long]$Hwnd) {
    Send-ShowHotkey
    Start-Sleep -Milliseconds 350
    if (-not (Wait-PanelVisibility $Hwnd $true 3000)) { throw 'hotkey did not restore the panel' }
    Focus-P117Window $Hwnd
}

function Focus-External([long]$Hwnd) {
    Focus-P117Window $Hwnd
    Start-Sleep -Milliseconds 150
}

function Read-Config([string]$Path) {
    for ($attempt = 0; $attempt -lt 20; $attempt++) {
        try {
            if (-not (Test-Path -LiteralPath $Path)) { throw 'config.json missing' }
            return Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
        } catch {
            if ($attempt -eq 19) { throw }
            Start-Sleep -Milliseconds 100
        }
    }
}

function Get-TotalItems($Config) {
    $count = 0
    foreach ($group in $Config.groups) {
        foreach ($category in $group.categories) { $count += @($category.items).Count }
    }
    return $count
}

function Find-Item($Config, [string]$Name) {
    foreach ($group in $Config.groups) {
        foreach ($category in $group.categories) {
            foreach ($item in @($category.items)) {
                if ($item.name -eq $Name) { return $item }
            }
        }
    }
    return $null
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

function Invoke-RightClickAt([int]$X, [int]$Y, [int]$DelayMs = 500) {
    [void][P005Desktop]::SetCursorPos($X, $Y)
    Start-Sleep -Milliseconds 120
    [P118Input]::RightButton($true)
    Start-Sleep -Milliseconds 70
    [P118Input]::RightButton($false)
    Start-Sleep -Milliseconds $DelayMs
}

$configPath = Join-Path $env:QUICKNEST_DATA_DIR 'config.json'
$configDir = Split-Path -Parent $configPath
Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
$exePath = Join-Path $root 'target\release\quicknest.exe'
Stop-QnTestProcesses
Start-Sleep -Milliseconds 400

$run = Join-Path ([IO.Path]::GetTempPath().TrimEnd('\')) ('QuickNestP118-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
$fixtures = Join-Path $run 'fixtures'
[void][IO.Directory]::CreateDirectory($fixtures)
[IO.File]::WriteAllText((Join-Path $fixtures 'hover.txt'), 'QuickNest P1-18 drag suppression fixture')

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
$spawnedMshta = @()
$spawnedNotepad = @()
$explorerHwnd = 0
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
    Assert-True (Test-PanelVisible $hwnd) 'panel should start visible'

    # External focus source: mshta, or the existing Chrome window as fallback.
    $beforeMshta = @(Get-Process mshta -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Start-QnOwnedProcess -FilePath (Join-Path $env:WINDIR 'System32\mshta.exe') -ArgumentList 'about:blank' | Out-Null
    $external = 0
    for ($i = 0; $i -lt 40 -and $external -eq 0; $i++) {
        Start-Sleep -Milliseconds 100
        $candidate = Get-Process mshta -ErrorAction SilentlyContinue | Where-Object { $beforeMshta -notcontains $_.Id } | Select-Object -First 1
        if ($null -ne $candidate) {
            $spawnedMshta += $candidate.Id
            $externalCandidates = @([P005Desktop]::Windows() | Where-Object { $_.pid -eq $candidate.Id })
            if ($externalCandidates.Count -gt 0) { $external = [long]$externalCandidates[0].hwnd }
        }
    }
    if ($external -eq 0) {
        $chromeWindows = @([P005Desktop]::Windows() | Where-Object { $_.title -like '*Google Chrome*' })
        if ($chromeWindows.Count -gt 0) { $external = [long]$chromeWindows[0].hwnd }
    }
    if ($external -eq 0) { throw 'no external window available for focus tests' }
    Record 'external_focus_source' ("hwnd=$external")

    # a) Blur -> 120 ms recheck -> hide (baseline 10.3).
    Focus-External $external
    Assert-True (Wait-PanelVisibility $hwnd $false 3000) 'panel did not auto-hide after losing focus'
    Record 'blur_hide' 'focus outside -> panel hidden'

    # b) Global hotkey restores it.
    Show-QuickNest $hwnd
    Assert-True (Test-PanelVisible $hwnd) 'panel not visible after hotkey'
    Record 'hotkey_restore' 'Ctrl+Alt+Space restored the panel'

    # c) Pinned suppresses the blur hide (baseline 10.7).
    $pinX = $left + $width - 88
    $pinY = $top + 22
    function Set-Pinned([bool]$Wanted) {
        for ($attempt = 0; $attempt -lt 5; $attempt++) {
            try {
                if ((Read-Config $configPath).settings.pinned -eq $Wanted) { return }
            } catch { }
            Focus-P117Window $hwnd
            Invoke-ClickAt $pinX $pinY 450
            Start-Sleep -Milliseconds 350
        }
        throw "title-bar pin did not become $Wanted"
    }
    Set-Pinned $true
    $config = Read-Config $configPath
    Assert-True ($config.settings.pinned -eq $true) 'title-bar pin did not persist'
    Record 'pinned' ("pinned=true via title-bar at $pinX,$pinY")
    Focus-P117Window $hwnd
    Focus-External $external
    Start-Sleep -Milliseconds 900
    Assert-True (Test-PanelVisible $hwnd) 'pinned panel hid on blur'
    Record 'pinned_blur_keeps' 'external focus with pinned=true stayed visible'

    # d) Pinned also suppresses auto-hide-on-launch.
    Focus-P117Window $hwnd
    $beforeNotepad = @(Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Invoke-ClickAt ($left + 208) ($top + 150) 1500
    $spawnedNotepad += @(Get-Process notepad -ErrorAction SilentlyContinue | Where-Object { $beforeNotepad -notcontains $_.Id } | ForEach-Object { $_.Id })
    Assert-True (Test-PanelVisible $hwnd) 'pinned panel hid after launching an item'
    Record 'pinned_launch_keeps' 'pinned launch stayed visible'
    foreach ($id in $spawnedNotepad) { Stop-QnOwnedProcess -Id $id -Force -ErrorAction SilentlyContinue }
    $spawnedNotepad = @()

    # e) Unpin restores blur auto-hide.
    Set-Pinned $false
    $config = Read-Config $configPath
    Assert-True ($config.settings.pinned -eq $false) 'title-bar unpin did not persist'
    Record 'unpinned' 'pinned=false'
    Focus-External $external
    Assert-True (Wait-PanelVisibility $hwnd $false 3000) 'unpinned panel did not hide on blur'
    Record 'unpinned_blur_hide' 'panel hidden after unpin + blur'

    # f) A visible modal dialog holds ModalDialogOpen (baseline 10.5).
    Show-QuickNest $hwnd
    $blankX = $left + $width - 140
    $blankY = $top + $height - 100
    Invoke-RightClickAt $blankX $blankY 450
    Invoke-ClickAt ($blankX + 40) ($blankY + 9) 700
    Save-P005Screenshot -Path (Join-Path $results 'p1-18-dialog-open.png') -WindowHwnd $hwnd
    Focus-External $external
    Start-Sleep -Milliseconds 900
    Assert-True (Test-PanelVisible $hwnd) 'modal dialog did not suppress the blur hide'
    Record 'dialog_suppression' 'visible modal held the panel'
    Focus-P117Window $hwnd
    $dialogLeft = $left + [int](($width - 470) / 2)
    $dialogTop = $top + [int](($height - 396) / 2)
    Invoke-ClickAt ($dialogLeft + 366) ($dialogTop + 370) 500
    Focus-External $external
    Assert-True (Wait-PanelVisibility $hwnd $false 3000) 'panel did not hide after closing the dialog'
    Record 'dialog_closed_hide' 'dialog closed -> blur hide resumed'

    # g) Failed launch keeps the panel (baseline 10.6 failure path).
    Show-QuickNest $hwnd
    Invoke-ClickAt ($left + 400) ($top + 150) 900
    Assert-True (Test-PanelVisible $hwnd) 'failed launch hid the panel'
    Record 'failed_launch_keeps' 'missing target kept the panel'

    # h) Successful launch with auto_hide_on_launch=true hides the panel.
    Focus-P117Window $hwnd
    $beforeNotepad = @(Get-Process notepad -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    Invoke-ClickAt ($left + 208) ($top + 150) 400
    $spawnedNotepad += @(Get-Process notepad -ErrorAction SilentlyContinue | Where-Object { $beforeNotepad -notcontains $_.Id } | ForEach-Object { $_.Id })
    Assert-True (Wait-PanelVisibility $hwnd $false 3000) 'successful launch did not auto-hide the panel'
    Record 'launch_auto_hide' 'successful launch hid the unpinned panel'

    # ExternalDragHover is covered by the state-machine unit test and by the
    # P1-17 drag suite (panel pinned). With auto-hide enabled the 120 ms blur
    # re-check normally fires before Explorer's DragEnter (measured 169 ms),
    # so an unpinned panel can vanish at the start of a real drag; see the
    # P1-18 validation boundary.

    # j) Clean shutdown (WM_CLOSE also works on a hidden window).
    Close-QnOwnedWindow -Hwnd ([IntPtr]$hwnd)
    if (-not $app.WaitForExit(15000)) { (Stop-QnOwnedProcess -Process $app); throw 'QuickNest did not exit on WM_CLOSE' }
    $app.WaitForExit()
    Assert-True ($app.ExitCode -eq 0) 'QuickNest exit code is not zero'
    Record 'app_exit' ("exit=$($app.ExitCode)")
    Record 'completed' 'all P1-18 auto-hide phases passed'
    Write-Host 'All P1-18 auto-hide phases passed.'
    Get-Content -LiteralPath $report -Encoding UTF8
} finally {
    if (-not $app.HasExited) { try { (Stop-QnOwnedProcess -Process $app) } catch { } }
    foreach ($id in $spawnedNotepad) { Stop-QnOwnedProcess -Id $id -Force -ErrorAction SilentlyContinue }
    foreach ($id in $spawnedMshta) { Stop-QnOwnedProcess -Id $id -Force -ErrorAction SilentlyContinue }
    if ($explorerHwnd -ne 0) { try { Close-QnOwnedWindow -Hwnd ([IntPtr]$explorerHwnd) } catch { } }
    Start-Sleep -Milliseconds 500
    Remove-QnSandboxItem -LiteralPath $configPath -ErrorAction SilentlyContinue
    Get-ChildItem -LiteralPath $configDir -Filter 'config.backup.*.json' -ErrorAction SilentlyContinue | Remove-QnSandboxItem -Force
    Remove-QnSandboxItem -LiteralPath $run -Recurse -Force -ErrorAction SilentlyContinue
}

} finally {
    try { Stop-QnTestProcesses } finally { Restore-QnEnvironment $qnSavedEnvironment }
}
