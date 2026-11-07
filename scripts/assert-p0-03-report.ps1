#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string] $ReportPath,
    [Parameter(Mandatory=$true)][string] $HwndPath,
    [Parameter(Mandatory=$true)][int] $TargetPid
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$rows = @(Import-Csv -LiteralPath $ReportPath -Delimiter "`t")
$hwnd = @(Import-Csv -LiteralPath $HwndPath -Delimiter "`t")
function Require([bool] $OK, [string] $Message) { if (-not $OK) { throw $Message } }
function Sta($row) { $row.apartment_hr -ceq '0x00000000' -and $row.apartment -in @('0','3') }
$expected = @{
    fresh = 'before,ole_outer,ole_inner,inner_released,outer_released,cleaned'
    sta = 'before,com_initialized,ole_outer,ole_inner,inner_released,outer_released,cleaned'
    mta = 'before,com_initialized,conflict,cleaned'
    ui = 'before,ole_initialized,backend_selected,loop_nested,loop_nested_released,dispatch_received,dispatch_received,dispatch_received,hwnd_lifecycle_passed,before_release,own_reference_released'
    all = 'completed'
}
Require ($rows.Count -eq 29) 'Unexpected OLE report row count'
Require (@($rows | Where-Object {[int]$_.pid -ne $TargetPid -or [int]$_.tid -le 0}).Count -eq 0) 'OLE report PID/TID mismatch'
foreach ($case in @('fresh','sta','mta','ui','all')) {
    $caseRows = @($rows | Where-Object {$_.case -ceq $case})
    Require (($caseRows.phase -join ',') -ceq $expected[$case]) "Unexpected phase order for $case"
    Require (@($caseRows.tid | Select-Object -Unique).Count -eq 1) "Thread changed within $case"
}
foreach ($case in @('fresh','sta','mta')) {
    $r = @($rows | Where-Object {$_.case -ceq $case})
    Require ($r[0].apartment_hr -ceq '0x800401F0' -and $r[-1].apartment_hr -ceq '0x800401F0') "$case did not start/end uninitialized"
    foreach ($row in $r) {
        if ($row.phase -eq 'ole_outer') { Require ($row.hresult -ceq '0x00000000' -and (Sta $row)) 'First OleInitialize must be S_OK/STA' }
        if ($row.phase -eq 'ole_inner') { Require ($row.hresult -ceq '0x00000001' -and (Sta $row)) 'Repeated OleInitialize must be S_FALSE/STA' }
        if ($row.phase -eq 'inner_released') { Require (Sta $row) 'Nested cleanup removed outer STA' }
        if ($row.phase -eq 'outer_released' -and $case -eq 'sta') { Require (Sta $row) 'OLE cleanup removed COM-only baseline STA' }
        if ($row.phase -eq 'conflict') { Require ($row.hresult -ceq '0x80010106' -and $row.apartment -eq '1' -and $row.apartment_hr -ceq '0x00000000') 'MTA conflict did not preserve MTA' }
    }
}
$ui = @($rows | Where-Object {$_.case -ceq 'ui'})
Require ($ui[0].apartment_hr -ceq '0x800401F0') 'Fresh UI thread was unexpectedly initialized before own guard'
Require ($ui[1].hresult -ceq '0x00000000') 'UI first OLE call must be S_OK'
foreach ($row in $ui | Where-Object {$_.phase -notin @('before','own_reference_released')}) { Require (Sta $row) "Non-STA UI phase: $($row.phase)" }
Require (@($ui | Where-Object {$_.phase -eq 'loop_nested' -and $_.hresult -ceq '0x00000001'}).Count -eq 1) 'UI nested initialization was not S_FALSE'
$after=$ui[-1]
Require ((Sta $after) -or $after.apartment_hr -ceq '0x800401F0') 'Unexpected apartment after releasing only our own OLE reference'
Require (@($hwnd | Where-Object {[int]$_.tid -ne [int]$ui[0].tid -or [int]$_.pid -ne $TargetPid}).Count -eq 0) 'HWND owner is not the OLE UI thread'
Require (($rows.case -join ',') -ceq ((@('fresh')*6+@('sta')*7+@('mta')*4+@('ui')*11+@('all')) -join ',')) 'OLE cases out of order'
Write-Host 'OLE report verified: S_OK, S_FALSE, RPC_E_CHANGED_MODE, thread-local cleanup and UI dispatch/STA.'
