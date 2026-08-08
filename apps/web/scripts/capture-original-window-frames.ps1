param(
  [string]$WindowTitlePattern = "*Legend of Mir 2*",
  [string]$OutputDir = "",
  [string]$Prefix = "original-window-frames",
  [string]$ReadyFile = "",
  [string]$StartSignalFile = "",
  [int]$StartSignalTimeoutMs = 30000,
  [string]$Label = "original-window-frames",
  [int]$DurationMs = 5200,
  [int]$SampleMs = 50,
  [ValidateSet("jpeg", "png")]
  [string]$ImageFormat = "jpeg",
  [int]$JpegQuality = 82,
  [switch]$ActivateWindow,
  [string[]]$MinimizeWindowTitlePatterns = @(),
  [switch]$RestoreMinimizedWindows,
  [string[]]$PreClickClientPoints = @(),
  [string[]]$PreKeys = @(),
  [int]$PreClickDelayMs = 150,
  [int]$PreKeyDelayMs = 120,
  [int]$CropLeft = 0,
  [int]$CropTop = 0,
  [int]$CropWidth = 0,
  [int]$CropHeight = 0,
  [int]$ExpectedClientWidth = 0,
  [int]$ExpectedClientHeight = 0,
  [switch]$ParkCursorOutsideClient
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..\..")
if ($OutputDir -eq "") {
  $OutputDir = Join-Path $RepoRoot "docs\generated\player-qa\movement-jitter"
}
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class Mir2OriginalWindowFramesWin32 {
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

  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hWnd);

  [DllImport("user32.dll")]
  public static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);

  [DllImport("user32.dll")]
  public static extern bool SetCursorPos(int X, int Y);

  [DllImport("user32.dll")]
  public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);

  [DllImport("user32.dll")]
  public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
}
"@

function Get-MatchingWindowProcesses {
  param([string]$TitlePattern)
  Get-Process |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitlePattern }
}

function Get-OriginalWindow {
  $process = Get-MatchingWindowProcesses $WindowTitlePattern |
    Select-Object -First 1

  if ($null -eq $process) {
    throw "Window not found: $WindowTitlePattern"
  }

  $rect = New-Object Mir2OriginalWindowFramesWin32+RECT
  $origin = New-Object Mir2OriginalWindowFramesWin32+POINT
  if (-not [Mir2OriginalWindowFramesWin32]::GetClientRect($process.MainWindowHandle, [ref]$rect)) {
    throw "GetClientRect failed for $($process.MainWindowTitle)"
  }
  if (-not [Mir2OriginalWindowFramesWin32]::ClientToScreen($process.MainWindowHandle, [ref]$origin)) {
    throw "ClientToScreen failed for $($process.MainWindowTitle)"
  }

  return @{
    title = $process.MainWindowTitle
    pid = $process.Id
    hwnd = $process.MainWindowHandle.ToInt64()
    screenX = $origin.X
    screenY = $origin.Y
    width = [Math]::Max(1, $rect.Right - $rect.Left)
    height = [Math]::Max(1, $rect.Bottom - $rect.Top)
  }
}

function Minimize-MatchingWindows {
  param(
    [string[]]$TitlePatterns,
    [int64]$ExcludeHwnd = 0
  )

  $minimized = New-Object "System.Collections.Generic.List[object]"
  foreach ($pattern in $TitlePatterns) {
    if ([string]::IsNullOrWhiteSpace($pattern)) {
      continue
    }

    foreach ($process in (Get-MatchingWindowProcesses $pattern)) {
      $hwnd = $process.MainWindowHandle.ToInt64()
      if ($ExcludeHwnd -ne 0 -and $hwnd -eq $ExcludeHwnd) {
        continue
      }

      [Mir2OriginalWindowFramesWin32]::ShowWindowAsync([IntPtr]$hwnd, 6) | Out-Null
      $minimized.Add(@{
        title = $process.MainWindowTitle
        pid = $process.Id
        hwnd = $hwnd
        pattern = $pattern
      }) | Out-Null
    }
  }

  if ($minimized.Count -gt 0) {
    Start-Sleep -Milliseconds 200
  }

  return $minimized
}

