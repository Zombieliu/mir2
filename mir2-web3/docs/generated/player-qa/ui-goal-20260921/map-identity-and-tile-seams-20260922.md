# Map identity and moving floor seams

User evidence on daylight client e3d32fa9f shows DeadMineEntrance scenery and
HUD title, but the green Bichon minimap and quest card still naming Bichon.
These are inconsistent projections of the same transfer. Entering D401 is
not the OmaCave objective; no quest completion or save was manufactured.

The Windows packet cursor previously retained only the minimap index, only
for explicitly matching snapshots. Partial metadata and delayed source-map
snapshots could replace the retained world used by the other projections.
The cursor now retains destination filename, title, index and map metadata;
partial/matching snapshots inherit this identity, while explicit source-map
snapshots cannot restore old scenery/actors. Session reset clears the cursor,
and omitted/zero destination minimaps clear the source image. Quest and
player/hero skill receipts are still delivered through their existing paths.
Opt-in `mapIdentity` trace markers record the transfer and accepted/rejected
snapshot identities without repeated per-frame logging.

Full Windows serial tests pass **701/701**. The new regressions include the
ordinary MapInformation-only -> UserLocation transfer, stale source snapshot
with a quest receipt, partial/same-file snapshots with old titles, MapChanged,
zero/missing minimaps and reconnect. Two initially invalid new fixtures were
corrected to the actual requestId and flat UserLocation wire fields before
the successful full rerun. Native release build passes. Test/build logs:
`C:/mir2-ui-repair-20260921/map-identity-full-tests-rerun.log` and
`map-identity-client-build.log`.

## Floor sampling evidence

The separate running-grid artifact is reproduced using the real Bevy 0.19
Sprite/TextureAtlas GPU renderer and readback, with nearest sampling, adjacent
48x32 tiles, a 128x128 atlas and the production Sample4 camera setting.
Eight warmup updates and four opaque/colored interior controls per frame
prevent empty startup images from masquerading as successful measurements.
The 48-frame matrix covers horizontal, vertical and diagonal camera offsets
0, .25, .5 and .75; transparent versus copied-edge gutters; Sample4 versus Off.

Transparent gutters with Sample4 produce up to 147 partial-alpha seam pixels
and minimum alpha 64. Copied-edge gutters with the same Sample4 setting produce
zero seam pixels and minimum alpha 255 in all 12 motion phases. An earlier
100x67 diagnostic also showed a half-pixel issue with Off, so disabling MSAA
alone was not adopted. The production camera and continuous movement remain
unchanged. This is a controlled GPU reproduction, not human visual acceptance.

The map atlas generator now assigns each source its own one-pixel border on
all sides, copies exact edge/corner RGBA, and uses two-pixel spacing between
frames. Source rectangles and original files remain unchanged. Old hashed
pages are retained during this live preparation; the new manifest is switched
atomically only after every page is written. The old client can finish using
its original cached manifest/pages. New client startup reads the new manifest.

Node atlas/gutter/release tests pass **12/12**, including current-pointer
selection while old immutable manifests remain for existing clients. The
normal atlas test/verify commands include these regressions; the explicit
`verify:map-atlas-gutters` command checks all generated source pixels.
Full generated-resource verification:
**79 pages, 2952 frames, 13,148,168 source pixels preserved, 783,028 copied
edge/corner pixels verified**. Manifest SHA256:
`5c20ccafe6a9b2be09f50c6b6b6dbee91715ccd321792acaafbce7f822812fe5`.
The generator still reports 19 existing near-black source frames; this change
does not substitute or reinterpret those original pixels.

Reproduction commands from the project root:

```powershell
cargo +1.95.0 test --manifest-path apps/game-client/platform-windows/Cargo.toml --offline -- --test-threads=1
cargo +1.95.0 run --manifest-path apps/game-client/runtime/Cargo.toml --example tile_seam_probe --no-default-features --offline
npm --prefix apps/web run test:map-atlas-budget
node apps/web/scripts/build-map-atlas-pack.mjs --preserveExisting true
node apps/web/scripts/audit-map-atlas-gutters.mjs
```

An initial GPU build exhausted the E: development cache volume. Only the
runtime target/debug build cache was moved to C:/mir2-build-cache-20260922
and linked back; source, assets and saves were not moved. The first GPU
capture was empty before readiness and was discarded as inconclusive;
the final matrix enforces positive controls. Evidence JSON accompanies this
record. Deployment and native visual checks remain pending normal logout.

Prepared client source `192e14ce14a9bfb248b4a03c96121e84964a86e8`, package
`C:/numeron-legend-of-rebirth-20260923-map-identity-seams`.
Client SHA256 `B94737CB547F574C77BE609E1F44F5FC230A18C91327390A22E70FD0B4B5D476`.
Gateway remains deployed source `1b60931f7` / PID42392; no service or store
restart is needed. The retained daylight config SHA256 is
`01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.
The candidate manifest pins the new map atlas hash and records the shared
mutable asset junction. A live-process check still found old client PID44840;
normal logout/close was requested only after packaging to preserve progress.
`deployed=false`, `visualAccepted=false` at this checkpoint.

After the user's explicit normal-exit/switch-for-personal-testing reply,
the old client log recorded the confirmed exit and successful event-loop
return. The verified candidate launched at 2026-09-23 00:07 Asia/Shanghai,
PID45952, with render/movement prefix
`C:/mir2-ui-repair-20260921/render-live/20260923-000717-726`.
Startup connected to the existing Gateway and retained force_daylight=true.
`deployed=true`; the user retains gameplay control. Actual map-transfer and
running-scene acceptance remain `visualAccepted=false` pending their test.
