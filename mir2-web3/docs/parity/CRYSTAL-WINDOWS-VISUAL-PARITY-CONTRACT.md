# Crystal Windows visual parity contract

Status: active design and implementation contract. This document is not a
visual acceptance claim and does not declare the full-game denominator
complete.

## Bound revisions and claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
Crystal sourceRootClean: false
native implementation base: 67a55b37900ced07d66bd788cbe06ef429ede8aa
visual branch: codex/windows-visual-parity
selectedTargetCheckpointRevision: a58ab0aaa2202731a5c55e7a684261d6c15c2f8d
inventoryButtonACheckpointRevision: 5b70511316b084ac677b5978f7f03e440241ca4c
characterHudCheckpointRevision: 849f1f0b5120867d1358e0e7db9ba675e9866f9c
helpDialogCheckpointRevision: e22f2aa4c683447b0e57805a580fd29e0a84c37c
helpDialogMovableCheckpointRevision: 4545465a2e31a6646f247c55906764952d44cd58
healingCheckpointRevision: 24d9b73a30fc18edf0649283d14495c6f4900aff
inventoryLockedTabCheckpointRevision: 83f081149375fb402b9c7e6711fdb4e6bed68a0e
scarecrowDeathAudioCheckpointRevision: cf4f5b5197c492324be23beb73611c0e0162c403
scarecrowAttackAudioCheckpointRevision: e1dd6d6379d23efeafe57aa01c170452f1261b83
scarecrowStruckAudioCheckpointRevision: 354bb9f9648758c9f38d5ce149a273ae07cd2a7e
hallucinationCheckpointRevision: 60eae9561c5b18bc79456105e455d6964c14fafe
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
```

The dirty Crystal files are server files (`Server/MirEnvir/Envir.cs` and
`Server/MirObjects/PlayerObject.cs`). They are unrelated to the client audit,
but the source root is still not clean; final source binding therefore remains
fail-closed and must be regenerated against a clean source checkout before
acceptance.

## 2026-08-28 execution goal and screenshot-grounded status

The current native window is not a blank placeholder. It already renders a
playable Bichon baseline with terrain, actors, labels, minimap, orb HUD, chat
strip, quick bar and the right-side HUD cluster visible. That is still far
from visual 100% and does not close same-EXE or human acceptance.

The current source-audited denominator remains materially open:

- player pixel libraries: 477 libraries / 541,010 frames, with only 7 roots /
  7,360 frames currently closed in the native audit base;
- monster pixel libraries: 546 libraries / 219,607 frames, with only
  8 Monster libraries / 1,742 frames currently closed in the native audit
  base;
- non-None spells: 129, with only the first bounded effect/audio leaves
  automated;
- fixed/template UI scope: 410 leaves, with only selected shell/button/dialog
  leaves source-bound so far.

Execution continues in four bounded waves:

1. HUD/button UI wave: exact visible controls, images, press/hover/disabled
   semantics, sound, geometry and enable gates.
2. Player-character wave: body/hair/weapon/mount/corpse/name overlays across
   the real class/gender/equipment matrix.
3. Skill/effect wave: cast, projectile, impact, persistence and the actor
   struck/die/dead/revive chain for the first observable combat slice.
4. Monster wave: finish the remaining `Monster/005` action semantics, then
   expand to more families without generic fallback claims.

The first bounded write target after this goal sync is the visible `VIS-03`
main-HUD button matrix expansion. It is the smallest user-facing slice that
answers the current report about button/UI fidelity. Whole-game percentages,
same-EXE, live WSS, real DPI, native soak and human visual/feel remain
explicitly open.

## 2026-09-02 user-reported dialog baseline

The screenshots added on 2026-09-02 expose three source-bound families inside
the existing Character (54), Inventory (141) and Quest (95) registries. These
are subdivisions of the existing denominator, not permission to replace a
family with one screenshot row or reduce its count.

| Leaf | Crystal authority | Exact target | Candidate state / next gate |
|---|---|---|---|
| Character paper doll | `Client/MirScenes/Dialogs/CharacterDialog.cs:27-132,227-342,596-695`; `.Lib` offset draw at `Client/MirGraphics/MLibrary.cs:658-710` | `Title/504`, gender page `Prguse/340|341`, 14 cells, then wing -> resolved armour -> weapon -> helmet-or-hair with original frame offsets | Exact root/page/tabs/cells and armour -> weapon -> helmet-or-hair composition are implemented. Runtime/Gateway retain gender, hair, guild and `ItemInfo.Image`; exact `StateItem` metadata and gender/hair assets are packaged. Authoritative wing input, full secondary-tab content, trusted package evidence and human acceptance remain open. |
| Inventory footer/window | `Client/MirScenes/Dialogs/InventoryDialog.cs:25-209,384-427,483-598`; generic movement at `Client/MirControls/MirControl.cs:852-935`; icon rules at `MirItemCell.cs:2511-2551`; amount geometry at `MirAmountBox.cs`; confirmation at `MirMessageBox.cs` | `Title/196` `316x236` movable root with child-owned hits and stage clamp; `(40,212)` numeric Gold; `(182,217)` 84x6 weight bar with `Prguse/24`, `UI_32bit/471|470`; `(268,212)` free count; `(291,212)` delete art and exact DeleteItem flow | Movable placement/Hide-Show retention, footer, true-sized centred icons, stack labels, QuestInventory read-only behavior, DeleteMode toggle/cursor/right-click, `204x109` amount dialog, `456x190` single-item confirmation, exact native command and receipt correlation are implemented/tested. Live stacked-item amount evidence, locked/sealed overlays, full tooltip, trusted package/light evidence and human acceptance remain open. |
| Quest surface topology | `Client/MirScenes/Dialogs/QuestDialogs.cs:15-260,463-786,796-911,1743-1980`; confirmation at `Client/MirControls/MirMessageBox.cs` | NPC List=`Prguse/950`; Detail=`Prguse/960`; grouped Diary=`Prguse/961`; tracking is frameless. Diary left-click opens Detail and right-click toggles one of five tracked quests. CANCEL uses the source message box and requires YES before abandon. | Windows-native Diary, independent Detail, NPC Quest List, ordered five-slot right-click tracking and exact CANCEL confirmation are implemented and exercised against the real Gateway. Runtime `@quest:accept:` / `@quest:finish:` links retain exact current-NPC validation. Credit-reward/item-frame fidelity, generic NPC conversation layout, hover/pressed animation, tracker persistence and capture metadata remain open. |

The submitted comparisons were not state-aligned: Candidate `demo/Scout` was a
seeded Lv7 fixture with 1,280 Gold and starter panel data, while Crystal `1231`
was a Lv1 Wizard/Male with Gold 0, empty bag, one belt potion, WoodenSword,
BaseDress(M), one quest-inventory item and no magic. The redacted Crystal-state
extractor/import contract now retains hair, quest progress/completed IDs, quest
inventory and Crystal's six-slot belt offset, and the canonical visible state
has been imported into an isolated Candidate account/character slot. The live
Diary evidence below therefore uses `1231` and the authoritative completed
`Assistant's Request` state instead of the unrelated `demo/Scout` fixture.

