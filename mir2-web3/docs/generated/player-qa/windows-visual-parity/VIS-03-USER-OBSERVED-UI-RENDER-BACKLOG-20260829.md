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

## 2026-09-03 NPC MirGoodsCell and HideAddedStats checkpoint

The bounded NPC-goods row no longer uses Candidate's combined compact label.
It follows Crystal `MirGoodsCell.cs:20-140`: one `205x32` click/hover surface,
the source-size item image centred within `40x32`, name at `(44,0)`, yellow
count at `(23,17)`, price at `(44,14)`, Lime selection border/divider and
original `Prguse/550` New art at `(190,5)`. The New rule also follows
`NPCDialogs.cs:1348`: `!IsShopItem || MultipleAvailable`, where multiple means
more than one same-index good and at least one non-shop instance. Cell rows
remain at `(10, 34 + row*33)` under source `Prguse/1000`.

Crystal's `NPCGoods.HideAddedStats` bit (`ServerPackets.cs:3082-3104`, assigned
at `GameScene.cs:4199`) now survives packet/snapshot adapters, serde and ECS
refresh. Only NPC-shop hint construction consumes it. Tests prove added
attack/defence values and `Cursed` disappear while base stats and other bind
lines remain intact.

The freshly built client EXE SHA-256 is
`159B13E722451C6F44B036C6B3ABD141E19362EDB28ED29180F34C6849A7DD8A`.
Real Windows input traversed login -> Scott -> View, then F12 captured a
populated baseline and Lime-selected/hover state at BichonProvince `(288,616)`
under `../native-ui-parity-20260903-npc-shop/`. Both sidecars freeze run
`npc-shop-20260903-r2`, `panel=NpcShop`, 1024x768 and DPI 1.0. Native UI passes
514/514, runtime 212/212 and Windows 519/520; only the already registered
Archer `/ARArmour/00/24.png` fixture assertion fails.

This bounds the NPC row/HideAddedStats leaf only. Duplicate/sub-goods panel
topology, other item-surface geometry and populated captures, trusted package/
light provenance, real DPI and human Crystal comparison remain open. All
claim-state flags above stay false/null.

## 2026-09-03 remaining item-tooltip surface projection checkpoint

The five sparse surfaces left by the personal-item checkpoint now retain the
complete input required by the shared Crystal hint renderer. NPC goods and
trade/guild storage preserve actual `UserItem` state; GameShop constructs the
full-durability/count preview used by `MirGameShopCell`; Quest constructs the
zero-count preview used by `QuestCell` and leaves reward quantity on the cell.
Viewer-specific `GetRealItem`, recursive sockets/binds and duplicate-index
failure are tested at the packet adapter, model and ECS hover boundaries.

Native UI passes 511/511. Windows passes 519/520 with only the already tracked
Archer atlas fixture assertion for `/ARArmour/00/24.png`; a current-tree Debug
EXE builds. This supersedes the sparse-model follow-up in the historical
checkpoint below. It does not supply populated same-EXE images for these five
surfaces and does not close exact GameShop/guild/trade layouts, source-sized hit
regions, NPC hide-added-stat presentation, trusted packaging/light, DPI or
human comparison. All claim-state flags above therefore stay false/null.

## 2026-09-03 Crystal item-tooltip checkpoint

The bounded personal-item tooltip leaf is now implemented and exercised in the
real Windows process. Simulation and Gateway carry exact `ItemInfo`,
viewer-resolved `RealItem`, recursive `UserItem`/socket records and live player
stats. One native renderer follows Crystal's eleven-section order and is used
by inventory, belt, equipment, personal storage and the warehouse-side bag;
disabled cells retain hover without emitting an action. The source clock,
unidentified masking, requirement colours and cursor `+(28,28)` clamping have
focused regressions. Native-ui passes 509/509.

Run `item-tooltip-20260903-r1` used exact EXE SHA-256
`5257E859B4AB173A8076B58778C59D09D291A7EB90F0FCFA38F696E46181A56F`.
The real-pointer F12 image
`../native-ui-parity-20260903-item-tooltip/item-tooltip-in-game-1788370099170-2.png`
shows the authoritative WoodenSword hint at BichonProvince `(288,616)`; its
SHA-256 is
`82F2D5ACAB20874FB31D3C3B3EF8EA105D03495BD32ACED27D7BDE0EE6B78AB`.
The report and process/hash ledger are in
`../native-ui-parity-20260903-item-tooltip/README.md`.

