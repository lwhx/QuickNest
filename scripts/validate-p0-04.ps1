#requires -Version 5.1
[CmdletBinding()]
param([string] $ExePath, [string] $ResultsDir)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ResultsDir) { $ResultsDir = Join-Path $root 'docs\test-results\P0-04' }
& (Join-Path $root 'scripts\validate-p0-02.ps1') -ExePath $ExePath -ResultsDir $ResultsDir -Mode DropTarget
exit $LASTEXITCODE