The aligned Character checkpoint uses that same imported `1231` state. The
current-tree linked EXE (SHA-256
`996CF54AF6A2560EECFA03E87296EDFF797FE3B59C7400DCEB453B53A86C3656`)
produced
`docs/generated/player-qa/native-ui-parity-20260902-character/character-clean-character-1788359861663-1.png`
(SHA-256
`DED8B034522963FDDD68913F5A41B7F6E2DBF02FBCE7FCC3C7702BCB6062DE36`).
The renderer-owned sidecar freezes `panel=Character`, `BichonProvince` and
`(290,620)` only after name, map, max HP and self position are authoritative;
the QA reducer also closes the login notice before capture. It remains a draft
because authoritative light and trusted package provenance are absent. It is
not final same-EXE or human acceptance.

The aligned Inventory precision capture is
`docs/generated/player-qa/native-ui-parity-20260902-inventory/inventory-final-inventory-1788360618712-1.png`
(SHA-256
`28FCCB8F7E7C0061823C9C3F494F489E04A7C4A79BC2FF5CA60A981498F087D3`).
The DeleteMode auto-target then used a disposable real-Gateway character with
four authoritative ordinary bag items and produced
`docs/generated/player-qa/native-ui-parity-20260902-inventory-delete/inventory-delete-final-inventory-delete-1788362950757-1.png`
(SHA-256
`50F30B959365E5E2F4B2F6C1BA677118A683A4A7E91E47FEB4E5E6953F8761B1`)
from current-tree EXE SHA-256
`61F053C88FB4BCFF6C6BE0FB4A43C1AE8C807437986B3289D3A55536CC5EFF26`.
Its renderer sidecar records `panel=Inventory`, page 0, DeleteMode and the
single-item confirmation at Bichon `(288,616)`. YES was not pressed, so the QA
run is non-destructive. Exact stacked-item amount behavior is automated but
does not yet have a live authoritative stack capture. Both sidecars also lack
authoritative light and trusted build provenance; therefore they are bounded
implementation evidence and `visualAccepted=false` remains unchanged.

The subsequent movable-window slice uses original `Title/196.png` (`316x236`,
SHA-256
`987ACE9AA582868FF589DD923C64109E8D883549C9B80FE72ED7AFD981A0CB3B`)
and final current-tree r5 EXE SHA-256
`F08FC69F744BC9D6895A7756CF98AAF5A69EEEF4CA8AF10F65BA78D8663B33D3`.
One real Windows drag produced same-run/same-character/same-world captures
`docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-inventory-1788365024393-1.png`
(origin, SHA-256
`64F8A6A4ECB500F40C508ED804FDA686DEEF11A4FDAF507601680202064A5B5C`)
and
`docs/generated/player-qa/native-ui-parity-20260903-inventory-drag-final/inventory-drag-in-game-1788365096792-2.png`
(moved, SHA-256
`AED8F77D8F8F6A43E80F985A13748ECFD11BA26C33B462A406F01F6185C8CFE3`).
Their draft sidecars hold BichonProvince `(287,616)`, Bag1, and exact
`inventoryLocation=0.00,0.00` / `275.00,208.00`; only window placement
changes. Native-ui passes 496/496. Trusted package and light fields remain
absent, so this is movable-leaf implementation evidence and
`visualAccepted=false` remains unchanged.

The renderer-owned 1024x768 evidence is
`docs/generated/player-qa/native-ui-parity-20260902/quest-diary-in-game-1788348421931-1.png`
(SHA-256 `29ACF7E67FBA7726441556C1AD07B054D386DD99C09D6870C4D453BDF49B9C8F`).
Its sidecar is deliberately a draft because this current-tree Debug EXE has no
trusted package provenance and the capture does not carry an authoritative
lighting signature. It proves the bounded Diary implementation and aligned
quest state, not same-EXE release acceptance or whole Quest-family parity.

The subsequent source Detail slice is captured in
`docs/generated/player-qa/native-ui-parity-20260902/quest-detail-final-in-game-1788351341163-1.png`
(Diary + Detail, SHA-256
`2F733E4EEC558B2C9D52B2D09A906AC6B56889ECA01A450258BBE0480B452B59`) and
`docs/generated/player-qa/native-ui-parity-20260902/quest-detail-final-in-game-1788351355082-2.png`
(Detail after Diary close, SHA-256
`2B7F52B46F439F44B88AAB0F6A7C5700B244CC710FECBA7EB520B5DABAE00A01`).
Both were produced by the rebuilt current-tree Windows EXE against an isolated
import of Crystal character `1231`. Their sidecars bind BichonProvince
`(290,620)` and safely freeze `questDetail=Some(1)`; the second also proves
`panel=None`, so the independent-window behavior is machine-auditable. They
remain draft implementation evidence because authoritative light and trusted
package provenance are absent.

The subsequent Quest interaction slice uses a fresh isolated QA character so
quest 5 can traverse the real available -> current lifecycle. A current-tree
Windows EXE with SHA-256
`5A6D52FEB89949E23CA177D5A56FC24069D18CBD69C8BB6E2E7E55790BC2C099`
opened Blacksmith Smith's source-shaped NPC Quest List, accepted the runtime
`@quest:accept:5` link through the local Gateway, observed the marker change
from `!` to `?`, opened Diary and independent Detail, toggled the frameless
tracker with a real secondary click, and exercised CANCEL -> NO without
abandoning or untracking the quest. Evidence under
`docs/generated/player-qa/native-ui-parity-20260902/` is:

