# Isolated local tile atlas — 2026-09-10

Starting HEAD `82e4724f9`, isolated Android worktree. No original checkout edits,
resource removal, download, production access, APK build or live session.

## Build

New Android wrapper `build-local-map-atlas.mjs` imports the existing Web
`packIntoPages` and raw-upload library selection functions. The existing CLI
could not be used unchanged: it restricts output to its original public root and
cleans stale artifacts. The wrapper instead requires a fresh output root outside
the source Web tree, never overwrites/cleans existing output, and caps library,
source/page counts and input image size. It preserves compact schema v2.

- Source Web root: `/Users/henryliu/obelisk/numeron/mir2-web3/apps/web` (read-only).
- Imported `scripts/build-map-atlas-pack.mjs` SHA-256:
  `0b3a10fd557f4300530ed9fc2521954a51be47882d5469434a5f8aa2908e98c1`.
  This is the inspected local helper version, not a claim of current branch equality.
- Output public root: `/Users/henryliu/obelisk/numeron-worktrees/android-player-journey/mir2-web3/apps/game-client/platform-android/target/local-world-20260910`.
- Manifest: `generated/map-atlas/manifest.json`, SHA-256:
  `b2ec2e3a73125e781ac8aeead31294e88e6510b1dfd62b2b00c2c2407ad609ce`.
- 10 libraries, 2111 source frames, 49 pages, 14785921 PNG bytes.
- Maximum compressed page: 441642 bytes; packer page-pixel budget: 262144.
- Build report is retained at output root `build-report.json`.

The pack covers only locally exported libraries allowed for raw tile upload.
It is not an exact Bichon-map dependency closure; keyed object/building images
are deliberately not packed as opaque floor images.

## Verification

- `node --check` wrapper: passed.
- Build with the two explicit roots above: exit 0;
  `/tmp/android-local-map-build.log`.
- Independent output check fully decodes all 49 PNGs and verifies hash filename
  prefixes, byte sizes, dimensions and all 2111 rectangle bounds:
  `/tmp/android-local-map-verify.json`.
- Reusing existing output: correctly rejected before writing, exit 1;
  `/tmp/android-map-existing-output.log`.
- Existing local packer tests: **5 passed, 1 skipped**. The skipped check expects
  a manifest at the original checkout's standard location, which was intentionally
  left absent; it is not counted as a pass. Log `/tmp/android-map-packer-tests.log`.
  Its cleanup test uses a temporary test directory, not project resources.
- `git diff --check`: passed. Runtime/Java tests were not rerun (tooling only).

No generated atlas bytes were committed. No APK now includes this pack, and no
map/entity GPU render or real player loop has been verified. Next resolve keyed
map-object production, raw-map dependency coverage and native producer/staging,
then render-ready; do not switch InGame just because this build passed.
