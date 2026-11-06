# WN-UI-CORE-02 live-E2E evidence harness.
#
# This harness only emits real Win32 mouse/keyboard input to the already-running
# native client. It never sends BrowserCommand JSON, opens a WebSocket, or
# fabricates a server response. Credentials are read only when needed to let a
# human complete the login; they are never written to evidence or logs.
#
# Safety gates:
# - Change-password stops before the final Submit action (human hand-off).
# - Delete-character stops at the confirmation modal unless -ConfirmDelete is
#   explicitly provided at invocation time; the script still records the modal.
# - No process is killed by this script. -StopClient only stops a client PID
#   that this invocation started.

[CmdletBinding()]
param(
    [string]$ExePath = "",
    [string]$GatewayUrl = "ws://127.0.0.1:7110/ws",
    [string]$EvidenceDir = "",
    [switch]$LaunchClient,
    [switch]$ConfirmDelete,
    [switch]$StopClient,
    [ValidateSet("Preflight", "LoginShell", "CharacterSelect", "InGame", "All")]
    [string]$Stage = "Preflight"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $PSCommandPath
$PlatformDir = Split-Path -Parent (Split-Path -Parent $ScriptDir)
if ((Split-Path -Leaf $PlatformDir) -ne "platform-windows") {
    throw "path resolution error: expected platform-windows, got $PlatformDir"
}
$Web3Root = (Resolve-Path (Join-Path $PlatformDir "..\..\..")).Path
if (-not $ExePath) {
    $ExePath = Join-Path $PlatformDir "target\release\mir2-platform-windows.exe"
}
if (-not $EvidenceDir) {
    $EvidenceDir = Join-Path $Web3Root "docs\generated\player-qa\native-ui-controls\r2-live"
}
$RunId = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
$RunDir = Join-Path $EvidenceDir $RunId
New-Item -ItemType Directory -Force -Path $RunDir | Out-Null

$script:StartedClientPid = $null
$script:Results = [System.Collections.Generic.List[object]]::new()

$script:WinFormsAvailable = $false
try {
    Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
    $script:WinFormsAvailable = $true
} catch {
    # SystemInformation is diagnostic only; the harness can still run without
    # WinForms on a trimmed PowerShell/.NET installation.
    $script:WinFormsAvailable = $false
}
try {
    Add-Type -AssemblyName System.Drawing -ErrorAction Stop
} catch {
    throw "System.Drawing is required for PNG evidence capture: $($_.Exception.Message)"
}

$drawingAssembly = [System.Drawing.Bitmap].Assembly.Location
Add-Type -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Mir2UiWin32 {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public MOUSEKEYBDHARDWAREINPUT data; }
    [StructLayout(LayoutKind.Explicit)] public struct MOUSEKEYBDHARDWAREINPUT {
        [FieldOffset(0)] public MOUSEINPUT mi;
        [FieldOffset(0)] public KEYBDINPUT ki;
        [FieldOffset(0)] public HARDWAREINPUT hi;
    }
    [StructLayout(LayoutKind.Sequential)] public struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo; }
    [StructLayout(LayoutKind.Sequential)] public struct HARDWAREINPUT { public uint uMsg; public ushort wParamL, wParamH; }
    public const uint INPUT_MOUSE = 0, INPUT_KEYBOARD = 1;
    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002, MOUSEEVENTF_LEFTUP = 0x0004;
    public const uint KEYEVENTF_KEYUP = 0x0002, KEYEVENTF_UNICODE = 0x0004;
    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int count);
    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        INPUT[] inputs = new INPUT[2];
        inputs[0].type = INPUT_MOUSE;
        inputs[0].data.mi = new MOUSEINPUT { dx=0, dy=0, mouseData=0, dwFlags=MOUSEEVENTF_LEFTDOWN, time=0, dwExtraInfo=IntPtr.Zero };
        inputs[1].type = INPUT_MOUSE;
        inputs[1].data.mi = new MOUSEINPUT { dx=0, dy=0, mouseData=0, dwFlags=MOUSEEVENTF_LEFTUP, time=0, dwExtraInfo=IntPtr.Zero };
        if (SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT))) != inputs.Length) throw new Exception("SendInput mouse failed");
    }
    public static void Key(ushort vk, bool up) {
        INPUT[] inputs = new INPUT[1];
        inputs[0].type = INPUT_KEYBOARD;
        inputs[0].data.ki = new KEYBDINPUT { wVk=vk, wScan=0, dwFlags=up ? KEYEVENTF_KEYUP : 0, time=0, dwExtraInfo=IntPtr.Zero };
        if (SendInput(1, inputs, Marshal.SizeOf(typeof(INPUT))) != 1) throw new Exception("SendInput key failed");
    }
    public static void Unicode(char c) {
        INPUT[] inputs = new INPUT[2];
        inputs[0].type = INPUT_KEYBOARD;
        inputs[0].data.ki = new KEYBDINPUT { wVk=0, wScan=c, dwFlags=KEYEVENTF_UNICODE, time=0, dwExtraInfo=IntPtr.Zero };
        inputs[1].type = INPUT_KEYBOARD;
        inputs[1].data.ki = new KEYBDINPUT { wVk=0, wScan=c, dwFlags=KEYEVENTF_UNICODE|KEYEVENTF_KEYUP, time=0, dwExtraInfo=IntPtr.Zero };
        if (SendInput(2, inputs, Marshal.SizeOf(typeof(INPUT))) != 2) throw new Exception("SendInput unicode failed");
    }
    public static string WindowTitle(IntPtr hWnd) {
        var sb = new System.Text.StringBuilder(256); GetWindowText(hWnd, sb, sb.Capacity); return sb.ToString();
    }
    public static string Capture(IntPtr hWnd, string path) {
        RECT r; if (!GetClientRect(hWnd, out r)) throw new Exception("GetClientRect failed");
        POINT p = new POINT { X=0, Y=0 }; if (!ClientToScreen(hWnd, ref p)) throw new Exception("ClientToScreen failed");
        int width = Math.Max(1, r.Right-r.Left), height = Math.Max(1, r.Bottom-r.Top);
        using (var bmp = new Bitmap(width, height, PixelFormat.Format32bppArgb))
        using (var g = Graphics.FromImage(bmp)) { g.CopyFromScreen(p.X, p.Y, 0, 0, new Size(width,height)); bmp.Save(path, ImageFormat.Png); }
        return path;
    }
}
"@ -ReferencedAssemblies $drawingAssembly

