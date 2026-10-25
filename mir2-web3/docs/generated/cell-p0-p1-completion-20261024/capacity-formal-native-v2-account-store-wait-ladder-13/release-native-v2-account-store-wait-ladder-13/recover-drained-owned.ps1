param([Parameter(Mandatory=$true)][switch]$StopDrainedOwnedGateway)
$ErrorActionPreference='Stop'
$taskFormalFile=Join-Path $PSScriptRoot 'bound-formal13-binding-v1.json'
if(-not (Test-Path -LiteralPath $taskFormalFile)){$taskFormalFile=Join-Path $PSScriptRoot 'pending-formal13-binding-v1.json'}
$taskFormal=Get-Content -LiteralPath $taskFormalFile -Raw|ConvertFrom-Json
if($taskFormal.state -ne 'FINAL_FRESH_PROBE17_AND_NEW50_BOUND_NOT_EXECUTED' -or $taskFormal.finalized -ne $true){throw 'PENDING_FORMAL13_FRESH_PROBE17'}
$taskCandidate=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'candidate-release-binding.json') -Raw|ConvertFrom-Json
if($taskCandidate.state -ne 'new50-release-and-full16-bound-not-executed' -or $taskCandidate.full16.state -ne 'passed'){throw 'PENDING_FRESH50_RELEASE_AND_FULL16'}
if(-not $StopDrainedOwnedGateway){throw 'Explicit drained owned-process recovery switch is required'}
$taskRecord=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'realm-process.json') -Raw | ConvertFrom-Json
$taskExpected=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'bin/mir2-gateway.exe'))
if([IO.Path]::GetFullPath($taskRecord.exe) -ne $taskExpected){throw 'Recorded executable is outside this unique realm'}
$taskOwned=Get-CimInstance Win32_Process -Filter ('ProcessId='+[int]$taskRecord.pid)
if($null -eq $taskOwned -or [IO.Path]::GetFullPath($taskOwned.ExecutablePath) -ne $taskExpected){throw 'Owned process identity differs'}
if((Get-FileHash -LiteralPath $taskExpected -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskRecord.sha256){throw 'Owned executable SHA differs'}
$taskListeners=@(Get-NetTCPConnection -State Listen -LocalPort 17500,17510 -ErrorAction Stop)
if($taskListeners.Count -ne 2 -or @($taskListeners|Where-Object {$_.LocalAddress -ne '127.0.0.1' -or $_.OwningProcess -ne $taskRecord.pid}).Count){throw 'Owned listener identity differs'}
$taskHealth=Invoke-RestMethod -Uri 'http://127.0.0.1:17510/health' -TimeoutSec 3
$taskKeys=@('currentWsConnections','currentActiveSessions','currentReconnectLeases','currentLoginInFlight','currentNewCharacterInFlight','currentStartGameInFlight')
if($null -eq $taskHealth.capacity -or @($taskKeys|Where-Object {$null -eq $taskHealth.capacity.$_ -or $taskHealth.capacity.$_ -ne 0}).Count){throw 'Six ordinary-client counters must be zero; no forced stop permitted'}
$taskOutput=Join-Path $PSScriptRoot 'realm-forced-recovery.json'
if(Test-Path -LiteralPath $taskOutput){throw 'This recovery was already recorded'}
Stop-Process -Id ([int]$taskRecord.pid)
for($taskAttempt=0;$taskAttempt -lt 30;$taskAttempt++){
 if($null -eq (Get-CimInstance Win32_Process -Filter ('ProcessId='+[int]$taskRecord.pid))){break}
 Start-Sleep -Milliseconds 100
}
$taskAbsent=$null -eq (Get-CimInstance Win32_Process -Filter ('ProcessId='+[int]$taskRecord.pid))
$taskFree=@(Get-NetTCPConnection -State Listen -LocalPort 17500,17510 -ErrorAction SilentlyContinue).Count -eq 0
$taskRecovery=[ordered]@{schemaVersion=1;pid=$taskRecord.pid;exe=$taskExpected;sha256=$taskRecord.sha256;stoppedAtUtc=[DateTimeOffset]::UtcNow.ToString('o');forcedOwnedStop=$true;normalExitClaimed=$false;preStopCapacity=$taskHealth.capacity;processAbsent=$taskAbsent;portsFree=$taskFree;capacityAccepted=$false;scope='Explicit recovery only after exact owned PID/executable/SHA/listeners and six zero counters. Original driver/cleanup verdict remains unchanged.'}
$taskRecovery | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $taskOutput -Encoding UTF8
$taskRecovery | ConvertTo-Json -Depth 5
if(-not $taskAbsent -or -not $taskFree){throw 'Owned recovery did not release process/ports'}
