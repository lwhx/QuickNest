#requires -Version 5.1
[CmdletBinding()]
param(
    [string] $ExePath,
    [string] $ResultsDir
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $ExePath) { $ExePath = Join-Path (Split-Path -Parent $scriptRoot) 'target\release\quicknest.exe' }
if (-not $ResultsDir) { $ResultsDir = Join-Path (Split-Path -Parent $scriptRoot) 'docs\test-results\P0-01' }
$validator = Join-Path $scriptRoot 'validate-p0-01.ps1'
$exe = (Resolve-Path -LiteralPath $ExePath).Path
$results = [IO.Path]::GetFullPath($ResultsDir)
$ps = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
# Ask the existing desktop Explorer to launch the validator. The child validator
# records its own and the app's token integrity; this call alone does not prove it.
$shell = New-Object -ComObject Shell.Application
$windows = $shell.Windows()
$desktopHandle = 0
$desktop = $windows.FindWindowSW(0, 0, 8, [ref] $desktopHandle, 1)
if (-not $desktop) { throw 'Interactive desktop Explorer is unavailable.' }
$arguments = '-NoProfile -File "{0}" -ExePath "{1}" -ResultsDir "{2}"' -f $validator, $exe, $results
$desktop.Document.Application.ShellExecute($ps, $arguments, (Split-Path -Parent $exe), 'open', 0)
Write-Host 'Validation dispatched through desktop Explorer. Check the new run directory for actual integrity and results.'
