#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RunDir,[Parameter(Mandatory=$true)][long]$SourceHwnd,[Parameter(Mandatory=$true)][long]$TargetHwnd)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
$launch=Get-Content (Join-Path $RunDir 'launch.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$windows=@([P005Desktop]::Windows() | Where-Object {$_.hwnd -in @($SourceHwnd,$TargetHwnd)})
if($windows.Count -ne 2 -or @($windows | Where-Object {$_.integrity -ne 8192}).Count){throw 'Both test windows must still be Medium'}
$target=@($windows | Where-Object {$_.hwnd -eq $TargetHwnd})[0]
$source=@($windows | Where-Object {$_.hwnd -eq $SourceHwnd})[0]
if($target.pid -ne $launch.target_pid){throw 'Target window ownership changed'}
$windows | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $RunDir 'window-tokens-final.json') -Encoding UTF8
$before=Get-Content (Join-Path $RunDir 'fixtures.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$after=@($before | ForEach-Object {
 $entry=$_;$item=Get-Item -LiteralPath $entry.path
 $hash=if($item.PSIsContainer){$null}else{(Get-FileHash -LiteralPath $entry.path).Hash}
 [pscustomobject]@{path=$entry.path;directory=$item.PSIsContainer;sha256_before=$entry.sha256;sha256_after=$hash;length_before=$entry.length;length_after=if($item.PSIsContainer){0}else{$item.Length};unchanged=if($item.PSIsContainer){$entry.directory -and @(Get-ChildItem -LiteralPath $entry.path -Force).Count -eq 0}else{$hash -ceq $entry.sha256 -and $item.Length -eq $entry.length}}
})
$after | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $RunDir 'fixtures-after.json') -Encoding UTF8
if(@($after | Where-Object {-not $_.unchanged}).Count){throw 'Fixture changed'}
$process=Get-Process -Id $target.pid
[void]$process.Handle
$responsive=@(Get-Process -Id $source.pid,$target.pid | Select-Object Id,Responding)
$root=Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$hash=(Get-FileHash (Join-Path $root 'target\release\quicknest.exe')).Hash
if($hash -cne $launch.sha256){throw 'Release EXE changed during actual test'}
if(-not [P005Desktop]::PostMessage([IntPtr]$TargetHwnd,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Could not close target normally'}
if(-not $process.WaitForExit(5000)){throw 'Target failed to exit normally'}
$exit=[ordered]@{targetPid=$target.pid;sourcePid=$source.pid;exitCode=$process.ExitCode;forcedTermination=$false;responsiveBeforeExit=$responsive;sha256=$hash}
$exit | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $RunDir 'exit.json') -Encoding UTF8
[void][P005Desktop]::SetWindowPos([IntPtr]$SourceHwnd,[IntPtr](-2),0,0,0,0,0x0013)
[void][P005Desktop]::PostMessage([IntPtr]$SourceHwnd,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)
$exit | ConvertTo-Json -Depth 4
Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8 -Tail 3
if($process.ExitCode -ne 0){exit 1}
