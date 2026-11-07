#requires -Version 5.1
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$RunDir,
 [Parameter(Mandatory=$true)][long]$SourceHwnd,
 [Parameter(Mandatory=$true)][long]$TargetHwnd,
 [Parameter(Mandatory=$true)][int]$SourceY,
 [ValidateSet('Drop','Leave','Cancel')][string]$Action='Drop',
 [string]$Capture
)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
$windows=[P005Desktop]::Windows()
$source=@($windows | Where-Object {$_.hwnd -eq $SourceHwnd})[0]
$target=@($windows | Where-Object {$_.hwnd -eq $TargetHwnd})[0]
if($source.integrity -ne 8192 -or $target.integrity -ne 8192){throw 'Both actual windows must use Medium integrity'}
# Layout was observed from ready.png / exe-dropped.png; no product hit testing is inferred.
[void][P005Desktop]::SetWindowPos([IntPtr]$SourceHwnd,[IntPtr](-1),10,40,900,760,0x0050)
[void][P005Desktop]::SetWindowPos([IntPtr]$TargetHwnd,[IntPtr](-1),940,40,800,650,0x0050)
Start-Sleep -Milliseconds 250
function Require-Hit([int]$X,[int]$Y,[long]$Expected){$p=New-Object P005Desktop+Point;$p.X=$X;$p.Y=$Y;if([P005Desktop]::GetAncestor([P005Desktop]::WindowFromPoint($p),2).ToInt64() -ne $Expected){throw 'Observed pointer position is covered by another window'}}
Require-Hit 260 $SourceY $SourceHwnd
Require-Hit 1200 350 $TargetHwnd
$before=@(Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8 | Where-Object {$_ -match "`tdrop`t"}).Count
[void][P005Desktop]::SetForegroundWindow([IntPtr]$SourceHwnd)
[void][P005Desktop]::SetCursorPos(260,$SourceY)
[P005Desktop]::MouseButton($true)
try{
 Start-Sleep -Milliseconds 160
 for($i=1;$i -le 30;$i++){[void][P005Desktop]::SetCursorPos((260+[int](940*$i/30)),($SourceY+[int]((350-$SourceY)*$i/30)));Start-Sleep -Milliseconds 25}
 Start-Sleep -Milliseconds 250
 if($Action -eq 'Leave'){
  for($i=1;$i -le 20;$i++){[void][P005Desktop]::SetCursorPos((1200-[int](480*$i/20)),(350+[int](500*$i/20)));Start-Sleep -Milliseconds 20}
  [P005Desktop]::Key(27,$true);Start-Sleep -Milliseconds 150;[P005Desktop]::Key(27,$false);Start-Sleep -Milliseconds 600
 }elseif($Action -eq 'Cancel'){[P005Desktop]::Key(27,$true);Start-Sleep -Milliseconds 150;[P005Desktop]::Key(27,$false);Start-Sleep -Milliseconds 600}
}finally{[P005Desktop]::MouseButton($false)}
Start-Sleep -Milliseconds 500
$after=@(Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8 | Where-Object {$_ -match "`tdrop`t"}).Count
if($Action -eq 'Drop' -and $after -ne $before+1){throw 'Expected exactly one real Drop callback'}
if($Action -ne 'Drop' -and $after -ne $before){throw 'Leave/cancel unexpectedly submitted data'}
$record=[ordered]@{dropCallbacksBefore=$before;dropCallbacksAfter=$after;action=$Action;sourceHwnd=$SourceHwnd;sourcePid=$source.pid;sourceIntegrity=$source.integrity;targetHwnd=$TargetHwnd;targetPid=$target.pid;targetIntegrity=$target.integrity;sourceY=$SourceY;timeUtc=[DateTime]::UtcNow.ToString('o')}
$record | ConvertTo-Json -Compress | Add-Content (Join-Path $RunDir 'pointer-actions.jsonl') -Encoding UTF8
if($Capture){Save-P005Screenshot (Join-Path $RunDir $Capture) -WindowHwnd $TargetHwnd}
Get-Content (Join-Path $RunDir 'events.tsv') -Tail 8
