# Android town-revive touch intent

Source `06a5dea1a`; only `platform-android/src/mobile_ui.rs` changed.

Android's collapsed rail now exposes a 64x48 OS-logical Revive target when
both shell/shared core are InGame and the current read model has max HP >0
and HP <=0. Unknown/alive/inactive state and amount modals cannot request it.
The button dispatches existing UiAction::TownRevive into the shared UiEffectQueue.
It does not restore HP, move the player, implement resurrection rules or send
an unapproved network request. The shared death overlay is unchanged.

## Verification

- API 31 Android 12 arm64 Pixel_5 emulator, 2340x1080, offline preview only.
- Before: death scene only displayed the desktop V-key instruction.
- After: Revive is visible without expanding Panels. An 800ms touch at
  `(2230,340)` records one preview intent event; Android tracing and stdout
  each print that same event (two sink lines, not two requests). HP remains 0.
- Alive HUD scene hides Revive. Screenshots and process log are included.
- Host tests: normal80/preview82 (overlapping). Added assertions cover
  exactly one shared effect while held, HP unchanged, known-dead/both-screen
  guards, and 64x48 target sizing after stage scale. Existing modal guard is reused.
- Both native APK package gates pass. Shared source and Java are unchanged;
  prior shared576/Java8 results are not fresh runs for this checkpoint.

APK paths under `apps/game-client/platform-android/android/app/build/outputs/apk/`:

- debug/app-debug.apk SHA-256:
  `00e325eeb2ce10ba373456027ead0b27659288a0eb4a09d6a5fb1dfd719bdc0f`
- uiPreview/app-uiPreview.apk SHA-256:
  `60ac93b37917111ddaf302ddcea2ffb746bb92a1a75e64d381127e6da44d6f4a`

Builds preceded source commit; the additional target-sizing test was added
after preview compilation, with no runtime-code change. Full host suites were
rerun after that test. Before APK was previous source bed749479.
APKs/assets/keys are ignored; generated log whitespace is normalized.

## Boundary / remaining work

This establishes touch → shared intent only, NOT actual resurrection.
The shared UI effect queue is not yet connected to the real Android Java
gameplay socket; request/result lifecycle, disconnect cancellation and stale
intent handling must be connected before live acceptance. Do not later flush
old UI effects across login generations. No success/pending UI is fabricated.
Desktop V-key copy is still present in the shared death overlay.
Physical-device, approved Gateway, full-screen layout, world projection and
touch movement/combat remain open. No real account/save/production operation.

PR #251 read-only check remains Draft at 99a1cd121; source is LOCAL ONLY.
No repeated push attempt or proxy change; original checkout remains untouched.
