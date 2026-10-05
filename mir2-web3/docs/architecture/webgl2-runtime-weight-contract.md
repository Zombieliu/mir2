# WebGL2 runtime weight: controlled feature pair before package selection

2026-10-01. The compact Quest f7 checkpoint remains an experimental UI above
the full DOM world. Its shared-UI-capable GL2 package is31,428,382 raw and
5,870,926 level9-gzip bytes, even on a default GL2 page that keeps React owners.
The previously sealed e275 GL2 package is21,419,537 raw/5,277,427gzip. This
comparison suggests newly reachable UI code, without attributing bytes to a
particular crate or proving a startup-time improvement.

## First bounded implementation and measurement

Add a compile-time `webgl2-shared-ui` capability that includes the existing
`web-quest-ui`/GL2 feature dependencies. The supported GL2 Quest/Bag/hint installer
and primary-UI/world-camera-off startup path must require this capability.
`webgl2,web-quest-ui` keeps the existing ABI1 exports, unsupported shared-UI
diagnostics and ordinary world rendering; raw opt-in queries cannot override a
missing compiled capability. `webgl2,web-quest-ui,webgl2-shared-ui` keeps the
existing exact first-raw-value behavior. WebGPU behavior stays the current path.
The same UI source/intent ledger remains shared; no second App/device/session
or runtime state is introduced.

The canonical two-package build must continue building its current UI-capable
GL2 package by explicitly adding the new capability when MIR2_BEVY_QUEST_UI=1.
Otherwise the existing immutable page mounting decision would be inconsistent
with its newly lean package. This first slice changes compile reachability and
measures a separately retained default/shared pair. It does not change Web URL
selection, current published package counts, manifest schema or production routes.

One Sol/high worker may edit only:

- `apps/game-client/runtime/Cargo.toml`
- `apps/game-client/runtime/src/lib.rs`
- `apps/game-client/runtime/src/quest_ui_host.rs`
- `apps/game-client/runtime/src/bag_ui_host.rs`
- `apps/game-client/runtime/src/webgl2_shared_ui.rs`
- `apps/web/scripts/build-bevy-runtime.mjs` (existing feature commands only)

Back up complete current files before mutation. Preserve prior changes in these
high-conflict files. Root owns the feature design, cross-file review, release
pair builds, canonical runtime publication and actual clients. A separate
Sol/high reviewer is read-only; Luna/medium checks sealed provenance/metrics.
Do not start source edits until the compact f7 independent audit is finished.
Do not run Cargo concurrently with root or publish packages from the worker.

Meaningful checks cover capability absent/present versus exact/encoded/duplicate
raw startup flags, default world-camera activation, ABI/status/intent isolation,
existing GL2 shared Query and WebGPU behavior. Run native runtime tests and WASM
checks for the default/shared GL2 and GPU UI feature combinations. Preserve the
existing no-backend UI-only compilation path; exclude only lean GL2 from the
supported installers, rather than removing that fallback by implication. Existing
native-only portable font assertion remains an explicit scoped limitation.

After source freeze, root serially builds both GL2 variants with the same pinned
toolchain, release profile and bindgen into a new QA archive. Retain full command
lines, frozen bounded inputs, JS/WASM copies/hashes, raw/gzip sizes,
WebAssembly.validate, exact export parity and the existing31MiB/7MiB/200KiB
limits. A smaller default artifact is a byte/compile-reachability result, not
per-client transfer, initialization, memory or mobile performance acceptance.
If reachability does not reduce weight, investigate the measured sections before
changing budgets or introducing another architecture.

## Subsequent integration gate

A real per-client download change requires three immutable selectable packages:
GPU, default GL2, shared GL2. Resolve the exact raw-first mode and complete
shared-artifact availability before first canvas mount/module import; keep the
choice fixed for the page. Missing/failed shared artifacts yield a compatible
DOM/React mode or explicit reload, with no silent lean/shared-canvas mismatch.

The version generator, atomic publisher/rollback, immutable local rewrite,
prebuilt fetch/remote release validators, release builder, smoke/byte budgets
and path/file-count allowlists must agree on the complete bundle. Retain the
existing explicit unsupported diagnostics and one runtime/ledger. Root will
scope that work after the controlled pair supplies evidence; the first slice
does not claim that an ordinary page downloads the lean artifact.

Integration acceptance requires actual ordinary default/shared/GPU fallback,
missing/load/boot failures, local UI/input/save/relogin and fresh/cached resource
timing. Physical devices, CPU/memory/first-usable-frame performance, full world
composition, locale/HUD and CP-02–04 stay open. No production deployment or
whole-goal completion follows from this feature pair.
