# Gate 5.2 — Remote Zone Host

Gate 5.2 moves authoritative `WorldRuntime` execution behind a separate Zone Host process. The
Gateway keeps Crystal TCP/WebSocket termination, authentication context, routing and event
publication; it no longer needs to execute a player's game commands in its own process when
`MIR2_ZONE_HOST_ADDR` is configured.

## Runtime path

```text
Crystal client
    -> Gateway (TCP/WebSocket, session auth, zone routing)
    -> length-prefixed Zone RPC over TCP
    -> Zone Host session runtime
    -> shared in-host Zone resources
    -> Crystal server packet frames back to Gateway
```

Each Gateway session receives a unique RPC session id. The Zone Host creates one hosted runtime
per `(rpc_session_id, zone_id)` and uses one `SharedInProcessZoneRuntimeFactory`, so player-local
state stays isolated while map/zone resources are shared by sessions in the same host.

## Protocol and safety boundaries

- Protocol version: `1` for this historical gate; Gate 5.3 upgrades the running protocol to `2`.
- Framing: four-byte big-endian payload length followed by JSON.
- Default maximum frame: 16 MiB.
- Default maximum concurrent connections: 64.
- Default maximum hosted sessions: 4096.
- Crystal `ClientPacket` and `ServerPacket` values cross the boundary using the existing binary
  codec, rather than a second hand-maintained packet schema.
- Every execute request includes `zone_id`, owner id and fencing token. The Zone Host validates the
  lease again at the authoritative execution boundary and returns `stale_lease` for an old token.
- A non-loopback Zone Host bind requires `MIR2_ZONE_HOST_TOKEN`. Production deployment should also
  place the connection on a private network or authenticated service mesh; protocol v1 is not an
  Internet-facing public API.
- The client opens a new bounded RPC connection per operation. A failed host is observable, and
  later calls reconnect without replacing the Gateway session object.

The bounds can be overridden with `MIR2_ZONE_RPC_MAX_FRAME_BYTES`,
`MIR2_ZONE_HOST_MAX_CONNECTIONS`, `MIR2_ZONE_HOST_MAX_SESSIONS` and
`MIR2_ZONE_RPC_TIMEOUT_MS`.

## Run locally

Start the Zone Host:

```bash
MIR2_ZONE_HOST_ADDR=127.0.0.1:7020 \
MIR2_ZONE_HOST_CRYSTAL_WORLD=1 \
cargo +1.89.0 run -p mir2-gateway --bin zone_host
```

Then start the existing Gateway with the same address:

```bash
MIR2_ZONE_HOST_ADDR=127.0.0.1:7020 \
cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway
```

For multi-process production fencing, configure the same
`MIR2_GATEWAY_ZONE_LEASE_DATABASE_URL` for Gateway and Zone Host. Without it, both processes use
the deterministic single-owner development lease (`in-process:<zone>`, token `1`).

## Acceptance

```bash
cargo +1.89.0 test -p mir2-gateway --test zone_rpc -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway --lib session::tests -- --test-threads=1
cargo +1.89.0 check -p mir2-gateway --all-targets
cargo +1.89.0 test -p mir2-simulation --test shared_zone
cargo +1.89.0 test -p mir2-simulation --test zone_replay
```

The Zone RPC suite verifies:

1. Gateway environment configuration selects the remote owner client.
2. Crystal packets and full world snapshots survive the process boundary.
3. Two RPC sessions do not share player-local identity.
4. A stale fencing token is rejected by the Zone Host.
5. One client instance recovers after the host becomes available.
6. Oversized frames are rejected before network I/O.
7. The `zone_host` binary has a different PID and executes the authoritative login/start-game
   commands in that process.

## Gate boundary

This gate deliberately kept one configured Zone Host address and request/response delivery.
Gate 5.3 adds reliable live outbounds, checkpoint replication and lease-aware endpoint rerouting.
Discovery and scheduler-driven multi-host placement remain later gates.
