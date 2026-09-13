# Native Android GLES emulator recovery — 2026-09-12

## Result

This Android-only slice started from
`codex/android-player-journey@f12af5ef674534a8aacf0f7a72acaca24179e0a1`.
The API31 ARM64 emulator now selects wgpu's GLES backend instead of the broken
Vulkan/SwiftShader path and renders the complete bounded offline Bichon
specimen. The visible frame contains 849 map draws, seven authoritative-shaped
objects, twelve entity layers, effects, drops and the shared Crystal UI.

Bevy 0.19's downlevel surface path also required a narrow vendored
`bevy_render` compatibility patch. Alternate surface/intermediate sRGB views
are requested only when the adapter advertises the corresponding capability;
the Android surface uses a stable linear format and is invalidated across the
Activity lifecycle. The runtime pauses the camera for three main-world frames
while the replacement surface settles and keeps Android rendering
non-pipelined. This prevents the old sRGB mesh phase from being submitted to a
new linear GLES attachment.

## Verification

- Rust formatting passes for the runtime and Android crate.
- Android `ui-preview` tests pass 154/154.
- The API31 ARM64 target check passes.
- The exact local 35-atlas/102-page entity release-alignment package passes.
  The 376,766,188-byte `uiPreview` APK has SHA-256
  `728822742c5ccc6de29cd414e74d899d5c5baa148187ae9f829be4f8128257fa`.
  The APK and local asset packs remain ignored and uncommitted.
- Streamed install and cold launch pass on emulator 37.1.11. The renderer
  reports `Android Emulator OpenGL ES Translator (Apple M1 Max)`, OpenGL ES
  3.0, backend `Gl`.
- Cold render emitted `ANDROID_WORLD_RENDER_READY` at center `(302,634)` with
  849 map draws, seven entities and twelve entity layers. The visible capture
  is `world-render-cold.png`.
- Home/resume kept the same process (`PID 7001`) and produced the shared
  offline-background recovery surface. A 300-second post-resume soak retained
  that PID with zero wgpu render errors, panics, exits, GLES errors or
  incomplete-framebuffer diagnostics. The final visible capture is
  `post-resume-soak.png`.

## Evidence boundary

`com.mir2.web3.uipreview` is a labelled offline fixture with networking
disabled. The post-resume `Connection Lost` surface is the expected shared
offline-background recovery state, not evidence of a live reconnect. No
approved WSS endpoint, account or physical Android device was used. This slice
proves visible emulator rendering and render-surface lifecycle recovery; it
does not prove real login, live `StartGame`, online map transition, physical
device behavior or human acceptance. Public Web/Android release alignment was
not evaluated by this local-manifest gate.
