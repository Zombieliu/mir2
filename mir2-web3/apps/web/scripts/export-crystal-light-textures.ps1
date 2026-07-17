param(
  [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
  $OutputDir = Join-Path $PSScriptRoot "..\public\original-effects\Lighting"
}
$OutputDir = [IO.Path]::GetFullPath($OutputDir)
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Add-Type -AssemblyName System.Drawing

$sizes = @(
  @(205, 156),
  @(285, 217),
  @(365, 277),
  @(445, 338),
  @(525, 399),
  @(605, 460),
  @(685, 521),
  @(765, 581),
  @(845, 642),
  @(925, 703)
)
$colours = [System.Drawing.Color[]]@(
  [System.Drawing.Color]::White,
  [System.Drawing.Color]::FromArgb(255, 210, 210, 210),
  [System.Drawing.Color]::FromArgb(255, 160, 160, 160),
  [System.Drawing.Color]::FromArgb(255, 70, 70, 70),
  [System.Drawing.Color]::FromArgb(255, 40, 40, 40),
  [System.Drawing.Color]::FromArgb(0, 0, 0, 0)
)
$positions = [single[]]@(0.0, 0.20, 0.40, 0.60, 0.80, 1.0)
$written = New-Object "System.Collections.Generic.List[object]"

for ($index = 0; $index -lt $sizes.Count; $index++) {
  $width = [int]$sizes[$index][0]
  $height = [int]$sizes[$index][1]
  $bitmap = New-Object System.Drawing.Bitmap $width, $height, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $brush = $null
  try {
    $graphics.Clear([System.Drawing.Color]::FromArgb(0, 0, 0, 0))
    $path.AddEllipse((New-Object System.Drawing.Rectangle 0, 0, $width, $height))
    $brush = New-Object System.Drawing.Drawing2D.PathGradientBrush $path
    $blend = New-Object System.Drawing.Drawing2D.ColorBlend
    $blend.Colors = $colours
    $blend.Positions = $positions
    $brush.InterpolationColors = $blend
    $brush.SurroundColors = $colours
    $brush.CenterColor = [System.Drawing.Color]::White
    $graphics.FillPath($brush, $path)
    $graphics.Save() | Out-Null

    $outputPath = Join-Path $OutputDir ("{0}.png" -f $index)
    $bitmap.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $written.Add(@{ index = $index; width = $width; height = $height; path = $outputPath }) | Out-Null
  } finally {
    if ($null -ne $brush) { $brush.Dispose() }
    $path.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
  }
}

@{
  ok = $true
  outputDir = $OutputDir
  count = $written.Count
  textures = $written
} | ConvertTo-Json -Depth 5
