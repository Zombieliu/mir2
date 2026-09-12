# Android shared scene model ingress — 2026-09-10

Source `ced102ba900aae5a6fe30392fc19e89d00c6eb40`, local-only isolated Android
worktree/branch. Two source files changed: Android `world_projection.rs` and
`shared_shell.rs`. No shared source, Windows/backend, account or resource changes.

## Implemented

The validated authoritative snapshot now also produces the actual shared
`MapModel` and `EntityModelSet` types and queues them through existing runtime
entry points. IDs, names, kind, level, direction and coordinates are preserved.
Terrain schema is checked by shared serde before queueing. Invalid scene center,
direction type or terrain kind rejects projection instead of publishing malformed
models. Queue failure follows the existing reset/disconnect path.

Map center uses server sceneView.center. When sceneView is absent/null, framing
uses the already validated authoritative self position, not a synthetic (0,0).
This is camera/read-model presentation, not client authority or movement logic.
Light settings 0..4 are projected; unknown settings remain absent.

No Mir2MapPlugin/Mir2EntitiesPlugin placeholder-color renderers are installed.
These models are not map atlas draws or character sprite sheets. The existing
world-data receipt remains distinct from full scene/GPU readiness, and InGame
is not forced.

## Verification

- Android host tests: 94 passed, `/tmp/android-scene-model-host.log`.
- UI-preview host tests: 97 passed, `/tmp/android-scene-model-preview.log`.
- API31 arm64 target check: passed, `/tmp/android-scene-model-target.log`.
- `git diff --check`: passed. Existing dependency warnings retained.
- New/extended assertions cover source center, authoritative fallback, patches,
  light setting, both self/remote entity positions and shared-schema rejection.

Java and shared-runtime suites were not rerun; their earlier results are not
new evidence for this increment. No APK rebuild/install, screenshot, emulator
or physical-device test. Last APK remains source `af5acf6ad`, not this source.

## Resource prerequisite check

Android `target/shared-ui-assets` has only `original-ui` at its top level.
The following documented renderer inputs were not found in either the Android
worktree or original checkout (original checkout was inspected read-only):

- `apps/web/public/generated/map-atlas/manifest.json`
- `apps/web/public/generated/native-map-keyed/manifest.json`

Android worktree `apps/web/public/generated/crystal-packs/full/index.json` was
also absent. This does not prove that no authorized resource pack exists elsewhere
on the Mac. No broad disk scan, download or environment change was performed.

Windows map_parser uses its assets resolver for `.map.gz`, map-atlas and keyed
manifest lookup. Next inspect the configured authorized resource root/consumer
setup and make a bounded portable producer adaptation. Do not copy Windows paths,
accept server mapFileName as an unchecked path, or substitute fallback colors for
the actual map. Resource availability and render-ready still require evidence.

Approved live Gateway is still missing; physical-device acceptance remains
deferred until playable. No online loop or global completion claim.
