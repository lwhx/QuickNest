#requires -Version 5.1
[CmdletBinding()]
param([long]$SourceHwnd=594970,[long]$TargetHwnd=2168044,[string]$RunDir,[int]$ExpectedDpi=120)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
. (Join-Path $PSScriptRoot 'p0-06-native.ps1')
$minimized=@()
try {
 # Minimize only the current agent app and test Settings windows temporarily.
 $windows=@([P005Desktop]::Windows() | Where-Object {$_.title -in @('ZCode',([string][char]0x8BBE+[char]0x7F6E))})
 foreach($w in $windows){if(-not ([P006Native]::Read($w.hwnd)).iconic){[void][P005Desktop]::ShowWindow([IntPtr]$w.hwnd,6);$minimized+=$w.hwnd}}
 [P006Native]::Place($TargetHwnd,720,20,1100,900)
 [P006Native]::Place($SourceHwnd,0,20,700,900)
 Start-Sleep -Milliseconds 350
 # Pointer reference already has separate evidence; this pass isolates the actual OLE drop.
 & (Join-Path $PSScriptRoot 'drag-p0-06-at-scale.ps1') -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -RunDir $RunDir -ExpectedDpi $ExpectedDpi
} finally {
 foreach($h in $minimized){[void][P005Desktop]::ShowWindow([IntPtr]$h,9)}
}
