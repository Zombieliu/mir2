# Windows visual parity user-observed UI, render, and quest backlog

Date: 2026-08-29

## Purpose

This document records user-observed Windows native gaps that remain open after
bounded automation checkpoints. It is a backlog and acceptance aid, not a
completion claim.

The list is the working denominator for the next Windows parity goal. A narrow
unit test or a button image being present does not close an item: the real
function, Crystal state rules, exact assets, hover/help behavior, same-EXE
route, and human-visible result must all be accounted for.

## Claim state

```text
branch: codex/windows-visual-parity
recordedHead: dd3179559c74756d15c4cbf2c712cb659437d1cb
globalParityPercent: null
visualAccepted: false
accepted: false
sameExeAuthenticatedLiveWssClosed: false
realDpiClosed: false
nativeThirtyMinuteSoakClosed: false
humanVisualAudioFeelClosed: false
formalPublisherSigningClosed: false
```

## Latest implemented checkpoint

Revision `dd3179559` closes the bounded automated part of `WN-INPUT-003` that
was known to contradict Crystal source. Native NPC left click now sends an
immediate interaction without auto-walking; Simulation owns the visible-NPC
square DataRange 16 decision for initial/follow-up/quest operations, rejects 17,
does not force a turn, and accepts `[@MAIN]`. The client also applies Crystal's
same-object five-second guard. Simulation 1485/1485, Gateway 664 active plus one
ignored, Windows 484/484 and ordinary Candidate loop 2/2 pass.

This is not visual or playable acceptance. The currently running Candidate is
still based on `4fc98ecc4`, so no screenshot or user observation is attributed
to `dd3179559`. Exact response-reset timing, blocked/removed NPC lifecycle,
same-EXE dialog capture and human feel remain in the denominator. The absent
daily-blue `Prguse 991..994` PNGs were not replaced with placeholders and were
not falsely added to the package-required gate.

## User evidence captured in this round

The following user-provided comparisons are treated as failure observations,
not acceptance images. Some screenshots were taken from successive local
Candidates, so each closure retest must bind the screenshot to the exact EXE
SHA-256 rather than assuming every image represents one build.

- `codex-clipboard-dc071350-a72c-499d-b8aa-21c7fe6923fa.png`: Crystal login-to-game announcement.
- `codex-clipboard-3a3a30fc-c54a-4a91-a402-6ea0cf99cda7.png`: Crystal/native same-scene HUD comparison.
- `codex-clipboard-ef808baf-8c85-4df6-b457-952929317178.png`: building solidity, foreground layering, and lamp/fire comparison.
- `codex-clipboard-829794fe-5910-4073-8c25-d6ff3edbdc68.png`: safe-zone and compact HUD/chat controls.
- `codex-clipboard-cb6e8065-c817-4786-ad84-2e3724c60540.png`: world-map dialog and right-side function strip.
- `codex-clipboard-e0b2ebc0-4737-431b-83c1-e00afbc22520.png`: right-side vertical function-strip detail.

## Open backlog denominator

