# Local Bichon standalone map images — 2026-09-10

Starting HEAD `d85a6c917`; no runtime source changed. Original checkout remains
read-only. No production resource request, full-pack download, APK or GPU run.

## Exact source recovery and execution

Local-only Git access confirmed the generator blob was absent. The existing
GitHub connector successfully read it at handoff commit
`58eab9a4a6684b2d2d11d2c533049b64995e7458`; no repeated failing Git transport.
Returned blobs match the current Android tree, and `git hash-object` verified
the materialized target-only mirror:

- `build-native-keyed-map-pack.mjs`: `68caa961dd8fadea7586112b2613a9fc6c00d07d`.
- `scene-alpha-key.ts`: `d8242631d561b94c26573a49dc95daef1be691dc`.

Mirror root: Android crate `target/keyed-source-jYmMN9`, with read-only dependency
resolution through the original Web node_modules. Source files were added using
apply_patch without modifications. They and generated resources are not committed.

The current generator preserves local Crystal PNG RGBA bytes, for both normal
and additive frames. Despite the legacy keyed name/import, it does NOT rerun
black-key/feather on local exports; doing that would damage dark roofs/walls.

Called `buildNativeKeyedMapPack` with mapFileNames=["0"], explicit original
checkout original-map/map-pack/starter-region paths, and
fullPackFallbackMapFileNames=[] to prohibit production fallback. Output used the
fresh mirror's default public/generated/native-map-keyed root. Removed artifacts=0.
The unchanged default missing-source budget is2969, not a zero-missing gate.

## Output and independent checks

- Raw map decoded as700x700 using the exact generator parser.
- Full-map standalone references7672, emitted4703: normal4520/additive183.
- Missing sources2969; full-pack entries0; no-draw classifications0.
- Referenced image bytes28932323 (not necessarily deduplicated disk total).
- Manifest SHA-256 `9ca5d062e039847823918b9a9956df2bb9e148b69369a945891e510c2a2660b5`.
- Output: `/Users/henryliu/obelisk/numeron-worktrees/android-player-journey/mir2-web3/apps/game-client/platform-android/target/keyed-source-jYmMN9/public/generated/native-map-keyed/manifest.json`.
- All4703 entries independently compared byte-for-byte with source PNGs and
  checked against their full content-addressed SHA-256 filename: pass.
- Logs: `/tmp/android-keyed-map-build.log`, `/tmp/android-keyed-map-verify.json`.

Conservative offline sample x270..334/y580..690 (65x111) around the historical
coordinate302,634:565 standalone references, six missing:

- WemadeMir2/Objects6#1040
- WemadeMir2/Objects6#1047
- WemadeMir2/Objects6#1051
- WemadeMir2/Objects6#1053
- WemadeMir2/Objects6#1054
- WemadeMir2/Objects6#1055

This is not the exact runtime viewport/draw list or an authenticated position.
Do not relabel missing images as legitimate no-draw without source evidence.

## Next boundary

Tile and standalone object resource outputs now exist, but remain outside the
APK. Next connect the actual native map draw-list producer, identify which
missing sources affect the initial rendered viewport, and preserve an explicit
incomplete-resource gate. Recover further missing source blobs through the working
connector at an exact known ref and verify against local-tree blob IDs; no need
to keep retrying the failed Git transport. Shared extraction still needs a declared
bounded write set. No gameplay/render-ready/whole-map acceptance is claimed.
