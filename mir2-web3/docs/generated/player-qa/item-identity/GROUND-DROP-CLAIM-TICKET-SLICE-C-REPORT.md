# Ground-drop Claim Ticket Slice C Report

Status: **PASS** (2026-08-25)

## Scope closed

- Every authoritative shared ground drop receives a non-reused generation and canonical payload digest.
- A claim produces `GroundDropClaimTicket`, binding Zone, object, generation, claim id, payload digest, idempotency key, session, owner, and complete drop payload.
- Object-id-only commit/cancel remains accepted only as a legacy protocol shape and fails closed; only an exact ticket can mutate an outstanding claim.
- Player and IntelligentCreature pickup both use the account-inventory transaction boundary. A committed receipt retires the drop; a failed receipt cancels the exact ticket and restores the drop without a personal-session fallback.
- Gateway checkpoints persist exact pending tickets and reject restore unless presence, session, player state, and the restored Zone's pending claim all match.
- v1 Zone checkpoints ignore injected v2 authority fields and rebuild generation/digest/tickets deterministically after verifying the legacy root.
- Generation and claim-id exhaustion fails closed instead of saturating and reusing `u64::MAX`.

## Safety cases covered

- exact commit and cancel;
- replay, duplicate, reordering, and ABA attempts;
- ticket tampering across claim id, generation, digest, idempotency key, session, owner, and payload;
- account-service failure with drop restoration;
- reconnect generation reset and checkpoint round-trip;
- v1 migration with unsigned v2-field injection;
- pending Gateway ticket restore with orphan presence, wrong session, or mismatched Zone claim;
- player and IntelligentCreature shared pickup through the same settlement boundary.

## Verification

```text
cargo +1.89.0 test --locked -p mir2-simulation --lib --jobs 1 -- --test-threads=1
1465 passed; 0 failed

cargo +1.89.0 test --locked -p mir2-simulation --test shared_zone --jobs 1 -- --test-threads=1
193 passed; 0 failed

cargo +1.89.0 test --locked -p mir2-gateway --lib --jobs 1 -- --test-threads=1
609 passed; 0 failed; 1 ignored (external Postgres fixture)

npm --prefix apps/web run typecheck
PASS
```

Focused additions also passed: Simulation checkpoint 11/11 and Gateway checkpoint ticket-tamper restore 1/1.

## Independent review

A read-only independent P0/P1 review found no P0 and two P1 findings. Both were fixed before this report: fail-closed ID exhaustion and Gateway restore cross-validation. No known P0/P1 remains inside Slice C.

## Files in this slice

- `apps/simulation/src/runtime/zone/types.rs`
- `apps/simulation/src/runtime/zone/runtime.rs`
- `apps/simulation/src/runtime/zone/runtime/checkpoint.rs`
- `apps/simulation/src/runtime/zone/manager.rs`
- `apps/simulation/src/runtime/zone/mod.rs`
- `apps/simulation/src/runtime/mod.rs`
- `apps/simulation/src/lib.rs`
- `apps/simulation/tests/shared_zone.rs`
- `apps/gateway/src/routing.rs`
- `apps/gateway/src/economy.rs`
- `apps/gateway/src/abnormal_teardown_zone_drain_tests.rs`

## Remaining boundary

Slice D must prove durable economy settlement across process failure after durable commit but before local projection/follow-up, including idempotent replay and payload-conflict rejection. Slice C alone is not the overall 100% Candidate declaration.