| ID | Priority | Area | Crystal expectation | Current native status | Required closure evidence |
| --- | --- | --- | --- | --- | --- |
| WN-BOOT-001 | P1 | Login announcement | After entering the game, show the Crystal-style notice window with the configured body, frame, scrolling/link rules, and close flow. | The current branch now contains a native in-game `UpdateNotice` -> `NoticeDialogState` -> Crystal notice panel chain using the original `Prguse/961`, `Prguse2 470..475`, and `Title 193..195` assets, and the Windows gameplay bridge resets/replays it per session generation so one close does not reopen from the same snapshot. Focused simulation and native notice tests cover trigger, close/re-login, and session reset behavior. Exact same-EXE screenshot/video, scrollbar/link fidelity review, and human acceptance remain open. | Source-bound trigger test, close/re-login test, exact same-EXE screenshot, and no duplicate popup in one session. |
| WN-BOOT-002 | P2 | Build identity | Candidate/test screens show sourced version, short Git commit, platform, and TEST/CANDIDATE marker; production uses the concise product version and no test marker. | Policy requested but not closed on this branch. | Build-metadata provenance test plus Candidate and production-shaped screenshots. |
| WN-WORLD-001 | P0 | Map transfer | A destination map renders its own terrain/objects atomically and retains the self actor. | Type1 fixes are automated, but the user's earlier GroceryStore failure remains a failed observation until exact-Candidate retest. | Exact EXE identity, source/destination route, no stale pixels, correct local actor, and map-wide denominator expansion. |
| WN-WORLD-002 | P1 | Raster sharpness and building solidity | Original pixels stay crisp at integer geometry; roofs/front cells have Crystal opacity, depth, and occlusion. | Global Bevy sampling is already nearest, so this is not closed by changing one sampler. User still sees softer buildings and weaker foreground volume. | Same viewport and DPI pair, integer-transform audit, front/middle layer count/depth assertions, and pixel-diff review. |
| WN-WORLD-003 | P0 | Environmental animation | Tile, middle, and front animations advance with Crystal frame counts/ticks; lamp flames and fires visibly animate. | Current head includes a bounded implementation that parses Type100 tile-animation fields, expands complete tile/middle/front animation families, advances a 100 ms Crystal-compatible clock with per-family tick dwell, and holds the base frame when a packaged family is incomplete. Focused map-parser and runtime animation tests are green on this head; exact Candidate timed capture, additive/blend fidelity, full asset-family audit, soak, and human acceptance remain open. | Timed capture proving at least two source-correct frames, phase/tick unit tests, full visible animation-family residency, and soak stability. |
| WN-WORLD-004 | P1 | Safe-zone presentation | When the imported server enables `SafeZoneBorder`, Crystal keeps `TrapHexagon` world objects visible at boundary tiles while they remain in AOI; local `inSafeZone` changes independently on entry/exit. | Revision `aae9c2c7e06dbceb6f6539c7b29eba63ece293c4` removes the hard-coded false default and derives the switch from imported `TrapHexagon` manifest evidence, while preserving explicit opt-out. The Windows effect path already renders exact `Magic 1390..1399` frames at 100 ms and retains them until authoritative remove; `inSafeZone` now also reaches the shared read model. Simulation 1482/1482 and focused Windows/read-model tests pass. Exact-Candidate capture and human acceptance remain open. | Enter/leave state automation, AOI add/remove and scene-clear tests, exact same-EXE timed capture, and human comparison. |
| WN-WORLD-005 | P1 | Lighting and foreground effects | Lamps, spell/world lights, roof darkness, additive pixels, and foreground ordering match Crystal without flattening the scene. | Static lighting exists, but the user comparison shows visible depth/effect drift and fire is static. | Source-bound light inventory, same-scene night/day captures, order assertions, and human review. |
| WN-ACTOR-001 | P0 | Player animation integrity | Idle/walk/run/direction/composite layers remain coherent, do not flicker, and use Crystal cadence. | Several bounded fixes passed tests and the user later reported direction animation improved, but sustained-run flash/cadence and full action/class denominator remain open. | Long movement trace, frame-sequence assertions for every in-scope direction/action, same-EXE video, and human feel acceptance. |
| WN-INPUT-001 | P0 | Ground movement | Left-click walk and held right-click run follow Crystal intent, marker, cadence, path, collision, and cancellation semantics. | User reported missing left-click walk, held-run behavior, and continued stutter across earlier Candidates; only bounded paths are automated. | Pointer event matrix, packet/ACK timing trace, collision/release cases, and five-minute live loop. |
| WN-INPUT-002 | P0 | Monster targeting/combat cursor | Monster hover changes to the correct cursor/highlight; click approaches and attacks with authoritative range rules. | Some cursor leaves exist, but the user reports live mouse combat is not complete. | Empty/NPC/monster/dead-target cursor matrix and real attack/approach outcome assertions. |
| WN-INPUT-003 | P0 | NPC hover/interact | NPC cursor, request timing, range, single-open behavior, dialog, and cancellation mirror Crystal. | Revision `dd3179559` removes the invented auto-approach/adjacency path. A visible NPC click immediately queues `CallNPC [@Main]` without Walk/Run; Simulation applies one visibility-aware square DataRange 16 gate to open/follow-up/quest operations, does not force a turn, and Windows applies the same-object five-second guard. Boundary, shared-authority and full crate tests pass. Exact response-reset timing, blocked/removed NPC lifecycle, same-EXE dialog capture and human retest remain open. | Near/far/blocked/removed/double-click tests, exact response-reset transcript, same-EXE dialog screenshot and no duplicate request. |
| WN-QUEST-001 | P0 | Quest availability/turn-in markers | NPCs show source-correct `!`, `?`, or equivalent Crystal marker states for available, active/incomplete, and ready-to-turn-in quests; markers update without relog. | Simulation now emits a per-character authoritative Crystal `questIcon`, keeps it out of shared Zone state, and Gateway reapplies it per requesting session. Selection follows Crystal's current-quest-first insertion order and exact start/finish NPC, level, class and prerequisite gates; the former invented status priority is removed. Windows maps all seven Crystal discriminants and 500 ms frame formula, retains the marker across partial NPC packets, and passes a fresh q1 Jane `!` -> Jude `?` -> next-q2 `!` transition plus 483/483 native tests. Marker placement now consumes each NPC standing-body frame-zero width and source offsets with Crystal's exact `/2 - 28, -40` formula, and remains visible when `NameView` is off. Candidate gates require the resident yellow/white/green frames. Same-EXE anchor/transitions, daily-blue source frames `991..994`, complete live transitions, occlusion/z-order, and human acceptance remain open. | State-transition matrix, NPC/object binding test, same-EXE body-anchor and occlusion/z-order acceptance, source closure for daily blue frames, focused Windows overlay tests, and same-EXE captures for every state. |
| WN-QUEST-002 | P0 | Quest end-to-end flow | Discover -> NPC dialog -> accept -> objective progress -> tracker -> completion -> turn-in -> reward/persistence works for the audited quest set. | The backend denominator is materially stronger than the current native feel suggests: simulation already carries explicit NPC dialog links plus deterministic `@quest:accept:*` / `@quest:finish:*` coverage across starter and broader quest loops, and native already has `QuestTracker`, `NpcDialogModel`, typed `AcceptQuest` / `FinishQuest` forwarding, and pending-operation de-duplication. What remains open is the Windows-native closure of that chain under the same exact EXE the user plays: dialog entry reliability, live marker/state transitions, tracker/diary visibility, reward selection, reconnect/save recovery, and human play validation across an explicitly bounded quest set. | Source-derived quest inventory, deterministic end-to-end tests, reconnect/save recovery, multi-session correctness where applicable, and human play pass. |
| WN-QUEST-003 | P1 | Quest diary/tracker/map guidance | Quest button, diary tabs, objective text, navigation hints, target/map markers, abandon/share rules, and empty/error states match Crystal. | Partial panels/models exist and many local/typed leaves are implemented, but the visible quest diary/tracker/navigation experience is not accepted. Current branch evidence shows the backlog must distinguish real missing behavior from unrelated dirty work: the present unstaged `crystal_ui/overlays.rs` draft is inventory-expansion UI, not quest guidance closure evidence. | Per-control behavior matrix, known quest snapshots, map/NPC link tests, and same-EXE screenshots. |
| WN-HUD-001 | P1 | Main HUD geometry | Orb HUD, belt, chat strip, experience/weight, minimap, labels, right controls, and scaling use Crystal bounds and ordering. | Visibly present but user reports substantial layout/texture drift. | Geometry assertions at 1024x768 plus real 100/125/150% DPI captures and same-scene comparison. |
| WN-HUD-002 | P1 | Main HUD functional buttons | Character, Inventory, Skill, Quest, Option, Menu, GameShop, Mail, BigMap, and Minimap toggle must open/close the correct functional panel with proper enable rules and sound. | Typed actions and several bounded leaves exist; user reports the right-side functions are broadly unimplemented in live play. | Ten-button open/close/state/sound matrix, panel-content assertions, and same-EXE interaction recording. |
| WN-HUD-003 | P1 | System-menu vertical strip | Visible functions must cover Exit, Logout, Help, Keyboard Layout, Ranking, Intelligent Creature, Ride, Fishing, Friend, Mentor, Relationship, Group, and Guild. Crystal's hidden/no-op Crafting entry must stay distinguished rather than fabricated as a working feature. | Art/spec entries exist; complete native behavior is not closed or human accepted. | Thirteen-visible-button behavior/tooltip matrix, deliberate hidden Crafting assertion, and server-backed tests where required. |
| WN-HUD-004 | P1 | HUD button tooltips | Every compact button shows the correct localized Crystal help text after the source hover delay and clears correctly. | User reports help text is missing. | Source string ledger, hover-delay/leave/overlap tests, and screenshot matrix. |
| WN-CHAT-001 | P1 | Chat control row | Home/Up/Down/End/position, All/Shout/Whisper/Lover/Mentor/Group/Guild/Trade, Resize, Settings, and related filters perform their Crystal actions. | The module contains typed actions, including historically presentation-only handoff comments; user reports the small buttons are not fully functional. | Full control matrix, outbound prefix/filter assertions, scroll/resize/settings persistence, and same-EXE recording. |
| WN-CHAT-002 | P1 | Chat control tooltips/states | Mini-buttons have exact normal/hover/pressed/disabled art, click sound, and localized help text. | User reports no explanatory hover text and visible mismatch. | Asset/source ledger, interaction-state tests, and hover screenshots. |
| WN-CHAT-003 | P1 | Chat panel fidelity/content | Background opacity, frame, line spacing, resizing, input editing, system-message provenance, and scroll bar match Crystal. | Earlier transparency and unwanted database-release messages were observed. The current branch now removes the default third-party `LOMCN` / `Suprcode` / `Now in Net` fallback from Gateway startup `LineMessage` rotation and from the Web comparison fallback, so an unconfigured local Candidate no longer injects unrelated project text by default. Full chat rendering, resizing, input-repeat, scrollbar fidelity, and same-EXE visual acceptance remain open. | Clean configured announcement source, input repeat tests, resize/scroll captures, and no third-party project text unless explicitly configured. |
| WN-ITEM-001 | P1 | Inventory/belt geometry | Inventory and belt cell size, spacing, frame, padding, clipping, and icon fit match Crystal. | User reports visible size differences. Current HUD declares six 35-pixel belt steps and wider inventory work remains mixed with an unrelated user draft. | Source-derived bounds, no-overlap assertions, same-item side-by-side screenshot, and 100/125/150% DPI checks. |
| WN-ITEM-002 | P0 | Item icons | Every carried/belt/equipment item in the audited slice renders its exact source icon, count, durability overlays, and unavailable state. | Earlier missing item art was repaired for one package, but full item denominator and current same-EXE evidence remain open. | Item manifest coverage, missing-asset fail-closed test, representative grid screenshot, and complete audited-slice count. |
| WN-ITEM-003 | P1 | Item detail tooltip | Hover shows source-correct name, type, weight, durability, stats, requirements, bind/trade restrictions, description, price, and comparison where Crystal does. | User reports detail popups are missing/incomplete. | Known-item field matrix, edge anchoring/overlap tests, and same-EXE screenshots. |
| WN-UI-001 | P1 | Overall bitmap/text fidelity | Frames, button bevels, bitmap/GDI-like text, colors, opacity, spacing, and z-order match Crystal rather than merely occupying similar regions. | User rejects current overall UI parity and says it trails Web. | Fixed-view screenshot pairs, per-panel geometry ledger, font/text raster review, and human acceptance. |
| WN-AUDIO-001 | P2 | Development audio policy | During active parity development, background music remains muted while effects/UI sounds stay enabled; release restores the intended music policy. | A bounded mute commit exists; final release restore is intentionally open. | Config/build-mode tests and physical audio acceptance. |

