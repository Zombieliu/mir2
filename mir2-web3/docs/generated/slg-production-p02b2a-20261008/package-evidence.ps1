$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskUtf8=[Text.UTF8Encoding]::new($false)
function Hash-TaskFile([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
$taskAudit=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'result-audit-01.json') -Raw|ConvertFrom-Json
if(-not $taskAudit.accepted -or $taskAudit.actualPassingExecutions -ne 208 -or $taskAudit.distinctNamedPassingTests -ne 204 -or $taskAudit.newUniqueTests -ne 16){throw 'Actual final result required'}
$taskFinal=@($taskAudit.checks.run)
$taskAllRuns=@(Get-ChildItem -LiteralPath $PSScriptRoot -Directory|Where-Object{Test-Path -LiteralPath (Join-Path $_.FullName 'result.json')}|Select-Object -ExpandProperty Name)
$taskHistory=@()
foreach($taskRun in $taskAllRuns|Where-Object{$_ -notin $taskFinal}){
    $taskDir=Join-Path $PSScriptRoot $taskRun
    $taskResult=Get-Content -LiteralPath (Join-Path $taskDir 'result.json') -Raw|ConvertFrom-Json
    $taskLogPath=Join-Path $taskDir 'cargo.log'
    if((Hash-TaskFile $taskLogPath) -ne $taskResult.cargoLogSha256 -or (Hash-TaskFile (Join-Path $taskDir 'source-snapshot.json')) -ne $taskResult.sourceSnapshotSha256){throw 'Historical raw evidence hash mismatch'}
    $taskNonce=@(Get-ChildItem -LiteralPath (Join-Path $taskDir 'cargo-receipts') -Directory)
    if($taskNonce.Count -ne 1){throw 'Historical original Guard nonce missing'}
    $taskBefore=Get-Content -LiteralPath (Join-Path $taskNonce[0].FullName 'before.json') -Raw|ConvertFrom-Json
    $taskAfter=Get-Content -LiteralPath (Join-Path $taskNonce[0].FullName 'after.json') -Raw|ConvertFrom-Json
    if($taskBefore.decision -ne 'FORWARD' -or $taskBefore.sampleProof -ne 'actualGetVolumeC' -or [uint64]$taskBefore.sample.remainingBytes -lt 53687091200 -or $taskAfter.completed -ne $true -or $taskAfter.exited -ne $true -or $taskAfter.disposed -ne $true -or $taskAfter.exitCode -ne $taskResult.exitCode){throw 'Historical Guard lifecycle mismatch'}
    $taskLog=Get-Content -LiteralPath $taskLogPath -Raw
    $taskPass=[regex]::Matches($taskLog,'(?m)^test ([A-Za-z0-9_:]+) \.\.\. ok\r?$').Count
    $taskFail=[regex]::Matches($taskLog,'(?m)^test ([A-Za-z0-9_:]+) \.\.\. FAILED\r?$').Count
    $taskExpected=@{'grant-tests-01'=@(101,0,0);'grant-tests-02'=@(101,4,4)}[$taskRun]
    if(-not $taskExpected -or $taskResult.exitCode -ne $taskExpected[0] -or $taskPass -ne $taskExpected[1] -or $taskFail -ne $taskExpected[2]){throw 'Unexpected historical result'}
    $taskHistory+=[ordered]@{run=$taskRun;exitCode=$taskResult.exitCode;actualPassed=$taskPass;actualFailed=$taskFail;includedInFinalTotal=$false;completed=$taskAfter.completed;exited=$taskAfter.exited;disposed=$taskAfter.disposed;note=if($taskRun -eq 'grant-tests-01'){'Four ItemGrade root-path compile errors, zero executed tests; product enum path corrected'}else{'New fixture used a nonexistent potion name; five literals corrected to original (HP)DrugSmall, all assertions unchanged'}}
}
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'historical-results-audit.json'),([ordered]@{schema='mir2.production-grants-history.v1';actualHistoricalRuns=$taskHistory.Count;runs=$taskHistory;serialPreflightOrWaitCount=2;serialPreflightCargoStarted=$false;serialPreflightTests=0;serialPreflightNonces=0}|ConvertTo-Json -Depth 6),$taskUtf8)
$taskQaRoot=Join-Path $taskRepo 'mir2-web3/docs/generated/slg-production-p02b2a-20261008'
if(Test-Path -LiteralPath $taskQaRoot){throw 'Evidence destination already exists; preserve previous bytes'}
New-Item -ItemType Directory -Path $taskQaRoot|Out-Null
$taskCopies=@()
function Copy-TaskProof([string]$Source,[string]$Relative){
    $taskDestination=Join-Path $taskQaRoot $Relative
    if(Test-Path -LiteralPath $taskDestination){throw 'Proof collision'}
    New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($taskDestination)) -Force|Out-Null
    Copy-Item -LiteralPath $Source -Destination $taskDestination
    $taskHash=Hash-TaskFile $Source
    if((Hash-TaskFile $taskDestination) -ne $taskHash){throw 'Raw proof bytes changed'}
    $script:taskCopies+=[ordered]@{path=$Relative.Replace('\','/');sha256=$taskHash;length=(Get-Item -LiteralPath $Source).Length}
}
foreach($taskRun in $taskAllRuns){
    $taskDir=Join-Path $PSScriptRoot $taskRun
    foreach($taskFile in Get-ChildItem -LiteralPath $taskDir -Recurse -File){Copy-TaskProof $taskFile.FullName ($taskRun+'/'+$taskFile.FullName.Substring($taskDir.Length+1))}
}
foreach($taskFile in Get-ChildItem -LiteralPath $PSScriptRoot -File|Where-Object{$_.Name -match '^(authored-source-|independent-source-review-|serial-|.*preflight.*\.console\.log$)' -or $_.Name -in @('run-check-01.ps1','run-check-02.ps1','run-check.ps1','audit-results.ps1','package-evidence.ps1','result-audit-01.json','private-cache.json','protected-parent-qualification.json','historical-results-audit.json')}){Copy-TaskProof $taskFile.FullName $taskFile.Name}
[IO.File]::WriteAllText((Join-Path $taskQaRoot '.gitattributes'),"# Preserve the original raw evidence exactly.`n** -text`nREADME.md text eol=lf`n.gitattributes text eol=lf`n",$taskUtf8)
$taskReceipt=[ordered]@{schema='mir2.production-grants-packaging.v1';createdUtc=[DateTime]::UtcNow.ToString('o');rawCopies=$taskCopies.Count;allCopiesByteVerified=$true;actualFinalRuns=$taskFinal.Count;actualHistoricalRuns=$taskHistory.Count;actualOriginalGuardNonces=$taskAllRuns.Count;actualPassingExecutions=208;distinctNamedPassingTests=204;newUniqueTests=16;files=$taskCopies}
[IO.File]::WriteAllText((Join-Path $taskQaRoot 'raw-evidence.json'),($taskReceipt|ConvertTo-Json -Depth 6),$taskUtf8)
[ordered]@{packagedRoot=$taskQaRoot;rawCopies=$taskCopies.Count;actualGuardNonces=$taskAllRuns.Count;allCopiesByteVerified=$true}|ConvertTo-Json
