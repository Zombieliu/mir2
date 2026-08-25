# Durable Ground-Drop Settlement — Slice D Report

Date: 2026-08-26
Status: accepted for the bounded Slice D item-identity settlement gate

## Scope

Slice D hardens the item-identity settlement boundary after a ground-drop pickup when the process, connection, or database response can fail between projection and acknowledgement. It is a bounded backend reliability slice supporting the Windows Candidate vertical gameplay flow. It is not a claim that every Crystal map, class, system, packet, or visual is already Accepted.

## Implemented guarantees

- PostgreSQL idempotency lookup and transact use the same per-key advisory transaction lock, preventing an unlocked lookup/transact race.
- Fenced settlement keeps `OutcomeUnknown` for bootstrap, initial lookup, transaction, and post-transaction lookup uncertainty. An unknown result is retained for retry rather than treated as a confirmed rejection.
- Ground-drop settlement applies the authoritative projection through the simulation atomic projection-plus-save boundary. A save failure rolls the projection back; a retry can reuse the stable operation identity without awarding the payload twice.
- Active ground claims can be detached into a durable unresolved-settlement record. The record retains stable account/character identity, the complete Zone key, the original claim ticket, idempotency identity, and the exact drop payload.
- World-only checkpoint conversion detaches active claims before session state is discarded, and restore validates the detached ticket/tombstone before making a drop visible again.
- StartGame performs pending shared-settlement recovery after the player rejoins the Zone, so recovery is not dependent on the old process-local session object.
- Teardown and `Drop` apply only already-finalized delivery/rollback packets. They do not perform unresolved settlement without an ordered economy context, so uncertainty is retained for a later fenced retry.
- A fresh factory/new-login regression restores an unresolved ground claim after a process-style checkpoint, credits the character exactly once during StartGame recovery, retains the claim tombstone, and proves a later KeepAlive cannot duplicate the award.

## Evidence currently verified

- Simulation library: 1472 passed, 0 failed.
- Shared Zone integration: 195 passed, 0 failed.
- Gateway full run: 642 passed, 0 failed, 1 ignored, 643 total.
- Focused new-recovery test: 1 passed, 0 failed.
- Focused trade-teardown test: 1 passed, 0 failed.
- Social-economy integration: 3 passed, 0 failed, using canonical `red-potion` fixture identities.
- Web typecheck: exit 0.
- Gate18 economy/migration acceptance-bin check: exit 0.
- Exact-file Rustfmt check: exit 0.
- `git diff --check`: exit 0.
- Independent final audit: GO, P0=0, P1=0, P2=1.

## Non-blocking follow-up

- P2 defense in depth: make the PostgreSQL service return an explicit `ContextUnavailable`/`Deferred` result when no execution context exists. Current routing already guarantees teardown/`Drop` never calls this path, so it is not a Slice D release blocker.
- Full Crystal Accepted remains open. Independent audits estimate complete backend Crystal semantics at about 49% and Windows Candidate automation at about 56%; no local Slice closure may be extrapolated to whole-game 100%.

## Meaning of 100% Candidate

100% Candidate means the defined Windows vertical slice is internally complete and packageable: startup, login, character selection, entry into Bichon, movement, combat, drop/inventory, quest reward, save, and relogin recovery, with the Web client not regressed. It does not mean the entire Crystal game is already pixel-identical or feature-complete across all maps, classes, guild/siege/economy systems, visual details, and human feel acceptance. Full Crystal Accepted remains a broader follow-up gate.

## Changed paths

- `docs/CRYSTAL-1TO1-ROADMAP.md`
- `docs/BACKEND-1TO1-PROGRESS.md`
- `docs/CRYSTAL-SERVER-PARITY.md`
- `docs/generated/player-qa/item-identity/DURABLE-GROUND-DROP-SETTLEMENT-SLICE-D-REPORT.md`