This checkpoint does not close the parent item-tooltip denominator. NPC shop,
cash shop, quest-reward, guild-storage and trade rows still use compact models
without the complete authoritative source; soul-bound name resolution and
Hero-stat requirement id 129 are also explicit follow-ups. Trusted packaged
provenance, authoritative light, DPI and human comparison remain open, so all
acceptance flags above stay false/null.

## 2026-09-02 active dialog-parity goal checkpoint

The three new user comparisons add stable denominator leaves; they do not
replace or shrink any earlier row. A row may be split into more precise leaves,
but it must retain a supersession link and every child must close before the
parent can close. `IMPLEMENTED_UNVERIFIED`, draft capture, or pixel similarity
alone remains open.

Source audit establishes three different failure classes:

- CharacterDialog lacks the runtime fields and original `StateItem` / hair /
  wing composition needed for the paper doll.
- InventoryDialog has a close shell but incomplete footer, icon-fit and delete
  semantics. The first bounded working-tree change restores numeric-only Gold,
  free-slot count, source weight-bar selection/crop and the original bin art.
  The bin is intentionally not wired to DropItem; Crystal DeleteMode,
  quantity/confirm dialogs and DeleteItem are still open.
- Quest uses a structural substitute. Crystal owns separate QuestDiary,
  QuestList, QuestDetail and frameless QuestTracking surfaces; the Candidate's
  five-filter, inline-detail `Title/670` panel is not a valid parity target.

The submitted screenshots are also not state-aligned. Candidate `Scout` is the
explicit `demo` development fixture (Lv7, 1,280 Gold and seeded
inventory/equipment/quests); Crystal character `1231` is Lv1 Wizard/Male at
the Bichon map, Gold 0, empty bag, one belt potion, WoodenSword, BaseDress(M),
one quest-inventory item and no magic. Those content differences are fixture
differences, not valid UI-delta evidence. The existing Crystal-state
extract/upsert path still drops hair, quest progress/completed IDs and quest
inventory details and maps Crystal bag slots without subtracting its six belt
slots. It must be repaired before a strict same-state pair is possible.

Current working-tree evidence is
`ui-gap-inventory-current-working-tree/inventory-working-tree-in-game-1788335861573-1.png`
(SHA-256
`0236FAA64D1FDF2A194154428E3E027A73667601827D11EA392DD0F19F97E847`)
with its adjacent JSON sidecar. It is a 1024x768 renderer-owned draft from EXE
SHA-256
`C9B0CE3DD549D82251430A410CACCB8B192A38E3FA2D890CA46A3C1F4D4B4481`.
The sidecar correctly reports `eligible=false`, incomplete authoritative world
state and unavailable trusted package provenance. It is not a same-state
Crystal pair and does not change any acceptance flag.

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
- `codex-clipboard-5edc6499-4a12-44f2-9d8f-b843c8635b2b.png`: CharacterDialog paper-doll/content mismatch.
- `codex-clipboard-f68887b4-adb2-48ab-b0e1-a8d3e75efb1a.png`: InventoryDialog footer/icon/state mismatch.
- `codex-clipboard-bc0327ec-6d49-4b41-b8d0-ed3fa0a63b5d.png`: QuestDiary versus custom combined Quest Log mismatch.

## Open backlog denominator

