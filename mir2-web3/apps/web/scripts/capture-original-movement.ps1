param(
  [string]$WindowTitlePattern = "*Legend of Mir 2*",
  [string]$OutputDir = "",
  [string]$Prefix = "original-movement",
  [int]$SampleMs = 50,
  [int]$HoldMs = 80,
  [int]$StepWaitMs = 900,
  [int]$WarmupMs = 400
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

public static class Mir2OriginalMovementWin32 {
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

  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT {
    public uint type;
    public MOUSEINPUT mi;
  }

  [StructLayout(LayoutKind.Sequential)]
  public struct MOUSEINPUT {
    public int dx;
    public int dy;
    public uint mouseData;
    public uint dwFlags;
    public uint time;
    public IntPtr dwExtraInfo;
  }

  [DllImport("user32.dll")]
  public static extern bool GetClientRect(IntPtr hWnd, out RECT lpRect);

  [DllImport("user32.dll")]
  public static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);

  [DllImport("user32.dll")]
  public static extern bool SetForegroundWindow(IntPtr hWnd);

  [DllImport("user32.dll")]
  public static extern bool SetCursorPos(int X, int Y);

  [DllImport("user32.dll")]
  public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);

  public const uint INPUT_MOUSE = 0;
  public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
  public const uint MOUSEEVENTF_LEFTUP = 0x0004;
  public const uint MOUSEEVENTF_RIGHTDOWN = 0x0008;
  public const uint MOUSEEVENTF_RIGHTUP = 0x0010;
}
"@

function Get-OriginalWindow {
  $process = Get-Process |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $WindowTitlePattern } |
    Select-Object -First 1

  if ($null -eq $process) {
    throw "Window not found: $WindowTitlePattern"
  }

  $rect = New-Object Mir2OriginalMovementWin32+RECT
  $origin = New-Object Mir2OriginalMovementWin32+POINT
  if (-not [Mir2OriginalMovementWin32]::GetClientRect($process.MainWindowHandle, [ref]$rect)) {
    throw "GetClientRect failed for $($process.MainWindowTitle)"
  }
  if (-not [Mir2OriginalMovementWin32]::ClientToScreen($process.MainWindowHandle, [ref]$origin)) {
    throw "ClientToScreen failed for $($process.MainWindowTitle)"
  }

  return @{
    process = $process
    hwnd = $process.MainWindowHandle
    title = $process.MainWindowTitle
    pid = $process.Id
    screenX = $origin.X
    screenY = $origin.Y
    width = [Math]::Max(1, $rect.Right - $rect.Left)
    height = [Math]::Max(1, $rect.Bottom - $rect.Top)
  }
}

function Capture-ClientArea {
  param(
    [hashtable]$Window,
    [string]$Path
  )

  $bitmap = New-Object System.Drawing.Bitmap $Window.width, $Window.height
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.CopyFromScreen($Window.screenX, $Window.screenY, 0, 0, $bitmap.Size)
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  } finally {
    $graphics.Dispose()
    $bitmap.Dispose()
  }

  return @{
    path = $Path
    width = $Window.width
    height = $Window.height
    screenX = $Window.screenX
    screenY = $Window.screenY
  }
}

function Send-MouseButton {
  param(
    [ValidateSet("left", "right")]
    [string]$Button,
    [ValidateSet("down", "up")]
    [string]$State
  )

  $flags = if ($Button -eq "right") {
    if ($State -eq "down") { [Mir2OriginalMovementWin32]::MOUSEEVENTF_RIGHTDOWN } else { [Mir2OriginalMovementWin32]::MOUSEEVENTF_RIGHTUP }
  } else {
    if ($State -eq "down") { [Mir2OriginalMovementWin32]::MOUSEEVENTF_LEFTDOWN } else { [Mir2OriginalMovementWin32]::MOUSEEVENTF_LEFTUP }
  }

  $input = New-Object Mir2OriginalMovementWin32+INPUT
  $input.type = [Mir2OriginalMovementWin32]::INPUT_MOUSE
  $input.mi.dwFlags = $flags
  $inputs = [Mir2OriginalMovementWin32+INPUT[]]@($input)
  $sent = [Mir2OriginalMovementWin32]::SendInput(1, $inputs, [Runtime.InteropServices.Marshal]::SizeOf([type][Mir2OriginalMovementWin32+INPUT]))
  if ($sent -ne 1) {
    throw "SendInput failed for $Button $State"
  }
}

