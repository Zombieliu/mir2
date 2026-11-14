$ErrorActionPreference = 'Stop'
$taskBase = 'C:/mir2-build/p01-completion-20261024'
$taskRepository = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey'
$taskProject = Join-Path $taskRepository 'mir2-web3'
$baseline = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'socket-diagnostics-adjacent-selection-v1.json') | ConvertFrom-Json
$additional = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'probe14-personal-projection-adjacent-v1/existing-adjacent-selection-v1.json') | ConvertFrom-Json
$selected = @($baseline.tests) + @($additional.additionalExisting | ForEach-Object { $_.name })
if ($baseline.tests.Count -ne 161 -or $additional.additionalExisting.Count -ne 25 -or $selected.Count -ne 186 -or @($selected | Sort-Object -Unique).Count -ne 186) { throw 'Expected exact disjoint 161+25 existing regression selection' }
$changedPins = @(
    [pscustomobject]@{path='mir2-web3/apps/gateway/src/routing.rs';bytes=1389827;sha256='9135f2f3a019b04c1bb6ffe3717f730c65632df4e642702057bfbf7a699564de'},
    [pscustomobject]@{path='mir2-web3/apps/gateway/src/routing/tests/personal_monster_projection_tests.rs';bytes=30640;sha256='fe507e961403679b3b92f23986dff3f53bb52a5c528fae251dd2f7ec546197f0'},
    [pscustomobject]@{path='mir2-web3/apps/simulation/src/runtime/zone/runtime.rs';bytes=749298;sha256='d19840e87e6b2e718303d90d02a03e8c1c4941e069f0b168d9d253644950f3b6'}
)
$oldManifest = Get-Content -Raw -LiteralPath (Join-Path $taskBase 'backend-source-socket-diagnostics-v1.json') | ConvertFrom-Json
$expectedPins = @($oldManifest.files | Where-Object { $_.path -notin $changedPins.path }) + $changedPins
if ($expectedPins.Count -ne 42 -or @($expectedPins.path | Sort-Object -Unique).Count -ne 42) { throw 'Expected 39 unchanged plus 3 explicitly changed backend inputs' }
function Read-CheckedPins {
    @($expectedPins | Sort-Object path | ForEach-Object {
        $taskSourcePath = Join-Path $taskRepository $_.path
        $taskSourceItem = Get-Item -LiteralPath $taskSourcePath
        $taskSourceHash = (Get-FileHash -LiteralPath $taskSourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($taskSourceItem.Length -ne $_.bytes -or $taskSourceHash -ne $_.sha256) { throw "Frozen regression source changed: $($_.path)" }
        [pscustomobject]@{path=$_.path;bytes=$taskSourceItem.Length;sha256=$taskSourceHash}
    })
}
$before = Read-CheckedPins
$testArguments = @('test','--locked','--offline','-p','mir2-gateway','--lib','--','--test-threads=1','--exact') + $selected
Set-Location -LiteralPath $taskProject
& (Join-Path $taskBase 'run-capacity-cargo.ps1') -LogName 'probe14-personal-projection-adjacent-v1.log' -CargoArgs $testArguments
$testExit = $LASTEXITCODE
$after = Read-CheckedPins
if (($before | ConvertTo-Json -Compress) -cne ($after | ConvertTo-Json -Compress)) { throw 'Source changed during adjacent regression' }
$logPath = Join-Path $taskBase 'probe14-personal-projection-adjacent-v1.log'
$logText = Get-Content -Raw -LiteralPath $logPath
$expectedSummary = $logText -match 'test result: ok\. 184 passed; 0 failed; 2 ignored;'
$receipt = [ordered]@{schemaVersion=1;verifiedAt=[DateTime]::UtcNow.ToString('o');coordinatorActuallyExecuted=$true;toolchain='1.95.0';sourceInputs=42;sourceBefore=$before;sourceAfter=$after;sourceUnchangedBeforeAfter=$true;sourcePins=$changedPins;selectedExactNames=$selected;selectedCount=186;exitCode=$testExit;expectedSummaryMatched=$expectedSummary;focusedNewNineNotRepeated=$true;ignoredAreNotPassed=$true;log=[ordered]@{path=$logPath;bytes=(Get-Item -LiteralPath $logPath).Length;sha256=(Get-FileHash -LiteralPath $logPath -Algorithm SHA256).Hash.ToLowerInvariant()};capacityAccepted=$false}
$receiptPath = Join-Path $taskBase 'probe14-personal-projection-adjacent-execution-v1.json'
$receiptBytes = [Text.UTF8Encoding]::new($false).GetBytes(($receipt | ConvertTo-Json -Depth 9) + [Environment]::NewLine)
$receiptStream = [IO.File]::Open($receiptPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try { $receiptStream.Write($receiptBytes,0,$receiptBytes.Length) } finally { $receiptStream.Dispose() }
if ($testExit -ne 0) { exit $testExit }
if (-not $expectedSummary) { throw 'Expected 184 passed and 2 original ignored summary absent' }
exit 0
