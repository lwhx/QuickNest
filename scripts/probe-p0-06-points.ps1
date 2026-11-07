#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][long]$TargetHwnd,[Parameter(Mandatory=$true)][string]$RunDir,[int]$ExpectedDpi=96,[string]$Label='stable')
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
. (Join-Path $PSScriptRoot 'p0-06-native.ps1')
$geometry=[P006Native]::Read($TargetHwnd)
if($geometry.dpi -ne $ExpectedDpi -or -not $geometry.pmv2 -or $geometry.iconic){throw 'Actual target DPI/context/visibility does not match test requirements'}
$w=$geometry.client.right-$geometry.client.left;$h=$geometry.client.bottom-$geometry.client.top
$points=@(@(2,2),@(($w-3),2),@(2,($h-3)),@(($w-3),($h-3)),@(([int][Math]::Floor($w/2)-1),([int][Math]::Floor($h/2)-1)),@([int][Math]::Ceiling($w/2),[int][Math]::Ceiling($h/2)),@(101,203))
$requested=@()
[void][P005Desktop]::SetForegroundWindow([IntPtr]$TargetHwnd)
foreach($xy in $points){
 $x=$geometry.client.left+$xy[0];$y=$geometry.client.top+$xy[1]
 if([P006Native]::Hit($x,$y) -ne $TargetHwnd){throw "Reference point obscured/offscreen at physical $x,$y"}
 [P006Native]::Move($x,$y);Start-Sleep -Milliseconds 220
 $requested+=[ordered]@{screenX=$x;screenY=$y;clientX=$xy[0];clientY=$xy[1];dpi=$geometry.dpi;physicalClient=$geometry.client}
}
$requested | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $RunDir ("pointer-requested-$ExpectedDpi-$Label.json")) -Encoding UTF8
Save-P005Screenshot (Join-Path $RunDir ("pointer-reference-$ExpectedDpi-$Label.png")) -WindowHwnd $TargetHwnd
Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8 | Where-Object {$_ -match "`treference`t"} | Select-Object -Last 10
