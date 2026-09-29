# Native primary-item true-size checkpoint (2026-09-03)

This is source, original-asset and headless regression evidence, not visual
acceptance. `visualAccepted=false`, `accepted=false`,
`globalParityPercent=null`. The goal and stacked Draft PR #250 remain open.
All 33 Windows backlog IDs are retained.

The implementation builds on `cb2c4ec3832bd589014178b15e3ca05cb70e05da`.
Exact implementation/reference hashes and commands are in `verification.json`.
This checkpoint supersedes only the primary GetTrueSize / image-zero leaves
left open by the preceding Guild-storage checkpoint. Older captures and their
binary identities have not been relabelled as evidence for this source.

## Cause and original authority

The primary renderer centred icons using full PNG dimensions, rejected every
image index zero, clipped icons to cells and used incorrect equipment/storage
cell sizes. Crystal instead centres using the *size* of the nonzero-alpha
bounds, then draws the unchanged full bitmap. It neither subtracts the alpha
origin nor crops, stretches or applies library offsets on these draw paths.
Signed integer division truncates toward zero, including oversized images.

The pinned original revision is
`92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`; all 11 reference files have a clean
scoped Git status. References below are in that Crystal checkout.

| Contract | Original source |
| --- | --- |
| Concrete `UserItem.Image` uses Info and current count, including valid zero | `Shared/Data/ItemData.cs:641-681` |
| Nonzero-alpha size; full-bitmap draw | `Client/MirGraphics/MLibrary.cs:611-658,959-1059` |
| Default 36x32 cell; image draw; StackSize > 1 shows even count 1 | `Client/MirControls/MirItemCell.cs:184-193,2511-2571,2595-2630` |
| Child controls are not scissored to each parent's rectangle | `Client/MirControls/MirControl.cs:693-737` |
| Default Index -1 / AutoSize does not replace item-cell size | `Client/MirControls/MirImageControl.cs:147-182` |
| Bag 36x32 cells, 37/33 pitch; belt explicitly 32x32 | `Client/MirScenes/Dialogs/InventoryDialog.cs:149-185,654-665` |
| Fourteen equipment cells use the default 36x32 size | `Client/MirScenes/Dialogs/CharacterDialog.cs:45-52,229-348` |
| Personal storage uses the same default size, 37/33 pitch | `Client/MirScenes/Dialogs/NPCDialogs.cs:2935-2954` |
| NPC row is 205x32 with a 40x32 icon area | `Client/MirControls/MirGoodsCell.cs:20-60,132-140` |
| Trade's own and guest cells use default MirItemCell size | `Client/MirScenes/Dialogs/TradeDialogs.cs:113-132,234-250` |
| Amount-modal item area is 38x34 and draws the original item | `Client/MirControls/MirAmountBox.cs:15-111` and `DrawItem` |

## Implemented scope

- Bag, quest inventory, belt, equipment, personal storage and NPC goods share
  the loaded-image alpha-size layout. Guild/coin retain their preceding
  implementation through this common helper. Warehouse-side bag rows, own
  and partner trade icon regions, and the delete-amount item use it too;
  their outer layouts/operations are not thereby accepted.
- Concrete items select current-count `UserItem.Image` from source Info,
  ignoring stale legacy icons, stale source-user counts and realInfo image
  substitutes. Known Items/0 is drawable; legacy zero without source remains
  absent. Missing images do not become guessed name abbreviations. Base-image
  GameShop/Quest/craft/mail-preview exceptions are not changed.
- Legacy `icon_width`/`icon_height` still describe full PNG frames for schema
  compatibility; they no longer decide native icon placement. Windows read
  models retain valid source-zero frame metadata without inventing it for
  missing legacy zero. No protocol schema or authoritative state is changed.
- The belt selects a changed image before layout; late loads work without
  another inventory update. Clear/remove/unavailable paths hide stale icons.
  An installed Bevy default white texture cannot become an empty-slot item.
- All fourteen equipment hit cells change from 32x32 to original 36x32;
  personal storage changes from 32x30 to 36x32. Positions, grid pitch and
  authoritative slot addressing remain unchanged. Oversized full bitmaps
  can draw beyond cells; bag/grid/NPC-row clipping is removed. Stack labels
  use source StackSize, including a visible count 1 for stackable items.

## Original assets and regression evidence

