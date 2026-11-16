# W13 ordinary economic preparation — original failure retained

This preparation-only continuation completed **29/100** ordinary actors and stopped on actor 30 L370e2a0t: **Navigation successful step budget exceeded (222)**. The [original report](1793076310690-5c1690ab.preparation-report.json), [public preparation samples](1793076310690-5c1690ab.preparation.jsonl) and [raw-copy manifest](copied-evidence-manifest.json) retain the failed verdict. There was no cohort activation or measurement; capacity/P0 acceptance and cold-bank eligibility remain false.

The source was the complete actual W12 cold bank and its latest authentication ledger, copied without editing game state, account identity, secrets or traffic history. The [source plan](source-bank-plan-pending-v1.json) and [actual exclusive-copy receipt](source-bank-copy-receipt-v1.json) record the public pins; no private payload is archived. sourceFirst4InventoryReuse=true remains a dedicated first 4 assertion, not a claim that all 29 were reused. All 29 completed actors separately have fresh stock/readiness of at least 12, normal logout and matching final normal-drain position. This is not yet a later durable ordinary-relogin proof.

## New ordinary economy evidence

Actors 22–29 each purchased 11 HP items for 440 ordinary gold and finished with 12 HP items. Exact source values vary; actor 23 sold 3 Venison rather than 2.

| Character index | Venison sold | Gold earned | Gold remaining | New hunts | Funding attempts |
| --- | ---: | ---: | ---: | ---: | ---: |
| 22 | 2 | 453 | 13 | 1 | 1 |
| 23 | 3 | 661 | 221 | 3 | 7 |
| 24 | 2 | 473 | 33 | 2 | 2 |
| 25 | 2 | 466 | 26 | 2 | 2 |
| 26 | 2 | 497 | 57 | 2 | 2 |
| 27 | 2 | 461 | 21 | 2 | 2 |
| 28 | 2 | 484 | 44 | 2 | 7 |
| 29 | 2 | 498 | 58 | 2 | 2 |

Their original purchase/sale, actual-capital and bounded approach proofs remain in the samples and manifest. Four recorded target-lost approach rows have navigationCalls>0 (actor 23 one; actor 28 three). Zero-navigation selection rejections stay separate; these records do not prove that every target loss was caused by AOI.

## Actor 30 and cleanup

The fresh initial scalar was HP 18/18, stock 1, gold 0 at 288,616. It issued 172 commands (161 movement: 113 Run + 48 Walk; 9 refresh, 1 interact, 1 dialog selection), with no attack/harvest/sale/purchase commands. Module elapsed time was 108.244s. It recorded 1 path hazard and 0 path replans, plus 6 hostile-near-Deer selection rejections with 0 navigation calls. The last capital proof was 0/440 and planning-only. The terminal navigation successful-step limit 222 and the module movement-command count 161 are different counters. The public evidence does not establish the unique cause of the terminal budget error.

The failed summary's after/readiness remain null and normalLogout=false. The public drain tail separately records the same actor at 296,616 and a successful ordinary logout ACK (314.906ms); neither proves final HP/stock. [Owned cleanup](cleanup-v1.json) retains the original driver exit 1, gateway/monitor exit 0, all 11 checks true and the monitor's normal requested-drain terminal reason. That terminal reason is not substituted for the earlier failure. [Health after](health-after-v1.json) has all 6 capacity counters zero. [Root independent OS cleanup](root-independent-cleanup-v1.json) at 2026-10-27T05:12:01.2355256Z confirms PID 62924 absent and ports 17500/17510 free. The [episode receipt](preparation-episode-receipt-v1.json) keeps preparationComplete/coldBankEligible false.

## Source-bound qualification and reused backend

Current driver/execution is 767cfa1535100cf38bf97cfe6d2402b5fd305c22. The [fresh Root receipt](node-v13/root-focused-receipt-v1.json) records **724=332+337+55**, zero fail/cancel/skip/todo, with 42 named sources unchanged before/after. The 32 static runtime JS sources are a separate denominator. The [runner](node-v13/root-validation-runner-v13.mjs) and all 3 actual stdout/stderr pairs are copied once:

- [Preparation/navigation stdout](node-v13/complete-preparation-and-protected-navigation.stdout.log), [stderr](node-v13/complete-preparation-and-protected-navigation.stderr.log).
- [Ordinary dependencies stdout](node-v13/ordinary-economy-and-navigation-dependencies.stdout.log), [stderr](node-v13/ordinary-economy-and-navigation-dependencies.stderr.log).
- [Full-lifecycle unit stdout](node-v13/ordinary-full-lifecycle-driver-units.stdout.log), [stderr](node-v13/ordinary-full-lifecycle-driver-units.stderr.log).

[Full16 v5 reuse proof](full16-reuse/ordinary-full16-reuse-proof-v13.json) retains actual d9ec full16 results after complete 14-driver and 55-backend equality; it is not a new full16 execution. [Root's preexecution verifier review](root-full16-reuse-tools-review-v1.json) truthfully retains actualProofExecuted=false at its own historical time; the later actual v13 proof is separate. Earlier verifier/schema history stays in [W10](../bank-prepare-v10/README.md).

The [frozen source record](frozen-source-v1.json) and [Root final binding](root-final-binding-receipt-v1.json) bind actual V6 backend ee06, all 55 raw/Git inputs and the original immutable EXE pin. Rust 32 and that backend Release are explicitly reused, with no new Rust build/test here. Their full originals remain in [W4](../bank-prepare-v4/README.md); no EXE or redundant large backend package is copied.

This archive contains 41 explicit public originals, plus this README and the copy manifest. Source and destination bytes are verified; Store/pool/ledger/secret bodies, journals, full recordings, raw Gateway logs and binaries are excluded. Private cold-bank originals remain untouched externally. Original 100 P0 measurement thresholds are unchanged. The user's future target is 500 online with 500 continuously fighting; W13 preparation does not validate that target, the original 100 profile, or sustainable two-hour supply.
