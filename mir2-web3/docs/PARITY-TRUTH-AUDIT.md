# Crystal / mir2-web3 Parity Truth Audit

**Last updated:** 2026-08-24
**Status:** scope-corrected; no global completion percentage is published

## Purpose and authority

This document is a traceable status audit, not a completion claim. It separates
direct evidence from implementation-only evidence, historical release notes,
and unverified or externally blocked work.

The normative completion rule is
[docs/CRYSTAL-SEMANTIC-PARITY-LEDGER.md](./CRYSTAL-SEMANTIC-PARITY-LEDGER.md).
That ledger defines the denominator as every player-observable capability
discovered in the pinned Crystal revision and requires inventoryComplete=true
before a global percentage can exist. Crystal source and original-client
runtime traces are the semantic references; Web behavior is a regression
surface, not a substitute for Crystal parity evidence.

No new implementation or acceptance evidence is created by this audit. The
purpose of this revision is to retire unsupported global wording and preserve
the boundaries of the evidence that already exists.

## Current truth snapshot

| Claim | Current value | Evidence boundary |
|---|---|---|
| Ledger inventory | inventoryComplete=false | The complete Crystal capability inventory and hashed source-inventory report required by the ledger are not complete. |
| Global completion percentage | **Undefined** | There is no complete denominator. Do not publish 100%, roughly 90%, or any other global estimate. |
| Bichon functional slice | Directly evidenced, scoped | Login/character/start, Bichon entry, movement/collision/AOI, the tested quest/combat/drop/pickup/reward path, and save/relogin have direct test/source evidence. This is a vertical slice, not all Crystal semantics. |
| Shared Zone functional slice | Directly evidenced, scoped | Zone movement, authoritative transform/save, AOI, presence, chat, monster defeat and drop ownership have direct tests. This does not prove production deployment, cross-process recovery, or every zone rule. |
| Windows native functional slice | Directly evidenced, scoped | Native shell/protocol/gateway/UI wiring and the tested player flow exist. This is not formal visual, real-hardware, signed-package, or human acceptance. |
| Automated global P0 | No direct evidence of a global zero | Scoped reports may show no P0 in their own matrix; that cannot be promoted to “global P0=0” while the ledger denominator is incomplete. |
| Scoped local P0/P1 statements | Valid only within their named scope and snapshot | For example, the GameShop/UI reports can state their local P0/P1 result. They do not establish project-wide P0/P1 status. |
| Formal Windows Candidate | Not established | Package signing/attestation, real-device gates, visual parity, stability, and human acceptance remain open. |

## Directly evidenced, but limited, capability slices

These are the strongest current facts. “Verified” here means directly evidenced
for the named slice; it does not mean every related ledger leaf is VERIFIED.

### Bichon player loop

Crystal provides the reference handlers and persistence path in
`Crystal/Server/MirNetwork/MirConnection.cs:316`
(Login/NewCharacter/StartGame/Logout), :1361 (PickUp), and :1454
(Attack); `Crystal/Server/MirObjects/HumanObject.cs:2848`
(Attack) and :3400 (Magic); `Crystal/Server/MirObjects/PlayerObject.cs:4398`
(CompleteQuest) and :7517 (PickUp); and
`Crystal/Server/MirDatabase/CharacterInfo.cs:391` (Save).
Map walkability and object insertion are represented by
`Crystal/Server/MirEnvir/Map.cs:516`, :653, and :2361.

The mir2-web3 slice is directly exercised by
`mir2-web3/apps/simulation/tests/vertical_slice.rs:1690`
(starter loop), :1925 (Q1-Q9), and :2613 (shared presence/movement/chat/
drop ownership); by
`mir2-web3/apps/simulation/tests/shared_zone.rs:1886`, :4354,
:9557, :9636, and :11226; and by the implementation paths
`mir2-web3/apps/simulation/src/runtime/save.rs:1754`,
`combat.rs:3176`, `skills.rs:8150`, `drops.rs:1775`, and
`npc_script.rs:487`.

This proves a playable Bichon vertical slice. It does not prove complete
Crystal skill, NPC, quest, AI, event, timing, RNG, failure, economy, or visual
semantics.

### Shared Zone slice

