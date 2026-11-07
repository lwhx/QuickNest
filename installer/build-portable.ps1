#requires -Version 5.1
# Builds the portable package (baseline 23 章 便携版 / P4-02):
# installer/output/QuickNest-<ver>-portable.zip
#
# Contents: quicknest.exe + QuickNest.ico + README.md + QuickNest.portable
# (empty marker). 解压即用：解压到任意目录、双击运行；标记文件开启便携模式
# （数据目录 <exe 目录>\data）。
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$exe = Join-Path $root 'target\release\quicknest.exe'
$ico = Join-Path $root 'assets\QuickNest.ico'
$readme = Join-Path $root 'README.md'
$outDir = Join-Path $root 'installer\output'
[void][IO.Directory]::CreateDirectory($outDir)
$zip = Join-Path $outDir 'QuickNest-1.2.0-portable.zip'
if (Test-Path -LiteralPath $zip) { Remove-Item $zip -Force }
$stage = Join-Path $env:TEMP ('qn-portable-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
[void][IO.Directory]::CreateDirectory($stage)
try {
    Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'quicknest.exe')
    Copy-Item -LiteralPath $ico -Destination (Join-Path $stage 'QuickNest.ico')
    Copy-Item -LiteralPath $readme -Destination (Join-Path $stage 'README.md')
    # 便携标记：空文件，存在即开启便携模式。
    [IO.File]::WriteAllText((Join-Path $stage 'QuickNest.portable'), '', [Text.Encoding]::ASCII)
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
    $f = Get-Item -LiteralPath $zip
    Write-Host ('portable package: ' + $zip + ' (' + $f.Length + ' bytes)')
    Write-Host ('sha256: ' + (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash)
} finally {
    Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
}
