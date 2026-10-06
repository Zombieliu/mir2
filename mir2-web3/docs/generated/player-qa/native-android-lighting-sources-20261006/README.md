# Native Android shared lighting sources — 2026-10-06

Current product source: `1f01c13519f369af1ff1f43a166c12053072e122`, parent shared-source implementation `c57a3cf06c8b8021617426b1d0e45137af56c25e`. Full Windows-alignment goal is **Active**, NI-17 **PARTIAL**; the AP-01–21 denominator is unchanged. This is a native Bevy diagnostic APK, not WebView or live gameplay.

## What is implemented

- Original Windows light-source producer and map-cell hook are shared, with thin Windows adapters. The original 200-source cap, 40×41 map scan, source priorities, ranges/colors, spell/dead-owner rules and offsets are retained. Original Windows lighting/map/effect test bodies are byte-unchanged; no second combat/trade/save/auth rules are created.
- Android loads actual light cells and ten original Lighting PNGs from the approved local pack; effects use optional actual catalogue light metadata. A matching world-asset request, real render-ready receipt and committed presentation pose are required. Stale generations, scene/session changes, bad inputs and queue backpressure remain bounded.
- Lighting producer now runs after pose commit and before the runtime consumer in the same frame. Four preview-only Java scenes traverse the original MainActivity/nativeEvent boundary. Normal build does not inject them; both Gateway URLs are empty and preview networking is disabled.
- Actual API31 runtime exposed missing `VIEW_FORMATS`: the first night/dawn/forced-dark renderer exited while the process remained alive. The two-file fix creates a direct `Rgba8UnormSrgb` target on unsupported adapters, preserving the original sRGB render/sample format. Supported adapters and non-Android retain the original alternate-view descriptor. Shader/palette/light/game rules are not changed.

## Exact source and regression evidence

First stage: 23 code files, 444 inputs, SHA-256 input-list `b8c18e381ac453711a90a850b60f512c38a94ab4b3a0ff4b506438b7e9798142`; 421 baseline whole files protected. Repair: two scoped runtime files (one overlap, 24 unique files total), 445 inputs, list `7701d8f968f342b280051a424bd82b03577f334c4c3d3840e427d6abc780a65c`; 443 other whole files and runtime bytes outside the target-allocation hook protected. Original two worktree Git checkpoints are unchanged; original untracked directory contents were not recursively hashed.

Four appended preview scenes require the explicit catalogue count change 61→65; the old 61-name prefix and other old tests are preserved. Old Java authentication/session callbacks and assertions are unchanged in both stages.

| Final clean-source gate | Result |
| --- | --- |
| Android / ui-preview Rust suites | 529/529; 577/577 |
| Shared native UI | 1324 passed, 10 existing ignored |
| Runtime | 301 passed, 1 existing ignored |
| Fresh Java Debug / uiPreview XML | 8 classes, 103/103 per variant, no failures/errors/skips |
| API31 arm64 normal / preview checks | Both pass |
| Mac-host Windows original lighting / map / effects | 14/14; 4/4; 2/2 |
| Mac-host Windows protocol / Hero FIFO / skill FIFO | 21/21; 1/1; 1/1 |
| Actual approved local Bichon-resource gate | Pass; 607 tiles, 7 atlases, unresolved draws 0 |

These are fourteen final bound gates. Focused/pre-commit runs overlap and are not added to totals. Mac-host Windows tests are not the full Windows OS gate. The resource test ran with the actual approved world root, not an absent-environment early return.

Compiled first producer red: 7 passed / 6 failed (exit101); only 434 old inputs were hash-captured in that initial red, not all new files. First green attempt: 12 passed / 1 failed, corrected in the new visibility-reset test only. Final focused producer14 passed. Compiled GPU descriptor red: 2 passed / 1 failed (exit101), then the final runtime301 gate passed. All failure logs/raw APKs are retained; no old assertion is weakened to count a pass.

## Installed packages

Both packages are diagnostic Debug/uiPreview, code35/name `0.1.32-gameshop-phone`, arm64-v8a. Data-preserving installation and pulled APK byte hashes match; their native ELFs match each current variant's stripped build output. Native-library stripping warnings remain; no Store/Release size or release-signing claim.

| Variant | Bytes | SHA-256 |
| --- | ---: | --- |
| Normal | 534025319 | `d6961d31cd1063de438918321eaa2a6576acddd4c5d760d33e4a852942fab45f` |
| uiPreview | 542058471 | `d5b83e2ce86007538f19fbce5e57b221579b40f00d235412a8cdef7839dc301f` |

Local packages relative to repository root:

- `mir2-web3/apps/game-client/platform-android/target/lighting-target-20261006-LSNH5q/final-apks/mir2-native-lighting-target-debug-1f01c1351.apk`
- `mir2-web3/apps/game-client/platform-android/target/lighting-target-20261006-LSNH5q/final-apks/mir2-native-lighting-target-preview-1f01c1351.apk`

Per APK, all selected 6647 UI PNGs, three metadata files and ten Lighting PNGs byte-match. UI_32bit470–473 and Lighting0–9 also match frozen Windows Git. Entity manifest SHA `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`. This selected/local check is **not** a complete approved resource release: the whole map still reports 2969 missing references.

## Actual emulator images

Dedicated `Mir2_API_31_ARM64`, emulator-5554, Android12/API31/sdk_gphone64_arm64, arm64-v8a; GLES ANGLE SwiftShader CPU. One emulator and no physical devices are connected. These are five independent cold launches, sampled after8s, with installed SHA, current source hashes, target resumed Activity, PID, crash/renderer logs and original2340×1080 PNGs checked. No wipe/reset or test-server request was made.

| Final scene | PID | Foreground / fatal / render exit | GL0x0506 | Original image observation |
| --- | --- | --- | ---: | --- |
| Normal isolation | 13707 | Yes / 0 / 0 | 1 | Original login, Test server not configured; no fixture injection |
| Night JNI | 13787 | Yes / 0 / 0 | 5 | Bichon map, self/torch monster/FireWall, darkened map and light pools |
| Dawn JNI | 13865 | Yes / 0 / 0 | 2 | Same bounded offline world and visible local light pools |
| Day JNI | 13954 | Yes / 0 / 0 | 0 | Visible world; dark lighting material disabled as expected |
| Daytime forced-dark map JNI | 14032 | Yes / 0 / 0 | 0 | Map dark-light2 overrides daytime, dark map and light pools visible |

Dark-scene actual consumer reports 6 map + 3 entity/effect sources, 9 retained layers and 10 loaded original textures; day has 0 retained layers and material disabled. Producer markers occur before enqueue acceptance; consumer diagnostics are not GPU fences. The native receipt's individual fields were not separately parsed from these logs. Original pixels and foreground state prove only this bounded visible sample, not exact Windows pixel/interpolation/action parity.

All five current original PNGs and seven first-stage originals were inspected at original resolution. The first-stage night/dawn/map images showed an underlying login after rendering exit; the isolated night repeat showed the launcher and no target foreground. Those are explicit **FAIL**, despite alive PID and fatal0. First-stage day was only bounded offline visual success. Do not rebind c57 APKs/images to 1f.

Five new distinct PIDs total **8 GL0x0506** entries: the zero-error GPU gate remains **FAIL**. No PathNotFound/cursor-warp/fatal/render-validation exit was seen in these five samples; this does not close global resource or long-duration stability gates.

Raw ignored evidence roots relative to repository:

- `mir2-web3/apps/game-client/platform-android/target/lighting-sources-20261006-vL6Lfp` — c57 source binding, red/green, first dual APKs, first five scenes plus isolated repeat, original validation errors.
- `mir2-web3/apps/game-client/platform-android/target/lighting-target-20261006-LSNH5q` — 1f source binding, descriptor red, fourteen final gates, final APKs/resources/install, device, five actual original images/logs, visual-review and SHA evidence archive.

Compact auditable metadata: [source-evidence.json](source-evidence.json). Raw APKs/logs/caches/keys are not committed. Local evidence directories must be retained separately for replay; this document is not a portable copy of the images.

## Windows baseline and remaining gates

Read-only remote snapshot `2026-10-06T00:57:53.488Z`: selected `codex/playtest-registration` is `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`; frozen functional source `3d735745f1117d42a7859e87604a106351dca935`. Difference: +1 commit, 83 documentation paths, 0 functional paths. Old `codex/windows-player-journey` is not silently substituted as the target. Independent [PR #253](https://github.com/Zombieliu/mir2/pull/253) remains Open/Draft, base `codex/playtest-registration`; final Android-only normal push/remote receipt is recorded separately after this documentation commit.

Remaining: zero-error GPU; complete map/object/spell/action and lighting interpolation coverage; whole phone UI and mobile login, multi-touch/IME; authenticated HTTPS/WSS/real receipts/online player loop; shared Zone authoritative movement and saved re-entry; nativeResumeV1/background/network/audio; approved full resources/audio/cache/update delivery; physical Android and human frontend acceptance. Offline IN_GAME fixtures do not authenticate, grant inventory/rewards, settle trades or save a real character.

Next: investigate the remaining framebuffer GL0x0506 with raw foreground/render evidence, then continue native lifecycle/network and full phone/resource gates against the unchanged complete Windows goal. No production deployment, real save modification, Windows push or PR merge.