The shared-zone state machine is implemented at
`mir2-web3/apps/simulation/src/runtime/zone/runtime.rs:118`
(state), :540 (commands), :993 (tick), :1280 (movement), and :8890
(native monsters). The tests above cover authoritative movement/save, AOI,
presence, chat, monster defeat, and one owner-claimed drop path.

The boundary is material: `mir2-web3/apps/simulation/src/world_runtime.rs:353`
still contains the session-local InProcessWorldRuntime adapter. Local Zone
tests therefore must not be described as production shared-backend,
cross-process, PostgreSQL, crash-recovery, or remote reconnect proof.

### Windows native slice

The native entry and shell wiring are visible at
`mir2-web3/apps/game-client/platform-windows/src/main.rs:35`,
:95, :107, :110, and :200. The outbound protocol surface is in
`mir2-web3/apps/game-client/platform-windows/src/native_protocol.rs:31`
and :337; reconnect plumbing is in gateway.rs:1339 and resume.rs:174.
These are implementation and automated functional evidence. They are not
direct evidence of the final Windows visual match, DPI behavior on real
devices, package signing, audio/layout parity, or a human-authenticated
play session.

## Seven material P1 areas still open

The following are substantive parity areas, not cosmetic backlog items. They
remain open until their complete ledger inventory is mapped and all leaves have
independent evidence. The cited Crystal locations identify the semantic
reference or known boundary; the mir2-web3 locations identify the current
implementation/evidence boundary.

| Open P1 area | Crystal reference | mir2-web3 reference and current gap |
|---|---|---|
| 1. Five classes and complete skill semantics | `Crystal/Shared/Enums.cs:824`, :1141; `Crystal/Server/MirObjects/HumanObject.cs:5237` | `mir2-web3/apps/simulation/src/runtime/skills.rs:8150` and `mir2-web3/docs/BACKEND-1TO1-PROGRESS.md:218` show a bounded skill slice, not a complete five-class/skill matrix with exact effects, restrictions, failure, timing, and visuals. |
| 2. Full AI, bosses, and events | `Crystal/Shared/Enums.cs:588` records several boss/AI TODO boundaries; `Crystal/Server/MirObjects/MonsterObject.cs:2150` is a reference path | `mir2-web3/apps/simulation/src/runtime/zone/runtime.rs:8890` and the current runtime/tests cover selected monsters, not the complete Crystal AI/boss/event matrix. Crystal TODOs must be inventoried and classified rather than silently excluded. |
| 3. Complete NPC and quest semantics | `Crystal/Server/MirDatabase/QuestInfo.cs:66` includes ZoneTasks/EscortTasks TODOs; `Crystal/Server/MirEnvir/Envir.cs:4788` has a missing BuffInfo implementation boundary | `mir2-web3/apps/simulation/src/runtime/npc_script.rs:487` and `packets.rs:3503` prove selected interactions only; Q1-Q9 evidence is not all NPC/quest semantics. |
| 4. Exact timing, RNG, rejection, and failure semantics | Crystal packet dispatch at `Crystal/Server/MirNetwork/MirConnection.cs:316` and combat/magic paths at `Crystal/Server/MirObjects/HumanObject.cs:2848`, :3400 are references for ordering and rejection behavior | `mir2-web3/apps/simulation/src/runtime/zone/runtime.rs:45` and `skills.rs:4375` implement bounded rules, but strict packet ordering, timers, RNG streams, edge failures, and negative traces are not globally proven. |
| 5. Production shared backend and recovery | Crystal map/object authority is represented by `Crystal/Server/MirEnvir/Map.cs:653`, :2361 and network lifecycle by `Crystal/Server/MirNetwork/MirConnection.cs:316` | `mir2-web3/apps/simulation/src/world_runtime.rs:353` is session-local; current docs record no direct deployed PostgreSQL/remote Zone/cross-process/crash-recovery proof (`mir2-web3/docs/BACKEND-1TO1-PROGRESS.md:90`). |
| 6. Formal signed Windows Candidate | Crystal’s client/server assets and runtime are the release reference, not a claim that a mir2 package is accepted | docs/AGENT-TASK-QUEUE.md:50-69 and docs/CRYSTAL-1TO1-ROADMAP.md:23-38 record local/package boundaries, unsigned or internal-playtest artifacts, and missing signed Candidate evidence. |
| 7. Windows visual parity, real hardware, and human acceptance | Crystal presentation references include `Crystal/Client/GameScene.cs:4697`, `Crystal/Client/SoundManager.cs:7`, and `Crystal/Client/MirControls/MirGameShopCell.cs:1` | `mir2-web3/apps/game-client/runtime/src/lighting.rs:143`, `runtime/src/lib.rs:207`, and `platform-windows/src/main.rs:95` show rendering/UI implementation; `mir2-web3/docs/CRYSTAL-1TO1-ROADMAP.md:178` still records visual/effect/text/Gemini/human gates as open. |