- `quest-ui-q2-in-game-1788354007393-1.png`: NPC Quest List visual,
  SHA-256 `F8401924BFED5AEF0764C287EEC30340D48EBD0063E0F32D0AE1213B96291679`;
- `quest-ui-q3-in-game-1788354708271-1.png`: Diary plus two-line tracker,
  SHA-256 `789449D4683A38DB272A1CB078279EF2172C4EBE114F420B3A762EC74D8DC4D9`;
- `quest-ui-q3-in-game-1788354757568-2.png`: exact CANCEL confirmation,
  SHA-256 `46FC9EDF6FB6A8706236900375B09B66FFB0421D6ADF6E2B2DBBB410C9BD68BE`;
- `quest-ui-q3-in-game-1788354839986-3.png`: state preserved after NO,
  SHA-256 `7D3EC23D6C17BC79BEEA595276C1D0E3C4A5AC8C5FD6D120CEDEA8EC6665F0B8`.

Quest UI tests pass 52/52 and the source export now contains 40,877 assets
with aggregate asset hash
`3d6b7f125a91121ccde5b9a2db5dfa4faba5e29be2f27bc4bcfc62a086ec45e4`.
The draft sidecars correctly remain ineligible, but currently encode neither
NPC Quest List nor tracker/message-box visibility; that provenance/state
coverage is an explicit open gate alongside generic NPC-dialog fidelity,
credit-reward/item-frame fidelity, trusted same-EXE comparison and human
acceptance.

The first Inventory capture is a deliberately non-acceptance draft at
`docs/generated/player-qa/windows-visual-parity/ui-gap-inventory-current-working-tree/`.
Its sidecar records `eligible=false`; the worktree is dirty and the compared
account/state is not aligned with Crystal, so it cannot satisfy the trusted
same-EXE or same-state gates.

## Hallucination bounded automated checkpoint

Revision `60eae9561c5b18bc79456105e455d6964c14fafe` closes one VIS-02
numerator leaf against Crystal `PlayerObject.cs`: spell 76 has no cast bitmap
or cast sound, waits for the 600ms Spell action, then owns a 16-direction
`Magic/1160` three-frame projectile driven by the 48ms process clock. A
present target receives `Magic2/1110..1119` and exact `M76-0.wav` unless its
completion-time rendered action is terminal `Dead`; a missing target never
receives an invented impact. Windows 421/421, exporters, audio, asset
preflight, Candidate self-tests and independent P0=0/P1=0/P2=0 review pass.

This revision was not launched or captured as an EXE. It closes neither the
129-spell denominator nor same-EXE/live-WSS, DPI, soak, human, clean-source or
publisher-signing gates; `globalParityPercent` remains null.

## Scarecrow struck-audio bounded automated checkpoint

Revision `354bb9f9648758c9f38d5ce149a273ae07cd2a7e` closes one exact
monster-audio action leaf. Crystal's `MonsterObject.Struck` calls
`PlayFlinchSound` before `PlayStruckSound`: Scarecrow resolves the first cue
as `BaseSound+2=52 -> 005-2.wav`, followed by the optional attacker-weapon
clang from the complete audited `60.wav` through `65.wav` grouping.

Native and Web preserve flinch-first order, including flinch-only behavior
when attacker context or the weapon image is unknown. Assassin equipped-weapon
handling follows Crystal's Short group. Native also covers lethal
flinch/clang/death order, ActionFeed tail deduplication, persistent
Remove/Hide actor termination, persistent map/logout scene termination and
connection-generation reset. A stale struck after a boundary in the same
batch remains fail-closed; actor reappearance or a proven scene change can
reopen the matching gate.

Focused native 3/3, Windows 406/406, Bevy native-ui 419/419, runtime 191/191,
Web 49 groups plus audio/export/typecheck and Candidate package/verifier
self-tests pass. Independent review is P0=0/P1=0/P2=0. Candidate scripts
require, copy and hash-bind `005-2.wav` and `60.wav` through `65.wav`, but no
exact-head package or EXE was built. Other Scarecrow/monster actions, other
monster families and the complete semantic denominator remain open, as do
same-EXE/live-WSS, device audio/GPU, real DPI, native soak, human acceptance,
legal asset review and signing. Global and visual acceptance remain
unreported.

The detailed evidence report is
`docs/generated/player-qa/windows-visual-parity/VIS-04-SCARECROW-STRUCK-AUDIO-REPORT.md`.

The broader execution-goal note for the current visual branch is
`docs/generated/player-qa/windows-visual-parity/VISUAL-GOAL-20260828.md`.

## Scarecrow Attack1-audio bounded automated checkpoint

Revision `e1dd6d6379d23efeafe57aa01c170452f1261b83` closes one exact
monster-audio action leaf. Crystal binds `Scarecrow=5`,
`BaseSound=BaseImage*10`, enters `MirAction.Attack1` with an immediate
`PlayAttackSound`, and resolves the default `BaseSound+1` numeric ID 51 as
`005-1.wav`. The tracked file is 90,118 bytes with SHA-256
`966E4163FC0000CF769B63C0F3379F1E9863645F43C1CCADEEE8066B73E6AE9A`.

Native enriches typed `ObjectAttack` with authoritative actor context and
requires exact Monster kind plus normalized `Monster/005`. Each distinct
Attack1 can emit the cue immediately; other actors and missing context fail
closed. Dead state suppresses attack audio, adjacent Remove/Hide cancels a
due-now cue, and map/logout/generation boundaries clear local sound state.
Web retains the source-derived `BaseImage*10+1` formula, and direct ID 51 is
present in both generated indices. Candidate package/verify require, copy and
hash-bind the exact file; the verifier removes it in self-test to prove the
required boundary.

Focused native 2/2 plus bridge 1/1, Windows 403/403, Bevy native-ui 419/419,
runtime 191/191, Web event/audio/export/typecheck and Candidate script tests
pass. Independent review is P0=0/P1=0. This does not implement `005-2`
flinch, the weapon-dependent struck cue/order, other monsters or the complete
monster-audio denominator. No exact-head package, EXE, same-EXE/live-WSS,
physical-audio, DPI, soak or human evidence was produced. Global and visual
acceptance remain unreported.

