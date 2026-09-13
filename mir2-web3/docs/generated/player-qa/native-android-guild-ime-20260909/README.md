# Guild notice IME — bounded offline regression

Source `77edc7afb`, isolated Android branch, 2026-09-09 Asia/Shanghai.

## Debug report (investigate)

- Symptom: Guild → Edit opens the keyboard but pans notice text off the top.
- Root cause: stage fitting queried only FormInput, omitting player draft fields;
  its fallback used the chat HUD bottom (750) for the guild notice.
- Fix: resolve player draft fields too; reserve notice text at shared guild
  panel top +61 with 12 pixels per shared permitted line (8), plus 8px gutter.
  No shared source, protocol, authentication or server rules changed.
- Regression: geometry assertion failed with 750 vs 333 before the fix.
  A second test checks shared Unicode/newline filtering and pending/closed draft guards.
- Fresh evidence: API 31 Pixel_5 arm64 emulator, 2340x1080; edit `(802,821)`,
  Enter and text through line 8, then Back hides IME and Cancel `(870,821)`
  restores the original notice. All eight lines remain visible above the IME.
- Status: DONE for this visibility defect, not whole Android UI acceptance.

Preview guild fixture now grants only the offline `notice` permission, exposing
the real shared Edit control. Preview networking remains compile-time disabled.
No Publish was pressed; cancellation did not replace the model notice.

Normal host tests 78/78; preview 80/80 (overlapping). Both APK package gates pass.
Shared/Java source unchanged; prior 575/575 and 8/8 were not rerun this round.

APKs under `apps/game-client/platform-android/android/app/build/outputs/apk/`:

- debug/app-debug.apk SHA-256:
  `1a6d13d97af6159e4acbbfb62a9d0e273044b7e95b4b88f841507d702f72a99b`
- uiPreview/app-uiPreview.apk SHA-256:
  `459eaa76a524f0b67bfdf1c7d48ce29045d97afc097f8a9014a0a43de4aa3cf6`

Before screenshot used the added permission fixture before geometry correction
(preview hash `bc412efea6c21326bd74e239ab7a9cbfe0e497b66936226effc0f57a917cef86`).
Builds ran before source commit; final source differs only by rustfmt whitespace
from the tested preview inputs. No APK/assets/keys are committed.

Remaining: notice-body retap reopening is not wired; Back then Cancel works.
Publish/Cancel remain behind IME until Back hides it. Other sizes/IME/fonts,
physical devices, live receipt handling and full-screen layout are not accepted.
Related prior defects: gold-editor and mail-editor geometry used the same HUD
fallback. A general layout/field-bounds solution remains preferable for broader adaptation.

PR #251 read-only check remains Draft at `99a1cd121`; this source is LOCAL ONLY.
No push retry, production operation, real login/save or original-worktree edit.
