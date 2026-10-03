# Native Android v20 received quest/dialogue — exact package and bounded UI

2026-10-02. Frozen Windows: `3d735745f1117d42a7859e87604a106351dca935`.
Exact diagnostic source: `7b62eda33cbf6c10a41c22516f1e6400dfe92202`, including
the [ccdd51a90 authoritative quest ingress](../native-android-quest-ingress-20261002/README.md).
Whole Windows-completeness goal remains **Active**, not complete or a percentage.

## Actual results, including failures

The primary inspected all seven original uncropped2340×1080 frames:

- [Normal login](raw/v20/device/debug-login.png): empty credentials and explicit
  `Test server not configured`. No real login/account/server was used.
- [Received quest list](raw/v20/device/quests-ingress.png): Daily group, received
  title and ready-to-turn-in state visible. A real emulator row tap at943,168
  opens [shared quest details](raw/v20/touch-device/quests-ingress-after.png),
  including received description, turn-in text and **2/2** progress. This is
  source-shaped offline progress, not a kill/quest-completion/reward result.
- [Received NPC dialogue](raw/v20/device/npc-ingress.png): title, source body and
  Exit link visible. Actual Exit tap at263,319 removes the dialogue window in
  [the after frame](raw/v20/touch-device/npc-ingress-after.png). **HUD restoration
  fails**: the original after frame has no restored HUD. Window disappearance
  alone does not pass the full Exit/player-input lifecycle.
- **Phone quest readability fails**: list/detail windows and text are still too
  narrow/small. No full phone/48dp/multitouch or human UI acceptance follows.
- **Zero-error renderer gate fails**: five distinct PID captures contain **73**
  GL0x506 framebuffer-incomplete entries (login/list/dialogue11/10/52; touch0/0).
  Count only the latest cumulative log per PID; zero-error touch runs do not
  erase initial failures. Prior v19's135 entries remain at their old source.

The two model-focused scenes deliberately do not request a map pack. Their black
playfield is not an online map/Zone/world-loop test. The missing HUD after Exit is
recorded separately from that expected absence of a scene map.

## Feature-only implementation and source gates

Android-only four-file delta: preview module wiring, new `quest_preview.rs`, two
scene entries/hooks/tests and Gradle version. Existing39 scene labels remain;
there are41 labels now. The isolated headless fixture app runs the actual
`AndroidQuestIngress` projection and production delivery installation, then copies
only shared tracker/history/dialogue/nearby resources to the offline preview.
It does not install/replace the main runtime inbox, authenticate, start transport,
change main HostState/character or grant items/currency/progress. Main host,
shell and disabled transport preservation is tested. Displayed rewards are not
inventory custody or settlement.

Failure-first compiled missing-feature regression shows the old manual quest1
instead of received2100004. Two compiler/setup errors (private host fields and
objective test-field name), and an intermediate test assuming default Login
instead of Connecting, are preserved separately, not product bug reds. Final
three received-model tests are included in these serial/offline totals:

| Fresh gate | Actual result |
| --- | --- |
| Android normal | 315 passed,0 failed/ignored |
| Android ui-preview | 337 passed,0 failed/ignored |
| Java Debug / UiPreview | 41/41 each;0 failure/error/skip;42 tasks rerun |
| API31 arm64 Debug / UiPreview | Both actual target checks pass |
| Input hashes | 35 committed inputs equal before/after all five gates |

See [source binding](raw/v20/source-v20.json) and original gate logs/XML. No shared,
Windows or server source changed in this four-file leaf; ccdd's shared/runtime
gates and broader Windows FAILED98/1 and39/12 parent comparisons retain their
original source scope, not a fresh full Windows gate.

## Exact native packages and install

Both builds start/end clean at the exact source above. Version20/name
`0.1.17-quest-ingress`, minAPI31/target35, arm64-v8a, native Bevy/GameActivity,
not WebView. Rust release cdylib inside Gradle Debug diagnostic variants, **not
store Release**. Both Gateway URLs empty; preview compile-time flag separate.

| Variant | Bytes | SHA-256 |
| --- | ---: | --- |
| mir2-native-quest-ingress-debug-v20.apk | 386804385 | `f5c28b73fabb7d819f38ee4c08581840bba303efd429779d3bc9cdcdaa0a75bf` |
| mir2-native-quest-ingress-preview-v20.apk | 390798117 | `d946ee116a52890c7137fc20dd7c55a41d2948ad645cb5c83c58e422a3e4bb13` |

Local APKs (not Git): `apps/game-client/platform-android/target/quest-preview-v20-20261002/final-apks/`.
[Package binding](raw/v20/package-v20.json) records absolute locations and hashes.
Each package's selected6647 PNGs/three metadata files equal approved input bytes;
four UI_32bit originals/metadata also equal frozen Windows Git. This remains the
bounded old proof-pack denominator, not all assets/maps/audio or a release feed.

Only emulator5554, Mir2_API_31_ARM64/Android12/API31, native1080×2340/density440,
landscape2340×1080. Both apps updated with `-r`; installed `base.apk` SHA checked
against local artifact before captures/taps. Task apps stopped after testing.
No uninstall/data clear/wipe/AVD change, real credentials or physical acceptance.

## Integrity and next leaf

[Manifest](manifest.json) binds133 raw artifacts, including seven originals and
four intermediate feature logs. `raw/** -text` applies before staging; logs,
XML, CRLF/ANSI and failed frames are not trimmed/repainted. No APK, cache,
extracted resource pack, passwords or signing keys are curated. Run
`git-integrity.mjs --cached` before commit or `git-integrity.mjs HEAD` afterwards
from repository root to verify working and actual Git blobs/source inputs.
[Runtime audit](runtime-audit.json) explicitly retains rendering/HUD/readability
FAIL and limits the two local tap observations.

Next: investigate/fix the Exit-to-HUD display lifetime and phone quest layout with
failure-first source tests and exact new packages, then bounded framebuffer
diagnosis, full NI-11 receipts and remaining Windows model/input/resource leaves.
Do not infer from this feature fixture that live-production Exit has the same
cause; first check the preview panel initialization against the shared lifecycle.

Actual authenticated Java-to-JNI, quest actions/routes/acknowledgements on a
server, real HTTPS/WSS/list/StartGame/Zone/save, all-phone/languages/audio/complete
resources and physical/human acceptance remain OPEN. Approved test environment
is still unanswered; continue safe code leaves without repeatedly asking for
credentials or using production/genuine saves. No Windows-branch push, deployment,
PR merge or whole-goal completion.
