param([Parameter(Mandatory=$true)][string[]]$Runs,[string]$AuditId='01')
$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskUtf8=[Text.UTF8Encoding]::new($false)
function Hash-TaskFile([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
$taskRows=@();$taskRoster=@()
foreach($taskRun in $Runs){
    if($taskRun -notmatch '^[a-z0-9-]+$'){throw 'Invalid run ID'}
    $taskDir=Join-Path $PSScriptRoot $taskRun
    $taskResult=Get-Content -LiteralPath (Join-Path $taskDir 'result.json') -Raw | ConvertFrom-Json
    $taskSnapshotPath=Join-Path $taskDir 'source-snapshot.json'
    if((Hash-TaskFile $taskSnapshotPath) -ne $taskResult.sourceSnapshotSha256){throw 'Snapshot mismatch'}
    $taskSnapshot=Get-Content -LiteralPath $taskSnapshotPath -Raw | ConvertFrom-Json
    foreach($taskRow in $taskSnapshot.files){if((Hash-TaskFile (Join-Path $taskRepo $taskRow.path)) -ne $taskRow.sha256){throw "Tested source drift: $($taskRow.path)"}}
    if($taskResult.exitCode -ne 0 -or @($taskResult.sourceDrift).Count -ne 0){throw 'Actual check failed'}
    $taskNonces=@(Get-ChildItem -LiteralPath (Join-Path $taskDir 'cargo-receipts') -Directory)
    if($taskNonces.Count -ne 1){throw 'Expected one original Guard nonce'}
    $taskBefore=Get-Content -LiteralPath (Join-Path $taskNonces[0].FullName 'before.json') -Raw | ConvertFrom-Json
    $taskAfter=Get-Content -LiteralPath (Join-Path $taskNonces[0].FullName 'after.json') -Raw | ConvertFrom-Json
    $taskPolicyPath=Join-Path $taskDir 'guard-policy.json'
    $taskPolicy=Get-Content -LiteralPath $taskPolicyPath -Raw | ConvertFrom-Json
    $taskGuardExe=Join-Path ([IO.Path]::GetDirectoryName($taskPolicy.guardSourcePath)) 'CargoGuard.exe'
    if((Hash-TaskFile $taskGuardExe) -ne $taskPolicy.guardExeSha -or (Hash-TaskFile $taskPolicyPath) -ne $taskBefore.policySha){throw 'Original guard/policy pin mismatch'}
    foreach($taskPair in @(@('guardSourcePath','guardSourceSha'),@('licensePath','licenseSha'),@('calleePath','calleeSha'),@('resolvedRustupPath','resolvedRustupSha'),@('powerShellPath','powerShellSha'),@('probePath','probeSha'),@('rootAuthorityPath','rootAuthoritySha'),@('sourceAcceptancePath','sourceAcceptanceSha'),@('sourceSnapshotPath','sourceSnapshotSha'))){
        if((Hash-TaskFile $taskPolicy.($taskPair[0])) -ne $taskPolicy.($taskPair[1])){throw "Guard pin mismatch: $($taskPair[0])"}
    }
    $taskAcceptance=Get-Content -LiteralPath $taskPolicy.sourceAcceptancePath -Raw|ConvertFrom-Json
    if(-not $taskAcceptance.sourceAccepted -or -not $taskAcceptance.independentReviewAccepted -or (Hash-TaskFile $taskAcceptance.independentSourceReview.path) -ne $taskAcceptance.independentSourceReview.sha256){throw 'Source review acceptance mismatch'}
    $taskFreshness=(([DateTimeOffset]$taskAfter.childStartedUtc).AddMilliseconds(1)-[DateTimeOffset]$taskBefore.sample.sampleEndUtc).TotalMilliseconds
    if($taskBefore.decision -ne 'FORWARD' -or $taskBefore.sampleProof -ne 'actualGetVolumeC' -or [uint64]$taskBefore.sample.remainingBytes -lt 53687091200 -or $taskFreshness -lt 0 -or $taskFreshness -gt 2000 -or $taskAfter.completed -ne $true -or $taskAfter.exited -ne $true -or $taskAfter.disposed -ne $true -or $taskAfter.exitCode -ne 0){throw 'Original Guard lifecycle or C sample invalid'}
    $taskLogPath=Join-Path $taskDir 'cargo.log'
    if((Hash-TaskFile $taskLogPath) -ne $taskResult.cargoLogSha256){throw 'Raw log mismatch'}
    $taskLog=Get-Content -LiteralPath $taskLogPath -Raw
    $taskMatches=[regex]::Matches($taskLog,'(?m)^test ([A-Za-z0-9_:]+) \.\.\. ok\r?$')
    if($taskMatches.Count -eq 0 -or -not $taskLog.Contains("$($taskMatches.Count) passed; 0 failed; 0 ignored")){throw 'Expected actual named passing tests, zero failed/ignored'}
    foreach($taskMatch in $taskMatches){$taskRoster+=[ordered]@{run=$taskRun;test=$taskMatch.Groups[1].Value;result='ok'}}
    $taskRows+=[ordered]@{run=$taskRun;actualPassingExecutions=$taskMatches.Count;declaredInputs=$taskSnapshot.files.Count;sourceDrift=0;remainingCBytes=$taskBefore.sample.remainingBytes;freshnessUpperBoundMs=$taskFreshness;nonce=$taskNonces[0].Name;completed=$true;exited=$true;disposed=$true;exitCode=0}
}
$taskNames=@($taskRoster.test|Sort-Object -Unique)
$taskNewTests=@()
foreach($taskPair in @(@('exact_drop_uid_tests.rs','inventory::exact_drop_uid_tests'))){
    $taskSource=Get-Content -LiteralPath (Join-Path $taskRepo "mir2-web3/apps/simulation/src/runtime/$($taskPair[0])") -Raw
    foreach($taskMatch in [regex]::Matches($taskSource,'#\[test\]\s+fn ([A-Za-z0-9_]+)')){
        $taskName="runtime::$($taskPair[1])::$($taskMatch.Groups[1].Value)"
        if($taskName -notin $taskNames){throw "New test was not actually executed: $taskName"}
        $taskNewTests+=$taskName
    }
}
$taskTotal=($taskRows|ForEach-Object{[int]$_.actualPassingExecutions}|Measure-Object -Sum).Sum
$taskOutput=[ordered]@{schema='mir2.production-drops-actual-audit.v1';createdUtc=[DateTime]::UtcNow.ToString('o');accepted=$true;actualPassingExecutions=$taskTotal;distinctNamedPassingTests=$taskNames.Count;newUniqueTests=$taskNewTests.Count;newTestNames=$taskNewTests;checks=$taskRows;roster=$taskRoster;liveGameplayAccepted=$false;accountAtomicPersistenceAccepted=$false;allIssuerMigrationAccepted=$false;postgresAccepted=$false}
$taskOutputPath=Join-Path $PSScriptRoot "result-audit-$AuditId.json"
if(Test-Path -LiteralPath $taskOutputPath){throw 'Audit ID already exists; preserve previous results'}
[IO.File]::WriteAllText($taskOutputPath,($taskOutput|ConvertTo-Json -Depth 8),$taskUtf8)
[ordered]@{auditPath=$taskOutputPath;actualPassingExecutions=$taskTotal;distinctNamedPassingTests=$taskNames.Count;newUniqueTests=$taskNewTests.Count}|ConvertTo-Json