The detailed evidence report is
`docs/generated/player-qa/windows-visual-parity/VIS-04-SCARECROW-ATTACK-AUDIO-REPORT.md`.

## Scarecrow death-audio bounded automated checkpoint

Revision `cf4f5b5197c492324be23beb73611c0e0162c403` closes one exact
monster-audio leaf. Crystal binds `Scarecrow=5`, `BaseSound=BaseImage*10` and
`PlayDieSound=BaseSound+3`; `SoundManager` synthesizes unlisted numeric ID 53
as `005-3.wav`. The unrelated SoundList `10053 -> 53.wav` is not accepted as a
substitute. The tracked file is 198,168 bytes with SHA-256
`CF1FAF157B49D1E014E9B3A56367234FDCFD54088F93F04BB653CB27A67B9FF7`.

Native requires exact Monster kind plus normalized `Monster/005`, keys the
cue by authoritative object identity, suppresses replay and lets adjacent
Remove/Hide cancel before the due-now queue drains. Map change, logout and
generation reset clear it. Web emits Monster death at action start while
retaining PlayerObject's 100 ms frame-one delay, and direct sound ID 53 is
present in both generated indices. Candidate package/verify require, copy and
hash-bind the exact file; the verifier removes it in self-test to prove the
required boundary.

Focused 2/2, Windows 401/401, Bevy native-ui 419/419, runtime 191/191, Web
event/audio/export/typecheck and Candidate script tests pass. Independent
review is P0=0/P1=0. That revision alone did not implement `005-1` attack;
the later Attack1 checkpoint above closes it. `005-2` flinch, public struck
clang, movement/Dead/Revive cues, other monsters and the complete
monster-audio denominator remain open. No exact-head package, EXE,
same-EXE/live-WSS,
physical-audio, DPI, soak or human evidence was produced. Global and visual
acceptance remain unreported.

The detailed evidence report is
`docs/generated/player-qa/windows-visual-parity/VIS-04-SCARECROW-DEATH-AUDIO-REPORT.md`.

## Inventory locked-second-tab bounded automated checkpoint

Revision `83f081149375fb402b9c7e6711fdb4e6bed68a0e` closes one additional
VIS-03 renderer/model/package leaf. Crystal's unexpanded `User.Inventory`
length is 46: six belt cells plus forty first-page carried-item cells. In that
state native renders the second tab with exact `Title/169`. Clicking it emits
one local ButtonA cue but cannot select a phantom page or enqueue any Gateway
or gameplay intent. The first, second and quest tabs use the exact source state
assets `Title/197|737`, `Title/169|738|168` and `Title/198|739` over the
`Title/196` Inventory background.

The authoritative read model accepts only capacity values Crystal can create:
`46,54,58,62,66,70,74,78,82,86`. Crystal's first expansion adds eight cells
and each later expansion adds four. Missing and illegal values, including
`47,50,87,100,65535`, normalize to 46 instead of fabricating expansion.
Occupied item count/slot is not treated as purchase evidence. When authority
downgrades while page two is selected, native returns to page one and clears
inspect, pending inventory-operation and drop-confirmation state.

Candidate package and verifier scripts now require
`Title/168,169,196,197,198,737,738,739`; the verifier's missing-169 probe proves
the locked state fails closed. Focused model/tab/Gateway tests pass 5/5, the
full Bevy native-ui suite 419/419, Windows 399/399 and runtime 191/191. Package
and verifier self-tests pass, and independent final review reports P0=0/P1=0.

This is not complete Inventory parity. Production world snapshots do not yet
emit authoritative `inventoryCapacity`, so expanded-empty accounts remain
locked. Crystal's locked-tab `ExtraSlots8` dialog, `@ADDINVENTORY` request,
expanded-page `Prguse2/307` lock bars, partial open-level states and
`Title/483..485` AddButton are not implemented or claimed. No exact-head
Candidate, package, EXE, launch, live-WSS transcript, GPU/audio capture or human
evidence was produced. Same-EXE pixels/audio, real 100/125/150% DPI, native
30-minute soak, human visual/feel, clean-source/complete-denominator closure,
legal asset packaging and formal publisher signing remain required. VIS-03
stays in progress and `globalParityPercent` remains null.

The detailed evidence report is
`docs/generated/player-qa/windows-visual-parity/VIS-03-INVENTORY-LOCKED-TAB-REPORT.md`.

## Healing bounded automated checkpoint

Revision `24d9b73a30fc18edf0649283d14495c6f4900aff` closes automated
presentation evidence for one additional VIS-02 spell leaf. A typed
`ObjectMagic(Healing)` starts the caster-owned `Magic/200..209` sequence:
ten 60 ms frames, 600 ms total, light 6 and exact `M61-0.wav`. The cast sound
is emitted only for an active resolved cast and is one-shot across native
sequence replay.

A typed raw `ObjectEffect` value 3 starts `Magic/370..379`: ten 80 ms frames,
800 ms total, light 6 and exact `M61-1.wav`. Crystal handles this packet
outside the generic delayed-effect branch, so Healing deliberately ignores
the packet delay. The effect is anchored to the authoritative target object,
follows it while alive, disappears on Hide/Remove and does not resolve or play
audio when the target is absent. This implementation also fixes generic
native object-effect anchoring rather than leaving the Healing target at its
initial screen tile.

Web projection now carries the Healing magic effect through its typed game
event, resolves either `"Healing"` or numeric 61 to the exact audio IDs and
keeps the target effect attached to the live actor. The Web event currently
does not carry a sequence/generation identity, so explicit retransmit or
reconnect deduplication remains a reviewed non-blocking P2 and is outside this
bounded claim. The native sequence-replay boundary is covered.

Exact source audio identities are:

- `M61-0.wav`: 194,008 bytes, SHA-256
  `AADE9DB9A46762B8C319A2FD3611FBB4CDC86D444B5C3FD14DC92AEC812F94A1`;
- `M61-1.wav`: 308,496 bytes, SHA-256
  `9E3942A729F886197B30D1CA0084AA020179F62BCA64C6044E36D6E080D74ED5`.