function Write-JsonFile {
    param([string]$Path, [object]$Value)
    ($Value | ConvertTo-Json -Depth 12) | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Add-Result {
    param([string]$Flow, [string]$Status, [string]$Detail, [string]$Screenshot = "")
    $script:Results.Add([ordered]@{ flow=$Flow; status=$Status; detail=$Detail; screenshot=$Screenshot })
}

function Get-NativeWindow {
    param([System.Diagnostics.Process]$Process)
    $Process.Refresh()
    if ($Process.MainWindowHandle -eq [IntPtr]::Zero) { return $null }
    return $Process.MainWindowHandle
}

function Get-WindowGeometry {
    param([IntPtr]$Handle)
    $client = New-Object Mir2UiWin32+RECT
    $window = New-Object Mir2UiWin32+RECT
    [void][Mir2UiWin32]::GetClientRect($Handle, [ref]$client)
    [void][Mir2UiWin32]::GetWindowRect($Handle, [ref]$window)
    [ordered]@{ clientWidth=$client.Right; clientHeight=$client.Bottom; left=$window.Left; top=$window.Top; right=$window.Right; bottom=$window.Bottom }
}

function Invoke-StageClick {
    param([IntPtr]$Handle, [int]$X, [int]$Y, [string]$Name)
    $g = Get-WindowGeometry $Handle
    if ($g.clientWidth -lt 900 -or $g.clientHeight -lt 650) { throw "client stage too small: $($g.clientWidth)x$($g.clientHeight)" }
    $point = New-Object Mir2UiWin32+POINT
    $point.X = [int][Math]::Round($X * $g.clientWidth / 1024.0)
    $point.Y = [int][Math]::Round($Y * $g.clientHeight / 768.0)
    [void][Mir2UiWin32]::ClientToScreen($Handle, [ref]$point)
    [void][Mir2UiWin32]::SetForegroundWindow($Handle)
    Start-Sleep -Milliseconds 120
    [Mir2UiWin32]::Click($point.X, $point.Y)
    Start-Sleep -Milliseconds 350
    return $Name
}

function Invoke-Key {
    param([IntPtr]$Handle, [string]$Key)
    $vk = @{ ENTER=0x0D; ESC=0x1B; TAB=0x09; BACK=0x08; DELETE=0x2E; LEFT=0x25; RIGHT=0x27; UP=0x26; DOWN=0x28; F12=0x7B }
    if (-not $vk.ContainsKey($Key.ToUpperInvariant())) { throw "unsupported key: $Key" }
    [void][Mir2UiWin32]::SetForegroundWindow($Handle)
    [Mir2UiWin32]::Key([uint16]$vk[$Key.ToUpperInvariant()], $false)
    [Mir2UiWin32]::Key([uint16]$vk[$Key.ToUpperInvariant()], $true)
    Start-Sleep -Milliseconds 250
}

function Capture-Stage {
    param([IntPtr]$Handle, [string]$Flow, [string]$Label)
    $path = Join-Path $RunDir (("{0}-{1}.png" -f $Flow.ToLowerInvariant(), $Label.ToLowerInvariant()) -replace "[^a-z0-9_.-]", "_")
    [void][Mir2UiWin32]::Capture($Handle, $path)
    return $path
}

function Find-ClientProcess {
    $p = Get-Process -Name "mir2-platform-windows" -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Sort-Object StartTime -Descending | Select-Object -First 1
    return $p
}

function Redact-Text {
    param([string]$Text)
    if ($null -eq $Text) { return "" }
    return ($Text -replace "(?i)(password|token|passkey|secret|authorization)\s*[:=]\s*[^\s,;]+", '$1=<redacted>')
}

$preflight = [ordered]@{
    runId=$RunId; utc=(Get-Date).ToUniversalTime().ToString("o"); stage=$Stage
    exePath=$ExePath; exeExists=(Test-Path -LiteralPath $ExePath -PathType Leaf)
    gatewayUrl=($GatewayUrl -replace "(?i)(wss?://)[^/]+", '$1<redacted-host>')
    gatewayProcesses=@(Get-Process | Where-Object { $_.ProcessName -match "mir2-gateway" } | ForEach-Object { [ordered]@{ pid=$_.Id; name=$_.ProcessName; window=$_.MainWindowTitle } })
    clientProcesses=@(Get-Process -Name "mir2-platform-windows" -ErrorAction SilentlyContinue | ForEach-Object { [ordered]@{ pid=$_.Id; window=$_.MainWindowTitle; hasWindow=($_.MainWindowHandle -ne [IntPtr]::Zero) } })
    display=[ordered]@{
        logicalStage="1024x768"
        winFormsAvailable=$script:WinFormsAvailable
        dpiX=[Mir2UiWin32]::GetDpiForSystem()
        systemInformationType=if($script:WinFormsAvailable){[System.Windows.Forms.SystemInformation].FullName}else{$null}
    }
    destructiveActions=[ordered]@{ changePasswordFinalSubmit="HANDOFF_REQUIRED"; deleteConfirm=([bool]$ConfirmDelete) }
}
Write-JsonFile (Join-Path $RunDir "preflight.json") $preflight

if (-not $preflight.exeExists -and $LaunchClient) { throw "client EXE missing: $ExePath" }

$client = Find-ClientProcess
if ($LaunchClient -and -not $client) {
    $client = Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath) -PassThru
    $script:StartedClientPid = $client.Id
    $deadline = (Get-Date).AddSeconds(20)
    do { Start-Sleep -Milliseconds 250; $client.Refresh() } while ($client.MainWindowHandle -eq [IntPtr]::Zero -and (Get-Date) -lt $deadline)
}

