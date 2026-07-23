# Gate 5.1: Deterministic Zone Replay

Gate 5.1 proves that a Mir2 authoritative zone can consume a canonical input
stream in independent processes and produce the same state commitment.

## Acceptance contract

- Every input is namespaced by `zone_id` and `epoch`.
- Sequences must be contiguous and start at zero.
- Logical time may stay equal within one tick but may never go backwards.
- The state root covers all authoritative ZoneRuntime state, including the
  selected collision map. Occupancy, AOI grids, and the ECS mirror are derived
  indexes and are not hashed twice.
- Every accepted input advances a rolling checkpoint commitment containing the
  previous commitment, canonical input bytes, and resulting state root.
- A checkpoint restore replays the accepted event log and rejects any state or
  commitment mismatch.

## Run the automated acceptance

```sh
cargo +1.89.0 test -p mir2-simulation --test zone_replay
```

The test suite verifies:

1. Missing, duplicate, and time-regressing inputs are rejected.
2. One hundred independent in-process replays produce identical commitments.
3. Two independent OS processes each execute 10,000 logical ticks and produce
   byte-identical reports.
4. A checkpoint taken halfway through 10,000 ticks restores and finishes with
   the same report as an uninterrupted run.
5. A tampered checkpoint is rejected.

## Run a human-visible replay

```sh
cargo +1.89.0 run -p mir2-simulation --bin zone_replay -- demo 10000
```

Optionally persist the replay checkpoint:

```sh
cargo +1.89.0 run -p mir2-simulation --bin zone_replay -- demo 10000 /tmp/mir2-zone-checkpoint.json
```

The JSON report includes the number of applied inputs and ticks, emitted
outbounds, final state root, and rolling checkpoint hash.

## POC boundary

This gate deliberately uses an event-sourced checkpoint: restart reconstructs
the zone by replaying its canonical input log. That is strong enough to prove
determinism and tamper detection, but it is not the compact snapshot needed for
production failover. Gate 5.3 will add compact state snapshots plus Executor to
Standby replication after the remote Zone Host boundary exists.
