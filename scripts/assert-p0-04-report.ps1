#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string] $ReportPath,
    [Parameter(Mandatory=$true)][string] $HwndPath,
    [Parameter(Mandatory=$true)][int] $TargetPid
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Require([bool] $OK, [string] $Message) { if (-not $OK) { throw $Message } }
$rows=@(Import-Csv -LiteralPath $ReportPath -Delimiter "`t")
$hwnd=@(Import-Csv -LiteralPath $HwndPath -Delimiter "`t")
$phases=@('registered','duplicate_rejected','failed_owner_preserved_registration','hidden','restored','revoked','not_registered_confirmed','target_released','reregistered','raii_revoked_and_released','stale_rejected')
$codes=@('0x00000000','0x80040101','0x80040101','0x80040101','0x80040101','0x00000000','0x80040100','0x00000000','0x00000000','0x80040100','0x80040102')
Require ($rows.Count -eq 34) 'Expected eleven records per generation and final completion'
Require (@($rows | Where-Object {[int]$_.pid -ne $TargetPid -or [int]$_.tid -le 0}).Count -eq 0) 'Registration report PID/TID mismatch'
$previous=-1L
foreach($row in $rows){Require ([long]$row.elapsed_ms -ge $previous) 'Registration report time moved backward';$previous=[long]$row.elapsed_ms}
foreach($g in 1..3){
    $r=@($rows | Where-Object {[int]$_.generation -eq $g})
    $h=@($hwnd | Where-Object {[int]$_.generation -eq $g})
    Require (($r.phase -join ',') -ceq ($phases -join ',')) "Unexpected registration phase order for generation $g"
    Require (($r.hresult -join ',') -ceq ($codes -join ',')) "Unexpected real HRESULT for generation $g"
    Require (@($r | Where-Object {$_.hwnd -cne $h[0].hwnd -or $_.tid -cne $h[0].tid}).Count -eq 0) "Registration not on HWND owning UI thread for generation $g"
    foreach($i in 0..9){Require ($r[$i].is_window -ceq 'true') 'Registration/revocation must occur before native window destruction'}
    foreach($i in @(0,1,2,3,4,8)){Require ($r[$i].registered -ceq 'true') 'Successful registration was lost before revoke'}
    foreach($i in @(5,6,7,9,10)){Require ($r[$i].registered -ceq 'false') 'Registration survived successful revoke'}
    Require ($r[3].visible -ceq 'false' -and $r[4].visible -ceq 'true') 'Hidden/restored registration visibility incorrect'
    Require ($r[7].destroyed -eq '1' -and $r[9].destroyed -eq '1') 'COM targets were not finally released after explicit/RAII revoke'
    Require ($r[10].is_window -ceq 'false' -and $r[10].visible -ceq 'false') 'Stale HWND was not rejected'
    $release=@($h | Where-Object {$_.phase -eq 'component_released'})[0]
    # The clocks start separately; order is also enforced synchronously by hooks before owner drop.
    Require ([long]$r[9].elapsed_ms -le [long]$release.elapsed_ms+200) 'Revocation did not precede component release within clock-start tolerance'
}
Require ($rows[-1].phase -ceq 'completed' -and $rows[-1].generation -eq '0' -and $rows[-1].hresult -ceq '0x00000000') 'Missing final registration completion'
Require (($rows.generation -join ',') -ceq ((@(1)*11+@(2)*11+@(3)*11+@(0)) -join ',')) 'Registration generations out of order'
Write-Host 'P0-04 report verified: register, duplicate conflict, explicit/RAII revoke, COM release, re-registration and native destruction.'
