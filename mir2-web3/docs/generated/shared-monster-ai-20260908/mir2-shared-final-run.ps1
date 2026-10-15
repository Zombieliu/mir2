$ErrorActionPreference = 'Continue'
$env:CARGO_TARGET_DIR = 'C:\Users\Administrator\AppData\Local\Temp\mir2-trade-gold-simulation-20260908'
$env:RUST_TEST_THREADS = '1'
$codePaths = @(rg --files apps/simulation apps/gateway packages/game-data packages/protocol -g '*.rs' -g 'Cargo.toml') + @('Cargo.lock','Cargo.toml')
$before = @{}
foreach ($path in $codePaths) { $before[$path] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
$before | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath C:\mir2-shared-final-source-before.json -Encoding utf8
cargo +1.95.0 test -p mir2-simulation --locked --target x86_64-pc-windows-msvc --config 'profile.test.package.mir2-simulation.debug=1' --no-fail-fast *> C:\mir2-shared-final-full-simulation.log
$simExit = $LASTEXITCODE
cargo +1.95.0 test -p mir2-gateway --locked --target x86_64-pc-windows-msvc --config 'profile.test.package.mir2-simulation.debug=1' --config 'profile.test.package.mir2-gateway.debug=1' --no-fail-fast *> C:\mir2-shared-final-full-gateway.log
$gatewayExit = $LASTEXITCODE
$after = @{}
foreach ($path in $codePaths) { $after[$path] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash }
$after | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath C:\mir2-shared-final-source-after.json -Encoding utf8
$changed = @($codePaths | Where-Object { $before[$_] -ne $after[$_] })
@{ simulationExit = $simExit; gatewayExit = $gatewayExit; changedDuringRun = $changed } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath C:\mir2-shared-final-run-result.json -Encoding utf8
