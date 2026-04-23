# Agent Resume Handoff

Last updated: 2026-04-23

Purpose: this file is the restart-safe handoff for continuing the autonomous Crystal / Mir2 1:1 push after the Codex session is closed, the machine is rebooted, or chat context is lost. A new session should not depend on prior chat history; read this file plus the queue/run-log docs and continue from the active round.

## Resume Order

1. Open `E:\mir2\mir2-web3`.
2. Read these files first:
   - `docs/AGENT-RESUME-HANDOFF.md`
   - `docs/AGENT-ORCHESTRATION.md`
   - `docs/AGENT-TASK-QUEUE.md`
   - `docs/AGENT-RUN-LOG.md`
   - `docs/CRYSTAL-1TO1-ROADMAP.md`
   - `docs/BACKEND-1TO1-PROGRESS.md`
3. Treat `docs/AGENT-TASK-QUEUE.md` as the source of truth for the active round.
4. Continue autonomously from the active round. Do not repeat completed rounds unless tests or code inspection show a regression.
5. Use subagents only for clearly bounded parallel work. Keep one writer per high-conflict file.

## Current Checkpoint

- Active round: `2026-04-23-R31`
- Active task: select the next highest-value small unchecked parity task after verified R30 rental binding flag parity completion.
- Active round state: selection only. R30 is complete and should not be reopened unless tests or source inspection reveal a regression.
- Last completed round: `2026-04-23-R30`
- Backend/server parity estimate: `77.18%`
- Whole-project 1:1 estimate: roughly `61.7%`
- Latest completed code work: runtime item/equipment state now preserves rental `BindingFlags`, exposes them through `UserItem.RentalInformation`, rejects rental `DontStore` in `StoreItem`, and rejects rental `DontUpgrade` ack-only for current socket/upgrade `CombineItem` branches.
- Latest full backend verification: `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1` passed with 472 tests.
- Latest formatting verification: `cargo +1.89.0 fmt --check` passed.
- Repository status note: `mir2` is a git repository on `main`; the known unrelated dirty item is outer-repo submodule drift at `refactor-pwa`. Do not revert or commit that drift unless explicitly asked.
- Toolchain note: this Mac environment needs `cargo +1.89.0` for Rust verification because the default `rustc 1.87.0` fails on locked `bevy_* 0.17.3`.

## R25 Completed State

R25 is complete and already accounted for. Do not rerun or reopen it unless current tests or code inspection show a regression.

Crystal source audit confirmed:

- `C.StoreItem` carries `from` and `to`; `S.StoreItem` returns `from`, `to`, and `success`.
- `C.TakeBackItem` carries `from` and `to`; `S.TakeBackItem` returns `from`, `to`, and `success`.
- Crystal gates both actions on active `[@STORAGE]`, NPC range, and `CanAccessStorage`.
- Store failure order is page/range/access, source/target bounds, `IsValidStorageIndex`, source item exists, `DontStore` / rental `DontStore`, then target slot empty.
- TakeBack failure order is page/range/access, source storage bounds, `IsValidStorageIndex`, target inventory bounds, source item exists, then target slot empty.
- Store target occupied fails; TakeBack target occupied fails. There is no swap.
- Store/TakeBack failures are ack-only `success=false` with no chat message.
- Store blocks base bind `DontStore` and rental `DontStore`; TakeBack has no bind/rental check.
- Current Rust simulation still models the service-context branch rather than a full NPC object/range check, but it now preserves the real `NPCStorage` activation path used by imported `@Storage` dialogs.

Implemented code/results:

- Added Crystal `DontStore` bind constant.
- Added storage active-service helper and inventory-slot validation helper.
- Reworked `store_item_impl` to require active storage service, return ack-only failures, reject storage lock, invalid slots, inaccessible storage slot, missing item, `DontStore`, and occupied target.
- Reworked `take_back_item_impl` to require active storage service, return ack-only failures, reject storage lock, invalid slots, inaccessible storage slot, missing item, and occupied target.
- Recorded `NPCStorage` in the normal service-context activation path so a real `@Storage` dialog can store/take back without the test-only helper.
- Added an end-to-end regression that opens the imported storage page and proves store/take-back succeeds through the actual NPC flow.
- Added a Unix/Mac `crystal_local_time_snapshot()` implementation using `libc`; the full suite exposed a pre-existing non-Windows test gap in current NPC time-condition coverage.
- Added direct `libc = "0.2"` in `apps/simulation/Cargo.toml` and refreshed `Cargo.lock`.

Rust Explorer audit completed:

- Packet dispatch is direct: `ClientPacket::StoreItem` / `TakeBackItem` route to `store_item_impl` / `take_back_item_impl`.
- The new active-service gate only accepts `active_npc_service.label_key == "STORAGE"`.
- `record_crystal_npc_service_context` now records `NPCStorage`, closing the real-dialog activation gap that previously only test helpers covered.

R25 verification completed:

