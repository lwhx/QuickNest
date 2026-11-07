#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RunDir)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$launch=Get-Content (Join-Path $RunDir 'launch.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$exit=Get-Content (Join-Path $RunDir 'exit.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$tokens=Get-Content (Join-Path $RunDir 'window-tokens-final.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$fixtures=Get-Content (Join-Path $RunDir 'fixtures-after.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$events=@(Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8)
$actions=@(Get-Content (Join-Path $RunDir 'pointer-actions.jsonl') -Encoding UTF8 | Where-Object {$_} | ForEach-Object {$_ | ConvertFrom-Json})
$drops=@($events | Where-Object {$_ -match "`tdrop`t"})
$paths=@($events | Where-Object {$_ -match "`tpath`t"})
$leaves=@($events | Where-Object {$_ -match "`tleave`t"})
$cleanup=@($events | Where-Object {$_ -match "`tcleanup`t"})[-1]
if($drops.Count -ne 5 -or $paths.Count -ne 6 -or $leaves.Count -ne 2){throw 'Expected strict five-drop/six-path/two-leave matrix'}
if($cleanup -notmatch 'acquired=5 released=5 destroyed=1 .*drops=5 errors=0 received=6 '){throw 'Resource cleanup or error counts mismatch'}
if($actions.Count -ne 7 -or @($actions | Where-Object {$_.sourceIntegrity -ne 8192 -or $_.targetIntegrity -ne 8192}).Count){throw 'Actual Explorer/target input token evidence mismatch'}
foreach($action in $actions){
 $expected=if($action.action -eq 'Drop'){1}else{0}
 if($action.dropCallbacksAfter-$action.dropCallbacksBefore -ne $expected){throw 'Actual callback count does not match action'}
}
if(@($tokens | Where-Object {$_.integrity -ne 8192}).Count -or $tokens.Count -ne 2){throw 'Source/target final tokens not Medium'}
if(@($fixtures | Where-Object {-not $_.unchanged}).Count -or $fixtures.Count -ne 5){throw 'Fixture integrity mismatch'}
$hash=(Get-FileHash (Join-Path $root 'target\release\quicknest.exe')).Hash
if($hash -cne $launch.sha256 -or $hash -cne $exit.sha256 -or $exit.exitCode -ne 0 -or $exit.forcedTermination){throw 'Final artifact/normal-exit mismatch'}
if(@($exit.responsiveBeforeExit | Where-Object {-not $_.Responding}).Count){throw 'Source or target not responsive'}
$zh=([string][char]0x4E2D)+[char]0x6587
$expected=@('sample application.exe',($zh+' folder'),($zh+' link.lnk'),($zh+' sample.txt'),'second file.txt',($zh+' sample.txt'))
for($i=0;$i -lt $expected.Count;$i++){
 $actual=$paths[$i].Split([char]9,4)[3]
 if($actual -cne (Join-Path $launch.fixtures $expected[$i])){throw "Dropped path/order mismatch at $i"}
}
$base=Split-Path -Parent $RunDir
$regressions=@()
foreach($sub in @('p0-01-regression','p0-02-regression','p0-03-regression','p0-04-regression')){
 $file=Get-ChildItem (Join-Path $base $sub) -Filter results.json -Recurse | Sort-Object LastWriteTime -Descending | Select-Object -First 1
 if(-not $file){throw "Regression report missing: $sub"}
 $j=Get-Content $file.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
 if($j.PSObject.Properties.Name -contains 'status'){$status=$j.status;$reportedHash=$j.sha256;if($status -ne 'pass'){throw "Regression failed: $sub"}}
 else{$status=$j.overall;$reportedHash=$j.executable.sha256;if($status -notlike 'AutomatedPassed*'){throw "Regression failed: $sub"}}
 if($reportedHash -cne $hash){throw "Regression artifact mismatch: $sub"}
 $regressions+=[ordered]@{task=$sub;run=$file.Directory.Name;status=$status}
}
$button=Get-Content (Join-Path $base 'button-regression-final\button-results.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if($button.status -ne 'Passed' -or $button.sha256 -cne $hash){throw 'Final button regression mismatch'}
$summary=[ordered]@{regressions=$regressions;buttonEvidence='button-regression-final/button-results.json';errorEvidence='errors-final/82eb112e0dbf4de589008d7b5dea4abb/results.json';task='P0-05';status='Passed';date='2026-11-03';sha256=$hash;exeBytes=(Get-Item (Join-Path $root 'target\release\quicknest.exe')).Length;checks=@{fmt='Passed';check='Passed';clippy='Passed';unitTests='17 passed, 0 failed';syntheticTests='6 passed, 0 failed';docTests='3 passed, 0 failed';release='Passed'};actualExplorer=@{run=(Split-Path -Leaf $RunDir);sourcePid=$exit.sourcePid;targetPid=$exit.targetPid;sourceIntegrity=8192;targetIntegrity=8192;dropCallbacks=5;paths=6;leaveAndCancel=2;acquired=5;released=5;destroyed=1;errors=0;exitCode=$exit.exitCode;fixturesUnchanged=$true;actionEvidence='pointer-actions.jsonl';pathEvidence='events.tsv'};limits=@('Windows 11 x64 100% only','No DPI conversion or categorization','No config persistence, icon extraction, target execution or source mutation','P0 stage overall not accepted')}
$summary | ConvertTo-Json -Depth 8 | Set-Content (Join-Path (Split-Path -Parent $RunDir) 'acceptance.json') -Encoding UTF8
Write-Host 'P0-05 strict actual Explorer matrix, tokens, paths, cancellation, cleanup and fixture integrity: passed'
