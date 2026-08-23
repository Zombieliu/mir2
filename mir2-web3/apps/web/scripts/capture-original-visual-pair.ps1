param(
  [Parameter(Mandatory = $true)]
  [string]$OutputPath,
  [string]$WindowTitlePattern = "*Legend of Mir 2*",
  [Nullable[int]]$ProcessId = $null
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class Mir2OriginalVisualPairWin32 {
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

  [DllImport("user32.dll", SetLastError = true)]
  public static extern bool GetClientRect(IntPtr hWnd, out RECT lpRect);

  [DllImport("user32.dll", SetLastError = true)]
  public static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);

  [DllImport("user32.dll")]
  public static extern uint GetDpiForWindow(IntPtr hWnd);
}
"@

function Get-ClientWindow {
  $matches = @(Get-Process | Where-Object {
    $_.MainWindowHandle -ne 0 -and
    $_.MainWindowTitle -like $WindowTitlePattern -and
    ($null -eq $ProcessId -or $_.Id -eq $ProcessId.Value)
  })
  if ($matches.Count -ne 1) {
    $selector = if ($null -eq $ProcessId) {
      "title '$WindowTitlePattern'"
    } else {
      "PID $($ProcessId.Value) and title '$WindowTitlePattern'"
    }
    throw "Expected exactly one window matching $selector; found $($matches.Count)."
  }

  $process = $matches[0]
  $rect = New-Object Mir2OriginalVisualPairWin32+RECT
  $origin = New-Object Mir2OriginalVisualPairWin32+POINT
  if (-not [Mir2OriginalVisualPairWin32]::GetClientRect($process.MainWindowHandle, [ref]$rect)) {
    throw "GetClientRect failed for process $($process.Id)."
  }
  if (-not [Mir2OriginalVisualPairWin32]::ClientToScreen($process.MainWindowHandle, [ref]$origin)) {
    throw "ClientToScreen failed for process $($process.Id)."
  }

  $width = $rect.Right - $rect.Left
  $height = $rect.Bottom - $rect.Top
  if ($width -ne 1024 -or $height -ne 768) {
    throw "Expected a 1024x768 client area; observed ${width}x${height}."
  }

  $executablePath = $null
  try {
    $executablePath = $process.Path
  } catch {
    $executablePath = $process.MainModule.FileName
  }
  if ([string]::IsNullOrWhiteSpace($executablePath) -or -not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
    throw "Could not resolve an executable file for process $($process.Id)."
  }

  $dpi = 96
  try {
    $reportedDpi = [Mir2OriginalVisualPairWin32]::GetDpiForWindow($process.MainWindowHandle)
    if ($reportedDpi -gt 0) { $dpi = [int]$reportedDpi }
  } catch {
    $graphics = [System.Drawing.Graphics]::FromHwnd($process.MainWindowHandle)
    try { $dpi = [int][Math]::Round($graphics.DpiX) } finally { $graphics.Dispose() }
  }

  return @{
    process = $process
    screenX = $origin.X
    screenY = $origin.Y
    width = $width
    height = $height
    executablePath = [System.IO.Path]::GetFullPath($executablePath)
    dpi = $dpi
  }
}

function Save-ExactlyOneClientAreaPng {
  param([hashtable]$Window, [string]$Path)

  $bitmap = New-Object System.Drawing.Bitmap $Window.width, $Window.height
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.CopyFromScreen($Window.screenX, $Window.screenY, 0, 0, $bitmap.Size)
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  } finally {
    $graphics.Dispose()
    $bitmap.Dispose()
  }
}

$fullOutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$outputDirectory = [System.IO.Path]::GetDirectoryName($fullOutputPath)
if ([string]::IsNullOrWhiteSpace($outputDirectory) -or -not (Test-Path -LiteralPath $outputDirectory -PathType Container)) {
  throw "Output directory does not exist: $outputDirectory"
}
if ([System.IO.Path]::GetExtension($fullOutputPath).ToLowerInvariant() -ne ".png") {
  throw "OutputPath must end in .png."
}
if (Test-Path -LiteralPath $fullOutputPath) {
  throw "Refusing to overwrite existing capture: $fullOutputPath"
}

$window = Get-ClientWindow
Save-ExactlyOneClientAreaPng $window $fullOutputPath
$captureFile = Get-Item -LiteralPath $fullOutputPath -ErrorAction Stop
$capturedAt = [DateTimeOffset]::UtcNow.ToString("o")

# Deliberately omit the window title and all client text. This helper observes
# process/window geometry only and does not activate, focus, or inject input.
@{
  ok = $true
  capturedAt = $capturedAt
  process = @{
    pid = $window.process.Id
    name = $window.process.ProcessName
  }
  window = @{
    handle = $window.process.MainWindowHandle.ToInt64()
    clientArea = @{ width = $window.width; height = $window.height }
  }
  executable = @{ path = $window.executablePath }
  dpi = @{ value = $window.dpi; scale = [Math]::Round($window.dpi / 96.0, 6) }
  image = @{ path = $fullOutputPath; bytes = $captureFile.Length; width = 1024; height = 768 }
} | ConvertTo-Json -Compress -Depth 6
