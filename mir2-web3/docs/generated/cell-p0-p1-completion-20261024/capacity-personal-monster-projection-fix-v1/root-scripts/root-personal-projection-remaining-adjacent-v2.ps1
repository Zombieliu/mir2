$ErrorActionPreference = 'Stop'
$taskBase = 'C:/mir2-build/p01-completion-20261024'
$taskRepository = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey'
$candidate = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'root-personal-projection-final-source-inputs-v1.json') | ConvertFrom-Json
foreach ($entry in (@($candidate.files) + @($candidate.sourcePins))) {
    if ($entry.path -eq 'mir2-web3/apps/gateway/src/routing/tests/live_aoi_order_tests.rs') {
        $entry.bytes=28187
        $entry.sha256='0f21bfd95760a94f189c6b95567b0e65f1a0018cf88e90b2342ff753010a7279'
    }
}
if ($candidate.files.Count -ne 44 -or $candidate.sourcePins.Count -ne 5) { throw 'Expected 44-input five-delta candidate' }
$baseline = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'socket-diagnostics-adjacent-selection-v1.json') | ConvertFrom-Json
$additional = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'probe14-personal-projection-adjacent-v1/existing-adjacent-selection-v1.json') | ConvertFrom-Json
$allExisting = @((@($baseline.tests) + @($additional.additionalExisting | ForEach-Object {$_.name})) | ForEach-Object {
    $_.Replace('live_aoi_order_monster_lifecycle_and_personal_rewards_keep_the_existing_direct_path','live_aoi_order_monster_lifecycle_uses_fifo_and_personal_rewards_stay_direct')
})
$focusedLog = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'probe14-personal-projection-focused-modules-v1.log')
$focusedNames = @([regex]::Matches($focusedLog,'(?m)^test ([^\s]+) \.\.\. (?:ok|FAILED)\r?$') | ForEach-Object {$_.Groups[1].Value})
$overflowLog = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'probe14-personal-projection-overflow-green-v1.log')
if ($focusedNames.Count -ne 27 -or $focusedLog -notmatch 'test result: FAILED\. 26 passed; 1 failed; 0 ignored;' -or $overflowLog -notmatch 'test result: ok\. 1 passed; 0 failed; 0 ignored;') { throw 'Expected final focused evidence and repaired overflow result' }
$selected = @($allExisting | Where-Object {$_ -notin $focusedNames})
if ($allExisting.Count -ne 186 -or $selected.Count -ne 178 -or @($selected | Sort-Object -Unique).Count -ne 178) { throw 'Expected 178 not-yet-repeated existing adjacent names' }
function Read-CheckedCandidate {
    @($candidate.files | Sort-Object path | ForEach-Object {
        $sourcePath=Join-Path $taskRepository $_.path
        $actual=[pscustomobject]@{path=$_.path;bytes=(Get-Item -LiteralPath $sourcePath).Length;sha256=(Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash.ToLowerInvariant()}
        if ($actual.bytes -ne $_.bytes -or $actual.sha256 -ne $_.sha256) { throw "Frozen source changed: $($_.path)" }
        $actual
    })
}
$before=Read-CheckedCandidate
$testArguments=@('test','--locked','--offline','-p','mir2-gateway','--lib','--','--test-threads=1','--exact')+$selected
Set-Location -LiteralPath (Join-Path $taskRepository 'mir2-web3')
& (Join-Path $taskBase 'run-capacity-cargo.ps1') -LogName 'probe14-personal-projection-remaining-adjacent-v2.log' -CargoArgs $testArguments
$testExit=$LASTEXITCODE
$after=Read-CheckedCandidate
if (($before|ConvertTo-Json -Compress) -cne ($after|ConvertTo-Json -Compress)) { throw 'Source changed during remaining adjacent regression' }
$logPath=Join-Path $taskBase 'probe14-personal-projection-remaining-adjacent-v2.log'
$logText=Get-Content -Raw -LiteralPath $logPath
$expectedSummary=$logText -match 'test result: ok\. 174 passed; 0 failed; 4 ignored;'
$receipt=[ordered]@{schemaVersion=1;verifiedAt=[DateTime]::UtcNow.ToString('o');coordinatorActuallyExecuted=$true;toolchain='1.95.0';sourceInputCount=44;sourceBefore=$before;sourceAfter=$after;sourceUnchangedBeforeAfter=$true;sourcePins=$candidate.sourcePins;selectedCount=178;selectedExactNames=$selected;coveredFocusedNamesNotRepeated=$focusedNames;renamedTestMappedExplicitly=$true;exitCode=$testExit;expectedSummaryMatched=$expectedSummary;ignoredAreNotPassed=$true;isolatedQuestTestsStillPending=$true;log=[ordered]@{path=$logPath;bytes=(Get-Item -LiteralPath $logPath).Length;sha256=(Get-FileHash -LiteralPath $logPath -Algorithm SHA256).Hash.ToLowerInvariant()};capacityAccepted=$false}
$receiptPath=Join-Path $taskBase 'probe14-personal-projection-remaining-adjacent-execution-v2.json'
$receiptBytes=[Text.UTF8Encoding]::new($false).GetBytes(($receipt|ConvertTo-Json -Depth 9)+[Environment]::NewLine)
$receiptStream=[IO.File]::Open($receiptPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try {$receiptStream.Write($receiptBytes,0,$receiptBytes.Length)} finally {$receiptStream.Dispose()}
if($testExit -ne 0){exit $testExit}
if(-not $expectedSummary){throw 'Expected 174 passed and 4 explicit ignored summary absent'}
exit 0
