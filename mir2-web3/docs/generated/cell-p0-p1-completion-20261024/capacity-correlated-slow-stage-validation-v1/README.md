# Connection-correlated existing slow-stage validation

Five existing spans now attach the existing opaque connection number: session_action.execute/flush and runtime_tick.execute/save_checkpoint/flush. Thresholds, authentication, execution order, snapshots, saves, typed receipts and game acceptance rules remain unchanged. No account, token or packet body is added.

Actual focused Rust results: **8 passed = original 7 + 1 new format regression**. The new test covers every label with distinct connections17/18, disabled legacy output, unchanged thresholds and no queued metadata. Rustfmt check and diff check passed; no unrelated format was written.

This increment has no new Release or load evidence. The previous d879 binary does not contain it. A future strict parser must permit these five existing labels with connection metadata. No capacity, P0/P1 or fixed Cell acceptance follows.

- [Before](source/web-before-v1.rs.txt) / [after](source/web-after-v1.rs.txt): exact raw source saved as text, with no real .rs file added to documentation.
- [Mutation and inverse](validation/mutation-v1.json), [actual focused log](validation/focused-v1.log), [execution](validation/focused-execution-v1.json), [validation](validation/validation-receipt-v1.json).
- [Independent source review](review/independent-review-v1.json) is static; it does not repeat tests.
- [Pure fmt command record](validation/rustfmt-command-record-v1.json) honestly records the observed successful tool result; there is no separately captured original stdout/stderr file.
- [Explicit whitelist](archive-tools/public-whitelist-v1.json) and [copy helper](archive-tools/archive-correlated-slow-stage-validation-v1.mjs) bind the bounded public scope.

The separately executed integration rustfmt and Root three-check summary are referenced in the [Node deadline validation archive](../capacity-attack-deadline-poll-fix-v1/README.md); those logs are not duplicated here. Business/resource capacity acceptance remains outstanding.