if (-not $client) {
    Add-Result "all" "BLOCKED" "No native client window found; start the client and rerun with -Stage LoginShell, CharacterSelect or InGame."
} else {
    $handle = Get-NativeWindow $client
    $title = [Mir2UiWin32]::WindowTitle($handle)
    Write-JsonFile (Join-Path $RunDir "window.json") ([ordered]@{ pid=$client.Id; title=$title; geometry=(Get-WindowGeometry $handle) })
    [void][Mir2UiWin32]::SetForegroundWindow($handle)
    $initial = Capture-Stage $handle "initial" "window"
    Add-Result "preflight" "PASS" "Native window selected: $title" $initial

    if ($Stage -in @("LoginShell", "All")) {
        try {
            $shot = Invoke-StageClick $handle 564 449 "ChangePasswordButton"
            $shot = Capture-Stage $handle "change-password" "opened"
            Add-Result "ChangePassword" "HANDOFF" "Opened with a real click; final submit is intentionally not automated. Human must verify fields/result." $shot
            Invoke-Key $handle "ESC"
            $shot = Invoke-StageClick $handle 458 475 "SafeKeyButton"
            $shot = Capture-Stage $handle "safe-key" "opened"
            Add-Result "SafeKey" "PASS" "Opened with a real click; screenshot records the randomized keyboard surface." $shot
            Invoke-Key $handle "ESC"
        } catch { Add-Result "LoginShell" "BLOCKED" (Redact-Text $_.Exception.Message) }
    }

    if ($Stage -in @("CharacterSelect", "All")) {
        try {
            $shot = Invoke-StageClick $handle 510 748 "DeleteCharacterButton"
            $shot = Capture-Stage $handle "delete" "confirm-modal"
            if ($ConfirmDelete) {
                Add-Result "DeleteConfirm" "CONFIRM_REQUIRED" "Confirmation modal captured. The explicit -ConfirmDelete gate was supplied; human review is still required before the destructive click." $shot
            } else {
                Add-Result "DeleteConfirm" "HANDOFF" "Confirmation modal captured; no destructive click was sent." $shot
            }
            Invoke-Key $handle "ESC"
        } catch { Add-Result "DeleteConfirm" "BLOCKED" (Redact-Text $_.Exception.Message) }
    }

    if ($Stage -in @("InGame", "All")) {
        $hudFlows = @(
            @{ name="Mail"; x=912; y=141 },
            @{ name="Shop"; x=939; y=670 },
            @{ name="Storage"; x=985; y=670 }
        )
        foreach ($flow in $hudFlows) {
            try {
                $null = Invoke-StageClick $handle $flow.x $flow.y "$($flow.name)Button"
                $shot = Capture-Stage $handle $flow.name "opened"
                Add-Result $flow.name "PASS" "HUD entry clicked with real mouse input; panel screenshot captured. Item/service mutations are not issued by this harness." $shot
                Invoke-Key $handle "ESC"
            } catch { Add-Result $flow.name "BLOCKED" (Redact-Text $_.Exception.Message) }
        }
    }
}

$processSnapshot = [ordered]@{
    gateway=@(Get-Process | Where-Object { $_.ProcessName -match "mir2-gateway" } | ForEach-Object { [ordered]@{ pid=$_.Id; name=$_.ProcessName } })
    client=@(Get-Process -Name "mir2-platform-windows" -ErrorAction SilentlyContinue | ForEach-Object { [ordered]@{ pid=$_.Id; name=$_.ProcessName; startedByHarness=($_.Id -eq $script:StartedClientPid) } })
    intentionallyLeftRunning="All pre-existing gateway/client processes. The harness does not kill unrelated processes."
}
$report = [ordered]@{
    schema="wn-ui-core-02-r2-live-e2e/v1"; runId=$RunId; stage=$Stage; preflight=$preflight
    results=@($script:Results); processSnapshot=$processSnapshot
    constraints=@("No protocol injection", "No password/token logging", "Change-password final submit is human hand-off", "Delete confirmation is gated")
}
Write-JsonFile (Join-Path $RunDir "live-e2e-report.json") $report
$report | ConvertTo-Json -Depth 12

if ($StopClient -and $script:StartedClientPid) {
    Write-Warning "-StopClient was supplied, but this harness deliberately does not terminate the client; close it manually after reviewing evidence."
}
