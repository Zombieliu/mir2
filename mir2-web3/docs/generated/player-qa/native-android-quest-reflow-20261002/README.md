# Native Android v24 — shared phone quest reflow, whole goal still Active

2026-10-02. Exact source `9a6bff0db9a7ff3eed003c72c90310ec632572d0`.
Frozen Windows gameplay baseline `3d735745f1117d42a7859e87604a106351dca935`.
Live Windows source branch was rechecked at `56ee063fb4f9d6b581ec548659066ef9c5e05e9b`:
one additional commit, 83 docs-only files, zero new game-source files.
[Original comparison](raw/v24/windows-live-compare.json). No whole-goal percentage
or new full Windows desktop acceptance is inferred from tests or Git distance.

## Actual phone results

The primary inspected all thirteen original, uncropped 2340×1080 emulator
frames. This is the real shared diary/detail renderer with optional phone
presentation, not a mock replacement or another scale-only adjustment.
The captured wide phone leaves HP/MP, six belt targets, chat and both thumb
areas clear in diary, pair and independent-detail states.

- Row tap (1300,530) opens the two independent shared windows:
  [diary](raw/v24/wide-touch-retry/diary.png),
  [pair](raw/v24/wide-touch-retry/pair.png).
- Actual vertical swipe (1660,730) → (1660,350) scrolls the full detail without
  selecting another quest: [after swipe](raw/v24/wide-touch-retry/detail-swipe.png).
- Actual Down (1800,947) reaches progress, rewards and reward-choice labels;
  Up (1430,947) returns to earlier content:
  [Down](raw/v24/wide-touch-retry/detail-scroll-next.png),
  [Up](raw/v24/wide-touch-retry/detail-scroll-back.png).
- Diary Close (1250,132) leaves the detail visible and wider, preserving its
  scroll and the HUD lane: [detail only](raw/v24/wide-touch-retry/detail-only.png).
  Its actual Up (1000,947) reaches the title again:
  [top](raw/v24/wide-touch-retry/detail-scroll-top.png).
- Detail Close (1800,132) removes the last quest window and restores the
  default HUD/belt/chat/thumb layout:
  [closed](raw/v24/wide-touch-retry/all-closed.png).

The first diary's canonical native diagnostics record body/control text 14dp,
headings 16dp, six actual computed controls each at least 48×48dp, and all
observed glyphs alpha-mask shaped (not intrinsic black color glyphs).
Floating-point 13.999999 is the 14dp authored size, not a smaller target.
These are first-diary computed metrics; pair metrics were not freshly logged.
Pair/detail readability and transitions above are primary visual observations
plus production-renderer dp tests, not a fresh pair telemetry claim.

**Bounded captured reflow PASS; full phone UI still OPEN/FAIL.** NPC quest list,
confirmation/alert surfaces, all nine locale reflow cases, compact/IME overlap,
physical touch and full multitouch remain. One mixed Chinese/English model
specimen is not nine-language acceptance. Desktop skin decorations still appear
behind the phone header/footer. No complete visual polish or accessibility pass.

**Zero-error renderer FAIL:** latest cumulative logs for five distinct PIDs have
26/8/0/28/20 GL 0x506 entries, total **82**. Only one latest log per PID counts;
diary/pair/scroll snapshots and dual diagnostic sinks are not counted twice.
Capture durations and scenes differ from v23, so 82 versus 38 is not a quality
trend or proof that this change caused/fixed the GPU issue. No selected missing
path/fatal/panic entry is not a global stability pass. All earlier 38/67/33/73/135
results and original failures stay at their own sources.

Everything marked OFFLINE is a source-shaped received-model specimen, directly
injected by the native preview producer, not authenticated Java/JNI ingress.
Its black field deliberately requests no map pack; it is not a live map or
StartGame/render-ready proof. Progress 2/2, Gold 50 and reward choices are not
earned rewards. No accept/deliver/abandon/reward-choice or transaction button was
tapped. The normal package shows empty credentials and “Test server not
configured.” Initial NPC dialogue was captured, but no fresh Exit/Menu/combat
acceptance is claimed this leaf.

## Source scope and failure-first verification

