& 'C:/mir2-build/p01-completion-20261024/run-capacity-cargo.ps1' -LogName 'probe14-personal-projection-baseline-overflow-v1.log' -CargoArgs @('test','--locked','--offline','-p','mir2-gateway','--lib','live_aoi_order_wss_overflow_stops_broken_epoch_preserves_personal_state_and_fences_replacement','--','--test-threads=1','--nocapture')
exit $LASTEXITCODE
