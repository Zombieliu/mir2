# Personal production transaction verification — 2026-10-08

Four actual, serial calls through the original CargoGuard on toolchain 1.95.0:

| Call | Actual passing named tests | Failed / ignored | Scope |
| --- | ---: | --- | --- |
| production-tests-01 | 19 | 0 / 0 | Production rules regression |
| simulation-tests-01 | 15 | 0 / 0 | New personal production Session/File-source transactions |
| npc-regression-01 | 30 | 0 / 0 | Existing NPC purchase/owner recovery |
| save-regression-01 | 59 | 0 / 0 | Existing item validation, login/save, cooldown and mail boundaries |

Total: **123 actual passing tests**, including **15 new transaction tests**. No game, GUI, gateway, external database or installer was started. Test recipes/material aliases are isolated fixtures using real Crystal carriers, not accepted gameplay resource bindings.

Each call used `--locked --offline --jobs 1`, incremental compilation disabled and serial test threads. The first simulation test executable compilation took 2m23s; its tests took 7.44s. The rule/NPC/save tests took 0.57s / 26.56s / 11.54s respectively. Those are observed durations, not timeout promises.

Every actual nonce records FORWARD, `actualGetVolumeC`, original Policy B and completed/exited/disposed true with exit 0. Original tools/probe/authority pins remain unchanged. C's minimum sampled remaining bytes were 221967093760, above the original 53687091200 threshold. Root's sample-end freshness upper bound was at most 73.36ms; independent conservative sample-begin bounds were at most 863.8625ms, all within the original 2000ms threshold.

Source snapshots declare 14 / 452 / 452 / 452 inputs, all matched after their call and during final review. The independent source review binds the ten authored paths; the result review rechecked all 1370 declared rows and actual logs/nonce/tool pins. Raw `cargo.log`, source snapshots, policy/authority/review bindings and all five original nonce files are retained per call. The empty root result-audit attempt was caused by a helper regex missing namespace colons; the corrected `result-audit-02.json` checks actual names. No product source or assertion changed after source acceptance.

Two wrapper serial-Cargo preflights refused to run while another Cargo/rustc invocation existed. They did not spawn Cargo, guards or probes, execute tests or change policy. They are recorded separately and are not test failures. Four source review findings—argument order, error Display, complete equipment/whitespace UID scan and legal full-bag fixture—were fixed before any real Cargo invocation.

The transaction tests cover paid frozen jobs, live/File consistency, bag and belt inputs, bound output and partial refunds with fresh IDs, original-request replay/conflict, relogin, two-session CAS, whole-batch full/overweight rejection, ore purity versus count, black-iron protection, another character's high-UID equipped carrier with leading JSON whitespace, save/restore history protection, prepublication failure, ambiguous publication freeze and known postcommit receipt recovery.

Public production remains disabled: no player packet/NPC/gateway route, no ordinary resource bindings or UI acceptance. The adapter explicitly receives a durable Crafting UID authority; legacy issuers still need fleet-wide migration to that same authority. Postgres/fault verification and explicit unknown-publication reconciliation remain release prerequisites. History archival, gathering/planting, construction, food and guild workshops remain subsequent work. See [implementation progress](../../SLG-PRODUCTION-IMPLEMENTATION.zh-CN.md).
