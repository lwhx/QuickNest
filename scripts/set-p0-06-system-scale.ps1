#requires -Version 5.1
[CmdletBinding()]
param([ValidateSet(100,125,150,175,200)][int]$Percent=100,[switch]$ListOnly)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$id='SystemSettings_Display_Scaling_ItemSizeOverride_ComboBox'
$condition=New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::AutomationIdProperty,$id)
$combo=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)
if(-not $combo){Start-Process 'ms-settings:display';Start-Sleep -Milliseconds 1200;$combo=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Descendants,$condition)}
if(-not $combo){throw 'Actual system display scaling control unavailable'}
$expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern)
$expand.Expand()
Start-Sleep -Milliseconds 250
$items=@($combo.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition) | Where-Object {$_.Current.ControlType -eq [Windows.Automation.ControlType]::ListItem})
if(-not $items.Count){$items=@([Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ControlTypeProperty,[Windows.Automation.ControlType]::ListItem))) | Where-Object {$_.Current.Name -match '^(100|125|150|175|200)%'})}
$names=@($items | ForEach-Object {$_.Current.Name} | Select-Object -Unique)
Write-Output ('Actual system scale options: '+($names -join ', '))
if($ListOnly){$expand.Collapse();return}
$choice=@($items | Where-Object {$_.Current.Name -match ('^'+$Percent+'%')}) | Select-Object -First 1
if(-not $choice){$expand.Collapse();throw "System Settings does not offer $Percent%"}
# Only invoke the system UI's exposed selection pattern. No registry/private DPI calls.
$selection=$choice.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern)
$selection.Select()
Write-Output "Requested actual Windows scale $Percent%; window DPI must verify it independently."
