#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string] $ResultsDir,
    [string] $ExePath
)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$exe = if ($ExePath) { (Resolve-Path -LiteralPath $ExePath).Path } else { Join-Path $root 'target\release\quicknest.exe' }
[void][IO.Directory]::CreateDirectory($ResultsDir)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class P001ButtonInput {
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsHungAppWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    public static void ClickClient(IntPtr window,int x,int y) {
        IntPtr point=new IntPtr((y<<16)|(x&0xffff));
        if(!PostMessage(window,0x0200,IntPtr.Zero,point) || !PostMessage(window,0x0201,new IntPtr(1),point) || !PostMessage(window,0x0202,IntPtr.Zero,point))
            throw new InvalidOperationException("Could not dispatch test mouse input");
    }
}
'@
$process = [Diagnostics.Process]::Start($exe)
$report = [ordered]@{sha256=(Get-FileHash $exe).Hash; targetPid=$process.Id; hwnd=$null; dpi=$null; input='Window mouse messages at button centers observed in the accepted 762x552 screenshot'; hiddenObserved=$false; restoredObserved=$false; exitCode=$null; status='Failed'; error=$null}
try {
    $wait = [Diagnostics.Stopwatch]::StartNew()
    do { $process.Refresh(); if($process.HasExited){throw 'Exited during startup'}; $handle=$process.MainWindowHandle; if($handle -ne [IntPtr]::Zero){break}; Start-Sleep -Milliseconds 50 } while($wait.Elapsed.TotalSeconds -lt 5)
    if($handle -eq [IntPtr]::Zero){throw 'No native window found'}
    $report.hwnd=$handle.ToInt64();$report.dpi=[P001ButtonInput]::GetDpiForWindow($handle)
    if($report.dpi -ne 96){throw 'Observed screenshot coordinates apply only to DPI 96'}
    Start-Sleep -Milliseconds 300
    $report['loadedModules'] = @($process.Modules | ForEach-Object { $_.FileName })
    $report['childProcesses'] = @(Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $process.Id } | Select-Object Name,ProcessId)
    # Screenshot: hide button center (217,482), client origin (1,31).
    [P001ButtonInput]::ClickClient($handle,216,451)
    $timer=[Diagnostics.Stopwatch]::StartNew()
    while($timer.Elapsed.TotalSeconds -lt 1){if(-not [P001ButtonInput]::IsWindowVisible($handle)){$report.hiddenObserved=$true;break};Start-Sleep -Milliseconds 20}
    if(-not $report.hiddenObserved){throw 'Hide button did not hide the window'}
    while($timer.Elapsed.TotalSeconds -lt 4){if([P001ButtonInput]::IsWindowVisible($handle)){$report.restoredObserved=$true;break};Start-Sleep -Milliseconds 20}
    if(-not $report.restoredObserved){throw 'Window did not restore after hide button'}
    $report['restoreSeconds']=$timer.Elapsed.TotalSeconds
    $process.Refresh()
    if($process.MainWindowHandle -ne $handle){throw 'Window handle changed after button restore'}
    if([P001ButtonInput]::IsHungAppWindow($handle)){throw 'Restored window is reported hung'}
    # Screenshot: exit button center (580,482), same client origin.
    [P001ButtonInput]::ClickClient($handle,579,451)
    if(-not $process.WaitForExit(5000)){throw 'Exit button did not terminate the process'}
    $report.exitCode=$process.ExitCode
    if($report.exitCode -ne 0){throw 'Exit button returned nonzero status'}
    $report.status='Passed'
} catch { $report.error=$_.Exception.Message }
finally {
    if(-not $process.HasExited){$process.Kill();[void]$process.WaitForExit(3000);$report['forcedTermination']=$true}
    $report | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $ResultsDir 'button-results.json') -Encoding UTF8
    $process.Dispose()
}
$report | ConvertTo-Json -Depth 5
if($report.status -ne 'Passed'){exit 1}