Sound and magic exporters, the Web present manifest, Bevy gameplay allowlist,
package script and copied-Candidate verifier require both sounds and all
`Magic/200..209` plus `Magic/370..379` frames. The verifier self-tests remove
each sound and both ends of the image ranges and prove fail-closed behavior.
Focused Healing 4/4, Windows 398/398, Bevy native-ui 416/416, Web game-event,
sound-export, magic-export, scene-runtime and type checks, Rust formatting and
both Candidate script self-tests pass. Independent final review is P0=0/P1=0.

This checkpoint certifies packet-to-presentation behavior only. It does not
alter or certify server healing amount, mana/cooldown, target eligibility,
Zone authority or live authenticated delivery. No exact-head Candidate,
package, EXE, client launch, live WSS transcript, GPU capture or human evidence
was produced. Same-EXE pixels/audio, real 100/125/150% DPI, native 30-minute
soak, human visual/feel review, clean-source/complete-denominator closure,
legal asset packaging and formal publisher signing remain required. The
inventory counts below do not change, VIS-02 stays in progress and
`globalParityPercent` remains null.

The detailed evidence report is
`docs/generated/player-qa/windows-visual-parity/VIS-02-HEALING-REPORT.md`.

## HelpDialog bounded default-English checkpoint

Revision `e22f2aa4c683447b0e57805a580fd29e0a84c37c` closes automated evidence
for one VIS-03 window leaf. It uses Crystal's 536x509 `Prguse/920` frame,
`Title/57`, `Prguse2/240..245` Previous/Next triples and
`Prguse2/360..362` Close triple at their source coordinates. Pages zero
through two render the source English shortcut catalog; pages three through
44 map exactly to `Help/0..41`, with the source 45 titles and circular
Previous/Next behavior.

Help has renderer-owned visibility and page state separate from the core panel
reducer. Menu Help and the default H shortcut toggle it without sound or
Gateway work. H requires Ctrl and Shift unpressed while Alt is irrelevant;
focused text input owns H. Hide preserves the current page, session reset
returns to page zero, and Escape closes Help together with other windows.
Previous, Next and Close each enqueue one typed ButtonA. The displayed Crystal
default P shortcut now opens Group and never Storage, including while Help is
visible. `MENU.HELP` is a typed Overlay action in the 174-entry control
registry; the deliberately disabled source-menu family has been reduced from
nine entries to eight.

All 42 Help page PNGs plus the frame and title are source-exported and present
in the remote/Candidate closure. Candidate scripts allowlist, require and copy
the Help tree and fail closed when `Help/41.png`, `Prguse/920.png` or
`Title/57.png` is missing. Focused Help 9/9, Bevy native-ui 411/411, Windows
394/394, ui-core registry 13/13, rustfmt/diff and both script self-tests pass.
Independent review reports P0=0 and retains one P1 outside this bounded claim:
Crystal reads current keybindings and localized strings, while native Help is
still hard-coded to default English/default bindings.

No exact-head Candidate, EXE, package, live WSS run, GPU capture or human
evidence was produced at that implementation revision. Follow-on revision
`4545465a2e31a6646f247c55906764952d44cd58` closes the automated movable/
`Sort=true` geometry: it preserves the grab offset, uses the shared stage
transform, clamps all four boundaries with Crystal's right/bottom `-1`, keeps
title/content/Close outside the blank-header drag surface, clears movement on
release/focus loss/Hide/headless absence/session reset, and raises Help above
peer dialogs without crossing Death or Menu. Focused Help 14/14, Bevy native-
ui 416/416 and Windows 394/394 pass; independent review is P0=0/P1=0 for this
follow-on leaf. Exact bold/font raster is still unaccepted. Dynamic rebind/
localization, 100/125/150% real-DPI interaction, same-EXE pixels/audio, native
30-minute soak, human visual/interaction acceptance and publisher signing all
remain required. This is not HelpDialog visual acceptance, VIS-03 completion
or whole-game UI parity.

The machine-readable companion is
`docs/generated/player-qa/windows-visual-parity/phase-a-denominator.json`.
Its counts are known source-backed scope registries, not a full-game
percentage. `node scripts/verify-windows-visual-parity-ledger.mjs` checks its
internal counting and fail-closed claim invariants; it is not a substitute for
complete source extraction.

## Character HUD bounded checkpoint

Revision `849f1f0b5120867d1358e0e7db9ba675e9866f9c` closes automated evidence
for one VIS-03 HUD control. The native Character button uses Crystal's exact
`Prguse/1900`, `1901` and `1902` normal/hover/pressed images in the 20x20
control at logical `(905,692)`. There is no dedicated disabled image; a
disabled image-button state is not a valid click and falls back to the normal
visual through the shared button contract.

An enabled pointer transition to Pressed queues typed `ButtonA` exactly once
before invoking the callback. Held Pressed cannot repeat and no outbound UI or
gameplay intent is produced. The callback matches Crystal's page-aware rule:
a closed dialog opens on CharacterPage; Stats1, Stats2 or Spells remains open
and switches to CharacterPage; an already visible CharacterPage closes.
Default C and F10 use the same state transition but bypass the pointer sound
producer and remain silent.

Bevy native-ui 401/401, Windows 376/376, focused Character 4/4, Candidate
package/verifier self-tests, rustfmt and diff checks pass. Independent review
initially identified the missing F10/page-aware keyboard route; after
remediation its final result is P0=0/P1=0. The prior Inventory checkpoint has
already closed the exact `103.wav` source/package identity used here, so this
revision adds no asset or package rule. No Candidate, EXE, live audio, GPU
capture or human evidence was created. Character panel contents, every other
control, same-EXE/live WSS, 100/125/150% DPI hit testing, native 30-minute
soak, human visual/audio/feel and publisher signing remain required.

## Inventory ButtonA bounded checkpoint

Revision `5b70511316b084ac677b5978f7f03e440241ca4c` closes automated evidence
for one VIS-03 interaction/audio rule. Crystal defines `ButtonA=10103`, maps
it to `103.wav`, and plays it once on an enabled `MirControl.OnMouseClick`
before invoking the click callback. The Windows Inventory HUD now queues that
typed local sound exactly once on a changed pointer transition to Pressed,
immediately before toggling the panel. Holding Pressed cannot repeat it;
releasing and pressing again can. The independent F9/I state toggle never
enters the pointer producer and remains silent.

