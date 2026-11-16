# P3a: one-manager cross-map ownership and original ground protection

Implementation baseline: `57729ba8c1473ca920a8ea8b3d6e11105d9d4d93`.
The eight existing production inputs are byte-identical to the reviewed
`51e8e49132bfb6fbce7582a9d1c21d8f685d09c9` death18 baseline. This round
does not publish a client, replace a Gateway, or establish complete P3 parity.

The delivery is limited to ordinary solo native-monster ownership inside one
`SharedInProcessZoneState` / `ZoneManager`. Prepared local protocol tests use
normal credential Login, StartGame, Poisoning, Walk, PickUp and LogOut packets.
Prepared level, HP, inventory, saved position and deterministic source drop
rolls are explicitly fixtures; they are not a leveled player journey, live
network test, human acceptance, or a natural Boss drop result.

## Original source contract

Source bytes and line excerpts are frozen under
`C:/mir2-playtest-releases/20261004-native-r17/cross-map-drop-plan-01/baseline-51e8-review/`.
`source-hashes.json` and `original-excerpts.json` bind the following rules.

| Crystal source | Rule retained | SHA-256 |
| --- | --- | --- |
| `Server/MirObjects/MapObject.cs` 204–225, 339–375, 780–800 | Global spawned Node is separate from map membership; invalid Node clears owner references; teleport does not despawn. EXPOwner expires strictly after its five-second deadline. | `f465ee5fe84bc9fe6ab3ef9dd0c138c8c6b1dee83d1ea5b007cc5cb3afe53f7f` |
| `Server/MirObjects/MonsterObject.cs` 1152–1183, 1493–1509, 2627–2633 | Owned monster item/gold has an absolute one-minute OwnerTime; periodic green poison renews a valid caster reference; a legal positive attacker may replace a dead owner. | `2cc4e673c589ad57452b4682c3bcb155b4b6b3027b1d3ee52a18a5ae1b7df98e` |
| `Server/MirObjects/ItemObject.cs` 53–141 | Strict `Time > OwnerTime` expiry; manual/player-death constructors do not automatically receive monster ownership. | `da52f31cd9d7ae51e05bae2f14c435c06048a6647e273d74b6be1312e18372e1` |
| `Server/MirObjects/PlayerObject.cs` 400–406, 643–650, 7519–7535 | StopGame despawns; death retains Node. Dead players cannot pick up; ordinary ownership and pickup group rules are distinct. | `48607c437fe56859c0fbc2a84195693395806e532e95bf5861aed9423a19ff11` |
| `Server/MirNetwork/MirConnection.cs` 790, 1122, 1130–1139 | Disconnect/accepted logout remove the old spawned player; a fresh login constructs a new player object. | `fc7b59487d21171e48cb8ce0bfa29141f8bd52160298cc0069aee1cf4812d292` |
| `Server/MirObjects/HumanObject.cs` 845–865 | LevelUp refills HP/MP without changing Dead; death18 behavior must remain. | `c4a4e060a6c88c41d869b4b3b6a78fe5a935e0456b8ee7c230db3d0596b299d3` |
| `Server/Settings.cs` 16 | One second is 1000 ms; the minute is elapsed time rather than scheduling tick count. | `c9fbfa492e33b1a4e07a79a86a002c7918418531b7bdf4ba6671916a38f2faec` |

This does not substitute contribution points or the existing Candidate equal
group share for Crystal's EXPOwner/LastHitter/group algorithms.

## Implementation

The manager owns a random incarnation namespace and a checked monotonic online
spawn epoch, bound to session, account, character and object. A normal map
transfer within this manager retains that identity. True leave, fresh same-map
admission and cold restore revoke it across every source map. Death only
changes typed life state. Epoch exhaustion fails closed through actual admission.

Each Zone reads an immutable manager presence snapshot. Native EXPOwner and
periodic poison therefore resolve the original online object after it changes
map. Five-second expiry remains strict `>` and only source-legal damage or
poison refresh can renew it. Despawn clears a poison caster reference while
the existing green poison value and original absolute effect clocks continue.
A replacement login cannot inherit that old effect's reward.

Shared native kills emit the server-only `OwnedMonsterKillAward`. Delivery must
match both the current online identity and an issuance retained by the named
source Zone for the original gameplay payload. A valid online identity alone
cannot choose another source, experience amount or loot. The old unbound
`MonsterKillAward` branch is rejected by the shared Gateway; queueing and the
missing-proof path cannot mint authority from current membership. Confirmation
consumes the source issuance. Known/unknown save failures retain the same source
receipt and payload under the existing durability fence.

The opaque identity JSON is an internal adapter across the simulation/Gateway
boundary. It is never a ClientPacket capability and is never serialized into
WorldSnapshot, AOI or server packet frames. It grants no authority without the
manager's current book and the source's retained issuance.

Native monster drops retain their source key, birth and absolute
`birth + 60_000` protection deadline. At exactly 60 seconds they are still
protected; strictly later they open. A dead online owner retains protection;
true despawn releases it. The old 300 ms countdown projection changes from 60
to 200 ticks, while authoritative protection uses elapsed milliseconds.
Repeated sync, irregular ticks, rejected pickup and detached claim rollback do
not restart protection or the independent object TTL. Custody retains source,
object, generation, payload digest, protection and TTL; an expired held object
cannot resurrect on cancellation. Manual/player-death drops do not gain this
native ownership policy.