## Historic evidence retained, but not current truth

The R301/R300 (and related R298/R302) artifacts remain useful evidence. They
must be read as historical, scope-limited slices with the denominator and
environment stated in each report. They do not override the semantic ledger.

- **R301:** preserves the historical Web/package/gate evidence and its
  limitations. It did not establish a complete Crystal capability inventory,
  complete Windows visual acceptance, or a globally signed Candidate.
- **R300/R298:** preserve historical stable-diff and packet-matrix evidence.
  A stable diff for a selected fixture is not proof of exact global timing,
  RNG, rejection, or failure semantics.
- **R302:** preserves the original-client launch/capture diagnostic. It does
  not constitute same-scene, independently attested human visual acceptance.
- **R12 (2026-08-23):** records the current functional-loop evidence and the
  remaining live Windows UI, deployed WebSocket, package, visual, and human
  gates. Its “2/2” or similarly scoped results must not be expanded into a
  whole-project percentage.

Older documents may still contain statements such as “100% Candidate” or
“roughly 90%” because they were written for a narrower release slice or a
previous denominator. Those statements are historical claims only and are
superseded as current truth by this audit and the semantic ledger. In
particular, docs/AGENT-ORCHESTRATION.md:206-212 and
docs/AGENT-TASK-QUEUE.md:405-418 must not be used to infer a current global
score.

## Wording that is explicitly retired

The following are no longer valid current conclusions:

1. “Whole-project automated evidence: 100% Candidate.”
2. “Whole-project accepted Crystal 1:1: roughly 90%.”
3. “Global P0=0/P1=0” derived from a GameShop, UI, native-shell, or other
   scoped matrix.

The accurate replacement is: **the ledger inventory is incomplete, the global
percentage is undefined, selected Bichon/Zone/Windows functional slices have
direct evidence, no global automated P0 zero has direct evidence, and the seven
material P1 areas above remain open.**

## Condition for a future 100% claim

The project may claim 100% only when all of the following are true at the same
release Git revision:

1. docs/CRYSTAL-SEMANTIC-PARITY-LEDGER.md has
   inventoryComplete=true, with the complete, hashed Crystal source
   inventory and no silently excluded capability families.
2. **All ledger leaves are VERIFIED**, including rows previously marked
   BLOCKED_EXTERNAL, IMPLEMENTED_UNVERIFIED, or unsupported until their
   status is actually resolved according to the ledger.
3. Independent automated evidence shows no global P0 or P1, including negative
   cases, ordering, timing, RNG, failure, persistence, and security behavior.
4. The Windows package gate passes, including a formally signed/attested
   Candidate package; Web regression/build gates pass; persistence and
   reconnect behavior pass; stability/soak gates pass.
5. Visual evidence is same-scene and independently attested on the required
   Windows DPI/devices, and human visual/feel acceptance is recorded. Evidence
   has hashes, schema, freshness, and release-revision binding.

Until then, completion is a set of named ledger statuses, not a percentage.

## Shortest next path

1. Finish the Crystal source inventory and publish its hash; only then can
   inventoryComplete change from false.
2. Convert the existing Bichon, Zone, and Windows slices into explicit ledger
   rows with immutable evidence, negative cases, and freshness/revision
   binding.
3. Close the seven open P1 families in dependency order: skill/semantic
   inventory, AI/events/NPC quests, exact packet/timing/RNG/failure behavior,
   production shared backend/recovery, then package and visual/human gates.
4. Re-run the Web regression, persistence, stability, signed-package, and
   real-device visual gates at one release revision. Recompute status from the
   ledger; do not infer it from file counts or prior scoped reports.
