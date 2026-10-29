# Native Crystal Guild storage: bounded source/logic checkpoint

Date: 2026-09-03. Implementation baseline:
`970532ec7ac23fbc09e1b4501607d62a705c3828` on
`codex/windows-player-journey`; same stacked Draft PR #250, based on
`codex/windows-visual-parity`. No merge, deployment or acceptance promotion.

This round replaces the invented four-column text list and page controls with
Crystal's storage grid, row scrolling, count-derived original icons and gold
amount box. It is **source, asset and headless-test evidence**, not an in-game
screenshot or complete Guild implementation. All 33 existing Windows backlog
IDs remain. `visualAccepted=false`, `accepted=false`,
`globalParityPercent=null`.

## Source authority

Crystal revision: `92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`.
The inspected source files are unchanged in that checkout. Exact file hashes
and test outcomes are recorded in `verification.json`; exported image and
original library hashes are in `source-assets.json`.

| Source at the pinned revision | Implemented contract |
| --- | --- |
| `Client/MirScenes/Dialogs/GuildDialog.cs:100,132-205,617-747,1943-2008` | Initial index/viewport asymmetry; original tabs, storage page, all 112 slot identities, eight visible rows, arrows, drag and wheel integer formulas |
| `GuildDialog.cs:1272-1310,2010-2045,2140` | Storage/rank tab bits; leader-only withdrawal control; current-gold maximum; amount prompt, positive-only packet and 100 ms gold-send gate |
| `Client/MirControls/MirControl.cs:852-935` | Thumb press records the grab offset; only movement changes its position; release ends dragging |
| `Client/MirControls/MirItemCell.cs:2511-2571,2595-2630` | Concrete `UserItem.Image`, alpha-bound true-size integer centring, ignored library offset and Yellow stack count including one |
| `Client/MirGraphics/MLibrary.cs:640-688,959-1059`, `MirImageControl.cs:220` | GetTrueSize scans nonzero alpha but returns only the bounding size; Draw still uses the full bitmap without subtracting the alpha origin or default library offset |
| `Client/MirControls/MirAmountBox.cs:15-111,172-230` | Original 204x109 modal, centred coin, exact buttons/input, maximum selected, invalid input hides OK, max clamp and Enter/Escape routes |
| `Client/MirControls/MirControl.cs:817-847`, `MirButton.cs:160-165` | Mouse clicks use default ButtonB except explicit close ButtonA; direct keyboard InvokeMouseClick bypasses sound and visibility, including invalid-amount disposal |
| `Shared/Data/ItemData.cs:641-681` | Original Amulet/Poison quantity-image selector; neither names nor viewer `GetRealItem` determine the image |

Local winit 0.30.13's Windows event adapter emits wheel delta divided by 120
as `LineDelta`; the native adapter restores that delta before Crystal's integer
division. Pixel-unit touchpad events have no implemented source-equivalent
mapping in this slice and are not treated as wheel notches.

## What changed

- Storage page `(0,60,352,372)`, `Prguse/1851` at `(30,19,292,308)`, eight
  columns by fourteen rows. The visible 8x8 viewport uses 35x35 cells on a
  36-pixel pitch at `(31,20)`; scrolling preserves server slot IDs `0..111`.
- Preserve the source constructor's `StorageIndex=1` with initial rows `0..7`
  and thumb y=16. The first Down shows rows `2..9`; a stationary thumb press
  must not silently change that state. Arrow/wheel snaps use integer `289/6`;
  dragging uses `289/8`, grab offset and y clamping to `16..298`.
- Original `Prguse2/197..199`, `207..209` arrow art is 12x12 inside 16x14 hit
  targets. The 12x18 thumb uses frame 206. Source Storage/Rank tabs use
  `Title/105..106` and `101..102`, including visibility bits and the absent
  source hover/Rank pressed image.
- Item icons use current authoritative `GuildStorageItemModel.count` and
  `source.info`, not stale tooltip count or `real_info`. Nonzero-alpha bounds
  supply Crystal's GetTrueSize for integer centring, while the full PNG still
  draws unstretched and without subtracting the alpha origin or library
  offset. Oversized icons remain unclipped. A per-image geometry cache
  invalidates on asset changes, including same-handle reloads; missing CPU
  pixels or unsupported formats do not invent geometry.
  Known `Items/0` is legitimate art; an absent source remains undrawn. The
  other older personal-image paths' zero sentinel is not closed by this fix.
