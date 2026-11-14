# Whole-map resource closure, 2026-09-22

The Bichon (659,215) rock-wall gap and DeadMineEntrance D401 (26,180)
missing cave walls extend beyond the earlier floor-only repair. Audit coverage
now includes every back/middle/front reference and every decoded animation
phase in maps 0, D001, D401, 1, D021, and D022. It does not claim all game maps.

The six-map union contains 12,001 valid original Lib frames. The repair exports
2,609 absent original PNGs with exclusive create, leaving existing files intact.
The final fresh audit reports zero missing valid PNGs, zero missing source Libs,
zero unsupported routes, and zero invalid middle-library indices.

| Map | Valid unique frames | Missing before | Original no-draw slots |
| --- | ---: | ---: | ---: |
| 0 | 8,483 | 1,049 | 274 |
| D001 | 939 | 436 | 3 |
| D401 | 856 | 350 | 1 |
| 1 | 2,953 | 640 | 980 |
| D021 | 366 | 360 | 0 |
| D022 | 391 | 384 | 0 |

Frames overlap across maps, so table rows must not be added to obtain the union.
The union has 1,154 original empty/out-of-range Lib slots; these are recorded as
no-draw and never fabricated. Of 114 animation families, 37 are wholly valid,
67 wholly original no-draw, and 10 mix valid frames with original no-draw slots.
Native handling of those mixed families remains a separate renderer parity
question; resource closure does not prove matching animation timing or fallback.

D401's inspected x=12..40, y=168..196 viewport is inside its 200x200 map.
The extended y<=199 region contained 123 unique unbundled keyed object frames,
including Objects2 frames 7033/7032/7031 at (18,176)/(18,177)/(18,178).
Their source PNGs existed but the old keyed pack only selected map 0.
This explains absent cave walls independently of normal black space outside
the map boundary. Bichon's missing tall frames include Objects21 frame 2133
at (658,213). Both cases are included in the whole-map repair.

Full raw and six-map keyed packs were built in staging. Validation compares
all 12,001 original Lib RGBA frames to original PNGs and then to packaged
pixels: 1,473 raw atlas routes and 10,528 standalone keyed routes. Both staging
and published canonical verification report zero differences or missing routes. Atlas budget tests
pass 8/8 and the keyed pack regression suite passes. No Cargo was run here.

Publication copies immutable hash pages before atomically replacing each stable
manifest. All 6,455 old page references remain available and hash-verified.
Stable manifests are replaced individually, not as a two-file transaction;
retaining both generations of pages and all old keys makes this transition safe.
Published SHA256:

- map-atlas: `cde58ce457106e28cb6ef183e0980a599335ab4bd3429c54c717836c23bc9be3`
- native-map-keyed: `d20c6ac0bd96cced7b24a45d4e45d40c00ed56fcc93240c10bd1ad80a6906a69`

Reproduction (run in `apps/web`, substituting the local Crystal Data/evidence
paths; staging keyed output must satisfy the builder's temporary-directory guard):

```powershell
node scripts/audit-map-resource-coverage.mjs --data=<CrystalData> --report=<audit.json> --export-missing
node scripts/build-map-atlas-pack.mjs --outDir public/generated/map-atlas-<new-stage>
node scripts/build-native-keyed-map-pack.mjs --maps 0,d001,d401,1,d021,d022 --outputRoot <temp-native-map-keyed-stage>
node scripts/verify-map-resource-coverage.mjs --report=<audit.json> --data=<CrystalData> --atlas=<staged-raw-manifest> --keyed=<staged-keyed-manifest> --result=<verification.json>
node scripts/publish-map-resource-staging.mjs --atlas=<staged-raw-manifest> --keyed=<staged-keyed-manifest> --backup=<new-backup-directory>
node scripts/verify-map-resource-coverage.mjs --report=<audit.json> --data=<CrystalData> --result=<published-verification.json>
```

Use a fresh audit report: verification does not cryptographically bind the
report to map files or independently reconstruct the reference set. Source Lib
comparison is optional in the command but was enabled for this repair. Do not
build directly into an in-use canonical atlas directory: the existing builder
prunes outdated hash pages in its output directory. Build in staging and publish.

Local evidence is under `C:/mir2-ui-repair-20260921`: `all-map-resource-before.json`,
`all-map-resource-export.json`, `all-map-resource-after.json`,
`all-map-stage-verification.json`, `all-map-published-verification.json`,
`d401-viewport-keyed-before.json`, builder/test logs and publication backups.
Generated packs are ignored build products; the committed source PNGs and
scripts are required to reproduce them.

No runtime rendering/alpha, map identity, input, character state or desktop
process was changed in this resource repair. The running client caches manifests
and needs a normal-exit restart to load the new resources. Same-version visual
acceptance at both reported viewports remains open, as do separate stale map
identity and animation-family renderer investigations.
