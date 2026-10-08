param([switch]$Push)
$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskUtf8=[Text.UTF8Encoding]::new($false)
$taskParent='fde81a0de5084d02b3daaa30bac4d4f1fde411f4'
$taskRef='refs/heads/codex/slg-production-core-20261008'
$taskRemote='https://github.com/Zombieliu/mir2.git'
function Invoke-TaskGit([string[]]$Arguments){
    $taskOut=@(& git -c "safe.directory=$taskRepo" -c gc.auto=0 -c credential.helper= -c 'credential.helper=!gh auth git-credential' -c http.lowSpeedLimit=1024 -c http.lowSpeedTime=30 -C $taskRepo @Arguments 2>&1)
    if($LASTEXITCODE -ne 0){throw ($taskOut | Out-String)}
    ($taskOut | Out-String).Trim()
}
$taskQaRoot=Join-Path $taskRepo 'mir2-web3/docs/generated/slg-production-p02b-20261008'
$taskReview=Get-Content -LiteralPath (Join-Path $taskQaRoot 'independent-result-review.json') -Raw | ConvertFrom-Json
if($taskReview.accepted -ne $true -or $taskReview.testCounts.total -ne 177){throw 'Final independent result acceptance required.'}
foreach($taskRun in @('uid-tests-05','simulation-tests-03','npc-regression-02','save-regression-02','inventory-regression-03','uid-file-regression-01')){
    $taskResult=Get-Content -LiteralPath (Join-Path $PSScriptRoot "$taskRun/result.json") -Raw | ConvertFrom-Json
    if($taskResult.exitCode -ne 0 -or @($taskResult.sourceDrift).Count -gt 0){throw 'Failed actual check.'}
    $taskSnapshot=Get-Content -LiteralPath (Join-Path $PSScriptRoot "$taskRun/source-snapshot.json") -Raw | ConvertFrom-Json
    foreach($taskRow in $taskSnapshot.files){if((Get-FileHash -LiteralPath (Join-Path $taskRepo $taskRow.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskRow.sha256){throw "Tested source drift: $($taskRow.path)"}}
}
if((Invoke-TaskGit -Arguments @('rev-parse',$taskRef)) -ne $taskParent){throw 'Production branch advanced; do not overwrite another writer.'}
$taskQualification=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'protected-parent-qualification.json') -Raw | ConvertFrom-Json
if($taskQualification.parent -ne $taskParent -or $taskQualification.declaredProtectedInputs -ne 441 -or $taskQualification.mismatchCount -ne 0){throw 'Protected tested dependencies do not match the production parent.'}
if((Invoke-TaskGit -Arguments @('remote','get-url','origin')) -notmatch 'github\.com[:/]Zombieliu/mir2(?:\.git)?$'){throw 'Unexpected origin.'}
$taskAuthored=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'authored-source-05.json') -Raw | ConvertFrom-Json
foreach($taskRow in $taskAuthored.files){if((Get-FileHash -LiteralPath (Join-Path $taskRepo $taskRow.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskRow.sha256){throw "Reviewed authored source drift: $($taskRow.path)"}}
$taskDocs=@(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'scoped-docs.json') -Raw | ConvertFrom-Json)
$taskPaths=@($taskAuthored.files.path)+@($taskDocs.path)+@('mir2-web3/docs/SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md')
$taskPaths+=@(Get-ChildItem -LiteralPath $taskQaRoot -Recurse -Force -File | ForEach-Object {$_.FullName.Substring($taskRepo.Length+1).Replace('\','/')})
$taskPaths=@($taskPaths | Sort-Object -Unique)
$taskNormalHead=Invoke-TaskGit -Arguments @('rev-parse','HEAD')
$taskNormalRef=Invoke-TaskGit -Arguments @('symbolic-ref','HEAD')
$taskNormalIndex=Invoke-TaskGit -Arguments @('rev-parse','--path-format=absolute','--git-path','index')
$taskNormalIndexBefore=(Get-FileHash -LiteralPath $taskNormalIndex -Algorithm SHA256).Hash
$taskOldIndex=[Environment]::GetEnvironmentVariable('GIT_INDEX_FILE','Process')
try {
    $env:GIT_INDEX_FILE=Join-Path $PSScriptRoot 'publication.index'
    $null=Invoke-TaskGit -Arguments @('read-tree',$taskParent)
    $null=Invoke-TaskGit -Arguments (@('add','--force','--')+$taskPaths)
    foreach($taskDoc in $taskDocs){
        if((Get-FileHash -LiteralPath $taskDoc.scopedPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskDoc.sha256){throw 'Scoped document drift.'}
        $taskBlob=Invoke-TaskGit -Arguments @('hash-object','-w',"--path=$($taskDoc.path)",$taskDoc.scopedPath)
        $null=Invoke-TaskGit -Arguments @('update-index','--add','--cacheinfo',"100644,$taskBlob,$($taskDoc.path)")
    }
    $taskActual=Invoke-TaskGit -Arguments @('diff','--cached','--name-only',$taskParent,'--')
    if((($taskActual -split '\r?\n' | Sort-Object) -join '|') -ne (($taskPaths | Sort-Object) -join '|')){throw 'Unexpected staged paths.'}
    # Keep exact evidence, including original Cargo trailing blank lines.
    $null=Invoke-TaskGit -Arguments @('diff','--cached','--check',$taskParent,'--','.',':(exclude)mir2-web3/docs/generated/slg-production-p02b-20261008')
    $taskTree=Invoke-TaskGit -Arguments @('write-tree')
    $taskMessage=Join-Path $PSScriptRoot 'commit-message.txt'
    [IO.File]::WriteAllText($taskMessage,"Share trusted File item UID authority across staged purchases, splits and crafting`n`nBind the same server-only allocator handle through config, sessions and live inventory. Raise historical and custody floors, preserve merged purchase identities, reject broken/replaced authorities, and stage fresh split IDs before consuming source items. Keep these staged replica mint paths unavailable and original transaction receipts queryable. The normal gateway/catalog remains disabled pending remaining issuer migration, complete census and PostgreSQL integration.`n`nValidation: original serial CargoGuard; UID11 + crafting18 + NPC35 + save59 + inventory37 + File authority17 = 177 passing executions, zero failures/ignored in the final source05 runs, 19 new unique tests. Retain earlier compiler and test-oracle failures as history. No game, gateway, PostgreSQL or installer rollout.`n",$taskUtf8)
    $taskCommit=Invoke-TaskGit -Arguments @('commit-tree',$taskTree,'-p',$taskParent,'-F',$taskMessage)
    foreach($taskPath in $taskPaths){
        $taskOverride=$taskDocs | Where-Object {$_.path -eq $taskPath} | Select-Object -First 1
        $taskSource=if($taskOverride){$taskOverride.scopedPath}else{Join-Path $taskRepo $taskPath}
        $taskBlob=Invoke-TaskGit -Arguments @('hash-object',"--path=$taskPath",$taskSource)
        if((Invoke-TaskGit -Arguments @('rev-parse',"${taskCommit}:$taskPath")) -ne $taskBlob){throw "Committed blob mismatch: $taskPath"}
    }
    [Environment]::SetEnvironmentVariable('GIT_INDEX_FILE',$taskOldIndex,'Process')
    $null=Invoke-TaskGit -Arguments @('update-ref',$taskRef,$taskCommit,$taskParent)
    $taskReceipt=[ordered]@{schema='mir2.production-publication.v2';createdUtc=[DateTime]::UtcNow.ToString('o');parent=$taskParent;commit=$taskCommit;branch=$taskRef;changedPaths=$taskPaths;normalIndexTouchedByPublisher=$false;checkoutSwitchCommandRun=$false;observedNormalIndexUnchanged=((Get-FileHash -LiteralPath $taskNormalIndex -Algorithm SHA256).Hash -eq $taskNormalIndexBefore);observedNormalHeadBefore=$taskNormalHead;observedNormalHeadAfter=(Invoke-TaskGit -Arguments @('rev-parse','HEAD'));observedNormalRefBefore=$taskNormalRef;observedNormalRefAfter=(Invoke-TaskGit -Arguments @('symbolic-ref','HEAD'));scopedDocsExcludeForeignFrontendWork=$true;sourceBlobValidation='canonical Git clean filters; original QA evidence explicitly -text';pushed=$false;remoteVerified=$false;normalProductionEnabled=$false;gatewayRestarted=$false}
    $taskReceiptPath=Join-Path $PSScriptRoot 'publication.json'
    [IO.File]::WriteAllText($taskReceiptPath,($taskReceipt | ConvertTo-Json -Depth 6),$taskUtf8)
    if($Push){
        $null=Invoke-TaskGit -Arguments @('push','--porcelain',$taskRemote,"${taskRef}:$taskRef")
        if(((Invoke-TaskGit -Arguments @('ls-remote','--heads',$taskRemote,$taskRef)) -split '\s+')[0] -ne $taskCommit){throw 'Remote verification failed.'}
        $taskReceipt.pushed=$true; $taskReceipt.remoteVerified=$true
        [IO.File]::WriteAllText($taskReceiptPath,($taskReceipt | ConvertTo-Json -Depth 6),$taskUtf8)
    }
    $taskReceipt | ConvertTo-Json -Depth 6
} finally { [Environment]::SetEnvironmentVariable('GIT_INDEX_FILE',$taskOldIndex,'Process') }
