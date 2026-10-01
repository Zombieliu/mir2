# Android original weight-bar package repair — 2026-10-01

Bounded NI-20 resource leaf; the Windows-completeness goal remains Active.
Frozen Windows: `3f5e61533235921369bc13a7760b4a56b0e467e5`.
Source parent: `9e29d7b67bead922ead99ae4179ae2e452f06e3e`.
This is the source/staging checkpoint **before** clean v13 APK construction.
Version13 /`0.1.10-weight-bars` is set, not yet an accepted artifact or display.
The exact v12 packages and missing471/473 images remain in the
[previous evidence](../native-android-player-ingress-20261001/README.md).

## Cause and bounded repair

The shared Windows HUD/BAG weight code uses original UI_32bit frames470..473.
Both actual v12 APKs omitted all four: numeric weight66/100 was visible, but
HUD473 and BAG471 produced real missing-image errors. This is Android staging,
not a request to replace the shared weight/UI rules or recolor fallback bars.

- `stage-shared-weight-assets.mjs` reads the four PNGs and metadata directly
  from the frozen Git blobs, copying them into a **new** ignored diagnostic pack.
  All13240 prior source files stay unchanged;13239 copied files are byte-identical.
  Only the cloned aggregate manifest changes. This is not a new raw-library
  export or image edit. PNGs, source metadata and the pack stay outside Git.
- Metadata version3 has library extent474 and exported count4:470/471 are84x6,
  472/473 are76x12, all offsets0. The Android guard requires these four, not474
  dense bitmap files. Other valid source frames remain bounded by library extent.
- Gradle checks integer/unique indices, exact paths, metadata/manifest agreement,
  required dimensions and zero offsets, PNG existence/byte bounds/signature/IHDR
  **before** decoding, then actual decoded size. Sync includes original PNGs
  and UI_32bit metadata. No shared renderer, wallet, gameplay, auth or server rules
  change; Windows workspaces and branch are untouched.

## Measured source gates

| Gate | Actual result |
| --- | --- |
| Sparse weight resource gate | 20/20: one valid input plus19 negatives |
| Existing Items/StateItem geometry gate | 13/13 on the supplemented pack |
| Existing MagIcon/MagIcon2 gate | 7/7 on the supplemented pack |
| Java Debug /uiPreview | 36/36 each, five suites each,0 failures/errors/skips |
| Actual Gradle Sync byte audit | 6647 PNGs and three metadata files match inputs |
| Bounded independent read-only source review | No remaining P0/P1/P2 reported |

`source-audit.mjs`/`source-results.json` bind the three source-file hashes, actual
control logs and XML, and staged image/metadata equality. Java/staging was run
with `--rerun-tasks`:44/44 tasks actually executed. The first Java command had a
wrong log-output directory and **did not start Gradle**; it is not counted. The
corrected actual run is `java-and-stage.log`.

`guard-before.log` retains actual BUILD SUCCESSFUL with a missing library,
followed by the failure-first assertion. `guard-after.log` is the intermediate
18/18 result; `guard-final.log` is the final20/20 including oversized-header and
wrong-signature controls. Each final weight control's actual Gradle log is kept.
Missing libraries/meta/PNG, malformed JSON, duplicate/fractional/missing indices,
wrong geometry/path, invalid PNG and manifest mismatches reject at this specific
guard, not due to an unavailable toolchain. All fixture edits are isolated in
ignored target; input byte digests remain unchanged.

Original manifest SHA-256:
`e14940711b944e54baa41254fca3f4b8e0ca0ad3e5d92e76e12a366823ca7735`.
Supplement manifest SHA-256:
`ea2a4cc8bf0a9e91f22213d4744587b0298d691021dcf405f51c553fd8632d3d`.
Frozen UI_32bit metadata SHA-256:
`9459d5a272a4a41ee0485a553ec3ce8f57996d5840cd99c8eae9b1dab5f14db9`.
Four individual original PNG hashes/dimensions are in `asset-identity.json`.

The reviewer compared source/frozen bytes/logs without running builds/tests or
operating a device. No Rust/shared gameplay source changed: v12 Rust257/preview270/
shared1203+8ignored results are historical, not freshly rerun for this resource leaf.
Gradle dependencies were resolved offline; wrapper network traffic was not traced.

## Remaining acceptance and publication

Next bind clean v13 source, both actual APK hashes/contents/installations, then
inspect original HUD/BAG weight bars in the actual emulator. Staging success alone
does not accept those images, startup performance, full original resource release,
all-phone controls/layout/IME, chat/NPC/services/remaining Windows models, real
HTTPS/WSS player flow or a physical phone. Existing v12 startup/Launcher failures
remain recorded, not erased by the resource fix.

Only emulator-5554 is connected. No approved live environment/account or physical
device exists for this run; empty Gateway diagnostics cannot accept online play.
The v12 evidence commit9e29d7b67 is currently local: ordinary SSH/HTTPS and verified
SSH443 attempts failed in transport. Fresh GitHub ref/PR reads still confirm only
f04a74399; PR253 stays Open/Draft, basecodex/playtest-registration. No publication
success, forced push, PR merge/base change, production deployment, save modification,
APK/raw-image/key commit or original workspace mutation is claimed here.
