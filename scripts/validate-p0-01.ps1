#requires -Version 5.1
<#
.SYNOPSIS
    External, local Release validation for QuickNest P0-01 (Windows PowerShell 5.1).
.DESCRIPTION
    Does NOT build the application or use Slint/raw-window-handle. Launches only the
    specified executable, observes its PID/HWND, captures real desktop pixels and
    sends WM_CLOSE in a separate normal-mode run. Do not interact with these windows
    while the test is running. Use an unlocked, interactive desktop with enough room
    for the entire window. Run at the same integrity level as the target application.

    Output is placed in a NEW per-run subdirectory of ResultsDir, never overwritten.
    Exit codes: 0 = automated checks passed (manual items still pending);
                1 = at least one failed check;
                2 = incomplete, e.g. missing mt.exe or unsupported desktop/host.
    A zero exit code is NOT full P0-01 approval; inspect manualItems and screenshots.
    Tool absence is never reported as a successful check. dumpbin is optional because
    the script independently reads PE imports. mt.exe is required to verify the
    EMBEDDED manifest, not the source manifest.
.EXAMPLE
    powershell.exe -NoProfile -File .\scripts\validate-p0-01.ps1 `
        -ExePath .\target\release\quicknest.exe -ResultsDir .\artifacts\p0-01
.EXAMPLE
    .\scripts\validate-p0-01.ps1 -ExePath C:\build\quicknest.exe `
        -ResultsDir C:\results -MtPath 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\mt.exe'
.NOTES
    Source is intentionally ASCII so Windows PowerShell 5.1 can read it without a BOM.
    No UI coordinates are guessed. Button behavior remains an explicit manual item:
    Slint accessibility/UIA support is not assumed for this minimal build.
#>
[CmdletBinding()]
param(
    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string] $ExePath,

    [Parameter()]
    [ValidateNotNullOrEmpty()]
    [string] $ResultsDir,

    [Parameter()]
    [string] $DumpbinPath,

    [Parameter()]
    [string] $MtPath,

    [ValidateRange(20, 200)]
    [int] $PollIntervalMs = 50,

    [ValidateRange(3, 30)]
    [int] $StartupTimeoutSeconds = 10,

    [ValidateRange(18, 60)]
    [int] $SmokeTimeoutSeconds = 25,

    [ValidateRange(2, 15)]
    [int] $CloseTimeoutSeconds = 5,

    [ValidateRange(0.2, 1.5)]
    [double] $TimingToleranceSeconds = 1.0
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $ExePath) { $ExePath = Join-Path (Split-Path -Parent $scriptRoot) 'target\release\quicknest.exe' }
if (-not $ResultsDir) { $ResultsDir = Join-Path (Split-Path -Parent $scriptRoot) 'docs\test-results\P0-01' }
$script:Checks = New-Object 'System.Collections.Generic.List[object]'
$script:Processes = New-Object 'System.Collections.Generic.List[object]'
$script:Samples = New-Object 'System.Collections.Generic.List[object]'
$script:Screenshots = New-Object 'System.Collections.Generic.List[object]'
$script:Transitions = New-Object 'System.Collections.Generic.List[object]'
$script:RunDir = $null
$script:LogPath = $null
$oldDpiContext = [IntPtr]::Zero
$dpiContextChanged = $false
$exitCode = 2
$startedUtc = [DateTime]::UtcNow
$report = [ordered]@{
    schemaVersion = 1
    startedUtc = $startedUtc.ToString('o')
    finishedUtc = $null
    overall = 'Incomplete'
    exitCode = 2
    executable = $null
    environment = [ordered]@{}
    parameters = [ordered]@{
        pollIntervalMs = $PollIntervalMs
        startupTimeoutSeconds = $StartupTimeoutSeconds
        smokeTimeoutSeconds = $SmokeTimeoutSeconds
        closeTimeoutSeconds = $CloseTimeoutSeconds
        timingToleranceSeconds = $TimingToleranceSeconds
    }
    tools = [ordered]@{}
    pe = $null
    manifest = $null
    processes = @()
    transitions = @()
    screenshots = @()
    checks = @()
    manualItems = @(
        [ordered]@{ id = 'buttons-hide-restore'; status = 'Pending'; instructions = 'In a separate normal-mode run, click the rendered hide/restore button, verify ~2 seconds hidden, restoration of the same PID/HWND and responsive controls. UIA is not assumed; this script does not guess button coordinates.' },
        [ordered]@{ id = 'buttons-exit'; status = 'Pending'; instructions = 'In a separate normal-mode run, click the rendered exit button and independently verify process exit code 0. WM_CLOSE automation does not cover this callback.' },
        [ordered]@{ id = 'visual-review'; status = 'Pending'; instructions = 'Inspect all PNGs for a real unobscured QuickNest window, icon grid, Chinese text, both buttons and correct restoration. Pixel capture alone cannot prove layout correctness or absence of every overlay.' }
    )
}

function Write-Log {
    param([string] $Message)
    $line = '{0} {1}' -f [DateTime]::UtcNow.ToString('o'), $Message
    Write-Host $line
    if ($script:LogPath) { Add-Content -LiteralPath $script:LogPath -Value $line -Encoding UTF8 }
}

function Add-Check {
    param([string] $Name, [ValidateSet('Passed', 'Failed', 'Incomplete')] [string] $Status, [string] $Detail)
    $script:Checks.Add([pscustomobject]@{ name = $Name; status = $Status; detail = $Detail })
    Write-Log ('[{0}] {1}: {2}' -f $Status, $Name, $Detail)
}

