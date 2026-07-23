# Gate 11.1 — Real Mir2 Workload on Remote Zone Hosts

Gate 11.1 replaces synthetic counter-only acceptance with one real Crystal-world state chain
executed through remote TCP Zone Hosts.

## Acceptance chain

The harness starts an active and standby `ZoneHostServer`, then proves:

1. a real `Login` and `StartGame` enter Crystal map `0` through its per-map Zone;
2. a real client `Attack` damages an authoritative hostile monster;
3. real client `DropItem` and `PickUp` commands mutate authoritative inventory/drop state;
4. a cross-map transfer commits the atomic `map:0 -> map:1` Zone handoff;
5. the active host exports a v3 journal checkpoint and the standby installs it;
6. the active listener stops, the owner authority issues a higher fencing token, and the existing
   Gateway session continues on the standby with its durable player state intact.

Run it with:

```bash
cargo +1.89.0 test -p mir2-gateway --test gate11_workload -- --nocapture
cargo +1.89.0 run -p mir2-gateway --bin gate11_acceptance
```

A cold debug build materializes the full Crystal map and respawn manifest more than once, so this
acceptance can take several minutes. Operator progress is printed at each boundary and the binary
finishes with machine-readable JSON.

## Trust boundary

The setup uses the trusted internal transfer command only to place the deterministic test actor
beside the selected monster and on the actual spread drop cell. Login, map join, attack, drop, and
pickup are genuine `ClientPacket` paths; the cross-map transfer uses the same atomic handoff path as
runtime transfers.

Checkpoint v3 deliberately commits the durable player/session projection: identity, map,
position/direction, vitals, economy, inventory/equipment, quests, skills, buffs, and other
persisted character systems. It does **not** claim a bit-for-bit image of autonomous shared map
state. Monster AI/timers and public ground drops continue to advance on Zone cadence and require a
separate map-state checkpoint before seamless whole-map failover can be accepted. That is the
Gate 11.2 boundary.
