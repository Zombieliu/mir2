# Native item-icon and high-armour asset checkpoint

Date: 2026-09-03 (Asia/Shanghai). Bounded development evidence only.
`visualAccepted=false`, `accepted=false`, `globalParityPercent=null`.

## Result and source

The previously empty MirArmour equipment cells and head-only high-armour self
actors had separate missing exports. This checkpoint adds exact original
pixels, not substitute art or a renderer fallback:

- The 1,628-row item catalogue requires 913 distinct base Images. Of these,
  632 had no exported PNG. All are now present, including `Items/595` and
  `605` for catalogue items 45 and 51.
- Crystal `UserItem.Image` also chooses eleven Amulet/Poison images by stack
  count. None occurs in the catalogue's base Image column; all eleven were
  missing and are now exported and included in the mandatory denominator.
- The full required union is **924 images**, with 643 new PNGs. All 360 older
  PNGs are preserved byte-for-byte. The Items export retains 1,003 frames in
  total, including 79 existing non-catalogue frames.
- `CArmour/09` and `/10` now each contain all 1,616 source frames. These 3,232
  frames restore HeavenArmour/MirArmour bodies through the existing native
  individual-library path; no production actor-routing change was required.

Crystal checkout is `92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d`; the audited
source files are clean:

- `Client/MirControls/MirItemCell.cs:195,2511-2551`: Items library, actual
  `UserItem.Image`, source true-size centring, selected/locked dimming.
- `Shared/Data/ItemData.cs:641-681` and `Shared/Enums.cs:879-888`: Amulet type
  8, positive StackSize, shape-specific quantity images; other types/shapes
  return Info.Image. Runtime quantity selection is still a separate open
  leaf; this checkpoint closes the asset prerequisite, not that behavior.
- `Client/MirObjects/PlayerObject.cs:548-593`: common-class CArmours/Hair/
  Weapons libraries, male/female body offsets 0/808 and weapon offsets 0/416.
  World wings use a separate CHumEffect library and are not fixed here.
- `Client/MirGraphics/MLibrary.cs`: original Items and CArmours loading.

Source library and catalogue SHA-256 values, all shown frame geometries, the
EXE identity and all screenshot hashes are in
[process-provenance.json](process-provenance.json).

## Export and regression contract

`export-crystal-ui.mjs` derives the complete image union from every catalogue
row plus the source Amulet/Poison rule, rather than the former small static
export list. Invalid identity/type/shape/stack fields, missing source frames
and zero-size required frames fail. Exported metadata records the exact
source library and catalogue identities, dimensions/offsets, decoded RGBA
digest and PNG digest. `--skipExisting` now decodes existing PNGs and verifies
their exact source pixels before reusing them; a mismatched file is rejected
without overwriting it.

`npm --prefix apps/web run test:item-icons` checks all 924 required PNGs and
their metadata. Tests reject missing file-plus-metadata pairs, duplicates,
renamed paths, wrong dimensions, substituted pixels, corrupt PNGs, missing
fingerprints, new unexported catalogue images and omitted quantity images.
The same command is mandatory in the local Candidate gate, the Windows
vertical-slice gate, frontend logic and offline asset verification. These
hashes are integrity checks, not a trusted publisher signature.

The two armour libraries also pass a direct read-only comparison of every
decoded PNG and geometry against its original `.Lib`. Native tests retain
all 3,232 frame identities and check **512** complete body/hair/weapon
composites: two armour libraries, both Warrior genders, eight directions,
and all Standing (4), Walking (6) and Running (6) phases.

## Same-EXE live matrix

Fresh Debug EXE SHA-256:
`55CA1D61A6977F164B1D6222DE2AAAFD4E21FEF4A1C5DB898EBEE6FF0FBDB993`
(88,405,504 bytes). It was built from the working tree based on
`0e10b08bc0865831d57839055415674be6ca0a65`, not an unchanged published commit.
The existing isolated loopback gateway and fixture account store were reused;
no production data or account migration was performed.

All eight renderer-owned captures are 1024x768 at DPI 1.0, run
`item-actor-assets-20260903-r1`, with adjacent unmodified JSON sidecars.

