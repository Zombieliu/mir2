# P7 source world actions: read-only audit, 2026-10-09

Status: **OPEN; implementation and ordinary shared-world acceptance remain incomplete.**
This records source inspection in the isolated gateway-lifecycle lane, based on the supplied 88a36e7a2f anchor and its current working diff. It is not a release receipt, test execution, public-server check or human frontend acceptance.
Project paths below are relative to `mir2-web3`; `Crystal/` denotes the separately supplied original-source checkout root alias. Line anchors were read on 2026-10-09 and may move during integration.

## Source and current execution anchors

- `NPC`: `Crystal/Server/MirObjects/NPC/NPCSegment.cs`.
- `N`: `apps/simulation/src/runtime/npc_script.rs`.
- `R`: `apps/gateway/src/routing.rs`.
- `D`: `apps/simulation/src/runtime/default_npc_events.rs`.
- `S`: `apps/simulation/src/runtime/shared_guild_experience.rs`.
- Imported ordinary source: `packages/game-data/data/generated/crystal_npc_manifest.json`; NPC identities: `crystal_npc_info_manifest.json` in the same directory.
- Expanded default source and sealed parser omissions: `packages/game-data/data/generated/crystal_default_npc_scripts.json`.
- Map/event import status: `packages/game-data/data/generated/crystal_map_event_manifest.json`; imported files are not proof of a scheduler.

Crystal executes `ActList` in order (`NPC:3047–3054`) before `ParseSay` (`NPC:4956–4973`); failed checks execute `ElseActList` before `ElseSay`.
`N:895–967,1033–1136` currently executes each action against the personal ECS and follows GOTO sections immediately. Later checks and actions must observe the earlier executed prefix.
Source `return`, BREAK, GOTO, invalid arguments and no-op behavior must be preserved separately from infrastructure/CAS failure. Replacing an Act-abort with Continue can execute rewards the original would never reach.

## Seven concrete gaps

| Action | Original rule and source | Current gap and anchor |
| --- | --- | --- |
| GIVEPET | `NPC:533–539,3421–3447`: defaults 1/0; byte count capped at 5 **per call**, valid zero stays zero; level capped at 7; only new pets receive that level. Assign real Master, owner direction and ActionTime +1000, invoke Spawn at the owner's exact cell, then append to Pets. | `N:2974–3040` clamps zero to 1, offsets cells and sets the level on every matching existing pet; creation is personal ECS. No ordered shared creation/identity receipt, source action delay or same-segment new-pet XP contract. |
| REMOVEPET | `NPC:3450–3458`: reverse owner Pets traversal, case-insensitive name match, invoke each matching pet's real Die. Dead references remain until Human processing removes them (`Crystal/Server/MirObjects/HumanObject.cs:317–320`). | `N:3050–3064` selects living personal pets and despawns them. Death/lifecycle, corpse/AOI, same-segment PETCOUNT/CHECKPET and authoritative owner custody differ. |
| CLEARPETS | `NPC:3461–3467`: mark each owner pet DieNextTurn. Actual next AI processing invokes Die (`Crystal/Server/MirObjects/MonsterObject.cs:1671–1678`). | `N:3043–3047` immediately despawns personal pets. Cannot collapse deferred death into removal or change what an immediately following check/XP action sees. |
| MONGEN | `NPC:3728–3772`: PARAM1 selects map and instance; PARAM2/3 are coordinates; zero coordinate/invalid byte count aborts Act. Spawn each monster at the same source cell, direction 0, ActionTime +1000. | `N:3067–3140` records instance but only spawns on the actor's current personal map, offsets each cell and defaults invalid count to 1. Target-map shared spawning, source order, abort behavior and durable identities are missing. |
| MONCLEAR | `NPC:858–865,3825–3849`: resolve map/instance, optional case-insensitive monster name, traverse cells/objects in source order and invoke Die on living Monster-race objects, including pets. Existing EXPOwner governs death rewards (`Crystal/Server/MirObjects/MonsterObject.cs:968–1007`). | `N:3144–3181` only touches current personal map and sets dead/HP0; ignores instance/name and does not perform authoritative Die/drop/XP/lifecycle. Do not forge an Attack or assign the script caller as a new damage owner. |
| GROUPTELEPORT | `NPC:872–890,3862–3883`: iterate real GroupMembers in order; resolve map/instance; either zero coordinate uses TeleportRandom(200,0,map), otherwise each member performs real Teleport. | `N:2649–2679,2707` relocates the caller and rebuilds private RemotePlayer snapshots; accepts only three arguments. Peers' actual Zone/personal state, destination admission, owner FIFO, save, instance and random landing semantics are missing. |
| CHECKHUM | `NPC:255–259,1764–1778`: comparator/count/map/optional instance, count the target map's actual Players collection; no AOI or alive-only filter. | `N:2149–2163` requires the caller's current map and compares constant 1. Cannot check an empty/occupied remote arena or serialize competing entrants. |

