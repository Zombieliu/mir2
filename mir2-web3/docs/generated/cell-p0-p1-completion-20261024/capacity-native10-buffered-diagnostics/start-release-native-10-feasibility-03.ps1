$ErrorActionPreference='Stop'
$taskRoot='C:/mir2-build/p01-completion-20261024/release-native-10-feasibility-03'
$taskPrivate=Join-Path $taskRoot 'private'
$taskExe=Join-Path $taskRoot 'bin/mir2-gateway.exe'
$taskProject='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3'
$listeners=@(Get-NetTCPConnection -State Listen -LocalPort 17500,17510 -ErrorAction SilentlyContinue)
if($listeners.Count -gt 0){throw 'Unique native10 feasibility probe ports are already in use'}
if(-not(Test-Path -LiteralPath (Join-Path $taskPrivate 'accounts.json'))){throw 'Account-only provisioned store is required before launch'}
if(Test-Path -LiteralPath (Join-Path $taskRoot 'realm-process.json')){throw 'This fresh probe was already launched'}
function New-TaskSecret([string]$fileName){
 $taskPath=Join-Path $taskPrivate $fileName
 $taskBytes=New-Object byte[] 32
 $taskRandom=[Security.Cryptography.RandomNumberGenerator]::Create()
 try{$taskRandom.GetBytes($taskBytes)}finally{$taskRandom.Dispose()}
 $taskHex=[BitConverter]::ToString($taskBytes).Replace('-','').ToLowerInvariant()
 $taskStream=[IO.File]::Open($taskPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 try{$taskEncoded=[Text.UTF8Encoding]::new($false).GetBytes($taskHex);$taskStream.Write($taskEncoded,0,$taskEncoded.Length);$taskStream.Flush($true)}finally{$taskStream.Dispose()}
 return $taskHex
}
Get-ChildItem Env:MIR2_* | ForEach-Object {Remove-Item -LiteralPath ('Env:'+$_.Name)}
$env:MIR2_GATEWAY_TCP_ADDR='127.0.0.1:17500'
$env:MIR2_GATEWAY_WEB_ADDR='127.0.0.1:17510'
$env:MIR2_GATEWAY_TOKIO_WORKER_THREADS='4'
$env:MIR2_GATEWAY_MAX_WS_CONNECTIONS='24'
$env:MIR2_GATEWAY_MAX_ACTIVE_SESSIONS='10'
$env:MIR2_GATEWAY_MAX_RECONNECT_LEASES='10'
$env:MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT='4'
$env:MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT='1'
$env:MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT='2'
$env:MIR2_ACCOUNT_STORE_BACKEND='file'
$env:MIR2_ACCOUNT_STORE_PATH=Join-Path $taskPrivate 'accounts.json'
$env:MIR2_SAVE_RECOVERY_DIR=Join-Path $taskPrivate 'save-recovery'
$env:MIR2_SAVE_RECOVERY_MAC_KEY=New-TaskSecret 'recovery-mac-key.txt'
$env:MIR2_IDENTITY_SESSION_SECRET=New-TaskSecret 'identity-session-secret.txt'
$env:MIR2_IDENTITY_RECOVERY_PEPPER=New-TaskSecret 'identity-recovery-pepper.txt'
$env:MIR2_PASSKEY_AUTH_SECRET=New-TaskSecret 'passkey-auth-secret.txt'
$env:MIR2_GATEWAY_ID='p01-native10-feasibility-03-20261024'
$env:MIR2_ZONE_ROUTING_MODE='single'
$env:MIR2_CONTENT_PROFILE='platinum_176'
$env:MIR2_MONTHLY_CARD_REQUIRED='0'
$env:MIR2_RUNTIME_ENV='development'
$env:MIR2_ALLOWED_WEB_ORIGINS='http://127.0.0.1:17510'
$env:MIR2_SPECTATOR_DATA_DIR=Join-Path $taskRoot 'recordings'
$env:RUST_LOG='warn'
$taskExpected='187775e2398ca593d4aa6021e1b2a7cf3a577ec2a19ca5e7446eaaa70a506f8c'
if((Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskExpected){throw 'Frozen executable hash differs'}
$taskStdout=Join-Path $taskPrivate 'gateway.stdout.log'
$taskStderr=Join-Path $taskPrivate 'gateway.stderr.log'
$taskProcess=Start-Process -FilePath $taskExe -WorkingDirectory $taskProject -WindowStyle Hidden -RedirectStandardOutput $taskStdout -RedirectStandardError $taskStderr -PassThru
$record=[ordered]@{schemaVersion=1;pid=$taskProcess.Id;exe=$taskExe;sha256=(Get-FileHash -LiteralPath $taskExe -Algorithm SHA256).Hash.ToLowerInvariant();endpoint='ws://127.0.0.1:17510/ws';startedAtUtc=[DateTimeOffset]::UtcNow.ToString('o');tokioWorkers=4;content='platinum_176';routing='single';accountBackend='file';maxWs=24;maxActive=10;maxReconnect=10;maxLoginInFlight=4;maxNewCharacterInFlight=1;maxStartGameInFlight=2;capacityAcceptanceEligible=$false;newSourceAcceptanceEligible=$false;parallelCompilation=$false;freshPrivateSecrets=4;sourceCommit='827d4c8b52066d033a2f18941be80c97b337d6de';spectatorDataDir=$env:MIR2_SPECTATOR_DATA_DIR;spectatorEnabledFromFrozenDefault=$true;spectatorRecordingFromFrozenDefault=$true;spectatorPublicFromFrozenDefault=$true}
$record|ConvertTo-Json -Depth 4|Set-Content -LiteralPath (Join-Path $taskRoot 'realm-process.json') -Encoding UTF8
$record|ConvertTo-Json -Depth 4

