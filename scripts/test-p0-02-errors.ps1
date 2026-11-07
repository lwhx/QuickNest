#requires -Version 5.1
[CmdletBinding()]
param([string] $ResultsDir)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ResultsDir) { $ResultsDir = Join-Path $root 'docs\test-results\P0-02\errors' }
$run = Join-Path $ResultsDir ([Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($run)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class P002ErrorInput {
    // Test-only close request to the exact child window; no arbitrary window mutation.
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
}
'@
$checks = @()
foreach ($case in @('existing-report', 'destroy-on-hide-override', 'premature-close')) {
    $path = Join-Path $run ($case + '.tsv')
    if ($case -eq 'existing-report') { [IO.File]::WriteAllText($path, 'preserve-existing-report') }
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = Join-Path $root 'target\release\quicknest.exe'
    $info.Arguments = '--hwnd-test "' + $path + '"'
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    if ($case -eq 'destroy-on-hide-override') { $info.EnvironmentVariables['SLINT_DESTROY_WINDOW_ON_HIDE'] = '0' }
    else { $info.EnvironmentVariables.Remove('SLINT_DESTROY_WINDOW_ON_HIDE') }
    $p = New-Object Diagnostics.Process
    $p.StartInfo = $info
    try {
        [void]$p.Start()
        $out = $p.StandardOutput.ReadToEndAsync()
        $err = $p.StandardError.ReadToEndAsync()
        if ($case -eq 'premature-close') {
            $clock = [Diagnostics.Stopwatch]::StartNew()
            do { $p.Refresh(); if($p.HasExited){throw 'Exited before close request'}; if($p.MainWindowHandle -ne [IntPtr]::Zero){break}; Start-Sleep -Milliseconds 10 } while($clock.ElapsedMilliseconds -lt 3000)
            if($p.MainWindowHandle -eq [IntPtr]::Zero){throw 'No window for premature-close test'}
            Start-Sleep -Milliseconds 100
            if(-not [P002ErrorInput]::PostMessage($p.MainWindowHandle,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Could not send test close request'}
        }
        if(-not $p.WaitForExit(8000)){throw 'Error case did not exit within eight seconds'}
        [void]$out.Wait(2000);[void]$err.Wait(2000)
        [IO.File]::WriteAllText((Join-Path $run ($case + '.stderr.log')), $err.Result)
        $preserved = $case -ne 'existing-report' -or [IO.File]::ReadAllText($path) -ceq 'preserve-existing-report'
        $checks += [pscustomobject]@{case=$case; passed=($p.ExitCode -ne 0 -and $preserved); exit_code=$p.ExitCode; existing_report_preserved=$preserved; stderr=$err.Result}
    } finally { if(-not $p.HasExited){$p.Kill();[void]$p.WaitForExit(3000)}; $p.Dispose() }
}
$checks | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $run 'results.json') -Encoding UTF8
$checks | Format-Table case,passed,exit_code,existing_report_preserved
Write-Host "Results: $run"
if(@($checks | Where-Object {-not $_.passed}).Count){exit 1}
