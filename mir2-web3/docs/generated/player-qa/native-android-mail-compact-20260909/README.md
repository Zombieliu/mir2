# Android mail keyboard layout checkpoint

Source: local `bc392f889`, `codex/android-player-journey`.
Status: DONE_WITH_CONCERNS for this bounded offline presentation fix;
whole Android UI, live mail and physical-device acceptance remain open.

## Root cause and implementation

The investigate workflow reproduced Send/Cancel hidden under API31 IME.
The mail adapter explicitly returned editor bottom zero (no panning), while
shared composer actions lived at y408. Panning that entire tall panel enough
to show its footer would push the recipient above the screen.

Shared `overlays.rs` now puts gold/attachment controls and footer actions into
two identity presentation groups. Desktop coordinates, reducers and action
identities are unchanged. Android `shared_shell.rs` hides the details group
only while IME is visible and translates the same footer from y408 to y110.
Enabled and disabled footer nodes both get at least 44 OS-logical-pixel height.
The IME panning bound includes panel top, compact footer, target height and
8 logical pixels of padding. IME dismissal restores y408/28px and details.
No attachment, draft, inventory or server state is changed by layout code.

## Verification

- Failure-first shared group test failed with missing footer group before
  grouping, then passed. The test checks field/action parent separation.
- Shared Rust: 585 passed, 0 failed; includes attachment paging/bounds tests.
- Android: 87 passed, 0 failed; preview: 90 passed, 0 failed.
- Adapter test repeats IME on/off at scales 0.35/0.535/1/2, checks disabled
  Send size, unchanged child identity/local y, details visibility and gutter.
- Both variants packaged sequentially with full `build-android.sh`.
- Java unchanged; Java tests and Windows audio-enabled suite not rerun.
- `git diff --check` passed. Existing build warnings retained.

Final APKs, relative to `mir2-web3/apps/game-client/platform-android/`:

- `android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA-256 `87a507d2de5280ec45ea0faa61e74d99c37e20a4ab9100c829e1d376313a3423`
- `android/app/build/outputs/apk/debug/app-debug.apk`
  SHA-256 `687a6736563c2bd2f2b32d2931a4cecf65455be0923f775c5acbc6a31a3c4925`

## API31 observations

Existing Pixel_5_API_31, emulator-5554, Android12 arm64, 2340×1080 landscape.
No AVD wipe or configuration change. Installed final preview APK and launched
`com.mir2.web3.uipreview/com.mir2.web3.MainActivity --es ui_scene mail-compose`.

- `before.png`: prior APK footer obscured by IME.
- `typed.png`: final package recipient/body visible and same-size Send/Cancel
  above actual IME. Test-only text Recipient/Message; no mail sent.
- `restored.png`: after Back, attachment selection, retap body, Back again,
  shared draft and attachment 1/5 remain; all details/footer restored.
- `cancel.png`: retap body then compact Cancel at physical `(1565,220)`
  returns to the offline mailbox and hides IME. Text targets `(1450,68)` /
  `(1450,110)`; first attachment `(1450,240)` with keyboard dismissed.

An initial validation pass exposed disabled Send remaining small because it
has no Button component. Adapter now sizes all footer child nodes, with test.

## Stability concern, not a passing soak

During a longer final-package attempt at emulator time 22:14:53, lowmemorykiller
killed preview PID7962 at foreground oom_score_adj0, reporting 1206148kB RSS
and low watermark/thrashing. It also killed the IME and launcher. Crash buffer
was empty. This establishes a low-memory termination, not a diagnosed leak
or a mail logic panic. Exact excerpts retained in `lowmemory.txt`.

A fresh cold launch subsequently passed the short typed/attachment/IME/Cancel
flow above. That does not close stability or draft recovery after process death.
Next investigation should measure asset/UI memory over time and transitions;
do not erase AVD data, inflate memory silently, or claim soak acceptance.

## Remaining boundaries

Offline native preview only; no remote Web page, Gateway, live mail request,
real account/save mutation, production deployment, real device or server acceptance.
Resources remain the local staged original-ui pack, not a release/version audit.
Mail body still uses the shared shortened presentation; phone-wide dialogs,
other targets, bottom HUD/chat, multitouch, network/read-model wiring and
physical-device checks remain. This is not whole phone UI completion.

Source and evidence are LOCAL ONLY. PR251 was previously Draft at
`99a1cd1219fe1223d42049de94fc82b61f53e318`; not rechecked or pushed this round.
Full local logs `/tmp/android-mail-{layout-red,layout-green,shared,normal,preview}.log`
and `/tmp/android-mail-{preview,debug}-build.log`; compact extracts `results.txt`.