function Get-CaptureArea {
  param([hashtable]$Window)

  $left = [Math]::Min([Math]::Max(0, $CropLeft), [Math]::Max(0, $Window.width - 1))
  $top = [Math]::Min([Math]::Max(0, $CropTop), [Math]::Max(0, $Window.height - 1))
  $availableWidth = [Math]::Max(1, $Window.width - $left)
  $availableHeight = [Math]::Max(1, $Window.height - $top)
  $width = if ($CropWidth -gt 0) { [Math]::Min($CropWidth, $availableWidth) } else { $availableWidth }
  $height = if ($CropHeight -gt 0) { [Math]::Min($CropHeight, $availableHeight) } else { $availableHeight }

  return @{
    title = $Window.title
    pid = $Window.pid
    hwnd = $Window.hwnd
    screenX = $Window.screenX + $left
    screenY = $Window.screenY + $top
    width = [Math]::Max(1, $width)
    height = [Math]::Max(1, $height)
    sourceWindow = $Window
    crop = @{
      left = $left
      top = $top
      width = [Math]::Max(1, $width)
      height = [Math]::Max(1, $height)
    }
  }
}

function Assert-ExpectedClientSize {
  param([hashtable]$Window)

  if ($ExpectedClientWidth -gt 0 -and $Window.width -ne $ExpectedClientWidth) {
    throw "Expected client width $ExpectedClientWidth but captured $($Window.width) for '$($Window.title)'."
  }
  if ($ExpectedClientHeight -gt 0 -and $Window.height -ne $ExpectedClientHeight) {
    throw "Expected client height $ExpectedClientHeight but captured $($Window.height) for '$($Window.title)'."
  }
}

