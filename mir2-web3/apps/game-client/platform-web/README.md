# Browser shared-client host

This is the first host slice: a small WASM adapter for `client-core` quest
interaction policy. It deliberately has no Bevy dependency so desktop and
touch compatibility renderers use the same rules without downloading the
full renderer. It does not yet replace the existing `runtime` browser renderer
or implement the CP-02 shared in-game UI.

From `apps/web`, run `npm run client-core:build` using Rust 1.95.0 and
wasm-bindgen 0.2.118. `MIR2_CLIENT_CORE_TARGET_DIR` places Cargo output on a disk
with adequate space. `npm run dev` and `npm run build` include this step.

The small JS/WASM package under `public/client-core/<content hash>/` and
`lib/generated/client_core_runtime.json` are committed together. Unlike the
large Bevy assets, this package is small enough to accompany the source.
Builds using `MIR2_USE_PREBUILT_BEVY_RUNTIME=1` also verify and reuse the shared
core package without requiring Rust; `MIR2_USE_PREBUILT_CLIENT_CORE` can override
that choice. Verification checks both artifact hashes and a normalized hash of
the shared source, dependency lockfile, toolchain and generator. Changed source
with an old package fails the release gate. Old hashed packages can coexist for
cached pages; publishing the manifest follows successful artifact generation.

Web uses `NEXT_PUBLIC_MIR2_QUEST_GUIDANCE` to select `crystal` (default),
`newcomer-v1` or `newcomer-v2`. Match the selected realm's server cadence and
native `MIR2_QUEST_GUIDANCE`. This public presentation switch does not grant
server authority. Missing or malformed endpoints do not authorize diary actions.

Verification: native crate tests, `node --test scripts/test-client-core-runtime.mjs`,
Web adapter tests and actual browser gameplay are separate gates. Track progress
in `docs/CROSS-PLATFORM-CLIENT-GOAL.md` from the project root.
