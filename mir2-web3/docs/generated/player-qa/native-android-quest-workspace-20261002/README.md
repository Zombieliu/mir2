# Native Android v23 — protected quest workspace, full phone UI still FAIL

2026-10-02. Exact diagnostic source `ef5e56d1c8e51abd1fad3ee79be9a4ce1aa604e4`.
Frozen Windows gameplay baseline `3d735745f1117d42a7859e87604a106351dca935`.
The full Windows-completeness goal remains **Active**; no percentage is inferred.

## What actually passed, and what did not

The primary inspected all ten original uncropped 2340×1080 emulator frames.
Actual row tap at (1350,170) opens the real shared detail beside the diary:
[before](raw/v23/touch-device/quests-ingress-before.png),
[after](raw/v23/touch-device/quests-ingress-after.png). Both windows now leave
the left HP/MP status, six shared belt targets, chat and both thumb areas clear.
Actual diary Close at (1197,909) leaves the independent detail visible and
retains the protected HUD lane:
[pair](raw/v23/detail-device/pair.png),
[detail only](raw/v23/detail-device/detail-only.png).
These are bounded layout/local-control checks, not gameplay acceptance.

NPC Exit at (263,319) still removes the received-model dialogue and restores
the default HUD/control layout:
[before](raw/v23/touch-device/npc-ingress-before.png),
[after](raw/v23/touch-device/npc-ingress-after.png).
No fresh Menu, inventory-use, multiplayer, combat or multitouch acceptance
is claimed. No accept/deliver/reward or transaction button was tapped.

**Full phone UI FAIL:** fitting windows into the protected region does not
make diary/body/footer/reward text readable or all targets 48dp. Some glyphs
remain small/dark; the pair becomes smaller when both windows are visible.
Compact landscape/IME still uses the existing safe fallback, not an accepted
protected/48dp layout. Full nine-language, scrolling and phone reflow remain.

**Zero-error renderer FAIL:** six distinct PID logs contain 38 GL 0x506 entries
(normal login 0, initial diary 7, initial NPC 20, row tap 8, NPC Exit 0,
independent detail 3). Only each PID's latest cumulative log is counted;
before/after and diary/pair/detail are not counted twice. This leaf does not
change the GPU renderer or prove improved stability. Previous v22/67, v21/33,
v20/73 and v19/135 and their original failures remain at their own sources.
No captured missing-path/fatal/panic entry is not a global stability pass.

All model specimens are explicitly OFFLINE. Their black playfield requests
no map pack; it is not failed map loading or real render-ready/StartGame proof.
The normal login shows empty credentials and “Test server not configured.”
Source-shaped progress 2/2, gold and rewards are not real earned outcomes.

## Scope, regression and source integrity

Only four Android source files change: `phone_quests.rs`, `phone_hud.rs`,
`world_input.rs` and the package version. Quest fitting and HUD placement
reuse the existing Android `PanelSidebar` workspace. The independent-detail
presence check reuses shared `QuestUiState::detail_quest` with the authoritative
tracker; stale/missing entries do not falsely keep a sidebar open. The lookup
is optional and presentation-only. Existing modal/action guards are unchanged.

No shared quest renderer/controller/model, Windows source, server, reward,
movement, trade, save or authentication rules change. Authored panel geometry,
shared children/handlers and pair gap remain. Physical insets are converted to
logical coordinates once; hidden/out-of-game panels still reset transforms.

The compiled original layout regression fails 0/1 before the fix and passes
1/1 afterwards. Retained setup failures are not labeled product regression:
the first green run used the old `SystemState::get` API (E0599); the first
normal suite had 324 passes/1 bad test fixture status spelling. Its 37-input
binding also omitted the new `world_input.rs` input. Only the corrected
`final-source-2` records below, covering 38 committed inputs, count as final.

| Fresh serial/offline source gate | Actual result |
| --- | --- |
| Android normal / preview | 325 / 348 passed, 0 failed/ignored |
| Shared native-player-ui | 1230 passed, 0 failed, 10 existing ignored |
| Java Debug / UiPreview | 41 each, 0 failure/error/skip, all 42 tasks rerun |
| Actual API 31 arm64 target checks | Debug and UiPreview pass |
| Input binding | 38 committed inputs equal before/after all six gates |

[Source audit](raw/v23/source-v23.json) checks exact committed input bytes and
the four-file Android scope. Older runtime and broader Windows failed gates
keep their own scope; this is not a fresh full Windows desktop/GUI pass.

## Packages, device and remaining goal

Both builds start/end clean at the exact source. Version 23 / name
`0.1.20-quest-workspace`, min API 31 / target 35, arm64-v8a, native Bevy/
GameActivity, not WebView. Rust release cdylib is inside Gradle Debug diagnostic
variants, not a store Release. Both Gateway URLs are empty; uiPreview is offline.

| Local-only APK | Bytes | SHA-256 |
| --- | ---: | --- |
| mir2-native-quest-workspace-debug-v23.apk | 386848869 | `69b526f48cce6c6a0d385d2586e85e6445d927f7827b203feda7bca445eaaacf` |
| mir2-native-quest-workspace-preview-v23.apk | 390827589 | `caedd343de692b6cdce291acc01d4a7b318021fdef187c662ddd8a8971611fa6` |

Local directory: `apps/game-client/platform-android/target/quest-workspace-v23-20261002/final-apks/`.
[Package audit](raw/v23/package-v23.json) retains the absolute paths; each
package's selected 6647 PNG and three metadata files match the approved inputs,
and UI_32bit equals the frozen Windows Git bytes. This is not full resource,
audio, update, signing or release acceptance. APKs are not committed.

Only existing emulator-5554 / Mir2_API_31_ARM64 / Android 12 / API 31 was used,
native portrait 1080×2340 / density 440, landscape captures 2340×1080. Both
apps update with data-preserving installs. Installed base.apk SHA is checked
before captures/taps. Task apps are stopped afterwards. No wipe/uninstall,
real credentials, production, human saves, physical device or public deployment.
Original main and Android checkouts remain at their original heads/status.

[Manifest](manifest.json) binds 147 original raw artifacts including ten PNGs.
No APK, cache, asset pack, password or key is curated. `raw/** -text` applies
before staging; `git-integrity.mjs --cached`/`HEAD` verifies working and actual
Git bytes plus all 38 committed inputs. [Runtime audit](runtime-audit.json)
records protected layout's bounded pass and readability/renderer failures.
Original system output contains CRLF/trailing spaces: it is preserved, not
reformatted to satisfy whitespace lint. That lint applies to authored files;
the exact `raw/` subtree is excluded only from whitespace lint, not SHA/Git
integrity checks. The first unrestricted lint stopped before commit; no
partial evidence commit or byte modification followed.

Next: readable phone reflow with 48dp controls and explicit scroll/paging,
bounded framebuffer diagnosis, then the complete NI-11–20 / Windows denominator.
Actual authenticated JNI/HTTPS/WSS, quest rewards, Zone/save, complete resources
and physical/human gates remain open; missing environment/device is not completion.
