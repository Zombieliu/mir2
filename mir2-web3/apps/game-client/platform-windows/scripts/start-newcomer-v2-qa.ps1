param(
    [Parameter(Mandatory = $true)]
    [string]$ClientDirectory,

    [Parameter(Mandatory = $true)]
    [string]$GatewayWsUrl
)

$ErrorActionPreference = 'Stop'
$clientDirectoryResolved = (Resolve-Path -LiteralPath $ClientDirectory).Path
$exe = Join-Path $clientDirectoryResolved 'mir2-platform-windows.exe'
$assets = Join-Path $clientDirectoryResolved 'mir2-assets'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw "Native client is missing: $exe"
}
if (-not (Test-Path -LiteralPath $assets -PathType Container)) {
    throw "Native assets are missing: $assets"
}
$uri = [Uri]$GatewayWsUrl
if ($uri.Scheme -notin @('ws', 'wss') -or $uri.Host -notin @('127.0.0.1', 'localhost')) {
    throw 'The isolated V2 QA gateway must be a local WebSocket URL.'
}

# The server owns quest state, but the native Diary and journey HUD require
# the matching presentation profile. Keep this scoped to this child process.
$start = [Diagnostics.ProcessStartInfo]::new($exe)
$start.WorkingDirectory = $clientDirectoryResolved
$start.UseShellExecute = $false
$start.Environment['MIR2_GATEWAY_WS_URL'] = $GatewayWsUrl
$start.Environment['MIR2_NATIVE_ASSET_ROOT'] = $assets
$start.Environment['MIR2_QUEST_GUIDANCE'] = 'newcomer-v2'
foreach ($key in @('MIR2_NATIVE_ACCOUNT', 'MIR2_NATIVE_PASSWORD', 'MIR2_NATIVE_CHARACTER_INDEX')) {
    [void]$start.Environment.Remove($key)
}
$process = [Diagnostics.Process]::Start($start)
if ($null -eq $process) {
    throw 'Native client did not start.'
}
Write-Output "Started newcomer V2 native client PID $($process.Id)"
