$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskAudit=@()
foreach($taskRun in @('uid-tests-05','simulation-tests-03','npc-regression-02','save-regression-02','inventory-regression-03','uid-file-regression-01')){
    $taskDir=Join-Path $PSScriptRoot $taskRun
    $taskResult=Get-Content -LiteralPath (Join-Path $taskDir 'result.json') -Raw | ConvertFrom-Json
    $taskSnapshotPath=Join-Path $taskDir 'source-snapshot.json'
    if((Get-FileHash -LiteralPath $taskSnapshotPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskResult.sourceSnapshotSha256){throw 'Snapshot hash mismatch'}
    $taskSnapshot=Get-Content -LiteralPath $taskSnapshotPath -Raw | ConvertFrom-Json
    $taskDrift=@($taskSnapshot.files | Where-Object {(Get-FileHash -LiteralPath (Join-Path $taskRepo $_.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $_.sha256})
    if($taskDrift.Count -gt 0 -or $taskResult.exitCode -ne 0){throw 'Source or exit mismatch'}
    $taskNonce=@(Get-ChildItem -LiteralPath (Join-Path $taskDir 'cargo-receipts') -Directory)
    if($taskNonce.Count -ne 1){throw 'Expected one original guard nonce'}
    $taskBefore=Get-Content -LiteralPath (Join-Path $taskNonce[0].FullName 'before.json') -Raw | ConvertFrom-Json
    $taskAfter=Get-Content -LiteralPath (Join-Path $taskNonce[0].FullName 'after.json') -Raw | ConvertFrom-Json
    $taskPolicy=Get-Content -LiteralPath (Join-Path $taskDir 'guard-policy.json') -Raw | ConvertFrom-Json
    $taskGuardExe=Join-Path ([IO.Path]::GetDirectoryName($taskPolicy.guardSourcePath)) 'CargoGuard.exe'
    if((Get-FileHash -LiteralPath $taskGuardExe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskPolicy.guardExeSha){throw 'Original Guard executable pin mismatch'}
    if((Get-FileHash -LiteralPath (Join-Path $taskDir 'guard-policy.json') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskBefore.policySha){throw 'Policy mismatch'}
    if($taskBefore.decision -ne 'FORWARD' -or $taskBefore.sampleProof -ne 'actualGetVolumeC' -or [uint64]$taskBefore.sample.remainingBytes -lt 53687091200 -or $taskAfter.completed -ne $true -or $taskAfter.exited -ne $true -or $taskAfter.disposed -ne $true -or $taskAfter.exitCode -ne 0){throw 'Guard lifecycle mismatch'}
    foreach($taskPair in @(@('guardSourcePath','guardSourceSha'),@('licensePath','licenseSha'),@('calleePath','calleeSha'),@('resolvedRustupPath','resolvedRustupSha'),@('powerShellPath','powerShellSha'),@('probePath','probeSha'),@('rootAuthorityPath','rootAuthoritySha'),@('sourceAcceptancePath','sourceAcceptanceSha'),@('sourceSnapshotPath','sourceSnapshotSha'))){
        if((Get-FileHash -LiteralPath $taskPolicy.($taskPair[0]) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskPolicy.($taskPair[1])){throw "Guard pin mismatch: $($taskPair[0])"}
    }
    $taskAcceptance=Get-Content -LiteralPath $taskPolicy.sourceAcceptancePath -Raw | ConvertFrom-Json
    if($taskAcceptance.sourceAccepted -ne $true -or $taskAcceptance.independentReviewAccepted -ne $true -or (Get-FileHash -LiteralPath $taskAcceptance.independentSourceReview.path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskAcceptance.independentSourceReview.sha256){throw 'Independent source acceptance pin mismatch'}
    $taskChild=[DateTimeOffset]$taskAfter.childStartedUtc
    $taskSample=[DateTimeOffset]$taskBefore.sample.sampleEndUtc
    $taskFreshness=($taskChild.AddMilliseconds(1)-$taskSample).TotalMilliseconds
    if($taskFreshness -lt 0 -or $taskFreshness -gt 2000){throw 'Guard sample freshness mismatch'}
    $taskLogPath=Join-Path $taskDir 'cargo.log'
    if((Get-FileHash -LiteralPath $taskLogPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskResult.cargoLogSha256){throw 'Raw log mismatch'}
    $taskLog=Get-Content -LiteralPath $taskLogPath -Raw
    $taskTests=[regex]::Matches($taskLog,'(?m)^test ([A-Za-z0-9_:]+) \.\.\. ok\r?$').Count
    $taskExpected=@{'uid-tests-05'=11;'simulation-tests-03'=18;'npc-regression-02'=35;'save-regression-02'=59;'inventory-regression-03'=37;'uid-file-regression-01'=17}[$taskRun]
    if($taskTests -ne $taskExpected -or -not $taskLog.Contains("$taskExpected passed; 0 failed; 0 ignored")){throw "Exact named test mismatch in $taskRun"}
    $taskAudit+=[ordered]@{run=$taskRun;verifiedDeclaredInputs=$taskSnapshot.files.Count;sourceDrift=0;actualNamedTestPasses=$taskTests;remainingCBytes=$taskBefore.sample.remainingBytes;freshnessUpperBoundMs=$taskFreshness;nonce=$taskNonce[0].Name;completed=$taskAfter.completed;exited=$taskAfter.exited;disposed=$taskAfter.disposed;exitCode=$taskAfter.exitCode}
}
$taskTotal=($taskAudit | ForEach-Object { [int]$_.actualNamedTestPasses } | Measure-Object -Sum).Sum
if($taskTotal -ne 177){throw 'Unexpected final execution total'}
$taskOutput=[ordered]@{schema='mir2.production-result-audit.v1';reviewer='Root';createdUtc=[DateTime]::UtcNow.ToString('o');accepted=$true;actualPassingExecutions=$taskTotal;newUniqueTests=19;checks=$taskAudit;controlledDurableInventoryTestsPassed=$true;liveGameplayAcceptance=$false;allIssuerMigrationAccepted=$false;postgresAccepted=$false}
$taskOutput | ConvertTo-Json -Depth 6
