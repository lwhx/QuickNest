#requires -Version 5.1
# Generates assets/QuickNest.ico from the demo-app.svg artwork (blue rounded
# square + white 2x2 grid). PNG-compressed ICO entries (Vista+): sizes
# 256/64/32/16, each rendered with GDI+ at high quality.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Add-Type -AssemblyName System.Drawing

function Render([int]$Size) {
    $bmp = New-Object Drawing.Bitmap($Size, $Size)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([Drawing.Color]::Transparent)
    # Blue rounded square (radius = 12/48 of the canvas).
    $radius = [float]($Size * 12.0 / 48.0)
    $d = 2.0 * $radius
    $path = New-Object Drawing.Drawing2D.GraphicsPath
    $path.AddArc(0, 0, $d, $d, 180, 90)
    $path.AddArc($Size - $d, 0, $d, $d, 270, 90)
    $path.AddArc($Size - $d, $Size - $d, $d, $d, 0, 90)
    $path.AddArc(0, $Size - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    $blue = New-Object Drawing.SolidBrush([Drawing.Color]::FromArgb(255, 0x25, 0x63, 0xEB))
    $g.FillPath($blue, $path)
    # White 2x2 grid (squares at 14/48 and 26/48, each 8/48 wide).
    $cell = [float]($Size * 8.0 / 48.0)
    $white = New-Object Drawing.SolidBrush([Drawing.Color]::White)
    foreach ($ox in @(14.0, 26.0)) {
        foreach ($oy in @(14.0, 26.0)) {
            $x = [float]($Size * $ox / 48.0)
            $y = [float]($Size * $oy / 48.0)
            $g.FillRectangle($white, $x, $y, $cell, $cell)
        }
    }
    $g.Dispose()
    $ms = New-Object IO.MemoryStream
    $bmp.Save($ms, [Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return ,$ms.ToArray()
}

$sizes = @(256, 64, 32, 16)
$images = @()
foreach ($s in $sizes) { $images += ,(Render $s) }

# ICO container: ICONDIR + ICONDIRENTRY per image + blobs.
$out = Join-Path $root 'assets\QuickNest.ico'
$fs = [IO.File]::Create($out)
$bw = New-Object IO.BinaryWriter($fs)
$bw.Write([uint16]0)      # reserved
$bw.Write([uint16]1)      # type: icon
$bw.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]
    $data = $images[$i]
    $bw.Write([byte]($(if ($s -ge 256) { 0 } else { $s })))   # width (0 = 256)
    $bw.Write([byte]($(if ($s -ge 256) { 0 } else { $s })))   # height
    $bw.Write([byte]0)     # palette
    $bw.Write([byte]0)     # reserved
    $bw.Write([uint16]1)   # color planes
    $bw.Write([uint16]32)  # bits per pixel
    $bw.Write([uint32]$data.Length)
    $bw.Write([uint32]$offset)
    $offset += $data.Length
}
foreach ($data in $images) { $bw.Write($data) }
$bw.Flush()
$bw.Dispose()
Write-Host ("icon written: " + (Get-Item $out).Length + " bytes")
