# Gate 5.4 — Map-to-Zone Topology and Independent Ticks

Gate 5.4 replaces the live Gateway's single implicit Zone with a versioned topology. A topology
can dedicate a busy map to one Zone, group quiet maps into a shared Zone, and give every Zone an
independent tick cadence. TCP, WebSocket, and standalone Zone Host boot paths all load the same
configuration.

## Configuration

Set `MIR2_ZONE_TOPOLOGY_FILE` to a JSON document such as
[`config/zone-topology.example.json`](../../config/zone-topology.example.json), or provide the
same document through `MIR2_ZONE_TOPOLOGY_JSON`.

```json
{
  "version": 1,
  "mode": "per_map",
  "defaultZoneId": "lobby",
  "defaultTickMs": 100,
  "zones": {
    "hot-bichon": { "maps": ["0"], "tickMs": 50 },
    "cold-leveling": { "maps": ["1", "2"], "tickMs": 250 }
  }
}
```

Rules:

- `single` mode routes every session to `defaultZoneId` and rejects map groups.
- `per_map` mode uses explicit groups first. An unlisted map receives a dedicated `map:<name>`
  Zone so a new or event map cannot silently overload a cold-map group.
- One map cannot appear in two groups. Zone and map identifiers must be non-empty and tick
  cadence must be between 10 and 5000 ms.
- `MIR2_ZONE_ROUTING_MODE=single|per_map` remains the zero-document shortcut.

## Runtime ownership

Each instantiated Zone owns a separate shared world state, bounded movement ingress, owner lease,
simulation thread, and tick counter. Per-Zone tick threads coalesce late ticks instead of replaying
a burst. A checkpoint install creates a fresh factory with the same tick policy, so promotion does
not lose topology cadence.

For a 700-map Mir2 deployment, use dedicated Zones for crowded cities, wars, bosses, and dungeon
instances; group genuinely cold leveling/interior maps. Do not create 700 permanently hot OS
threads up front: Zones instantiate lazily when the first routed session arrives.

## Boot

```bash
MIR2_ZONE_TOPOLOGY_FILE=config/zone-topology.example.json \
cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway

MIR2_ZONE_TOPOLOGY_FILE=config/zone-topology.example.json \
cargo +1.89.0 run -p mir2-gateway --bin zone_host
```

## Acceptance

```bash
cargo +1.89.0 test -p mir2-gateway topology::tests -- --test-threads=1
cargo +1.89.0 test -p mir2-gateway --test zone_rpc -- --test-threads=1
cargo +1.89.0 check -p mir2-gateway --all-targets
```

The topology tests prove explicit grouping, isolated unknown maps, duplicate rejection, independent
hot/cold tick rates, and separate owner leases. Zone RPC acceptance proves a standalone host keeps
the configured runtime factory across checkpoint install.

## Gate 5.5

Atomic live rebind after `StartGame` or `TransferMap`, rollback, fenced remote close, and global
messages are implemented in [`GATE5-ATOMIC-ZONE-HANDOFF.md`](GATE5-ATOMIC-ZONE-HANDOFF.md).
