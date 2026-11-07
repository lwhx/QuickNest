#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$RunDir,[Parameter(Mandatory=$true)][long]$SourceHwnd,[Parameter(Mandatory=$true)][long]$TargetHwnd)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'p0-05-desktop.ps1')
[void][P005Desktop]::ShowWindow([IntPtr]$SourceHwnd,9)
[void][P005Desktop]::SetWindowPos([IntPtr]$SourceHwnd,[IntPtr](-1),10,40,900,760,0x0050)
[void][P005Desktop]::SetWindowPos([IntPtr]$TargetHwnd,[IntPtr](-1),940,40,800,650,0x0050)
Start-Sleep -Milliseconds 300
$shell=New-Object -ComObject Shell.Application
$view=@($shell.Windows() | Where-Object {try{$_.HWND -eq $SourceHwnd}catch{$false}})[0].Document
$zh=([string][char]0x4E2D)+[char]0x6587
$selectionLog=Join-Path $RunDir 'source-selection.jsonl'
function Select-Fixture([string[]]$Names){
 $first=$true
 foreach($name in $Names){$item=$view.Folder.ParseName($name);if(-not $item){throw "Fixture missing: $name"};$flags=if($first){29}else{1};$view.SelectItem($item,$flags);$first=$false}
 $paths=@($view.SelectedItems() | ForEach-Object {$_.Path})
 [ordered]@{timeUtc=[DateTime]::UtcNow.ToString('o');paths=$paths} | ConvertTo-Json -Compress | Add-Content $selectionLog -Encoding UTF8
 Start-Sleep -Milliseconds 150
}
$drag=Join-Path $PSScriptRoot 'drag-p0-05-fixture.ps1'
Select-Fixture @('sample application.exe')
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 248 -Capture final-exe.png
Select-Fixture @($zh+' folder')
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 220 -Capture final-folder.png
Select-Fixture @($zh+' link.lnk')
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 304 -Capture final-link.png
Select-Fixture @($zh+' sample.txt')
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 332 -Capture final-unicode.png
Select-Fixture @('second file.txt',($zh+' sample.txt'))
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 277 -Capture final-multiple.png
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 332 -Action Leave -Capture final-leave.png
& $drag -RunDir $RunDir -SourceHwnd $SourceHwnd -TargetHwnd $TargetHwnd -SourceY 332 -Action Cancel -Capture final-cancel.png