Four source files only: shared `quest_ui.rs`, new `quest_phone.rs`, Android
`phone_quests.rs`, and the package version. The optional resource is supplied
only by the Android host; its absence retains the Windows geometry/controller
path. Phone presentation retains grouped/source order and newcomer tabs,
chapters/paging/graduation, full source detail lines, rewards and shared actions.
Existing detail primary-action eligibility is extracted once and called by
both layouts; no phone accept/finish/authentication/reward rule is invented.

Phone bodies use Bevy `Overflow::scroll_y` / `ScrollPosition`, dp fonts and
48dp controls. Release, cancellation, reset, focus and owning touch id determine
a click; a swipe never dispatches a quest operation. The common fit uses the
same protected workspace as the HUD, without shrinking the reflow's dp sizes.

Failure-first investigation reproduced the actual source 15px row (compiled
0/1 red). The first touch suite then exposed shared no-op resource changes
rebuilding child entities every frame (5/2 failing). A phone-only view-input
snapshot retains unchanged controls until actual presentation/model changes;
all eight focused cases now pass. Default Windows churn is not rewritten.
Compiler/setup errors remain in their original logs and are not product reds.
The first preview gate failed on a missing diagnostic type import; only the
fresh final-source-2 six-gate set counts below.

| Fresh final source gate | Actual result |
| --- | --- |
| Android normal / preview | 327 / 350 passed, zero failure/ignored |
| Shared native-player-ui | 1238 passed, zero failed, 10 existing ignored |
| Java Debug / UiPreview | 41 each, zero failure/error/skip, all 42 tasks rerun |
| Actual API31 arm64 checks | Debug and UiPreview pass |
| Immutable source binding | 39 committed inputs equal before/after all six gates |

[Source audit](raw/v24/source-v24.json) binds the final inputs and exact four-file
diff. The eight new shared cases cover actual generated dp nodes/full late
paragraphs, native touch release through the shared controller, scrolling,
cancellation/second-finger ownership, reset/lost focus and retained entities.
No server, authentication, Zone, combat/trade/save rules or Windows host source
changed. Older broader Windows/runtime failed gates retain their original scope.

## APKs, device and remaining denominator

Both packages start/end clean at the source above. Version24 /
`0.1.21-quest-reflow`, min API31 / target35 / arm64-v8a. Native Bevy/GameActivity,
not WebView; Rust release cdylib inside Gradle Debug diagnostic variants,
not store Release. Both Gateway URLs are empty; no remote web page was used.

| Local-only APK | Bytes | SHA-256 |
| --- | ---: | --- |
| mir2-native-quest-reflow-debug-v24.apk | 387145917 | `23796b1da9d191953bf27b70ba021d57a41bddb0c51faff59340ff35de42b842` |
| mir2-native-quest-reflow-preview-v24.apk | 391013025 | `0e12b1d1276f3774ca9b609c52815690595d413d7c820220279ab2ff55bc95d3` |

Local directory: `apps/game-client/platform-android/target/quest-reflow-v24-20261002/final-apks/`.
[Package audit](raw/v24/package-v24.json) retains absolute paths and byte checks.
Each package's selected 6647 PNG and three metadata files equal approved inputs;
UI_32bit equals the frozen Windows Git bytes. This is not full world/entity,
audio, update, signing or release acceptance. APKs/cache/packs are not committed.

Existing emulator-5554 / Mir2_API_31_ARM64 / Android12 / API31, native panel
1080×2340 / density440; app frames are landscape2340×1080. Data-preserving
update installs only. Installed base.apk SHA is checked before evidence runs
and again after the actual touch sequence. Task apps are stopped afterwards.
No wipe/uninstall, real credentials, human save, production deployment or device.
[Original checkout audit](raw/v24/original-checkouts.json) retains original heads,
the main dirty status and original Android clean status.

[Manifest](manifest.json) binds **162** unmodified raw artifacts including
thirteen PNGs. `raw/** -text` precedes staging; `git-integrity.mjs --cached`/HEAD
checks working and actual Git bytes plus all39 source inputs. Raw system output
retains CRLF/trailing spaces, excluded only from whitespace lint—not integrity.
[Runtime audit](runtime-audit.json) records the bounded passes and retained fails.

Next: remaining shared quest/NPC surfaces and locales, bounded framebuffer
diagnosis and the whole NI-11–20/Windows denominator. Real authenticated JNI,
HTTPS/WSS, full player loop/authoritative Zone and save, complete resources,
physical/human gates remain OPEN. Missing environment/device is not completion.
The formal Windows-alignment goal remains **Active**, not complete.
