[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateSet('probe', 'capture', 'hover', 'pressed')][string]$Operation,
    [Parameter(Mandatory = $true)][int]$Pid,
    [int]$X = 0,
    [int]$Y = 0,
    [int]$LogicalWidth = 1024,
    [int]$LogicalHeight = 768,
    [string]$CapturePath = '',
    [int]$SettleMs = 220,
    [string]$ExpectedProcessName = 'mir2-platform-windows',
    [string]$ExpectedProcessPath = '',
    [string]$ExpectedProcessSha256 = '',
    [string]$ExpectedWindowTitle = '',
    [string]$ExpectedHandle = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# This helper is intentionally attach-only. It never starts or terminates a
# process. The Node runner refuses to call it in list/dry-run mode.
Add-Type -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;

public static class Mir2NativeUiMatrixWin32 {
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

    public const uint INPUT_MOUSE = 0;
    public const uint MOUSEEVENTF_MOVE = 0x0001;
    public const uint MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const uint MOUSEEVENTF_LEFTUP = 0x0004;

    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool IsWindow(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll", SetLastError=true)] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
    [DllImport("user32.dll", SetLastError=true)] public static extern uint GetDpiForWindow(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool GetCursorPos(out POINT point);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);

    public static void MakeDpiAware() {
        try { SetProcessDpiAwarenessContext(new IntPtr(-4)); } catch { }
    }

    public static string WindowTitle(IntPtr hWnd) {
        var sb = new StringBuilder(256);
        GetWindowText(hWnd, sb, sb.Capacity);
        return sb.ToString();
    }

    public static int LastError() { return Marshal.GetLastWin32Error(); }

    public static void Move(int x, int y) {
        if (!SetCursorPos(x, y)) throw new Exception("SetCursorPos failed: " + LastError());
        var input = new INPUT[1];
        input[0].type = INPUT_MOUSE;
        input[0].data.mi = new MOUSEINPUT { dx=0, dy=0, mouseData=0, dwFlags=MOUSEEVENTF_MOVE, time=0, dwExtraInfo=IntPtr.Zero };
        if (SendInput(1, input, Marshal.SizeOf(typeof(INPUT))) != 1) throw new Exception("SendInput move failed: " + LastError());
    }

    public static void Button(uint flags) {
        var input = new INPUT[1];
        input[0].type = INPUT_MOUSE;
        input[0].data.mi = new MOUSEINPUT { dx=0, dy=0, mouseData=0, dwFlags=flags, time=0, dwExtraInfo=IntPtr.Zero };
        if (SendInput(1, input, Marshal.SizeOf(typeof(INPUT))) != 1) throw new Exception("SendInput button failed: " + LastError());
    }

    public static string Capture(IntPtr hWnd, string path) {
        RECT client;
        if (!GetClientRect(hWnd, out client)) throw new Exception("GetClientRect failed: " + LastError());
        POINT origin = new POINT { X=0, Y=0 };
        if (!ClientToScreen(hWnd, ref origin)) throw new Exception("ClientToScreen failed: " + LastError());
        int width = Math.Max(1, client.Right - client.Left);
        int height = Math.Max(1, client.Bottom - client.Top);
        using (var bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb))
        using (var graphics = Graphics.FromImage(bitmap)) {
            graphics.CopyFromScreen(origin.X, origin.Y, 0, 0, new Size(width, height));
            bitmap.Save(path, ImageFormat.Png);
        }
        return path;
    }
}
'@ -ReferencedAssemblies ([System.Drawing.Bitmap].Assembly.Location)

[Mir2NativeUiMatrixWin32]::MakeDpiAware()

function Fail([string]$Message, [hashtable]$Diagnostics = @{}) {
    $payload = [ordered]@{ ok = $false; error = $Message; diagnostics = $Diagnostics }
    $payload | ConvertTo-Json -Depth 8 -Compress
    exit 2
}

