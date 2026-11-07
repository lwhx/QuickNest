#requires -Version 5.1
[CmdletBinding()]
param([string] $ExePath, [string] $ResultsDir)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ResultsDir) { $ResultsDir = Join-Path $root 'docs\test-results\P0-03' }
# Reuse the independent P0-02 HWND sampler while OLE is held across that lifecycle.
& (Join-Path $root 'scripts\validate-p0-02.ps1') -ExePath $ExePath -ResultsDir $ResultsDir -Mode Ole
exit $LASTEXITCODE