The bounded UI queue and its spawned-player marker are separate from packet-
authoritative gameplay audio, including simultaneous same-frame playback.
Missing `103.wav`, disabled sound and zero volume discard the pending cue
without selecting a fallback. Package and verifier scripts allowlist,
require, copy and identity-bind the existing source file at 26,546 bytes and
SHA-256
`7A55D27DEA18F70EB4FF4F324B682EFAB4996406EFAE3E94467D3C39CCCC674A`;
their self-tests reject missing, wrong-size and wrong-hash probes.

Windows 376/376, Bevy native-ui 397/397, focused ButtonA/audio 4/4,
package/verifier self-tests, rustfmt and diff checks pass. Independent final
review reports P0=0/P1=0 after separating UI and gameplay player lifecycles.
No Candidate, EXE, live audio device, GPU capture or human evidence was
created. This is not completion of the Inventory panel, main HUD, VIS-03 or
UI parity. All other controls, same-EXE/live WSS, 100/125/150% DPI, native
30-minute soak, human visual/audio/feel and publisher signing remain required.

## Selected-target bounded checkpoint

Revision `a58ab0aaa2202731a5c55e7a684261d6c15c2f8d` closes automated evidence
for one VIS-01 rendering rule: an explicitly selected remote player or monster
is redrawn as its complete resolved exact-atlas composite at opacity 0.3 after
the world pass. Selection accepts the Gateway's numeric or string identity,
has no Crystal death restriction for monsters, and fails closed for self,
NPCs, missing type or any rendered layer without exact atlas identity. Stable
clone keys make selection replacement, removal and unknown targets remove the
old redraw without retained geometry.

The implementation reserves three non-overlapping depth bands. Normal map and
actors remain in the world band; the selected composite is next; Crystal
foreground scene/actor effects are last. ObjectEffect and default-foreground
MapEffect are explicitly post-world, as are Cast, AttackOverlay, Projectile
and Impact. Persistent ObjectSpell remains in-world. This is only the currently
modelled subset of Crystal's `Effect.DrawBehind` contract, not a complete
classification of every effect producer.

Windows 376/376, Bevy native-ui 393/393, shared runtime 191/191, focused
selected 3/3 and foreground-depth 1/1 pass, and independent review reports
P0=0/P1=0. No Candidate, EXE, GPU capture or human evidence was created from
this revision, so the rule is automated-only and `visualAccepted` remains
false. Hover `MouseObject`, HighlightTarget option wiring, transparent-pixel
hit testing, special wing/behind/internal-blend composites, Web symmetry,
same-EXE/live WSS, 100/125/150% DPI, native 30-minute soak, human visual/feel
and publisher signing remain required.

## Counting rule

One source-visible control slot, stable fixed-array element, action record,
direction/phase record, asset library or explicit rendering rule is one leaf
in the corresponding registry. A button's normal, hover, pressed and disabled
states are required gates of one button leaf; they are not four denominator
leaves. Dynamic lists use one stable template leaf plus `instanceBound`.

A leaf passes only when all required gates are present:

- exact source identity and source line;
- asset/library/frame identity and hash where pixels are involved;
- geometry, anchor, layer order, blend/opacity and clock semantics;
- packet or local-state trigger;
- automated contract/render evidence;
- exact-head Windows same-EXE evidence where the leaf is visible;
- required DPI and human visual/feel evidence.

`UNKNOWN`, `BLOCKED`, `FAIL`, missing evidence and required gates marked `N/A`
all count as zero. No aggregate percentage is reported until the inventory for
that aggregate is closed.

## Known Phase-A registries

### HUD, buttons and main panels

The current fixed/template UI scope contains 410 leaves:

| Family | Leaves | Source authority | Current state |
|---|---:|---|---|
| Main HUD | 28 | `MainDialogs.cs:13-381` | partial implementation, unaccepted |
| Chat | 8 | `MainDialogs.cs:563-1254` | partial implementation, unaccepted |
| Chat control | 12 | `MainDialogs.cs:1255-1512` | partial implementation, unaccepted |
| Skill bar | 28 | `MainDialogs.cs:1513-1763` | partial implementation, unaccepted |
| Minimap | 22 | `MainDialogs.cs:1764-2112` | partial implementation, unaccepted |
| Inventory | 141 | `InventoryDialog.cs:10-209` | 40-slot QuestGrid and other leaves open |
| Character | 54 | `CharacterDialog.cs:8-342` | partial shell; content/typography open |
| Quest family | 95 | `QuestDialogs.cs:15-1600` | Diary/Detail/NPC List, exact five-slot tracking and CANCEL confirmation landed; remaining content/animation/provenance leaves open |
| Big map | 22 | `BigMapDialog.cs:12-590,800+` | partial implementation, unaccepted |

Crystal initializes 14 equipment cells in `CharacterDialog.cs:227-342`, not
15. Older parity text using 15 is corrected by this change.

### Player and monster rendering

| Registry | Source denominator | Current native coverage | Claim state |
|---|---:|---:|---|
| Player pixel libraries | 477 libraries / 541,010 frames | 7 roots / 7,360 frames | open |
| Monster-family pixel libraries | 546 libraries / 219,607 frames | 8 Monster libraries / 1,742 frames | open |
| Player action records | 33 | 17 after adding Skeleton plus Show/Hide to the shared vocabulary; only 14 apply to players | open |
| Player body direction/phase | 1,384 | 560 expressible at the audit base | open |
| Player effect/wing direction/phase | 1,240 | 0 | open |
| Explicit monster action records | 3,332 across 455 libraries | 3,205 expressible at the audit base | open |
| Explicit monster direction/phase | 153,416 | 147,208 expressible at the audit base | open |
| Monster libraries without explicit contracts | 91 | unresolved fallback audit | open |
| Visual rendering rules (`VIS-RULE-v1`) | 32 | inventory established; verification open | open |

The starter atlas' included PNG pixels and `MImage.X/Y` anchors are reliable.
No fake ellipse shadow may be added: Crystal's monster/player PNGs already
contain the shadow pixels and the current 10,482 atlas rects have zero
`shadowX/Y`.

### Skills, combat effects and environment

