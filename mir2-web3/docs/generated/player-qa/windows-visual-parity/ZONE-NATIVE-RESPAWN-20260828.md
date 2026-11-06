# Shared-Zone native monster respawn report

Date: 2026-08-28

## Claim state

```text
implementation revision: 7f991ec34fbde6ac07a5799b35d352f2785c1aa9
branch: codex/windows-visual-parity
zoneNativeRespawnAutomatedCheckpoint: complete
gatewayQ1ToQ4ClientPacketReloadCheckpoint: complete
completeZonePersistence: false
crossGatewayOwnershipComplete: false
semanticDenominatorComplete: false
globalParityPercent: null
accepted: false
```

This report closes only the bounded monster-incarnation scheduler and its
ordinary Gateway vertical path. It does not claim complete multiplayer, Zone,
quest, economy, skill, monster-AI or whole-game parity.

## Implemented authority

- `ZoneRuntime` owns the only live respawn schedule. Personal
  `SimulationSession` no longer advances a second monster respawn clock.
- `ZoneMonsterRespawnPolicy` retains the Crystal floor/base/step/outcome,
  subtract, rule and static-slot data needed to reproduce the source `D10/R30`
  distribution rather than storing an opaque fixed delay.
- A death schedules one absolute wall-clock due time. `ZoneCommand::Tick` is
  the only authority that creates the next living incarnation.
- Deer and the complete audited harvestable-AI set (`1,2,4,5,7,9,28,35,153`)
  remain corpses until the authoritative harvest completes; harvest then arms
  the same Zone scheduler.
- World checkpoint schema v4 persists the scheduler. v1-v3 restore paths are
  retained, while forward-field injection into v3 is rejected.
- A late join or stale private snapshot cannot revive a scheduled corpse.
  Positive-HP resync is rejected when either the incoming snapshot or retained
  Zone object has a respawn policy.
- Two sessions observe one `ObjectRevived` lifecycle; the same object ID is a
  new attackable incarnation only after the due time.

## NPC interaction race closed

The full Gateway test exposed a separate production race: `CallNpc` could open
at an adjacent tile, then a previously accepted Walk intent could land during
the tail Zone tick and move the player two tiles away before `FinishQuest`.
The fix does not relax the Crystal adjacency gate. A trusted, server-only
`CancelPendingMovement` command now clears queued movement and emits the
authoritative `UserLocation` before NPC/dialog/AcceptQuest/FinishQuest work.

## Automated evidence

| Gate | Result |
|---|---|
| Crystal respawn formula focused tests | PASS, 2/2 |
| Harvest contract focused test | PASS, 1/1 |
| Legacy v3 compatibility focused tests | PASS, 2/2 |
| Late-join/stale-resync focused test | PASS |
| Interaction-boundary queued-movement test | PASS |
| Full `shared_zone` suite | PASS, 203/203 |
| Gateway authoritative quest-transform regression | PASS |
| Ordinary Gateway Q1-to-Q4 client packets + logout/reload | PASS, 1/1 in 748.77 s; Q4 used 7 kills and 5 real DeerMeat |
| `cargo check -p mir2-simulation -p mir2-gateway` | PASS |
| Independent read-only review | PASS, P0=0/P1=0; one layered-cadence P2 remains |

Only pre-existing compiler warnings were emitted.

## Explicitly open gates

The true global-cadence-to-pending-revive-to-two-private-ECS lifecycle still
has layered rather than one monolithic test. Complete Zone persistence,
cross-Gateway ownership/failover, the full task/economy/skill/monster-AI
denominator, same-EXE UI/live WSS, real DPI, native 30-minute soak, human
visual/feel acceptance, legal asset review and formal publisher signing remain
open. Therefore `globalParityPercent=null` and `accepted=false` are mandatory.
