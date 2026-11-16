# Native Bichon merchant map coverage — 2026-10-02

Status: future packaging default repaired; packaged and human visual acceptance
remain separate. The reviewed game source is
`ab970e0ef0656bcb30c719b97f752fa0de422b23`. This change updates packaging inputs
and documentation only; it does not rebuild or relabel that executable.

## Source finding

Frozen Candidate12 contains 1,620 raw map layouts, but its native terrain image
indices cover only 15 maps. The coverage gate audits the named image closure,
not all layouts. `0103` is BichonWall's `WeaponStore` / 武器店, with
Merchant_Vincent at `(19,18)`. It is absent from that 15-map image list.

WeaponStore needs 228 unique standalone terrain keys. R12 supplies one,
`WemadeMir2/Objects#3839`; 226 drawable keys exist in the approved full source
but are absent from the native index. The remaining `Objects#0` is a genuine
empty source slot, already documented in R12's other maps. The missing keys
are skipped before an image URL reaches the asset loader, so the existing
R2 page fallback cannot recover them. The layout and native/floor indices are
byte-identical in the approved R10 and R12 packages.

The original [map definitions](../packages/game-data/data/generated/crystal_respawn_manifest.json),
[NPC definitions](../packages/game-data/data/generated/crystal_npc_info_manifest.json),
[merchant scripts](../packages/game-data/data/generated/crystal_npc_manifest.json),
and [display catalogue](../packages/game-data/data/native-i18n/content.json)
identify these 13 merchant interiors. Runtime NPC placement consumes the same
definitions in `apps/simulation/src/runtime/map.rs`.

All 13 layouts use supported Crystal `LoadMapType1`. Counts are unique terrain
keys against frozen R12; they do not certify NPC/entity visuals.

| Map | Source title | Merchant/service | Supplied | Drawable missing | Source NoDraw |
| --- | --- | --- | ---: | ---: | ---: |
| 0101 | Tavern | Glenn pearls; Mogu buying | 0 | 204 | 0 |
| 0102 | MeatStore | Kim buy/sell | 0 | 54 | 0 |
| 0103 | WeaponStore | Vincent buy/sell/repair | 1 | 226 | 1 |
| 0104 | Library | Steven books | 1 | 42 | 0 |
| 0105 | AccessoryStore | Olivia, Diana, Kristin jewelry/repair | 0 | 217 | 0 |
| 0106 | DrapersStore | Anne clothing/repair | 0 | 216 | 0 |
| 0107 | BeautySaloon | Chris haircuts | 0 | 88 | 0 |
| 0108 | ReagentStore | Larry potions | 88 | 0 | 0 |
| 0109 | MedicineRoom | Travis potions | 35 | 0 | 0 |
| 0125 | Inn | Mary storage/buy; Andy pets | 0 | 159 | 0 |
| 0132 | Library | BorderVillage Brian books | 0 | 136 | 0 |
| 0140 | Warehouse | BorderVillage Anthony storage/buy | 0 | 116 | 0 |
| 0141 | GroceryStore | BorderVillage Alice, Betty, Clara jewelry | 307 | 0 | 0 |

The ten additions require 1,458 unique drawable keys from
`WemadeMir2/Objects`, `Objects2`, and `Objects3`. All three catalogued library
manifests and 36 source pages passed SHA checks; the pages total 37,729,016
bytes. There are zero out-of-range references or unavailable source libraries
in this merchant scope. The source-page byte count is not a new-package size
estimate. All 13 raw layouts also match their signed R12 file records.

Kitchen `0100`, MageHouse `0115`, Palace `0122`, Inn upstairs `0126`, Prison
`0127`, and dungeon/event interiors are outside this merchant scope.

## Default and explicit lists

`Get-CandidateDefaultMapNames` retains the 15 journey/supply maps and adds:

```text
0101,0102,0103,0104,0105,0106,0107,0125,0132,0140
```

The exact default is:

```text
0,0101,0102,0103,0104,0105,0106,0107,0108,0109,0125,0132,0140,0141,1,2,3,d001,d021,d022,d401,d601,d602,d605,d607
```

Canonicalization: lowercase ASCII identifiers, deduplicate, sort ordinal
ascending, join with LF and append one final LF, UTF-8 without BOM. The
25-map representation is 113 bytes with SHA256
`31a38749179cd53877e42a02bc62d5127a7a80225f3b9c6a7cdbb6ce1a209170`.

The package entrypoint uses this default only when `-NativeMapFileNames` is
omitted or empty. `Get-CandidateRequiredMapNames` remains the 15-map validation
minimum, preserving explicit legacy/custom lists and older package checks.
Explicit lists retain selected maps, lowercase/deduplicate them, and continue
to reject unsafe identifiers or missing required journey maps.

From `mir2-web3`, inspect and test the profile without generating a package:

```powershell
. ./apps/game-client/platform-windows/scripts/candidate-release-profile.ps1
Test-CandidateReleaseConfiguration
$maps = @(Get-CandidateDefaultMapNames)
$maps -join ','
```

Leave `-NativeMapFileNames` unset on the existing attested packaging command
to use the new default. To carry this list into an older attested source,
pass `-NativeMapFileNames $maps` explicitly. Keep `-SourceRevision` equal to
the actual executable/attestation source. The separate Candidate13 preparation
uses the unchanged `ab970` executable plus this explicit list; it is not an
executable built from the future-profile commit.

## Verification and pending acceptance

- [x] Frozen signed R12 layout/native/floor records matched the inputs above.
- [x] All 1,458 absent drawable keys have genuine catalogued source frames;
  the 36 page contents match their content-addressed SHA names.
- [x] Release configuration guards pass under Windows PowerShell 5.1 and
  PowerShell 7.6.5. They bind the reviewed default hash and preserve explicit
  legacy/custom selection, normalization and rejection behavior.
- [x] Four actual package map-selection cases pass in each shell: empty
  default, explicit legacy list, custom list, and incomplete-list rejection.
  Only the parsed selection branch and normalizer execute.
- [ ] Independently verify a fresh signed 25-map package's indices and PNGs.
- [ ] Walk the merchant interiors in the matching native package; check floors,
  walls, props, animation, alpha edges and ordinary merchant services.
- [ ] Human visual acceptance against the original Crystal scene.
- [ ] Whole-world image coverage for all map layouts and general on-demand
  resolution of image keys absent from native indices.

No full Candidate generation, native build, signing, installation, publication,
activation, service restart or account mutation was performed for this branch.
Paused goals retain their status.

Host-local proof root:
`C:/mir2-build/platform-windows-hotkeys-20261002/native-shop-resources-20261002`.
Profile receipts are `profile-powershell51.json` and `profile-pwsh7.json`.
`r12-shop-resource-audit.json` has schema
`mir2.native-shop-resource-audit.v1`, 3,238,358 bytes and SHA256
`d38a2d23365888e06c8bfb9b70ccd905e48bf0e81de94cfd9a1280cbb6baa56e`.
It records map types, signed layout hashes, representation/blend requirements,
map associations, full source pins and separate NoDraw/out-of-range lists.
The first helper attempt rejected a 65-character transcription of the expected
package pin. Its failure log is retained; the corrected, approved 64-character
pin passes with unchanged package inputs.
