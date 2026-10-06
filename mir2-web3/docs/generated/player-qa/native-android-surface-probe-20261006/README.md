# Native Android bounded surface probe — 2026-10-06

Diagnostic source: `951a9ed717019273011454fbda3a627261b2244a`, parent `6c12eac0c400e81cc4bb7e0c8216b991be7dce6f`. This stage adds opt-in observations only; **no framebuffer behavior fix is implemented**. The full Windows-alignment goal remains **ACTIVE**, NI-17 **PARTIAL**, with AP-01–21 unchanged. Functional lighting implementation remains the separately evidenced c57/1f lineage.

## What this records

Three code files: Android-only surface configure/acquire/present/lifecycle hooks, one new diagnostic module, and a non-Android test-only inclusion of that same module. With `MIR2_ANDROID_GPU_SURFACE_PROBE` unset, empty or anything except exactly `1`, the observer is not called. A process-global atomic budget stops at128; surface/lifecycle changes never reset it. The native HAL guard reads existing descriptor/handle metadata only, is dropped before ordinary rendering continues, and makes no GL calls.

Shared render bodies outside these hooks and the runtime body outside the test inclusion byte-match the parent. The other443 build input files are protected. Total446 inputs are bound to the exact clean source; input-list SHA-256 `d4cefe59d3a41d526370ca9f283c15fb86cbdae81b65c0b9031e6753d44c706e`. The original two worktree Git checkpoints remain unchanged; their untracked directory contents are not recursively hashed.

This does not modify renderer scheduling, lifecycle draining, palette/shader/light rules, UI layout, authentication, protocol, dependencies, or Windows/WASM behavior.

## Compiled regression and applicable gates

The new budget regression genuinely compiled: first3 passed /5 failed, then8/8 passed after bounded atomic ticket allocation. Two earlier attempts selected non-workspace test package entry points and did **not** compile; they are retained separately and are not counted as product or GPU red tests. A commit-audit parser failure also exited before staging; its separate record is retained.

Fourteen clean-source bound checks pass:

| Gate | Result |
| --- | --- |
| Android normal / preview Rust | 529/529; 577/577 |
| Shared native UI | 1324 passed, 10 existing ignored |
| Runtime, including8 budget tests | 309 passed, 1 existing ignored |
| Fresh Java Debug / uiPreview | 103/103 each,8 classes each, no failures/errors/skips |
| API31 arm64 checks with actual diagnostic option enabled | Normal and preview pass |
| Mac-host Windows original lighting / map / effects | 14/14; 4/4; 2/2 |
| Mac-host Windows protocol / Hero FIFO / skill FIFO | 21/21; 1/1; 1/1 |
| Actual approved Bichon resource gate | Pass;607 tiles,7 atlases, unresolved draws0 |

Focused/precommit counts overlap; do not add them to these totals. Mac-host Windows checks are not the full Windows OS gate. This tests the diagnostic budget, **not** a regression fix for GL0x0506.

## Exact diagnostic APK

Only uiPreview is newly packaged in this stage; do not rebind previous normal/c57/1f APKs to this source. Native Bevy, arm64-v8a, Debug/uiPreview code35/name `0.1.32-gameshop-phone`; Rust release profile and explicit `MIR2_ANDROID_GPU_SURFACE_PROBE=1`. Gateway URL remains empty and preview networking is disabled. This is not WebView, live gameplay, Store release or device acceptance.

- Bytes: 542071047
- SHA-256: `4c3a5bd0b01ac1e1ef23e11eeeef975b9fe688de1bb3277b12c71401a18df291`
- Native ELF SHA-256: `be688a6d0d22970ce8c4eb9e47c33ecb6afb6c20ebd74090571979b13a0d7e1d`, matching this variant's stripped build output.
- Local path relative to repository: `mir2-web3/apps/game-client/platform-android/target/gpu-framebuffer-20261006-edydFW/final-apks/mir2-native-surface-probe-preview-951a9ed71.apk`.

All selected6647 UI PNGs,3 metadata files and10 original Lighting PNGs byte-match approved local/frozen Windows resources. This is not complete resource release: whole-map2969 missing references remain open. Data-preserving install and pulled installed APK SHA match; no wipe/uninstall/log clearing was done.

## Actual first-resize evidence

Dedicated `Mir2_API_31_ARM64`, emulator-5554, Android12/API31, sdk_gphone64_arm64, arm64-v8a, GLES ANGLE SwiftShader CPU. No physical device is connected. Five independent cold-start samples after8s, all target resumed Activity/PID alive, fatal0, no render validation/AppExit, no missing-asset/cursor-warp log in these samples. All five original2340×1080 PNGs were inspected; bounded offline Bichon floor, self/torch monster/FireWall and local light pools are visible, with still-incomplete phone fixture controls.

| Scene | PID | GL0x0506 | Surface configurations | Unique probe records |
| --- | --- | ---: | ---: | ---: |
| Night repeat1 | 14753 | 5 | 2 | 128 |
| Night repeat2 | 14831 | 1 | 2 | 128 |
| Night repeat3 | 14910 | 5 | 2 | 128 |
| Dawn | 14991 | 0 | 1 | 128 |
| Day | 15071 | 0 | 1 | 128 |

Observation: all three failing night samples configure3520×1980, acquire/present renderbuffer3, then reconfigure2340×1080 and acquire renderbuffer5. Errors occur **between acquire and present on that first reconfigured frame**, before Java fixture injection and before lighting material activation. The acquired HAL descriptor is Rgba8Unorm/internal32856, while the guest GL encoder reports a color RBO format0/incomplete attachment. Only initial Running lifecycle is consumed before these errors. Dawn/day directly configure actual2340×1080 in these samples.

Inference: investigate retained render-pass output attachment handling during size changes next. The exact failing pass and stale GL attachment are **not yet proven**. Do not claim that all lifecycle or cross-thread issues are excluded. The diagnostic can change timing; its zero-error samples never substitute for uninstrumented acceptance.

Total11 GL0x0506 entries: zero-error GPU gate remains **FAIL**. This stage is diagnostic progress, not a successful behavioral repair. Previous8-error/failed-render evidence and APKs remain separately retained.

Raw ignored evidence: `mir2-web3/apps/game-client/platform-android/target/gpu-framebuffer-20261006-edydFW` (exact binding, compiled red/green, earlier entry failures,14 bound gates/XML, package/resources/install, device, five raw logs/PNGs, original visual review and first-resize triage). [Compact source and delivery evidence](source-evidence.json). Raw APKs/logs/caches/keys are not committed or portably embedded in this document.

## Full goal boundaries

Read-only remote refresh `2026-10-06T01:58:10.512Z`: selected Windows `codex/playtest-registration` at `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`; frozen functional `3d735745f1117d42a7859e87604a106351dca935`; +1 commit,83 documentation paths,0 functional paths. [PR #253](https://github.com/Zombieliu/mir2/pull/253) remains Open/Draft, base `codex/playtest-registration`. Only normal Android-branch pushes are allowed; no Windows push, production deployment or PR merge.

Still open: zero-error GPU and uninstrumented resize/resume, whole phone UI, complete map/object/spell/action/interpolation coverage, authenticated HTTPS/WSS/real online player loop, shared Zone movement and saved re-entry, nativeResumeV1/network/audio, approved complete resource/cache/update delivery, physical Android and human frontend acceptance. Offline fixtures are not credentials, authority, inventory settlement or real saved state.