## Explicit non-claims

This backlog means the current Windows native line must not be described as:

- full Windows visual parity;
- playable Crystal 1:1 completion;
- whole-game `90%` under a complete semantic denominator;
- or `100%` Candidate/Accepted for native UI/visual work.

The six-hour execution window is a prioritization deadline, not permission to
rename an incomplete denominator as a complete game. Items remain open until
their listed evidence exists.

## Execution order

1. Preserve the exact baseline and add deterministic capture/interaction
   ledgers for the IDs above.
2. Close P0 playability: map animation/transfer, movement/combat/NPC input,
   quest markers and an end-to-end quest slice, and item-icon closure.
3. Close visible P1 controls: main HUD, system menu, chat row, tooltips,
   inventory geometry/details, safe-zone presentation, and world depth.
4. Run Crystal and native through the same scenario with exact EXE hashes,
   compare captures, then perform DPI and soak gates.
5. Keep `globalParityPercent` null until the aggregate denominator is complete;
   leave legal assets, production installer/updater, authenticated live WSS,
   human visual/audio/feel, and formal publisher signing explicit.

## Next-use guidance

When a bounded item above is implemented, update this backlog together with the
more specific VIS report for that leaf and keep unresolved items listed until
they have both automated evidence and human acceptance where required.

## Current branch-head architecture notes

