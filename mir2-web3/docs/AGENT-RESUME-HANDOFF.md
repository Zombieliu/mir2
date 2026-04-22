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

- Active round: `2026-04-23-R25`
- Active task: Crystal storage item flag/rejection semantics.
- Active round state: partial code landed and `cargo fmt` passed, but R25 tests have not been run after the latest edits.
- Last completed round: `2026-04-23-R24`
- Backend/server parity estimate: `77.12%`
- Whole-project 1:1 estimate: roughly `61.7%`
- Latest completed code work: Crystal NPC `SellItem` `DontSell`, script `[Types]`, ack-only failure, `UserItem.Price() / 2`, and gold-cap semantics.
- Latest full backend verification: `cargo test -p mir2-simulation -- --test-threads=1` passed with 457 tests.
- Latest formatting verification: `cargo fmt` passed after the partial R25 edits. The latest `cargo fmt --check` remains the R24 pass.
- This directory is not known to be a git repository in the current environment, so rely on docs and file inspection rather than `git status`.

## R25 Partial State Before Reboot

R25 has started and should be continued, not restarted as a fresh task. Do not move the backend parity estimate above `77.12%` until R25 passes focused and broader tests.

Crystal source audit completed:

- `C.StoreItem` carries `from` and `to`; `S.StoreItem` returns `from`, `to`, and `success`.
- `C.TakeBackItem` carries `from` and `to`; `S.TakeBackItem` returns `from`, `to`, and `success`.
- Crystal gates both actions on active `[@STORAGE]`, NPC range, and `CanAccessStorage`.
- Store failure order is page/range/access, source/target bounds, `IsValidStorageIndex`, source item exists, `DontStore` / rental `DontStore`, then target slot empty.
- TakeBack failure order is page/range/access, source storage bounds, `IsValidStorageIndex`, target inventory bounds, source item exists, then target slot empty.
- Store target occupied fails; TakeBack target occupied fails. There is no swap.
- Store/TakeBack failures are ack-only `success=false` with no chat message.
- Store blocks base bind `DontStore` and rental `DontStore`; TakeBack has no bind/rental check.
- Current Rust simulation does not model the Crystal NPC object/range branch beyond the active service context.

Partial code already edited in `apps/simulation/src/runtime.rs`:

- Added Crystal `DontStore` bind constant.
- Added storage active-service helper and inventory-slot validation helper.
- Reworked `store_item_impl` to require active storage service, return ack-only failures, reject storage lock, invalid slots, inaccessible storage slot, missing item, `DontStore`, and occupied target.
- Reworked `take_back_item_impl` to require active storage service, return ack-only failures, reject storage lock, invalid slots, inaccessible storage slot, missing item, and occupied target.
- Partially patched storage tests to activate the storage service and expect Crystal ack-only behavior.

Rust Explorer audit completed:

- Packet dispatch is direct: `ClientPacket::StoreItem` / `TakeBackItem` route to `store_item_impl` / `take_back_item_impl`.
- The new active-service gate only accepts `active_npc_service.label_key == "STORAGE"`.
- `record_crystal_npc_service_context` currently records service context for sell/goods/repair-style packets, but not `NPCStorage`.
- Because `set_dialog` clears `active_npc_service`, a real `@Storage` NPC flow may emit `NPCStorage` but still leave `StoreItem` / `TakeBackItem` failing unless the test-only helper sets service state.
- Smallest next patch: include `NPCStorage` in service-context activation and add an end-to-end regression that opens an NPC `@Storage` page, then stores/takes back successfully without using the test-only helper.

R25 not yet verified:

- `cargo fmt` passed after the partial R25 edits, but `cargo fmt --check` has not been rerun.
- No R25 storage tests have been run after the partial edits.
- Some storage tests may still need active-service setup or expectation updates.

Immediate restart commands:

```powershell
cd E:\mir2\mir2-web3
rg -n "StoreItem \{|TakeBackItem \{|store_item|take_back|storage_" apps\simulation\src\runtime.rs
cargo fmt --check
cargo test -p mir2-simulation storage -- --test-threads=1 --nocapture
cargo test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo test -p mir2-simulation -- --test-threads=1
```

