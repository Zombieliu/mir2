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

## Classification and verification follow-up

The 5,467 count above initially proved resource/pixel closure, not successful
native routing. A read-only review found that `SmObjectsc/2766.png` at map-0
front cells (0,28) and (0,52) was present in the repaired keyed pack but excluded
by both the builder and native library classifier. The prior `smobjects` rule
accepted only the plain or numbered Mir2 form, omitting the actual Mir3 `c`
suffix. This did not affect the ten Tiles frames around the user's screenshot.

The builder and native classifiers now also accept the real `SmObjectsc`
family, including Shanda's `cwood`, `csand`, `csnow`, and `cforest` suffixes.
No RGBA data or blend behavior changes. A normal local full rebuild now discovers
all 2,076 added PNGs: 394 raw frames and 1,682 standalone references. Source
classification and actual standalone selection have Node/Rust regressions;
the Node pack suite passes, while Rust execution is pending the parent's serial
Windows test run. The running binary still needs the rebuilt classification code.

The pixel verifier now rejects empty/incomplete reports, missing libraries,
duplicate report/manifest keys, unsupported render routes, out-of-bounds atlas
rectangles, and target PNG dimensions inconsistent with their manifest. Its two
Node regression tests pass, including equal-length RGBA buffers with transposed
image dimensions. It still requires a freshly generated audit report: it does
not independently bind the report to current map/Lib hashes or re-decode every
existing source PNG from the original Lib. Pixel closure and runtime/live visual
acceptance remain separate checks.