```powershell
cd E:\mir2\mir2-web3
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation crystal_npc_storage_service_context_allows_store_and_take_back_without_helper -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation crystal_npc_time_and_bag_conditions_follow_runtime_state -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 72 / 72 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 458 / 458 passed

## Last Completed Round: R30

R30 aligned the current rental binding flag item paths:

- Added runtime persistence for `UserItemRentalInformation.BindingFlags` through item/equipment state, inventory/equipment round-trips, and `UserItem.RentalInformation` payload generation.
- `StoreItem` now rejects both base `DontStore` and rental `DontStore`, matching Crystal's storage bind checks.
- Current inventory-grid `CombineItem` shape-7 socket and shape-3/4 upgrade branches now reject rental `DontUpgrade` ack-only, preserving the source item and target state.
- The round intentionally did not add a seal rental check because the audited Crystal paths only checked rental `DontUpgrade` on socket and upgrade branches.
- Remaining bounded combine gaps include hero-inventory handling, belt/id-collision cleanup, player `GemRatePercent`, and other gem-family branches.

R30 verification commands:

```powershell
cargo +1.89.0 fmt
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 13 / 13 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 17 / 17 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 85 / 85 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 472 / 472 passed

## Previous Completed Round: R29

R29 aligned the next bounded real client `CombineItem` branch:

- Added Crystal repair-combine parity for shape `1/2/5/6` sources in packet-driven inventory-grid `CombineItem`.
- Runtime now rejects `DontRepair` and wrong hammer-vs-sewing target families ack-only, matching Crystal's no-chat failure behavior for those branches.
- Full-durability targets now emit Crystal `ItemNoRepairNeeded` hint plus failure ack instead of silently mutating or consuming the source.
- Successful repair-combine now mutates durability, emits `ItemRepaired`, consumes the source stack, and ends with a success `CombineItem` ack.
- This round remains intentionally bounded: hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, player `GemRatePercent`, and other remaining gem-family branches stay open.

R29 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 11 / 11 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 83 / 83 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 469 / 469 passed

## Previous Completed Round: R28

R28 aligned the shared Crystal `CombineItem` target gate:

- Added the Crystal top-level target item-type gate to packet-driven `CombineItem`, matching `PlayerObject.CombineItem` by ack-failing any target outside item types `1..=11` before socket/seal/upgrade branch-specific handling.
- This closes a real parity gap where current packet `CombineItem` could previously emit `InvalidCombination` for shape-7 on non-equipment targets or even seal non-equipment inventory items.
- Added focused regressions that prove stage-5-style socket targets such as `BengalTiger` are rejected under the Crystal item-type window and that shape-8 seal attempts against inventory consumables fail ack-only without mutation.
- The round remains intentionally bounded: hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, player `GemRatePercent`, and other gem-family branches remain open.

R28 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 8 / 8 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 80 / 80 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 466 / 466 passed

## Previous Completed Round: R27

R27 aligned the next bounded real client `CombineItem` branch:

- Added Crystal `ServerPacket::ItemUpgraded` / id `216` to protocol ids, codec, gateway JSON conversion, and trace output.
- Runtime `ClientPacket::CombineItem` now covers the current inventory-grid shape-3/4 gem/orb upgrade semantics instead of stopping at socket/seal-only handling.
- Persisted `gem_count` through runtime item state, inventory/equipment round-trips, and `UserItem` encoding so upgrade state survives the same flows as Crystal.
- Added focused regressions for upgrade success, max-added-stat rejection, invalid combinations, and failure-destroy behavior.
- This round is intentionally bounded: full Crystal target-type gating across combine branches, hero-inventory handling, belt/id-collision cleanup, rental `DontUpgrade`, and player `GemRatePercent` remain open.

R27 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-protocol item_slot_seal_and_upgrade_server_packets_use_crystal_ids -- --nocapture
cargo +1.89.0 test -p mir2-gateway item_slot_and_seal_server_events_expose_crystal_payload_fields -- --nocapture
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

Observed results:

- `cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture`: 7 / 7 passed
- `cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture`: 16 / 16 passed
- `cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture`: 79 / 79 passed
- `cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1`: 465 / 465 passed

## Previous Completed Round: R26

R26 aligned the current real client `CombineItem` packet path:

- Added Crystal `ClientPacket::CombineItem` / id `111` and `ServerPacket::CombineItem` / id `215` to protocol ids, codec, and trace output.
- Gateway JSON now exposes Crystal `CombineItem` payload fields (`grid`, `idFrom`, `idTo`, `success`, `destroy`).
- Runtime `ClientPacket::CombineItem` now dispatches to the current inventory-grid shape-7 socket-growth and shape-8 seal semantics instead of leaving those flows Stage-5-only.
- Successful packet-driven socket/seal changes now mutate the same persisted runtime state as the existing helpers, including `UserItem.SealedInfo`, inventory/equipment round-trips, and item-change packets.
- This round is intentionally bounded: full Crystal target-type gating, hero-inventory handling, and other gem/combine branches remain open.

R26 verification commands:

