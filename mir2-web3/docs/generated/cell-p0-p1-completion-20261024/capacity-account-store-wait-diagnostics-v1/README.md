# AccountStore wait diagnostics validation

This archive binds the diagnostic-only simulation change to seven exact source pins and Root's completed Rust validation. The new three diagnostic tests and nineteen distinct existing adjacent tests passed: **22 distinct tests, zero failures and zero ignored**. The adjacent checks used `MIR2_GATEWAY_SLOW_STAGE_MS=1`; the two Rust executions preserved the exact seven source pins before and after. Root also checked all seven files with rustfmt 1.95.0, exit 0 and an empty log.

- [Functional aggregation and exact test union](verification/functional-validation-v1.json)
- [Source candidate and scope proof](source/source-candidate-v1.json)
- [Git baseline/raw-pin mapping](source/source-baseline-and-diff-v1.json)
- [Six existing-file diagnostic changes](source/simulation-source-changes-v1.patch)
- [One new private diagnostic module, stored as text](source/config_account_store_diagnostics.rs.txt)
- [Independent source review](verification/independent-source-review-v1.json)
- [Root focused execution receipt](validation/focused-execution-v1.json)
- [Root adjacent execution receipt and six logs](validation/adjacent/execution-receipt-v1.json)
- [Actual test inventory](validation/adjacent/test-list-v1.log)
- [Root rustfmt receipt](validation/rustfmt-execution-v1.json)
- [Read-only aggregation code](verification/aggregate-functional-v1.cjs)
- [Explicit public whitelist](public-whitelist-v1.json)

The original six source preimages are preserved by Git commit `7644faa14402aa1bfdad8096fedfe61dc6a0a660`, their exact raw pins and Git-filtered blob proof; this archive does not duplicate their full contents. The new private module is copied once with a `.rs.txt` destination, so the documentation archive does not introduce a Rust build input. The source-edit script is historical authoring evidence, not a runtime entry point.

The diagnostics are off by default and retain the existing positive slow-stage threshold. Inner timers only store bounded scalar samples; output occurs after the instrumented guards have left scope. The fourteen fixed stages and five fixed operation values contain no account/name/path payload. Durations are completed-occurrence maxima with explicit count/total fields, and nested spans overlap. An unfinished wait/holder produces no completed row; original API callers not connected to an outer record remain outside this coverage. The new nine-field slow-line grammar requires its separately reviewed new external consumer. Trace and duration integers are not claimed safe for JavaScript for every possible process lifetime.

The Node nine-input source, d240 driver freeze and prior 210-test evidence were rechecked as unchanged historical inputs, **not rerun**. This archive is bounded functional validation only: no new Release, fresh full16 or capacity run is certified here. Earlier Formal12 remains FAILED; P0 remains incomplete, capacity acceptance is false, and P2 remains frozen.
