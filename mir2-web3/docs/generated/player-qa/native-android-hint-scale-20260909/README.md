# Scaled hint positioning — 2026-09-09

Source `8423b220c` (full SHA in source-head.txt), isolated Android branch,
LOCAL ONLY. Shared presentation file `crystal_ui/widget.rs` only.

## Debug report (investigate)

- Symptom: after moving minimap controls to the phone edge, their hover
  hints appeared around the middle of the screen.
- Root cause: window OS-logical cursor/bounds were written directly into
  Node::Px, which is additionally scaled by UiScale. This is a coordinate
  space mismatch, NOT a hardcoded 1024-wide clamp as the earlier evidence
  tentatively described. ComputedNode size/inverse_scale_factor already
  gives unscaled UI size.
- Fix: divide cursor/bounds once by UiScale for ordinary and rich-item hint
  roots; include effective scale in layout invalidation. Do not subtract
  the centered stage offset from standalone hint roots. Missing UiScale
  defaults to one; invalid scale input also falls back to one.
- Failure-first test: at scale0.5 the old system wrote x1200 instead of
  x2400. System tests now cover scale0.5/1/2 and remeasurement visibility
  for ordinary and rich-item hints, in addition to prior edge-clamp tests.
- Fresh tests: shared581/581, Android normal84/84, preview86/86. Rust1.95.0,
  offline through platform-android manifest. Java tests were not rerun here.
- API31 Android12 arm64 emulator-5554, 2340x1080: preview installed and
  cold-launched hud. Touch (2302,38) collapses map and shows Mini Map hint
  near the button; touch (2160,67) opens Mail and shows its hint near that
  button. See both PNGs. Rich-item positioning has system tests, not a fresh
  populated rich-item screenshot in this checkpoint.
- Status: DONE_WITH_CONCERNS for the coordinate mismatch. All-device cutout/
  IME tooltip containment, rich-item visual acceptance and whole-screen
  layout/gameplay input masks remain open. No real player loop is claimed.

Full-script builds with external assets and OpenJDK21 pass. Paths relative
to `mir2-web3`:

- `apps/game-client/platform-android/android/app/build/outputs/apk/uiPreview/app-uiPreview.apk`
  SHA256 `062ae55aef610d9fe8bf679eb6820f8a9f51d0ad6fc3ecf16534fd3966d132ce`.
- `apps/game-client/platform-android/android/app/build/outputs/apk/debug/app-debug.apk`
  SHA256 `5acacb7dcb21467d120d825c7316b58c6aa3aa62cee7c85c2548488a66266942`.

Compact extracts tracked; full logs remain locally at `/tmp/android-hint-*.log`.
No APK/resources/keys committed. PR251 API still Draft at remote99a1cd121;
no failed push retry, production change, real login or save mutation.
