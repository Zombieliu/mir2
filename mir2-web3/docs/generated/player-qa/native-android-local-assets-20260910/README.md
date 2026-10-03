# Local world-resource prerequisite audit — 2026-09-10

Android worktree was clean at starting HEAD `eda09dcbe`. Original checkout was
read-only. No resource copying, download, credentials, environment edits, APK
build or install. No gameplay or visual-acceptance claim.

## Located inputs

- Public root: `/Users/henryliu/obelisk/numeron/mir2-web3/apps/web/public`
- Raw map pack: `/Users/henryliu/obelisk/numeron/mir2-web3/apps/web/lib/generated/crystal-map-pack`
- Entity manifest `bevy-entity-atlases/manifest.json`, generatedAt
  `2026-08-12T14:01:21.740Z`, SHA-256
  `1b1420e04690f1da84930a49a7fa62ee70062e5876053e41ca9444a034f1b54a`.
- Starter atlas: `starter-bichon-base`, 7 PNG pages, 13,164,823 compressed image
  bytes, 9,650 rects. Every page matches its manifest size/SHA-256 and IHDR
  dimensions; rect page/geometry bounds pass. This is not full PNG pixel decode.
- `0.map.gz`: 512,762 bytes; SHA-256
  `bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5`.
  Gunzip passes to 12,740,008 bytes, decoded SHA-256
  `ed4783215ffa989658f79892c2dd6720753fb111e1102cb14da8e6182d88d7f6`.
  Map format/cell semantics are not parsed by this preflight.

The new bounded read-only `apps/game-client/platform-android/audit-world-assets.mjs`
reproduces the check using explicit PUBLIC_ROOT and MAP_PACK_ROOT arguments.
It restricts page names, rejects escaping symlinks, caps input sizes/counts and
gzip output, and performs no staging or network calls. JSON result:
`/tmp/android-local-world-assets.json`.

Verification: `node --check` passes; existing resources exit 0; missing arguments
exit 2; selecting the UI-only Android staging root exits 1 as expected. Failure
logs: `/tmp/android-assets-no-args.log`, `/tmp/android-assets-ui-only.log`.
No Rust/Java suites rerun because no runtime source changed.

## Remaining prerequisites

The checked public root still lacks `generated/map-atlas/manifest.json` and
`generated/native-map-keyed/manifest.json`. Original-map PNGs exist, so next work
can inspect the existing generators and produce a bounded local Bichon resource
set without assuming the whole full pack is absent. Android Gradle currently
stages only original-ui categories, not these entity/map inputs.

`MIR2_NATIVE_ASSET_ROOT` was unset. The exact current-branch Windows assets.rs
blob (`0deeb9507c127632967a42690371c8227368233e`) is absent from the partial clone.
`git show HEAD:.../assets.rs` triggered a promisor fetch which timed out with
`Recv failure: Operation timed out`. No repeat fetch/proxy change was attempted.
The original checkout contains an older 2,753-byte resolver, inspected only to
discover the env variable and public-root sentinel. Do not treat that source as
the Android branch's map-relocation fix. Prefer local-only reads for subsequent
discovery and explicitly record any unavailable source.

Integrity of existing bytes does not prove an approved current release or
Android render readiness. No full pack was fetched, no production endpoint used,
and no old APK is relabelled as this result. Continue resource producer/staging
and render-ready implementation; approved test Gateway remains needed for live
verification, with physical-device acceptance deferred until playable.
