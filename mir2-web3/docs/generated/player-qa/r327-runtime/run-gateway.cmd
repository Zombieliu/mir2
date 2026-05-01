cd /d E:\mir2\mir2-web3
set MIR2_GATEWAY_TCP_ADDR=127.0.0.1:7310
set MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7110
set MIR2_ACCOUNT_STORE_PATH=E:\mir2\mir2-web3\.mir2-data\accounts.json
set MIR2_GATEWAY_MOVE_LOG=1
cargo +1.89.0 run --locked -p mir2-gateway --bin mir2-gateway > docs\generated\player-qa\r327-runtime\gateway.log 2>&1
