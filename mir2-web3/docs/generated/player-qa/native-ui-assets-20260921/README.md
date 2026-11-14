# Native UI asset closure, 2026-09-21

The keyboard settings renderer waits for Prguse2 frames 190, 191, 192, 201,
and 202 before drawing its panel. All five were absent from both exported
assets and the export manifest. Restored those exact original frames.

The NPC service UI repair additionally requires Prguse2/351 (176×147) and
Title/290–295 (48×25 confirm/hold button states). Restored these seven frames.
No Rust files were changed by this asset repair.

All 12 PNGs came from the existing exporter, using the original Crystal
Data libraries in a separate temporary output directory. Only the required
frames were copied into the project. Their original frame metadata, RGBA and
PNG SHA-256 hashes were merged into each library's metadata and the generated
summary. Export configuration includes all 12 indexes for reproduction.

## Verification

Run from the repository root (supply your original Crystal Data directory):

```powershell
node apps/web/scripts/audit-native-ui-assets.mjs --dataDir E:/mir2/Crystal/Build/Client/Debug/Data --report docs/generated/player-qa/native-ui-assets-20260921/audit.json
```

The checked-in script discovers native Rust asset-path literals, library/index
literals, frame and button specs, button tuples/structs, literal preload vectors,
and simple required-frame array/range loops. It also checks every configured
index in nine UI libraries: Prguse, Prguse2, Title, ChrSel, BuffIcon, GuildSkill,
MagIcon, MagIcon2, and MapLinkIcon. Windows and Android platform sources are
inside the scanned game-client tree.

The final run checked 2,443 unique configured/discovered indexes: 2,361 positive
original frames decoded and matched source pixels, dimensions and available
metadata hashes; zero repairable issues remain in this scope. Mask pixels are
checked when the original frame has a mask. The source library hashes are in
`audit.json`. Without `--dataDir`, PNG/metadata checks still run but original
empty slots cannot be classified and will correctly appear as failures.

The audit initially failed on the five absent keyboard assets, then passed
after restoration. Prguse2/351 and /190 were also visually inspected as the
original service panel and keyboard field respectively.

## Original empty slots and limits

82 configured Prguse indexes have no positive original frame: 81 still have
legacy invalid zero-dimension PNGs and one has no PNG. All are enumerated in
the report with their exported state. None is a static native UI reference
found by this scanner. They are recorded as source exceptions rather than
replaced with invented imagery. A newly discovered direct UI reference to an
empty slot makes the audit fail.

Preview animation base-plus-frame arithmetic, dynamic server-selected
item/creature icons, computed skill indexes, and service-mode-selected titles
are outside static extraction unless written in a supported literal, spec,
array, or simple range form. The complete configured MagIcon/MagIcon2 ranges
are still checked. The audit covers the nine named UI libraries,
not the entire game art catalogue. It does not prove live window interaction,
button placement, DPI behavior, package distribution, or Crystal visual parity.
Native keyboard/NPC screenshot acceptance remains part of the parent UI task.
