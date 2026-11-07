#requires -Version 5.1
[CmdletBinding()]
param([string] $ExePath, [string] $ResultsDir)
throw 'BLOCKED: historical desktop/system fixture has no current sandbox and owned-process acceptance contract. Migrate before execution; no acceptance is claimed.'
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ResultsDir) { $ResultsDir = Join-Path $root 'docs\test-results\P0-04' }
& (Join-Path $root 'scripts\validate-p0-02.ps1') -ExePath $ExePath -ResultsDir $ResultsDir -Mode DropTarget
exit $LASTEXITCODE
