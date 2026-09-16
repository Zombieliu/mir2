$env:CARGO_TARGET_DIR = 'C:\Users\Administrator\AppData\Local\Temp\mir2-trade-gold-simulation-20260908'
$env:RUST_TEST_THREADS = '1'
cargo +1.95.0 test -p mir2-simulation --locked --target x86_64-pc-windows-msvc --config 'profile.test.package.mir2-simulation.debug=1' --lib runtime::zone *> C:\mir2-shared-postguard-zone.log
$zoneExit = $LASTEXITCODE
cargo +1.95.0 test -p mir2-simulation --locked --target x86_64-pc-windows-msvc --config 'profile.test.package.mir2-simulation.debug=1' --test shared_monster_hell_ai --test shared_zone *> C:\mir2-shared-postguard-integration.log
@{zoneExit=$zoneExit;integrationExit=$LASTEXITCODE;entityCombatSHA256=(Get-FileHash apps/simulation/src/runtime/zone/runtime/entity_combat.rs -Algorithm SHA256).Hash} | ConvertTo-Json | Set-Content C:\mir2-shared-postguard-result.json -Encoding utf8
