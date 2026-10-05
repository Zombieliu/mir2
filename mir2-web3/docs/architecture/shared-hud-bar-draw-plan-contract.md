# Shared EXP/WEIGHT Rust draw-plan prototype contract

Root leads architecture and integration. This is a separate source prototype after the independently reviewed WEIGHT v2 candidate (372 declared files, 17 changes). It does not change or replace frozen EXP63280e, WEIGHT34b06f or sealed707 evidence. Desktop resumption is unanswered; no GUI, browser, authentication, gameplay or private-store action is authorized by this lease.

## Why this representation

EXP/WEIGHT currently share a flattened canvas with HP/MP/Quest/Bag. On a shorter, zero or unavailable bar, React fallback can expose previous bar pixels while another owner retains the canvas. A full-slot hole can cut interactive footer/control pixels. PostUpdate, a setter acknowledgement, requestAnimationFrame or a delay is not a presented-pixel fence.

Prototype a versioned Rust sprite draw plan and a thin Canvas2D renderer. Reuse Rust state validation, normalization, the original weight selector and horizontal crop arithmetic. The browser owns only rendering resources, committed identity and presentation. It must not recompute the f32 threshold/crop decisions or create a second gameplay model. Native HUD behavior stays unchanged.

## Exact product write lease

New files:
- apps/game-client/client-bevy/src/portable_hud_bar_draw_plan.rs
- apps/web/lib/bevy-hud-bar-draw-plan.ts
- apps/web/scripts/test-bevy-hud-bar-draw-plan.mjs

Existing files:
- apps/game-client/client-bevy/src/lib.rs
- apps/game-client/runtime/src/quest_ui_host.rs
- apps/game-client/runtime/src/lib.rs (only if needed for the explicit export/startup boundary)
- apps/web/lib/bevy-quest-ui.ts
- apps/web/lib/use-bevy-quest-ui.ts
- apps/web/app/components/original-client-shell-types.ts
- apps/web/app/page.tsx
- apps/web/app/original-client-shell.tsx
- apps/web/app/globals.css
- apps/web/scripts/test-bevy-experience-bar.mjs
- apps/web/scripts/test-bevy-weight-bar.mjs
- apps/web/scripts/test-bevy-hp-orb-ui.mjs
- apps/web/scripts/test-bevy-quest-ui.mjs

The four existing scripts may change only where the new explicit capability/lifecycle requires a meaningful adjacent assertion. Preserve all legacy, raw-pair, layout, passive-input and independent-owner checks. No broad reformatting. All unlisted product paths are read-only. In particular keep quest_ui.rs, native crystal_ui/hud.rs and hud_bar.rs, both existing portable bar painters, the EXP adapter and WEIGHT authority/projection adapter byte-identical. No dependencies, simulation/protocol/Gateway/auth/save/assets/font/native-entry changes. Root alone updates global documents and leases.

## Fresh runtime and capability boundary

Choose the route before plugin/root installation. A fresh new Web lifetime with the exact supported draw-plan version must never install these two painters into the flattened interactive target. Leave HP/MP/Quest/Bag plugins, target camera, operation intents and pointer routing intact. Native must retain its old route. Lean or unsupported runtimes retain DOM fallback.

Use an explicit version-1 capability and getter, never a truthy/malformed inference. The returned record is a plan-valid receipt, distinct from image-ready/layout/GPU-painted status. Do not label Canvas2D operations as Bevy GPU painting. On the new route, legacy EXP/WEIGHT records advertise unsupported/not-ready with no painted geometry. New clients retain raw pairs and slots through the exact supported plan branch without falsely enabling legacy capabilities. Older Web clients get honest DOM fallback; old deny-unknown DTOs receive no new optional token/plan fields. New clients retain a separately limited legacy route when the capability is absent. No hot retrofit of a framebuffer already containing old bar pixels is claimed safe.

Omitting the painter plugins must not omit resources required by host systems. Initialize the four public EXP/WEIGHT HostContext and Observation resources explicitly with default observations, or branch the actual systems so omitted resources are not required. Install none of their painter roots, sync/observation systems or private image leases on the new route. A route-specific fixture must exercise actual required resource resolution. PostUpdate ordering is not a GPU/layout proof. Native and legacy routes retain their existing plugin installation.

## Applied causal plan

Generate plans only from the ingested/applied snapshot, with bounded immutable metadata. Include schema version, applied generation/revision, a common runtime/lifetime identity, separate EXP and WEIGHT commit tokens and visual sequences, each exact full logical slot, safe raw pair/known state, allowlisted sprite and Rust-computed source/destination crop. Bound serialized output, finite rectangles and each opaque identity/token to at most 128 ASCII bytes; visual sequences must be bounded nonnegative safe integers. Reject malformed or unsupported records to DOM.

The host DTO currently has no player object identity. The browser proves the common current generation/player/runtime lifetime. Each bar token binds only that bar's raw pair/known state, slot, presentation and blocking state within that common identity. Submit metadata only behind the exact capability and echo it only from the applied snapshot; never manufacture a Rust player identity. Ownership rechecks the live common lifetime and that bar's DOM raw pair and measured full slot. A sibling-only pair, image, slot or commit-token change must not invalidate the other output or its pending decode. Shared runtime/generation/player resets or global modal/presentation changes may withdraw both. Late callbacks compare the common lifetime plus their own bar token, never the sibling token. An accepted setter is not an applied plan.

