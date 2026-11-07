#requires -Version 5.1
[CmdletBinding()]
param(
 [Parameter(Mandatory=$true)][string]$RunDir,
 [Parameter(Mandatory=$true)][long]$SourceHwnd,
 [Parameter(Mandatory=$true)][long]$TargetHwnd,
 [Parameter(Mandatory=$true)][int]$SourceY,
 [Parameter(Mandatory=$true)][int]$TargetX,
 [Parameter(Mandatory=$true)][int]$TargetY,
 [ValidateSet('Drop','Cancel','Leave','Spring')][string]$Action='Drop',
 [int]$HoverMs=600,
 [string]$Capture
)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
. (Join-Path $PSScriptRoot 'p1-17-focus.ps1')

$source=@([P005Desktop]::Windows() | Where-Object {$_.hwnd -eq $SourceHwnd})[0]
$target=@([P005Desktop]::Windows() | Where-Object {$_.hwnd -eq $TargetHwnd})[0]
if($null -eq $source){throw 'Explorer window not found'}
if($null -eq $target){throw 'QuickNest window not found'}

function Require-Hit([int]$X,[int]$Y,[long]$Expected){
 $p=New-Object P005Desktop+Point;$p.X=$X;$p.Y=$Y
 if([P005Desktop]::GetAncestor([P005Desktop]::WindowFromPoint($p),2).ToInt64() -ne $Expected){throw "Observed pointer at $X,$Y is covered by another window"}
}

$sourceX=420
Focus-P117Window $TargetHwnd
Focus-P117Window $SourceHwnd
Start-Sleep -Milliseconds 200
Require-Hit $sourceX $SourceY $SourceHwnd
Require-Hit $TargetX $TargetY $TargetHwnd
Start-Sleep -Milliseconds 100
if($Capture){Save-P005Screenshot -Path (Join-Path $RunDir ("src-"+$Capture)) -WindowHwnd $SourceHwnd}
[void][P005Desktop]::SetCursorPos($sourceX,$SourceY)
Start-Sleep -Milliseconds 100
[P005Desktop]::MouseButton($true)
$visibleDuringHover = $null
try{
 Start-Sleep -Milliseconds 160
 for($i=1;$i -le 30;$i++){
  [void][P005Desktop]::SetCursorPos(($sourceX+[int](($TargetX-$sourceX)*$i/30)),($SourceY+[int](($TargetY-$SourceY)*$i/30)))
  Start-Sleep -Milliseconds 25
 }
 Start-Sleep -Milliseconds 250
 if($Capture -and $Action -ne 'Spring'){Save-P005Screenshot -Path (Join-Path $RunDir $Capture) -WindowHwnd $TargetHwnd}
 if($Action -eq 'Spring'){
  Start-Sleep -Milliseconds $HoverMs
  $visibleDuringHover = [P117Focus]::IsWindowVisible([IntPtr]$TargetHwnd)
  if($Capture){Save-P005Screenshot -Path (Join-Path $RunDir $Capture) -WindowHwnd $TargetHwnd}
 }elseif($Action -eq 'Cancel'){
  [P005Desktop]::Key(27,$true);Start-Sleep -Milliseconds 150;[P005Desktop]::Key(27,$false);Start-Sleep -Milliseconds 400
 }elseif($Action -eq 'Leave'){
  for($i=1;$i -le 16;$i++){[void][P005Desktop]::SetCursorPos((1650-[int](650*$i/16)),(800+[int](60*$i/16)));Start-Sleep -Milliseconds 20}
  [P005Desktop]::Key(27,$true);Start-Sleep -Milliseconds 150;[P005Desktop]::Key(27,$false);Start-Sleep -Milliseconds 400
 }
}finally{[P005Desktop]::MouseButton($false)}
Start-Sleep -Milliseconds 700
$record=[ordered]@{action=$Action;sourceHwnd=$SourceHwnd;sourceY=$SourceY;targetHwnd=$TargetHwnd;targetX=$TargetX;targetY=$TargetY;visibleDuringHover=$visibleDuringHover;timeUtc=[DateTime]::UtcNow.ToString('o')}
$record | ConvertTo-Json -Compress | Add-Content (Join-Path $RunDir 'drags.jsonl') -Encoding UTF8
