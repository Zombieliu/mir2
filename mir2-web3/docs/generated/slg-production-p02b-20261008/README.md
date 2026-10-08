# Staged item UID authority verification — 2026-10-08

Six final, actual serial calls through the original CargoGuard on toolchain 1.95.0, bound to authored source05:

| Call | Actual passing named tests | Failed / ignored | Scope |
| --- | ---: | --- | --- |
| uid-tests-05 | 11 | 0 / 0 | New trusted binding, actual staged purchases/splits and history boundaries |
| simulation-tests-03 | 18 | 0 / 0 | Personal production Session/File transactions, including 3 new authority/custody cases |
| npc-regression-02 | 35 | 0 / 0 | Existing purchase/owner recovery and 5 new File UID purchase cases |
| save-regression-02 | 59 | 0 / 0 | Existing login/save, item, cooldown and mail boundaries |
| inventory-regression-03 | 37 | 0 / 0 | Existing stack identity, splitting, belt movement and purchase evidence |
| uid-file-regression-01 | 17 | 0 / 0 | Original durable File allocator, restart, corruption and concurrency |

Total: **177 passing executions**, **19 new unique tests** (11 UID + 5 NPC + 3 production). Nested child-worker summaries in the concurrency log are not added to this total. The unchanged production rule crate's earlier 19 passes are historical and are not added either. This is controlled code/File-source verification, not ordinary player gameplay or public production activation.

All final calls used `--locked --offline --jobs 1`, disabled incremental compilation and serial test threads. Each declares 455 inputs, all byte-matched after its call and in the final Root audit. Fourteen authored files bind to source05; the 441 protected inputs also match production parent `fde81a0de5084d02b3daaa30bac4d4f1fde411f4` using canonical Git clean filters. This excludes concurrent frontend work from the publication. Five independent source reviews and the bounded cache/helper review are retained.

The original Guard executable/source, license, Cargo/rustup, PowerShell, probe, policy template and release thresholds are unchanged. Every final actual nonce records FORWARD, `actualGetVolumeC`, and Policy B completed/exited/disposed with exit0. The final minimum sampled C remaining bytes are 199120957440, above the original 53687091200 threshold. Root's conservative sample-end freshness upper bound is at most 81.9889ms, within the original 2000ms limit. The independent result review reports its own verification; observed compilation times are not timeout promises.

The 16 actual Cargo invocations (six final + ten historical) retain all original raw logs, policy/authority/acceptance/source bindings and **80 original nonce files**. Copies were byte-checked. `.gitattributes` preserves their original bytes, including empty files and Cargo trailing blank lines. The six final results supersede these historical attempts:

| Historical call | Actual result | Repair or qualification |
| --- | --- | --- |
| uid-tests-01 | E0599 compile failure, 0 tests | New fixture corrected to actual `world_snapshot()` |
| uid-tests-02 | 9 pass / 1 fail | Count bag and belt together because potion splitting selects belt first; quantity and identity assertions retained |
| simulation-tests-01 | 16 pass / 2 fail | Compare exact before/after high-water after trusted binding raises historical floor; original rejection and state assertions retained |
| inventory-regression-01 | 35 pass / 2 fail | Actual product fix: Legacy naked World must not read missing runtime config. Durable missing/mismatched config now returns error, with a new test |
| inventory-regression-02 | LNK1104 link failure, 0 tests | Restart Manager identified DeltaForce holding the old test executable. Copy non-executable cache into a fresh workspace target; preserve the user's game and all source/assertions/Guard settings |
| uid-tests-03 / uid-tests-04 / simulation-tests-02 / npc-regression-01 / save-regression-01 | Earlier passes | Superseded by later source changes; not accumulated into 177 |

Four wrapper preflights (review file pending or another Cargo/rustc active) created no Guard nonce, spawned no Cargo and ran no test. Slot waits that started no check are not test results. The empty `result-audit-01.json` is preserved: its helper attempted to sum OrderedDictionary keys using `Measure-Object -Property`. The fixed helper projects integers before summing; `result-audit-02.json` verifies all six exact counts. No product or assertion changed for that audit repair, and no extra Cargo run was needed.

This batch shares one trusted server-only File UID handle between config, sessions, inventory and personal crafting. It protects discarded IDs, historical merged-purchase incoming IDs, numeric custody reservations, restored bindings and replica/live service boundaries. Actual staged purchase failures preserve currency and stock; splitting acquires a new ID before consuming source quantities. Existing saved original requests remain queryable when new minting is fenced. Fresh directory relocation changed only `CARGO_TARGET_DIR`; the inverse helper substitution matches every baseline byte.

**P02b-1 is bounded preparation, not complete all-issuer migration.** Default config and the normal gateway do not enable the new binding. Remaining grants/add/normalization, drops, mining/fishing, mail, shops/exchanges, initial seeds and Hero issuance still require migration. Complete external/world/history census, legacy duplicate-ID reconciliation, one PostgreSQL authority and explicit unknown-publication reconciliation remain release prerequisites. No game/GUI, gateway restart, external database, deployment or installer was started. Ordinary gathering/planting, Bichon/Mongchon workshops, side-management UI, construction, food and guild production remain subsequent work. The normal catalog stays disabled. See [implementation progress](../../SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md).