Keep each visual token/sequence stable across movement-only revisions while its exact own inputs and common identity remain unchanged. Applied revision is monotonic metadata, not an equality requirement against every movement snapshot. Reuse an older applied plan only while its live common identity, own token, raw pair, slot, presentation and blockers still match. Use the running host heartbeat/status for liveness; a stable visual sequence is not evidence of a stall.

Use explicit draw, known-empty and withdrawn states. Genuine zero and a positive raw value whose Rust crop floors to zero both clear without loading a sprite or using a one-pixel substitute; unavailable/default-like/partial/invalid authority clears and stays DOM. Preserve genuine max1 and the WEIGHT v2 missing-both preservation/one-sided withdrawal semantics. Nothing in the new renderer changes packet projection or stored character data.

## Sprite and geometry resources

EXP fill is Prguse/8.png, 1004x8, SHA dfca5edbca559509d20fd0cade0b5d8d906c06773b1d926326e042ec1fd9be1e. Prguse/1.png is the retained 1024x152 HUD backing and is not redrawn by this adapter. WEIGHT remains Prguse/76 (76x12, SHA9bd1d3...), UI_32bit/473 (76x12, SHA18c357...) or /472 (76x12, SHAa008c9...), using the unchanged Rust selector. Full hashes are in read-only-references.json.

Manage two thin passive canvases with native backings 1004x8 and 76x12: 8,944 pixels /35,776 raw RGBA bytes before browser overhead. Never allocate from unbounded stage/DPR/DTO dimensions. Apply the measured existing anchor/CSS stage scale once, with pixelated presentation; source crop has no DPR multiplier. Use exact Rust source and destination rectangles, nine-argument drawImage, alpha/source-over and smoothing disabled after every resize. Preserve source alpha and all backing/text; no matte, full-stage image, GPU readback or world renderer.

Keep at most four allowlisted decoded handles/attempt records in the current generation (one EXP plus three WEIGHT) to preserve per-path reuse. The cache is generation/lifetime-scoped, never bar-commit-token-scoped: changing a raw pair or switching back must reuse the successful or failed path record. Acquire only selected paths, once per owned generation/key; remember failure instead of polling retries. On generation/runtime/cleanup release handles and detach/cancel owned pending work; any late result fails its immutable token before drawing. Logical cache retention is bounded; actual browser network/decode cancellation, resource overhead and request counts remain live gates. Do not claim global HTTP counts or cancellation from pure fixtures.

## Browser composition and ownership

Mount each output as a passive positioned sibling before both existing shared canvas elements in the same stage stacking context; add no transform/opacity/isolation wrapper that creates another stacking context. Never mask/clear the existing interactive canvas. Keep both outputs at passive z9 between backing z8 and text/controls z10; DOM order places them below both equal-z shared canvases. Never inherit touch Quest z40 or Bag z90. Extend all three retained backing/content interleave groups and the wide-mobile transform-to-left correction to the honest canvas2d owner, preserving legacy bevy/React selectors. Hide only each affected DOM fill. Do not add canvas2d ownership to Quest/Bag pointer capture or either shared-canvas visibility predicate.

Commit/layout/identity changes synchronously clear the affected complete backing and return its fill to DOM before accepting a replacement. Moving the independent output cannot leave an old position behind. A still-current sibling bar remains independently usable. Apply existing modal/touch blocking and preserve footer, close/control and input semantics. Closing a blocking panel starts with DOM until a current valid plan is drawn. Missing geometry, hidden/stalled/cleanup, image failure or any drawing exception clears/withdraws only the affected output.

Browser receipt means validated plan plus successful issued clear/draw operations, not compositor-presented pixels. Keep all actual alpha, sampling, overlap, DPR2/resize, backend, device and original-media-origin acceptance gates open.

## Required source evidence and handoff

One Sol/high code worker owns the exact write set. Use read-only references and frozen current baseline; no second writer. Tests should exercise real planner/controller functions and integration, not duplicated crop formulas or source assertions alone. Cover raw authority, genuine zero/max1, thresholds, unsafe geometry/token/plan, stale/late decode and plan, switching/failure/no-hot-retry, multiple layout moves, generation/runtime/cleanup, independent sibling output, modal and legacy field stripping. Run the existing focused Web matrix and direct TypeScript compiler, Rust planner and adjacent host fixtures with the established portable-quest-ui feature command, and appropriate wasm32 checks for supported UI backends. Retain every failed raw receipt and exact command. Pure ctx fixtures are not pixel tests.

Do not publish canonical runtimes, start Next/servers, operate GUI, call HTTP/Gateway/auth/gameplay, commit/push/deploy or read private data. Root owns those later stages. After tests, freeze the full baseline plus authorized new sources, exact changed-file manifest, 20 read-only references, raw stdout/stderr/exit receipts and HANDOFF in implementation/. Release source ownership for independent review. Stop and report a concrete compile/design obstacle if the required boundary cannot be implemented within this lease; do not replace it with timing/mask/opaque tricks.

Actual browser/native/device pixels, ordinary gameplay/persistence, public origin, full HUD/locales/performance, CP02-04 and human acceptance remain open. The overall goal is active.