function Invoke-PreClickClientPoints {
  param(
    [hashtable]$Window,
    [string[]]$Points,
    [int]$DelayMs
  )

  $expandedPoints = New-Object "System.Collections.Generic.List[string]"
  foreach ($entry in $Points) {
    if ([string]::IsNullOrWhiteSpace($entry)) {
      continue
    }
    foreach ($point in $entry.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)) {
      $expandedPoints.Add($point) | Out-Null
    }
  }

  foreach ($point in $expandedPoints) {
    if ([string]::IsNullOrWhiteSpace($point)) {
      continue
    }

    $parts = $point.Split(",", [System.StringSplitOptions]::RemoveEmptyEntries)
    if ($parts.Count -ne 2) {
      throw "Invalid -PreClickClientPoints value '$point'. Expected 'x,y'."
    }

    $clientX = [int]$parts[0].Trim()
    $clientY = [int]$parts[1].Trim()
    $screenX = $Window.screenX + $clientX
    $screenY = $Window.screenY + $clientY
    [Mir2OriginalWindowFramesWin32]::SetCursorPos($screenX, $screenY) | Out-Null
    [Mir2OriginalWindowFramesWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 20
    [Mir2OriginalWindowFramesWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
    if ($DelayMs -gt 0) {
      Start-Sleep -Milliseconds $DelayMs
    }
  }
}

function Convert-KeyNameToVirtualKey {
  param([string]$KeyName)

  switch ($KeyName.Trim().ToUpperInvariant()) {
    "ESC" { return 0x1B }
    "ESCAPE" { return 0x1B }
    "ENTER" { return 0x0D }
    "RETURN" { return 0x0D }
    default { throw "Unsupported -PreKeys value '$KeyName'." }
  }
}

function Invoke-PreKeys {
  param(
    [string[]]$Keys,
    [int]$DelayMs
  )

  $expandedKeys = New-Object "System.Collections.Generic.List[string]"
  foreach ($entry in $Keys) {
    if ([string]::IsNullOrWhiteSpace($entry)) {
      continue
    }
    foreach ($key in $entry.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries)) {
      $expandedKeys.Add($key) | Out-Null
    }
  }

  foreach ($key in $expandedKeys) {
    $virtualKey = [byte](Convert-KeyNameToVirtualKey $key)
    [Mir2OriginalWindowFramesWin32]::keybd_event($virtualKey, 0, 0x0000, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 20
    [Mir2OriginalWindowFramesWin32]::keybd_event($virtualKey, 0, 0x0002, [UIntPtr]::Zero)
    if ($DelayMs -gt 0) {
      Start-Sleep -Milliseconds $DelayMs
    }
  }
}

function Get-JpegCodec {
  [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() |
    Where-Object { $_.MimeType -eq "image/jpeg" } |
    Select-Object -First 1
}

function Save-ClientArea {
  param(
    [hashtable]$Window,
    [string]$Path
  )

  $bitmap = New-Object System.Drawing.Bitmap $Window.width, $Window.height
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.CopyFromScreen($Window.screenX, $Window.screenY, 0, 0, $bitmap.Size)
    if ($ImageFormat -eq "jpeg") {
      $codec = Get-JpegCodec
      $encoder = [System.Drawing.Imaging.Encoder]::Quality
      $encoderParams = New-Object System.Drawing.Imaging.EncoderParameters 1
      $encoderParams.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter $encoder, ([int64]$JpegQuality)
      $bitmap.Save($Path, $codec, $encoderParams)
      $encoderParams.Dispose()
    } else {
      $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    }
  } finally {
    $graphics.Dispose()
    $bitmap.Dispose()
  }
}

function Safe-Label {
  param([string]$Value)
  $safe = $Value -replace "[^a-zA-Z0-9_-]+", "-"
  $safe = $safe.Trim("-")
  if ($safe -eq "") { return "sample" }
  return $safe
}

$window = Get-OriginalWindow
$minimizedWindows = Minimize-MatchingWindows $MinimizeWindowTitlePatterns $window.hwnd
if ($ActivateWindow) {
  # A minimized WinForms window reports its compact taskbar geometry. Restore it
  # before reading the client rect so a bad 160x28 capture cannot look valid.
  [Mir2OriginalWindowFramesWin32]::ShowWindowAsync([IntPtr]$window.hwnd, 9) | Out-Null
  Start-Sleep -Milliseconds 300
  [Mir2OriginalWindowFramesWin32]::SetForegroundWindow([IntPtr]$window.hwnd) | Out-Null
  Start-Sleep -Milliseconds 200
  $window = Get-OriginalWindow
}
if ($PreClickClientPoints.Count -gt 0) {
  Invoke-PreClickClientPoints $window $PreClickClientPoints $PreClickDelayMs
  $window = Get-OriginalWindow
}
if ($PreKeys.Count -gt 0) {
  Invoke-PreKeys $PreKeys $PreKeyDelayMs
  $window = Get-OriginalWindow
}
Assert-ExpectedClientSize $window
$parkedCursor = $null
if ($ParkCursorOutsideClient) {
  $outsideScreenX = $window.screenX - 16
  $outsideScreenY = $window.screenY + [Math]::Floor($window.height / 2)
  if ($outsideScreenX -lt 0) {
    $outsideScreenX = $window.screenX + $window.width + 16
  }
  $parkedCursor = @{
    requestedScreenX = $outsideScreenX
    requestedScreenY = $outsideScreenY
    succeeded = $false
  }
}
$captureArea = Get-CaptureArea $window
$samples = New-Object "System.Collections.Generic.List[object]"
$safeLabel = Safe-Label $Label
$extension = if ($ImageFormat -eq "jpeg") { "jpg" } else { "png" }
$mimeType = if ($ImageFormat -eq "jpeg") { "image/jpeg" } else { "image/png" }

if (-not [string]::IsNullOrWhiteSpace($ReadyFile)) {
  $readyDirectory = Split-Path -Parent $ReadyFile
  if (-not [string]::IsNullOrWhiteSpace($readyDirectory)) {
    New-Item -ItemType Directory -Force -Path $readyDirectory | Out-Null
  }
  $ready = @{
    ready = $true
    stage = "waiting"
    readyAt = [DateTimeOffset]::UtcNow.ToString("o")
    window = $window
    captureArea = $captureArea
    sampleMs = $SampleMs
    captureMs = $DurationMs
  }
  $readyJson = $ready | ConvertTo-Json -Depth 6
  $readyUtf8NoBom = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($ReadyFile, "$readyJson`n", $readyUtf8NoBom)
}

if (-not [string]::IsNullOrWhiteSpace($StartSignalFile)) {
  $signalDeadline = [DateTime]::UtcNow.AddMilliseconds([Math]::Max(1, $StartSignalTimeoutMs))
  while (-not (Test-Path -LiteralPath $StartSignalFile)) {
    if ([DateTime]::UtcNow -ge $signalDeadline) {
      throw "Timed out waiting for capture start signal: $StartSignalFile"
    }
    Start-Sleep -Milliseconds 10
  }
}

if ($ParkCursorOutsideClient) {
  # Park immediately before the first frame; capture coordinators may move the
  # system cursor while this process is waiting for its start signal.
  $parkedCursor.succeeded = [Mir2OriginalWindowFramesWin32]::SetCursorPos(
    $parkedCursor.requestedScreenX,
    $parkedCursor.requestedScreenY
  )
  if (-not $parkedCursor.succeeded) {
    throw "Could not park the system cursor outside the Crystal client."
  }
  Start-Sleep -Milliseconds 60
}

$captureStartedAt = [DateTimeOffset]::UtcNow
$captureStartedAtMs = $captureStartedAt.ToUnixTimeMilliseconds()
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
if (-not [string]::IsNullOrWhiteSpace($ReadyFile)) {
  $ready.stage = "capturing"
  $ready.startedAt = $captureStartedAt.ToString("o")
  $ready.startedAtMs = $captureStartedAtMs
  $readyJson = $ready | ConvertTo-Json -Depth 6
  [System.IO.File]::WriteAllText($ReadyFile, "$readyJson`n", $readyUtf8NoBom)
}
$nextSampleMs = 0
$index = 0
while ($stopwatch.ElapsedMilliseconds -le $DurationMs) {
  $elapsedMs = [int]$stopwatch.ElapsedMilliseconds
  $imagePath = Join-Path $OutputDir ("{0}-{1:D4}-{2}-{3:D5}ms.{4}" -f $Prefix, $index, $safeLabel, $elapsedMs, $extension)
  Save-ClientArea $captureArea $imagePath
  $samples.Add(@{
    label = $Label
    index = $index
    elapsedMs = $elapsedMs
    capture = @{
      path = $imagePath
      width = $captureArea.width
      height = $captureArea.height
      mimeType = $mimeType
      screenX = $captureArea.screenX
      screenY = $captureArea.screenY
      crop = $captureArea.crop
    }
  }) | Out-Null

  $index += 1
  $nextSampleMs += $SampleMs
  $sleepMs = $nextSampleMs - $stopwatch.ElapsedMilliseconds
  if ($sleepMs -gt 0) {
    Start-Sleep -Milliseconds $sleepMs
  } else {
    Start-Sleep -Milliseconds 1
  }
}
$stopwatch.Stop()

if ($RestoreMinimizedWindows) {
  foreach ($minimizedWindow in $minimizedWindows) {
    [Mir2OriginalWindowFramesWin32]::ShowWindowAsync([IntPtr]$minimizedWindow.hwnd, 9) | Out-Null
  }
}

$reportPath = Join-Path $OutputDir ("{0}.json" -f $Prefix)
$report = @{
  ok = $true
  interaction = "windowFrameCapture"
  startedAt = $captureStartedAt.ToString("o")
  startedAtMs = $captureStartedAtMs
  window = $window
  captureArea = $captureArea
  sampleMs = $SampleMs
  captureMs = $DurationMs
  frameImageFormat = $ImageFormat
  frameImageQuality = if ($ImageFormat -eq "jpeg") { $JpegQuality } else { $null }
  minimizedWindows = $minimizedWindows
  cursorParking = $parkedCursor
  sampleCount = $samples.Count
  samples = $samples
}

$reportJson = ($report | ConvertTo-Json -Depth 8)
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($reportPath, "$reportJson`n", $utf8NoBom)

@{
  ok = $true
  jsonPath = $reportPath
  sampleCount = $samples.Count
  elapsedMs = [int]$stopwatch.ElapsedMilliseconds
} | ConvertTo-Json -Compress -Depth 4
