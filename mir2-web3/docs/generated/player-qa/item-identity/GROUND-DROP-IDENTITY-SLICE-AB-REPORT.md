# GroundDrop Identity Slice A/B Report

Date: 2026-08-25

Status: PASS for Slice A/B. Overall Crystal/Mir2 100% parity is not claimed.

## Delivered

- Preserves the complete recursive Crystal `UserItem` payload for monster,
  player, death, local, shared-Zone, and quest GroundDrop paths.
- Uses one staged planner for exact preflight and commit, preventing partial
  inventory mutation when capacity or identity validation fails.
- Preserves assigned item UIDs on insertion and retires an assigned source UID
  when a stack is fully absorbed; unassigned multi-stack gains update and emit
  every changed stack.
- Canonicalizes legacy display-name keys only when exactly one Crystal template
  matches. Unknown or ambiguous legacy payloads fail closed without inventory
  mutation.
- Keeps internal `WorldSnapshot`, JSON/MessagePack Zone RPC, checkpoint active
  and claimed drops, and state roots lossless while Web and spectator client
  projections omit `exactItem`.
- Binds GroundDrop pickup idempotency to the serialized exact payload digest.

## Verification

- `cargo +1.89.0 test --locked -p mir2-simulation --lib --jobs 1 -- --test-threads=1`: 1461 passed, 0 failed.
- `cargo +1.89.0 test --locked -p mir2-gateway --lib --jobs 1 -- --test-threads=1`: 606 passed, 0 failed, 1 ignored.
- Focused GroundDrop identity coverage: 15 passed, 0 failed.
- `npm --prefix apps/web run typecheck`: passed.
- Exact-file `rustfmt +1.89.0 --check`: passed.
- `git diff --check`: passed (line-ending warnings only for unrelated dirty files).
- Independent read-only review: P0 = 0, P1 = 0.

## Remaining P1 Work

- Slice C: authoritative GroundDrop generation and claim identity, stable
  canonical payload digest, owner/fencing validation, cancel restoration, and
  pet pickup through the same account-inventory transaction boundary.
- Slice D: durable economy-store abstraction and a recoverable PostgreSQL
  claim ledger committed atomically with inventory/economy/outbox effects,
  including replay and every pre/post-commit crash window.