Zone checkpoint v6 and manager checkpoint v3 bind the new private metadata.
Original v1–v5 canonical payloads validate before uncommitted forward metadata
is ignored. Cold decode always removes live owners and pending award authority,
even when the original state commitment is valid. The complete authenticated
player/loot payload, UID, generation, digest, tombstone and absolute TTL survive.
Capturing a checkpoint does not revoke a still-retained in-memory runtime;
deserializing that image is cold recovery and cannot restore its live Node.
These state roots are integrity commitments, not a client authority signature.

## Focused verification

All commands use `cargo +1.95.0 test --locked --jobs 2`, only
`C:/mir2-build/gateway-tests-r50`, and isolated local fixtures. Final focused
selection contains **89 distinct passing tests**, with zero current failures:

| Selection | Passing |
| --- | ---: |
| `mir2-simulation --test shared_cross_map_drop_lifecycle` | 19 |
| `mir2-gateway --lib cross_map_drop_lifecycle_tests` | 9 |
| `mir2-simulation --test shared_monster_ownership` | 21 |
| `mir2-simulation --lib runtime::zone::runtime::checkpoint::tests` | 17 |
| `mir2-simulation --lib runtime::zone::runtime::experience_ownership::tests` | 6 |
| `mir2-simulation --lib runtime::zone::online_identity::tests` | 1 |
| Four approved Gateway award fixture selectors | 4 |
| Frozen `mir2-gateway --lib dead_experience_chain_tests` | 12 |

The ordinary packet variant uses the imported D001 `(30,328)` → D002
`(34,323)` walk transition and its normal return, with the exact respawn
manifest SHA-256
`5303f8093be7f15ddd9f860e6db1787ac96606508db3086bab38bdc4c6353dd3`.
The caster crosses before a real poison tick kills the prepared Skeleton.
The source drop remains in D001, the caster receives one experience receipt in
D002, and ordinary source pickup/LogOut/Login retain gold and exact item payload.
InventoryItem gets one real UID; repeated pickup does not duplicate it. A source
rate roll with deterministic seed 6 gives the single-item payload fixture;
earlier empty/multiple-item rolls and the original mistaken-repeat failure are
retained. This cannot count as natural Boss, equipment or skill-book acceptance.

Negative tests include all online identity substitutions, another manager
namespace, changed source/payload, missing proof, consumed issuance, fresh-login
replay, metadata tampering, valid cold recovery, strict clock boundaries, full
bag cancellation, original expired TTL, and known/unknown reward publication.
Proof absence is checked in actual server packet encodings and WorldSnapshot.
Unknown publication uses the isolated file-store fault after rename before
directory sync; it is not a SQL/network/recovery-journal test.

The original six-test RED precedes production edits. Raw compiler failures,
process abort, wrong prepared HP, scatter-position and multiple-item fixture
failures are retained, not rewritten as successful runs. Adjacent old tests
failed under the new cold/Node policy before their approved adaptation.

## Approved test fixture adaptations

These are fixture maintenance, not new RED or network acceptance:

- `shared_in_process_runtime_emits_gain_experience_after_kill_award_commit`,
  `shared_in_process_runtime_uses_account_inventory_service_boundary`, and
  `gateway_kill_award_level_up_updates_zone_and_personal_vitals` now obtain the
  award through real Manager Join, a prepared HP-1 monster and valid positive
  damage. Original experience, inventory boundary, life and vitals assertions
  remain. They cannot directly invent a shared native award.
- `shared_zone_state_dispatches_current_and_pending_monster_kill_awards` is
  renamed `shared_zone_state_rejects_unbound_legacy_monster_kill_awards` and
  verifies rejection of both current and pending forged legacy delivery.
- `authenticated_checkpoint_retains_owner_without_extending_deadline` is
  renamed `authenticated_cold_checkpoint_preserves_state_but_revokes_live_owner`.
  It verifies the original root, player/monster payload and absolute clocks,
  payload/epoch tamper rejection, empty cold authority, fresh admission and B's
  own positive killing damage at both former five-second boundaries.
- `fresh_same_map_join_revokes_old_claim_and_old_poison_identity` preserves
  green poison instead of erasing it. It verifies original next/expiry clocks,
  new epoch, B's positive damage and an actual old-poison lethal tick that awards
  neither former nor replacement epoch despite reusing all durable IDs.
- `complete_zone_checkpoint_restores_authoritative_and_derived_state` validates
  the original root and rejects changed HP before checking cleared online
  authority, complete player/hazard payload, position, vitals and ECS mirror.

Adaptation-before full bytes/SHA and exact hunks are separate receipts. The
death18 real Magic/poison-death test source is byte-unchanged.

## Delivery and remaining boundaries

The authorized delivery is eight production files, four new Rust files, this
scoped document and one additional existing test file: **14 files**.
Full patch, before/after hashes, case-to-test mapping, failure history and raw
logs are frozen under
`C:/mir2-playtest-releases/20261004-native-r18/cross-map-drop-implementation-01/`.
Global roadmap/progress/parity documents are reserved for parent integration.

Still open: cross-Gateway ZoneId/process handoff, original level-weighted
object-reference party experience/pickup, Boss and PK LastHitter parity, full
transport resume-token lifecycle, both database and recovery-journal failure,
live network rollout, natural Boss/equipment/book drops, leveled journeys and
human frontend acceptance. The 34 planned cases are individually classified in
the receipt; bounded variants or partial coverage are not upgraded to complete
planned-scenario acceptance. No current player saves, services, generated
resources, P2/v27 inputs or published R18 artifacts were changed.