function Quote-NativeArgument {
    param([string] $Value)
    # CommandLineToArgvW-compatible quoting, including trailing backslashes.
    '"' + [regex]::Replace([regex]::Replace($Value, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1') + '"'
}

function Invoke-CapturedTool {
    param([string] $Path, [string[]] $ArgumentList, [string] $LogName)
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $Path
    $info.Arguments = (($ArgumentList | ForEach-Object { Quote-NativeArgument $_ }) -join ' ')
    $info.WorkingDirectory = $script:RunDir
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $tool = New-Object System.Diagnostics.Process
    $tool.StartInfo = $info
    try {
        if (-not $tool.Start()) { throw 'Tool process did not start.' }
        $outTask = $tool.StandardOutput.ReadToEndAsync()
        $errTask = $tool.StandardError.ReadToEndAsync()
        if (-not $tool.WaitForExit(30000)) {
            $tool.Kill()
            [void] $tool.WaitForExit(3000)
            throw "Tool timed out: $Path"
        }
        if (-not $outTask.Wait(3000) -or -not $errTask.Wait(3000)) { throw 'Tool output did not close.' }
        $text = $outTask.Result + [Environment]::NewLine + $errTask.Result
        Set-Content -LiteralPath (Join-Path $script:RunDir $LogName) -Value $text -Encoding UTF8
        if ($tool.ExitCode -ne 0) { throw "Tool exited $($tool.ExitCode); see $LogName" }
        return $text
    }
    finally { $tool.Dispose() }
}

function Resolve-ValidationTool {
    param([string] $ExplicitPath, [ValidateSet('dumpbin.exe', 'mt.exe')] [string] $Name)
    if ($ExplicitPath) {
        $item = Get-Item -LiteralPath $ExplicitPath -ErrorAction Stop
        if ($item.PSIsContainer -or $item.Name -ine $Name) { throw "Expected absolute file path to $Name." }
        return $item.FullName
    }
    $onPath = Get-Command -Name $Name -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($onPath) { return $onPath.Source }
    $roots = @(${env:ProgramFiles(x86)}, $env:ProgramFiles) | Where-Object { $_ } | Select-Object -Unique
    $candidates = @()
    foreach ($root in $roots) {
        if ($Name -eq 'dumpbin.exe') {
            $pattern = Join-Path $root 'Microsoft Visual Studio\*\*\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe'
            $candidates += @(Get-ChildItem -Path $pattern -File -ErrorAction SilentlyContinue)
            # vswhere covers VS installed on a nondefault drive; never run a relative tool.
            $vswhere = Join-Path $root 'Microsoft Visual Studio\Installer\vswhere.exe'
            if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
                $found = Invoke-CapturedTool -Path $vswhere -ArgumentList @('-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-find', 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe') -LogName 'vswhere.log'
                foreach ($line in ($found -split '\r?\n')) {
                    if ($line.Trim() -and (Test-Path -LiteralPath $line.Trim() -PathType Leaf)) {
                        $candidates += Get-Item -LiteralPath $line.Trim()
                    }
                }
            }
        }
        else {
            $candidates += @(Get-ChildItem -Path (Join-Path $root 'Windows Kits\10\bin\*\x64\mt.exe') -File -ErrorAction SilentlyContinue)
            $candidates += @(Get-ChildItem -Path (Join-Path $root 'Windows Kits\8.1\bin\x64\mt.exe') -File -ErrorAction SilentlyContinue)
        }
    }
    $chosen = $candidates | Sort-Object -Property @{ Expression = { $_.VersionInfo.FileVersionRaw }; Descending = $true }, FullName | Select-Object -First 1
    if ($chosen) { return $chosen.FullName }
    return $null
}

function Start-ValidationProcess {
    param([string] $Mode, [string] $Arguments)
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $script:ResolvedExe
    $info.Arguments = $Arguments
    $info.WorkingDirectory = Split-Path -Parent $script:ResolvedExe
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.CreateNoWindow = $false
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    if (-not $process.Start()) { $process.Dispose(); throw 'Application did not start.' }
    $entry = [pscustomobject]@{
        mode = $Mode; process = $process; clock = $clock
        targetPid = $process.Id; stdout = $process.StandardOutput.ReadToEndAsync()
        stderr = $process.StandardError.ReadToEndAsync(); integrity = $null
        hwnd = $null; exitCode = $null; forcedTermination = $false; outputSaved = $false
    }
    $script:Processes.Add($entry)
    try { $entry.integrity = [QuickNestP001.Native]::Integrity($entry.targetPid) }
    catch { $entry.integrity = 'Unavailable: ' + $_.Exception.Message }
    Write-Log ("Started {0}: PID={1}; integrity={2}" -f $Mode, $entry.targetPid, $entry.integrity)
    return $entry
}

function Save-ProcessOutput {
    param($Entry)
    if ($Entry.outputSaved -or -not $Entry.process.HasExited) { return }
    $Entry.exitCode = $Entry.process.ExitCode
    if (-not $Entry.stdout.Wait(3000) -or -not $Entry.stderr.Wait(3000)) {
        throw "Output pipes did not close for PID $($Entry.targetPid)."
    }
    Set-Content -LiteralPath (Join-Path $script:RunDir ($Entry.mode + '.stdout.log')) -Value $Entry.stdout.Result -Encoding UTF8
    Set-Content -LiteralPath (Join-Path $script:RunDir ($Entry.mode + '.stderr.log')) -Value $Entry.stderr.Result -Encoding UTF8
    $Entry.outputSaved = $true
    Write-Log ("Exited {0}: PID={1}; exitCode={2}" -f $Entry.mode, $Entry.targetPid, $Entry.exitCode)
}

function Get-TargetWindows {
    param([int] $TargetPid)
    @([QuickNestP001.Native]::Windows($TargetPid) | Where-Object { $_.Title -ceq [QuickNestP001.Native]::ExpectedTitle })
}

function Wait-InitialWindow {
    param($Entry)
    while ($Entry.clock.Elapsed.TotalSeconds -lt $StartupTimeoutSeconds) {
        if ($Entry.process.HasExited) { Save-ProcessOutput $Entry; throw 'Process exited before its initial window appeared.' }
        $windows = @(Get-TargetWindows $Entry.targetPid)
        if ($windows.Count -gt 1) { throw 'Multiple top-level windows have the expected title for this PID.' }
        if ($windows.Count -eq 1 -and $windows[0].Visible) {
            if ($windows[0].Hung) { throw 'Initial window is reported hung by IsHungAppWindow.' }
            $Entry.hwnd = $windows[0].Handle
            return $windows[0]
        }
        Start-Sleep -Milliseconds $PollIntervalMs
    }
    throw "Expected visible window was not found within $StartupTimeoutSeconds seconds."
}

function Save-WindowScreenshot {
    param($Entry, [long] $Handle, [string] $Name)
    $bitmap = $null
    $graphics = $null
    $wasTopmost = $null
    $window = [IntPtr] $Handle
    $capture = [ordered]@{ name = $Name; status = 'Failed'; path = $null; elapsedSeconds = $Entry.clock.Elapsed.TotalSeconds; detail = $null }
    try {
        $window = [IntPtr] $Handle
        if (-not [QuickNestP001.Native]::IsWindow($window) -or -not [QuickNestP001.Native]::IsWindowVisible($window)) { throw 'Window is not visible/alive for capture.' }
        if ([QuickNestP001.Native]::IsIconic($window)) { throw 'Window is minimized; refusing a misleading desktop capture.' }
        if ([QuickNestP001.Native]::WindowPid($window) -ne $Entry.targetPid) { throw 'HWND no longer belongs to the launched process.' }
        $wasTopmost = [QuickNestP001.Native]::IsTopmost($window)
        if (-not [QuickNestP001.Native]::SetWindowPos($window, [IntPtr](-1), 0, 0, 0, 0, 0x0013)) {
            throw 'Could not temporarily bring validation window above other windows.'
        }
        Start-Sleep -Milliseconds 100
        $rect = [QuickNestP001.Native]::CaptureBounds($window)
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($width -lt 100 -or $height -lt 100) { throw "Invalid window bounds: ${width}x${height}." }
        $bounds = New-Object System.Drawing.Rectangle -ArgumentList $rect.Left, $rect.Top, $width, $height
        # A virtual-screen bounding box alone incorrectly accepts gaps between monitors.
        $region = New-Object System.Drawing.Region -ArgumentList $bounds
        try {
            foreach ($screen in [System.Windows.Forms.Screen]::AllScreens) { $region.Exclude($screen.Bounds) }
            $scan = $region.GetRegionScans((New-Object System.Drawing.Drawing2D.Matrix))
            if ($scan.Length -ne 0) { throw 'The window is not fully inside the available monitors; move it and rerun.' }
        }
        finally { $region.Dispose() }
        $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size, [System.Drawing.CopyPixelOperation]::SourceCopy)
        if (-not [QuickNestP001.Native]::IsWindowVisible($window)) { throw 'Visibility changed during pixel capture.' }
        $colors = New-Object 'System.Collections.Generic.HashSet[int]'
        for ($y = 0; $y -lt $height; $y += [Math]::Max(1, [int]($height / 20))) {
            for ($x = 0; $x -lt $width; $x += [Math]::Max(1, [int]($width / 20))) { [void] $colors.Add($bitmap.GetPixel($x, $y).ToArgb()) }
        }
        if ($colors.Count -lt 2) { throw 'Captured pixels are uniform/blank; refusing a false positive.' }
        $path = Join-Path $script:RunDir ($Name + '.png')
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        $capture.status = 'Passed'
        $capture.path = $path
        $capture.detail = [ordered]@{
            left = $rect.Left; top = $rect.Top; width = $width; height = $height
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
            captureMethod = 'Desktop pixels; target temporarily topmost without activation'
            dpi = [QuickNestP001.Native]::WindowDpi($window)
            sampledDistinctColors = $colors.Count
        }
        Add-Check ('screenshot-' + $Name) 'Passed' "$path (${width}x${height})"
    }
    catch {
        $capture.detail = $_.Exception.Message
        Add-Check ('screenshot-' + $Name) 'Failed' $_.Exception.Message
    }
    finally {
        if ($wasTopmost -eq $false -and [QuickNestP001.Native]::IsWindow($window)) {
            [void] [QuickNestP001.Native]::SetWindowPos($window, [IntPtr](-2), 0, 0, 0, 0, 0x0013)
        }
        if ($graphics) { $graphics.Dispose() }
        if ($bitmap) { $bitmap.Dispose() }
        $script:Screenshots.Add([pscustomobject] $capture)
    }
}

function Test-SmokeRun {
    $entry = Start-ValidationProcess 'smoke' '--smoke-test'
    try {
        $window = Wait-InitialWindow $entry
        $initialTime = $entry.clock.Elapsed.TotalSeconds
        $initialHandle = $window.Handle
        $script:Transitions.Add([pscustomobject]@{ state = 'visible'; step = 0; elapsedSeconds = $initialTime; hwnd = $initialHandle; targetPid = $entry.targetPid })
        Write-Log ("Smoke initial window: HWND=0x{0:X}; title={1}" -f $initialHandle, $window.Title)
        Save-WindowScreenshot $entry $initialHandle 'smoke-initial'
        $lastVisible = $true
        $step = 0
        $lastSample = $initialTime
        $maxGap = 0.0
        while (-not $entry.process.HasExited) {
            $now = $entry.clock.Elapsed.TotalSeconds
            if ($now -gt $SmokeTimeoutSeconds) { throw "Smoke run exceeded $SmokeTimeoutSeconds seconds." }
            $gap = $now - $lastSample
            $maxGap = [Math]::Max($maxGap, $gap)
            $lastSample = $now
            # A full two-second phase might otherwise be missed while taking screenshots.
            if ($gap -gt 1.5) { throw "Observation gap ${gap}s is too large to establish every 2-second phase." }
            $windowPtr = [IntPtr] $initialHandle
            if (-not [QuickNestP001.Native]::IsWindow($windowPtr)) {
                if ($entry.process.WaitForExit(200)) { break }
                throw 'Original smoke HWND was destroyed before process exit.'
            }
            if ([QuickNestP001.Native]::WindowPid($windowPtr) -ne $entry.targetPid) { throw 'Original HWND changed ownership.' }
            $matching = @(Get-TargetWindows $entry.targetPid)
            if ($matching.Count -ne 1 -or $matching[0].Handle -ne $initialHandle) { throw 'Smoke window was replaced, duplicated, or its expected title changed.' }
            $visible = [QuickNestP001.Native]::IsWindowVisible($windowPtr)
            $hung = [QuickNestP001.Native]::IsHungAppWindow($windowPtr)
            $script:Samples.Add([pscustomobject]@{ mode = 'smoke'; elapsedSeconds = $now; targetPid = $entry.targetPid; hwnd = $initialHandle; visible = $visible; hung = $hung })
            if ($hung) { throw 'Smoke window is reported hung by IsHungAppWindow.' }
            if ($visible -ne $lastVisible) {
                $step++
                if ($step -gt 6) { throw 'Observed more than six visibility transitions.' }
                $expectedVisible = ($step % 2 -eq 0)
                if ($visible -ne $expectedVisible) { throw "Unexpected visibility at step $step." }
                $relative = $now - $initialTime
                $expectedTime = 2.0 * $step
                if ([Math]::Abs($relative - $expectedTime) -gt $TimingToleranceSeconds) {
                    throw "Step $step observed at ${relative}s from first display; expected ${expectedTime}s +/- ${TimingToleranceSeconds}s."
                }
                $state = if ($visible) { 'visible' } else { 'hidden' }
                $script:Transitions.Add([pscustomobject]@{ state = $state; step = $step; elapsedSeconds = $now; secondsFromInitial = $relative; hwnd = $initialHandle; targetPid = $entry.targetPid })
                Write-Log "Smoke step ${step}: $state at ${relative}s; same PID/HWND."
                $lastVisible = $visible
                if ($visible) { Save-WindowScreenshot $entry $initialHandle ('smoke-restored-' + [int]($step / 2)) }
            }
            # Check deadlines even when a broken application stays in one state.
            if ($step -lt 6 -and ($now - $initialTime) -gt (2.0 * ($step + 1) + $TimingToleranceSeconds)) { throw "Visibility transition $($step + 1) was not observed before its deadline." }
            Start-Sleep -Milliseconds $PollIntervalMs
        }
        Save-ProcessOutput $entry
        $exitAt = $entry.clock.Elapsed.TotalSeconds - $initialTime
        if ($step -ne 6 -or -not $lastVisible) { throw "Expected three hide/show rounds; observed $step transitions." }
        if ([Math]::Abs($exitAt - 14.0) -gt $TimingToleranceSeconds) { throw "Smoke exit observed at ${exitAt}s from initial display, not 14s +/- ${TimingToleranceSeconds}s." }
        if ($entry.exitCode -ne 0) { throw "Smoke exit code was $($entry.exitCode), not 0." }
        Add-Check 'smoke-same-pid-hwnd-three-rounds-and-exit' 'Passed' "Six timed transitions, same PID/HWND, no observed hung state, exit 0 at ${exitAt}s; maximum sampling gap ${maxGap}s. IsHungAppWindow is a sampled heuristic, not a proof of perpetual responsiveness."
    }
    catch { Add-Check 'smoke-same-pid-hwnd-three-rounds-and-exit' 'Failed' $_.Exception.Message }
    finally { Stop-ValidationProcess $entry }
}

function Stop-ValidationProcess {
    param($Entry)
    if (-not $Entry.process.HasExited) {
        $Entry.forcedTermination = $true
        $Entry.process.Kill()
        if (-not $Entry.process.WaitForExit(5000)) { throw "Could not terminate launched PID $($Entry.targetPid)." }
        Write-Log "Cleanup forcibly terminated launched PID $($Entry.targetPid); not a successful normal exit."
    }
    Save-ProcessOutput $Entry
}

function Test-NativeClose {
    $entry = Start-ValidationProcess 'normal-close' ''
    try {
        $window = Wait-InitialWindow $entry
        $handle = [IntPtr] $window.Handle
        Save-WindowScreenshot $entry $window.Handle 'normal-before-wm-close'
        if ($entry.process.HasExited) { throw 'Normal process exited before WM_CLOSE was sent.' }
        if (-not [QuickNestP001.Native]::IsWindow($handle) -or [QuickNestP001.Native]::WindowPid($handle) -ne $entry.targetPid) { throw 'Normal-mode HWND is no longer valid/owned by target.' }
        if ([QuickNestP001.Native]::IsHungAppWindow($handle)) { throw 'Normal-mode window is hung before WM_CLOSE.' }
        # PostMessage avoids blocking PowerShell on a stuck window procedure.
        if (-not [QuickNestP001.Native]::PostMessage($handle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) {
            throw (New-Object System.ComponentModel.Win32Exception -ArgumentList ([Runtime.InteropServices.Marshal]::GetLastWin32Error()))
        }
        Write-Log ("Posted WM_CLOSE to normal PID={0}, HWND=0x{1:X}." -f $entry.targetPid, $window.Handle)
        if (-not $entry.process.WaitForExit($CloseTimeoutSeconds * 1000)) { throw "Normal mode did not exit within $CloseTimeoutSeconds seconds of WM_CLOSE." }
        Save-ProcessOutput $entry
        if ($entry.exitCode -ne 0) { throw "Normal WM_CLOSE exit code was $($entry.exitCode), not 0." }
        Add-Check 'normal-mode-native-wm-close-exit0' 'Passed' 'WM_CLOSE sent to the exact PID/HWND; process terminated with exit code 0, without forced termination.'
    }
    catch { Add-Check 'normal-mode-native-wm-close-exit0' 'Failed' $_.Exception.Message }
    finally { Stop-ValidationProcess $entry }
}

try {
    if ($env:OS -ne 'Windows_NT') { throw 'This script requires Windows.' }
    $baseDir = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ResultsDir))
    [void] [IO.Directory]::CreateDirectory($baseDir)
    $script:RunDir = Join-Path $baseDir ('run-' + $startedUtc.ToString('yyyyMMddTHHmmssfffZ') + '-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
    [void] [IO.Directory]::CreateDirectory($script:RunDir)
    $script:LogPath = Join-Path $script:RunDir 'validation.log'
    Write-Log 'P0-01 external Release validation started. This script does not build anything.'
    $item = Get-Item -LiteralPath $ExePath
    if ($item.PSIsContainer) { throw 'ExePath must point to an executable file.' }
    $script:ResolvedExe = $item.FullName
    $initialHash = (Get-FileHash -LiteralPath $script:ResolvedExe -Algorithm SHA256).Hash
    $report.executable = [ordered]@{ path = $script:ResolvedExe; sha256 = $initialHash; length = $item.Length; lastWriteTimeUtc = $item.LastWriteTimeUtc.ToString('o') }
    $report.environment = [ordered]@{
        osVersion = [Environment]::OSVersion.VersionString
        is64BitOS = [Environment]::Is64BitOperatingSystem
        is64BitPowerShell = [Environment]::Is64BitProcess
        powershell = $PSVersionTable.PSVersion.ToString()
        userInteractive = [Environment]::UserInteractive
        sessionId = [Diagnostics.Process]::GetCurrentProcess().SessionId
        user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
        currentDirectory = (Get-Location).Path
    }
    try {
        $os = Get-CimInstance Win32_OperatingSystem
        $report.environment['osCaption'] = $os.Caption
        $report.environment['osBuild'] = $os.BuildNumber
        $report.environment['osArchitecture'] = $os.OSArchitecture
    }
    catch { $report.environment['osDetailsError'] = $_.Exception.Message }
    if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
        Add-Check 'host' 'Incomplete' 'AMD64 OS and 64-bit PowerShell are required (avoid WOW64 System32 redirection).'
        throw 'Unsupported host architecture.'
    }
    if (-not [Environment]::UserInteractive -or $report.environment.sessionId -eq 0) {
        Add-Check 'interactive-desktop' 'Incomplete' 'Run from an unlocked interactive user session, not session 0/headless CI.'
        throw 'No suitable interactive desktop.'
    }
    Add-Type -AssemblyName System.Drawing
    Add-Type -AssemblyName System.Windows.Forms
    if (-not ('QuickNestP001.Native' -as [type])) {
        # All interop below uses IntPtr + Marshal and bounds-checked managed reads.
        # There is NO unsafe C# and no /unsafe compiler prerequisite or option.
        Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace QuickNestP001 {
    public struct Rect { public int Left, Top, Right, Bottom; }
    public sealed class WindowInfo {
        public long Handle; public string Title; public string ClassName;
        public bool Visible; public bool Hung;
    }
    public sealed class PeInfo {
        public string Machine; public string OptionalMagic; public int Subsystem;
        public string[] Imports; public string[] DelayImports;
    }
    public static class Native {
        public const string ExpectedTitle = "QuickNest \u00b7 P0-01 \u6280\u672f\u9a8c\u8bc1";
        private delegate bool EnumProc(IntPtr hwnd, IntPtr data);
        [DllImport("user32.dll", SetLastError=true)] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
        [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
        [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
        [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
        [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);
        [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
        [DllImport("user32.dll")] public static extern bool IsHungAppWindow(IntPtr hwnd);
        [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
        [DllImport("user32.dll", SetLastError=true)] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
        [DllImport("user32.dll", EntryPoint="GetWindowLongW")] private static extern int GetWindowLong(IntPtr hwnd, int index);
        public static bool IsTopmost(IntPtr hwnd) { return (GetWindowLong(hwnd, -20) & 8) != 0; }
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll", SetLastError=true)] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
        [DllImport("user32.dll", SetLastError=true)] private static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
        [DllImport("dwmapi.dll")] private static extern int DwmGetWindowAttribute(IntPtr hwnd, int attr, out Rect rect, int size);
        [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
        [DllImport("user32.dll")] private static extern uint GetDpiForWindow(IntPtr hwnd);
        [DllImport("kernel32.dll", SetLastError=true)] private static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
        [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr handle);
        [DllImport("advapi32.dll", SetLastError=true)] private static extern bool OpenProcessToken(IntPtr process, uint access, out IntPtr token);
        [DllImport("advapi32.dll", SetLastError=true)] private static extern bool GetTokenInformation(IntPtr token, int info, IntPtr buffer, int size, out int needed);
        [DllImport("advapi32.dll")] private static extern IntPtr GetSidSubAuthorityCount(IntPtr sid);
        [DllImport("advapi32.dll")] private static extern IntPtr GetSidSubAuthority(IntPtr sid, uint index);
        public static uint WindowPid(IntPtr hwnd) { uint pid; GetWindowThreadProcessId(hwnd, out pid); return pid; }
        public static WindowInfo[] Windows(int targetPid) {
            List<WindowInfo> list = new List<WindowInfo>();
            EnumProc callback = delegate(IntPtr hwnd, IntPtr data) {
                if (WindowPid(hwnd) == (uint)targetPid) {
                    StringBuilder title = new StringBuilder(1024), cls = new StringBuilder(256);
                    GetWindowText(hwnd, title, title.Capacity); GetClassName(hwnd, cls, cls.Capacity);
                    list.Add(new WindowInfo { Handle=hwnd.ToInt64(), Title=title.ToString(), ClassName=cls.ToString(), Visible=IsWindowVisible(hwnd), Hung=IsHungAppWindow(hwnd) });
                }
                return true;
            };
            if (!EnumWindows(callback, IntPtr.Zero)) throw new Win32Exception(Marshal.GetLastWin32Error());
            GC.KeepAlive(callback); return list.ToArray();
        }
        public static Rect CaptureBounds(IntPtr hwnd) {
            Rect rect;
            try { if (DwmGetWindowAttribute(hwnd, 9, out rect, Marshal.SizeOf(typeof(Rect))) == 0) return rect; }
            catch (DllNotFoundException) { } catch (EntryPointNotFoundException) { }
            if (!GetWindowRect(hwnd, out rect)) throw new Win32Exception(Marshal.GetLastWin32Error());
            return rect;
        }
        public static uint WindowDpi(IntPtr hwnd) {
            try { return GetDpiForWindow(hwnd); } catch (EntryPointNotFoundException) { return 0; }
        }
        public static string Integrity(int pid) {
            IntPtr process = OpenProcess(0x1000, false, pid), token = IntPtr.Zero, memory = IntPtr.Zero;
            if (process == IntPtr.Zero) throw new Win32Exception(Marshal.GetLastWin32Error());
            try {
                if (!OpenProcessToken(process, 0x0008, out token)) throw new Win32Exception(Marshal.GetLastWin32Error());
                int needed; GetTokenInformation(token, 25, IntPtr.Zero, 0, out needed);
                if (needed <= 0) throw new Win32Exception(Marshal.GetLastWin32Error());
                memory = Marshal.AllocHGlobal(needed);
                if (!GetTokenInformation(token, 25, memory, needed, out needed)) throw new Win32Exception(Marshal.GetLastWin32Error());
                IntPtr sid = Marshal.ReadIntPtr(memory);
                byte count = Marshal.ReadByte(GetSidSubAuthorityCount(sid));
                if (count == 0) throw new InvalidDataException("Invalid integrity SID");
                int rid = Marshal.ReadInt32(GetSidSubAuthority(sid, (uint)(count - 1)));
                string level = rid < 0x1000 ? "Untrusted" : rid < 0x2000 ? "Low" : rid < 0x3000 ? "Medium" : rid < 0x4000 ? "High" : rid < 0x5000 ? "System" : "Protected";
                return level + " (RID=0x" + rid.ToString("X") + ")";
            } finally {
                if (memory != IntPtr.Zero) Marshal.FreeHGlobal(memory);
                if (token != IntPtr.Zero) CloseHandle(token); CloseHandle(process);
            }
        }
    }
    public static class PeReader {
        private sealed class Section { public uint VirtualAddress, RawAddress, RawSize; }
        private static void Need(byte[] b, long p, long size) {
            if (p < 0 || size < 0 || p > b.LongLength - size) throw new InvalidDataException("Truncated or invalid PE offset");
        }
        private static ushort U16(byte[] b, long p) { Need(b,p,2); return BitConverter.ToUInt16(b,(int)p); }
        private static uint U32(byte[] b, long p) { Need(b,p,4); return BitConverter.ToUInt32(b,(int)p); }
        private static ulong U64(byte[] b, long p) { Need(b,p,8); return BitConverter.ToUInt64(b,(int)p); }
        private static long Rva(byte[] b, uint rva, uint headers, List<Section> sections) {
            if (rva < headers) { Need(b,rva,1); return rva; }
            foreach (Section s in sections) {
                if (rva >= s.VirtualAddress && (ulong)rva - s.VirtualAddress < s.RawSize) {
                    long p = s.RawAddress + ((long)rva - s.VirtualAddress); Need(b,p,1); return p;
                }
            }
            throw new InvalidDataException("RVA does not map to file-backed PE data");
        }
        private static string Name(byte[] b, long p) {
            StringBuilder name = new StringBuilder();
            for (int i=0; i<4096; i++) {
                Need(b,p+i,1); byte ch=b[p+i]; if (ch==0) {
                    if (name.Length==0) throw new InvalidDataException("Empty DLL name");
                    return name.ToString();
                }
                if (ch < 32 || ch > 126) throw new InvalidDataException("Non-ASCII DLL import name");
                name.Append((char)ch);
            }
            throw new InvalidDataException("Unterminated DLL name");
        }
        private static string[] Imports(byte[] b, long optional, int optionalSize, int directory, uint headers, List<Section> sections, bool delay) {
            long dirOffset=112+directory*8;
            if (U32(b,optional+108) <= directory) return new string[0];
            if (dirOffset+8 > optionalSize) throw new InvalidDataException("Invalid PE directory table");
            uint address=U32(b,optional+dirOffset), size=U32(b,optional+dirOffset+4);
            if (address==0 && size==0) return new string[0];
            int stride=delay?32:20;
            if (address==0 || size<stride) throw new InvalidDataException("Invalid import directory");
            SortedSet<string> names = new SortedSet<string>(StringComparer.OrdinalIgnoreCase);
            for (uint i=0; i<size/stride && i<4096; i++) {
                uint descriptor=checked(address + i*(uint)stride);
                long p=Rva(b,descriptor,headers,sections); Need(b,p,stride);
                bool zero=true; for(int j=0;j<stride;j++) if(b[p+j]!=0) { zero=false; break; }
                if (zero) { string[] result=new string[names.Count]; names.CopyTo(result); return result; }
                uint nameRva=U32(b,p+(delay?4:12));
                if (delay && (U32(b,p)&1)==0) {
                    ulong imageBase=U64(b,optional+24);
                    if (nameRva < imageBase) throw new InvalidDataException("Invalid VA-based delay import");
                    nameRva=checked((uint)((ulong)nameRva-imageBase));
                }
                if (nameRva==0) throw new InvalidDataException("Missing imported DLL name");
                names.Add(Name(b,Rva(b,nameRva,headers,sections)));
            }
            throw new InvalidDataException("Import descriptors lack a bounded terminator");
        }
        public static PeInfo Read(string path) {
            byte[] b=File.ReadAllBytes(path);
            if (U16(b,0)!=0x5A4D) throw new InvalidDataException("Missing MZ signature");
            long pe=U32(b,0x3c);
            if (U32(b,pe)!=0x00004550) throw new InvalidDataException("Missing PE signature");
            ushort machine=U16(b,pe+4), sectionCount=U16(b,pe+6), optionalSize=U16(b,pe+20);
            long optional=pe+24; Need(b,optional,optionalSize);
            ushort magic=U16(b,optional);
            if (machine!=0x8664 || magic!=0x20b) throw new InvalidDataException("Expected AMD64 (0x8664), PE32+ (0x020B)");
            if (optionalSize<112 || sectionCount==0 || sectionCount>96) throw new InvalidDataException("Invalid PE header sizes");
            ushort characteristics=U16(b,pe+22);
            if ((characteristics&0x0002)==0 || (characteristics&0x2000)!=0) throw new InvalidDataException("Expected an executable image, not a DLL");
            int subsystem=U16(b,optional+68);
            if (subsystem!=2) throw new InvalidDataException("Expected Windows GUI subsystem (2), not console");
            uint headers=U32(b,optional+60);
            List<Section> sections=new List<Section>();
            for (int i=0;i<sectionCount;i++) {
                long s=optional+optionalSize+i*40; Need(b,s,40);
                sections.Add(new Section { VirtualAddress=U32(b,s+12), RawSize=U32(b,s+16), RawAddress=U32(b,s+20) });
            }
            return new PeInfo { Machine="AMD64 (0x8664)", OptionalMagic="PE32+ (0x020B)", Subsystem=subsystem,
                Imports=Imports(b,optional,optionalSize,1,headers,sections,false),
                DelayImports=Imports(b,optional,optionalSize,13,headers,sections,true) };
        }
    }
}
'@
    }
    try { $report.environment['scriptIntegrity'] = [QuickNestP001.Native]::Integrity([Diagnostics.Process]::GetCurrentProcess().Id) }
    catch { $report.environment['scriptIntegrity'] = 'Unavailable: ' + $_.Exception.Message }
    try {
        $oldDpiContext = [QuickNestP001.Native]::SetThreadDpiAwarenessContext([IntPtr](-4))
        if ($oldDpiContext -eq [IntPtr]::Zero) { throw 'SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2) failed.' }
        $dpiContextChanged = $true
        $report.environment['captureDpiContext'] = 'PER_MONITOR_AWARE_V2 (thread-local, restored at end)'
    }
    catch {
        Add-Check 'physical-pixel-dpi-context' 'Incomplete' $_.Exception.Message
        throw 'Cannot guarantee correct physical screenshot bounds on this Windows version/desktop.'
    }
    $report.environment['monitors'] = @([System.Windows.Forms.Screen]::AllScreens | ForEach-Object {
        [ordered]@{ device = $_.DeviceName; primary = $_.Primary; left = $_.Bounds.Left; top = $_.Bounds.Top; width = $_.Bounds.Width; height = $_.Bounds.Height }
    })
    Add-Check 'interactive-host-and-dpi-context' 'Passed' '64-bit interactive session; per-monitor-aware physical-pixel capture enabled. Desktop capture still must succeed independently.'

    $pe = [QuickNestP001.PeReader]::Read($script:ResolvedExe)
    $allDlls = @(@($pe.Imports) + @($pe.DelayImports) | Sort-Object -Unique)
    $dllRecords = @()
    foreach ($dll in $allDlls) {
        # Never load any imported DLL into PowerShell merely to inspect it.
        if ([IO.Path]::GetFileName($dll) -cne $dll -or $dll.Contains(':')) { throw "Unsafe/non-basename DLL import: $dll" }
        $local = Join-Path (Split-Path -Parent $script:ResolvedExe) $dll
        $system = Join-Path ([Environment]::SystemDirectory) $dll
        $resolution = if ($dll -match '^(api|ext)-ms-') { 'Windows API-set contract (virtual DLL)' }
            elseif (Test-Path -LiteralPath $local -PathType Leaf) { $local }
            elseif (Test-Path -LiteralPath $system -PathType Leaf) { $system }
            else { 'Not found beside EXE or in System32; review PATH/SxS/optional delay-load resolution' }
        $dllRecords += [pscustomobject]@{ name = $dll; delayLoaded = ($pe.DelayImports -contains $dll); resolutionHint = $resolution }
    }
    $report.pe = [ordered]@{ machine = $pe.Machine; optionalMagic = $pe.OptionalMagic; subsystem = $pe.Subsystem; imports = @($pe.Imports); delayImports = @($pe.DelayImports); dllResolutionHints = $dllRecords }
    $report.pe | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $script:RunDir 'pe-imports.json') -Encoding UTF8
    Add-Check 'pe-amd64-gui-and-import-table' 'Passed' ("AMD64 PE32+ GUI executable; read {0} unique static/delay-import DLLs. Resolution hints are not a transitive dependency audit." -f $allDlls.Count)

    try {
        $dumpbin = Resolve-ValidationTool $DumpbinPath 'dumpbin.exe'
        $report.tools['dumpbin'] = $dumpbin
        if ($dumpbin) {
            [void] (Invoke-CapturedTool $dumpbin @('/nologo', '/headers', '/imports', $script:ResolvedExe) 'dumpbin.log')
            Add-Check 'dumpbin-cross-check' 'Passed' "Absolute-path dumpbin completed; see dumpbin.log ($dumpbin)."
        }
        else { Write-Log 'dumpbin not installed/discovered; PE headers and DLL imports were independently parsed. No dumpbin check is claimed.' }
    }
    catch { Add-Check 'dumpbin-cross-check' 'Failed' $_.Exception.Message }

    try {
        $mt = Resolve-ValidationTool $MtPath 'mt.exe'
        $report.tools['mt'] = $mt
        if (-not $mt) {
            Add-Check 'embedded-manifest' 'Incomplete' 'Windows SDK mt.exe not found. Install Windows SDK tools or pass -MtPath; no manifest success is claimed.'
        }
        else {
            $manifestPath = Join-Path $script:RunDir 'embedded.manifest'
            [void] (Invoke-CapturedTool $mt @('-nologo', ('-inputresource:' + $script:ResolvedExe + ';#1'), ('-out:' + $manifestPath)) 'mt-extract.log')
            [void] (Invoke-CapturedTool $mt @('-nologo', '-manifest', $manifestPath, '-validate_manifest') 'mt-validate.log')
            $settings = New-Object System.Xml.XmlReaderSettings
            $settings.DtdProcessing = [System.Xml.DtdProcessing]::Prohibit
            $settings.XmlResolver = $null
            $reader = [System.Xml.XmlReader]::Create($manifestPath, $settings)
            $xml = New-Object System.Xml.XmlDocument
            $xml.XmlResolver = $null
            try { $xml.Load($reader) } finally { $reader.Dispose() }
            $ns = New-Object System.Xml.XmlNamespaceManager -ArgumentList $xml.NameTable
            $ns.AddNamespace('a', 'urn:schemas-microsoft-com:asm.v1')
            $ns.AddNamespace('v3', 'urn:schemas-microsoft-com:asm.v3')
            $ns.AddNamespace('dpi05', 'http://schemas.microsoft.com/SMI/2005/WindowsSettings')
            $ns.AddNamespace('dpi16', 'http://schemas.microsoft.com/SMI/2016/WindowsSettings')
            $identity = $xml.SelectSingleNode('/a:assembly/a:assemblyIdentity', $ns)
            $level = $xml.SelectSingleNode('/a:assembly/v3:trustInfo/v3:security/v3:requestedPrivileges/v3:requestedExecutionLevel', $ns)
            $dpiOld = $xml.SelectSingleNode('/a:assembly/v3:application/v3:windowsSettings/dpi05:dpiAware', $ns)
            $dpiNew = $xml.SelectSingleNode('/a:assembly/v3:application/v3:windowsSettings/dpi16:dpiAwareness', $ns)
            if (-not $identity -or $identity.GetAttribute('processorArchitecture') -ne 'amd64') { throw 'Embedded assemblyIdentity must declare amd64.' }
            if (-not $level -or $level.GetAttribute('level') -ne 'asInvoker' -or $level.GetAttribute('uiAccess') -ne 'false') { throw 'Embedded execution level must be asInvoker, uiAccess=false.' }
            if (-not $dpiOld -or $dpiOld.InnerText.Trim() -ne 'true/pm') { throw 'Embedded legacy dpiAware must be true/pm.' }
            if (-not $dpiNew -or $dpiNew.InnerText.Trim() -ne 'PerMonitorV2') { throw 'Embedded dpiAwareness must be PerMonitorV2.' }
            $report.manifest = [ordered]@{ path = $manifestPath; architecture = 'amd64'; executionLevel = 'asInvoker'; uiAccess = $false; dpiAware = $dpiOld.InnerText; dpiAwareness = $dpiNew.InnerText }
            Add-Check 'embedded-manifest' 'Passed' 'mt.exe extracted and validated RT_MANIFEST #1; verified amd64, asInvoker/uiAccess=false and PerMonitorV2/true-pm.'
        }
    }
    catch { Add-Check 'embedded-manifest' 'Failed' $_.Exception.Message }

    Test-SmokeRun
    Test-NativeClose
    $finalHash = (Get-FileHash -LiteralPath $script:ResolvedExe -Algorithm SHA256).Hash
    if ($finalHash -ne $initialHash) { Add-Check 'executable-unchanged' 'Failed' 'EXE SHA256 changed while validation was running; results cannot identify one artifact.' }
    else { Add-Check 'executable-unchanged' 'Passed' "SHA256 unchanged: $initialHash" }
}
catch {
    # If a prior check already describes an incomplete prerequisite, do not turn
    # that into a misleading application failure. Unexpected errors are failures.
    if (@($script:Checks | Where-Object { $_.status -eq 'Incomplete' }).Count -gt 0 -and @($script:Checks | Where-Object { $_.status -eq 'Failed' }).Count -eq 0) {
        Write-Log ('Validation stopped: ' + $_.Exception.Message)
    }
    else { Add-Check 'validation-fatal-error' 'Failed' $_.Exception.ToString() }
}
finally {
    foreach ($entry in $script:Processes) {
        try { Stop-ValidationProcess $entry }
        catch { Add-Check ('cleanup-' + $entry.mode) 'Failed' $_.Exception.Message }
    }
    if ($dpiContextChanged) {
        try { [void] [QuickNestP001.Native]::SetThreadDpiAwarenessContext($oldDpiContext) }
        catch { Add-Check 'restore-thread-dpi-context' 'Failed' $_.Exception.Message }
    }
    $failedCount = @($script:Checks | Where-Object { $_.status -eq 'Failed' }).Count
    $incompleteCount = @($script:Checks | Where-Object { $_.status -eq 'Incomplete' }).Count
    if ($failedCount -gt 0) { $exitCode = 1; $report.overall = 'Failed' }
    elseif ($incompleteCount -gt 0 -or $script:Checks.Count -eq 0) { $exitCode = 2; $report.overall = 'Incomplete' }
    else { $exitCode = 0; $report.overall = 'AutomatedPassedManualPending' }
    $report.exitCode = $exitCode
    $report.finishedUtc = [DateTime]::UtcNow.ToString('o')
    $report.checks = $script:Checks.ToArray()
    $report.transitions = $script:Transitions.ToArray()
    $report.screenshots = $script:Screenshots.ToArray()
    $report.processes = @($script:Processes | ForEach-Object {
        [ordered]@{ mode = $_.mode; targetPid = $_.targetPid; hwnd = $_.hwnd; integrity = $_.integrity; exitCode = $_.exitCode; forcedTermination = $_.forcedTermination }
    })
    if ($script:RunDir) {
        try {
            $script:Samples.ToArray() | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $script:RunDir 'window-samples.json') -Encoding UTF8
            $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $script:RunDir 'results.json') -Encoding UTF8
            Write-Log ("Overall={0}; exitCode={1}; manual items remain pending. Results: {2}" -f $report.overall, $exitCode, $script:RunDir)
        }
        catch { Write-Warning ('Failed to persist final results: ' + $_.Exception.Message); $exitCode = 1 }
    }
    foreach ($entry in $script:Processes) { $entry.clock.Stop(); $entry.process.Dispose() }
}
exit $exitCode
