#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][long]$SourceHwnd,[Parameter(Mandatory=$true)][long]$TargetHwnd,[Parameter(Mandatory=$true)][string]$RunDir,[Parameter(Mandatory=$true)][int]$ExpectedDpi)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
. (Join-Path $PSScriptRoot 'p0-06-native.ps1')
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$target=[P006Native]::Read($TargetHwnd)
if($target.dpi -ne $ExpectedDpi -or -not $target.pmv2 -or $target.iconic){throw 'Target DPI/context/visibility mismatch'}
$source=[P006Native]::Read($SourceHwnd)
if($source.iconic){throw 'Source is minimized'}
if([P005Desktop]::Rid([int]$source.pid) -ne 8192 -or [P005Desktop]::Rid([int]$target.pid) -ne 8192){throw 'Actual source and target must be Medium'}
$shell=New-Object -ComObject Shell.Application
$view=@($shell.Windows() | Where-Object {try{$_.HWND -eq $SourceHwnd}catch{$false}})[0].Document
$view.SelectItem($view.Folder.ParseName('sample application.exe'),29)
$root=[Windows.Automation.AutomationElement]::FromHandle([IntPtr]$SourceHwnd)
$item=@($root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition) | Where-Object {$_.Current.Name -like 'sample application*' -and $_.Current.ControlType -eq [Windows.Automation.ControlType]::ListItem})[0]
$bounds=$item.Current.BoundingRectangle
# UIA row extends into the horizontally clipped list. Intersect it with the actual
# client rectangle, rather than using its off-window center or virtualized GetWindowRect.
$source=[P006Native]::Read($SourceHwnd)
$left=[Math]::Max($bounds.Left,$source.client.left)
$right=[Math]::Min($bounds.Right,$source.client.right)
$top=[Math]::Max($bounds.Top,$source.client.top)
$bottom=[Math]::Min($bounds.Bottom,$source.client.bottom)
if($right-$left -lt 12 -or $bottom-$top -lt 4){throw 'Source item has no visible intersection'}
$sx=[int]($left+[Math]::Min(80,($right-$left)/2));$sy=[int](($top+$bottom)/2)
$tx=$target.client.left+[int](($target.client.right-$target.client.left)*0.6)
$ty=$target.client.top+[int](($target.client.bottom-$target.client.top)*0.55)
if([P006Native]::Hit($sx,$sy) -ne $SourceHwnd){throw "Source obscured at physical $sx,$sy"}
if([P006Native]::Hit($tx,$ty) -ne $TargetHwnd){throw "Target obscured at physical $tx,$ty"}
$before=@(Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8 | Where-Object {$_ -match "`tdrop`t"}).Count
[void][P005Desktop]::SetForegroundWindow([IntPtr]$SourceHwnd)
[P006Native]::Move($sx,$sy);[P005Desktop]::MouseButton($true)
try{
 Start-Sleep -Milliseconds 180
 for($i=1;$i -le 35;$i++){[P006Native]::Move(($sx+[int](($tx-$sx)*$i/35)),($sy+[int](($ty-$sy)*$i/35)));Start-Sleep -Milliseconds 25}
 Start-Sleep -Milliseconds 350
}finally{[P005Desktop]::MouseButton($false)}
Start-Sleep -Milliseconds 500
$events=Get-Content (Join-Path $RunDir 'events.tsv') -Encoding UTF8
$after=@($events | Where-Object {$_ -match "`tdrop`t"}).Count
if($after -ne $before+1){throw 'Expected exactly one actual Explorer Drop callback'}
$matches=@($events | Where-Object {$_ -match "`tcoordinate`tphase=drop .*dpi=$ExpectedDpi "})
if(-not $matches.Count){throw 'No actual Drop DPI record for the requested scale'}
$sample=$matches[-1]
if($sample -notmatch 'pmv2=true .*sizes_match=true scale_match=true'){throw 'DPI/context/size cross-check failed'}
[ordered]@{dpi=$target.dpi;sourceHwnd=$SourceHwnd;sourcePid=$source.pid;targetHwnd=$TargetHwnd;targetPid=$target.pid;sourceIntegrity=8192;targetIntegrity=8192;sourcePoint=@($sx,$sy);targetPoint=@($tx,$ty);uiaSourceRow=$bounds.ToString();sourcePhysicalClient=$source.client;dropBefore=$before;dropAfter=$after;coordinate=$sample;method='Actual Explorer with physical pointer input; visible source row/client intersection'} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $RunDir ("actual-drop-$ExpectedDpi.json")) -Encoding UTF8
Save-P005Screenshot (Join-Path $RunDir ("actual-drop-$ExpectedDpi.png")) -WindowHwnd $TargetHwnd
Write-Output $sample
