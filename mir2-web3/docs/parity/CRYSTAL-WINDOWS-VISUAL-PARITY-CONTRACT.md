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

The machine-readable companion is
`docs/generated/player-qa/windows-visual-parity/phase-a-denominator.json`.
Its counts are known source-backed scope registries, not a full-game
percentage. `node scripts/verify-windows-visual-parity-ledger.mjs` checks its
internal counting and fail-closed claim invariants; it is not a substitute for
complete source extraction.

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
| Quest family | 95 | `QuestDialogs.cs:15-1600` | source four-dialog structure open |
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
contain the shadow pixels and the current 9,650 atlas rects have zero
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
5. Subsequent waves expand the source-derived actor, monster, spell,
   environment and UI registries. The denominator may grow; existing leaf IDs
   and failures may not be silently removed.

The current VIS-01 source/test checkpoint is bound in
`docs/generated/player-qa/windows-visual-parity/VIS-01-REPORT.md`. It closes
only CannibalPlant's `Monster/010` Show/Hide clock and native packet lifecycle.
The fixed scene, Scarecrow additive death, real-map occlusion, same-EXE capture
and visual acceptance remain open.

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
