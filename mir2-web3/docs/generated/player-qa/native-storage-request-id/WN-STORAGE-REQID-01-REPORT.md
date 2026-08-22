# WN-STORAGE-REQID-01 — Ordinary Storage Request Correlation

Status: **Candidate code gate passed**. This goal is non-visual and does not
claim Android personal-storage controls are rendered.

## Delivered contract

- Legacy `StoreItem` / `TakeBackItem` packet IDs and JSON commands remain
  unchanged.
- New protocol packets use client IDs `153/154` and server IDs `281/282`.
- New browser commands are distinct `storeItemV2` / `takeBackItemV2` types.
  An older Gateway rejects those unknown types before Simulation can mutate
  state; it cannot silently consume a request ID as a legacy operation.
- `requestId` is 1–64 printable ASCII bytes and is validated by protocol
  encode/decode plus the Gateway JSON boundary.
- Simulation mutates through the existing authoritative storage implementation
  exactly once and echoes the same request ID on both ACK and NACK.
- Zone RPC advertises `storageRequestIdV1`, rejects an old Host before Execute,
  and treats V2 Storage as a mutation-once operation. Response loss is an
  unknown commit and never triggers endpoint replay.
- Web, Windows and the shared Android adapter match the exact request ID,
  operation and coordinates. Legacy or delayed duplicate ACKs cannot release a
  newer V2 request with reused coordinates.
- Windows retains one correlated mutation outside the ordinary saturated
  command lane. Pre-Normal consumption and a saturated inbound receipt queue
  transition through a non-evictable DataReset instead of replaying or leaving
  a permanent pending request.
- Android exposes a bounded ECS-driven GameActivity/JNI Host transport with C
  ABI start/poll/write-result/connection-loss entry points. A failed write or
  sent-without-ACK disconnect marks the exact Storage V2 request unknown,
  never requeues it, preserves the request sequence, and rejects late results.
- Process-lifetime request sequences are monotonic and fail closed on overflow;
  session cleanup clears pending state without resetting the sequence.

## Verification

| Gate | Result |
|---|---:|
| `cargo +1.95.0 test -p mir2-protocol` | 74 passed |
| `cargo +1.95.0 test -p mir2-simulation --test storage_request_id` | 2 passed |
| `cargo +1.95.0 test -p mir2-ui-core` | 42 passed |
| Android crate tests | 56 passed |
| client-bevy with `native-ui` | 378 passed |
| Windows crate tests | 282 passed |
| runtime crate tests | 180 passed |
| Gateway library suite | 534 passed, 1 environment test ignored |
| Storage V2 targeted mapping/legacy/no-replay tests | 4 passed |
| `npm --prefix apps/web run typecheck` | passed |
| `git diff --check` | passed |

The first attempt to relink the Gateway test executable after the full suite
was blocked by a transient Windows `LNK1104` file lock. The new no-replay test
was then built and passed from the isolated
`target-storage-reqid-gateway` target directory.

## Compatibility boundary

V1 remains available for old clients but does not gain exact ACK correlation.
Only V2 requests receive the no-old-ACK guarantee. The Android work in this
goal supplies the shared action/state/effect and real Gateway adapter contract;
it does not add a new native personal-storage renderer or fabricate controls.
