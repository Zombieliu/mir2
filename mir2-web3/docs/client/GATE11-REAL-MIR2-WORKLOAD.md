# Gate 11 — Real Mir2 Zone Recovery Acceptance

Gate 11 replaces counter-only failover evidence with real Mir2 protocol and Crystal-world state.
The four sub-gates form one release contract: the build is accepted only when all four pass.

## 11.1 Real Crystal workload

The harness starts active and standby TCP `ZoneHostServer` processes and proves:

1. `Login` and `StartGame` enter Crystal map `0` through `map:0`;
2. a client `Attack` damages an authoritative hostile monster;
3. `DropItem` and `PickUp` mutate authoritative inventory and ground-drop state;
4. the same session completes an atomic `map:0 -> map:1` handoff;
5. the active host stops, the owner authority emits a higher fencing token, and the existing
   session continues on the standby.

Run the real-world slice alone:

```bash
cargo +1.89.0 run -p mir2-gateway --bin gate11_acceptance
```

## 11.2 Complete Zone checkpoint

Host checkpoint v4 contains the command journal, an exact serialized durable active-character
record for every in-world session, and an exact shared-Zone image. The image includes players and
combat vitals, native monsters and AI timers, pending attacks/projectiles/heals/summons/ground
spells, objects, public drops and claims, doors, hazards, map snapshot lifecycle, pending
packets/outcomes, trades, rentals, and NPC saved state. The character record preserves exact
inventory/equipment data (including durability), economy, quest/skill, rental, NPC, and Stage 5
state even when command replay timing differs.

Restore validates the outer checksum and per-session durable commitments, reconstructs occupancy,
AOI grids, collision-door state, and the ECS mirror, then validates each Zone's canonical state
root. Live outbound senders are intentionally not serialized; clients register a new live stream
after takeover.

Player transform and live combat vitals are Zone-authoritative in v4. The presentation snapshot
commitment normalizes tick/light fields and does not duplicate Zone-owned transform or vitals;
the Zone canonical root commits those values, while the separate exact character record commits
the private persisted projection. This prevents an autonomous tick between the two captures from
creating a false mismatch while still failing closed on inventory, equipment durability, economy,
quest, skill, identity, map, or player-vital loss.

## 11.3 Multi-session repeated-failure acceptance

The scale harness creates four real Mir2 protocol sessions across maps `0` and `1`, then performs
two complete checkpoint/install/promote cycles across three hosts. Every generation verifies:

- all four identities and durable state projections survive;
- all four sessions accept a command under the new owner lease;
- both old per-Zone leases are rejected;
- fencing tokens increase from generation 1 to 2 and then 3;
- the second checkpoint includes commands committed after the first takeover.

Run it alone:

```bash
cargo +1.89.0 run -p mir2-gateway --bin gate11_scale_acceptance
```

## 11.4 Operations manifest

The full harness runs 11.1, 11.2, and 11.3, checks the v4/v5 format contract and 16 MiB frame
ceiling, and emits one JSON evidence manifest. A non-accepted sub-gate exits non-zero and no
accepted manifest is written.

```bash
cargo +1.89.0 run -p mir2-gateway --bin gate11_full_acceptance -- \
  --output target/gate11-acceptance.json
```

The file is written atomically and records:

- schema, Zone RPC protocol, and checkpoint-format versions;
- checkpoint checksum, total/frame and Zone-state byte counts;
- export, install, and post-failure RTO measurements;
- monster HP and retained-drop fingerprints;
- session/Zone/map counts, both generation checksums, install times, and fencing tokens;
- every boolean used by the fail-closed acceptance decision.

### Operator sequence

1. Export and durably replicate the v4 checkpoint before promoting a replica.
2. Install it and require checksum, Zone count, canonical root, and session commitments to pass.
3. Finalize the higher owner token for every affected Zone.
4. Route clients to the new host and require a successful command/heartbeat.
5. Probe the previous owner with its old token; promotion is incomplete if it is not fenced.
6. Preserve the accepted JSON beside the binary revision and deployment record.

Do not promote when a checkpoint exceeds `DEFAULT_ZONE_RPC_MAX_FRAME_BYTES`, an install fails, a
session commitment differs, or any prior owner still accepts writes.

## Trust and scope

The deterministic actor placement uses the trusted internal transfer command; login, map join,
attack, death/TownRevive, drop, pickup, keepalive, checkpoint transport, and fencing use their
real runtime paths.

This gate accepts application-level Zone recovery and repeated fencing on real TCP listeners. It
does not claim that localhost timings are a multi-AZ latency benchmark, or replace production
Postgres/Redis backups, authenticated private networking, scheduler heartbeats, and an external
durable checkpoint store.