- The current worktree still carries an unrelated dirty
  `apps/game-client/client-bevy/src/crystal_ui/overlays.rs` inventory-expansion
  draft. It is not closure evidence for any backlog item above and should be
  preserved while adjacent parity work proceeds.
- Safe-zone status and source settings are no longer speculative: the world
  snapshot carries `in_safe_zone`, the imported manifest contains the boundary
  `TrapHexagon` objects produced by this project's `SafeZoneBorder=True`, and
  the Windows renderer already owns the exact persistent effect loop. The
  remaining `WN-WORLD-004` gates are exact-Candidate/live/human evidence, not a
  claim that all safe-zone semantics or visuals are globally accepted.
- Login announcement implementation is no longer speculative on this branch:
  `UpdateNotice` now reaches a Crystal-framed native in-game dialog with
  connection-scoped de-duplication. Exact Candidate capture, source link/color
  span fidelity, persistent `LastUpdate > LastLogoutDate` semantics, and human
  acceptance remain open, so `WN-BOOT-001` is implemented but not accepted.
- NPC/quest interaction implementation is also no longer speculative on this
  branch: Windows now emits the source-shaped immediate request and Simulation
  owns the common visible-NPC DataRange gate, while `questIds`, typed quest
  dialog targets, and deterministic accept/finish flows remain available.
  Remaining Windows quest items are closure and parity gates, not proof that no
  quest chain exists.
