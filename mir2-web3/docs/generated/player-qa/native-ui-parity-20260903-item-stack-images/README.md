# Native source stack-image checkpoint

Date: 2026-09-03 (Asia/Shanghai). Bounded development evidence only.
`visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

## Implemented behavior

Carried items previously kept the catalogue image regardless of quantity.
The shared `crystal_user_item_image` selector now implements Crystal's actual
`UserItem.Image` property. Simulation bag/belt/storage/equipment snapshots,
Gateway NPC goods and the native item decoder derive the image from the exact
source `ItemInfo` and the current instance quantity. Width and height are
resolved **after** choosing the image, including the poison width changes.

| Source identity | Count bands and Items images |
| --- | --- |
| Amulet, index 712, type 8 / shape 0 | 0..199: 3660; 200..299: 3661; 300+: 3662 |
| GreenPoison, index 710, type 8 / shape 1 | 0..49: 3673; 50..99: 3674; 100..149: 2960; 150+: 3675 |
| RedPoison, index 711, type 8 / shape 2 | 0..49: 3670; 50..99: 3671; 100..149: 2961; 150+: 3672 |
| Other types/shapes or StackSize 0 | Unchanged source ItemInfo.Image |

The source condition is `StackSize > 0`, not `> 1`. Count is not clamped to
StackSize inside this property. The tests cover zero as a property boundary,
not as permission to create an empty live stack.

Known source identity also overrides stale legacy icon fields for ordinary
items. Two former legacy expectations were corrected: index 658 maps to
source image 398, and index 321 maps to 61, not the unrelated cached image
24. This is a projection correction, not a save-store migration. Raw
`ItemInfo.Image`, instance identity/count, persistence fields and StateItem
paper-doll images are not rewritten. Unknown/ambiguous/partial source data
does not trigger a name, rarity or `realInfo` guess; the existing explicit
fallback icon is not promoted into source-parity evidence.

## Source and surface boundary

Authority: local Crystal revision
`92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`. The relevant source files are clean.

- `Shared/Data/ItemData.cs:641-681`: complete image rule above. Neither display
  name nor viewer-dependent `GetRealItem` participates in this property.
- `Client/MirControls/MirItemCell.cs:2511-2571`: actual items use `Item.Image`
  and original true-size integer centring. Craft shadow cells deliberately
  use `ShadowItem.Info.Image` instead.
- `Client/MirControls/MirGoodsCell.cs:135-142`: actual NPC goods use Item.Image
  centred in 40x32. Their complete wire UserItem record remains unchanged.
- `Client/MirControls/MirGameShopCell.cs:294-297`: GameShop previews use the
  base `Item.Info.Image`, even when the preview count is 300. A native
  regression protects this distinction.
- `Client/MirScenes/Dialogs/QuestDialogs.cs:1643-1700`: QuestCell.Item is an
  ItemInfo, so reward preview images stay at the base image; the separate
  hover UserItem is not the source of its painted image.
- `Client/MirScenes/Dialogs/MailDialogs.cs:519`: the mail-list thumbnail also
  deliberately uses the first attachment's base Info.Image.
- `Server/MirObjects/ItemObject.cs:365-375` sends the actual Item.Image, but
  `Client/MirObjects/ItemObject.cs:24-38` draws it from **FloorItems**, not
  Items. Ground drops remain a separate source/asset/runtime leaf.

This implementation does not yet change the guild-storage/trade icon paths
in `client-bevy/src/crystal_ui/overlays.rs`, which still select Info.Image.
A read-only follow-up audit identifies the next exact contracts:

| Remaining actual-item surface | Source and current boundary |
| --- | --- |
| Guild storage | `GuildDialog.cs:672-688` constructs 112 MirItemCells, 35x35, grid origin `(31,20)`, step `(36,36)`. Native GuildStorageItemModel already has authoritative `count`; image selection and the full panel still need repair/verification. |
| Own/guest trade | `TradeDialogs.cs:104-120,237-253` uses actual MirItemCells in both grids, with `2*x+y` slot ordering and origin `(10,39)`, step `(37,33)`. Native TradeItemModel has `count`; its current generic row and base-image path do not prove source layout or quantity parity. |
| Mail attachment cells | `MailDialogs.cs:778-788,1229-1238` uses MirItemCell, unlike the base-image mail-list thumbnail. Keep these as separate obligations. |
| Own/guest rental | `ItemRentingDialog.cs:128-137,291-300` uses actual MirItemCell at `(16,35)` for Renting/GuestRenting; count/image and populated native evidence remain unverified. |

Auction/craft actual-item cells and all other concrete-item surfaces still
require individual audits. A correct base-image preview must not be
mechanically replaced just to make every surface equal.

## Automated regression

Tests use Rust 1.95.0 explicitly and the repository's `RUST_TEST_THREADS=1`.
The final source identities are in [process-provenance.json](process-provenance.json).

| Check | Result |
| --- | --- |
| Windows binary unit tests | 527 passed, 0 failed, on the final source correction |
| Shared game-data unit tests | 39 passed; includes all 196,608 shape/count combinations, guards and the 1,628-row catalogue |
| `item_stack_images` integration | 4 passed on final source |
| Full Simulation library regression | In progress at report preparation; not yet claimed passed |
| Full Gateway library regression | In progress in isolated system-temp build directory; not yet claimed passed |
| Original item-icon suite | 11 passed; all 924 required original images pass integrity verification |
| Both Rust formatting checks / git diff check | Passed |
| Fixture and capture-verifier syntax | Passed |

The four integration tests cover 19 threshold/guard cases in each of bag,
belt, storage and equipment; a real Amulet 300 -> SplitItem 101 -> 199/101 ->
cross-belt MergeItem -> 300 -> equip -> save/logout/login -> remove sequence;
both poisons splitting 150 into 49/101 and merging back to 150; and a known
ordinary item whose stale icon must not override source Info.Image. Split
children use the existing authoritative add-item policy and land in the belt.
These protocol-level tests do not claim source native mouse/keyboard parity.

Commands, relative to `mir2-web3`:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml -- --quiet
cargo +1.95.0 test -p mir2-simulation --test item_stack_images -- --quiet
cargo +1.95.0 test -p mir2-simulation --lib --features test-support -- --quiet
npm.cmd --prefix apps/web run test:item-icons
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 fmt --manifest-path apps/game-client/platform-windows/Cargo.toml -- --check
```