function Invoke-OriginalClick {
  param(
    [hashtable]$Window,
    [ValidateSet("left", "right")]
    [string]$Button,
    [int]$ClientX,
    [int]$ClientY
  )

  [Mir2OriginalMovementWin32]::SetForegroundWindow($Window.hwnd) | Out-Null
  Start-Sleep -Milliseconds 120
  [Mir2OriginalMovementWin32]::SetCursorPos($Window.screenX + $ClientX, $Window.screenY + $ClientY) | Out-Null
  Start-Sleep -Milliseconds 40
  Send-MouseButton $Button "down"
  Start-Sleep -Milliseconds $HoldMs
  Send-MouseButton $Button "up"
}

function Get-DefaultRoute {
  param([hashtable]$Window)

  $centerX = [int]($Window.width / 2)
  $centerY = [int]($Window.height / 2)
  return @(
    @{ label = "run-right-1"; button = "right"; clientX = $centerX + 118; clientY = $centerY - 4 },
    @{ label = "run-right-2"; button = "right"; clientX = $centerX + 118; clientY = $centerY - 4 },
    @{ label = "walk-down-1"; button = "left"; clientX = $centerX + 2; clientY = $centerY + 84 },
    @{ label = "walk-left-1"; button = "left"; clientX = $centerX - 62; clientY = $centerY + 2 }
  )
}

function Sample-AfterAction {
  param(
    [hashtable]$Window,
    [string]$Label,
    [int]$StartIndex
  )

  $samples = @()
  $deadline = (Get-Date).AddMilliseconds($StepWaitMs)
  $index = $StartIndex
  $startedAt = Get-Date
  while ((Get-Date) -lt $deadline) {
    $elapsed = [int]((Get-Date) - $startedAt).TotalMilliseconds
    $imagePath = Join-Path $OutputDir ("{0}-{1:D3}-{2}.png" -f $Prefix, $index, $Label)
    $capture = Capture-ClientArea $Window $imagePath
    $samples += @{
      label = $Label
      index = $index
      elapsedMs = $elapsed
      capture = $capture
    }
    $index += 1
    Start-Sleep -Milliseconds $SampleMs
  }

  return @{
    samples = $samples
    nextIndex = $index
  }
}

$window = Get-OriginalWindow
[Mir2OriginalMovementWin32]::SetForegroundWindow($window.hwnd) | Out-Null
Start-Sleep -Milliseconds $WarmupMs

$route = Get-DefaultRoute $window
$sampleIndex = 0
$allSamples = @()
$actions = @()

foreach ($step in $route) {
  Invoke-OriginalClick $window $step.button $step.clientX $step.clientY
  $actions += @{
    label = $step.label
    button = $step.button
    clientX = $step.clientX
    clientY = $step.clientY
  }
  $result = Sample-AfterAction $window $step.label $sampleIndex
  $allSamples += $result.samples
  $sampleIndex = $result.nextIndex
}

$reportPath = Join-Path $OutputDir ("{0}.json" -f $Prefix)
$report = @{
  ok = $true
  startedAt = (Get-Date).ToString("o")
  window = @{
    title = $window.title
    pid = $window.pid
    width = $window.width
    height = $window.height
    screenX = $window.screenX
    screenY = $window.screenY
  }
  sampleMs = $SampleMs
  holdMs = $HoldMs
  stepWaitMs = $StepWaitMs
  actions = $actions
  sampleCount = $allSamples.Count
  samples = $allSamples
}

($report | ConvertTo-Json -Depth 8) | Set-Content -Path $reportPath -Encoding UTF8
$report | ConvertTo-Json -Compress -Depth 4
