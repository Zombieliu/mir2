# Bichon floor coverage repair

The user's ui-text client screenshot at Bichon (213,376) shows rectangular
black terrain holes. This is a new failure observation, not a passed map page.
The client executable remains c2a2d9f0c; assets are a shared mutable junction.

Decoding map 0 identifies missing Tiles frames 1001,1003,1005,1007,1008,1011,
1017,1018,1020,1022 in the surrounding viewport. Neither their original PNGs
nor atlas rects existed. The original Crystal Tiles.Lib has valid 96x64 frames.
For example, missing frame 1020 occurs at (210,364)/(210,368), and 1022 at
(210,366)/(210,370). The native producer skips absent rects, exposing the black
background. Existing atlas crops in the same area match source RGBA exactly;
no additional black-key transparency operation is justified.

The map-0 floor audit exported 2,076 absent source PNGs using exclusive create.
The updated raw atlas adds 394 frames; the standalone keyed manifest adds
1,682. Coverage comprises 1,049 raw and 4,418 standalone valid floor references
(5,467 total), excluding 18 original empty back-layer slots. Pixel verification
reports zero missing entries and zero RGBA differences. Map atlas budget tests
pass 8/8. This does not cover every decoration or animation; the keyed manifest
still reports 1,287 missing sources outside the repaired floor scope.

New hash-named pages were added and stable manifests replaced atomically.
All 4,647 previous immutable page references were retained and hash-checked.
No client process, character save, runtime rendering or alpha code was changed
as part of this resource repair. The running client caches manifests and needs
a normal-exit restart before visual revalidation.

Published manifest SHA256:

- map-atlas: A07AA3D50BAF34588520E0E2C74853B93EE73CFB46F8E7AAABBD47492DC6A179
- native-map-keyed: 59B7F11D90C06A8D456C16A82864FD730117B81592787DF335978DDC6773CBA3

Reproduction tools: `apps/web/scripts/audit-map0-floor-coverage.mjs` takes the
Crystal Data directory and optional `--report=<path>`; default is read-only.
`--export-missing` only writes absent originals. Rebuild atlas/keyed packs after
export, retaining old immutable pages when a live client shares the asset root.
`apps/web/scripts/verify-map0-floor-coverage.mjs <report>` checks actual packaged
RGBA against source PNGs. Local evidence is under C:/mir2-ui-repair-20260921:
map0-floor-source-coverage-after.json, map0-floor-final-verification.log,
map0-floor-budget-tests.log and map0-floor-pack-merge.log.

Live verification remains open: revisit the reported coordinates after restart,
walk across the affected region, and capture same-asset-version screenshots.
