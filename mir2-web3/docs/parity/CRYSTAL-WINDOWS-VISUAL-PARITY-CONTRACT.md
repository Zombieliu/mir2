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
