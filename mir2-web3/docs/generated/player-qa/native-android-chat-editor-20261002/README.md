# Native Android shared chat editor — v16, 2026-10-02

Bounded G4/NI-09 repair: **DONE_WITH_CONCERNS**. The full Windows-alignment
goal remains **Active**; no percentage or full UI/online/device acceptance.
Exact clean APK source: `8cd2e7eaeead8c71f74fbbaa9250bb2d875fed04`.
Parent: `30b65841f2e2e7f1a3983d13386613a1c48519f7`.
Original frozen Windows: `3f5e61533235921369bc13a7760b4a56b0e467e5`.

## Debug report and source gate

Symptom: Android Back hid the keyboard, but tapping the unchanged shared chat
draft could not reopen it. The original [v15 failure](../native-android-phone-chat-layout-20261002/preview-chat-reopen-attempt.png)
is untouched. This round [freshly reproduces it on still-installed v15](before-draft-retap.png);
both the actual1000,978 tap and `mInputShown=false` are retained.

Root cause: the original shared `CrystalChatInput` child lacked `Button` and
`NativeTextInputTarget`. Android's existing PreUpdate editor-touch collector never
saw its press. Back leaves the same shared field focused, so PostUpdate keyboard
processing correctly did not treat it as a newly selected field.

Fix: only Android's `phone_hud` decoration adds those two components to that same
child. Existing held-press rising edge, epoch validation and before-Update touch
capture are reused. Received rows are not editable targets. Shared chat source,
desktop UI, Java editor, sending/echo/filter rules, authentication and server
authority are unchanged. Version16 distinguishes the new APK.

Three compiled regressions first failed **0/3**, then passed **3/3**. A fixture
import compilation error and a Java log-redirect failure before Gradle executed
are retained separately, not counted as product failures or passes.

| Source gate | Actual result |
| --- | --- |
| Android normal /actual ui-preview feature |276 /290 passed,0 failures/ignores |
| Java Debug /uiPreview |37 each,5 suites each,0 failures/errors/skips |
| Java task execution |42/42 actually executed with `--rerun-tasks` |
| API31 arm64 checks |Both real feature variants pass |
| Format /scoped diff |Pass |
| Four-file read-only review |No new P0/P1/P2; reviewer ran no tests/build/device |

These tests inject `Interaction`, verify markers and before-rebuild capture;
they do not replace actual picking/Java/IME evidence below. Shared/Windows full
suites were not rerun for this Android-only leaf. `source-results-v16.json` binds
14 actual source inputs. Audit scripts run **from repository root**, against the
exact ignored QA target; source/package modes take the exact source SHA.

## Exact packages and device

Version16 /`0.1.13-chat-editor`, API31 minimum, target35, arm64-v8a. Both builds
were clean before/after at8cd2e7eae. Each actual APK has6647 selected PNGs and three
metadata files byte-equal to approved inputs; four original weight images and
metadata match frozen Git. This is not complete resource-release acceptance.

Ignored directory, relative to repository root:
`mir2-web3/apps/game-client/platform-android/target/chat-editor-v16-20261002/final-apks/`.

| APK | Bytes | SHA-256 |
| --- | --- | --- |
| `mir2-native-chat-editor-debug-v16.apk` |386309777 |`30951cb50420d73125aa863b6bf8e853e6f308b723dd0f2e97aee18acd428a39` |
| `mir2-native-chat-editor-preview-v16.apk` |390063805 |`7aeebe96c08a3c03cae1ffda020f2fd9ac73d7781d95beed2b7d0b4f93850772` |

Both install-r operations return Success and installed versions are16. These
contain release Rust cdylibs in Gradle Debug diagnostics, not store Release or
production signing. Gateway stays empty; the [normal login](debug-login.png)
shows `Test server not configured.` No remote Web page, credentials or real save.

Same dedicated Mir2_API_31_ARM64 /emulator5554, Android12 sdk_gphone64_arm64,
density440 (2.75), actual uncropped landscape frames2340x1080. Emulator PID22274
remains the same before/after. No restart, GPU change, wipe or Pixel5 action.
The two owned apps are force-stopped after capture; data and AVD are retained.

## Fresh actual input verification

All14 raw frames were manually inspected. The first `before-keyboard-hidden.png`
is a retained hide-animation frame, not stable evidence; the separately captured
`before-keyboard-hidden-settled.png` is stable. Scripts verify integrity, IME and
logs, not OCR. Physical pixel actions were chosen from original SDK frames.

| Actual action | Observed result |
| --- | --- |
| [Initial v16 chat](preview-chat.png) |System/peer rows and actual soft keyboard visible |
| Actual keyboard q@240,694 and w@445,694 |[Shared unsent draft displays qw](preview-draft-typed.png) |
| Android Back |[Keyboard hidden, qw retained](preview-keyboard-hidden.png); IME false |
| Actual draft tap@1000,978 |[Keyboard reopens, qw retained](preview-keyboard-reopened.png); IME true |
| Second Back, same-point1500ms hold |[Keyboard reopens, qw retained](preview-keyboard-held-reopened.png); one additional showSoftInput call |
| Actual keyboard e@655,694 after reopening |[Draft becomes qwe](preview-draft-edited-after-reopen.png); reopened editor is still usable |

Three cumulative showSoftInput calls total: initial opening, retap, held retap.
No per-frame flood observed in this one held gesture. No send/echo/scroll/Apply/
filter/online/persistence/physical-keyboard or general long-press acceptance claim.

## Real render failure remains

Fresh v15 first-chat and no-keyboard HUD PID logs have0 error entries, while its
next chat startup has6 GL0x506 entries (2 unique full timestamp/message strings).
V16 preview PID8321 has**10 startup GL0x506 entries**, also2 unique full strings;
six cumulative captures repeat those same entries, not60 new reports. Debug
PID capture has0 errors. No fatal/panic/signal matches in the bounded logs.

The **zero-error gate FAILS**. `runtime-audit-v16.mjs` emits the measured report
and intentionally exits1 for this real failure. The entries coincide with first
IME/surface changes, but that timing is only a diagnostic lead, not a confirmed
root cause. Runtime/GPU/backend were not changed; renderer/stability is not
accepted. Old v15/v14/v13 failures remain intact.

## Windows upstream drift and next work

Fresh GitHub API verification observes Windows source branch at
`3d735745f1117d42a7859e87604a106351dca935`,14 commits ahead/0 behind the original
frozen3f5e61533, touching238 filenames (mostly QA evidence). The exact commit/file
inventory is [windows-upstream-delta.json](windows-upstream-delta.json). It includes
shared NPC response/service lifetime and caster/map-state changes, not only docs.
This is not a source-body or acceptance audit. V16 does **not** contain that delta;
no older evidence is rebound or denominator silently advanced.

Next: review/import that exact delta by a normal merge preserving Android and
run affected shared/Android/Zone/gateway gates; track NPC/services/skills changes
in the leaf inventory. Then continue GL startup diagnosis, extreme IME controls,
HUD weight, full NPC/services and remaining Windows abilities. Approved real
HTTPS/WSS, Shared Zone online loop/save, full resources, true device and human
acceptance remain OPEN. No production deployment or Windows write is authorized
by this diagnostic result.

Source/evidence are normally published only to `codex/android-shared-sync`, PR253
kept Open/Draft/base unchanged. APK/cache/unpacked originals/keys are not committed.
Later evidence commits do not change either APK's exact8cd2e7eae source.
