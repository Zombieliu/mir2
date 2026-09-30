param(
    [Parameter(Mandatory = $true)][string]$GatewayExe,
    [Parameter(Mandatory = $true)][string]$EvidenceRoot,
    [int]$TcpPort = 18800,
    [int]$WebPort = 18810
)

$ErrorActionPreference = 'Stop'
$taskExe = (Resolve-Path -LiteralPath $GatewayExe).Path
$taskEvidence = [IO.Path]::GetFullPath($EvidenceRoot)
New-Item -ItemType Directory -Path $taskEvidence -Force | Out-Null
$taskProfile = Join-Path $taskEvidence 'profile.json'
if (Test-Path -LiteralPath $taskProfile) {
    $taskSavedProfile = Get-Content -LiteralPath $taskProfile -Raw | ConvertFrom-Json
    if ($taskSavedProfile.cadence -ne 'newcomer-v2') { throw 'Evidence directory belongs to a different cadence' }
} elseif (Test-Path -LiteralPath (Join-Path $taskEvidence 'accounts.json')) {
    throw 'Existing account store has no V2 marker; choose a fresh evidence directory'
} else {
    '{"schema":1,"cadence":"newcomer-v2","contentProfile":"platinum_176"}' |
        Set-Content -LiteralPath $taskProfile
}
if (Get-NetTCPConnection -LocalPort $TcpPort,$WebPort -State Listen -ErrorAction SilentlyContinue) {
    throw 'V2 QA ports are already in use; existing gateways will not be stopped'
}

# Explicit optional cadence, original level curve and full shared Crystal maps.
# This store must be independent of existing V1 player evidence.
$env:MIR2_QUEST_CADENCE = 'newcomer-v2'
$env:MIR2_CONTENT_PROFILE = 'platinum_176'
$env:MIR2_GATEWAY_TCP_ADDR = "127.0.0.1:$TcpPort"
$env:MIR2_GATEWAY_WEB_ADDR = "127.0.0.1:$WebPort"
$env:MIR2_GATEWAY_ID = 'newcomer-v2-ordinary-qa'
$env:MIR2_ACCOUNT_STORE_PATH = Join-Path $taskEvidence 'accounts.json'
$env:MIR2_SAVE_RECOVERY_DIR = Join-Path $taskEvidence 'recovery'
$env:MIR2_ACCOUNT_STORE_BACKEND = 'file'
$env:MIR2_RUNTIME_ENV = 'development'
$env:MIR2_DEPLOYMENT_ENV = 'development'
$env:MIR2_ACCOUNT_STORE_REQUIRE_POSTGRES = 'false'
$env:MIR2_GATEWAY_SAVE_DEBOUNCE_MS = '5000'
$env:MIR2_GATEWAY_SAVE_QUEUE_LIMIT = '256'
$env:MIR2_GATEWAY_SLOW_STAGE_MS = '500'
$env:MIR2_SPECTATOR_ENABLED = 'false'
$env:MIR2_SPECTATOR_RECORDING_ENABLED = 'false'
Remove-Item Env:MIR2_ACCOUNT_STORE_DATABASE_URL -ErrorAction SilentlyContinue
foreach ($taskSecret in @('MIR2_SAVE_RECOVERY_MAC_KEY','MIR2_IDENTITY_SESSION_SECRET','MIR2_IDENTITY_RECOVERY_PEPPER')) {
    $taskKeyPath = Join-Path $taskEvidence "$taskSecret.private.txt"
    if (-not (Test-Path -LiteralPath $taskKeyPath)) {
        [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)) |
            Set-Content -LiteralPath $taskKeyPath
    }
    [Environment]::SetEnvironmentVariable($taskSecret, (Get-Content -LiteralPath $taskKeyPath -Raw).Trim(), 'Process')
}

$ErrorActionPreference = 'Continue'
& $taskExe 2>&1 | ForEach-Object {
    [IO.File]::AppendAllText((Join-Path $taskEvidence 'gateway.log'), [string]$_ + [Environment]::NewLine)
}
exit $LASTEXITCODE