Likely remaining R25 work:

- Fix any compile/test failures from the partial storage test edits.
- Record `NPCStorage` as an active Crystal storage service in the normal NPC link flow.
- Add a focused `DontStore` rejection test using a Crystal item with bind flag `0x0008`, such as a mapped sealed/rental-safe fixture if available.
- Add or keep coverage proving inactive storage service returns only `StoreItem(success=false)` / `TakeBackItem(success=false)`.
- Update roadmap/progress/server-parity docs only after tests pass.

## Last Completed Round: R24

R24 aligned current Crystal NPC `SellItem` behavior:

- `SellItem` now returns ack-only failures for zero count, inactive service, missing item, oversized count, `DontSell`, and partial-stack gold overflow.
- Script `[Types]` mismatch emits `CannotSellItemHere` plus the failure ack.
- Sale pages remain Crystal-compatible: `@SELL` and `@BUYSELL` are accepted; `@BUYSELLNEW` opens UI packets but is not accepted by `PlayerObject.SellItem`.
- Sale payout now follows Crystal `UserItem.Price() / 2`, including durability and added-stat price factors for mapped Crystal items.
- Partial-stack overflow rejects before mutation; full-stack overflow succeeds and clamps gained gold, including `GainedGold(0)` when already capped.
- Buy-back tests now sell allowed WickedTrader item types rather than potions rejected by that script's `[Types]`.

R24 verification commands:

```powershell
cargo fmt
cargo fmt --check
cargo test -p mir2-simulation sell_item -- --test-threads=1 --nocapture
cargo test -p mir2-simulation sell -- --test-threads=1 --nocapture
cargo test -p mir2-simulation item -- --test-threads=1 --nocapture
cargo test -p mir2-simulation -- --test-threads=1
```

## Active Round: R25 Continuation

The active target remains Crystal storage item flag/rejection semantics. The Crystal source audit is complete and partial Rust code exists. Continue by running `cargo fmt --check`, repairing tests, and verifying the R25 patch.

Confirmed source facts:

- Crystal gates on `@STORAGE`, NPC object/range, password lock, `DontStore` for store only, storage capacity, and direct slot indexes.
- `StoreItem` / `TakeBackItem` send action acks for modeled failures and successes; no system chat is emitted for the storage rejection branches covered by R25.
- Current Rust implementation and tests are in `apps/simulation/src/runtime.rs`; search for `store_item`, `take_back`, `ServerPacket::StoreItem`, `ServerPacket::TakeBackItem`, `locked_storage`, and `storage_`.

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

## R25 Suggested Subagent Prompts

Crystal Explorer prompt:

```text
In E:\mir2\mir2-web3, do a read-only Crystal source audit for PlayerObject.StoreItem and TakeBackItem / storage behavior. Answer with source file paths and line numbers. Need: client packet fields; active @Storage page and NPC range requirements; inventory/storage UniqueID/count lookup; DontStore/rental/bound restrictions; storage password lock behavior; full storage/full bag behavior; exact rejection order; whether each failure is silent, sends StoreItem/TakeBackItem ack, or sends a system message. Do not edit files.
```

Rust Explorer prompt:

```text
In E:\mir2\mir2-web3, do a read-only audit of current Rust storage implementation and tests. Map ClientPacket::StoreItem/TakeBackItem dispatch, active NPC service state, storage lock/password handling, item lookup/count handling, bind flag helpers, full storage/full bag checks, and focused storage tests. Recommend the smallest safe R25 patch after Crystal semantics are known. Do not edit files.
```

Backend Worker prompt, only after Crystal semantics are known:

```text
Implement the bounded R25 storage parity patch in E:\mir2\mir2-web3. You are not alone in the codebase; do not revert others' edits. Own only apps/simulation/src/runtime.rs unless explicitly told otherwise. Align storage item flag/rejection semantics with the audited Crystal behavior, add focused tests, run cargo fmt, and report changed files plus tests. Do not update docs.
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
