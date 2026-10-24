param(
    [Parameter(Mandatory=$true)][string]$CheckId,
    [ValidateSet('production-tests','simulation-check','simulation-tests','npc-regression','save-regression','uid-tests','inventory-regression','uid-file-regression','grant-tests','quest-regression','mining-regression','fishing-regression','drop-uid-tests','drop-regression','pickup-regression')][string]$Kind='production-tests'
)
$ErrorActionPreference='Stop'
$taskRepo=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$taskProject=Join-Path $taskRepo 'mir2-web3'
$taskUtf8=[Text.UTF8Encoding]::new($false)
function Write-TaskJson([string]$Path,$Value){[IO.File]::WriteAllText($Path,($Value | ConvertTo-Json -Depth 12),$taskUtf8)}
function Hash-TaskFile([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
$taskReviewPath=Join-Path $PSScriptRoot 'independent-source-review-03.json'
$taskReview=Get-Content -LiteralPath $taskReviewPath -Raw | ConvertFrom-Json
if($taskReview.acceptedForFocusedTests -ne $true){throw 'Independent source review has not accepted focused tests.'}
if((Hash-TaskFile $taskReview.authoredSourcePath) -ne $taskReview.authoredSourceSha256){throw 'Reviewed authored snapshot hash mismatch.'}
$taskAuthored=Get-Content -LiteralPath $taskReview.authoredSourcePath -Raw | ConvertFrom-Json
foreach($taskRow in $taskAuthored.files){if((Hash-TaskFile (Join-Path $taskRepo $taskRow.path)) -ne $taskRow.sha256){throw "Reviewed authored source drift: $($taskRow.path)"}}
$taskActive=@(Get-Process cargo,rustc -ErrorAction SilentlyContinue)
if($taskActive.Count -gt 0){throw 'Another Cargo/rustc invocation is active; keep Cargo serial.'}
$taskCheck=Join-Path $PSScriptRoot $CheckId
if(Test-Path -LiteralPath $taskCheck){throw 'Check ID already exists; preserve historical runs.'}
$null=New-Item -ItemType Directory -Path $taskCheck
$taskReceiptRoot=Join-Path $taskCheck 'cargo-receipts'
$null=New-Item -ItemType Directory -Path $taskReceiptRoot
$taskSourcePaths=@(
    'mir2-web3/Cargo.toml','mir2-web3/Cargo.lock','mir2-web3/rust-toolchain.toml',
    'mir2-web3/docs/SLG-PRODUCTION-CATALOG.v0.2.json',
    'mir2-web3/apps/simulation/Cargo.toml','mir2-web3/apps/simulation/src/lib.rs'
)
$taskSourcePaths+=@(Get-ChildItem -LiteralPath (Join-Path $taskProject 'packages/production') -Recurse -File | ForEach-Object {$_.FullName.Substring($taskRepo.Length+1).Replace('\','/')})
foreach($taskConfig in @('.cargo/config.toml','mir2-web3/.cargo/config.toml')){if(Test-Path -LiteralPath (Join-Path $taskRepo $taskConfig)){$taskSourcePaths+=$taskConfig}}
if($Kind -ne 'production-tests'){
    foreach($taskRelative in @('apps/simulation','packages/game-data','packages/protocol')){
        $taskSourcePaths+=@(Get-ChildItem -LiteralPath (Join-Path $taskProject $taskRelative) -Recurse -File | Where-Object {$_.Extension -in @('.rs','.json','.gz') -or $_.Name -in @('Cargo.toml','Cargo.lock')} | ForEach-Object {$_.FullName.Substring($taskRepo.Length+1).Replace('\','/')})
    }
    foreach($taskRelative in @('config/quest-guidance','infra/postgres/migrations')){
        $taskSourcePaths+=@(Get-ChildItem -LiteralPath (Join-Path $taskProject $taskRelative) -File | ForEach-Object {$_.FullName.Substring($taskRepo.Length+1).Replace('\','/')})
    }
}
$taskRows=@($taskSourcePaths | Sort-Object -Unique | ForEach-Object {[ordered]@{path=$_;sha256=(Hash-TaskFile (Join-Path $taskRepo $_));length=(Get-Item -LiteralPath (Join-Path $taskRepo $_)).Length}})
$taskSnapshotPath=Join-Path $taskCheck 'source-snapshot.json'
Write-TaskJson $taskSnapshotPath ([ordered]@{schema='mir2.production-source.v1';scope=$Kind;createdUtc=[DateTime]::UtcNow.ToString('o');files=$taskRows})
$taskSnapshotSha=Hash-TaskFile $taskSnapshotPath
$taskGuardRoot='C:/mir2-cross-platform-storage-20261002/repo-qa-shared-quest-heading-locale-01/cargo-per-call-guard-implementation01'
$taskTemplatePath='C:/mir2-cross-platform-storage-20261002/repo-qa-web-windows-catchup-20261006-01/w0-transport-source37-native-transport-test01.policy.json'
$taskPolicy=Get-Content -LiteralPath $taskTemplatePath -Raw | ConvertFrom-Json
$taskAuthorityPath=Join-Path $taskCheck 'root-authority.json'
Write-TaskJson $taskAuthorityPath ([ordered]@{
    owner='Root';allowRealCargo='true';guardExeSha=$taskPolicy.guardExeSha;guardSourceSha=$taskPolicy.guardSourceSha;
    sourceSnapshotSha=$taskSnapshotSha;calleeSha=$taskPolicy.calleeSha;probeSha=$taskPolicy.probeSha;independentShimReviewAccepted='true'
})
$taskAcceptancePath=Join-Path $taskCheck 'source-acceptance.json'
Write-TaskJson $taskAcceptancePath ([ordered]@{
    sourceAccepted=$true;independentReviewAccepted=$true;sourceSnapshot=@{sha256=$taskSnapshotSha};
    independentSourceReview=@{path=$taskReviewPath;sha256=(Hash-TaskFile $taskReviewPath)};
    scope='Controlled File fresh exact drop pickup, pure capacity preview and preserved custody identities. Ground creation timing, account atomic persistence, shared durable claims, other issuers, census, Postgres, gateway/UI/installers are not accepted.'
})
$taskPolicy.rootAuthorityPath=$taskAuthorityPath;$taskPolicy.rootAuthoritySha=Hash-TaskFile $taskAuthorityPath
$taskPolicy.sourceAcceptancePath=$taskAcceptancePath;$taskPolicy.sourceAcceptanceSha=Hash-TaskFile $taskAcceptancePath
$taskPolicy.sourceSnapshotPath=$taskSnapshotPath;$taskPolicy.sourceSnapshotSha=$taskSnapshotSha;$taskPolicy.receiptRoot=$taskReceiptRoot
$taskPolicyPath=Join-Path $taskCheck 'guard-policy.json'
Write-TaskJson $taskPolicyPath $taskPolicy
$taskEnvNames=@('QA_CARGO_GUARD_POLICY','QA_CARGO_GUARD_POLICY_SHA256','CARGO_TARGET_DIR','CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_NET_OFFLINE','CARGO_TERM_COLOR','RUST_MIN_STACK','RUST_TEST_THREADS')
$taskOldEnv=@{};foreach($taskName in $taskEnvNames){$taskOldEnv[$taskName]=[Environment]::GetEnvironmentVariable($taskName,'Process')}
$taskArgv=switch($Kind){
    'production-tests' {@('+1.95.0','test','-p','mir2-production','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'simulation-check' {@('+1.95.0','check','-p','mir2-simulation','--locked','--offline','--jobs','1')}
    'simulation-tests' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::production::tests::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'npc-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::npc_purchase_transaction::tests::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'uid-tests' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::item_uid_issuance::tests::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'inventory-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::inventory::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'uid-file-regression' {@('+1.95.0','test','-p','mir2-simulation','--test','user_item_uid_file','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'grant-tests' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::item_grants::tests::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'quest-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','quest','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'mining-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','mining','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'fishing-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','fishing','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'drop-uid-tests' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::inventory::exact_drop_uid_tests::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'drop-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::drops::','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'pickup-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','pickup','--locked','--offline','--jobs','1','--','--test-threads=1')}
    'save-regression' {@('+1.95.0','test','-p','mir2-simulation','--lib','runtime::save::','--locked','--offline','--jobs','1','--','--test-threads=1')}
}
$taskStart=[DateTime]::UtcNow
$taskExit=$null
try {
    $env:QA_CARGO_GUARD_POLICY=$taskPolicyPath;$env:QA_CARGO_GUARD_POLICY_SHA256=Hash-TaskFile $taskPolicyPath
    $env:CARGO_TARGET_DIR=Join-Path $PSScriptRoot 'target';$env:CARGO_INCREMENTAL='0';$env:CARGO_BUILD_JOBS='1';$env:CARGO_NET_OFFLINE='true';$env:CARGO_TERM_COLOR='never';$env:RUST_MIN_STACK='67108864';$env:RUST_TEST_THREADS='1'
    Push-Location -LiteralPath $taskProject
    try {& (Join-Path $taskGuardRoot 'CargoGuard.exe') @taskArgv 2>&1 | Tee-Object -FilePath (Join-Path $taskCheck 'cargo.log');$taskExit=$LASTEXITCODE}finally{Pop-Location}
}finally{foreach($taskName in $taskEnvNames){[Environment]::SetEnvironmentVariable($taskName,$taskOldEnv[$taskName],'Process')}}
$taskDrift=@($taskRows | Where-Object {(Hash-TaskFile (Join-Path $taskRepo $_.path)) -ne $_.sha256})
$taskSummary=[ordered]@{schema='mir2.production-check.v1';kind=$Kind;startedUtc=$taskStart.ToString('o');finishedUtc=[DateTime]::UtcNow.ToString('o');exitCode=$taskExit;argv=$taskArgv;sourceSnapshotSha256=$taskSnapshotSha;declaredInputs=$taskRows.Count;sourceDrift=$taskDrift;cargoLogSha256=(Hash-TaskFile (Join-Path $taskCheck 'cargo.log'));gameStarted=$false;gatewayRestarted=$false;controlledInventoryCommitTests=($Kind -eq 'simulation-tests')}
Write-TaskJson (Join-Path $taskCheck 'result.json') $taskSummary
$taskSummary | ConvertTo-Json -Depth 8
if($taskExit -ne 0 -or $taskDrift.Count -gt 0){exit 1}