These are source registries, not a single closed semantic denominator:

| Registry | Source count | Native audit-base coverage |
|---|---:|---:|
| Non-None spells | 129 | routing skeleton exists; visual closure open |
| Non-None `SpellEffect` values | 34 | 11 manifest entries |
| Unique `SpellObject` branches | 29 | 7 corresponding branches among 13 ground-manifest entries |
| Map event spells | 19 | 0; the 2 map-manifest entries are `SpellEffect.Mine`/`Tester`, not map-event spells |
| Non-None poison types | 11 | no complete status renderer |
| Buff types | 59; 17 world-observable branches | no complete world overlay |
| `MirAction` values | 45 | 17 shared runtime actions after Show/Hide; full action parity remains open |
| Weather flags | 10 | missing |
| Light settings | 5 plus darkness/blindness paths | blindness missing |

The first combat-effect slice is FlamingSword, FireBall, Lightning,
SoulFireBall and FireWall, followed by PoisonCloud. It must include cast,
projectile/target tracking, impact/persistence, sound and the actor
Struck/Die/Dead/Revive chain. Source-routed assets without same-EXE playback do
not pass the slice.

Lightning is the first bounded automated checkpoint inside that slice. At
revision `53483ccf4`, `cast=true` waits for the 600 ms Spell-action completion,
then attaches six 100 ms `Magic` frames at `970 + direction*20` to the caster
and emits the exact allowlisted `M40-0.wav` once. `cast=false` emits neither;
no projectile or impact is fabricated. The fixed fixture closes typed packet,
state-clock, frame/audio identity and lifecycle automation only. It does not
pass the same-EXE, live-WSS, GPU-raster or human-audio gates.

FireBall is the second bounded automated checkpoint at revision
`d85d7368119053e6b2609316c4f5c76faaa298cb`. Typed `ObjectMagic` owns its
immediate `Magic/0..9` cast, 600 ms actor-action boundary and local missile;
the adjacent simulation compatibility `ObjectProjectile` is deduplicated.
The missile locks Crystal Direction16 at launch, uses all 16 ranges
`10 + direction*10 .. +5`, tracks the bound destination with a finite
`MaxDistance*50 ms` movement clock, and promotes only a bound target to
`Magic/170..179` impact. M31-0/1/2 have exact byte/hash closure. Frame cycling
does not extend projectile lifetime. This passes packet, clock, frame/audio,
asset, package and verifier automation only. The explicit
`Target.CurrentAction == Dead` impact suppression branch remains open until
dead state reaches the effect input. FlamingSword, SoulFireBall and FireWall
remain open, as do every same-EXE/live-WSS/GPU/DPI/human gate.

SoulFireBall is the third bounded automated checkpoint at revision
`19991af6ddb289dc2fb22569849599caabf9195e`. `ObjectMagic` immediately emits
M64-0 with no cast bitmap, then a successful cast launches the local missile at
the 600 ms Spell-action boundary. At launch, a live target supplies the locked
Direction16 and bound destination; the three frames are
`1160 + direction*10 .. +2`, flight is finite at `distance*50 ms`, and only a
bound completion promotes to `Magic/1360..1369` plus M64-2. M64-0/1/2 have
exact byte/hash closure. The Rust compatibility `ObjectProjectile` is ignored
in all replay orders. The Gateway fixture is explicitly a
`server_packet_to_event` projection contract, not proof of the currently
absent production no-amulet `cast=false` route. Target-dead impact suppression,
post-launch removal fidelity and shared-Zone timing/revalidation/PvP gaps
remain open. This passes packet projection, clock, frame/audio, asset, package
and verifier automation only; FlamingSword, FireWall and every same-EXE/live-
WSS/GPU/DPI/human gate remain open.

FireWall is the fourth bounded automated checkpoint at revision
`f6f78f3eddb813897cf4ce4c6056183130ab7f35`. Typed `ObjectMagic` starts the
600 ms `Magic/1620..1629` caster action and exact M39-0; successful `cast=true`
queues M39-1 at action completion. Five all-valid center/cardinal
`ObjectSpell` projections use repeating `Magic/1630..1635`, light 3 and remain
until authoritative removal. Exact M39 byte/hash identities and required
source/package paths are fail-closed. The Gateway fixture proves typed
projection only, not authenticated wall-clock delivery; its `cast=false`
compatibility case is labeled synthetic outside the canonical timeline. This
passes packet projection, clock, frame/audio, source asset and package/
verifier self-test automation only. No exact-head package was produced.
FlamingSword, the complete backend negative/lifecycle matrix and every same-
EXE/live-WSS/GPU/DPI/human gate remain open.

FlamingSword is the fifth bounded automated checkpoint at revision
`160e8d3ccc0eb17f8e49b6505c5a58666a35029f`. `SpellToggle` is presentation-
silent; only typed `ObjectAttack(spell=8)` starts the Attack1-bound overlay.
The live attacker owns six 100 ms frames for each of eight directions at
`Magic/3480 + direction*10`, with additive opacity 0.7, no light and no
generated shadow. Exact M8-1 starts at time zero and the generic weapon swing
remains on frame 1 at 100 ms; actor/map/session lifecycle cancels pending work.
Ordinary attacks do not create the overlay or dedicated sound. The Gateway
fixture proves typed projection, not production reachability or authenticated
timing. This passes packet projection, state-clock, frame/audio, Web/native
consumer, source asset and package/verifier self-test automation only. All five
initial presentation checkpoints are now bounded, but VIS-02 still requires
the Struck/Die/Dead/Revive chain plus every same-EXE/live-WSS/GPU/DPI/human
gate, and the full semantic inventory remains incomplete.

GreatFireBall is an additional bounded automated checkpoint after the initial
five at revision `9457e5618449d22350baedd01e3775f5b1fe59c6`. Typed
`ObjectMagic` starts `Magic/400..409` and exact M34-0 immediately. Successful
cast completion launches the client-owned missile at 600 ms with six frames
from `410 + direction*10` for all sixteen Crystal directions and exact M34-1;
only a still-bound target promotes to `Magic/570..579` plus M34-2. The Rust
compatibility `ObjectProjectile` is ignored to prevent a duplicate. Target
removal and map/session lifecycle cancel retained impact/audio. The source
export now tracks all 90 previously absent direction PNGs plus their metadata,
and package/verifier require all 116 cast/projectile/impact frames and exact
M34 byte/hash identities. The fixture proves typed projection only and labels
`cast=false` as compatibility-only. A target that remains in AOI while already
Dead still lacks an explicit dead bit at the effect boundary, so Crystal's
dead-target impact suppression remains open. This checkpoint supplies no
exact-head package, authenticated live-WSS timing, same-EXE pixels, DPI, soak
or human acceptance and does not change VIS-02 or global completion state.

