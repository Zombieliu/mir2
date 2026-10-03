# Unsent shared UI command invalidation

Source `13f001d0b`; Android-only `platform-android/src/shared_shell.rs`.

The shared UiEffectQueue is not wired to the Java gameplay socket yet. Before
adding that consumer, prevent old unsent GatewayCommand effects from crossing
the currently observable session boundaries:

- DISCONNECTED, UNCONFIGURED and CONNECTING phase events clear network effects
  immediately while reading the event batch.
- Logout clears them before asking the Java host to disconnect.
- PreUpdate and PostUpdate discard network effects when not InGame or when
  there is no single focused Window. The second pass catches later UI producers.
- Non-network effects retain their order. No server rollback is inferred;
  already-sent operations and authoritative receipts are not affected.

Tests normal82/preview84 (overlapping) pass. New tests cover network-only
filtering, retained local effect order, idempotence, missing/unfocused Window,
inactive shell, active preservation, producers between cleanup passes and
resume retaining only newly generated effects. Both APK package gates pass.

API 31 Android 12 arm64 Pixel_5, 2340x1080, offline preview verification:

1. Launch death fixture; tap Revive `(2230,340)`.
2. Process log records TownRevive queued, with no discard while active.
3. Home (`keyevent 3`), then bring the same Activity to front (HOT launch).
4. Log records `ANDROID_UI_PREVIEW_DISCARD count=1`; screenshot shows the
   shared Connection Lost dialog. There is no new intent event or local revival.

Each tracing event also appears through stdout; two sink lines are one event.
The fixture label stays visible after background; it is not a live session.

APK paths under `apps/game-client/platform-android/android/app/build/outputs/apk/`:

- debug/app-debug.apk SHA-256:
  `00cf7718fd7ee7b8e3529984c223444261e3175608bdeb383ec55d15ed4d5ad3`
- uiPreview/app-uiPreview.apk SHA-256:
  `d94333da7a547fb741f4e119f35ca4d6633d953fdff9b419f477495dbd93a8f1`

Builds/tests used source subsequently committed above. APKs/assets/keys are
not committed; log whitespace normalized. Shared/Java unchanged, so earlier
shared576/Java8 results are not fresh results for this checkpoint.

Boundary: this is cleanup of one UNSENT shared effect queue, not a completed
transport generation/acknowledgement implementation. Other native intent
queues, in-flight commands, stale inbound callbacks, pending read models and
authenticated login-generation tagging still need explicit integration before
enabling a live consumer. No approved Gateway, physical device, real login,
save or production mutation. Whole Android/gameplay/full-screen gates remain.

PR #251 read-only check remains Draft at `99a1cd121`; this source is LOCAL ONLY.
No push retry, proxy change or original-worktree edit.
