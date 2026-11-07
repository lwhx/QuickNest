#requires -Version 5.1
[CmdletBinding()]
param(
    [string] $ResultsDir,
    [ValidateSet('FileDrop','Dpi')]
    [string] $Mode = 'FileDrop'
)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$root=Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if(-not $ResultsDir){$ResultsDir=Join-Path $root 'docs\test-results\P0-05'}
$run=Join-Path $ResultsDir ('interactive-'+[Guid]::NewGuid().ToString('N').Substring(0,8))
$fixtures=Join-Path $run 'fixtures'
[void][IO.Directory]::CreateDirectory($fixtures)
# All fixtures are local references. Neither the sample EXE nor shortcut is executed.
$unicode=[string]([char]0x4E2D)+[char]0x6587
[IO.File]::WriteAllText((Join-Path $fixtures ($unicode+' sample.txt')),'QuickNest reference-only drop fixture')
[IO.File]::WriteAllText((Join-Path $fixtures 'second file.txt'),'Second reference-only fixture')
Copy-Item (Join-Path $env:WINDIR 'System32\notepad.exe') (Join-Path $fixtures 'sample application.exe')
[void][IO.Directory]::CreateDirectory((Join-Path $fixtures ($unicode+' folder')))
$shortcut=(New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $fixtures ($unicode+' link.lnk')))
$shortcut.TargetPath=Join-Path $env:WINDIR 'System32\notepad.exe';$shortcut.Save()
Get-ChildItem -LiteralPath $fixtures | ForEach-Object {[pscustomobject]@{path=$_.FullName;directory=$_.PSIsContainer;length=if($_.PSIsContainer){0}else{$_.Length};sha256=if($_.PSIsContainer){$null}else{(Get-FileHash $_.FullName).Hash}}} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $run 'fixtures.json') -Encoding UTF8
. (Join-Path $root 'scripts\run-p0-01-medium-validation.ps1') -LoadLauncherOnly
$app=Join-Path $root 'target\release\quicknest.exe'
$flag=if($Mode -eq 'Dpi'){'--dpi-test'}else{'--file-drop-test'}
$targetPid=[P001MediumLauncher]::Launch($app,($flag+' "'+(Join-Path $run 'events.tsv')+'"'),$root)
$explorer=Join-Path $env:WINDIR 'explorer.exe'
$sourcePid=[P001MediumLauncher]::Launch($explorer,('/separate,"'+$fixtures+'"'),$fixtures)
$info=[ordered]@{run=$run;fixtures=$fixtures;target_pid=$targetPid;requested_explorer_pid=$sourcePid;sha256=(Get-FileHash $app).Hash;note='Requested Explorer PID may delegate to an existing process. Inspect actual window owner and both tokens.'}
$info | ConvertTo-Json | Set-Content (Join-Path $run 'launch.json') -Encoding UTF8
$info | ConvertTo-Json
