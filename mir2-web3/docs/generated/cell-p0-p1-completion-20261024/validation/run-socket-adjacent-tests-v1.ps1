$selection = Get-Content -LiteralPath 'C:/mir2-build/p01-completion-20261024/socket-diagnostics-adjacent-selection-v1.json' -Raw | ConvertFrom-Json
$projectRoot = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3'
foreach ($source in $selection.sourceFiles) {
    $file = Join-Path $projectRoot $source.file
    $actual = Get-FileHash -LiteralPath $file -Algorithm SHA256
    if ($actual.Hash.ToLowerInvariant() -ne $source.sha256 -or (Get-Item -LiteralPath $file).Length -ne $source.bytes) { throw 'Frozen test source changed' }
}
$testArgs = @('test','--locked','--offline','-p','mir2-gateway','--lib','--','--test-threads=1','--exact') + @($selection.tests)
& 'C:/mir2-build/cell-preproduction-p0-p1-20261024/run-cargo.ps1' -LogName 'p0-socket-diagnostics-adjacent-v1.log' -CargoArgs $testArgs
exit $LASTEXITCODE