Pet checks also currently filter out dead personal pets (`N:1997–2032,3185–3201`), so fixing shared spawn alone cannot establish original within-segment pet semantics.

## Why the current bridge is insufficient

`R:15239–15260` computes one before/after entity diff after the personal script. `R:7420–7426` returns no effects when the map changed.
An end diff loses ordered create/remove pairs, deferred death and intermediate check/XP visibility; MissDo's MOVE also changes the map after MONCLEAR.
`SharedNpcWorldService` and its committed flag (`R:1896–1957`) carry raw outcome packets; the in-process implementation returns committed without a durable world transaction.
It is not an outbox receipt or restart-safe exactly-once proof. A raw ObjectDied packet is not an authoritative monster death.
`S:308–363,365–407` provides the complete personal Source checkpoint/persistence boundary, including default-event queue, transient NPC state and pet progress; it does not make another Zone's mutation durable.
`D:482–507` drains trusted hooks with a budget and marks their queue sequence. That sequence proves hook progression, not shared world-action consumption.

## Minimum trusted ordered plan and outbox

The following is a proposed implementation contract, not an existing API or schema.

1. At a normal authenticated NPC/default-hook boundary, capture immutable actor/account/character identity, save revision, current lifecycle and Zone owner fence; bind NPC object/map/instance, source script digest, resolved page/arguments and source event identity.
2. Ask the single Zone writer for a bounded prepared read-set/reservation: target-map population, source-ordered live monsters/pets, owner pet collection including deferred/dead states, actual ordered group identities and destination authority. An AOI snapshot or personal RemotePlayer is insufficient.
3. Execute one Source interpretation against a logical shared-world shadow. Append typed operations in exact order while updating that shadow immediately. Preserve original branch, Act-abort and same-segment visibility; do not interpret the source twice to recover.
4. GIVEPET followed by GIVEEXP must apply each real final XP amount to the pet collection visible at that point. Preserve per-call PetExp progression, not an aggregated sum. The existing admission captured from old pets (`apps/simulation/src/runtime/shared_pet_progress.rs:39–56`) cannot silently stand in for pets newly created by this plan.
5. Commit the frozen executed prefix, allocated logical object identities, original costs/rewards/flags/RNG and pending ordered world plan in **the same Source CAS**. A later independent world write cannot protect an already consumed AdmissionOrb or arena fee.
6. Following confirmed CAS, have the authoritative Zone consume the immutable outbox using durable plan/op receipts and source-order effects. Preserve source spawn delays and deferred AI death; do not restart a delay or reroll IDs/landing/death rewards on replay.
7. Known pre-persist rejection restores the complete Source checkpoint and releases unused preparations. Lost/uncertain persistence keeps the exact plan and its identities pending/frozen until authoritative readback; no success ACK, fake rollback or automatic reward retry.
8. Crashes after Source CAS, during Zone consumption or before receipt ACK resume the same plan. Zone receipts must distinguish applied, rejected and uncertain outcome and survive owner/process restart; replay cannot duplicate pets, monsters, drops, XP, costs or teleport.
9. Validate prepared map/instance/life/group revisions and owner lease at apply/recovery. If the target changed, retain the committed outbox and resolve its original authority; do not silently retarget the current character, respawn incarnation or replacement group member.
10. Serialize competing CHECKHUM/read-write plans under shared authority. Two Source CAS transactions must not both admit from an unreserved zero-population observation. Cross-Zone operations need explicit durable coordination, not a mutex held across blocking storage.

Reuse the Source guard/save-CAS and prepared/recovery patterns, single-writer Zone commands, authoritative death/claim attribution, normal Join/Leave/transfer lifecycle, owner FIFO and ordinary save verification.
Do not reuse raw packet side-effect receipts, default-event committed_sequence, ground-item custody or an XP-death ticket as generic world-plan authorization.
Any new outbox/receipt representation is an explicit compatibility and security change led by the root integrator; this audit adds none.
No new public QA, raw world-plan, client-supplied actor, arbitrary map spawn, account selection or receipt-issuance command is needed.

## Real source stories to qualify

### MissDo arena

Original file: `Crystal/Build/Server/Debug/Envir/NPCs/BichonProvince/Event/MissDo-EM000.txt`, SHA256 `2d0b304656224e0fba445c4a1686a7fdc90df428157dcc3c24fca22f285187ea`.
The Main link points directly to @checklevel (line 8); that page checks level <=30, sets flags 500–520, then GOTO @start2 (lines 25–50).
The separate @start page checks `CHECKHUM >= 1 EM001` (lines 16–23); it is **not** on that Main link's current path. Preserve this source fact rather than inventing a required occupancy gate.
@start2 checks 3000 gold, then executes TAKEGOLD 3000 → MONCLEAR EM001 → TIMERECALL 3600 → MOVE EM001 13 16 (lines 57–67).
The actor begins on EM000; current MONCLEAR cannot clear EM001, and current TIMERECALL only logs. The explanatory text says two hours while the actual command is 3600 seconds; do not silently substitute 7200.
Qualification must exercise ordinary NPC links, insufficient gold, level failure and the actual successful sequence; separate source-supported @start invocation must test population branches without changing menu routing.
Observe target-map occupants, monsters/pets with original ownership, fee and flags, timed return identity, transfer and save; inject pre-CAS failure, post-CAS crash and lost receipt ACK in isolated fixtures. No public account/spawn grants.