| ID | Priority | Area | Crystal expectation | Current native status | Required closure evidence |
| --- | --- | --- | --- | --- | --- |
| WN-BOOT-001 | P1 | Login announcement | After entering the game, show the Crystal-style notice window with the configured body, frame, scrolling/link rules, and close flow. | The current branch now contains a native in-game `UpdateNotice` -> `NoticeDialogState` -> Crystal notice panel chain using the original `Prguse/961`, `Prguse2 470..475`, and `Title 193..195` assets, and the Windows gameplay bridge resets/replays it per session generation so one close does not reopen from the same snapshot. Focused simulation and native notice tests cover trigger, close/re-login, and session reset behavior. Exact same-EXE screenshot/video, scrollbar/link fidelity review, and human acceptance remain open. | Source-bound trigger test, close/re-login test, exact same-EXE screenshot, and no duplicate popup in one session. |
| WN-BOOT-002 | P2 | Build identity | Candidate/test screens show sourced version, short Git commit, platform, and TEST/CANDIDATE marker; production uses the concise product version and no test marker. | Policy requested but not closed on this branch. | Build-metadata provenance test plus Candidate and production-shaped screenshots. |
| WN-QA-001 | P0 | Canonical same-state visual fixture | Crystal and Candidate captures bind the same visible character state: character/class/gender/hair/level, canonical map/coordinates/direction/light, HP/MP/EXP, Gold, belt/bag/equipment, quest inventory/progress/completed IDs and open-panel state. Credentials and source account identity are redacted. | Failed. Current comparisons mix the seeded `demo/Scout` fixture with Crystal `1231`; extractor/upsert drops required appearance/quest state and mis-maps bag slots. Current capture is therefore draft-only. | Fixture tests for complete extraction/upsert and slot mapping, redacted canonical-state JSON plus hash in both sidecars, fail-closed pair verification, then three same-state Character/Inventory/Quest pairs. |
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
| WN-QUEST-004 | P1 | Quest surface topology | Q opens source-sized `Prguse/961` QuestDiary grouped by `Group`; row left-click opens the separate `Prguse/960` QuestDetail, right-click toggles one of at most five tracked quests, and NPC quest operations use the separate `Prguse/950` QuestList. | Failed. Candidate currently uses Mail frame `Title/670`, five invented filter tabs, inline details and four combined actions. `group` is already emitted by Gateway Web but is dropped before the Windows quest model. | Preserve `group`/`minLevel`, export all source assets, split state and interaction ownership, geometry/behavior tests, same-state same-EXE screenshots and human acceptance. |
| WN-HUD-001 | P1 | Main HUD geometry | Orb HUD, belt, chat strip, experience/weight, minimap, labels, right controls, and scaling use Crystal bounds and ordering. | Visibly present but user reports substantial layout/texture drift. | Geometry assertions at 1024x768 plus real 100/125/150% DPI captures and same-scene comparison. |
| WN-HUD-002 | P1 | Main HUD functional buttons | Character, Inventory, Skill, Quest, Option, Menu, GameShop, Mail, BigMap, and Minimap toggle must open/close the correct functional panel with proper enable rules and sound. | Typed actions and several bounded leaves exist; user reports the right-side functions are broadly unimplemented in live play. | Ten-button open/close/state/sound matrix, panel-content assertions, and same-EXE interaction recording. |
| WN-HUD-003 | P1 | System-menu vertical strip | Visible functions must cover Exit, Logout, Help, Keyboard Layout, Ranking, Intelligent Creature, Ride, Fishing, Friend, Mentor, Relationship, Group, and Guild. Crystal's hidden/no-op Crafting entry must stay distinguished rather than fabricated as a working feature. | Art/spec entries exist; complete native behavior is not closed or human accepted. | Thirteen-visible-button behavior/tooltip matrix, deliberate hidden Crafting assertion, and server-backed tests where required. |
| WN-HUD-004 | P1 | HUD button tooltips | Every compact button shows the correct localized Crystal help text after the source hover delay and clears correctly. | User reports help text is missing. | Source string ledger, hover-delay/leave/overlap tests, and screenshot matrix. |
| WN-CHAT-001 | P1 | Chat control row | Home/Up/Down/End/position, All/Shout/Whisper/Lover/Mentor/Group/Guild/Trade, Resize, Settings, and related filters perform their Crystal actions. | The module contains typed actions, including historically presentation-only handoff comments; user reports the small buttons are not fully functional. | Full control matrix, outbound prefix/filter assertions, scroll/resize/settings persistence, and same-EXE recording. |
| WN-CHAT-002 | P1 | Chat control tooltips/states | Mini-buttons have exact normal/hover/pressed/disabled art, click sound, and localized help text. | User reports no explanatory hover text and visible mismatch. | Asset/source ledger, interaction-state tests, and hover screenshots. |
| WN-CHAT-003 | P1 | Chat panel fidelity/content | Background opacity, frame, line spacing, resizing, input editing, system-message provenance, and scroll bar match Crystal. | Earlier transparency and unwanted database-release messages were observed. The current branch now removes the default third-party `LOMCN` / `Suprcode` / `Now in Net` fallback from Gateway startup `LineMessage` rotation and from the Web comparison fallback, so an unconfigured local Candidate no longer injects unrelated project text by default. Full chat rendering, resizing, input-repeat, scrollbar fidelity, and same-EXE visual acceptance remain open. | Clean configured announcement source, input repeat tests, resize/scroll captures, and no third-party project text unless explicitly configured. |
| WN-ITEM-001 | P1 | Inventory/belt geometry | Inventory and belt cell size, spacing, frame, padding, clipping, footer and icon fit match Crystal. | Bag1/Bag2/QuestInventory now use source `316x236` geometry, exact grid/icon centring, footer values/states and a source movable-window path with real-pointer before/after evidence. The same-EXE tooltip run also freezes the four starter icons in the source grid. Belt-wide geometry, trusted same-state comparison and DPI/human acceptance remain open. | Source-derived bounds, no-overlap assertions, original item-size/offset tests, same-item side-by-side screenshot, trusted exact-EXE evidence, and 100/125/150% DPI checks. |
| WN-ITEM-004 | P1 | Inventory delete flow | The footer bin enters Crystal DeleteMode, follows the pointer with source cursor/audio, prompts for stack quantity where required, confirms deletion and sends DeleteItem; it never aliases ground DropItem. | The bounded implementation now has Crystal DeleteMode cursor/audio, right-click cancel, stack amount/single confirm, exact `DeleteItem` instance/count forwarding, receipt correlation and stale-item guards. A same-EXE single-item prompt was captured without pressing YES; live stacked-item evidence and human acceptance remain open. | Live authoritative stacked-item amount recording, locked/stale/cancel regression ledger, trusted package provenance and human comparison. |
| WN-ITEM-002 | P0 | Item icons | Every carried/belt/equipment item in the audited slice renders its exact source icon, count, durability overlays, and unavailable state. | Source-sized/centred starter bag icons and stack-count rules are implemented and present in the current same-EXE run. Complete audited item-manifest coverage, durability/unavailable overlay denominator, DPI and human comparison remain open. | Item manifest coverage, missing-asset fail-closed test, representative grid screenshot, and complete audited-slice count. |
| WN-ITEM-003 | P1 | Item detail tooltip | Hover shows source-correct name, type, weight, durability, stats, requirements, bind/trade restrictions, description, price, and comparison where Crystal does. | Bounded personal-item implementation is green: exact/real item plus recursive socket/user metadata and player stats drive Crystal's eleven ordered sections on inventory, belt, equipment, personal storage and warehouse-side bag; disabled cells stay hoverable, .NET expiry/seal/rental text and `+(28,28)` clamping are tested, and real-pointer same-EXE WoodenSword evidence exists. NPC shop now also has the source `205x32` MirGoodsCell layout, `HideAddedStats` behavior and a populated selected/hover F12 capture. GameShop, quest rewards, guild storage and trade retain complete tooltip metadata but still need exact layouts and populated captures; duplicate/sub-goods topology, soul-bound name lookup and Hero requirement id 129 remain open. | Close remaining specialized-surface geometry/topology and populated captures; add known specialized-item matrix, trusted package/light plus 100/125/150% DPI captures, and human same-state Crystal comparison. |
| WN-UI-001 | P1 | Overall bitmap/text fidelity | Frames, button bevels, bitmap/GDI-like text, colors, opacity, spacing, and z-order match Crystal rather than merely occupying similar regions. | User rejects current overall UI parity and says it trails Web. | Fixed-view screenshot pairs, per-panel geometry ledger, font/text raster review, and human acceptance. |
| WN-CHAR-001 | P1 | Character paper doll | CharacterDialog uses gender-specific page art and composes wing, resolved armour, weapon, helmet-or-hair and 14 equipment cells in Crystal draw order with each `.Lib` frame's source offset. | Failed. Candidate draws the shell and equipment icons but lacks `gender/hair/wingEffect` in the UI read model, lacks the `StateItem` export/offset chain, omits the composite layers, and displays a non-source `class + level` title. | Runtime-field provenance, `StateItem`/hair/wing export hashes, layer/order/offset geometry tests, representative class/gender/equipment matrix, same-state same-EXE screenshots and human acceptance. |
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
