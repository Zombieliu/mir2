$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskUtf8=[Text.UTF8Encoding]::new($false)
$taskFinalAudit=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'result-audit-02.json') -Raw | ConvertFrom-Json
if($taskFinalAudit.accepted -ne $true -or $taskFinalAudit.actualPassingExecutions -ne 177){throw 'Package only after actual focused results pass.'}
$taskFinal=@('uid-tests-05','simulation-tests-03','npc-regression-02','save-regression-02','inventory-regression-03','uid-file-regression-01')
$taskHistorical=@('uid-tests-01','uid-tests-02','uid-tests-03','simulation-tests-01','simulation-tests-02','uid-tests-04','npc-regression-01','save-regression-01','inventory-regression-01','inventory-regression-02')
$taskHistory=@()
foreach($taskRun in $taskHistorical){
    $taskDir=Join-Path $PSScriptRoot $taskRun
    $taskResult=Get-Content -LiteralPath (Join-Path $taskDir 'result.json') -Raw | ConvertFrom-Json
    $taskLogPath=Join-Path $taskDir 'cargo.log'
    if((Get-FileHash -LiteralPath $taskLogPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskResult.cargoLogSha256){throw 'Historical raw log hash mismatch.'}
    if((Get-FileHash -LiteralPath (Join-Path $taskDir 'source-snapshot.json') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskResult.sourceSnapshotSha256){throw 'Historical source snapshot hash mismatch.'}
    if(@($taskResult.sourceDrift).Count -ne 0){throw 'Unexpected historical source drift.'}
    $taskNonce=@(Get-ChildItem -LiteralPath (Join-Path $taskDir 'cargo-receipts') -Directory)
    if($taskNonce.Count -ne 1){throw 'Expected single original historical Guard nonce.'}
    $taskBefore=Get-Content -LiteralPath (Join-Path $taskNonce[0].FullName 'before.json') -Raw | ConvertFrom-Json
    $taskAfter=Get-Content -LiteralPath (Join-Path $taskNonce[0].FullName 'after.json') -Raw | ConvertFrom-Json
    if($taskBefore.decision -ne 'FORWARD' -or $taskBefore.sampleProof -ne 'actualGetVolumeC' -or [uint64]$taskBefore.sample.remainingBytes -lt 53687091200 -or $taskAfter.completed -ne $true -or $taskAfter.exited -ne $true -or $taskAfter.disposed -ne $true -or $taskAfter.exitCode -ne $taskResult.exitCode){throw 'Historical Guard lifecycle mismatch.'}
    $taskFreshness=(([DateTimeOffset]$taskAfter.childStartedUtc).AddMilliseconds(1)-[DateTimeOffset]$taskBefore.sample.sampleEndUtc).TotalMilliseconds
    if($taskFreshness -lt 0 -or $taskFreshness -gt 2000){throw 'Historical Guard freshness mismatch.'}
    $taskLog=[IO.File]::ReadAllText($taskLogPath)
    $taskPassed=[regex]::Matches($taskLog,'(?m)^test ([A-Za-z0-9_:]+) \.\.\. ok\r?$').Count
    $taskFailed=[regex]::Matches($taskLog,'(?m)^test ([A-Za-z0-9_:]+) \.\.\. FAILED\r?$').Count
    $taskExpected=@{'uid-tests-01'=@(101,0,0);'uid-tests-02'=@(101,9,1);'uid-tests-03'=@(0,10,0);'simulation-tests-01'=@(101,16,2);'simulation-tests-02'=@(0,18,0);'uid-tests-04'=@(0,10,0);'npc-regression-01'=@(0,35,0);'save-regression-01'=@(0,59,0);'inventory-regression-01'=@(101,35,2);'inventory-regression-02'=@(101,0,0)}[$taskRun]
    if($taskResult.exitCode -ne $taskExpected[0] -or $taskPassed -ne $taskExpected[1] -or $taskFailed -ne $taskExpected[2]){throw 'Historical actual outcome mismatch.'}
    $taskNote=switch($taskRun){
        'uid-tests-01' {'Compile E0599: new fixture called nonexistent snapshot(); zero tests ran. Fixed to actual world_snapshot().'}
        'uid-tests-02' {'Split succeeded into belt first, but fixture counted bag only; 9 passed / 1 failed. Fixed bag+ belt counting with all quantity/identity assertions preserved.'}
        'uid-tests-03' {'10 passed before the later production fixture oracle repair. Superseded; not added to final total.'}
        'simulation-tests-01' {'16 passed / 2 failed: trusted binding had raised historical floor to 5 while old capacity/weight fixtures asserted zero. Fixed exact before/after equality; source and failure assertions retained.'}
        'inventory-regression-01' {'35 passed / 2 failed: real Legacy naked-World compatibility defect. The new floor refresh read RuntimeConfig before checking policy. Product fix limits config access to Durable mode, rejects missing/mismatched config without panic, adds one new boundary test. Existing 37 inventory tests unchanged.'}
        'inventory-regression-02' {'Linker LNK1104: prior executable held by DeltaForce (read-only Restart Manager query). Zero tests ran. Reuse non-executable cache in a fresh workspace target; original code, assertions, toolchain, Guard and thresholds unchanged. User game not stopped.'}
        default {'Passed before a later real product fix to history refresh. Retained and superseded; not added to the final source05 total.'}
    }
    $taskHistory+=[ordered]@{run=$taskRun;exitCode=$taskResult.exitCode;actualPassed=$taskPassed;actualFailed=$taskFailed;testsActuallyRun=($taskPassed+$taskFailed);sourceSnapshotSha256=$taskResult.sourceSnapshotSha256;cargoLogSha256=$taskResult.cargoLogSha256;guardNonce=$taskNonce[0].Name;guardCompleted=$taskAfter.completed;guardExited=$taskAfter.exited;guardDisposed=$taskAfter.disposed;remainingCBytes=$taskBefore.sample.remainingBytes;freshnessUpperBoundMs=$taskFreshness;includedInFinalPassTotal=$false;note=$taskNote}
}
$taskPreflights=@(Get-ChildItem -LiteralPath $PSScriptRoot -File -Filter 'preflight*.json' | Select-Object -ExpandProperty Name)
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'historical-results-audit.json'),([ordered]@{schema='mir2.production-historical-results.v1';createdUtc=[DateTime]::UtcNow.ToString('o');runs=$taskHistory;preflightRefusals=$taskPreflights;preflightCargoSpawned=$false;preflightTestsRun=0;preflightNonces=0} | ConvertTo-Json -Depth 8),$taskUtf8)
$taskQaRoot=Join-Path $taskRepo 'mir2-web3/docs/generated/slg-production-p02b-20261008'
if(Test-Path -LiteralPath $taskQaRoot){throw 'Preserve previously packaged raw evidence.'}
$null=New-Item -ItemType Directory -Path $taskQaRoot
function Copy-TaskEvidence([string]$Source,[string]$Destination){
    if(Test-Path -LiteralPath $Destination){throw 'Do not overwrite raw evidence.'}
    $null=New-Item -ItemType Directory -Path ([IO.Path]::GetDirectoryName($Destination)) -Force
    Copy-Item -LiteralPath $Source -Destination $Destination
    if((Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash){throw 'Copied evidence bytes differ.'}
}
foreach($taskRun in ($taskFinal+$taskHistorical)){
    $taskDir=Join-Path $PSScriptRoot $taskRun
    foreach($taskFile in @(Get-ChildItem -LiteralPath $taskDir -Recurse -File)){
        $taskRelative=$taskFile.FullName.Substring($taskDir.Length+1)
        Copy-TaskEvidence $taskFile.FullName (Join-Path $taskQaRoot "$taskRun/$taskRelative")
    }
}
$taskAdditional=@('authored-source.json','authored-source-02.json','authored-source-03.json','authored-source-04.json','authored-source-05.json','independent-source-review.json','independent-source-review-02.json','independent-source-review-03.json','independent-source-review-04.json','independent-source-review-05.json','independent-cache-review-01.json','build-cache-relocation.json','protected-parent-qualification.json','historical-results-audit.json','result-audit-01.json','result-audit-02.json','audit-helper-history.json','run-check-01.ps1','run-check-02.ps1','run-check-03.ps1','run-check-04.ps1','run-check-05.ps1','run-check.ps1','audit-results.ps1','package-evidence.ps1')
$taskAdditional+=$taskPreflights
$taskAdditional+=@(Get-ChildItem -LiteralPath $PSScriptRoot -File -Filter '*preflight*.console.log' | Select-Object -ExpandProperty Name)
foreach($taskName in $taskAdditional){Copy-TaskEvidence (Join-Path $PSScriptRoot $taskName) (Join-Path $taskQaRoot $taskName)}
[IO.File]::WriteAllText((Join-Path $taskQaRoot '.gitattributes'),"# Preserve original Guard, source and raw log bytes.`n** -text`nREADME.md text eol=lf`n.gitattributes text eol=lf`n",$taskUtf8)
[ordered]@{packagedRoot=$taskQaRoot;actualFinalRuns=$taskFinal.Count;historicalActualRuns=$taskHistorical.Count;actualFinalPassTotal=177;actualGuardNonces=16;actualNonceFiles=80;allCopiesByteChecked=$true} | ConvertTo-Json
