# Opt-in WebGL2 single-surface Quest/Bag prototype

2026-10-01, CP-02/03 bounded round. This contract authorizes implementation and
local verification; it is not WebGL2 UI or whole-client acceptance.

The current GLES device cannot present the second `mir2-quest-ui-canvas` window.
The retained WebGL2 default therefore uses React Quest/Bag. The independent
read-only compositor audit is archived with this round's player QA evidence.

## Fixed startup mode

Enable the prototype only when the first raw query value of `bevyBackend` is
exactly `webgl2`, `bevySharedCanvas` is exactly `1`, and either `bevyQuestUi` or
`bevyBagUi` is exactly `1`. Require the compiled `web-quest-ui` feature and a
WebGL2-only runtime. Do not infer the mode from storage, automatic backend
fallback, or a later owner change. Encoded/nonexact/duplicate later flags must
not bypass the first-value gate. Find the first target key by decoded key, then
require both that key's original spelling and its raw value to be exact; an
encoded earlier key blocks a later plain duplicate rather than enabling mode.
WebGPU and ordinary WebGL2 keep their current
surface and ownership paths.

Keep the primary canvas a direct child of the stage from its first rendered
client mount in this mode. Do not reparent or replace a booted canvas. Retain
the second DOM canvas for the default WebGPU path; never create a second Bevy
Window/GL surface for the WebGL2 prototype.

## One UI surface; retained DOM scene

The primary Bevy Window hosts the existing portable Quest, hint and Bag UI
camera/roots. Keep the camera on render layer 30, explicitly above the inactive
world camera, with a transparent clear on every frame. Reuse existing target
camera, window metrics, stamps, intent sink and geometry observation. Keep ABI
1, current DTOs, protocol, authoritative models and the single equipment ledger.

For the entire opt-in page lifetime, deactivate the primary world camera and
use the existing complete DOM scene path below the shared UI canvas. Suppress
Bevy entity/map ownership and raw map/entity atlas drawing in the Shell for
this mode. Preserve DOM floor/object/entity/projectile, nameplate, lighting,
weather and HUD layers. World/session state and normal server routing remain
live. This deliberately bounded bridge avoids promoting a world+UI bitmap
above DOM HUD, changing cameras on ownership edges, or introducing a cold
fallback transaction. Its performance and complete effect coverage need actual
measurement before any default rollout; it is not the final shared world
compositor.

The UI camera remains active for layout while preparing. Keep its canvas laid
out at full stage dimensions but visually hidden and pointer-inert until the
existing shared owner is ready. Empty/closed UI renders transparent. Read the
presentation from the primary canvas in this fixed mode. Reuse current ready,
asset, active-frame, focus, session and measured-region gates. React remains
the usable owner whenever those gates fail.

## Input and lifecycle

Recognize the primary canvas as the shared pointer target only in this mode;
otherwise retain the second canvas target. Preserve the Bag down-origin lease,
two-finger exclusion, world-origin mouse handling, Quest modal blocking and
blur/resize/owner-change cancellation. No frame delay may substitute for tree
readiness. Keep More/text-input/unsupported layout compatibility handoffs and
ordinary logout/save behavior.

## Ownership and validation

- Root owns this contract, source/artifact freezes, publication of local build
  artifacts, actual ordinary clients, integration and progress/QA docs.
- Sol/high Rust owns `runtime/src/lib.rs`, `runtime/src/quest_ui_host.rs` and
  an optional bounded mode helper. Root additionally authorizes the existing
  `bag_ui_host::install` cfg gate to compile for WASM WebGL2; its behavior and
  ordinary `install_unsupported` remain unchanged. It does not edit the painter,
  Web, backend or global docs.
- Sol/high Web owns Page, Shell, Shell prop types, scoped CSS, a bounded mode
  helper and focused Web tests. It does not edit Rust, ledger/protocol,
  generated artifacts, services or global docs.
- Luna/medium reviews sealed source/artifact/input/wire/save evidence and
  records failures and limits independently.

Focused checks cover exact opt-in/default isolation, one primary UI window,
inactive world camera with active UI layout, primary presentation/pointer
mapping and unchanged default routing. Compile both WASM backends, run the
appropriate shared/native checks and the runtime asset budget. Freeze changed
and reused sources plus the exact four runtime files before actual QA.

Actual browser gates: direct WebGL2 startup and advancing frames without a
second-surface panic; transparent pixels and retained scene/HUD; desktop and
landscape touch Bag/Inspect/Quest controls; ordinary equip/restore/move receipts
and normal save/relogin; compatibility, resize and focus recovery; unchanged
WebGPU and ordinary WebGL2 defaults. Retain failed attempts. Desktop Chrome
WebGL2 forcing/emulated touch is not a WebGL2-only physical device, Android/iOS,
DPR2 or whole-client acceptance.