The read-only verifier decodes **all 1,003 currently exported Items PNGs**
independently with Sharp and compares exact RGBA and frame metadata to the
original `Items.Lib`. It runs the source's four edge scans independently of
the Rust bounding-box pass. The complete checked-in fixture is
`apps/game-client/client-bevy/test-fixtures/original-item-true-sizes.json`.
All 1,003 match. This is the current exported set, not a claim that the entire
original library or every item surface has been accepted.

550 of these frames have different full-frame and alpha-bound sizes; 478
produce a different offset in a 35-pixel cell. The original library SHA-256 is
`5d5f6e0251d2e5f7d87cb18352be2c2999ea311a7aa988de63dcf2fa78f9fb5a`.
The RGBA-plus-geometry fingerprint is
`e453722861498d71ae0facaa90c506d0e3d4a9ec362431639499255b9c84849d`.
The final verifier output reproduces the fixture after newline normalization.
No PNG, asset metadata or original library was rewritten.

| Final check | Observed result |
| --- | --- |
| Shared native UI, full harness | 562 passed; 0 failed; 0 ignored |
| Windows host, full harness | 528 passed; 0 failed; 0 ignored |
| Client runtime, full harness | 212 passed; 0 failed; 0 ignored |
| UI core, full harness | 43 passed; 0 failed; 0 ignored |
| Focused `primary_` filter | 13 passed; a subset of native UI, not 13 additional tests |
| Item-icon gate | 11 passed; all 924 required images (913 base + 11 quantity) |
| Independent original RGBA/metadata verifier | 1,003 / 1,003; exact fixture reproduction |
| Formatting / script syntax / diff whitespace | All four Rust crates, verifier and diff pass |

The native harness adds eleven tests (seven primary ECS, two belt ECS and
two model tests); Windows adds one. An older frame-size-only test is replaced
with an actual-PNG regression, not disabled or ignored. One native test loads
all 1,003 PNGs and asserts **5,015 exact node geometries** across 36x32, 32x32,
40x32, 35x35 and 38x34 cells. These are model/node assertions, not GPU pixel or
new screenshot comparisons. Additional tests cover all fourteen equipment
cells, storage, first/last bag rows and ancestors, image zero, Poison/Amulet
thresholds, stale metadata, late loading, resource removal and default-white
clearing. Existing Guild and item-hint tests remain in the full passing suite.

Run from `mir2-web3`, using Rust 1.95.0 and an isolated system-temp Cargo
target with jobs=2, incremental=0 and test/dev debug=0. The repository's
`RUST_TEST_THREADS=1` is unchanged. The actual target is in the JSON ledger.

```powershell
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui -- --quiet
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/platform-windows/Cargo.toml -- --quiet
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/runtime/Cargo.toml -- --quiet
cargo +1.95.0 test --locked --offline --manifest-path apps/game-client/ui-core/Cargo.toml -- --quiet
npm.cmd --prefix apps/web run test:item-icons
node apps/web/scripts/verify-item-true-size.mjs <Crystal-client-Data-directory>
```

Existing compiler warnings are retained. No current full-server test result
is inferred from historical runs: server logic, schema, locks, saves, auth and
shared Zone code are untouched by this checkpoint.

## Remaining work and safety boundary

Computer Use remains paused after user Escape. This round neither launches
an application window, injects input nor captures a screenshot; test harness
executables are not a new running-game or same-EXE acceptance claim. The prior
recoverable PDB backups and unrelated running programs are untouched.

Next bounded CLI work is source TradeDialog/GuestTradeDialog window/cell
geometry and gold/confirm/cancel state. Their current list layout is still
provisional despite corrected icon pixels. Guild item operations/state
overlays, full text editing, overlap/movement/shared throttle, other concrete
item surfaces, base-image preview layouts, FloorItems and WN-CHAR-002 source
operation/raw-versus-normalized slot issues remain open.

Only after the user explicitly resumes foreground QA may a final-source
binary be used for populated bag/belt/equipment/storage/NPC/trade transitions,
quantity changes, oversized/zero images and original paired-state captures.
Trusted package/light, real 100/125/150% DPI, soak, full class/action and human
visual/audio/feel acceptance remain separate gates. The 33-ID denominator is
unchanged; no overall Candidate or accepted percentage is claimed.
