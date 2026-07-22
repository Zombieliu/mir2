# Gate 6 — Multi-Host Zone Scheduler and Drain

Gate 6 turns the Gate 5 endpoint list into an explicit placement control plane. A Zone is assigned
one primary and a configurable number of replicas under a generation-fenced placement lease.

## Control-plane state machine

`ZoneHostControlPlane` owns deterministic state that can be replicated by the Gate 8 consensus log:

- host registration: stable host id, RPC endpoint, failure domain, session/Zone capacity and weight;
- heartbeats: observed sessions/connections and an explicit timestamp;
- placement: a stable primary plus replicas, placement generation and expiry;
- capacity scheduling: least normalized session/Zone load first, deterministic rendezvous hash as
  the tie-breaker, and failure-domain spreading before same-domain fallback;
- drain: stop scheduling onto a host, compute replacement placements, increment generations, and
  reject removal until every placement has left;
- failure recovery: an expired heartbeat invalidates its placements and `rebalance` moves them to
  healthy hosts while fencing the previous generation.

All clock values are supplied by the caller. Given the same ordered registrations, heartbeats and
commands, every replica produces the same placement decisions.

## Data-plane enforcement

Zone RPC protocol version 4 expands health with host identity, session/Zone capacity, current load,
active connections and drain state. `ZoneHostServer::set_draining(true)` preserves existing
sessions but rejects new sessions. A multi-endpoint client treats `host_draining` and `capacity` as
retryable placement failures and tries the next replica.

`TcpZoneOwnerRpcTransport::with_placement` binds a session to the primary-first endpoint ordering
in a `ZonePlacementLease`. Owner leases continue to fence gameplay commands independently; the
placement generation fences scheduler changes.

## Operational sequence

1. Probe each Zone Host health and register it with its advertised capacity and deployment failure
   domain.
2. Feed health probes back as heartbeats before the configured TTL.
3. Call `place_zone`; retain and renew its generation while the Zone is active.
4. Before maintenance, set the data-plane host to draining and call `begin_drain`.
5. Replicate/install the latest Gate 5 checkpoint on each replacement, switch traffic to the next
   placement generation, then call `finish_drain`.

The scheduler returns the rebalance moves but deliberately does not copy checkpoints itself. That
keeps placement policy separate from the Gate 5 replication transport and gives Gate 8 one clear
ordered command boundary.

## Acceptance

```bash
cargo +1.89.0 test -p mir2-gateway --lib control_plane::tests -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway --test zone_rpc -- --test-threads=1
cargo +1.89.0 check -p mir2-gateway --all-targets
```

The integration test starts three real TCP Zone Hosts, registers their live health, places a Zone
across failure domains, executes on the scheduled primary, drains it, proves a new session falls
through to the replica, and verifies the scheduler emits a new fenced generation.
