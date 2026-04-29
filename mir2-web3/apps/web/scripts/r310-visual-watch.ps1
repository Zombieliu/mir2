param(
  [double]$DurationHours = 6,
  [int]$IntervalSeconds = 600,
  [int]$MaxSamples = 0,
  [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
if ($OutputDir -eq "") {
  $OutputDir = Join-Path $RepoRoot "docs\generated\player-qa\r310-visual-watch"
}
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$LogPath = Join-Path $OutputDir "r310-visual-watch-log.jsonl"
$EndAt = (Get-Date).AddHours($DurationHours)
$SampleIndex = 0

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class Mir2Win32Capture {
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT {
    public int Left;
    public int Top;
    public int Right;
    public int Bottom;
  }

  [StructLayout(LayoutKind.Sequential)]
  public struct POINT {
    public int X;
    public int Y;
  }

  [DllImport("user32.dll")]
  public static extern bool GetClientRect(IntPtr hWnd, out RECT lpRect);

  [DllImport("user32.dll")]
  public static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);
}
"@

function Write-JsonLine {
  param([hashtable]$Data)
  ($Data | ConvertTo-Json -Compress -Depth 8) | Add-Content -Path $LogPath -Encoding UTF8
}

function Get-HttpStatus {
  param([string]$Url)
  try {
    $response = Invoke-WebRequest -UseBasicParsing -Uri $Url -TimeoutSec 5
    $content = [string]$response.Content
    if ($content.Length -gt 240) {
      $content = $content.Substring(0, 240)
    }
    return @{
      ok = $true
      status = [int]$response.StatusCode
      content = $content
    }
  } catch {
    return @{
      ok = $false
      error = $_.Exception.Message
    }
  }
}

function Capture-ClientArea {
  param(
    [string]$TitlePattern,
    [string]$Path
  )

  $process = Get-Process |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitlePattern } |
    Select-Object -First 1

  if ($null -eq $process) {
    throw "Window not found: $TitlePattern"
  }

  $rect = New-Object Mir2Win32Capture+RECT
  $point = New-Object Mir2Win32Capture+POINT
  if (-not [Mir2Win32Capture]::GetClientRect($process.MainWindowHandle, [ref]$rect)) {
    throw "GetClientRect failed for $($process.MainWindowTitle)"
  }
  if (-not [Mir2Win32Capture]::ClientToScreen($process.MainWindowHandle, [ref]$point)) {
    throw "ClientToScreen failed for $($process.MainWindowTitle)"
  }

  $width = [Math]::Max(1, $rect.Right - $rect.Left)
  $height = [Math]::Max(1, $rect.Bottom - $rect.Top)
  $bitmap = New-Object System.Drawing.Bitmap $width, $height
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.CopyFromScreen($point.X, $point.Y, 0, 0, $bitmap.Size)
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  } finally {
    $graphics.Dispose()
    $bitmap.Dispose()
  }

  return @{
    path = $Path
    title = $process.MainWindowTitle
    pid = $process.Id
    width = $width
    height = $height
    screenX = $point.X
    screenY = $point.Y
  }
}

function Invoke-WebCapture {
  param([string]$Prefix)
  $arguments = @(
    "apps\web\scripts\capture-crystal-parity.mjs",
    "--output",
    $OutputDir,
    "--prefix",
    $Prefix
  )
  $output = & node @arguments 2>&1
  if ($LASTEXITCODE -ne 0) {
    throw ($output -join "`n")
  }
  return ($output -join "`n")
}

do {
  $startedAt = Get-Date
  $stamp = $startedAt.ToString("yyyyMMdd-HHmmss")
  $SampleIndex += 1
  $originalPath = Join-Path $OutputDir ("watch-{0}-original.png" -f $stamp)

  $sample = @{
    sample = $SampleIndex
    startedAt = $startedAt.ToString("o")
    original = $null
    web = $null
    webHealth = Get-HttpStatus "http://127.0.0.1:3002/"
    gatewayHealth = Get-HttpStatus "http://127.0.0.1:7110/health"
    errors = @()
  }

  try {
    $sample.original = Capture-ClientArea "*Legend of Mir 2*" $originalPath
  } catch {
    $sample.errors += "original capture: $($_.Exception.Message)"
  }

  try {
    $webPrefix = "watch-$stamp-web"
    $sample.web = @{
      prefix = $webPrefix
      output = Invoke-WebCapture $webPrefix
    }
  } catch {
    $sample.errors += "web capture: $($_.Exception.Message)"
  }

  $sample.finishedAt = (Get-Date).ToString("o")
  Write-JsonLine $sample

  if ($MaxSamples -gt 0 -and $SampleIndex -ge $MaxSamples) {
    break
  }

  $remaining = [int][Math]::Ceiling(($EndAt - (Get-Date)).TotalSeconds)
  if ($remaining -le 0) {
    break
  }
  Start-Sleep -Seconds ([Math]::Min($IntervalSeconds, $remaining))
} while ((Get-Date) -lt $EndAt)