function Get-WindowState {
    try { $process = Get-Process -Id $Pid -ErrorAction Stop } catch { Fail "process not found" @{ pid = $Pid; processMissing = $true } }
    $process.Refresh()
    if ($ExpectedProcessName -and $process.ProcessName -ine [IO.Path]::GetFileNameWithoutExtension($ExpectedProcessName)) {
        Fail "unexpected process name" @{ pid=$Pid; actual=$process.ProcessName; expected=$ExpectedProcessName }
    }
    if ($ExpectedProcessPath) {
        $actualPath = $process.Path
        if (-not $actualPath -or ([IO.Path]::GetFullPath($actualPath) -ine [IO.Path]::GetFullPath($ExpectedProcessPath))) {
            Fail "unexpected process path" @{ pid=$Pid; actual=$actualPath; expected=$ExpectedProcessPath }
        }
    }
    if ($ExpectedProcessSha256) {
        if (-not $process.Path) { Fail "process path unavailable for hash validation" @{ pid=$Pid } }
        $actualHash = (Get-FileHash -LiteralPath $process.Path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actualHash -ne $ExpectedProcessSha256.ToLowerInvariant()) { Fail "unexpected process SHA-256" @{ pid=$Pid; actual=$actualHash; expected=$ExpectedProcessSha256 } }
    }
    $handle = $process.MainWindowHandle
    if ($handle -eq [IntPtr]::Zero) {
        Fail "native window unavailable" @{ pid = $Pid; processHasExited = $process.HasExited; mainWindowHandle = '0x0'; windowDisappeared = $true }
    }
    if (-not [Mir2NativeUiMatrixWin32]::IsWindow($handle) -or -not [Mir2NativeUiMatrixWin32]::IsWindowVisible($handle)) {
        Fail "native window is not visible" @{ pid = $Pid; processHasExited = $process.HasExited; mainWindowHandle = $handle.ToInt64(); windowDisappeared = $true }
    }
    if ([Mir2NativeUiMatrixWin32]::IsIconic($handle)) { Fail "native window is minimized" @{ pid=$Pid; windowDisappeared=$true } }
    if ($ExpectedHandle -and (('0x{0:X}' -f $handle.ToInt64()) -ine $ExpectedHandle)) { Fail "native window handle changed" @{ pid=$Pid; actual=('0x{0:X}' -f $handle.ToInt64()); expected=$ExpectedHandle; windowDisappeared=$true } }
    [uint32]$windowPid = 0
    [void][Mir2NativeUiMatrixWin32]::GetWindowThreadProcessId($handle, [ref]$windowPid)
    if ($windowPid -ne [uint32]$Pid) { Fail "window does not belong to requested PID" @{ pid=$Pid; windowPid=$windowPid; windowDisappeared=$true } }
    $title = [Mir2NativeUiMatrixWin32]::WindowTitle($handle)
    if ($ExpectedWindowTitle -and $title -cne $ExpectedWindowTitle) { Fail "unexpected window title" @{ pid=$Pid; actual=$title; expected=$ExpectedWindowTitle } }
    $client = New-Object Mir2NativeUiMatrixWin32+RECT
    $window = New-Object Mir2NativeUiMatrixWin32+RECT
    if (-not [Mir2NativeUiMatrixWin32]::GetClientRect($handle, [ref]$client)) { Fail "GetClientRect failed" @{ pid = $Pid; windowDisappeared = $true; lastError = [Mir2NativeUiMatrixWin32]::LastError() } }
    if (-not [Mir2NativeUiMatrixWin32]::GetWindowRect($handle, [ref]$window)) { Fail "GetWindowRect failed" @{ pid = $Pid; windowDisappeared = $true; lastError = [Mir2NativeUiMatrixWin32]::LastError() } }
    $state = [ordered]@{
        pid = $Pid
        handle = ('0x{0:X}' -f $handle.ToInt64())
        title = $title
        clientWidth = $client.Right - $client.Left
        clientHeight = $client.Bottom - $client.Top
        window = [ordered]@{ left=$window.Left; top=$window.Top; right=$window.Right; bottom=$window.Bottom }
        dpi = [Mir2NativeUiMatrixWin32]::GetDpiForWindow($handle)
        visible = $true
    }
    if ($state.dpi -le 0) { Fail "GetDpiForWindow returned invalid DPI" @{ pid=$Pid; dpi=$state.dpi; windowDisappeared=$true } }
    return $state
}

function Assert-Foreground([IntPtr]$Handle) {
    $foreground = [Mir2NativeUiMatrixWin32]::GetForegroundWindow()
    if ($foreground -eq [IntPtr]::Zero -or $foreground -ne $Handle) {
        Fail "target window is not foreground" @{ pid=$Pid; targetHandle=('0x{0:X}' -f $Handle.ToInt64()); foregroundHandle=('0x{0:X}' -f $foreground.ToInt64()); windowDisappeared=$false }
    }
}

