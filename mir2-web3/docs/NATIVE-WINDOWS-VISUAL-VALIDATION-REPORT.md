# Windows Native Visual Validation Report

Status: **Visual Validation In Progress — not Accepted**
Run: `20260822-current-source-live`
Logical stage: `1024x768`
Reference: Crystal debug client and current-source native Windows client, both on Bichon Province at `288,616`

## Outcome first

The current Windows client is no longer correctly described as having only P2 visual debt. Login and character selection are close to the Crystal reference, and the HUD frame is recognizable, but the in-game scene and most secondary panels still have release-blocking visual defects.

This run found and fixed two code-path blockers in the working tree:

1. `update_hud_minimap_visibility` used three overlapping mutable `Node` queries and caused Bevy `B0001` during current-source startup. It now uses one `ParamSet`, with a regression test.
2. native `MapRenderState.standaloneTiles[].imageUrl` values were never loaded. The atomic map handoff therefore waited forever and rendered a black world. URL-backed standalone images now enter the same AssetServer preload path as atlas pages, with diagnostics and regression coverage.

Two additional live defects were fixed after the first comparison:

3. Escape no longer closes Quest and opens Menu in the same update; the overlay keyboard defers Quest/NPC Escape to the dedicated quest handler, with ordering and a regression test.
4. Big Map now loads the existing `original-ui/MMap/101.png` through Bevy `ImageNode` instead of rendering a green placeholder containing the asset-path text.

The subsequent same-coordinate investigation found that the remaining grid-aligned black rectangles were missing map draws, not opaque entity sprites. Map atlas page keys remain stable across viewports, while each producer snapshot carries only the rects used by that viewport. The Runtime had cached the first partial layout forever, so rects encountered after the `330,270` to `288,616` center transition could not bind. The map layout cache now grows each page's rect set monotonically and rebuilds/rebinds only when that set changes.

Live revalidation at `288,616` now reports `657 + 125 = 782` live draws with `missingBindings=0`, and the black grid holes are gone. This closes the map P0 found in this run. The Candidate is still not visually Accepted because secondary panels, combat-target presentation, HUD data/name presentation, effect/lighting density and exact Crystal geometry remain P1.

## Live comparison

| Surface | Functional result | Visual result | Severity | Decision |
|---|---|---|---|---|
| Login | Account/password fields and buttons work | Crystal composition is close; native title-bar difference is allowed | P2 | pass for this run |
| Character select | Roster selection and Start work | Four slots, preview and bottom controls closely follow Crystal | P2 | pass for this run |
| In-game map | Authoritative scene reaches `BichonProvince`, `288,616` | Ground/buildings render and the previously observed grid-aligned black holes are gone | closed | pass for this scene |
| Main HUD | HP/MP, belt, chat frame, minimap and buttons render | Main frame is recognizable, but combat target overlay is oversized and labels/entities overlap | P1 | fail |
| Quest button | Opens Quest Log, no longer opens Bag | Generic brown full-width panel, not Crystal dialog art/layout | P1 | functional pass, visual fail |
| Options button | Opens staged settings | Generic wide panel; original uses compact framed Crystal Options dialog and different controls | P1 | functional pass, visual fail |
| Mail | Opens and exposes compose/close actions | Generic text panel | P1 | functional pass, visual fail |
| Big Map | Opens, zoom controls and player marker exist | Real `original-ui/MMap/101.png` now renders; outer panel is still generic rather than Crystal-framed | P2 | improved; frame parity open |
| Menu | Entries are actionable | Generic text list, not a Crystal-framed menu | P1 | visual fail |
| Escape from Quest | Quest closes without opening Menu | Live regression and focused automated test pass | closed | pass |
| Effects/lighting | Some border/safe-zone effects and lamps render | Density, clipping, scene lighting and layering do not match the reference | P1 | fail |

## Evidence

- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/original-ingame-reference.jpg`
- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/candidate-ingame-map-after-url-load-fix.jpg`
- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/candidate-ingame-after-layout-growth-fix.jpg`
- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/original-options.jpg`
- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/candidate-bigmap-placeholder.jpg`
- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/candidate-bigmap-real-image-after-fix.jpg`
- `docs/generated/player-qa/native-windows-visual-validation/20260822-current-source-live/candidate-quest-generic-panel.jpg`

The two in-game captures use the same map name and authoritative coordinate (`BichonProvince`, `288,616`). They are valid same-scene evidence for defect triage, but not acceptance evidence because character identity and transient entity positions differ.

## Runtime evidence

Before the standalone URL fix:

```text
[runtime-map] waiting-standalone missing=[standalone:WemadeMir2/Objects#...]
```

After standalone URL loading, before layout growth:

```text
[runtime-map] synced center=Some(PresentationGridCenter { x: 288, y: 616 })
              tiles=657 standalone=125 live=539
```

After the stable-page layout growth fix:

```text
[runtime-map] synced center=Some(PresentationGridCenter { x: 288, y: 616 })
              tiles=657 standalone=125 live=782 missingBindings=0 sample=[]
```

The exact equality closes the skipped-binding diagnosis for this scene. Runtime traces now also print the missing binding count and up to eight `tile:atlas#rect:reason` samples, so a future stale manifest cannot silently regress into black holes.

The map tooling now mirrors the Web Mir3 library-index mapping when collecting native keyed-map sources and includes alpha/additive references from all three map layers. A separate source-quality diagnostic detects uniform opaque near-black PNGs. It currently reports seven known `WemadeMir2/Tiles` placeholders; normal development builds warn, while `--strictSourceQuality` makes the finding release-blocking. The existing `--skipIfPresent` development fast path remains scan-free and non-destructive.

The packaged asset root also lacks:

```text
original-ui/frame-sets.generated.json
```

The fallback affects entity animation/frame metadata. It does not explain the original total-black map, but it must be added to Candidate packaging before release.

## Automated gates run in this validation

```text
cargo +1.95.0 test --manifest-path apps/game-client/runtime/Cargo.toml
  169 passed; 0 failed

cargo +1.95.0 test --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui
  256 passed; 0 failed

cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml
  231 passed; 0 failed

node --test apps/web/scripts/test-native-keyed-map-pack.mjs
  1 suite passed; 0 failed

node --test apps/web/scripts/test-map-atlas-budget.mjs
  8 passed; 0 failed

npm --prefix apps/web run typecheck
  passed

cargo +1.95.0 build --manifest-path apps/game-client/platform-windows/Cargo.toml --release
  passed; 65,077,760 bytes
  SHA-256 22BF02AABB42ED34D32B3BE5578B4005CD1824CF5200626150CD33C300A45E8F
```

These tests prove state transitions and renderer contracts, not visual equivalence. The live defects above take precedence over stale documentation that calls the HUD or full Candidate Accepted.

## Ordered closure plan

1. **MAP-P0 (closed for baseline scene)** — retain `missingBindings=0` and exact expected/live counts while walking and changing maps; add broader map coverage before release.
2. **PACK-P1** — package and verify `original-ui/frame-sets.generated.json`; rebuild EXE/assets/VERSION atomically from one revision.
3. **UI-P1** — replace Character, Inventory, Skill, Quest, Options, Menu, Mail and the remaining Big Map frame with Crystal assets and exact geometry.
4. **INPUT-P1** — extend single-consumer input coverage across the remaining modal/shortcut combinations; the observed Quest-to-Menu Escape fallthrough is closed.
5. **WORLD-P1** — reconcile effect density, clipping, lighting, entity origins, labels, authoritative HUD data and combat-target placement against the same scene.
6. **VIS-GATE** — recapture Login, Select, InGame and every core panel at 100%, then 125%/150% DPI; require no P0/P1 before model scoring.
7. **FINAL** — use a visual model only as a defect classifier, then require a separate human 20-minute play/feel acceptance. Do not relabel model review as human Accepted.