### FoxRock / GuardianRock party entrance

Original file: `Crystal/Build/Server/Debug/Envir/NPCs/MongchonProvince/FoxCave/Rock.txt`, SHA256 `a818b876db13c3f984285693102e2fb53733276c34a5d8b119870a1fbc2d0825`.
Imported NPC is GuardianRock on Fox02 (`crystal_npc_info_manifest.json:6873–6876`); “FoxRock” here names this story, not a fabricated NPC or scheduler.
@MAIN requires GROUPLEADER and GROUPGOTO @grouphere. @grouphere checks GROUPCOUNT >4, CHECKMAP FOX02 and CHECKRANGE 223 304 5 before @checkorb (source lines 1–35).
Source GROUPGOTO schedules the page for each real member at the current server time (`NPC:4053–4061`); `N:2407–2411` instead follows one caller's synchronous GOTO.
@pass checks the actor's AdmissionOrb again, then TAKEITEM AdmissionOrb 1 → GROUPTELEPORT FOX03 47 21 → BREAK (lines 37–46).
Current GROUPLEADER is always true and group checks resolve personal entities (`N:1650–1689`); their trusted shared inputs are prerequisites, not proof supplied by an existing group UI.
Use five normal authenticated members and actual source NPC range/pages. Test nonleader, missing member, wrong map/range, missing orb, disconnect/replacement identity, occupied/random/fixed destination and ordinary transfer/save/relogin.
Verify the same orb UID is consumed once, every eligible real member moves through its actual Zone lifecycle in source order, observers see remove/add and recovery resumes pending members without recharging or remapping identities.

## Scheduling, holes and timed recall remain open

Original Envir calls `Robot.Process(RobotNPC)` (`Crystal/Server/MirEnvir/Envir.cs:2382`) and creates RobotNPC (`:3412`); `Crystal/Server/MirEnvir/Robot.cs:62–76,79–86` matches declared calendar pages and calls them.
The supplied `Crystal/Build/Server/Debug/Envir/NPCs/00Robot.txt` is zero bytes, SHA256 `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
The 18 `Events/...` files in `crystal_map_event_manifest.json` are imported source data; their original general scheduler caller has not been identified. Do not invent timers from file names or mark these scheduled events implemented.
NeedHole originally requires an actual ground SpellObject with DigOutZombie or DigOutArmadillo (`Crystal/Server/MirObjects/PlayerObject.cs:4514–4520`) before movement/NeedMove binding (`:4529–4533`).
Current trusted source-entry binding explicitly excludes need_hole (`D:356–359,407,437`). No ground-hole producer/complete entry story has been qualified.
TIMERECALL records NPC/script/page, original map/coordinate and deadline as a DelayedAction; BREAKTIMERECALL cancels NPC delayed actions (`NPC:3775–3813`).
Current `N:3546–3557` only appends text to a stage5 event log; TIMERECALLGROUP/DELAYGOTO, expiry dispatch and recovery are not established by that log.
Malformed source omission must remain exact: `CHECKHUM 1 D10071` has three tokens; the original parser requires at least four (`NPC:255–259`). Do not reinterpret it as a newly valid arena check.

## Bounded implementation rounds and acceptance

1. **Ordered interpretation:** new `apps/simulation/src/runtime/npc_world_plan.rs` and dedicated tests, bounded `npc_script.rs` hooks; root owns module/export and Source checkpoint integration. Establish seven rules, abort/no-op, same-segment checks/XP and immutable plan identities in isolated fixtures.
2. **Durability and authoritative application:** new Zone world-plan module/tests and Gateway outbox adapter/tests; bounded Zone types/manager routing. Root alone owns common save/outbox compatibility, source-CAS changes, `world_runtime.rs`, save guards and `routing.rs` glue. Qualify pre-persist rollback, lost commit ACK, partial apply, restart, stale owner/life/map rejection and once-only death/drop/XP.
3. **Ordinary stories and group lifecycle:** bounded actual shared-group input/page scheduling and per-member transfer adapter. Run MissDo and GuardianRock through normal auth/NPC protocol, real population/group identity and ordinary saves; preserve source menu/flags/fees/orb and failure pages. Windows client rendering/player acceptance is separate from transport qualification.
4. **Observed delayed/entry callers:** implement source-bound TIMERECALL/DELAYGOTO and real NeedHole production in separately owned modules with normal clock, cancellation, persistence and replay tests. Only implement general scheduling after an actual source declaration/caller is traced; empty Robot/event imports remain explicitly open.

Freeze and retain exact source/data hashes, positive and negative logs and failure evidence for each round; update Candidate/public/human status separately after tests pass. No test results were generated by this audit.
