# Mail compose source assets — 2026-09-21

Exported exactly seven missing original PNGs: Title671,674,607–609 and
Prguse2 203–204. Title metadata grew from344 to349 frames; Prguse2 from380
to382. All existing indices were retained. Export uses the existing
`export-crystal-ui.mjs --libraries Title,Prguse2 --skipExisting true` pipeline.

| Source frames | Actual bitmap size |
| --- | --- |
| Title671 letter | 236×300 |
| Title674 parcel | 236×384 |
| Title607–609 Send | 76×25 |
| Prguse2 203–204 stamp | 20×18 |
| Title193–195 Cancel, already present | 68×25 |
| Title676 attachment cover, already present | 144×36 |
| Prguse660 recipient input background, already present | 288×156 |
| Title200–205 recipient OK/Cancel, already present | 76×25 |

All18 listed frames were decoded and compared with original Crystal RGBA and
dimensions. PNG and RGBA SHA-256 hashes match metadata; local/global manifests
match. [Per-frame dimensions and hashes](mail-compose-assets.json). None is a
source-empty frame. Source code declares the stamp20×20 and cover144×33, differing
from the actual source bitmaps; this export does not stretch or substitute them.

The complete native UI asset audit passes after export:2,478 requirements,
2,396 drawable verified frames,82 original source-empty references,0 issues.
[Audit evidence](mail-compose-assets-native-ui-audit.json).

Source Title.Lib SHA-256:
`bd3e9485548e9b3cb5d4cb261c9a01a07752fb41fcea7f5d8569e602feaed648`.
Source Prguse2.Lib SHA-256:
`510ebc77315089075fc472c9ef745be2e16c6dece970c307d6859ce074cce861`.

No UI implementation, build, desktop operation or commit occurred. Live compose,
recipient input, sending and visual parity remain unverified.
