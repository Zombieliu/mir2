# Windows visual parity VIS-00 report

Date: 2026-08-27

## Claim state

```text
Crystal source revision: 484983404e3d6afa584e93801f8006ae3429bea9
Crystal sourceRootClean: false
implementation base: 67a55b37900ced07d66bd788cbe06ef429ede8aa
implementation revision: 76f61ddabadd976f36c360fe2a942d4e67426dff
branch: codex/windows-visual-parity
semanticLeafInventoryComplete: false
inventoryComplete: false
globalParityPercent: null
accepted: false
visualAccepted: false
sameExeCaptureProduced: false
```

This report covers the bounded VIS-00 implementation/code gate. It does not
claim Crystal visual acceptance, full-game parity, or a visual percentage. The
playable frozen Candidate process was not stopped or replaced and does not
contain these changes.

## Denominator checkpoint

The source-backed contract is
`docs/parity/CRYSTAL-WINDOWS-VISUAL-PARITY-CONTRACT.md`; its machine-readable
companion is `phase-a-denominator.json` beside this report. Known registries
include 410 Phase-A fixed/template UI leaves, player and monster libraries and
action/direction-phase records, 32 explicit rendering rules, 129 non-None
spells, 34 `SpellEffect` values, 29 `SpellObject` branches, 19 map-event
spells, 11 poison types, 59 buffs, 10 weather flags and 5 light settings. The
13 ground-manifest entries correspond to only 7 of the 29 `SpellObject`
branches. Current map-event visual coverage is 0/19; the two map-manifest
entries are `SpellEffect.Mine` and `SpellEffect.Tester`, a different registry.

These are known registries, not a complete whole-game denominator. Unknown,
blocked or unverified leaves pass as zero, and no aggregate is emitted while
the relevant inventory remains incomplete.

## Implemented VIS-00 slice

- Shared native text now routes through Arial. Chat and ordinary entity labels
  use Crystal's 8pt-at-96-DPI logical default; HUD pixel sizes and damage-text
  bold/size remain open.
- Entity labels use the source MirLabel four-pass black outline geometry before
  the foreground pass.
- The ordinary NameView pass no longer displays dead entity names. Crystal's
  separate hover-only corpse-name path remains open.
- Hidden entities use alpha 0.5; ordinary corpses are no longer faded to 0.45.
- Remote `ObjectPlayerInfo` class, gender, guild, hair, armour, weapon,
  mount/fishing and normal/Transform body routes survive Gateway projection and
  native ingestion. Weapon-effect/wing additive rendering remains open.
- `ObjectHarvest` and `ObjectHarvested` drive Harvest and persistent Skeleton
  actions. Harvest routes an untransformed player, including empty hand, to
  `CWeapon/01`;
  Skeleton can transition to Revive.

## Automated evidence

| Gate | Result |
|---|---|
| `mir2-client-runtime --lib` | PASS, 185/185 |
| `mir2-client-bevy --features native-ui`, typography filter | PASS, 1/1 |
| Gateway remote layered appearance filter | PASS, 1/1 |
| Windows six new focused regressions | PASS, 6/6 |
| Phase-A ledger integrity verifier | PASS |
| Windows full suite using frozen Candidate assets | FAIL, 316/318 passed |

Exact reproducible commands use Rust `+1.95.0` for Bevy 0.19 client crates and
`+1.89.0` for the workspace Gateway:

```powershell
cargo +1.95.0 test --manifest-path apps/game-client/runtime/Cargo.toml --lib
cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui crystal_default_matches_source_arial_eight_point_at_96_dpi
cargo +1.89.0 test -p mir2-gateway object_player_info_preserves_authoritative_layered_appearance
$env:MIR2_NATIVE_ASSET_ROOT='<frozen-candidate>/mir2-assets'
cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml
node scripts/verify-windows-visual-parity-ledger.mjs
```

The two full-suite failures are asset-closure failures, not accepted skips:

- Archer walk expects `ARArmour/00/24.png`; the frozen Candidate asset root
  does not contain it.
- Mounted player expects `Mount/00/32.png`; the frozen Candidate asset root
  does not contain it.

Both remain failures until a complete, legal developer asset pack is staged
and the same tests pass. They are not converted to `N/A` and are not hidden
from a percentage. The frozen pack also lacks the newly routed `CWeapon/01`,
`Transform/00` and `TransformRide2/04` families; those string-routing
regressions pass, but their pixels have no same-EXE visual evidence and
therefore remain unaccepted.

## Open gates and next slices

`VIS-01` must produce a fixed Bichon actor/monster scene with live, combat,
harvest and occlusion phases. `VIS-02` must implement and capture
FlamingSword, FireBall, Lightning, SoulFireBall and FireWall, including actor
Struck/Die/Dead/Revive, projectile/target timing, impact or persistence, and
audio. `VIS-03` must cover HUD normal, Inventory hover/pressed and BigMap
Teleport disabled states.

The following stay open: clean Crystal source binding, complete legal
actor/effect assets, additive weapon/wing layers, same-EXE UI and authenticated
live-WSS execution, real 100/125/150% DPI, 30-minute native soak, human
visual/animation/audio/feel acceptance, complete semantic inventory and formal
publisher signing.