- Gold `+`/`-` open source-shaped `MirAmountBox`, initially selecting the
  captured maximum. Valid numeric input above the maximum clamps; uint
  overflow/empty input hides OK. Zero closes without a request. Nonzero OK
  rechecks current guild identity, leader rank for withdrawal, current balance
  and cooldown, then uses the existing correlated pending-operation queue.
  No optimistic gold mutation or server-permission change is introduced.
- Keyboard messages retain their sequence within a frame: digits, Backspace,
  Ctrl-down/A/Ctrl-up, and main/numpad Enter edit then submit the intended
  amount rather than the previous draft. Closing drains that overlay reader's
  entire batch so leftover text cannot enter a later prompt. Source Enter on
  an invalid uint disposes with no packet, although the mouse OK is hidden.
  Mouse gold buttons/OK/Cancel use default ButtonB; close uses ButtonA;
  keyboard direct callbacks remain silent.
- While open, amount modals gate gameplay/HUD/underlying-window input through
  the existing state checks and full-stage blocker. The overlay button batch
  retains its initial modal ownership even if Cancel closes it mid-batch.
  Focus loss ends dragging; closed panels, session reset, guild changes and
  loss of withdrawal rank invalidate the relevant local state.
- The unchanged pure `crystal_user_item_image` helper lives in `mir2-protocol`
  so the renderer can reuse it without embedding the game-data catalogue.
  `mir2-game-data` re-exports the same API and retains its exhaustive tests.
  Windows, runtime and Android lockfiles retain consistent dependencies; no
  wire format, schema, persistence, gateway authority or Zone behavior changes.
- The typed control registry deliberately grows from 177 to 181 entries.
  All five previous Guild storage IDs remain, with row semantics replacing
  invented pages; four entries describe the thumb and modal OK/Cancel/Close.
  No new control has a claimed reference screenshot.

## Verification

| Check | Result |
| --- | --- |
| Focused native Guild regressions | 44 passed, 0 failed, 0 ignored on final source |
| Full shared Bevy `native-ui` library | 551 passed, 0 failed, 0 ignored; zero doctests on final source |
| UI core | 43 passed, 0 failed |
| Protocol | 40 passed, 0 failed |
| Game data | 39 passed, 0 failed; retained exhaustive quantity/catalogue coverage |
| Windows binary unit harness | 527 passed, 0 failed, 0 ignored on final source |
| Simulation `item_stack_images` | 4 passed, 0 failed, 0 ignored on the relocated shared helper |
| Bevy runtime library | 212 passed, 0 failed, 0 ignored; zero doctests |
| Android dependency graph | `cargo metadata --locked --offline` passed; no Android build/device claim |
| Original UI/Guild/item frame comparison | 41/41 exact decoded RGBA and width/height/x/y metadata matches |
| GetTrueSize audit of all exported Items | 1,003 inspected; 550 frame-size/alpha-size differences, 478 different 35-pixel-cell centring offsets; findings are retained, not all fixed |
| Whole item-icon gate | 11/11 tests and all 924 required images pass |
| Formatting/whitespace | Rust fmt and `git diff --check` pass |

The 22 new headless ECS tests exercise actual native systems, not only a
parallel model: grid/image entities, late-load dimensions, frame 0,
current-count changes, permissions, modal geometry, no optimistic mutation,
pending/cooldown deduplication, keyboard ownership/ordered edits/invalid Enter,
mouse versus keyboard sound callbacks, scale/focus/window/wheel
gates, stationary and compressed press/move/release, lifecycle cancellation,
alpha-size versus full-bitmap drawing, same-handle cache invalidation and
missing pixel data. One test decodes 13 actual original PNGs through Bevy and
asserts both the true-size offsets and full draw dimensions. Six new pure
Guild geometry/state tests, two shared image-adapter tests and two pure
alpha-bound tests bring the final full suite from 519 to 551. UI-core adds
one registry test.

