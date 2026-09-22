param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][string]$LogDirectory
)
$ErrorActionPreference = 'Stop'
$packagePath = (Resolve-Path -LiteralPath $PackageDirectory).Path
$clientPath = Join-Path $packagePath 'mir2-platform-windows.exe'
if (-not (Test-Path -LiteralPath $clientPath -PathType Leaf)) { throw 'Packaged client not found.' }
if (Get-Process -Name 'mir2-platform-windows' -ErrorAction SilentlyContinue) {
    throw 'Please exit the running game normally before starting the diagnostic client.'
}
New-Item -ItemType Directory -Path $LogDirectory -Force | Out-Null
$logPath = (Resolve-Path -LiteralPath $LogDirectory).Path
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$renderLog = Join-Path $logPath "$stamp-render.jsonl"
$movementLog = Join-Path $logPath "$stamp-movement.jsonl"
$oldRenderPath = $env:MIR2_NATIVE_RENDER_TRACE_PATH
$oldMovementPath = $env:MIR2_NATIVE_MOVEMENT_TRACE_PATH
try {
    $env:MIR2_NATIVE_RENDER_TRACE_PATH = $renderLog
    $env:MIR2_NATIVE_MOVEMENT_TRACE_PATH = $movementLog
    # This is the interactive game window requested by the player.
    $clientProcess = Start-Process -FilePath $clientPath -WorkingDirectory $packagePath -WindowStyle Normal `
        -RedirectStandardOutput (Join-Path $logPath "$stamp-client.stdout.log") `
        -RedirectStandardError (Join-Path $logPath "$stamp-client.stderr.log") -PassThru
    [pscustomobject]@{ ProcessId = $clientProcess.Id; Client = $clientPath; RenderTrace = $renderLog; MovementTrace = $movementLog }
} finally {
    $env:MIR2_NATIVE_RENDER_TRACE_PATH = $oldRenderPath
    $env:MIR2_NATIVE_MOVEMENT_TRACE_PATH = $oldMovementPath
}
