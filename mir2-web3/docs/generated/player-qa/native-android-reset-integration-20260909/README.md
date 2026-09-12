# Android shared reset integration audit

Source `7770110db` adds one Android host integration test, with no runtime change.

Inspection found the existing shared plugin already installs
observe_native_session_boundary and apply_overlay_session_reset. Do not add a
second implementation of player-queue or pending-state reset in Android.
The observer requests a reset on StartingGame/InGame → inactive transitions;
the reset system runs before it, so cleanup completes on the next Update.

The new test follows that order for Login and ConnectionLost destinations:

- Seed an unsent storage operation and old chat intent/draft.
- Observe one reset revision, then run the next update.
- Player intent queue, ordinary pending state and draft are cleared.
- Enqueue another storage operation through the same queue; its request ID
  differs from the earlier one. Queue clearing retains the ID generator.

Fresh verification:

- Android normal83/preview85 pass (overlapping).
- Shared pending_operations tests: 31 passed, 545 filtered. Not a fresh full suite.
- Normal native APK package gate passes; runtime is unchanged and SHA-256
  matches the prior build: `00cf7718fd7ee7b8e3529984c223444261e3175608bdeb383ec55d15ed4d5ad3`.
  Path: `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`.
- Preview APK and GUI capture were not rebuilt/repeated for a test-only change.
  The prior API 31 background/resume capture is in native-android-ui-session-20260909.

Clarification of the previous checkpoint: Android's early command filter
preserves non-network effects, but the existing complete shared session reset
later drains the remaining effect queue. Therefore preservation by the early
filter is not a claim that every local effect survives complete session reset.
Shared reset has an exact GameShop receipt preservation path; this new test
checks ordinary reset only and must not be cited as live transaction recovery.

Remaining: real Java gameplay consumer, correlated host/inbound generations,
already-sent/unknown outcomes, live account transition and device validation.
No new runtime rules, network traffic, real account/save, APK/resource/key commit,
production operation or original-worktree edit. No new UI completion claim.
PR #251 remains Draft at read-only verified 99a1cd121; this is LOCAL ONLY,
with no retry of the unchanged publication failure. Logs normalized for whitespace.
