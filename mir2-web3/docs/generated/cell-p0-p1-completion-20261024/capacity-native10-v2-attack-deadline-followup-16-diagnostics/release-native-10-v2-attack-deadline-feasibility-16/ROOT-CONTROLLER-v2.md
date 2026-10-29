# Probe16 Root controller

This controller only seals reviewed public source inputs and runs a separate read-only source guard. Root owns execution. It never invokes provisioning, accounts, Cargo, tests, a monitor, or a realm.

The input revision and independent review are explicit arguments. Historical runtime-prepared-v1.json stays unchanged. The latest reviewed runtime-prepared-v2.json is 27071 bytes, SHA dd09c3db343dd150cc1de53ffd63bcfa31d2c10ee6493bfc42a633e2526a9c66; the controller derives counts and exact pins from whichever final revision the independent review binds. It does not hardcode this revision or its counts.

Root runs these commands in this directory, replacing REVIEW_FILE and REVIEW_SHA with the actual final independent review path and SHA:

```powershell
node ./root-seal-and-guard-v2.mjs --seal-reviewed-inputs --prepared-file=C:/mir2-build/p01-completion-20261024/release-native-10-v2-attack-deadline-feasibility-16/runtime-prepared-v2.json --prepared-sha256=dd09c3db343dd150cc1de53ffd63bcfa31d2c10ee6493bfc42a633e2526a9c66 --review-file=REVIEW_FILE --review-sha256=REVIEW_SHA
node ./root-seal-and-guard-v2.mjs --readonly-sourceguard
```

After both succeed, Root may separately invoke the existing authorization-gated preparation and runner entries. Those steps are absent from this controller. No seal or guard has been executed as part of controller preparation.

Before creating the new freeze, the controller checks all reviewed local/external pins, its own preparation and source, the exact review, and the actual new full16 validator. That validator invokes the original-location strict44 source library. Driver evidence, six immutable declarations, compact scene and historical wire source are checked separately. The preflight does not call the own input-lib.verifyRelease that requires the future freeze. The independent readonly mode does call it after the freeze exists.

Final files are canonical-path deduplicated, equal duplicate pins are required, and at most128 unique public source/evidence files are allowed. The freeze adds the final runtime preparation, independent review, controller, controller preparation, this README and pinned full16 validator. Existing approved/prepared/static runtime files are not rewritten. Both new output files use wx and existing runtime/lock/private/output state is rejected.

Active backend and driver execution commit is d2407fb84055e2088e3ca6ccdb4f8b80d15a9d16; backend44 aggregate13e515 and binary1980b7 are checked by their original strict contracts. Current Node9 aggregate5ba46a has actual210-source evidence. Historical wire6d70/187+28 and adapter8 remain separate evidence and do not claim current210 coverage. This controller does not grant probe success, capacity/P0 acceptance, or P2 readiness.

Revision2 changes only the full16 validator argument from the repository root to input.project, its actual mir2-web3 project-root API. PREP and README names point to new v2 companions. The v1 source/preparation/README and its review remain unchanged. Root observed the first v1 attempt fail before outputs; this preparation does not claim original stderr capture or a repeated execution. The exact inverse proof preserves the three unique source substitutions.