The full exported-Items alpha audit found a broader reason for small visual
offsets: 550 of 1,003 PNG frame sizes differ from Crystal GetTrueSize, and
478 produce a different centring offset in a 35-pixel cell. This round fixes
only the new Guild-item/coin helper. Primary bag, belt, equipment, personal
storage and NPC-goods icon paths still need the same correction and a
source-grounded regression. The earlier fixed-position inventory PNG check
used PNG-frame geometry, not an original GetTrueSize comparison; its exact
asset/quantity identity results remain valid but do not prove Crystal icon
centring. The complete affected-index lists remain in `source-assets.json`.

Headless tests create Window metadata but do not install a WindowPlugin,
launch rendering, inject computer input or capture screenshots. Early failing
fixture/lockfile attempts are not passing evidence. The initial byte-for-byte
PNG re-encoding experiment differed for 18 existing files because PNG encodings
can differ; independent PNG decoding verifies all 41 images' actual RGBA bytes.
No PNG was rewritten to obtain a match.

Reproduce the read-only original asset comparison from the project directory:

```powershell
node apps/web/scripts/verify-guild-storage-assets.mjs <Crystal-client-Data-directory>
npm.cmd --prefix apps/web run test:item-icons
```

Use Rust 1.95.0 and an isolated system-temp `CARGO_TARGET_DIR`,
`CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, and test/dev debug symbols disabled
for local regressions. Existing `RUST_TEST_THREADS=1` is preserved. Do not use
the nearly full working-volume targets or alter a manifest to skip failures.

The previous full Simulation 1491/1491 and Gateway 667 passed / one existing
ignored result belongs to implementation commit
`2cb9098407e21c47fbd43863aea6b3655a609c8b`, not a new full-server regression of
this helper relocation. The earlier stack-image PNG also predates its final
ordinary-item correction; it is not reused as Guild or current-code evidence.

## Open leaves and foreground pause

The user stopped Computer Use with Escape before this round. No Computer Use,
window activation, GUI application launch, gameplay input or new screenshot
was performed. Automatic goal continuation does not authorize resuming it.

Still required: same-final-code native EXE and original paired-state captures,
trusted package/light identity, actual 100/125/150% DPI and human comparison.
On an explicitly resumed GUI route, capture populated first/last storage rows,
real wheel/drag and gold deposit/withdraw/cancel/max/permission-change outcomes
with matching authoritative packets and balances. Do not issue mutations just
to produce screenshots without an isolated authorized fixture.

Guild item drag/store/retrieve/merge UI, selected/locked/durability/seal overlays,
full WinForms caret/selection/clipboard and GDI text, movable Guild window,
topmost overlapping-window pointer dispatch, cross-plugin closing-frame
input ordering and the shared cross-action
`LastGuildMsg` clock remain open. The implemented cooldown is gold-local.
Primary-surface alpha-bound centring and legitimate Items/0 handling also
remain open; this report supersedes older claims of exact icon centring only,
not their source grid, item-identity or footer evidence.
Trade/other actual-item surfaces, ground `FloorItems`, and source GameShop/
Quest/craft-shadow/mail-list base-image exceptions keep their own denominator.
The missing local WebGPU WASM artifact remains an earlier whole-frontend
verification limitation, not a green build claim for this round.

## Recoverable build-symbol relocation

The working volume reached approximately 112 MiB free. After verifying the
exact file stayed inside the generated target directory, was not a reparse
point and its test executable was no longer running, one 539,758,592-byte
(about 515 MiB) Gateway test PDB was moved to a fresh system-temp backup.
Source absence and destination length/SHA-256 were verified; no source,
asset, store, executable, unrelated process or other cache was removed.

- Original: `E:/mir2-player-journey/mir2-web3/target/debug/deps/mir2_gateway-23e520f2c94d8714.pdb`
- Recoverable backup: `C:/Users/Administrator/AppData/Local/Temp/mir2-guild-ui-symbol-backup-55f779c43b494709b51afbf8e54eb080/mir2_gateway-23e520f2c94d8714.pdb`
- SHA-256: `95fe292cbd337d3eee4def59039f050a999df27eea6696ed7f3a1f4e716d59e0`

Move the verified backup back to the original path only if symbols for that
old test executable are needed. The prior Simulation PDB backup is unchanged
and documented in the preceding item-stack-image report.
