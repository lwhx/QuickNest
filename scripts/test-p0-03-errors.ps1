#requires -Version 5.1
[CmdletBinding()]
param(
    [string] $ResultsDir,
    [ValidateSet('Ole', 'DropTarget', 'FileDrop')]
    [string] $Mode = 'Ole'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ResultsDir) { $ResultsDir = Join-Path $root 'docs\test-results\P0-03\errors' }
$run = Join-Path $ResultsDir ([Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($run)
$checks=@()
$cases=if($Mode -eq 'FileDrop'){@('existing-report','missing-parent','missing-argument')}else{@('existing-report','existing-hwnd-report','missing-parent','missing-argument')}
foreach ($case in $cases) {
    $path=Join-Path $run ($case+'.tsv')
    $protected=$null
    if($case -eq 'existing-report'){$protected=$path}
    if($case -eq 'existing-hwnd-report'){$protected=[IO.Path]::ChangeExtension($path,'hwnd.tsv')}
    if($protected){[IO.File]::WriteAllText($protected,'preserve-existing-report')}
    if($case -eq 'missing-parent'){$path=Join-Path $run 'absent\report.tsv'}
    $info=New-Object Diagnostics.ProcessStartInfo
    $info.FileName=Join-Path $root 'target\release\quicknest.exe'
    $flag=if($Mode -eq 'DropTarget'){'--drop-target-test'}elseif($Mode -eq 'FileDrop'){'--file-drop-test'}else{'--ole-test'}
    $info.Arguments=if($case -eq 'missing-argument'){$flag}else{$flag+' "'+$path+'"'}
    $info.UseShellExecute=$false;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $p=New-Object Diagnostics.Process;$p.StartInfo=$info
    try {
        [void]$p.Start();$out=$p.StandardOutput.ReadToEndAsync();$err=$p.StandardError.ReadToEndAsync()
        if(-not $p.WaitForExit(8000)){throw 'Error case timed out'}
        [void]$out.Wait(2000);[void]$err.Wait(2000)
        [IO.File]::WriteAllText((Join-Path $run ($case+'.stderr.log')),$err.Result)
        $preserved=(-not $protected) -or [IO.File]::ReadAllText($protected) -ceq 'preserve-existing-report'
        $checks+=[pscustomobject]@{case=$case;passed=($p.ExitCode -ne 0 -and $preserved);exit_code=$p.ExitCode;protected_report_preserved=$preserved;stderr=$err.Result}
    }finally{if(-not $p.HasExited){$p.Kill();[void]$p.WaitForExit(3000)};$p.Dispose()}
}
$checks | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $run 'results.json') -Encoding UTF8
$checks | Format-Table case,passed,exit_code,protected_report_preserved
Write-Host "Results: $run"
if(@($checks | Where-Object {-not $_.passed}).Count){exit 1}