Regular Gateway/native output files were externally locked. An attempted
temporary Gateway test target also caused Cargo to build the locked regular
binary; that attempt failed and is **not** counted as passing. Both temporary
manifest additions (one earlier native QA bin, one failed Gateway test target)
were removed and the original manifest hashes verified. Neither locked output
was forcibly replaced, no unrelated user program was closed, no target cache
was deleted and no test was newly ignored.

The final full Gateway and game-data commands use an independently created
temporary directory on the system volume, avoiding the locked outputs and
nearly full work volume without changing the package manifest:

```powershell
$stackRegressionBuild = Join-Path $env:TEMP ('mir2-stack-image-regression-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stackRegressionBuild
$env:CARGO_TARGET_DIR = $stackRegressionBuild
$env:CARGO_BUILD_JOBS = '2'
$env:CARGO_INCREMENTAL = '0'
$env:CARGO_PROFILE_TEST_DEBUG = '0'
cargo +1.95.0 test -p mir2-gateway --lib --features mir2-simulation/test-support -- --quiet
cargo +1.95.0 test -p mir2-game-data --lib -- --quiet
```

Only debug-symbol generation/incremental caching differ in that build. The
normal complete library test source, assertions and feature set are retained.

## Initial native capture, not final-source visual acceptance

The exclusive-create fixture helper prepares a separate local QA account
store from an existing saved character. It leaves the seed and catalogue
unchanged and refuses to overwrite its destination. The fixture contains
24 bag, six belt and six storage items, including all eleven count images and
the unaffected AmuletOfRevival/Amulet(Bundle) guards. It has no equipped gear
and no GM authority. Local account stores, passwords, recovery files and logs
are excluded from this evidence directory and from the commit.

[Initial inventory PNG](stack-images-r1-inventory-1788384324956-1.png), with its
[unchanged draft sidecar](stack-images-r1-inventory-1788384324956-1.json), shows
StackImageQA, Taoist level 50, BichonProvince `(290,620)`, 1024x768, DPI 1.0,
inventory page 0 at `(0,0)`. It was captured automatically at
`2026-09-02T21:25:24.956Z` by QA process 254696 using the isolated loopback
Gateway process 253296.

The capture EXE is the temporary `mir2-stack-images-qa` target built from the
same `src/main.rs`, dependencies and Debug profile; it is not the unchanged
regular `mir2-platform-windows.exe`. Its SHA-256 is
`77178c9d79089e4f136c5d5409c71d5e274046224b8a08e8dba6874edc61670a`.
The capture Gateway hash is
`c1df115d13faed1ba6aa74b688dd485849d00a8194be29880fb33692e4db81ff`.

**Both capture binaries predate the final correction that makes known ordinary
items override stale legacy icons.** The shown count-band implementation is
unchanged, but this historical initial image must not be represented as a
fresh final-source EXE capture. The regular EXE remains the earlier asset
checkpoint binary, hash `55ca1d61a6977f164b1d6222de2aaafd4e21fef4a1c5db898ebee6ff0fbdb993`.

The user stopped Computer Use with Escape before interactive validation. No
further foreground/window input was performed; split/merge/equip/storage/shop
interactions were not captured in this run. Work continued only through code,
tests, saved-image inspection and documentation.

## Read-only pixel sample

```powershell
node apps/game-client/platform-windows/scripts/verify-item-stack-image-capture.mjs --capture docs/generated/player-qa/native-ui-parity-20260903-item-stack-images/stack-images-r1-inventory-1788384324956-1.png
```

[pixel-sample.json](pixel-sample.json) records **24 matched bag slots / 6,161
exact opaque RGB samples / zero mismatches** at the source coordinates:
grid origin `(9,37)`, step `(37,33)`, cell 36x32, integer true-size centring.
There is no position search, score tolerance or fitted alignment. All **86**
wrong sibling/base-frame comparisons and **96** one-pixel translations fail.

This deliberately samples only opaque pixels above the lower 14px count-label
region. It proves neither count-text pixels nor transparent edges, other
panels, manual transitions, final-source executable identity, original paired
state, other DPI or human acceptance. The verifier never edits screenshots or
sidecars and never promotes their acceptance flags.

## Remaining denominator

Keep WN-ITEM-002 open for all remaining concrete-item surfaces, FloorItems,
locked/selected/sealed/unavailable/durability overlays, final-source same-EXE
manual transitions, paired original state, real DPI and human review. Keep
WN-CHAR-002 open for source double-click/drag/Shift behavior, the generic
operation popup and normalized versus raw Crystal/belt addressing. Existing
world-wing/remote-player/Character-tab/other-dialog/backlog leaves are intact.

The original sidecar is still draft/ineligible: authoritative light and
trusted package provenance are absent. The separate ledger cannot fill those
fields retroactively. The local WebGPU WASM artifact required by the broader
frontend logic suite remains absent; no full-frontend, packaged-Candidate,
full-green CI, same-state Crystal or whole-game completion claim is made.