| Character / armour index | Original body / icon | Observed Bichon coordinate | Capture |
| --- | --- | --- | --- |
| WingOneM / 375 | CArmour/09, Items/87 | 290,620 | [HeavenArmour male](assets-one-m-r1-character-1788382226980-1.png) |
| WingOneF / 379 | CArmour/09, Items/86 | 288,618 | [HeavenArmour female](assets-one-f-r1-character-1788382178664-1.png) |
| WingTwoM / 45 | CArmour/10, Items/595 | 290,620 | [MirArmour male](assets-two-m-r1-character-1788382000886-1.png) |
| WingTwoF / 51 | CArmour/10, Items/605 | 289,619 | [MirArmour female](assets-two-f-r1-character-1788382140018-1.png) |

These are four separate labelled QA cases, not a same-state Crystal pair.
Their coordinates differ and are not normalized after capture. All shown
icons and body frames were visually inspected in the saved renderer images.
The eleven additional stack images were exported after this matrix; the
ledger therefore preserves separate capture-time and final asset-manifest
hashes. The EXE, shown PNGs and armour metadata are unchanged.

## Post-auto manual interaction

`auto_capture_system` now checks the one-shot completion flag **before**
preparing the target. It stops reopening Character or closing subsequent
notices after the screenshot is queued. A Bevy App regression covers active
preparation followed by preservation of manual Inventory page 1, Character
Stats II and a later notice over three updates.

Process 240912 keeps the auto-Character option enabled throughout this real
pointer/keyboard sequence on WingTwoM, at the unchanged `(290,620)` coordinate:

| Step | Observed result | Capture |
| --- | --- | --- |
| Auto | MirArmour body, equipment icon and paper doll present | [initial](assets-two-m-r1-character-1788382000886-1.png) |
| Manual I | Inventory remains open after automatic capture | [released UI](assets-two-m-r1-in-game-1788382025049-2.png) |
| Unequip | Armour and paper-doll wing disappear; default world body returns | [unequipped](assets-two-m-r1-in-game-1788382062892-3.png) |
| Inventory | Exact Items/595 icon occupies first bag cell; free count 45 | [bag icon](assets-two-m-r1-in-game-1788382076681-4.png) |
| Re-equip | MirArmour world body, equipment icon and paper doll return | [restored](assets-two-m-r1-in-game-1788382107539-5.png) |

An attempted ITEMS II click did **not** select page two: this fixture has no
inventory expansion. Its sidecar truthfully records `inventoryPage=0`.
The live evidence proves panel release, not live second-page retention.
Second-page/Stats II/later-notice retention currently has unit-test evidence.

## Validation and limitations

- Windows binary tests: **524/524**, rerun after the final asset export.
- Fresh Windows Debug build and `cargo fmt --check`: pass.
- Item-icon suite: **11/11**, plus the full **924-image** integrity gate.
- Crystal library/source-snapshot/exporter regression suite: pass, including
  synthetic sparse and quantity-derived export, safe reuse and missing source.
- Windows PowerShell 5.1 vertical-slice self-test: **10 controls**, pass.
- Packaging's CArmour closure: four libraries / **5,760** currently exported
  frames pass; direct original-pixel/geometry comparison: **3,232** new frames.
- All eight PNG dimensions and image/sidecar hashes verify.
- Full frontend logic is **not green**: it passes through the new icon gate
  and presentation-pose tests, then fails because the local WebGPU WASM file
  `public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm` is absent. All
  five later commands were run independently and pass. No fake WASM or gate
  bypass was introduced.
- The optional PowerShell 7 self-test attempt fails the existing native
  environment-scoping compatibility probe; that helper code is unchanged.
  The supported CI Windows PowerShell 5.1 invocation above passes.

World CHumEffect wings, count-dependent runtime icon selection/live stacks,
all other class/equipment/action combinations, sustained movement, source
double-click/drag/lock interactions, belt/amulet destinations, specialized
item-surface details, original paired evidence, trusted build/light, real
125/150% DPI, signing and human acceptance remain open. Fast switches between
these QA processes also leave prior-player name-only observations in some
initial captures; remote appearance and disconnect/AOI timing need their own
trace and cannot be inferred correct from the restored self bodies.

Every sidecar remains `mir2-native-visual-capture-draft-v1`, `eligible=false`,
with missing authoritative light and trusted package provenance. The separate
ledger does not rewrite or promote them. No backlog leaf was removed.
