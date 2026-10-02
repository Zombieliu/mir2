# Native Android v22 — bounded shared quest focus, phone UI still FAIL

2026-10-02. Frozen Windows `3d735745f1117d42a7859e87604a106351dca935`.
Exact diagnostic source `afea4b358680bb1d1a2ec8362ca6ffb538c6d648`.
Whole Windows-completeness goal remains **Active**. This is not complete UI.

## Actual images and failures

The primary inspected all seven original uncropped2340×1080 frames. Actual
offline row tap at1120,170 opens the shared detail beside the enlarged diary:
[before](raw/v22/touch-device/quests-ingress-before.png),
[after](raw/v22/touch-device/quests-ingress-after.png). Both windows retain a
gap and visible received description/turn-in/progress. No accept/deliver/reward
action was tapped, and source-shaped2/2 is not a real quest success.

**Phone UI FAIL:** the full-height pair covers part of the status card and
belt/chat controls; small/dark glyphs and below-phone-size targets remain.
Scaling a desktop window is not font/48dp/scroll reflow. The viewport geometry
and one row-to-detail action pass only their bounded checks, not G4. Do not
present this diagnostic as a finished mobile layout or hide this new occlusion.

Received NPC title/body/Exit remain visible, and actual Exit at263,319 restores
HP/MP/belt/joystick/action chrome in [the after frame](raw/v22/touch-device/npc-ingress-after.png).
No fresh Menu tap this leaf; v21's Menu result retains its own exact source.
Model-only scenes deliberately request no map pack; their black playfield is
not real login, world rendering, Zone or render-ready acceptance.

**Renderer FAIL:** five distinct PID captures have67 GL0x506 entries:
login0/list43/NPC0/row-tap0/NPC-exit24. Count only the latest cumulative log per
PID, not before/after twice. Zero in some processes does not erase others.
Prior v21/33, v20/73 and v19/135 remain unchanged at their original sources.
No selected captured missing-path/fatal/panic entry is not global stability.

## Source scope and exact gates

Android `phone_quests` reads the real shared panel visibility and fits the
diary/detail union as one group. Separate-center scaling would overlap them;
each panel center is shifted around the common group center. Node authored
rectangles, all shared children/controllers/models and outbound rules remain
unchanged. Host insets are converted from physical to logical pixels once;
hidden/out-of-game or invalid geometry resets the presentation transform.

Shared `quest_ui.rs` changes exactly two marker declarations to public; every
other byte equals the published4d2e parent. [Strict byte audit](raw/v22/shared-ui-export-parity.json)
records this; no shared rule/default Windows geometry changed. This is a
production Android presentation adapter, not merely a feature fixture, but its
captured gameplay/model input is explicitly offline.

Compiled current-stage regression fails0/1 before the adapter, then passes1/1.
Private inset field E0616 compiler/setup failure is retained separately in
`quest-focus-green.log`, followed by successful `quest-focus-green-2.log`.
Initial audit inherited an incorrect fixture-only label; both initial metadata
and the explicit correction record remain, with no result/hash changes.

| Fresh serial/offline gate | Actual result |
| --- | --- |
| Android normal / preview | 320 /343 passed;0 failed/ignored |
| Shared native-player-ui | 1230 passed;0 failed;10 existing ignored |
| Java Debug / UiPreview | 41 each;0 failure/error/skip;42 tasks rerun |
| API31 arm64 Debug / UiPreview | Both target checks pass |
| Input binding | 37 committed inputs equal before/after all6 gates |

[Source audit](raw/v22/source-v22.json) binds those exact gates. Older runtime
and broader Windows failed gates retain their original scopes; no fresh full
Windows desktop/GUI acceptance is inferred from two public marker declarations.

## Packages, install and integrity

Both builds start/end clean at this exact source. Version22/name
`0.1.19-quest-focus`, minAPI31/target35, arm64-v8a, native Bevy/GameActivity,
not WebView. Rust release cdylib inside Gradle Debug diagnostic variants,
not store Release. Both Gateway URLs empty; preview is offline-only.

| APK, local only | Bytes | SHA-256 |
| --- | ---: | --- |
| mir2-native-quest-focus-debug-v22.apk | 386833517 | `c06b8b3ee028836974bfaa5629fbb56606de492924eaae3685e0816b1cb768b8` |
| mir2-native-quest-focus-preview-v22.apk | 390828257 | `ad6b01d67fd6711a8f2b18fb568f5ee17e0f67f7b2a33b8a73c6301bf54597c3` |

Local directory: `apps/game-client/platform-android/target/quest-focus-v22-20261002/final-apks/`.
[Package audit](raw/v22/package-v22.json) records absolute paths and selected
6647 PNG/three metadata input byte checks each; UI_32bit also equals frozen
Windows Git. This is not complete assets/audio/update/release acceptance.

Only existing emulator5554/Mir2_API_31_ARM64/Android12/API31, native1080×2340,
density440/landscape2340×1080. Both apps update with data-preserving installs;
installed base.apk SHA-256 is checked before captures/taps. Task apps stopped
afterwards. No wipe, uninstall, real credentials, production or physical device.

[Manifest](manifest.json) binds135 raw artifacts including seven PNGs; all
original bytes, failed compile, original metadata and correction preserved.
`raw/** -text` applies before staging. No APK/cache/extracted asset/password/key
is curated. `git-integrity.mjs --cached` before commit or `HEAD` after verifies
actual Git blobs and37 committed source inputs. [Runtime audit](runtime-audit.json)
retains occlusion/readability/renderer FAIL rather than changing their labels.

Next: protect shared status/belt/chat and both thumb zones before more scaling;
use a phone reflow with readable fonts,48dp rows and explicit scroll/paging while
keeping shared controls/rules. Then bounded GPU diagnosis, complete NI-11–20,
real HTTPS/WSS/Zone/save and physical/human gates. The full denominator stays.