VIS-03 has one bounded automated checkpoint at implementation revision
`448db4f72`. The 1024x768 HUD base and Inventory control are source-bound to
`Prguse/1` and normal/hover/pressed `Prguse/1903..1905`. BigMap Teleport keeps
`Title/821/822/823` for normal/hover/pressed and now explicitly uses
`Title/823` while disabled. Its enable gate also requires the active target
map to equal the authoritative current map, matching Crystal's
`TargetMapIndex == map.Index` rule. Buttons without explicit disabled art
continue to render their normal frame. This passes render-state, input-gate,
asset-closure and package/verifier automation only; no same-EXE capture, GPU
raster, real-DPI or human acceptance is implied.

VIS-01 living hover-name revision
`066f6f3b576cbdc03106c8a221ccdaf13f7dfa83` separates three Crystal paths:
ordinary living `NameView`, non-self `MouseObject` / self MouseOver names, and
the living self health bar. `HighlightTarget` gates only actor redraws;
selected-only objects do not gain names; self and overlapping non-self hover
may both draw one name; and health remains stable across name visibility.
Windows 394/394 and independent P0=0/P1=0 review pass. Corpse TargetDead,
DisplayBodyName, guild dual-line layout, NPC line-color split, special monster
offsets and all same-EXE/live-WSS/GPU/DPI/soak/human/signing gates stay open.

## Delivery waves

1. `VIS-00` routes native text through Arial, applies the 8pt-at-96-DPI
   logical default to chat/nameplates, and closes obvious actor-state
   corruption: exact four-pass MirLabel outline, normal/transform remote body
   routing, Harvest/Skeleton packet actions, Harvest `CWeapon/01` routing, ordinary
   NameView alive-only labels, Hidden 0.5 opacity and ordinary corpse opacity
   1.0. HUD point-size normalization, damage-text bold/size, hover-only corpse
   names, weapon/wing additive layers and same-EXE raster evidence stay open.
2. `VIS-01` builds the fixed Bichon actor scene: male Warrior self, female
   remote player, Hen, Deer, Scarecrow and CannibalPlant in live, combat,
   harvest and occlusion phases.
3. `VIS-02` builds the first five-skill combat/effect slice with deterministic
   clock, packet fixtures, effect/audio traces and same-EXE capture.
4. `VIS-03` closes the first UI state slice at 1024x768: normal HUD, Inventory
   hover, Inventory pressed and BigMap Teleport explicit disabled state.
5. `VIS-04` begins the source-derived monster-audio registry. Its first bounded
   leaves are Scarecrow death `005-3.wav` and Attack1 `005-1.wav`; flinch,
   struck ordering, movement and other monster families remain open.
6. Subsequent waves expand the source-derived actor, monster, spell,
   environment and UI registries. The denominator may grow; existing leaf IDs
   and failures may not be silently removed.

The current VIS-01 source/test checkpoint is bound in
`docs/generated/player-qa/windows-visual-parity/VIS-01-REPORT.md`. It closes
CannibalPlant's `Monster/010` Show/Hide clock and native packet lifecycle plus
Scarecrow's `Monster/005` Die-phase `224..233` additive source path. The latter
shares the real map producer's six-cell guard-band/front-depth contract, obeys
the Effect option without another packet, and has ECS material/cache/reset
coverage. Commit `ef619b551` also closes the automated fixed-scene transcript:
17 exact typed events drive six actors through 15 exact render checkpoints and
one damage checkpoint, checking production frame-set hashes, exact layers,
Candidate atlas routes, death transforms and a real `0.map` front-tile binding
and geometry intersection. Real Gateway/WSS ordering, opaque-pixel and blend
raster evidence, same-EXE capture and visual acceptance remain open;
source/render-state tests are not raster acceptance.

Review follow-up `434bb06e6` preserves raw-snapshot relationship authority over
retained packet overlays and makes every schema-v2 entity-atlas page fail closed
on missing content, byte/hash mismatch, PNG decode failure or wrong dimensions.
That page closure is shared by runtime loading, the VIS-01 production test,
source packaging and copied-Candidate verification.

The bounded Lightning evidence is recorded in
`docs/generated/player-qa/windows-visual-parity/VIS-02-LIGHTNING-REPORT.md`.
The bounded FireBall, SoulFireBall and FireWall evidence is recorded in
`docs/generated/player-qa/windows-visual-parity/VIS-02-FIREBALL-REPORT.md` and
`docs/generated/player-qa/windows-visual-parity/VIS-02-SOUL-FIREBALL-REPORT.md`
and
`docs/generated/player-qa/windows-visual-parity/VIS-02-FIREWALL-REPORT.md`.
The Windows functional gate also generates the native keyed/additive map pack
before its host tests; this keeps VIS-01's real `0.map` front-cell binding
fail-closed on clean runners rather than weakening the visual assertion.

The bounded VIS-03 evidence is recorded in
`docs/generated/player-qa/windows-visual-parity/VIS-03-BUTTON-STATE-REPORT.md`.
It closes only the listed source-bound button-state and same-map intent checks;
the wider HUD, Inventory and BigMap denominators remain incomplete and
unaccepted.

## Evidence and final gates

Every same-EXE capture records source revision, implementation revision,
package/EXE hash, asset manifest hashes, client-area size, DPI, input state,
map/coordinates and deterministic clock/seed. Automated ROI comparisons mask
only declared dynamic world regions.

The following remain external/human gates and cannot be converted into a code
percentage: clean Crystal source binding, 100/125/150% real DPI, full same-EXE
UI and live WSS, 30-minute native soak, human visual/animation/audio/feel
acceptance, complete legal asset pack, and formal publisher signing. Until
they close,
`visualAccepted=false` and no strict visual 100% statement is valid.