function Resolve-Point($state) {
    if ($LogicalWidth -le 0 -or $LogicalHeight -le 0) { Fail "logical stage must be positive" @{} }
    if ($state.clientWidth -lt 1 -or $state.clientHeight -lt 1) { Fail "client stage has invalid dimensions" @{ state=$state; windowDisappeared=$true } }
    $scale = [Math]::Min($state.clientWidth / [double]$LogicalWidth, $state.clientHeight / [double]$LogicalHeight)
    if ($scale -le 0) { Fail "invalid contain scale" @{ state=$state; windowDisappeared=$true } }
    $stageWidth = [int][Math]::Round($LogicalWidth * $scale)
    $stageHeight = [int][Math]::Round($LogicalHeight * $scale)
    $offsetX = [int][Math]::Round(($state.clientWidth - $stageWidth) / 2.0)
    $offsetY = [int][Math]::Round(($state.clientHeight - $stageHeight) / 2.0)
    $px = $offsetX + [int][Math]::Round($X * $scale)
    $py = $offsetY + [int][Math]::Round($Y * $scale)
    if ($px -lt $offsetX -or $py -lt $offsetY -or $px -ge ($offsetX + $stageWidth) -or $py -ge ($offsetY + $stageHeight)) {
        Fail "logical point is outside contained client stage" @{ logicalX=$X; logicalY=$Y; clientWidth=$state.clientWidth; clientHeight=$state.clientHeight; scale=$scale; offsetX=$offsetX; offsetY=$offsetY; windowDisappeared=$false }
    }
    $point = New-Object Mir2NativeUiMatrixWin32+POINT
    $point.X = $px; $point.Y = $py
    $handle = [IntPtr]::new([Convert]::ToInt64($state.handle.Substring(2), 16))
    if (-not [Mir2NativeUiMatrixWin32]::ClientToScreen($handle, [ref]$point)) { Fail "ClientToScreen failed" @{ state=$state; logicalX=$X; logicalY=$Y; lastError=[Mir2NativeUiMatrixWin32]::LastError(); windowDisappeared=$true } }
    [ordered]@{ logicalX=$X; logicalY=$Y; clientX=$px; clientY=$py; screenX=$point.X; screenY=$point.Y; scale=$scale; offsetX=$offsetX; offsetY=$offsetY; stageWidth=$stageWidth; stageHeight=$stageHeight }
}

$mouseDown = $false
try {
    $state = Get-WindowState
    if ($Operation -eq 'probe') {
        [ordered]@{ ok=$true; operation=$Operation; state=$state } | ConvertTo-Json -Depth 10 -Compress
        exit 0
    }
    if ($Operation -eq 'capture') {
        if ([string]::IsNullOrWhiteSpace($CapturePath)) { Fail 'capture path is required' @{} }
        $handle = [IntPtr]::new([Convert]::ToInt64($state.handle.Substring(2), 16))
        if (-not [Mir2NativeUiMatrixWin32]::SetForegroundWindow($handle)) { Fail "SetForegroundWindow failed before capture" @{ pid=$Pid; lastError=[Mir2NativeUiMatrixWin32]::LastError(); windowDisappeared=$false } }
        Start-Sleep -Milliseconds 80
        Assert-Foreground $handle
        [void][Mir2NativeUiMatrixWin32]::Capture($handle, $CapturePath)
        [ordered]@{ ok=$true; operation=$Operation; capture=$CapturePath; state=$state } | ConvertTo-Json -Depth 10 -Compress
        exit 0
    }
    $target = Resolve-Point $state
    if ($Operation -eq 'pressed' -and [string]::IsNullOrWhiteSpace($CapturePath)) { Fail 'pressed capture path is required' @{ target=$target } }
    $handle = [IntPtr]::new([Convert]::ToInt64($state.handle.Substring(2), 16))
    if (-not [Mir2NativeUiMatrixWin32]::SetForegroundWindow($handle)) { Fail "SetForegroundWindow failed" @{ pid=$Pid; lastError=[Mir2NativeUiMatrixWin32]::LastError(); windowDisappeared=$false } }
    Start-Sleep -Milliseconds 80
    Assert-Foreground $handle
    [Mir2NativeUiMatrixWin32]::Move($target.screenX, $target.screenY)
    if ($Operation -eq 'hover') { Start-Sleep -Milliseconds $SettleMs }
    if ($Operation -eq 'pressed') {
        Assert-Foreground $handle
        [Mir2NativeUiMatrixWin32]::Button([Mir2NativeUiMatrixWin32]::MOUSEEVENTF_LEFTDOWN)
        $mouseDown = $true
        Start-Sleep -Milliseconds ([Math]::Max(80, $SettleMs))
        Assert-Foreground $handle
        [void][Mir2NativeUiMatrixWin32]::Capture($handle, $CapturePath)
        Assert-Foreground $handle
        [Mir2NativeUiMatrixWin32]::Button([Mir2NativeUiMatrixWin32]::MOUSEEVENTF_LEFTUP)
        $mouseDown = $false
        Start-Sleep -Milliseconds $SettleMs
    }
    [ordered]@{ ok=$true; operation=$Operation; target=$target; capture=$CapturePath; state=$state } | ConvertTo-Json -Depth 10 -Compress
    exit 0
} catch {
    if ($mouseDown) { try { [Mir2NativeUiMatrixWin32]::Button([Mir2NativeUiMatrixWin32]::MOUSEEVENTF_LEFTUP) } catch { } }
    Fail $_.Exception.Message @{ pid=$Pid; operation=$Operation; windowDisappeared=$true }
}
