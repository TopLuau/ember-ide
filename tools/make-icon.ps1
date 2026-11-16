# Generates assets/icon.png and assets/icon.ico (multi-size, PNG-compressed) for Ember IDE.
# Draws a layered flame with a glow and sparks on a dark squircle.
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root "assets"
New-Item -ItemType Directory -Force $assets | Out-Null

function C([int]$a, [int]$r, [int]$g, [int]$b) { [System.Drawing.Color]::FromArgb($a, $r, $g, $b) }

# Flame outline as 4 cubic beziers in unit coordinates (tip near the top, base at the bottom).
$flame = @(
    @(0.50, 0.88,  0.22, 0.88,  0.20, 0.58,  0.36, 0.46),
    @(0.36, 0.46,  0.44, 0.38,  0.40, 0.26,  0.55, 0.12),
    @(0.55, 0.12,  0.58, 0.30,  0.80, 0.38,  0.78, 0.62),
    @(0.78, 0.62,  0.76, 0.80,  0.64, 0.88,  0.50, 0.88)
)

function New-VGrad([double]$y1, [double]$y2, $c1, $c2) {
    $p1 = New-Object System.Drawing.PointF -ArgumentList ([single]0), ([single]$y1)
    $p2 = New-Object System.Drawing.PointF -ArgumentList ([single]0), ([single]$y2)
    return [System.Drawing.Drawing2D.LinearGradientBrush]::new($p1, $p2, $c1, $c2)
}

function New-FlamePath([double]$size, [double]$scale, [double]$cx, [double]$cy, [double]$dy) {
    # scale about the base point (0.5, 0.88), then shift
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    foreach ($s in $flame) {
        $p = @()
        for ($i = 0; $i -lt 8; $i += 2) {
            $x = ($cx + ($s[$i] - 0.5) * $scale) * $size
            $y = ($cy + ($s[$i + 1] - 0.88) * $scale + $dy) * $size
            $p += New-Object System.Drawing.PointF ([single]$x), ([single]$y)
        }
        $path.AddBezier($p[0], $p[1], $p[2], $p[3])
    }
    $path.CloseFigure()
    return $path
}

function New-Icon([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)

    # squircle-ish rounded square
    $r = [single]($size * 0.225)
    $d = $r * 2
    $m = [single]($size * 0.02)
    $w = $size - 2 * $m
    $bgPath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $bgPath.AddArc($m, $m, $d, $d, 180, 90)
    $bgPath.AddArc($m + $w - $d, $m, $d, $d, 270, 90)
    $bgPath.AddArc($m + $w - $d, $m + $w - $d, $d, $d, 0, 90)
    $bgPath.AddArc($m, $m + $w - $d, $d, $d, 90, 90)
    $bgPath.CloseFigure()

    $rect = New-Object System.Drawing.RectangleF 0, 0, $size, $size
    $bg = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, (C 255 38 33 52), (C 255 12 11 18), 90
    $g.FillPath($bg, $bgPath)

    $g.SetClip($bgPath)

    # warm glow behind the flame
    $glowPath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $glowPath.AddEllipse([single]($size * 0.05), [single]($size * 0.25), [single]($size * 0.9), [single]($size * 0.9))
    $glow = New-Object System.Drawing.Drawing2D.PathGradientBrush $glowPath
    $glow.CenterColor = C 170 255 110 40
    $glow.SurroundColors = @((C 0 255 90 30))
    $glow.CenterPoint = New-Object System.Drawing.PointF ([single]($size * 0.5)), ([single]($size * 0.72))
    $g.FillPath($glow, $glowPath)

    # outer flame (red -> orange)
    $outer = New-FlamePath $size 1.0 0.5 0.88 0.0
    $ob = New-VGrad ($size * 0.1) ($size * 0.9) (C 255 255 70 50) (C 255 255 168 40)
    $g.FillPath($ob, $outer)

    # middle flame (orange -> yellow)
    $mid = New-FlamePath $size 0.72 0.5 0.88 -0.02
    $mb = New-VGrad ($size * 0.3) ($size * 0.88) (C 255 255 150 40) (C 255 255 214 90)
    $g.FillPath($mb, $mid)

    # hot core (yellow -> near white)
    $core = New-FlamePath $size 0.42 0.5 0.88 -0.025
    $cb = New-VGrad ($size * 0.5) ($size * 0.88) (C 255 255 226 120) (C 255 255 250 220)
    $g.FillPath($cb, $core)

    # sparks (skipped on tiny sizes)
    if ($size -ge 48) {
        $sparks = @(@(0.73, 0.22, 0.020, 230), @(0.27, 0.32, 0.014, 190), @(0.66, 0.08, 0.011, 160), @(0.80, 0.40, 0.009, 140), @(0.20, 0.50, 0.008, 120))
        foreach ($s in $sparks) {
            $rr = [single]($s[2] * $size)
            $br = New-Object System.Drawing.SolidBrush (C ([int]$s[3]) 255 190 90)
            $g.FillEllipse($br, [single]($s[0] * $size - $rr), [single]($s[1] * $size - $rr), $rr * 2, $rr * 2)
        }
    }

    $g.ResetClip()

    # subtle rim light
    $penW = [single][Math]::Max(1.0, $size * 0.008)
    $pen = New-Object System.Drawing.Pen (C 60 255 255 255), $penW
    $g.DrawPath($pen, $bgPath)
    $g.Dispose()
    return $bmp
}

function Get-PngBytes($bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    return , $ms.ToArray()
}

$big = New-Icon 512
$big.Save((Join-Path $assets "icon.png"), [System.Drawing.Imaging.ImageFormat]::Png)

$sizes = 16, 24, 32, 48, 64, 128, 256
$images = @()
foreach ($s in $sizes) { $images += , (Get-PngBytes (New-Icon $s)) }

$ms = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $ms
$bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]
    $wb = if ($s -ge 256) { 0 } else { $s }
    $bw.Write([byte]$wb); $bw.Write([byte]$wb); $bw.Write([byte]0); $bw.Write([byte]0)
    $bw.Write([uint16]1); $bw.Write([uint16]32)
    $bw.Write([uint32]$images[$i].Length); $bw.Write([uint32]$offset)
    $offset += $images[$i].Length
}
foreach ($img in $images) { $bw.Write($img) }
$bw.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $assets "icon.ico"), $ms.ToArray())
Write-Host "Wrote icon.png (512px) and icon.ico to $assets"