```powershell
cargo +1.89.0 fmt --check
cargo +1.89.0 test -p mir2-protocol item_and_combat_client_packets_use_crystal_payloads -- --nocapture
cargo +1.89.0 test -p mir2-protocol item_action_ack_server_packets_use_crystal_ids -- --nocapture
cargo +1.89.0 test -p mir2-gateway combine_item_server_event_exposes_crystal_payload_fields -- --nocapture
cargo +1.89.0 test -p mir2-simulation combine_item_packet -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo +1.89.0 test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo +1.89.0 test --locked -p mir2-simulation -- --test-threads=1
```

## Active Round: R31 Selection

The active target is no longer R30 implementation. Use this round to choose the next bounded parity bite from the unchecked backend/frontend queue before starting more code work.

Current selection constraints:

- Prefer the highest-value small unchecked task over a large multi-system refactor.
- Keep one writer on `apps/simulation/src/runtime.rs`.
- Do not move the backend parity estimate again until the selected R31 task is implemented, verified, and documented.

Explorer recommendations already captured in docs/run log:

- Frontend candidate: screenshot baseline pack plus stage screenshot comparison harness.
- Backend candidates should now be re-selected from the remaining queue; do not reopen the current bounded `CombineItem` work unless protocol/runtime regressions appear.
- Current recommendation: choose the next bounded backend/frontend bite from the unchecked queue with the same one-writer discipline used in R25-R29.

## Subagent Workflow After Restart

The user explicitly wants the previous multi-agent workflow to continue. Use this pattern:

1. Coordinator reads queue/log/roadmap locally and chooses the active task.
2. Spawn a Crystal Explorer for source behavior. Read-only, no file edits.
3. Spawn a Rust Explorer for local code/test map when the implementation surface is not already clear. Read-only, no file edits.
4. Only spawn a Worker when the implementation scope is bounded and its write set does not overlap another writer.
5. For `apps/simulation/src/runtime.rs`, keep the Coordinator as the only writer unless a worker has a very narrow non-overlapping patch.
6. Coordinator integrates, runs focused tests, then broader regressions if shared behavior changed.
7. Update all relevant docs before opening the next round:
   - `docs/AGENT-TASK-QUEUE.md`
   - `docs/AGENT-RUN-LOG.md`
   - `docs/CRYSTAL-1TO1-ROADMAP.md`
   - `docs/BACKEND-1TO1-PROGRESS.md`
   - `docs/CRYSTAL-SERVER-PARITY.md`

## Model And Effort Policy

Use the observed quota profile from the prior session unless the new session shows a different one:

- Prefer `gpt-5.3-codex-spark` because Spark-specific quota was abundant.
- Use `xhigh` for the Coordinator and high-risk `runtime.rs` implementation.
- Use `high` for backend/frontend workers.
- Use `medium` for read-only explorers and docs/QA work.
- Avoid multiple code-writing agents on the same file.

## R31 Suggested Subagent Prompts

Crystal Explorer prompt:

```text
In E:\mir2\mir2-web3, do a read-only Crystal/source audit of the top unchecked backend/frontend queue candidates. Use docs/AGENT-TASK-QUEUE.md as the source of truth, identify the best next bounded parity bite, and summarize exact Crystal behavior, file paths, line numbers, and the smallest safe scope. Do not edit files.
```

Rust Explorer prompt:

```text
In E:\mir2\mir2-web3, do a read-only audit of the current Rust code/test surface for the top unchecked queue candidates. Recommend the smallest safe write set, likely regression risks, and focused/full verification commands for the best next bounded round. Do not edit files.
```

Backend Worker prompt, only after Crystal semantics are known:

```text
Implement the selected bounded R31 parity patch in E:\mir2\mir2-web3. You are not alone in the codebase; do not revert others' edits. Own only the explicitly assigned files, keep one writer on apps/simulation/src/runtime.rs when it is in scope, add focused regressions, run cargo +1.89.0 fmt/test commands, and report changed files plus tests. Do not update docs unless explicitly assigned.
```

## Ready-To-Paste Resume Prompt

Use this when reopening Codex:

```text
Continue E:\mir2\mir2-web3 toward 100% Crystal/Mir2 1:1 Candidate. Read docs\AGENT-RESUME-HANDOFF.md, docs\AGENT-ORCHESTRATION.md, docs\AGENT-TASK-QUEUE.md, docs\AGENT-RUN-LOG.md, docs\CRYSTAL-1TO1-ROADMAP.md, and docs\BACKEND-1TO1-PROGRESS.md first. Continue from the active round using the previous subagent workflow. Do not repeat completed rounds and do not ask for routine confirmation. Use gpt-5.3-codex-spark with xhigh/high for implementation and medium/high explorers unless current quota says otherwise.
```

## Completion Accounting

- Small Crystal edge-semantics rounds may move backend parity by only `0.01%`; this is expected at the current maturity level.
- Larger cross-cutting systems can move more, but only after code, tests, and docs are all complete.
- The backend score is not the whole-project score. Backend has 45% weight in `docs/CRYSTAL-1TO1-ROADMAP.md`.
- Do not mark a checkbox complete from inspection alone; use source evidence plus tests or trace evidence.
