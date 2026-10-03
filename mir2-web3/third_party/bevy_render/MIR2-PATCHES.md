# Mir2 Bevy render compatibility patch

This directory is an exact vendored copy of the crates.io `bevy_render`
`0.19.0` source, under its existing MIT or Apache-2.0 licenses, plus the narrow
Mir2 changes listed below. `Cargo.toml.orig` remains the crates.io manifest and
the package checksums/lock files are retained for provenance. The source is
temporary compatibility code until the equivalent Android GLES lifecycle and
downlevel-format behavior exists upstream.

Only these upstream source files have behavioral changes:

- `src/view/window/mod.rs`
  - Requests alternate sRGB surface views only when the adapter advertises
    `SURFACE_VIEW_FORMATS`.
  - Uses a stable linear RGBA8 Android fallback, publishes the confirmed
    surface format before the next camera extraction and avoids requesting an
    unsupported view reinterpretation.
  - Drops acquired textures and surfaces across Android lifecycle boundaries,
    including the case where Android reuses the same raw native-window pointer.
- `src/view/mod.rs`
  - Requests alternate sRGB views for intermediate textures only when the
    adapter advertises `VIEW_FORMATS`.

Pre-existing trailing whitespace was also normalized in
`src/color_operations.wgsl` and `src/maths.wgsl` so the repository's
`git diff --check` gate remains clean; shader behavior is unchanged.

Mir2's Android-only runtime selection is intentionally outside this vendored
crate in `apps/game-client/runtime/src/lib.rs`: it selects wgpu GLES, applies
WebGL2-compatible limits, disables unsupported OIT startup, uses single-sample
2D rendering, pauses the camera while the Activity surface is replaced and
keeps Android rendering non-pipelined. Desktop and WASM retain their existing
backends and plugin behavior.
