# Gate 5.3 — Reliable Zone Delivery and Failover

Gate 5.3 turns the Gate 5.2 request/response boundary into a resumable remote Zone path and adds
host-wide active-to-standby checkpoint replication.

## Reliable live outbounds

Remote player movement and other real-time shared-zone packets are delivered through a bounded
host outbox. Every packet receives a monotonic sequence. The Gateway polls from its last
acknowledged sequence and only advances the acknowledgement after the packet has entered the
local socket task's bounded channel.

- Unacknowledged packets are redelivered after a transport reconnect.
- Each outbox has a stream id. A replaced/restarted host resets the cursor instead of looping on
  an invalid acknowledgement from the old process.
- The outbox is bounded by `MIR2_ZONE_RPC_MAX_OUTBOUND_MESSAGES` (default 1024). Falling behind the
  retained window produces `outbound_gap` and requires snapshot resynchronization.
- `MIR2_ZONE_RPC_OUTBOUND_POLL_LIMIT` bounds each batch (default 128).
- Re-registering a WebSocket/TCP live sender uses a generation fence, so a superseded polling
  worker cannot acknowledge a packet that the new socket registration has not received.

## Host checkpoint replication

The Zone Host records one globally ordered journal across all hosted sessions. Export captures:

1. the ordered commands, their session/zone, execution mode and owner fencing token;
2. the final active identity and deterministic `WorldSnapshot` digest for every session;
3. a SHA-256 commitment over the complete checkpoint.

Install verifies the commitment, replays into a new shared runtime factory, verifies every final
session commitment, and only then atomically swaps the standby's runtime/session/journal set.
This preserves cross-session command order rather than replaying each player independently.

`zone_replicator` performs continuous or one-shot replication:

```bash
MIR2_ZONE_ACTIVE_ADDR=127.0.0.1:7020 \
MIR2_ZONE_STANDBY_ADDR=127.0.0.1:7021 \
cargo +1.89.0 run -p mir2-gateway --bin zone_replicator
```

Use `--once` for an acceptance copy. `MIR2_ZONE_REPLICA_INTERVAL_MS` defaults to 250 ms.

## Lease-aware rerouting

Configure ordered endpoints on the Gateway:

```bash
MIR2_ZONE_HOST_ADDRS=127.0.0.1:7020,127.0.0.1:7021
```

The RPC transport keeps the last healthy endpoint and reroutes on connection/read/write failure.
Application faults are not retried. Promotion still requires a newer owner fencing token; the old
host is rejected at the authoritative execution boundary once the lease changes.

## Protocol

Gate 5.3 uses Zone RPC protocol version 2. It retains the bounded length-prefixed JSON envelope and
Crystal binary packet codec from Gate 5.2, adding live-outbound cursor operations and checkpoint
export/install operations.

## Acceptance

```bash
cargo +1.89.0 test -p mir2-gateway --test zone_rpc -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway --lib session::tests -- --test-threads=1
cargo +1.89.0 test -p mir2-simulation --test zone_replay
```

The suites prove unacknowledged redelivery, registered Gateway live delivery, two-session
checkpoint replay, corrupted/stale checkpoint rejection, independent-process replication,
multi-endpoint rerouting, standby promotion under a newer fence, and rejection of the stale active.

## Deliberate next boundary

The checkpoint is event-sourced and therefore grows with the command journal. Gate 10 compaction
will combine a versioned compact state snapshot with a short journal tail. Per-zone placement and
independent tick ownership begin in Gate 5.4.
