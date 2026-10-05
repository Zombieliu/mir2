# Native Android — shared lighting environment metadata

2026-10-06 bounded NI-17 delivery. Full Windows-alignment goal is **Active**; NI-17 is **PARTIAL**.
This is native Bevy, not a WebView. It does not complete lighting, real login, a player loop or device acceptance.

## Exact source and scope

- Product source: `8d4fd2ad9920981947256e291b34f0816f62f805`; parent: `6413212c060744cc7c94fbd5d609c057df2bf491`.
- Android branch: `codex/android-shared-sync`; independent [Draft PR #253](https://github.com/Zombieliu/mir2/pull/253).
- Frozen full Windows functional baseline: `3d735745f1117d42a7859e87604a106351dca935`.
- Read-only current Windows source at `2026-10-05T23:08:51.618Z`: `56ee063fb4f9d6b581ec548659066ef9c5e05e9b` on `codex/playtest-registration`; +1 commit/83 documentation-evidence paths/0 functional paths. Old handoff branch is not the selected baseline.
- Eleven source/test paths,434 exact inputs,423 protected whole files. Input-list SHA-256: `54874104037a1c304df7f44e57a7bb130b1c677ae2337dcdcc8249551e8d37ca`.
- Original dirty checkout and prior clean Android worktree HEAD/branch/status checkpoints unchanged; no recursive untracked-byte claim.

Original four Windows environment methods and five helpers were extracted unchanged into the shared client. Windows uses thin delegation; original renderer/source-selection/force-daylight rules and old14 test bodies remain unchanged. Old Windows test-footer SHA-256: `9d78d9173454c837c4b44390cc04605b9fb8f94f27a33be3e420e7593281006a`.

Java forwards exactly `TimeOfDay`, `MapInformation`, `MapChanged`, `NewMapInfo` through a separate opt-in observer. MainActivity routes it into the existing nativeEvent envelope. Old callback traffic,62 network-test bodies and auth/session methods are preserved. Existing MapInformation/MapChanged lifecycle handling is unchanged; NewMapInfo metadata is not destination bootstrap.

Android uses only the existing validated player owner/name/map snapshot. Pre-owner metadata is one bounded coalesced state, not login or authority. A newer snapshot is not followed by replay of older packets. Scene reset drops map/binding but retains connection time; disconnect/session reset drops all. Different-map metadata cannot publish against the old scene. Rejected enqueue retains the exact latest absolute model.

**Rendering is explicitly disabled.** Output stage is1024×768 with empty mapLights/entityLights. The shared runtime still owns darkness/range/palette/material rules. Map/object/effect source production, actual lighting JNI and visual/GPU acceptance remain OPEN.

## Fresh gates bound to this clean source

| Gate | Result |
| --- | --- |
| Android normal / uiPreview Rust |502/502 and549/549|
| Shared native-player-ui |1314 passed;10 original ignored|
| Shared runtime |296 passed;1 original ignored|
| Java Debug / UiPreview |99/99 each,7 classes,0 failures/errors/skips|
| Windows original lighting / native protocol |14/14 and21/21, on Mac|
| Windows Hero FIFO / skill FIFO |1/1 and1/1, on Mac|
| API31 arm64 normal / uiPreview |both offline checks exit0|

Eleven gates, not eleven Android gameplay flows. Focused18/10/14 counts overlap and are not summed. Existing ignored tests are not passes. Windows subsets on Mac are not a full Windows OS gate. Local TLS fixtures are not real account/server acceptance.

## Failures retained

- Initial Java test helper failed compilation with checked JSONException; fixed without claiming a semantic red.
- Compiled original ingress red:5 tests/3 failures, proving missing metadata forwarding.
- Shared test8 pass/1 fail came from an incorrect oracle: mapDarkLight=0 is valid in the original Windows rules. The new assertion was corrected; source rules were not changed.
- First combined Java request unexpectedly ran full Debug:98 tests/13 old callback failures. All seven raw Debug XMLs retained; UiPreview did not run. The generic observer route was corrected with a separate opt-in callback, without editing old assertions.
- Dedicated callback red:5 tests/2 failures before routing; final fresh full variants99/99.
- Source-audit/device-list harness formatting errors and extraction patch verification failure are described separately; their raw error capture is only the tool transcript, not falsely claimed as a saved file.
- One read-only GraphQL EOF preserved; REST fallback verified current Windows/Draft state.

## Native diagnostic APKs

APK/cache/password/signing-key files are not committed. These use native Rust release in diagnostic Debug/uiPreview packages, not a store release. VersionCode35/name0.1.32-gameshop-phone remain unchanged; minimumAPI31,arm64-v8a. The native strip warning is retained: matching the current variant output does not prove symbols were stripped or release size optimized.

Local artifact root, resolved under this worktree: `apps/game-client/platform-android/target/lighting-environment-20261006-nOqNqD`.

- Normal: `final-apks/mir2-native-lighting-environment-debug-8d4fd2ad9.apk`,533850007 bytes; SHA-256 `873c6fe040fe19fe2ae386e69a50f5ffd7af0a201bb0c28b82f2623869e100cf`.
- Preview: `final-apks/mir2-native-lighting-environment-preview-8d4fd2ad9.apk`,542008527 bytes; SHA-256 `a005810a71d9f30910144779bd89bf655302192c8c4a90cabf2e2e4e128dbf5b`.

Each APK's ELF matches that variant's build output; selected6647 UI PNGs/3 metadata files and all ten original Lighting textures match source bytes. Lighting textures and UI_32bit470–473 match the frozen Windows Git blobs. This is not full-resource-release acceptance. Existing Archer/mount proof pack manifest remains `929d146535bde9b61c75c33af0e98852dcc3aef83320869aac9f06f4bdd530d9`; full entity/resource coverage remains OPEN.

Gateway URL is empty; preview networking forbidden. No remote Web page version applies. Real HTTPS/WSS/Gateway/ordinary-test-account verification is still missing; credentials must be entered locally, not supplied in chat/logs.

## Installation and reviewed originals

Data-preserving `-r` installs on the existing dedicated Mir2_API_31_ARM64/emulator-5554 succeeded. Pulled installed APK SHA-256 equals each artifact. No wipe/uninstall/global log clearing. Physical devices:0 in this snapshot. API31/Android12/arm64 emulator,density440, landscape captures2340×1080.

Both original PNGs were viewed:

- `normal-hud-isolation-pid12571.png`: Crystal login page, empty credentials and visible “Test server not configured”; bounded normal-build preview isolation only. Login sizing/keyboard/touch not accepted.
- `offline-hud-baseline-pid12638.png`: explicitly labelled offline black-background HUD fixture (Lv22,HP160/200,MP60/100,Gold12345), joystick/belt/action controls. No map/player visible; not map entry, lighting or gameplay proof.

At the8-second cold-start samples both PIDs alive,fatal0,PathNotFound0,cursor-warp-errors0. Independent GL0x0506 counts1/0: **GPU zero-error gate remains FAIL**. Earlier failures are retained, not relabelled. Source-bound screenshot hashes/raw paths are in [source-evidence.json](source-evidence.json); only compact evidence is committed, not the full local raw artifacts.

Raw append index:111 files/8823080 raw bytes; index22218 bytes, SHA-256 `28c547557e41a2cf147873c4d9740f2b9138dafb78833566e2e8d996035a80aa`. It excludes APKs,self,later delivery/documentation/push receipts; their hashes/status are separate. Final visual-record time is in baseline-visual-verdict-final.json; the estimated timestamp in the first preserved verdict is not an exact view-time claim.

## Still required

Complete original map/object/effect light-source production and actual Android JNI/GPU/light-view cases; full animation/spell/action variants and Android audio/focus. Full phone UI/keyboard/multi-touch,resources/cache/update,real login/character/StartGame/Zone movement/save,actual resume/online operations,cross-platform interaction,physical device and human acceptance remain OPEN. AP-01–21 denominator is unchanged. Do not deploy production,change real saves,push Windows or merge the Draft PR.
