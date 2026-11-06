# WN-CANDIDATE-01 R12 Functional Slice Report

Date: 2026-08-23

Status: functional protocol/simulation gate passed; the full Windows Candidate
and human/Gemini visual acceptance gates remain open.

## Outcome

R12 closes the player-visible task/pickup reliability holes in the intended
Windows vertical slice:

- opening the Village Guide no longer auto-accepts or auto-completes the starter
  task. The authoritative dialog exposes explicit `@AcceptQuest:1001` and
  `@FinishQuest:1001` actions, Windows maps those buttons to the exact protocol
  commands. That native path requires the matching current dialog link,
  correct NPC identity, one-tile authoritative proximity, valid task stage,
  and the Wasp Stinger proof before granting rewards. Web quest-log Accept and
  Complete are enabled only when the current server-owned NPC dialog exposes
  the exact matching link, and the server revalidates that link before the
  ordinary Web command mutates state. The old `npcIndex: 0`/no-dialog path is
  rejected, so Web, Windows and Crystal share the same world interaction gate;
- a full native normal-command lane no longer reports success while discarding
  pickup. Saturation reaches the producer, the batch drain no longer consumes a
  ninth reliable command past its limit, and the gameplay bridge retains old
  pickup/quest intents ahead of new input until they enter the transport;
- native typed `AcceptQuest`/`FinishQuest`/`AbandonQuest` submissions now carry a
  monotonic `requestId`. Normal authoritative execution and pre-execution
  capacity rejection echo that id in an ACK or NACK in the causative world
  snapshot. Windows consumes every ACK in a frame, strips it before caching,
  and releases only the matching logical key plus request id; a delayed ACK for
  an earlier submission cannot release a replacement using the same quest key.
  Transport retries reuse the id until a connection generation changes, then
  retained unsent intent is rebound to a new id. Malformed ACK envelopes fail
  native transport validation. Server panic, Zone-registration and flush
  failures do not manufacture an ACK; they terminate the socket, and the next
  generation retires the old correlations. Extreme retry eviction returns the
  dropped intent so its pending key is released explicitly. Imported Crystal
  `@quest:*` links still execute through `SelectNpcDialog`; they do not claim
  the typed request-id contract.

The ordinary candidate test now drives an unprivileged new Warrior through the
following exact state sequence:

1. enter map file `0` with the authoritative `BichonProvince` identity and
   move/turn using ordinary client packets;
2. reject remote task mutation, then walk to the Village Guide and open the
   Available dialog through `CallNpc` without changing task state;
3. observe and select the exact `@AcceptQuest:1001` option through the NPC
   dialog target seam; only the resulting explicit accept action changes the
   task to `InProgress` and closes the dialog;
4. fight live Field Wasps with ordinary turn/attack packets;
5. reach `ReadyToTurnIn` and receive the Wasp Stinger in quest inventory;
6. walk onto a visible Wasp gold drop and collect the exact `GainedGold` value;
7. reject remote turn-in while away from the guide;
8. return to the guide, open the Ready dialog without completing the task,
   observe and select `@FinishQuest:1001` through the dialog target seam, then
   receive `CompleteQuest`, 300 gold, two Repair Powders, and the Guide Ring
   while consuming the task item;
9. drop one rewarded Repair Powder through `DropItem`, walk onto the resulting
   visible item object, pick that exact object id up through
   `SimulationSession::pick_up`, and prove exact bag merge and `ObjectRemove`
   behavior. Gateway `pickUp(objectId)` JSON-to-action mapping is verified
   separately; an authenticated live WebSocket pickup is not claimed here;
10. save, construct a fresh session, log in again, and compare the complete
   inventory, belt, equipment, quest log, known skills, vitals, progression,
   class/gender/level, position, direction, and gold state.

The combat loop refreshes the moving monster's authoritative position after
walking, avoiding a stale-coordinate false failure without weakening combat
rules.

## Verification

All commands below used one build job and one test thread.

| Gate | Result |
| --- | --- |
| `ordinary_candidate_loop` through Cargo after exact dialog-target hardening | 2 passed, 0 failed, 2.28 s |
| focused Bichon vertical-slice loop | 1 passed, 0 failed, 0.83 s |
| starter plus Crystal/Web `@quest:accept` / `@quest:finish` link compatibility and strict no-dialog rejection | 2 passed, 0 failed |
| direct quest packet exact-dialog, stale-link and proof-item lifecycle | 1 passed, 0 failed, 0.53 s |
| stale dialog replay after ordinary movement | included in `ordinary_candidate_loop`; passed |
| native request-level ACK/NACK and delayed-old-ACK isolation | 1 passed, 0 failed |
| native same-frame NACK retention | 1 passed, 0 failed |
| native reconnect-generation stale-ACK isolation | 1 passed, 0 failed |
| retained quest retry rebinds to a new id after generation release | 1 passed, 0 failed |
| malformed ACK transport rejection | 1 passed, 0 failed |
| native exact-dialog click, reward selection, remote quest-log rejection | 3 passed, 0 failed |
| native modal gate preserves exact quest commands and blocks world actions | 1 passed, 0 failed |
| retry-saturation dropped-intent reporting | 1 passed, 0 failed |
| native sustained-backpressure pickup retention | 1 passed, 0 failed |
| native ninth reliable command survives next batch | 1 passed, 0 failed |
| Gateway object/tile pickup mapping | 1 passed, 0 failed |
| Gateway test-target type check, including optional request-id schema and failure paths | exit 0, 19.81 s incremental |
| starter-scene Bichon identity | 1 passed, 0 failed |
| earlier full vertical-slice integration baseline | 8 passed, 0 failed, 192.04 s |
| earlier pre-ACK Windows package baseline | 296 passed, 0 failed; not claimed as a current full-suite run |
| Web `next typegen` + `tsc --noEmit` | exit 0 |
| scoped `rustfmt` on touched Rust files | exit 0 |
| scoped `git diff --check` | exit 0 |

The normal vertical-slice output filename remains held open by an unrelated
process. The verified run used a unique Rust test suffix; no user process was
killed. A full 1,285-test Simulation library run showed no failure in the
observed prefix but was manually interrupted before completion after the host's
recent `0xA` bugchecks. It is not counted as a passing gate.

The focused Gateway request-id tests compile under `cargo check --tests`; a
direct Gateway unit-test run is still not claimed because linking that test
binary is a materially heavier host load. Independently executed native schema,
pending-release and generation/retry tests cover the receiving side. A low-load
CI Gateway test execution remains required before Candidate sign-off.

If a native command is written successfully but its ACK is lost with the
connection, the client does not blindly replay a potentially committed quest
mutation. Reconnect clears the old correlation and refreshes authoritative
state; the player may retry only after seeing that state. This is an explicit
at-most-once/commit-unknown boundary, not an automatic exactly-once guarantee.

## Candidate evidence boundary

This report proves the deterministic personal-session gameplay and persistence
contract, the native reliable-command boundary, and Gateway command mapping.
It does not yet prove:

- actual Windows mouse/keyboard traversal from launcher, login and selection
  through the same task;
- authenticated live Gateway WebSocket routing of every step;
- a fresh Windows release artifact or full Web production regression;
- original-client screenshot baseline coverage and Gemini per-screen scoring;
- 125%/150% real DPI, long soak, external independent review, or human visual
  and feel acceptance.

Those remain required before the project may claim 100% Candidate.
