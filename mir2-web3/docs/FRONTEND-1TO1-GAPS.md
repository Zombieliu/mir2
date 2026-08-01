# Frontend 1:1 Gaps

Last updated: 2026-08-02

> 2026-08-02 mobile PWA/fullscreen shell closure: Player Web now publishes a
> standards-based fullscreen/landscape Web App Manifest with 192px, 512px,
> maskable, and Apple touch icons. Root metadata includes `viewport-fit=cover`,
> explicit iOS standalone/status-bar tags, and a dynamic-viewport page shell.
> The mobile-only install surface handles Android `beforeinstallprompt`, iOS
> Add-to-Home-Screen guidance, app installation/display-mode changes, and a
> user-gesture Fullscreen API plus advisory landscape lock fallback. Copy is
> available in English, Brazilian Portuguese, and Chinese; dismissal storage
> fails open in private browsing. It reuses the existing asset Service Worker
> rather than adding a competing worker. PWA contract/icon tests, responsive
> stage tests, TypeScript, HTTP manifest/metadata/icon probes, and the direct
> Next 16 production build pass. Automated local visual capture is not claimed:
> the in-app browser policy rejected control of the `127.0.0.1` tab. Physical
> iPhone and Android installed-mode login/wallet/fullscreen acceptance remains
> a human/device gate, separate from Crystal scene parity.

> 2026-08-01 Crystal map-weather/light override closure: generated map data now
> carries `map_dark_light` and `weather_particles` from `Server.MirDB`; typed
> `MapInformation`/`MapChanged` browser events preserve lights, dark tint, and
> weather bits; Web resolves fixed map light before global `TimeOfDay`, applies
> all five Crystal night tints, and renders Fog/Ember/Snow/Rain/Leaves variants
> from seven selectively exported `Weather.Lib` base frames. The layer stays
> lazy and compositor-bounded. Developer Compose and direct debug builds default
> to Day for readability; Release production remains UTC-dynamic, while named
> or numeric overrides retain deterministic Dawn/Day/Evening/Night QA.

> 2026-08-01 low-end rendering/resource closure: screen-staged prewarm removes
> character-selection and game/HUD packs from login blocking work, serializes
> later stages after Service Worker setup, and lets low tier skip optional audio
> and scene-frame scatter. Standalone WebGL2 map residency is now byte-bounded
> with visible-page pinning, LRU low-watermark eviction and explicit texture
> release on replacement, disable/Bevy takeover, context change and unmount.
> Cold atlas pages load before canvas clear so crossing a page boundary keeps the
> last complete frame rather than flashing transparent. The map shelf packer no
> longer rounds exact 4096 content up to 8192; all 40 generated pages are at most
> 1024x4096, and build/dev gates reject stale oversized manifests. Full frontend
> logic, TypeScript, focused tests and a forced-low live login pass with no
> browser warnings/errors. The live immutable 20260730 R2 release still contains
> the two old 8192 pages and requires a new release rather than an in-place
> overwrite. Physical 4 GiB Android soak remains the support gate; this resource
> work does not change the final human Crystal visual/feel gate.

> 2026-07-23 final deterministic frontend Candidate closure: the remaining
> fixed-font, chat-history, and actor-phase work is implemented. Eight exact
> Crystal acceptance strings are exported through Windows TextRenderer at Arial
> 8pt/96 DPI with source-accurate outline/background semantics and verified ARGB
> hashes; dynamic text uses the normal accessible renderer unless its complete
> key matches. Chat renders the original 17 types/colours, 614px wrapping,
> four-line history, filters and scroll position with no timestamps, driven by
> real shared Gateway broadcasts instead of capture-only startup strings.
> Persistent Rust animation state now owns each object's incarnation, seeded
> idle/harvest phase, FIFO action queue, death and revive lifecycle, and supplies
> one pose to the Bevy/WebGL2/DOM presentation paths. Game-screen lifecycle
> transitions reset the bridge so relogging cannot reuse a prior action queue,
> while an in-game network reconnect preserves visual continuity.
>
> Evidence `cwp-20260723-r40-gdi-chat-final` at Bichon `0 @ 328,275`, light 1,
> runtime `bevy-e9d354eada933661`, is 100% automated Candidate with 0 critical
> console errors and 0 non-favicon 404s. It records 6 exact GDI nodes, 12 Rust
> animation poses, the real `Online/LineMessage/Online/Online` four-line state,
> 89% world similarity, 88% full HUD, 91% HUD UI, 84% chat, and 87% MiniMap.
> Strict current WebGPU and WebGL2 movement captures remain clean, and the
> native/current-Web four-action report aligns all actions and emits 4/4 frame
> pairs. No automated P0/P1 frontend gap remains for this scene. The only open
> item is final human **Accepted** visual/feel review; 24% full-window and 26%
> world thresholded pixel change still includes different roaming actor
> positions, idle/effect sampling, and compositor output, so it is not described
> as bit-for-bit identity or as a movement defect.

> 2026-07-22 reproducible developer-handoff closure: the root repository now
> pins the maintained Crystal fork/branch, includes Windows bootstrap/start/
> verification entry points, and tracks an immutable private developer-bundle
> manifest instead of requiring an undocumented local asset tree. The full
> bundle contains the exact verified closure of one index, 1,440 library
> shards, and 4,446 unique PNG pages; its content hash is
> `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`.
> Packaging is deterministic USTAR, installation rejects unsafe entries and
> performs a transactional swap, and remote release generation retains a
> source path plus SHA-256 for all 45,398 objects. Local release-doctor and R2
> upload-plan checks pass with exactly 5,887 full-pack objects. This closes the
> undocumented-code/assets handoff gap; it does not close the remaining human
> Crystal visual/feel acceptance or the not-yet-published full R2 endpoint.

> 2026-07-18 original q1-q9 frontend contract closure: quest dialogs now keep
> fixed rewards separate from mandatory q3/q6 selectable rewards and preserve
> item icon, template index, and selection index through Gateway JSON. The Web
> packet adapter, quest window, overlays, and objective tracker expose the
> original task progress and selected reward path instead of treating every
> reward as an automatic grant. Extended-packet 28/28, tutorial-flow 14/14,
> onboarding-guidance 17/17, stage5-adapter 68/68, TypeScript, and the Next
> production build pass against the completed simulation q1-q9 route. Final
> human dialog layout and route-feel acceptance remains open. Automated visual
> recapture was not claimed in this round because the in-app Browser rejected
> navigation from its post-restart error page when the existing tab URL
> contained a nested `ws://` query; the rebuilt local Web and Gateway health
> endpoints are green and the existing play tab can be refreshed manually.

> 2026-07-18 safe-zone TrapHexagon depth closure: persistent `ObjectSpell`
> effects were rendered at layer offset 72, above the entity offset 64, while
> Crystal sorts world spells before actors inside each map cell. Ground and
> world-spell bodies now use offset 48 and optional masks use 49; transient
> combat spells remain at 90. The live Bichon `0 @ 287,618` WebGPU scene kept
> all 52 visible TrapHexagon nodes on the exact Magic `1390-1399` loop with
> `plus-lighter` blending while restoring actor/NPC occlusion. The persistent
> effect path now decodes its deduplicated body/mask frame set before first
> display without adding per-frame DOM nodes; transient combat spells remain
> ungated. A cache-cleared first-entry MutationObserver measured all 52 beams
> with zero undecoded images at insertion, the next paint, and 100ms later.
> Unobstructed native/Web beam pixels were already matched, so opacity and
> source frames were intentionally left unchanged. `test:scene-effect-runtime`
> passes 10/10, `tsc --noEmit` passes, and the in-app 960x720 game capture has
> no missing beams or HUD blend leakage.

> 2026-07-18 Bevy run-camera transaction closure: the visible full-scene
> tremor was not a server rollback. Map/entity snapshots could commit a new
> center one Bevy frame before the local-command camera compensation, producing
> a zero-offset whole-cell flash. Consecutive commands could also rebase from a
> fractional prior pose while the TypeScript camera window used a neighboring
> phase, causing per-frame ownership to switch between `localCommand` and
> `selfWindow`. The runtime now reconciles camera/entity offsets in the same
> committed-center frame, preserves fractional motion-window coordinates across
> the JS/WASM boundary, and latches local presentation ownership across connected
> commands until a correction clears the segment. A live WebGPU run sampled 462
> display frames and five center changes with zero uncompensated center frames,
> zero active `selfWindow`/`static` samples, zero active source switches, and
> 5/5 matching command plus ACK diagnostics with no rollback. Evidence:
> `docs/generated/player-qa/movement-jitter/bevy-run-camera-transaction-20260718.json`
> and `.png`; the optimized `bevy-97470d40cbe1b310` smoke independently sampled
> 551 frames and six center changes with the same zero-failure result. Rust
> 107/107 plus the presentation-pose, local-command latency, scene-motion, and
> movement-controller web suites passed. Crystal's intentional
> six-phase 100ms stepped cadence remains; this closes the extra one-frame
> shake, not that original cadence.

> 2026-07-18 fresh-native TrapHexagon/Belt closure: live r05 captured the
> post-Rust-fix Crystal client with ordinary NPC primary names Lime and
> secondary underscore lines White at `0 @ 332,275`, Day setting 2. It exposed
> two deterministic Web regressions: `viewport-sprite-overlay` received
> `translate: 0px 0px`, creating an auto-level stacking context below the GPU
> entity canvas, and CSS rendered the nearly opaque Belt overlay at opacity
> 0.5. Correctly positioned Magic/1397 TrapHexagon nodes were therefore hidden,
> while six transparent Belt slots were darkened. Effects now live under an
> untransformed pass-through parent with per-node camera translation, and the
> non-equivalent Belt overlay is transparent. Final live r16 improves from the
> r05 15.0%/14.8% full/world changed ratios to 7.1%/6.0%, with world similarity
> 91.4%, world MAE 4.499, HUD UI 88.4%, Belt similarity 89.7% / MAE 10.765,
> chat 82.1%, and MiniMap 87.2%. The capture now fails if locked effects do not
> alter pixels: r16 sees 28 visible nodes and 57,282 changed pixels; forced
> WebGL2 r09 sees 55,462, both with 0 critical errors and 0 404s. Long native
> raw paths are read as Buffers, proven by r15 at 271 characters. The native
> top-left is still contaminated by the Codex Computer Use status bubble, so
> final clean pixel/human acceptance remains open.

> 2026-07-18 deterministic r03/r04 closure: the Web-only same-scene harness now
> completes Edge 150 CDP `Runtime.enable` through Next's compiled `ws`. r04 at
> Bichon `0.map @ 332,275`, paired Day setting 2, has 0 critical console errors,
> 0 404s, 100% runtime/layout/entity/pixel automated gates, a 100% weighted
> Candidate trend score, and a 93-100% estimated human band. This is not final
> acceptance: thresholded pixels still differ by 10% full-window and 9% world;
> chat is 82%, HUD UI 87%, and MiniMap 87%. The only r03 P0 was an Edge
> extension message-port closure, now narrowly ignored without masking real
> errors. Nameplate diagnostics confirm ordinary NPC primary lines Lime and
> secondary underscore lines White. Because the fixed r01 native frame was
> captured before the Rust duplicate White ObjectNpc path was removed, the next
> valid color comparison requires a fresh native/Web pair.

> 2026-07-14 full-pack/low-tier closure: all 1,440 Crystal libraries now have a
> deterministic, resumable, hash-verified offline conversion into 1,440 lazy
> manifest shards and 4,446 unique immutable PNG pages. All 2,143,132 frame
> slots are classified as 1,869,869 packed frames or 273,263 no-draw frames;
> the verified content hash is
> `f71b89aa38504c6c127b937043d4af6ecd26d9dd1a2b9ed3b91100e6a1f0052e`.
> Entity rendering prefers full-pack shards and retains the legacy path as a
> rollback. Bevy and raw WebGL2 caches now evict by entry and decoded-byte LRU,
> preserve active pages, release browser image references, and reject pages
> larger than the GPU limit. Forced-low WebGL2 Bichon QA held 13 pages and
> 1,598 rects in 58,379,430 bytes, passed 28/28 movement/render assertions, and
> used no DOM entity fallback. Low-tier prewarm fell to 403/403 successful
> requests with background warming off; cold transfer was 18,993,684 bytes and
> warm transfer 600 bytes, with 69,027,432 CacheStorage bytes. Evidence:
> `docs/generated/assets/crystal-full-pack-coverage.generated.json` and
> `docs/generated/player-qa/full-asset-pack-low-tier/`. This closes source-pack
> completeness and desktop low-tier automation, not all visual parity: maps
> remain regional, HUD/audio/effect-specific paths remain dedicated, and Brazil
> still needs CDN plus physical 2/4 GiB Android throttled-network soak.

> 2026-07-13 deterministic Crystal/Web temporal acceptance closure: one
> fail-closed scenario now records the native client and Web at Bichon
> `0.map @ 332,275`, aligns the same four left-walk actions, validates exact
> 1024x768 geometry, and emits bounded overlay/heatmap evidence. Two defects
> were coupled: the pose selector compared an ACK-advanced requested entity
> center with the still-rendered map center, and Rust pruned inactive standalone
> animation images while the Web upload set remembered them forever. The
> runtime now selects motion against the coherent applied map/entity center;
> each validated `map-render-synced` ACK also reconciles Web upload ownership
> with Rust's resident keys so a recurring frame is uploaded again. Runtime
> `bevy-90fb96239f221a47` passes the strict route on WebGPU with 4/4 pose events,
> 41ms maximum command-to-sink latency, and zero failed assertions; Bevy WebGL2
> passes the same movement gate at 4/4 and 42ms. Evidence is under
> `docs/generated/player-qa/movement-jitter/temporal-packs/bichon-332275-left4/`
> and `.../bichon-332275-left4-webgl2/`. The four native/Web pairs are aligned
> within 1-13ms, but their full-window changed-pixel ratios remain
> 75.0%-76.8%. That metric includes different population, lighting, HUD text,
> and effects, so it is not a movement failure and is not visual acceptance;
> those visible scene-composition gaps remain open.

> 2026-07-13 first-principles asset/render Candidate closure: the Web client now
> derives its runtime semantics from the complete Crystal source tree instead
> of hand-maintained frame guesses. The deterministic snapshot parses all
> 1,440 libraries (7,638,253,548 source bytes and 2,143,132 frame slots),
> including 703 non-empty v3 FrameSets and 3,643 actions with start/count/skip,
> interval, reverse, blend, and secondary-effect tracks. Player/NPC/monster
> presentation consumes those actions in production; player Spell uses the
> dedicated Crystal frame range at 296, and the packet-backed scene-effect
> queue resolves 62 spells, 11 object effects, two map effects, 35 explicit
> SpellEffect mappings, directional ranges, masks, offsets, light, and blend.
> Packed map pages now load directly in Bevy through `AssetServer` URLs, retain
> the previous complete frame until all replacement pages are ready, and use an
> exact generation/revision ACK before JS releases old image ownership. Unified
> atlas residency is ref-counted and bounded by decoded byte budgets, while the
> immutable CAS release/channel layout publishes assets before the mutable
> channel pointer. Full offline release verification passes with 38,846 assets,
> map renderability 191,938/192,391 (99.76%, with all references accounted for),
> minimaps 227/226 with none missing, SoundList 450/450 backed by 320/320 distinct
> wav files, and headline render coverage 99.88%. WebGPU and WebGL2 map smokes
> both report no failed assertions or critical console errors; evidence:
> `docs/generated/assets/crystal-source-snapshot.generated.json`,
> `docs/generated/assets/latest-asset-coverage-summary.json`,
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-asset-pipeline-final-20260713.json`,
> `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260713001211-8821c193-report.json`,
> and `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgl2-20260713001238-3c4011f6-report.json`.
> The final PNGs are fully opaque and have matching cross-screen RGBA samples;
> a viewer-only black surround was not present in the image bytes. This
> supersedes the older phase-1 note below
> that called FrameSet/runtime consumption open. Remaining work is final human
> Crystal-vs-Web lighting, density, and feel acceptance, not another asset
> architecture rewrite.

> 2026-07-13 map-render pipeline closure: the Bichon black rectangles were not
> missing map data. Raw packed atlases bypassed Crystal black-key conversion,
> including floor-sized frames stored in object libraries, while per-cell
> middle/front additive flags were discarded. Object-like frames now use the
> decoded standalone path, Mir3 `Dungeonsc` is covered, scene/cache and packaged
> starter blueprints carry explicit `normal|additive`, and the Bevy additive
> shader preserves Crystal's RGB equation without writing opaque black alpha to
> the transparent browser canvas. Floor layers now occupy a bounded band below
> objects/entities instead of using offsets that could overdraw buildings.
> At BichonProvince `0.map @ 320,43`, the same compressed screenshot crop went
> from 345 pure-black pixels before the fix to zero on both WebGPU and WebGL2;
> DOM map fallback and browser console errors were also zero. Evidence:
> `docs/generated/player-qa/map-rendering/bichon-320-43-map-pipeline-20260713.json`
> and matching final screenshots. Runtime 101/101, full frontend logic,
> TypeScript, map routing tests, and both release WASM builds pass. Remaining
> map-pipeline risks are GPU-ready ownership ACK precision, bounded additive
> material residency, and final Crystal lighting/effect visual acceptance.

> 2026-07-13 monster lock/chase acceptance: clicking a live monster now enters
> a persistent target-combat state instead of immediately sending one attack.
> The client reuses the authoritative movement intent/ACK pipeline, refreshes
> the adjacent destination when the monster moves, waits for the final accepted
> movement action to settle, then starts Crystal-local melee and sends only an
> in-range attack. The same lock continues at the local attack cadence until the
> target dies/disappears, selection changes, the map/session ends, or the player
> issues manual movement. Browser verification chased Royal Archer from player
> `310,51` to `320,43`, first observed `.attacking` at 2.3s with the target still
> selected and `Attack · 1 tiles`, then observed zero attacking samples for
> 3.5s after a manual ground click. Evidence:
> `docs/generated/player-qa/combat/web-monster-lock-chase-20260713.{json,png}`.
> `test:frontend-logic`, focused target-combat tests, and TypeScript pass. This
> closes target engagement flow only; transparent sprite hit interception and
> the separately reported map-rendering defects remain open.

> 2026-07-13 local melee fix and automated visual acceptance: pre-fix, an
> adjacent Space attack reduced Deer HP `11/25 -> 4/25` but the self
> `.attacking` class timed out at 900ms. Diagnostics proved the shared Zone echo
> used observer id `50001`, while the owner `SelfPlayer` uses personal id `1000`.
> Crystal queues local `Attack1` before sending `C.Attack` and ignores its own
> `S.ObjectAttack`; Web now mirrors that sequence for a live adjacent target.
> The rAF world flush remains packet-coalesced but is no longer a low-priority
> React transition, so 600ms combat windows cannot be starved. Post-fix the
> class appeared in 123ms, the screenshot captured a visible swing, and it
> detached normally after the action. Evidence:
> `docs/generated/player-qa/combat/web-local-melee-attack-20260713.{json,png}`.
> Full frontend logic and TypeScript pass. Stable all-direction/action atlas
> membership, Bevy cached-layout refresh, and combat-over-movement pose priority
> remain green. Separate open gap: alpha-transparent CherryTree sprite bounds
> can intercept pointer input intended for nearby entities.

Purpose: track frontend/client visual, interaction, and human-feel gaps separately from backend/server parity.

Status values:

- `[ ]` open
- `[~]` active
- `[x]` fixed and verified
- `[a]` accepted difference

## Current Automated Evidence

- 2026-07-13 compact-window movement flicker investigation: 264 pre-fix A/B
  frames across Bevy local-pose on/off and 820/1024 viewports produced zero
  scene blackouts and zero atomic-pose warnings. Entity/layer changes matched
  AOI membership rather than one-frame removals, so the defect was not a
  Gateway packet, entity-atlas failure, or DOM/Bevy ownership toggle. Crystal
  source confirms its 100ms movement phase is correct, while its integer/even
  `OffSetMove` is drawn directly into the selected backbuffer. Web instead had
  a 1024x768 canvas transformed to `820.02x615.01` at `top=102.49`, causing
  whole-scene fractional resampling. The responsive stage now derives an exact
  integer 4:3 rectangle and integer origin; the 820 regression is exactly
  `820x615` at `(0,103)` over 93 frames with zero blackout, pose, console, or
  404 warnings. Evidence: `docs/generated/player-qa/flicker-ab/current-820.json`,
  `current-1024.json`, `no-local-820.json`, and `aligned-820.json`. True pixel
  1:1 still requires a 1024x768-or-larger browser content viewport; compact
  presentation necessarily remains downsampled.

- 2026-07-12 canonical movement-presentation closure: WebGPU report
  `docs/generated/player-qa/movement-jitter/movement-mounted-scene-transaction-full-phases-webgpu-20260712-r12.json`
  passes 33/33 and WebGL2 report
  `docs/generated/player-qa/movement-jitter/movement-mounted-scene-transaction-full-phases-webgl2-20260712-r16.json`
  passes 33/33 on `bevy-bd9004a17f2873ea`. Both issue exactly one Walk and one
  mounted Run through keyboard, controller, WebSocket, shared Zone, and Bevy.
  They capture every Crystal Walk phase `0..7` at effective map offsets
  `-6,-12,-18,-24,-30,-36,-42,-48px` and Run phase `0..5` at
  `-24,-48,-72,-96,-120,-144px`, while the self sprite remains pinned.
  Map/entity centers never split, no synthetic non-endpoint logical centers
  appear, shadow command/ACK mismatches are zero, post-warmup pose provenance
  errors are zero, and phase, rollback, queue, console, and network warnings
  are zero. WebGPU ACKs are 2/6ms; WebGL2 ACKs are 7/2ms.
- The final causes were architectural rather than browser throughput: page
  logical position and Bevy both interpolated the same action; map and entity
  producers could expose half a scene transaction; one rejected pose changed
  immediately to a TypeScript clock; movement shadow hard-coded a Run target;
  and phase 0 advanced at the nearest global pulse, sometimes lasting only
  about 20ms. Bevy is now the sole local interpolation owner, scene provenance
  commits atomically, fallback uses a 250ms last-good-pose watchdog, shadow uses
  the explicit command target, and local phase cadence is anchored to
  `started + 100ms` with no catch-up. This closes the automated movement core,
  not overall visual parity: the screenshots still show lighting/effect and
  extra demo-population differences that remain open below.
- Unattended movement QA no longer risks attaching to a stale Chrome because a
  PID-derived debug port collided. Default capture lets Chrome allocate an
  ephemeral port, accepts only the `DevToolsActivePort` from that run's profile,
  and cleans up failed launches; explicit occupied ports are rejected before
  spawn. `movement-mounted-autocdp-cleanup-webgpu-20260712-r19.json` was
  produced without `--debugPort`, passes the same 33/33 movement gate, and
  leaves zero new Chrome profiles after completion.

- 2026-07-12 mounted movement acceptance candidate:
  `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`
  and matching `.png` exercise real keyboard input after granting, equipping,
  and using Crystal `RedTiger`. Exactly two commands are sent. Walk advances one
  tile in eight 100ms phases; Run advances three tiles in six phases. ACKs are
  18/22ms, final delta is `(4,0)`, local Pose coverage is 2/2 with a 26ms maximum
  sink latency, all 27 assertions pass, and pose atomicity/rollback/direction/
  queue/console/404 warnings are zero. Runtime is
  `bevy-78d40eb80133609c`; dual WebGPU/WebGL2 smoke report is
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-phasecount-pose-final-20260712.json`.
- The mounted eighth-phase blackout was not GPU throughput. Rust emitted frame
  indexes 6 and 7 correctly, but the TypeScript Pose parser still rejected any
  `frameIndex > 5`. Pose motion now carries `phaseCount`, defaults legacy frames
  to 6, accepts Crystal's maximum 8, and rejects indexes outside that contract.
- 2026-07-12 final normal-port movement presentation pass: strict Release
  keyboard capture
  `docs/generated/player-qa/movement-jitter/movement-zone-owned-cadence-final-release-keyboard-20260712.json`
  exercises the actual input/controller/Bevy pose path rather than injecting
  raw packets. Walk and Run ACK in 23/6ms, both commands reach the pose sink in
  at most 12ms, final movement is exactly `(3,0)`, and logical rollback,
  direction lag, stale prediction, queue latency, camera stair-step, pose-frame
  atomicity, console, and 404 warnings are all zero. Its matching `.png` is a
  complete scene-ready WebGPU frame. Raw `packetSequence` captures remain
  protocol-only evidence and must not be interpreted as local-pose coverage.
- 2026-07-12 Zone-owned cadence/live-observer pass: realtime owner and AOI
  `UserLocation`/player appearance/removal/Turn/Walk/Run packets now reach a
  bounded token-fenced socket channel without waiting for React activity or an
  observer's private Session Tick. The Zone owner also advances one global
  300ms cadence, while personal ticks no longer multiply shared-world time.
  Strict Release evidence
  `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`
  holds personal Tick at 5000ms and sends no observer pulse: movement arrives
  in 12ms, both clients retain 16 entities, Bevy observes the remote packet and
  drives 29 packed offsets, with zero decode errors, queue drops, console
  errors, or 404s. The QA bridge now exposes live `worldRef` map/entities/tick
  getters so background-page rAF throttling cannot create stale automation
  state; StartGame/transfer snapshot timestamps and foreground screenshot
  readiness are also gated. Scene-ready screenshots are the matching `-a.png`
  and `-b.png` files beside the report.
- 2026-07-12 bounded Zone-ingress pass: normal Web movement no longer waits for
  a blocked private Session tick. The capacity-256 reader sends authenticated
  Walk/Run/Turn through a capacity-64 per-Zone actor while preserving serial
  action order, owner fencing, event publication, and save-transform sync.
  Release expired-run evidence
  `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-zone-ingress-release-keepalive-snapshot-ready-20260712.json`
  records ACKs at 15/14ms, one degradation, zero corrections, and `(2,0)`.
  Strict keyboard evidence
  `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-zone-ingress-release-keepalive-snapshot-ready-rerun-20260712.json`
  records ACKs at 17/21ms, pose/sink maxima at 23/24ms, zero failed assertions,
  and `(3,0)`. Event-observed evidence records 11/2ms and exactly one Walk plus
  one Run event. The harness waits for a new post-transfer world snapshot and
  fails a stuck CDP command after 15s instead of hanging indefinitely.
  Remote-observer push independence and one global Zone cadence are closed by
  the evidence above. Remaining feel gaps are mounted eight-frame motion, true
  three-cell sprint, lighting/effects, and final human side-by-side acceptance.
- 2026-07-12 movement degradation pass: the early page ACK path and controller
  reconciliation now use one `classifyMovementAckOutcome` decision. A requested
  Run acknowledged at its first cell is a confirmed Crystal-style degradation,
  not a correction that clears animation or arms the 400ms correction lock.
  Release raw protocol evidence
  `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
  records ACKs at 16/99ms, `degradedRunCount=1`, `correctionCount=0`, and final
  delta `(2,0)`. Normal UI Walk -> Run evidence
  `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`
  records ACKs at 22/28ms, command-to-pose latency 17/1ms, zero degradation or
  correction, and final delta `(3,0)`. `npm.cmd run test:frontend-logic` is
  green. Bevy intentionally retains the TypeScript fallback for a degraded path
  until phase-preserving retargeting exists; taking over the wrong two-cell
  segment would be less faithful than that bounded fallback.
- 2026-07-12 default shared-clock and additive-world pass: the normal URL now
  enables guarded Bevy local self/camera ownership plus synchronous pose commit;
  the tested rollback is `?bevyLocalMotion=0&bevyPoseCommit=0`. A single
  Crystal-compatible 100ms pulse advances all six movement phases and does not
  freeze on delayed ACKs. Default continuous evidence
  `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-continuous-202607120610.json`
  sent one click as three walks at 601/601ms and kept command-to-pose latency at
  10ms maximum. Committed keyboard evidence
  `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-keyboard-committed-ref-202607120617.json`
  matched 4/4 commands, returned to `328,275`, and stayed within 15ms with zero
  long tasks, interaction pollution, warnings, errors, or 404s. Native/Web
  action-aligned evidence
  `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-default-shared-clock-horizontal-20260712-001.md`
  measured the same 2701ms four-action span and 24 active Web frame pairs,
  matching four commands times six movement phases. Its full-window pixel
  ratio remains confounded by different world objects, ambient effects, HUD,
  browser chrome, and capture geometry; it is not an actor-isolated movement
  score. Explicit rollback evidence
  `docs/generated/player-qa/movement-jitter/movement-explicit-legacy-rollback-202607120623.json`
  proves both ownership flags inactive, 2/2 command and ACK matches, and exact
  coordinate return. The final 25 additive world sprites now render through a
  Bevy `SrcAlpha + One` material; WebGPU report
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260711213830-dee09cfc-report.json`
  and the matching WebGL2 smoke both report zero DOM world sprites and zero
  image/network failures. Runtime `bevy-630a77b3535f95bd` passes 94/94 Rust
  tests plus dual-backend report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-default-shared-clock-202607120620.json`.
  Remaining frontend gates are real correction/degraded-run capture, mounted
  eight-frame and sprint cases, scene population/ambient/light parity, and
  combat-effect polish.
- 2026-07-10 release early-pose and incremental-map pass: a clean local command
  can now own the unified Bevy pose immediately, without waiting for React to
  publish a delayed TypeScript motion window. Map and entity producers share one
  exact render center, viewport entities are rebased atomically, and correction,
  degraded-run, target-mismatch, and path-mismatch cases still fall back to the
  TypeScript path. Pose commit remains default-off behind `?bevyPoseCommit=1` /
  `mir2-bevy-pose-commit`, alongside the existing default-off local-motion flag.
  The normal `npm run dev` path now builds release WASM; the explicit diagnostic
  alternative is `npm run dev:debug-runtime`. Producer semantic deduplication and
  retained Rust map entities reduced the four-step route from revision `687 ->
  999` (about 70 revisions/second, 53 sampled states) to `13 -> 21` (five sampled
  states: initial plus four real centers). Existing tiles now update only their
  transform, image bindings change only on image revision, and the runtime no
  longer clones the roughly 202 KB draw list after every apply. Strict WebGPU
  evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710220403-44ba1f45-report.json`
  is fully green at exact final tile `328,275`: all 4/4 commands reached an
  accepted `localCommand` sink in `14/18/32/16ms` (32ms maximum under the 75ms
  gate), with zero drops, provenance failures, visual jumps, console errors, or
  non-favicon 404s. Default-off compatibility evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710221024-ce1066ce-report.json`
  is also green. Runtime `bevy-9ce93936c0841d7e` passes 86/86 Rust tests,
  TypeScript, scene/controller/pose/latency tests, and dual-backend report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710221430.json`.
  Remaining acceptance gaps are an exact native correction/degraded-run temporal
  comparison and the longer-term world-space/chunk map model; the rollback flags
  must stay available until those gates pass.
- 2026-07-10 guarded Bevy local-motion presentation pass: copies of normalized
  self movement commands and authoritative ACKs now feed a bounded Rust
  `PreUpdate` resource. It can own the packed self sprite, Bevy camera, and DOM
  overlays through the existing unified pose buffer, but remains
  presentation-only: shared Zone still owns acceptance, correction, collision,
  occupancy, cooldown, AOI, and persisted transforms. Takeover is default-off
  behind `?bevyLocalMotion=1` / `mir2-bevy-local-motion`; disabling it preserves
  the previous TypeScript path. An object + target + from/to path handshake
  prevents a degraded run or visually rebased TS window from attaching the
  wrong Bevy segment. Corrections clear the segment, path/target mismatch falls
  back to TS, and a completed matched segment settles both camera and self at
  exact zero without reconnecting a delayed window. Runtime
  `bevy-e50cfdd1e6c8d229` passes Rust 83/83, pose parser 6/6, movement bridge
  9/9, TypeScript, release build, and validated WebGPU/WebGL2 packages. Final
  backend probe
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710173210.json`
  is fully green and explicitly proves mismatched-path `selfWindow` fallback,
  matched-path `localCommand` ownership, disable isolation, package fetches,
  raw WebGL2 rendering, and zero critical console errors. Real WebGPU A/B routes
  are both `ok=true`: default-off report
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173245-17db8e6b-report.json`
  records 76/76 exact local geometry samples, while forced-on report
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710173356-7b3abddd-report.json`
  also records 76/76, final self/camera source `localCommand`, 4/4 command matches,
  4/4 ACK matches, 0 visual jumps, 0 queue/decode drops, 0 critical errors, and
  0 non-favicon 404s. The final map regression
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710173500-ca321fe7-report.json`
  also remains green with 109 standalone draws and 108/108 decodes. Remaining
  acceptance gap: command-timestamp presentation differs from the delayed TS
  window by up to 32px / 326ms in this route. Keep takeover default-off until an
  exact native Crystal vs Web A/B frame sequence proves that earlier phase is
  closer to native and correction/degraded-run routes remain visually clean.
- 2026-07-10 unified Bevy presentation-pose pass: packed sprite transforms,
  self-camera translation, and residual DOM nameplates/HP/chat now consume the
  same per-frame Rust pose buffer. The packed wire marks `isSelf`; Bevy computes
  one camera screen pose at frame start, derives the self sprite as its exact
  inverse, records the actual selected remote/fallback offsets, and publishes a
  versioned 256-entry bounded snapshot. DOM reads it at rAF frequency and falls
  back to the previous TypeScript curve on missing, malformed, stale (>250ms),
  disabled, or unsupported runtime data. `?bevyPresentationPose=0` disables only
  the DOM bridge and cannot change Bevy rendering. The first real route exposed
  a genuine dual-window race as two 20/22px self-label jumps; centralizing the
  self pose removed both rather than weakening the test. Runtime
  `bevy-8a40d0bdcf0dc14a` passes Rust 72/72, pose-parser 5/5, movement-bridge
  9/9, TypeScript, and dual-backend release/self-check gates. Chrome/WASM report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
  is fully green in default/forced WebGPU and forced WebGL2, including remote
  packet `-24px`, camera `+24px`, source tags, bridge disable isolation, package
  fetches, and console gates. Real keyboard-route evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710163125-1a4aff1b-report.json`
  is `ok=true`: 0 visual jumps, 4/4 command matches, 4/4 ACK matches, 1219 Bevy
  pose samples vs 4 startup fallbacks, 38908 entity-pose hits, 0 pose overflows,
  0 critical errors, and 0 non-favicon 404s. Final map regression
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
  remains green with 109 standalone draws, 108/108 decoded images, and only 25
  DOM sprites, all additive. Next gap: migrate local self prediction and ACK
  reconciliation into a guarded Bevy presentation source while shared Zone
  remains authoritative for acceptance, correction, collision, cooldown, AOI,
  and persisted transforms.
- 2026-07-10 Bevy packet-driven remote-motion presentation pass: normalized
  `ObjectWalk` / `ObjectRun` / `ObjectTurn` / remove events now feed a bounded,
  presentation-only Bevy resource during `PreUpdate`, without changing input,
  collision, cooldown, AOI, reconciliation, persistence, or shared-Zone
  authority. Walk/run segments use Crystal's 600ms stepped cadence, connected
  segments continue from the currently displayed fractional pose, stale events
  are ignored, large discontinuities snap, and remove/disable clears state.
  Packed sprites consume the Rust offset only when the packet target matches
  the latest packed entity grid target; otherwise the existing TypeScript
  motion window remains the safe fallback. The path is default-on when packed
  Bevy entities are active and can be disabled with `?bevyRemoteMotion=0`.
  Runtime `bevy-63449641a633efc2` passes all 67 Rust runtime tests, including 13
  focused remote-presentation tests, and the TypeScript bridge passes 9/9.
  Real Chrome + WASM evidence
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-remote-motion-probe-20260710.json`
  is `ok=true`: default WebGPU, forced WebGPU, and forced WebGL2 each proved the
  target-mismatch fallback, then matched-target Bevy offset takeover, then
  disable-and-clear, with zero decode/event drops and no critical console
  errors. Current map and movement regressions remain green at
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
  and
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3-report.json`.
  This proves renderer ownership with synthetic packet injection, not real
  shared-Zone transport: repeated two-client runs exposed a native Gateway
  multi-session/reconnect crash (`0xc0000005` / `0xc0000374`), recorded at
  `docs/generated/player-qa/two-client-zone/two-client-zone-native-crash-20260710.json`.
  The unified sprite/camera/DOM pose work named here is complete in the pass
  above; local self prediction/reconciliation is the next guarded migration.
- 2026-07-10 Bevy movement shadow-ECS pass: the production input, WebSocket,
  and authoritative shared-Zone paths are unchanged, but every accepted local
  walk/run/turn decision and its `UserLocation` ACK now mirrors into an
  observation-only Bevy resource on a 100ms `FixedUpdate`. Rust independently
  derives the destination from source/direction/mode, correlates ACKs through a
  bounded FIFO, treats one-tile run landings as explicit degradation, requires
  direction parity for turns, and records bounded remote motion segments.
  The bridge cannot throw into production movement; pending event JSON is capped
  at 256, pending commands at 16, remote segments at 256, and ObjectRemove/Hide
  evicts remote state. Focused tests pass Rust 15/15 and TypeScript 9/9.
  Isolated WebGPU evidence
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3-report.json`
  is `ok=true`: a four-step Right/Right/Left/Left route produced 4/4 command
  matches and 4/4 ACK matches, 0 command/ACK mismatches, 0 queue or command
  drops, 0 pending commands, 0 bridge errors, 0 decode errors, 0 critical
  console errors, and 0 non-favicon 404s; random credentials, Gateway, Chrome,
  and temporary files were cleaned automatically. Screenshot:
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710154640-c847d5b3.png`.
  Runtime `bevy-63449641a633efc2` passes
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-remote-motion-probe-20260710.json`
  with the movement-shadow API present in default/forced WebGPU and forced
  WebGL2 packages. This is a shadow diagnostic milestone, not production motion
  ownership. The remote presentation step named here is complete in the pass
  above; self/camera and the DOM overlay pose bridge remain open.
- 2026-07-10 Bevy map standalone-texture and ownership-handoff pass: packed
  atlas misses with normal alpha blending now decode through the bounded
  standalone-tile cache and upload as Bevy `Image` assets; additive Crystal
  glows deliberately remain in the DOM until the Bevy material path supports
  the same blend equation. Runtime readiness is now independent from status
  telemetry such as `scene-ready` / `map-render-synced`, so those events no
  longer stop the 33ms world-snapshot emitter or disable the map renderer.
  DOM/WebGL2 ownership stays live until Rust publishes a complete
  `map-render-synced` acknowledgement for every required atlas image, and
  standalone sprites hand off only after the same acknowledgement. Failed
  atlas decodes are removed from the promise cache so transient failures can
  retry. Isolated WebGPU evidence
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e-report.json`
  is `ok=true`: map `0 @ 324,41`, 421 atlas tiles, 109 standalone draws / 108
  decoded standalone images, 7 atlas pages / 115 total images, 0 standalone
  failures, 0 map 404s, 0 critical console errors, and exactly 25 remaining
  DOM sprites, all 25 additive. Screenshot:
  `docs/generated/player-qa/bevy-map-standalone/bevy-map-standalone-webgpu-20260710162936-ca18422e.png`.
  Backend package evidence
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-unified-pose-20260710.json`
  passes default/forced WebGPU, forced WebGL2, package-fetch, raw WebGL2 probe,
  and console gates; the current runtime is `bevy-8a40d0bdcf0dc14a`. Remaining map
  renderer gap: implement Crystal-compatible additive materials in Bevy, then
  remove the final 25-sprite DOM world-render fallback.
- 2026-07-10 native/Web movement temporal rerun: after the Crystal stepped
  motion pass, the automated comparison now has valid same-cadence window-frame
  evidence instead of the earlier black native capture. Native Crystal was
  relaunched from `E:\mir2\Crystal\Build\Client\Debug\Client.exe`, logged in as
  `cdx0708235326`, and captured
  `docs/generated/player-qa/movement-jitter/original-crystal-valid-step-route-20260710.json`
  with 90 JPEG frames, four real Computer Use clicks, average sample delta
  `50.12ms`, and no black-screen/device-lost frame set. Web was captured
  against the live `7111` Gateway with a fresh QA account and window-level
  frame capture:
  `docs/generated/player-qa/movement-jitter/web-crystal-window-fresh-step-route-20260710.json`
  (`ok=true`, 86 JPEG frames, average sample delta `50.11ms`, 3/3 walk ACKs,
  avg ACK `233ms`, max ACK `457ms`, 0 failed assertions, 0 entity-hit
  pollution, 0 critical console errors, 0 non-favicon 404s). The report
  `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-window-20260710.md`
  records aggregate visual delta/sec Crystal `68.0367` vs Web `37.9166`
  (Web ratio `0.5573`). Interpretation: the Web movement pipeline is
  protocol-clean and sampled at native cadence, but the current moving scene
  still has only about 56% of native's per-second visual motion energy in this
  capture. Next frontend gap: use this evidence to tune residual camera/object
  draw/layer motion and then rerun an exact-route pack; do not judge the
  remaining feel gap from static screenshots alone.
- 2026-07-09 Crystal movement/render cadence pass: source audit confirmed the
  Web map/entity viewport origins already match Crystal's split anchors
  (`DrawFloor/DrawObjects` use tile-left origin `470`, while entity
  `DrawLocation` uses `480` at 1024x768), so this round deliberately did not
  "fix" the 10px floor/entity difference. The actual hand-feel mismatch was
  the movement offset curve: Web/Bevy used a free-running linear lerp while
  Crystal advances walking/running through 6 movement frames on the
  `GameScene.CanMove` 100ms cadence and truncates movement offsets to even
  pixels. Web `original-client-scene-motion.ts` and Bevy runtime
  `motion.rs` now share that stepped cadence for entity offsets, camera
  offsets, and fractional chained movement. Runtime packages were rebuilt as
  `bevy-e48cd43dadfddb17`. Verification passed focused Web
  `node scripts/test-scene-motion.mjs`, Rust
  `cargo fmt --check; cargo test --lib motion -- --nocapture` (24/24), and
  Bevy backend smoke
  `docs/generated/player-qa/bevy-runtime-backends/crystal-step-motion-runtime-20260709.json`
  with `ok=true`, package fetches healthy, default WebGPU selected, forced
  WebGL2 rendered, and 0 critical console errors. Remaining frontend gap:
  rerun same-route native/Web movement video capture to score temporal parity
  after this cadence change, then continue map-cell/object light tuning.
- 2026-07-09 Crystal/Web main-scene light render pass: Web now renders a
  Crystal-style scene light overlay for non-Day `lightSetting` values. Day and
  Normal keep the previous no-overlay path, while Dawn/Evening/Night mount
  `.viewport-crystal-light-overlay` between sprite rendering and nameplates so
  the world/actors darken but HUD, MiniMap, chat, and labels stay readable like
  Crystal's `DrawLights()` order. Evidence
  `docs/generated/player-qa/visual-parity/scene-light-render-20260709/`
  uses a temporary updated Gateway on `7311`, enters `demo` / `demo` as
  `Scout`, and records the clean screenshot
  `scene-light-render-clean-20260709.png` plus DOM state
  `overlayClass=viewport-crystal-light-overlay night`,
  `overlayLight=4`, `z-index=6`, `pointer-events=none`, `tutorialOpen=false`,
  and browser console errors `0`. The same pass now exports
  `OriginalMapCell.light` and renders viewport map-cell light nodes inside the
  overlay; API probe `map-light-export-probe-20260709.json` confirms map `0`
  samples with 127 / 127 / 25 / 26 light cells. A fresh map-light DOM screenshot
  was not captured because the real Crystal UTC light window rotated back to
  Day, correctly suppressing non-Day overlay rendering. Remaining frontend gap:
  recapture Night/Evening/Dawn map lights, tune intensity against native
  screenshots, and add object/equipment/effect light sources.
- 2026-07-09 Crystal/Web dynamic TimeOfDay/lightSetting pass: Web now receives
  the same dynamic light state that Crystal's server sends. Crystal source
  seeds `Envir.Now` from `DateTime.UtcNow` and maps `Now.Hour * 2 % 24` to
  Dawn/Day/Evening/Night; Simulation StartGame and `WorldSnapshot.lightSetting`
  use the same formula, and the browser applies `snapshot.lightSetting` plus
  exposes it through `window.__mir2Stage5.state.lightSetting`. Evidence
  `docs/generated/player-qa/visual-parity/light-setting-snapshot-20260709/`
  records direct WS `TimeOfDay.lights=4`, `worldSnapshot.lightSetting=4`, and
  browser state `lightSetting=4` with 0 critical console errors and 0
  non-favicon 404s. This closes light-state propagation only; the active
  frontend gap is still the main-scene Crystal ambience render for Night,
  Evening, and Dawn.
- 2026-07-09 Crystal/Web 335,266 evidence ladder: pack
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0060-minimap-source-panel-viewrect-native335266-clean/`
  is the latest clean rebuilt-gateway same-coordinate proof for account
  `cdx0708235326`: runtime/layout/entities `100%`, 0 network 404s, 0 critical
  console errors, MiniMap `86%`, HUD UI `86%`, and Web player `0 @ 335,266`
  with native-synced vitals/items/gold/belt. Follow-up packs
  `0061-chat-override-replace-native335266` and
  `0062-current-chat-colors-native335266` fixed capture-only chat behavior
  (`crystalVisibleChatLines` replaces startup logs; `[Mode]`, `[Pet]`, and
  `Now in Net` infer Crystal green/blue channels), but also confirmed native
  `LineMessage.txt` rotation/history makes chat pixels unstable unless the
  current native visible slots are controlled. Packs
  `0063-belt-quantity-ones-native335266` through
  `0065-belt-label-colors-native335266` fixed Web Belt quantity `1` visibility,
  black shortcut labels, and yellow belt counts; 4x crops verify the visual
  correction, while the remaining `hud-belt=78%` is mostly transparent-slot
  exposure of world/camera/light mismatch rather than missing Belt data.
  Validation: web `npm.cmd exec tsc -- --noEmit` passed. Next frontend gap:
  camera/viewport parity plus world light render/AOI/object-set alignment before
  using HUD-belt/chat pixels as acceptance gates.
- 2026-07-09 Crystal/Web MiniMap light-icon bootstrap pass (historical,
  superseded by dynamic light state above): this Web same-scene lane stopped
  bootstrapping the browser into Night for the current Crystal Day/Normal
  capture. Crystal maps `LightSetting.Day` and `LightSetting.Normal` to
  `Prguse/2093`, while Night maps to `2092`; Web was seeing the old fixed
  `TimeOfDay { lights: 4 }` path and rendered `2092`. Simulation StartGame
  bootstrap emitted `lights=2` for this proof, and evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0057-minimap-light-day-bootstrap/`
  records Web `miniMapLight.originalSrc=/original-ui/Prguse/2093.png`, player
  `334,263`, 0 network 404s, 0 critical console errors, runtime/layout/entities
  `100%`, overall `98.4%`, pixel trend `95.7%`, and MiniMap moving from the
  0056 fair-coordinate proof `0.784` / meanAbsDelta `32.788` to `0.786` /
  `32.545`. Remaining MiniMap work is true raster/color/marker parity, not the
  time-of-day icon.
- 2026-07-09 Crystal/Web fair-coordinate evidence gate: the same-scene pack
  exposed that `qa.applyNativeState` could update vitals/items but leave the
  shared Zone authoritative transform at the previous coordinate, causing
  native/Web world and MiniMap comparisons to be one tile apart. The capture
  harness now verifies `mapFileName` plus `position.x/y` during
  `qa.applyNativeState`, and the Gateway shared-Zone path now syncs native-state
  transforms into Zone presence. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0056-main-hud-fair-visible-coord/`
  records both Web `player` and `authoritativePlayer` at `334,263`, transfer
  mode `alreadyAtTarget`, 0 network 404s, 0 critical console errors,
  runtime/layout/entities `100%`, overall `99.5%`, pixel trend `98.6%`, world
  `85.8%`, HUD UI `86.8%`, chat `85.8%`, and MiniMap `78.4%`. Use 0056 as the
  current fair coordinate-lock proof before judging MiniMap/world deltas.
- 2026-07-09 Crystal/Web main-HUD content-y pass: Web keeps the
  `.main-hud-shell` anchored at `0,616` for layout parity, but shifts the
  inner `.main-hud` content down by `2px`. Pixel analysis of the 0050 and 0054
  crop pairs showed the main-HUD-only subregions (`hud-left`,
  `hud-right-controls`, `hud-right-status`, and `hud-bottom-center`) all had
  their best alignment with Web shifted down 2px, while independent Belt and
  Chat crops did not. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0055-main-hud-content-y-offset/`
  records 0 network 404s, 0 critical console errors, runtime/layout/entities
  `100%`, and the HUD improvements from 0054: `hudRightControls` similarity
  `0.720` / meanAbsDelta `49.436` to `0.986` / `0.303`;
  `hudRightStatus` `0.734` / `42.642` to `0.824` / `14.189`; `hudUi`
  `0.782` / `34.113` to `0.856` / `15.453`; and `hudBottomCenter` `0.800` to
  `0.886`. Treat 0055 as a HUD proof, not a new fair overall baseline, because
  world/minimap/chat differed dynamically in this run (`overall=95.9%`,
  `chat=70.7%`, `world=77.3%`).
- 2026-07-09 Crystal/Web Belt overlay draw-order pass: Web now mirrors Crystal
  `InventoryDialog.BeltDialog` rendering order. Crystal hooks
  `BeltPanel_BeforeDraw`, and `MirControl.Draw()` calls `BeforeDrawControl()`
  before `DrawControl()`, so the `Index + 1` Belt overlay (`1933` horizontal,
  `1945` vertical) is drawn at `0.5F` opacity behind the main Belt frame
  (`1932` / `1944`). Web previously rendered the overlay after the base image,
  which darkened the Belt panel and item slots. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0054-belt-overlay-draw-order/`
  uses the new auto-generated crop pairs and records `hudBelt` improving from
  the 0050 baseline similarity `0.765` / meanAbsDelta `48.963` to `0.791` /
  `38.920`; `hudUi` also moves from `0.778` / `35.215` to `0.782` / `34.113`.
  Treat 0054 as the Belt proof, not a new overall baseline, because native chat
  rotated during capture (`chat=75.9%`, overall `97.9%`).
- 2026-07-09 Crystal/Web same-scene crop automation: `capture-crystal-web-pack.mjs`
  now writes native/Web crop pairs for the same regions used by
  `report-crystal-visual-parity.mjs`: `world`, `hud-full`, `hud-left`,
  `hud-belt`, `hud-right-controls`, `hud-right-status`,
  `hud-bottom-center`, `minimap`, and `chat`. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0053-auto-region-crops/`
  confirms 9 crop pairs are generated and recorded in the pack summary. Treat
  0053 as an evidence-tooling validation, not a new visual baseline: the native
  chat line rotated again, dropping chat to `67%` and overall to `96.9%`, while
  the right-status HUD metric returned to the 0050 baseline
  (`hudRightStatus=0.734`, meanAbsDelta `42.642`). The attempted 0051/0052
  GDI-outline HUD text experiments were diagnostic only and were not retained,
  because they did not improve the right-status similarity over 0050.
- 2026-07-09 Crystal/Web clean chat-slot baseline: Web capture now supports a
  `crystalVisibleChatLines` JSON override for same-scene evidence, allowing the
  harness to reproduce the native client's current four visible ChatDialog
  slots instead of comparing against a randomly rotated or scrolled
  `LineMessage.txt` state. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0050-chat-visible-slots-current/`
  records Web visible chat lines
  `Online Players: 1 / Welcome to Crystal Mir 2 released by Suprcode. / Online Players: 1 / Online Players: 1`,
  0 network 404s, 0 critical console errors, runtime/layout/entities `100%`,
  overall `98.5%`, pixel trend `96%`, chat `83%`, HUD full `78%`, HUD UI
  `78%`, world `83%`, and MiniMap `80%`. The run also preserves the 0046
  weight-bar fix (`weightRatio=0.2258`, `fillWidth=16`,
  `originalSrc=/original-ui/Prguse/76.png`) and keeps `hudRightStatus=0.734`.
  Use 0050 as the latest fair automated visual baseline; 0047-0049 are retained
  as diagnostics for LineMessage rotation and line-slot control.
- 2026-07-09 Crystal/Web chat LineMessage capture-control diagnostic:
  `capture-crystal-web-pack.mjs` now accepts `--gatewayWs` and
  `--crystalLineMessage`, appending those query parameters to the Web base URL
  before the browser capture. This removes the brittle hand-built URL problem
  seen during 0046 retries and proves the Web startup chat can be seeded with
  the currently visible Crystal `LineMessage.txt` entry. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0047-chat-line-message-sync/`
  records visible Web chat lines
  `Online Players: 1 / Make sure to follow JevLomcn on github for the latest Database releases. / Online Players: 1 / Online Players: 1`,
  keeps the 0046 weight-bar diagnostics (`weightRatio=0.2258`, `fillWidth=16`),
  and has 0 network 404s / 0 critical console errors. The chat crop still
  scores only `65%` because native Crystal leaves an empty/filtered line slot
  before the LineMessage while Web renders the seeded lines contiguously. Treat
  0047 as a chat `History` / `StartIndex` diagnostic, not a visual-score
  improvement; 0046 remains the current weight-bar proof and 0042 remains the
  cleaner fair overall score.
- 2026-07-09 Crystal/Web HUD weight-bar source-fill pass: Web now mirrors
  Crystal `MainDialogs.cs` for the main-HUD weight bar. Crystal sets
  `WeightBar.DrawImage = false` and draws only
  `(WeightBar.Size.Width - 2) * CurrentBagWeight / Stats[BagWeight]` pixels in
  `WeightBar_BeforeDraw`, choosing `Prguse/76` at <=50%, `UI_32bit/473` at
  <=75%, and `UI_32bit/472` above 75%. Web previously rendered `Prguse/76.png`
  as a full 76px bar for every weight state. Web now clips the source sprite
  to the Crystal fill width, records the DOM fill diagnostics, and exports the
  missing `UI_32bit/472.png` and `UI_32bit/473.png` resources. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0046-weightbar-source-fill/`
  records `currentWeight=14`, `maxWeight=62`, `weightRatio=0.2258`,
  `fillWidth=16`, `originalSrc=/original-ui/Prguse/76.png`, 0 network 404s,
  0 critical console errors, runtime/layout/entities `100%`, and a measured
  right-status improvement from 0045's similarity `0.727` / meanAbsDelta
  `45.137` to `0.734` / `42.642`. Overall is `97%` because the chat crop is
  still dynamically mismatched (`71%`); use this pack for the weight-bar proof
  and keep 0042 as the cleaner fair overall score.
- 2026-07-09 Crystal/Web HUD right-button coordinate pass: Web now aligns the
  right-side main-HUD button coordinates with Crystal `MainDialogs.cs` for the
  1024px HUD. Crystal positions the buttons at `Size.Width - 105/55/119/96/73/50/27`
  (`919`, `969`, `905`, `928`, `951`, `974`, `997`), while Web had each button
  one pixel further left. The CSS button anchors now use the Crystal source
  coordinates. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0045-hud-right-button-source-coords/`
  records 0 network 404s, 0 critical console errors, runtime/layout/entities
  `100%`, and a small right-controls improvement: `hudRightControls`
  similarity `0.715 -> 0.720`, meanAbsDelta `51.576 -> 49.436` compared with
  the cleaner 0042 baseline. Overall remains `97%` in 0045 because the chat
  crop is dynamically mismatched (`67%`), so use this pack for the right-button
  coordinate proof and keep 0042 as the cleaner overall visual score.
- 2026-07-09 Crystal/Web Belt label and HUD-subregion diagnostic pass: Web now
  mirrors Crystal `BeltDialog` shortcut-label layering. Crystal creates
  `Key[i]` as direct `BeltDialog` children at `(8 + i*35, 2)` for horizontal
  mode, while item cells sit at `(i*35 + 12, 3)`; labels therefore remain
  visible over occupied potion slots. Web previously nested labels inside each
  slot, adding an accidental 12px offset and letting potion buttons cover
  labels `1` and `2`. The labels are now rendered as direct belt children with
  Crystal parent coordinates and a higher z-index. The capture harness now
  records `labelRect`, and the visual report now emits clean HUD subregions
  (`hudLeft`, `hudBelt`, `hudRightControls`, `hudRightStatus`,
  `hudBottomCenter`) plus a `hudUi` aggregate so full-HUD world/edge pollution
  is visible instead of hidden. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0044-belt-key-label-diagnostics/`
  records belt label rects `1 @ 238,620 26x14`, `2 @ 273,620 26x14`, 0 network
  404s, 0 critical console errors, runtime/layout/entities `100%`, and
  `hudUi=78%` with subregions `left=79%`, `belt=77%`, `rightControls=72%`,
  `rightStatus=73%`, `bottomCenter=80%`. The overall score is `97%` because
  the native/Web chat crop in this sample is dynamically mismatched (`67%`);
  use the screenshot crop/DOM label evidence for this Belt fix and keep the
  cleaner 0042 pack as the latest fair overall visual score.
- 2026-07-09 Crystal/Web MiniMap label/light/radar parity pass: Web now mirrors
  the source-level MiniMap label, light indicator, and radar-dot behavior used
  by Crystal's `MiniMapDialog`. Crystal sets
  `LocationLabel.Text = Functions.PointToString(...)`, whose format is
  `"{0}, {1}"`, so Web displays `335, 262`; the coordinate label now keeps the
  same `56x18` vertically centered box, and MiniMap labels use Arial like
  Crystal `MirLabel`. Missing `Prguse` light frames `2092`, `2094`, and `2095`
  were exported so the Web light indicator follows Crystal's `TimeOfDay`
  mapping. The radar overlay now draws Crystal-style 2x2 `RadarTexture` rects
  at `(x - 0.5, y - 0.5)`, skips dead entities, and preserves Crystal's
  white/player, green/NPC, red/other, and blue/owned-object color path where
  the Web state exposes ownership. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0042-minimap-radar-dot-label-welcome/`
  records `miniMapLightOriginal=/original-ui/Prguse/2092.png`, 0 network 404s,
  0 critical console errors, overall `98%`, estimated human band `91-100%`,
  pixel trend `96%`, HUD `78%`, world `83%`, minimap `80%`, and chat `83%`.
  MiniMap meanAbsDelta moved slightly from the 0039 `29.718` to `29.535`;
  crop pairs and `*-diffx4.png` heatmaps are attached. Remaining minimap work
  is now true raster crop/color and source sampling parity, not coordinate,
  light-icon, label-box, or radar-dot semantics.
- 2026-07-09 Crystal/Web HUD text parity pass: Web now mirrors Crystal
  `MainDialogs.cs` for the main-HUD gold label
  (`GoldLabel.Text = GameScene.Gold.ToString("###,###,##0")`) and pins the
  main HUD to Crystal's default `Settings.FontName = "Arial"` instead of
  inheriting the page's Georgia serif. The HUD no longer renders raw `3457`;
  same-scene Web DOM state and right-HUD crops show `3,457`, matching native
  Crystal. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0036-hud-font-arial-cleanline/`
  records HUD gold text `3,457`, weight `48`, space `38`, HP `51/51`, EXP
  `48.33%`, 0 network 404s, 0 critical console errors, overall `98%`,
  estimated human band `91-100%`, pixel trend `95%`, HUD `77%`, world `83%`,
  minimap `79%`, and chat `82%`. The 0034 gold-only pass scored HUD `78%`,
  so the font fix is kept as source-backed visual cleanup rather than a
  claimed score win. Remaining HUD work is true bottom-panel asset/layout
  drift, button/glow/text placement polish, and residual chat scrollbar/font
  pixels.
- 2026-07-09 Crystal/Web ChatDialog and HP orb parity pass: startup chat
  content/state now follows the native running Crystal client instead of
  showing Web debug/status pollution. Web seeds the same visible four-line
  window (`Online Players`, the current Crystal `LineMessage`, then two more
  `Online Players`) while keeping the backend `Welcome` chat in older history,
  supports a `?crystalLineMessage=...` capture override for Crystal's rotating
  `Envir/LineMessage.txt`, maps `ChatType.LineMessage` to a blue/white
  Crystal line, renders chat rows as AutoSize-width labels, and hides the empty
  input box like Crystal `ChatTextBox.Visible=false`. The low-level Warrior
  HP-only orb also no longer uses the two-resource 50px half-orb crop, so full
  HP renders as a complete red orb. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0033-chat-and-hp-orb-clean/`
  captured `Online Players: 1 / Welcome to Crystal Mir 2 released by Suprcode.
  / Online Players: 1 / Online Players: 1` on both clients, kept HUD readouts
  at `HP 51/51`, `MP 32/32`, EXP `435/900`, gold `3457`, and weight `14/62`,
  and scored overall `98%`, estimated human band `91-100%`, pixel trend `96%`,
  HUD `78%`, and chat `83%` with 0 network 404s / 0 critical console errors.
  Remaining gaps are now HUD bottom-panel asset/layout drift, residual chat
  font/scrollbar pixels, world scene review (`83%`), and minimap crop/color
  (`79%`).
- 2026-07-09 Crystal/Web bottom-right HUD parity pass: the main HUD now follows
  Crystal `MainDialogs.cs` semantics for the two small right-side readouts.
  Web `WorldSnapshot.maxWeight` is sourced from Crystal player stats instead
  of the old fixed `100`, and the main HUD displays remaining bag weight
  (`maxWeight - currentWeight`) plus Crystal's 46-slot inventory free-space
  view, with the gold row visible below it. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0027-hud-weight-diagnostics/`
  captured the same native-state character at `0 @ 335,262` with
  `currentWeight=14`, `maxWeight=62`, HUD `weight=48`, HUD `space=38`, and
  gold `3457`; the right-HUD crop now visually matches Crystal's
  `48 / 38 / 3,457` readout. The score remains overall `94%`, estimated human
  band `87-100%`, runtime/layout/entities `100%`, pixel trend `85%`, and 0
  network 404s / 0 critical console errors. Remaining HUD work is now broader
  asset/layout and chat-panel parity, not this bottom-right status semantic.
- 2026-07-09 Crystal/Web native-state, max-MP, and EXP-curve pass: same-scene capture can now
  seed Web from the native Crystal account state and apply the same live
  character snapshot through the token-gated QA-control path before scoring.
  Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0025-exp-debug/`
  captured `Cdx0708235326` on `BichonProvince` map `0 @ 335,262` with Web
  state aligned to native level `6`, `HP 51/51`, `MP 32/32`, EXP `435/900`
  (`48.33%`), gold `3457`, 6 inventory items, 2 belt items, and 8 equipment
  items. The latest score is overall `94%`, estimated human band `87-100%`,
  runtime/layout/entities `100%`, pixel trend `85%`, and 0 network 404s / 0
  critical console errors.
  This clears the previous P0 runtime hygiene issue from missing potion icons
  (`Items/398.png`, `Items/394.png`) and the post-snapshot `playerMaxMp`
  drop (`32` now remains visible in Web state after transfer). The upsert path
  now reads Crystal `ExpList.ini`, so level-6 max EXP is `900` instead of the
  old Web placeholder `100`. Remaining visible gaps are true frontend parity
  work rather than account-state pollution: P1 HUD assets/layout (`71%`,
  especially chat overlap and remaining bottom-panel asset drift), P2 world human
  review (`83%`), P2 minimap crop/color (`79%`), and P2 chat content/state
  (`62%`).
- 2026-07-09 Crystal/Web HUD-state diagnostic pass: same-scene evidence now
  includes both Web HUD/item DOM state and a read-only extraction of native
  Crystal `Server.MirADB` account state. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0019-hud-state-diagnostics/`
  keeps runtime/layout/entities green at `100%`, overall `95%`, pixel trend
  `87%`, and 0 network 404s / 0 critical console errors, but explicitly marks
  the top P1 gap as dynamic state pollution rather than pure HUD art drift.
  Web is captured as `Cdx0708235326` level `1`, `HP 18/18`, `MP 14/?`, gold
  `0`, empty belt/inventory, and starter Web equipment; native Crystal
  account state for the same visible character is level `6`, HP `51`, MP `32`,
  gold `3457`, belt `(HP)DrugSmall` + `(MP)DrugSmall`, and equipment
  `EbonySword`, `BaseDress(M)`, `Candle`, `GoldNecklace`,
  `WornIronBracelet`, `CopperRing`, `OldCopperRing`, `OldLoafer`. The next HUD
  pass should therefore align capture character state before treating the
  remaining HUD/chat pixels as asset/layout defects.
- 2026-07-09 Crystal/Web same-account same-scene blend pass: Bevy map rendering now leaves
  Crystal additive glow sprites on the DOM fallback instead of folding them
  into normal-alpha atlas tiles. DOM blend sprites use cleaned
  `/generated/original-map-blend/...` frames with `mix-blend-mode: screen`;
  tall blue/white columns use opacity `1` plus
  `brightness(2.35) saturate(1.08)`, while compact Bichon torch glows use
  opacity `0.78` plus `brightness(2.25) saturate(0.72)`. The visual capture
  scripts now support `--createAccount` / `--characterName`, allowing Web
  evidence to use the same visible character name as the native Crystal
  client. Evidence
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260709-0017-same-account-native335/`
  captured same-name Crystal/Web at the native client coordinate `0 @
  335,262`: overall visual score `97%`, estimated human band `90-100%`,
  runtime/layout/entities `100%`, pixel trend `92%`,
  `bevyMapRenderer.tileCount=400`, `domBlendSpriteCount=12`, and
  `network404Count=0` / `criticalConsoleErrorCount=0`. Remaining visible gaps
  from the same report are HUD state/assets (`77%`), world human review
  (`90%`), minimap crop/color (`80%`), chat panel state (`71%`), and mismatched
  HP/MP/equipment/belt/HUD state between native and Web captures.
- 2026-07-08 QA-control rerun: the browser automation can now use a safe local
  control wrapper instead of pretending production clients may send debug
  commands. `qaControl` is token-gated and production command safety remains on.
  Report
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
  passed incoming damage (`18 -> 0`) and death/revive (`0 -> 18`) on Rust
  `7111`. Active frontend gaps from the same evidence: server sent a
  `DamageIndicator` but DOM `.scene-damage-floater` stayed at peak `0`; the
  seeded pickup route failed to reach the Blue Potion tile; normal kill/XP/drop
  remain red; Monster `007` original-ui metadata still 404s. Automation gap:
  QA-control transfer/spawn needs explicit ack/settle handling because some
  packets landed late in the trace.
- 2026-07-08 Rust `7111` attack-trace rerun: incoming monster damage is now
  proven from the real Web client rather than inferred from backend-only tests.
  The updated harness records map/object ids, sent attack frames, melee
  approach, delayed server combat packets, and retry details for the first
  `StartGame` race. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
  reached melee with `ForestYeti` object `258949`, captured target
  `ObjectAttack`, `ObjectStruck`, and `DamageIndicator` packets, and saw player
  HP fall `18 -> 3`. Active frontend/client gaps remain: normal attack-kill,
  XP, loot, and death/revive feel still need a stable same-scene rerun; the
  current QA control lane is unreliable because `transferMap` reports sent but
  map/position stay unchanged, `event.spawn RakingCat0` yields no visible
  hostile, and death/revive can fail if a live hostile keeps attacking during
  the revive beat. Missing original sound/monster metadata 404s still affect
  feel.
- 2026-07-08 Rust `7111` pickup/death rerun: the drop-click route is now less
  overdriven by QA fallback movement, and latest targeted evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-pickupwait5s-20260708/report.md`
  passes deterministic Blue Potion pickup plus death/revive on the real Rust
  gateway. This confirms the current pickup failure class is no longer the stale
  predicted/self-coordinate bug. Active gaps remain: hostile-retaliation proof
  still needs a stable adjacent attack packet sequence (`survivaltick` did not
  produce accepted player-damage evidence), real kill/XP/drop should be rerun
  with a normal combat window, and original sound/monster metadata 404s still
  affect feel.
- 2026-07-08 Rust `7111` pickup/death lane: Web action gating now uses
  `state.authoritativePlayer` from packet ACKs instead of predicted/render
  self, and the QA harness counts carried items across inventory plus belt.
  Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-authpickupseed7-20260708/report.md`
  passed deterministic Blue Potion pickup (`GainedItem x1`, carried `0 -> 1`)
  and death/revive via `@DIE` plus `townRevive` (`0 -> 18`, respawn
  `0:330,270`). This closes the specific frontend-side pickup misclassification
  and stale-coordinate action-gating gap. Active frontend/client gaps remain:
  combat kill/XP feedback still lacks green evidence in this seeded run,
  monster retaliation did not reduce HP, and missing original UI sound assets
  (`Sound/103.wav`, `Sound/144.wav`) still create runtime 404s.
- 2026-07-07 Rust-gateway combat/effect settled pass: damage feedback is no
  longer purely backend-blocked. `qa-combat-survival.mjs` now uses normal
  `walk` packet fallback when WebGL2 has no DOM tile hit layer, rotates melee
  approach tiles, and settles late CDP WS frames before scoring. `page.tsx`
  sends targeted combat-confirm ticks and renders `DamageIndicator` directly
  into the scene overlay. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
  connected to Rust `7111`, landed melee damage, saw target HP fall
  (`minPercent=95`), observed 4 server damage indicators, and passed the DOM
  `.scene-damage-floater` gate with peak 1. Active frontend/client gaps remain:
  kill/death animation, loot/XP feedback, and death/revive UI cannot be
  accepted until backend gameplay emits `ObjectDied`/XP/drop/dead-state
  evidence; missing original UI sound/monster metadata still causes run-time
  404s.
- 2026-07-07 Rust-gateway combat/effect anchor pass: the current combat gap is
  now a strong backend/effect integration blocker, not just a frontend feel
  suspicion. Hardened `qa-combat-survival.mjs` writes partial reports per beat,
  writes final JSON/Markdown atomically, avoids known Crystal field safe-zone
  circles, and transfers to Woomyon combat anchor `1:315,100`. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-anchor-20260707/report.md`
  connected to Rust `ws://127.0.0.1:7111/ws` and fought outside safe zone. Red
  results: melee attack packets were sent against `ForestYeti`, but there was
  no `ObjectStruck`, no `DamageIndicator`, no target health drop, and no kill;
  provoking `RakingCat0` did not reduce player HP; `@DIE` did not transition to
  dead/revive. Frontend acceptance should therefore keep damage floaters,
  attack/struck/death animation feel, loot/XP feedback, and death/revive UI in
  `[~]` active status until the gateway/Zone combat outcome is fixed and this
  same evidence path is rerun. Asset gaps from the run: missing
  `original-ui/Sound/103.wav` and missing `Monster/007` original-ui metadata.
- 2026-07-07 combat/effect-heavy probe: default self-camera movement now has a
  follow-up combat/effects evidence lane, and it is currently red. Report
  `docs/generated/player-qa/combat-survival-default-selfcamera-20260707/report.md`
  produced 11 screenshots and 11/11 completed harness beats on the default
  Bevy WebGL2 route, but connected through `ws://127.0.0.1:7110/ws` instead of
  Rust `7111`, failed to reliably reach/engage a hunting-field monster, skipped
  `.scene-damage-floater` because no landed damage was observed, and failed
  death/revive (`@DIE` left the player at `0/18` without a dead-state transition).
  Positive signal: the survival beat did observe player HP falling `18 -> 9`.
  Magic/effect QA is not yet evidence: the attempted
  `magic-skills-default-selfcamera-*` runs currently stall before report
  generation around login/register and need harness repair. Harness follow-up:
  both combat and magic QA scripts now wrap CDP commands in a 15s timeout, so
  future stalls should write a fatal report instead of hanging silently.
- 2026-07-07 default self-camera held/chorded pass: keyboard movement now has
  the same default self-camera evidence style as the Bichon click route. The
  chorded/cardinal capture
  `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
  is `ok=true`, 148 JPEG frames, 8 movement commands, final `329,270`, no
  failed assertions, no logical rollback, no interaction pollution, and Bevy
  WebGL2 packed rendering. The first held Shift+Right default capture
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
  intentionally documents the red repro: movement reached `345,270`, but one
  logical rollback occurred when predicted self position briefly fell from
  `332,270` to the server `331,270` between run ACKs. Fix: fresh unconsumed
  direction `queuedMoveIntent` now counts as self-movement transport evidence,
  so prediction is not cleared during sustained held-key cadence. Verified
  rerun
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-queuedintentfix-jpeg-20260707-2000.json`
  is `ok=true`, 122 JPEG frames at ~50ms cadence, 8 movement commands, average
  ACK `198.5ms`, max ACK `439ms`, final `345,270`, 0 logical rollback
  warnings, 0 failed assertions, 0 frame capture errors, 0 interaction
  pollution, and no console/network failures. Remaining active gap:
  equal-duration native held/video capture plus combat/effect-heavy scenes.
- 2026-07-07 default self-camera temporal pass: the previous equal-cadence
  Web motion/change-intensity gap is now closed for the current Bichon
  four-click route. Bevy self-camera + per-entity interpolation is requested by
  default and activates only when the Bevy entity/map renderer is live; the DOM
  self overlay cancels the parent camera transform so nameplate/health overlays
  stay pinned instead of jumping. Native evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-highfps-20260707-2000.json`
  remains `ok=true`, 104 JPEG frames over 5167ms, average sample delta
  `50.17ms`, and 4 native clicks at `51/950/1860/2763ms`. Matching default-URL
  Web content-only evidence
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-samedir-4click-windowfps-content-default-selfcamera-jpeg-20260707-2000.json`
  is `ok=true`, 105 JPEG frames at ~50ms cadence, 4/4 walk ACKs, average ACK
  `139.25ms`, max ACK `369ms`, no visual jumps, no interaction pollution, no
  failed assertions, and no console/network failures. The final high-fps report
  `docs/generated/player-qa/movement-jitter/temporal-native-highfps-route-vs-web-windowfps-content-default-selfcamera-clicksequence-bichon-20260707.md`
  records normalized visual delta/sec `Crystal 63.7831` vs `Web 62` (Web ratio
  `0.972`) and changed-pixel/sec `Crystal 1.718936` vs `Web 1.7788` (Web ratio
  `1.0348`). Remaining active gap: broaden this evidence to held/chorded
  movement plus combat/effect-heavy scenes, then tune HUD/chat temporal polish
  and effect-layer motion.
- 2026-07-07 native/Web 4-click temporal pass: the real-input native evidence
  now covers a sustained four-click route, not only a one-step click. Native
  Computer Use evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-20260707-2000.json`
  is `ok=true` with 23 captured native frames and 4 real window clicks. Web
  capture now has explicit `clickSequence` support for fixed relative routes.
  The first same-area route
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-4click-left-jpeg-20260707-2000.json`
  is intentionally retained as red pollution evidence because it hit
  `Teleport_Gilbert` and emitted an `interact`; the clean accepted route
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-leftclean-4click-jpeg-20260707-2000.json`
  passed with 29 JPEG frames, 4/4 walk ACKs, average ACK `204.25ms`, max ACK
  `590ms`, 0 frame capture errors, 0 critical console errors, and 0
  interaction pollution. Report
  `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-route-vs-web-clicksequence-bichon-leftclean-20260707.md`
  records aggregate visual delta `Crystal 11.42` vs `Web 10.11` (ratio
  `0.8853`). Remaining active gap: native higher-cadence/video capture and
  exact same clean-route replay before human smoothness acceptance.
- 2026-07-07 native Computer Use frame-cadence pass: the previous native
  synthetic-input blocker is closed for one-step click movement. New script
  `apps/web/scripts/capture-original-computer-use.mjs` drives the native
  `Legend of Mir 2` window through Computer Use and saves screenshots in the
  temporal-report JSON shape. Evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-click-620-520-20260707-2000.json`
  captured 9 real native frames; matched Web evidence
  `docs/generated/player-qa/movement-jitter/web-motion-clicktarget-bichon-287-611-plus1-left-jpeg-1800ms-20260707-2000.json`
  passed with one clean `walk DownRight`, final `288,612`, 10 JPEG frames, 0
  failed assertions, and 0 interaction pollution. Report
  `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-click-vs-web-clicktarget-bichon-1800ms-20260707.md`
  records native mean visual delta `7.09` / changed-pixel ratio `0.16855`
  versus Web `4.51` / `0.108783`. Remaining active gap: repeat on longer
  route/run samples and improve capture cadence before calling human-feel
  parity accepted.
- 2026-07-07 frame-cadence automation pass: Web movement evidence can now be
  sampled as real full-stage frames instead of relying only on final
  screenshots and movement ACKs. `capture-web-movement-jitter.mjs` supports
  scheduled per-sample frame capture, JPEG frame output, and blank WebGL canvas
  detection/fallback; `report-movement-temporal-parity.mjs` scores
  consecutive-frame visual deltas. Evidence
  `docs/generated/player-qa/movement-jitter/web-motion-keyhold-right-jpeg-cadence-20260707-2000.json`
  passed with 23 JPEG frames, about 98ms average frame-sample spacing, 0 frame
  capture errors, 0 failed assertions, 0 interaction pollution, and final
  player `335,270`. Report
  `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-static-vs-webjpeg-cadence-20260707.md`
  records aggregate visual delta `Crystal 0.37` vs `Web 7.09`; this highlights
  that the current native Crystal synthetic-input sample is not a valid moved
  baseline yet. SendInput scan-code keyboard, right-click target, and
  left-click target probes also stayed near static visual deltas (`0.43`,
  `0.33`, `0.46`). Remaining active gap: automate native Crystal real input or
  video capture so animation cadence can be compared against Web's now-clean
  held-run/JPEG trace.
- 2026-07-07 held/chorded keyboard movement closeout: the first forced WebGL2
  held Shift+Right Bichon repro exposed a backend world-runtime issue rather
  than a frontend renderer hitch. Before the fix,
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-webgl2-movelog-20260707.json`
  was `ok=false` because the player reached `0:339,270`, hit the leftover
  starter demo transfer, received delayed ACKs `7481/4066ms`, and rolled back
  toward `0:330,270`. After clearing starter transfers from full Crystal world
  runtime,
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
  is `ok=true` with 8/8 movement ACKs, max ACK 359ms, no logical rollback,
  no stale prediction, no command queue warnings, no interaction pollution,
  and Bevy WebGL2 packed rendering with no DOM entity fallback. The chorded
  cardinal rerun
  `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
  also passed strict checks with all eight ACKs under 300ms. Remaining
  frontend feel gap: native Crystal animation cadence, per-frame sprite timing,
  and camera/HUD temporal polish still need side-by-side recording; static
  screenshots and now-clean movement ACK traces alone do not prove human
  smoothness parity.
- 2026-07-07 crowded Bichon click-route closeout: the earlier Bichon red sample
  was polluted by entity hit targets and then by a Gateway post-ACK scheduling
  race. `capture-web-movement-jitter.mjs` now supports clean route patterns,
  entity-hit avoidance, pollution-fail assertions, and final Bevy renderer
  readiness waits; the self player sprite/nameplate no longer intercepts ground
  movement clicks. Evidence:
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
  is `ok=true` with clean settle, 4/4 ACKs, ACK latencies `490/164/33/5ms`,
  0 entity-hit clicks, 0 non-movement gameplay frames, Bevy WebGL2 packed
  rendering, and no DOM entity fallback. The matching temporal summary is
  `docs/generated/player-qa/movement-jitter/temporal-clickroute-postgrace1500-20260707.md`.
  A repeat capture
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-rerun-20260707.json`
  also passed with ACK latencies `582/78/109/7ms`.
- 2026-07-07 movement temporal click-route pass: native Crystal short-sequence
  frame capture is now paired with Web per-sample frame capture and a generated
  temporal summary. `capture-web-movement-jitter.mjs` can save frame images via
  `--captureFrameImages true`, align mouse timing with `--routeStepMs` /
  `--clickHoldMs`, and now filters movement ACK latency against self
  `UserLocation`-class packets instead of other entities' `ObjectWalk` noise.
  A Web input gap was fixed: right-click target movement now primes run
  immediately, closing the previous "right-click route sent only walk packets"
  mismatch. Evidence:
  `docs/generated/player-qa/movement-jitter/original-motion-frames-20260707-183007.json`
  captured 16 native Crystal frames;
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-runfix-woods-20260707-183748.json`
  passed strict Web click-route checks on WoomyonWoods(S) with `ok=true`, 8/8
  self ACKs, max ACK 301ms, Bevy WebGL2 drawn, 0 critical console errors, and 0
  non-favicon 404s; and
  `docs/generated/player-qa/movement-jitter/temporal-clickroute-runfix-20260707-183748.md`
  summarizes the comparison. Remaining gap: the Bichon crowded route sample
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-runfix-clean-20260707-183601.json`
  still fails strict ACK responsiveness after the first run due to crowded
  AOI/blocked-route conditions, so Bichon mouse-route feel is not accepted yet.
- 2026-07-07 Crystal/Web same-scene movement/resource clean pass: the local
  Bichon `0:286,610` keyboard sequence capture now suppresses the Web-only
  tutorial, waits for playable scene state, and runs against a resource set
  expanded with Crystal `NPC/09`, `Monster/011`, and `Monster/013`. Evidence:
  `docs/generated/player-qa/movement-jitter/local-crystal-visual-baseline-keyseq-clean-20260707-181953.json`
  passed with `ok=true`, `strictStatus="settled"`, 4/4 movement frames ACKed,
  all 15 movement assertions green, no visual jumps, no logical rollback, no
  residual movement plan, Bevy WebGL2 gameplay layers drawn, 0 critical console
  errors, and 0 non-favicon 404s. This closes the polluted 367-resource-404
  movement sample as a measurement problem; the remaining Crystal "smoothness"
  gap should now be investigated with temporal recording/animation cadence
  comparisons instead of resource-load noise.
- 2026-07-07 Crystal/Web same-scene visual harness refresh: added
  `apps/web/scripts/report-crystal-visual-parity.mjs` and
  `npm run qa:visual-parity` so Windows Crystal screenshots and Web captures
  can be scored as repeatable trend evidence instead of ad-hoc screenshots.
  `capture-crystal-parity.mjs` now waits for visual scene readiness, suppresses
  the Web-only tutorial overlay during parity captures, emits Bevy map/entity
  renderer diagnostics, and separates raw console errors from critical console
  errors. The Web-only top-center objective tracker now defaults off unless
  explicitly enabled with `?objectiveTracker=1` or
  `localStorage["mir2:objectiveTracker"]="1"`, removing the previous P1
  silhouette mismatch while preserving an opt-in onboarding path. Current
  local evidence:
  `docs/generated/player-qa/visual-parity/current-20260707-181734-report.md`
  reports weighted 95%, runtime/layout/entities 100%, pixel trend 86%, and
  estimated human visual/feel parity band 88-100% for Bichon `0:286,610`,
  with no recurring automated top gaps. Remaining visible differences are now
  mostly temporal/state-sensitive: Crystal lighting/shadow timing, animation
  frame mismatch, chat text, and live HP/MP/gold state. Movement/feel still
  requires a separate recording pass; this static visual score must not be used
  to close the "Crystal feels smoother" gap by itself.
- 2026-06-14 gameplay-feel pass (merged to `main`, deployed): floating damage
  numbers + hit flash (Crystal `DamageIndicator`, #98) close the "combat felt
  dead" gap; all Crystal sound effects are wired with faithful triggers (#99);
  ground drops show real item icons with walk-over-to-pick-up (#97); a loading
  overlay replaces the black stage on first entry (#95); movement + map-object
  alpha-keying run off the main thread (#93, #96). The active R2 release
  `mir2/v/20260601-fullcrystal-a2f10be0` is complete (0 missing), so the prior
  sprite-404 storms are resolved and the per-deploy asset-cache wipe is fixed
  (#100). Continuous monster-click AutoHit (chase + re-path + swing) also landed.
- 2026-05-27 NPC input and skill preflight closeout: Player Web now preserves
  server skill cast metadata (`spell`, `castKind`, `offensive`, hotkey/timing)
  from the world snapshot and routes skill clicks by Crystal cast mode instead
  of always sending an opaque `castSkill`. Passive skills are not actively
  cast, target skills require a selected live monster target when offensive,
  ground skills wait for a clicked tile, direction skills use the player's
  facing, and self/toggle skills use the matching magic/toggle packet shape.
  Debug world snapshots now also carry `npcScriptDiagnostics` so admin/debug
  surfaces can inspect script parser/runtime diagnostics. Evidence: Web
  typecheck plus the focused simulation NPC/skill preflight regressions in
  this pass.
- 2026-05-27 Crystal map/minimap/resource parity closeout: scene blueprint
  application now preserves existing `miniMapIndex`/`bigMapIndex` when a
  partial blueprint reports `null`; Bichon map `0` resolves mini and big map
  index `101` from `CRYSTAL_MINI_MAP_TRANSFORMS` rather than relying on the
  respawn manifest; minimap transform map names normalize by basename,
  lowercase, and `.map` stripping; and object-mode original-map frames now use
  exported Crystal frame offsets for all frames with offset metadata. The old
  Bichon torch offset remains only as a starter-JSON fallback for missing
  metadata. Scene asset readiness keys now use a stable visible URL-set hash
  instead of raw player coordinates, reducing per-step preload churn. Evidence:
  `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` passed,
  including Web typecheck, movement-controller, minimap-transform,
  resource-loading, focused Rust gateway/simulation/admin checks, and
  `git diff --check`.
- 2026-05-27 Crystal resource loading hardening: Player Web now treats Crystal
  `.Lib` files like indexable MLibrary sources instead of decoding every frame
  during parse. `crystal-map-loader` stores library frame offsets and lazily
  decodes only requested frames behind decoded-frame/server map/server library
  LRU byte caps. Production request paths are read-only by default:
  `MIR2_DISABLE_REQUEST_FILE_WRITES=1` or production without the explicit dev
  opt-in returns a visible `resource_missing` error when a required PNG/lib/map
  is absent, and synthetic map fallback requires
  `MIR2_ALLOW_SYNTHETIC_MAP_FALLBACK=1`. Scene blueprint cache keys are now
  quantized by map/chunk/size bucket/schema version with disk TTL/size trim.
  `sceneAssetReadiness` actually preloads visible center-priority URLs and
  separates `interactionReady` from `visualReady`; runtime metrics now include
  scene cache key, original-map sprite/cell counts, sprite library cache count,
  DOM image count, Bevy atlas bytes, and alpha-keyed blob count/bytes. Evidence:
  `MIR2_CANDIDATE_SCOPE=local bash infra/check-candidate-gate.sh` passed,
  including Web typecheck, `test:movement-controller`,
  `test:resource-loading`, focused Rust gateway/simulation/admin checks, and
  `git diff --check`. Production/browser visual acceptance remains open.
- 2026-05-26 Crystal Movement Authority Convergence v1: Player Web movement
  now treats server `UserLocation`/movement packets and `worldSnapshot` as the
  only sources allowed to write self `world.entities` coordinates. Normal UI
  movement no longer sends debug `moveTo`; tile, direction, keyboard, and
  mouse movement are queued as Crystal `walk`/`run`/`turn` direction packets
  behind one pending self move, 600ms walk/run cadence, render-only local
  prediction, and Crystal run prewarm. ACK or snapshot disagreement clears the
  pending move/prediction and keeps world state at the server coordinate.
  Verification passed `pnpm --dir apps/web run test:movement-controller`,
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, Rust fmt checks,
  focused `mir2-simulation` Crystal movement tests, and focused
  `mir2-gateway` movement/Zone route tests. Full `mir2-gateway` was attempted
  but manually stopped after an unrelated two-sided trade rollback test ran for
  several minutes without exiting; focused gateway movement coverage passed.
- 2026-05-26 production walk-run-reverse input closeout: the live repro was
  not just a slow ACK; production could intermittently omit or delay the Run
  edge when the player walked, pressed run, then reversed direction quickly.
  Player Web now preserves Shift/run key edges, keeps a one-action reverse
  backlog instead of overwriting the current queued move, upgrades same-direction
  queued Walk to Run, and lets the movement confirm tick drain that backlog
  instead of leaving prediction state behind. The movement capture harness now
  asserts that a declared keyboard sequence really sends the expected
  `walk/run` WebSocket frames. Web deployment
  `dpl_HttHWiP21hufr1d3mm6fMsHNwcmW` is live behind
  `https://mir2.obelisk.build`, paired with UCloud Gateway release
  `20260526T1918CST-move-input-buffer`. Verification passed Web typecheck,
  movement harness syntax, production `/health`, direct Gateway WSS smoke
  `docs/generated/load/remote-move-input-buffer-wss-smoke-20260526.json`, and
  headed Chrome WebGL2 evidence
  `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-webgl2-20260526b.json`
  plus faster 180ms stress
  `docs/generated/player-qa/movement-jitter/prod-move-input-buffer-walk-run-turn-fast-webgl2-20260526a.json`.
  Both captures sent `walk Right -> run Right -> walk Left`, settled at
  `332,270 Left`, had no visual/logical rollback, no stale prediction, no
  residual pending plan/queue, raw WebGL2 `renderedLayers=20`, zero critical
  console errors, and zero non-favicon 404s.
- 2026-05-26 production asset-404 and movement-tick closeout: the live console
  spam for `original-map/WemadeMir2/Objects/2652..2661` and
  `Objects23/1418/1420/1423/1425/1429` was caused by incomplete remote asset
  coverage for the active immutable asset prefix plus overly aggressive retry
  behavior. The missing current-scene files were uploaded to R2 under
  `mir2/v/37596e16d64fde7c`, and immutable original-map/original-ui failures
  now negative-cache instead of appending repeated `mir2ImgRetry` cache busters.
  Production web deployment `dpl_8s8BqYBXe5q5DN9jajRUFnFwFwkt` shipped that
  retry hardening. Follow-up headed Chrome evidence first proved resource
  errors were clean but exposed a second Walk ACK at about `1648ms`; that was
  traced to Gateway deferring runtime ticks by the old 1200ms movement input
  grace. Gateway release `20260526T1435CST-move-tick-grace0` is now installed
  on UCloud with default movement input grace `0`. Verification passed
  `cargo +1.89.0 fmt --check -p mir2-gateway`, focused Gateway tick coverage
  locally and on UCloud, public health, WSS smoke
  `docs/generated/load/remote-move-tick-wss-smoke-20260526.json`, and headed
  Chrome production WebGL2 evidence
  `docs/generated/player-qa/movement-jitter/prod-move-tick-grace0-webgl2-existing-20260526.json`
  / `.png` with `ok=true`, direct WSS host
  `wss://165.154.65.136.sslip.io/ws`, raw WebGL2 atlas `renderedLayers=21`,
  two ordered Walk ACKs at `398ms` and `609ms`, clean settle, no critical
  console errors, and no non-favicon 404s. `Objects/289.png` remains a separate
  source-data or map-library mapping gap because that exact file is absent from
  the local source tree too; the new immutable negative cache prevents it from
  becoming a retry storm while the mapping gap is investigated.
- 2026-05-27 production original-asset manifest hardening: Web now generates
  `public/original-asset-manifest.generated.json` during build/test, hashes it
  into `/api/asset-manifest`, and makes `/api/scene/crystal` refuse map frames
  that are neither present locally nor declared in that manifest. The R2 release
  builder stages every manifest-declared `/original-map` and `/original-ui` PNG,
  and the R2 upload workflow can HEAD the final CDN object for each declared
  original asset before accepting the release. Resource tests now lock the
  previously failing `Objects23/1422/1426/1427/1428`, `NPC/16/27/83`, and
  `Monster/000/139` paths plus Bichon scene blueprint frame coverage.
- 2026-05-27 deterministic asset release wiring: `/api/asset-manifest` now
  prefers `MIR2_ASSET_VERSION`, ignores file mtimes for versioning, and uses
  only `original-asset-manifest.generated.json.assetHash` from the original
  asset manifest. The Web Assets R2 workflow resolves
  `MIR2_ASSET_VERSION=${GITHUB_SHA::12}`, stages/uploads/verifies R2 under
  `mir2/v/$MIR2_ASSET_VERSION`, can deploy the `mir2-domain-proxy` Worker with
  the same version, and only then deploys Vercel. The player-domain Worker now
  serves same-origin `/original-map`, `/original-ui`, and
  `/generated/original-map-blend` requests from R2, so Bevy `/original-ui`
  requests do not depend on React image fallback.
- 2026-05-26 raw WebGL2 atlas gameplay closeout: Player Web now has a
  browser-native `WebGl2EntityAtlasLayer` that reuses the existing
  `BevyEntityRenderState`/entity atlas schema and draws atlas-backed entity
  layers into a transparent WebGL2 canvas. In gameplay wiring, WebGPU still
  uses the Bevy entity renderer; forced WebGL2 keeps the Bevy canvas hidden
  for opaque-surface safety and uses the raw WebGL2 atlas layer once an entity
  atlas is active. The raw WebGL2 path now drives atlas warmup through the same
  GPU renderer condition as WebGPU, and initial scene interaction waits for the
  raw atlas to be ready so first movement does not overlap atlas warmup.
  Headed Chrome local gameplay evidence
  `docs/generated/player-qa/movement-jitter/local-webgl2-raw-atlas-gameplay-gated-20260526.json`
  passed with `ok=true`, selected/compiled backend `webgl2`, `canvasHidden=true`,
  `rawWebGl2Enabled=true`, packed prebuilt atlas `starter-bichon-base`,
  `textureReady=true`, `renderedLayers=21`, `skippedLayers=0`, three Walk sends,
  three ordered `UserLocation` ACKs, no camera-offset stair-step warnings, no
  movement queue warnings, no critical console errors, and no non-favicon 404s.
  The deterministic `/qa/webgl2-entity-renderer` probe remains covered by
  `smoke:bevy-runtime-backends`; evidence
  `docs/generated/player-qa/bevy-runtime-backends/local-webgl2-raw-atlas-probe-20260526.json`
  passed with `rawWebGl2ProbeRendered=true`. Production deployment
  `dpl_Q1k4QFSbGigw9gJ64cfBNcAehjEQ` then shipped the same gate plus hosted
  default Gateway targeting to `wss://165.154.65.136.sslip.io/ws`, away from
  the higher-jitter custom-domain `/ws` route. Bundle probing on
  `https://mir2.obelisk.build` found the direct WSS host in the shipped JS and
  no hard-coded `mir2.obelisk.build/ws`. Headed Chrome production evidence
  `docs/generated/player-qa/movement-jitter/prod-webgl2-raw-atlas-gameplay-focused-direct-default3-20260526.json`
  / `.png` passed with `ok=true`, actual WebSocket
  `wss://165.154.65.136.sslip.io/ws`, selected/compiled backend `webgl2`, raw
  WebGL2 `textureReady=true`, `renderedLayers=21`, prebuilt atlas
  `starter-bichon-base`, three Walk ACKs at `93/51/46ms`, clean settle, no
  camera-offset stair-step warnings after headed-window foregrounding, no
  critical console errors, and no non-favicon 404s.
- 2026-05-26 Bevy runtime backend smoke slice: added
  `npm run smoke:bevy-runtime-backends` so WebGPU-first/WebGL2 fallback is no
  longer checked only by ad-hoc page state. The smoke launches real Chrome,
  exercises default backend selection, forced `?bevyBackend=webgl2`, and
  forced `?bevyBackend=webgpu`, waits for post-boot runtime errors, records the
  selected/compiled backend plus fetched runtime package URLs, and fails on
  critical console errors. Local evidence
  `docs/generated/player-qa/bevy-runtime-backends/local-webgpu-webgl2-runtime-20260526.json`
  passed with default and forced WebGPU selecting/compiling `webgpu`, forced
  WebGL2 selecting/compiling `webgl2`, all runtime JS/WASM package fetches
  succeeding, and zero critical console errors. Important renderer constraint:
  WebGL2 remains unsafe as a transparent Bevy/WGPU surface because the local
  browser advertises opaque alpha only; the newer raw WebGL2 atlas probe above
  is the transparent WebGL2 renderer path to harden instead.
- 2026-05-25 production scene-input unlock closeout: the live "walking command
  feels delayed" repro was caused by movement-triggered viewport asset preloads
  toggling the page back into `scene-assets-pending` and making
  `sceneInteractionReady=false` after the first playable scene. Player Web now
  keeps movement interaction unlocked once the first playable scene has ever
  become ready; later viewport preloads continue in the background without
  gating keyboard, pointer, or mobile repeat input. Production deployment
  `dpl_7iG3bPgA7HTxkvEzN4LxP2rmFmFC` is live behind
  `https://mir2.obelisk.build`, and bundle probing confirmed the new logic is
  present while the old `ready`-only gate is absent. Evidence:
  `pnpm --dir apps/web exec tsc --noEmit --pretty false` and
  `pnpm --dir apps/web exec next build` passed locally before source deploy.
  Headed Chrome production evidence
  `docs/generated/player-qa/movement-jitter/prod-scene-input-unlocked2-webgpu-headed-keyboard-a-nosample-hold-20260525.json`
  / `.png` passed with `ok=true`, WebGPU selected, compiled runtime
  `bevy-b9389323fd0dbead`, packed prebuilt atlas active, no DOM entity
  fallback, `sceneInteractionReady=true` while 699 scene assets were still
  background-loading, five held `A` Walk sends at roughly Crystal cadence,
  authoritative `UserLocation` ACKs `343,342,341,340,339`, no critical console
  errors, and no non-favicon 404s.
- 2026-05-25 production starter-transfer movement closeout: the earlier
  `339 -> 330` rollback in live captures was a server/config issue rather than
  a WebGPU, DOM, or atlas renderer issue. The production Gateway had inherited
  the early demo `starter-east-field-gate` same-map transfer through
  `with_crystal_map_runtime()`. Gateway release
  `20260525T0334CST-starter-transfer-cleanup` is active on UCloud; health
  checks and WSS smoke
  `docs/generated/load/remote-starter-transfer-cleanup-wss-smoke-20260525.json`
  passed. Production headed WebGPU packet-walk evidence crossed from
  `0:338,270` through `0:343,270` with ACKs `339..343`, no map-change packet,
  no rollback to `330,270`, WebGPU selected, packed prebuilt atlas active, no
  critical console errors, and no non-favicon 404s. The packet-walk harness
  still reported expected false route-spam/direction-animation warnings because
  it intentionally sends several same-direction packets in one post-action
  sample window; the authoritative packet evidence is clean.
- 2026-05-25 Bevy entity-atlas direct-image slice: the prebuilt atlas path no
  longer decodes `starter-bichon-base.png` into a 4096x4096 canvas and sends
  64MiB of RGBA pixels to wasm. Prebuilt manifest hits now carry `imageUrl`
  through `BevyEntityRenderState`; the Bevy runtime loads that PNG through
  `AssetServer` and binds the resulting image handle to the atlas layout. The
  existing `setMir2EntityRenderAtlas` pixel-upload API remains as the fallback
  for live browser-packed or explicit pixel atlases. Evidence: `cargo +1.89.0
  check --manifest-path apps/game-client/runtime/Cargo.toml --target
  wasm32-unknown-unknown --no-default-features --features webgl2`, the same
  check with `--features webgpu`, `pnpm --dir apps/web run
  runtime:build:release` producing `bevy-b9389323fd0dbead`, `pnpm --dir
  apps/web exec tsc --noEmit --pretty false`, `MIR2_USE_PREBUILT_BEVY_RUNTIME=1
  pnpm --dir apps/web exec next build`, and headed Chrome local WebGPU play
  against `http://localhost:3100/?bevyEntities=1&bevyBackend=webgpu...`.
  Chrome page-asset inventory observed
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js`,
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm`, and
  `/bevy-entity-atlases/starter-bichon-base.png`. Movement diagnostics
  `docs/generated/player-qa/movement-diagnostics/manual-mplj7xmo-rpw2ln.jsonl`
  recorded 4 keyboard Walk sends, 4 `UserLocation` ACKs, 367-443ms ACK latency
  with 398ms average, final player `328:256`, and 0 anomalies. Remaining work:
  deploy the rebuilt web bundle and rerun production headed Chrome WebGPU
  acceptance on `https://mir2.obelisk.build`.
- 2026-05-25 mobile/touch black-ground guard closeout: a user follow-up crop
  still showed the remaining failure shape where entity sprites and lamps were
  visible over a black ground plane. Source PNG/atlas alpha spot-checks showed
  the sprite alpha channels were present, so the residual risk was still the
  Bevy canvas surface covering the DOM original map on some browser/device
  path. Player Web now treats mobile/touch and explicit
  `?bevyCanvas=0` / `?bevyCanvasHidden=1` as a DOM-entity fallback path: the
  Bevy canvas is hidden in-game and Bevy entity rendering is disabled. Desktop
  WebGPU remains the default experimental Bevy sprite path, while
  `?bevyCanvas=1` / `?bevyEntities=1` can force it back on. The movement QA
  capture script also gained `--finalSceneReadyTimeoutMs` so screenshot evidence
  waits for the post-movement scene asset key to settle before capture.
  Deployment `dpl_8hgZxTUoDTUokZ1tkTkpVQeU2uwf` is live behind
  `https://mir2.obelisk.build`; `/health`, the WebGPU/WebGL2 runtime JS files
  for `bevy-6732ca9f6ab18f6d`, and `/bevy-entity-atlases/manifest.json` all
  returned 200. Production mobile/touch evidence
  `docs/generated/player-qa/movement-jitter/prod-mobile-dom-fallback-canvas-hidden-finalready-20260525.json`
  / `.png` passed with selected/compiled backend `webgpu`,
  Bevy entity renderer `enabled=false`, `canvasHidden=true`, one Walk send, one
  UserLocation ACK, visible ground/entities, no critical console errors, and no
  non-favicon 404s. Production desktop WebGPU evidence
  `docs/generated/player-qa/movement-jitter/prod-desktop-webgpu-transparent-guard-finalready-20260525.json`
  / `.png` passed with Bevy entity renderer `enabled=true`,
  `canvasHidden=false`, `atlasMode="packed"`, `prebuiltHits=2`,
  `lastSource="prebuilt"`, one Walk send, one UserLocation ACK, visible ground,
  no critical console errors, and no non-favicon 404s. Production escape-hatch
  evidence
  `docs/generated/player-qa/movement-jitter/prod-bevy-canvas-off-dom-fallback-finalready-20260525.json`
  / `.png` passed with `?bevyCanvas=0`, `enabled=false`, `canvasHidden=true`,
  one Walk send, one UserLocation ACK, no critical console errors, and no
  non-favicon 404s.
- 2026-05-25 map black-screen transparent-canvas closeout: the black gameplay
  map was not a missing map-resource or atlas decode failure. The DOM original
  map/backdrop layer was rendering underneath the Bevy canvas, while the Bevy
  web surface was composited as opaque black; higher z-index DOM object/entity
  overlays still appeared, making the ground alone look missing. The WebGPU
  runtime now creates a transparent primary window with premultiplied alpha so
  the original map layer remains visible under Bevy entity sprites. Because
  forced WebGL2 only advertised opaque surface support in the local browser,
  WebGL2 fallback now hides the Bevy canvas for original-map gameplay and keeps
  the DOM entity renderer active instead of panicking or covering the map.
  Evidence: `pnpm --dir apps/web run runtime:build:release` produced runtime
  `bevy-6732ca9f6ab18f6d`, `pnpm --dir apps/web exec tsc --noEmit --pretty
  false` passed, local WebGPU capture
  `apps/web/docs/generated/player-qa/movement-jitter/local-transparent-canvas-webgpu-release-20260525.json`
  / `.png` passed with `ok=true`, selected/compiled backend `webgpu`,
  `canvasHidden=false`, Bevy entity renderer enabled, prebuilt atlas hit, one
  Walk send, one UserLocation ACK, no critical console errors, and no
  non-favicon 404s. Local forced-WebGL2 capture
  `apps/web/docs/generated/player-qa/movement-jitter/local-transparent-canvas-webgl2-release-20260525.json`
  / `.png` passed with selected/compiled backend `webgl2`,
  `canvasHidden=true`, Bevy entity renderer disabled for DOM fallback, one Walk
  send, one UserLocation ACK, no critical console errors, and no non-favicon
  404s. Production deployment `dpl_4i4fFrooS8Esuyjh1b1oSb1NCTMb` is live
  behind `https://mir2.obelisk.build`; `/health` returned 200 and both
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js?v=bevy-6732ca9f6ab18f6d`
  and
  `/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js?v=bevy-6732ca9f6ab18f6d`
  returned 200 with `x-mir2-asset-cache: bevy-runtime`. Production WebGPU
  evidence
  `docs/generated/player-qa/movement-jitter/prod-transparent-canvas-webgpu-readywait-20260525.json`
  / `.png` passed with selected/compiled backend `webgpu`,
  `canvasHidden=false`, Bevy entity renderer enabled, `atlasMode="packed"`,
  `prebuiltHits=1`, one Walk send, one UserLocation ACK, no critical console
  errors, and no non-favicon 404s. Production forced-WebGL2 evidence
  `docs/generated/player-qa/movement-jitter/prod-transparent-canvas-webgl2-readywait-20260525.json`
  / `.png` passed with selected/compiled backend `webgl2`,
  `canvasHidden=true`, Bevy entity renderer disabled for DOM fallback, one Walk
  send, one UserLocation ACK, no critical console errors, and no non-favicon
  404s.
- 2026-05-25 Bevy entity atlas prebuild/cache slice: Player Web now checks a
  persistent IndexedDB atlas cache, then a prebuilt
  `/bevy-entity-atlases/manifest.json` atlas pack, before falling back to live
  browser packing. Prebuilt atlas pixels are reused within the page so viewport
  changes do not repeatedly decode/read back the same pack. The starter Bichon
  entity pack is generated by
  `npm run assets:bevy-entity-atlas:build`, covers player/NPC plus common
  Bichon monster roots, and emits
  `public/bevy-entity-atlases/starter-bichon-base.png` with 2,631 source rects
  in a 4096x4096 PNG. Local evidence:
  `docs/generated/player-qa/movement-jitter/local-atlas-prebuilt-postcache-order-a-20260525.json`
  passed on WebGPU with `ok=true`, `sceneInteractionReady=true`,
  `atlasMode="packed"`, 700 active atlas sources, `builds=0`,
  `prebuiltHits=2`, `lastSource="prebuilt"`, one Walk send, one UserLocation
  ACK, no critical console errors, and no non-favicon 404s. Fallback evidence:
  `docs/generated/player-qa/movement-jitter/local-atlas-prebuilt-webgl2-20260525.json`
  passed with forced `bevyBackend=webgl2`, `builds=0`, `prebuiltHits=2`, and
  `lastPrebuiltKey="starter-bichon-base"`. Production deployment
  `dpl_C8sriwUxAeuCyzoY9rAnd24QTw6D` is live behind
  `https://mir2.obelisk.build`; `/bevy-entity-atlases/manifest.json` reports
  `sourceCount=2631`, `imageBytes=4272109`, and `rgbaBytes=67108864`, and the
  PNG returns 200 with `content-length: 4272109`. Production movement evidence:
  `docs/generated/player-qa/movement-jitter/prod-atlas-prebuilt-keyboard-final-20260525.json`
  passed keyboard movement with `builds=0`, `prebuiltHits=1`,
  `lastSource="prebuilt"`, one Walk send, one UserLocation ACK, no rollback,
  no route spam, no critical console errors, and no non-favicon 404s. Mobile
  hand-feel evidence:
  `docs/generated/player-qa/movement-jitter/prod-atlas-prebuilt-mobile-pixelcache-20260525.json`
  passed mobile joystick movement with `atlasMode="packed"`,
  `atlasPendingKey=null`, `builds=0`, `prebuiltHits=1`,
  `lastPrebuiltKey="starter-bichon-base"`, one Walk send, one UserLocation
  ACK, and the same clean assertion set.
- 2026-05-25 WebGPU-first Bevy runtime support: Player Web now builds and
  publishes separate Bevy wasm runtime packages for WebGPU and WebGL2. The
  loader prefers WebGPU on secure browsers with `navigator.gpu`, falls back to
  WebGL2 when WebGPU is unavailable or init fails, and supports explicit
  `bevyBackend=webgpu|webgl2` query/localStorage overrides for diagnostics.
  Runtime debug state is exposed through `window.__mir2BevyRuntimeDebug` and
  included in movement captures. Evidence: `cargo +1.89.0 check
  --manifest-path apps/game-client/runtime/Cargo.toml
  --target wasm32-unknown-unknown --no-default-features --features webgl2`,
  the same check with `--features webgpu`, `pnpm --dir apps/web
  runtime:build:release`, Web typecheck, and headed Chrome local verification
  against `http://127.0.0.1:13014/`: default selected compiled WebGPU,
  forced `bevyBackend=webgl2` selected compiled WebGL2, and a simulated
  missing `navigator.gpu` browser fell back to WebGL2. Screenshot evidence is
  `output/playwright/mir2-webgpu-runtime.png`. Production deployment
  `dpl_HNZTKmg7jPkNju3GhJgAdzk3N9oV` is live behind
  `https://mir2.obelisk.build`; direct runtime probes for
  `/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js` and
  `/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js` return 200 with
  `x-mir2-asset-cache: bevy-runtime`. Headed Chrome production movement
  evidence
  `docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-gateway-20260525.json`
  passed with `ok=true`, selected/compiled backend `webgpu`, WebGPU and WebGL2
  support visible in runtime debug, zero visual jumps, zero route-spam
  warnings, zero logical rollback, zero direction-lag warnings, responsive
  movement queue, clean settle, no critical console errors, and no
  non-favicon 404s. After the follow-up Gateway release
  `20260525T0630CST-zone-magic-mp-cooldown`, the same headed Chrome WebGPU
  movement gate passed again at
  `docs/generated/player-qa/movement-jitter/live-webgpu-keyboard-after-magic-mp-20260525.json`.
  Screenshot evidence is the adjacent `.png`. Remaining frontend risk: the
  first cold entity atlas can still warm in DOM fallback mode, so atlas
  cache/offline pack hardening remains the next renderer optimization.
- 2026-05-25 production Bevy WebGL2 entity-atlas hardening closeout: the
  visible entity sprite renderer is now deployed behind
  `https://mir2.obelisk.build` on Vercel deployment
  `dpl_4PXPyp3VuAT7vHRQr4ueKBTikbtU`. The atlas source set is hardened against
  common movement animation churn by preloading standing/walking/running frames
  for the current action direction, plus all eight player directions, so
  keyboard movement no longer switches from a standing atlas key to a walking
  atlas key mid-action. While a cold atlas is still building, DOM entity sprites
  remain visible as a fallback; once the packed atlas is active, DOM entity
  sprites are hidden and Bevy owns the body/hair/weapon layers. Verification:
  Web typecheck and scoped diff checks passed, public `/health` returned 200,
  production capture
  `docs/generated/player-qa/movement-jitter/prod-bevy-atlas-dir-20260525T043729.json`
  passed with `ok=true`, `atlasMode="packed"`,
  `atlasCurrentKey="entity-atlas-1iogxdg"`, `atlasPendingKey=null`,
  `atlasCachedCurrent=true`, `atlasLatestCurrent=true`,
  `domEntityFallback=false`, 584 atlas sources, two keyboard Walk sends, two
  `UserLocation` ACKs, no non-favicon 404s, and no critical console errors.
  Screenshot evidence is the adjacent `.png`. A headed Chrome production
  hand-feel pass also entered the live game, moved `Scout` with keyboard input
  to `312:249`, and saved
  `docs/generated/player-qa/movement-jitter/headed-chrome-prod-bevy-atlas-final-20260525T0439.png`.
  Remaining renderer optimization: the first cold production atlas build is
  still expensive (`lastBuildMs=54672` for 584 sources), so a future slice
  should move toward a prebuilt/offline entity atlas or narrower CDN-warmed
  pack strategy.
- 2026-05-25 Bevy WebGL2 packed entity-atlas renderer slice: Player Web now
  has a local-verified path that renders visible entity body/hair/weapon sprite
  layers through the Bevy canvas instead of DOM image stacks, while keeping the
  React map, HUD, hit boxes, nameplates, health bars, and quest markers in the
  existing UI layer. The frontend builds a packed RGBA atlas from the current
  visible entity frames, uploads it to the wasm runtime through
  `setMir2EntityRenderAtlas`, and sends layer state through
  `setMir2EntityRenderState`; the Bevy runtime ingests atlas pixels into an
  `Image` asset and renders sprite layers with `TextureAtlas` indices. Toggles:
  `?bevyEntities=1` / `?bevyEntities=0`, `?bevyAtlas=1` / `?bevyAtlas=0`,
  plus matching localStorage overrides. Evidence: `pnpm --dir apps/web
  runtime:build:release`, `cargo +1.89.0 check --manifest-path
  apps/game-client/runtime/Cargo.toml`, `pnpm --dir apps/web exec tsc --noEmit
  --pretty false`, `node --check
  apps/web/scripts/capture-web-movement-jitter.mjs`, and local capture
  `docs/generated/player-qa/movement-jitter/local-bevy-atlas-chain-20260525.json`
  / `.png` passed with `ok=true`, Bevy entity renderer
  `{ready:true, enabled:true, entityCount:19, layerCount:21,
  atlasMode:"packed", atlasCount:1}`, scene assets `185/185`, no critical
  console errors, and no non-favicon 404s. This is not yet a production deploy
  or full all-asset/offline atlas rollout; next slice is live Chrome feel and
  broader atlas cache/perf tuning.
- 2026-05-25 production asset-domain CORS closeout: the live browser CORS
  error for
  `assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c/original-map/WemadeMir2/Objects/2136.png`
  was caused by a cached asset-domain response missing
  `Access-Control-Allow-Origin`, not by a missing PNG. The R2 asset-cache
  Worker now reapplies `access-control-allow-origin: *`,
  `access-control-allow-methods`, `access-control-allow-headers`, exposed
  headers, and `alt-svc: clear` to Cache API HIT responses as well as fresh R2
  responses. Worker version `ea9ec199-d3e4-4627-a57a-c677ddd426be` is live on
  `assets.mir2.obelisk.build/*`. Evidence:
  `docs/generated/remote-assets/cors-asset-worker-20260525.json` passed GET,
  cache-busted GET, HEAD, and OPTIONS probes from
  `Origin: https://mir2.obelisk.build`; the normal GET remains an
  `x-mir2-edge-cache=HIT` response and now includes
  `access-control-allow-origin: *`.
- 2026-05-25 map-object CORS/canvas hardening: scene map-object `<img>`
  elements now set `crossOrigin="anonymous"` before the existing black
  alpha-key canvas pass reads pixels. This complements the live asset-domain
  CORS Worker fix and prevents cross-origin object sprites from tainting the
  alpha-key canvas when assets are served from `assets.mir2.obelisk.build`.
  Evidence: the reported `Objects/2136.png` URL currently returns 200 with
  `access-control-allow-origin: *`, `access-control-allow-methods`, and exposed
  headers, and `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed.
- 2026-05-24 production movement command-log and hydration closeout: the
  frontend did send movement commands, but production console output did not
  show them because movement diagnostics were only retained in internal debug
  arrays. Player Web now emits `[mir2-move:send]`, `[mir2-move:ack]`, and
  correction logs when `?movementLog=1`, `?moveLog=1`,
  `?movementConsole=1`, `window.__mir2MovementLogEnabled`, or
  `localStorage["mir2-movement-log"]="1"` is enabled, and the movement harness
  captures those console events. The React #418 path was also mitigated by
  marking the app document `notranslate`/`suppressHydrationWarning` and making
  the original-client random overlay name deterministic across hydration.
  Production deployment `dpl_BommXyKsMcAX3Lmw4TYcg82a7Rsw` is live behind
  `https://mir2.obelisk.build` and now bakes
  `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=wss://165.154.65.136.sslip.io/ws`. Evidence:
  Web typecheck, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`,
  scoped diff checks, public `/health`, and production browser capture
  `docs/generated/player-qa/movement-jitter/prod-normal-directws-keyboard-d-20260524T1513.json`
  with `ok=true`, actual WebSocket `wss://165.154.65.136.sslip.io/ws`,
  six `walk Right` sends, six `UserLocation` ACKs, ACK frame latencies
  `555/522/516/523/517/517ms`, 12 movement console events, zero visual jumps,
  zero logical rollback, zero scene blackouts, clean settle, no critical
  console errors, and no non-favicon 404s. Screenshot evidence is the adjacent
  `.png`.
- 2026-05-24 production Chrome movement renderer closeout: a real Chrome tab
  against `https://mir2.obelisk.build` reproduced the user-visible failure as
  a browser renderer/main-thread runaway during held movement, not as a server
  rollback: the stuck tab reached a 400%+ renderer and 100% Chrome main-process
  CPU before restart. The frontend now caches the original-map region cell
  lookup and only rebuilds viewport map sprites on tile/scene-frame/map-region
  changes, so pixel-interpolated movement frames no longer rescan the full
  original-map region every RAF. Production deployment
  `dpl_FW2JQim28WxQTXsYahXjfFzv1Z7c` is live behind
  `https://mir2.obelisk.build`; `/health` returns 200. Verification passed
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, production
  Vercel build/deploy, real Chrome held-`D` movement from `323:264` to
  `327:264` without another unresponsive-page dialog, and production movement
  capture
  `docs/generated/player-qa/movement-jitter/prod-after-map-sprite-cache-d-hold-20260524T1433.json`
  with `ok=true`, zero visual jumps, zero route spam, zero logical rollback,
  zero direction lag, zero stale prediction warnings, responsive command queue,
  continuous camera offset, clean settle, no scene blackouts, no critical
  console errors, and no non-favicon 404s. Screenshot evidence is the adjacent
  `.png`.
- 2026-05-24 production Web movement rollback correction: Web self prediction no longer
  commits predicted coordinates into authoritative `world.entities`; prediction
  remains a render-only ActionFeed/local-anchor layer until server
  `UserLocation` confirms or corrects it. Prediction also waits for server ACK
  when the original map region is not loaded, the next step is outside the
  loaded region, or the loaded cell is blocked, so server-side collision
  rejections do not first draw the player onto an invalid tile. Evidence:
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, scoped
  `git diff --check`, and local movement smoke
  `docs/generated/player-qa/movement-jitter/local-left-walk-wait-map-20260523T233000.json`
  passed with `ok=true`, zero visual jumps, zero logical rollback, zero
  scene-layer blackouts, clean settle, no critical console errors, and no
  non-favicon 404s. Production deployment
  `dpl_3BwwKyjXY9UFZS3jSZk3vCsybCrW` is live through
  `https://mir2.obelisk.build`; production smoke
  `docs/generated/player-qa/movement-jitter/prod-left-walk-web-rollback-fix-20260524T0034.json`
  passed with the same movement assertions. Caveat: the final production
  sample had `sceneAssetReadiness.status=loading`, so this row is a movement
  rollback gate; resource readiness remains tracked by the dedicated resource
  smoke entries. After the matching Gateway release
  `20260524T0310Z-rollbackfix` was installed, production Web smoke
  `docs/generated/player-qa/movement-jitter/prod-left-walk-gateway-rollbackfix-20260524T0320.json`
  also passed with `ok=true`, zero visual jumps, zero logical rollback, zero
  scene blackouts, no critical console errors, and no non-favicon 404s.
- 2026-05-23 production Crystal action-queue closeout: the movement pipeline is
  now deployed on remote Gateway release `20260523T071900Z-actionqueue` and
  Player Web action-queue verification deployment `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`, with custom-domain `https://mir2.obelisk.build/health` passing. Web self movement is driven by local
  `QueuedAction`/ActionFeed state, treats self `UserLocation` as ACK/correction
  instead of a new animation source, renders two-tile Run in one Crystal 600ms
  action window, caps local ActionFeed lead to two tiles, and treats
  non-matching `UserLocation` as correction instead of a stale echo.
  Production walk evidence
  `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json`
  and run evidence
  `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`
  both report `ok=true`, zero visual jumps, zero logical tile rollback, zero
  scene-layer blackouts, responsive movement queue, clean settle, no critical
  console errors, and no non-favicon 404s. Screenshots are the adjacent `.png`
  files.
- 2026-05-23 Chrome manual movement/NPC-click follow-up: direct control of
  the live Chrome game tab confirmed that the player now loads and cycles real
  Crystal walk/run frames during manual movement. A clean right-click route
  around Bichon advanced from `327:271 -> 328:270 -> 330:268` and used
  `CArmour/00` walk frames `38-43` followed by run frames `86-91`; a left-click
  walk to `331:268` used walk frames `44-49`. No scene-layer blackouts were
  observed in the live tab. The sticky-feel issue reproduced around the Bichon
  NPC cluster was a frontend Crystal mismatch: `handleViewportTileAction`
  previously activated the nearest NPC when the target tile was merely near an
  NPC and the player was within interaction range. Crystal only suppresses
  movement when the actual clicked object is an NPC/player/special monster.
  That radius-based `nearbyNpc` shortcut has been removed, so empty ground near
  NPCs remains a movement target. Web typecheck passed after the fix:
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`. A production harness
  run, `docs/generated/player-qa/movement-jitter/prod-manual-click-run-open-20260523-analysis.json`,
  is clean for blackouts/404s/console errors but did not emit a player movement
  command, so the manual Chrome sample is the movement evidence for this item.
- 2026-05-23 production scene-blackout follow-up: the user-reported movement
  flicker where the main scene went black while HUD/minimap/chat stayed visible
  was caused by the `scene-assets-pending` CSS state hiding all primary scene
  layers (`game-scene-backdrop`, sprite overlay, entity overlay, and drop
  overlay) with `opacity: 0` while movement-triggered scene asset readiness was
  loading. The fix keeps the previous scene visible during pending asset checks
  and only disables the scene grid pointer target. The production deployment
  is `dpl_5J4k5qF8mAbnjoj79gGYw2ypZTNv`, visible through
  `https://mir2.obelisk.build`. Verification passed Web typecheck, movement
  harness syntax, scoped diff check, production `/health`, direct production
  probes for `NPC/83/1.png`, `Monster/010/3.png`, `Title/321.png`, and Bevy
  wasm, plus the production keyboard movement capture
  `docs/generated/player-qa/movement-jitter/prod-scene-blackout-normal-walk-20260523134030.json`.
  That capture reports `ok=true`, `noSceneLayerBlackouts.count=0`,
  no visual jumps, no route spam, no logical tile rollback, no direction lag,
  stale prediction cleared, command queue responsive, clean settle, no critical
  console errors, and no non-favicon 404s. Screenshot evidence is
  `docs/generated/player-qa/movement-jitter/prod-scene-blackout-normal-walk-20260523134030.png`.
- 2026-05-22 production movement/resource closeout: the live Gateway rollback
  cause was a shared-zone snapshot merge overwriting the Zone-authoritative
  player transform with the stale personal `SimulationSession` transform on the
  same map. The remote Gateway was rolled forward to
  `20260522T174413Z-zone-transform`, and Player Web was deployed through
  Vercel production deployment `dpl_BHimAGw5LRUVHUTFaWSUZsGhf2AH`. Frontend
  follow-up fixed self `UserInformation` class/gender sprite hydration, scaled
  movement animation lifetime by tile distance, preloaded the whole current
  entity action frame set, removed the stale scene-readiness ready/loading
  loop, and made transient sprite metadata failures retry instead of silently
  dropping CArmour/CHair body layers. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, script syntax
  checks, production `/health`, Gateway public `/health`, and direct production
  probes for `CArmour/00`, `CHair/00`, `NPC/83/1.png`, and
  `Monster/010/3.png` all passed. Final production keyboard movement capture
  `docs/generated/player-qa/movement-jitter/prod-zone-transform-sprite-retry-2m-20260522.json`
  reports `ok=true`, `noVisualJumps.count=0`, no route spam, no logical tile
  rollback, no direction lag, stale prediction cleared, command queue
  responsive, clean settle, no critical console errors, no non-favicon 404s,
  and 186/186 scene assets loaded. Screenshot evidence is
  `docs/generated/player-qa/movement-jitter/prod-zone-transform-sprite-retry-2m-20260522.png`;
  the self player, NPC bodies, and monster bodies are visibly rendered.
- 2026-05-22 production long-session movement/resource follow-up: the
  user-reported Chrome resource errors and non-smooth movement were retested
  against `https://mir2.obelisk.build` with real production login and keyboard
  movement. Landed fixes split WebSocket keepalive from QA-only `autoTick`,
  relaxed movement prediction waiting to exact occupied path tiles, held
  turn visuals for a full Crystal action frame, suppressed repeated held
  blocked-direction attempts, and deployed Cloudflare domain/R2 Workers with
  asset proxy response cleanup. Vercel production deployment
  `dpl_8NeUFDsKu2NKMTFuAf1yF9YEoxXV` is promoted current and `/health`
  returns 200. Evidence:
  `docs/generated/player-qa/movement-jitter/prod-movement-fix-15m-20260522.json`
  ran for 15 minutes with `packetRuntimeModes={"packetRefresh":3513}` and no
  reconnect samples, clean settle, no residual `predictedPlayer`,
  `movementPlan`, `directionStepPendingQueue`, or
  `outstandingSelfMovementActions`, 196/196 scene assets loaded, and no
  non-favicon 404s. It improved but did not close all movement-feel checks:
  visual jumps 156, logical tile rollback 41, route spam 5, direction lag 4,
  and console errors 31. All console errors were
  `net::ERR_QUIC_PROTOCOL_ERROR`; direct probes for affected URLs returned
  200, and `NPC/83/1.png` returned 200 through the R2-backed player-domain
  proxy. Cloudflare still injects `alt-svc: h3`; Worker/Next response headers
  cannot override that edge setting, and the current Wrangler OAuth token can
  deploy Workers but receives 403 for zone setting `http3`. Follow-up:
  disable Cloudflare HTTP/3/QUIC for `obelisk.build` with a zone-settings token
  or dashboard access, then rerun the production movement/resource capture.
- 2026-05-22 production movement rollback/smoothness pass: rapid opposite
  direction input no longer lets locally predicted `worldRef` position clear
  direction-step pending state as though it were a server acknowledgement. The
  settlement path now uses `lastSelfMovementAck` while packet-transport
  movement evidence is active, so old local prediction cannot prematurely clear
  pending movement or snap the self sprite across tiles. The production baseline
  `docs/generated/player-qa/movement-jitter/prod-movement-baseline-fresh-20260522-173704.json`
  reproduced the issue with `ok=false` and `noVisualJumps.count=1`. After the
  fix, Web `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed and
  production deployment `dpl_xryqwBF4NVPh7KdNio2ppv6EFPYh` is visible through
  `https://mir2.obelisk.build`. The production movement capture
  `docs/generated/player-qa/movement-jitter/prod-movement-fix-keyseq-20260522-180533.json`
  reports `ok=true`, `noVisualJumps.count=0`, no logical tile rollback, clean
  movement settle, stale prediction cleared, responsive movement command queue,
  running animation state present, no console errors, and no non-favicon 404s;
  screenshot evidence is
  `docs/generated/player-qa/movement-jitter/prod-movement-fix-keyseq-20260522-180533.png`.
- 2026-05-21 production map-monster screenshot pass: a production-safe QA
  screenshot surface now covers original map terrain plus Crystal respawn data
  without exposing debug player teleport commands. `/api/qa/map-monster-scenes`
  enumerates 807 representative scenes from the Crystal respawn manifest,
  covering 463 source maps, 284 maps with positive respawns, and 6340 positive
  respawn rows. `/qa/map-monsters` renders each scene through
  `loadCrystalSceneBlueprint`, using the loader-clamped `sceneView.center` for
  sprite placement and clamped labels for respawns whose source coordinates sit
  outside the renderable map bounds. The capture tool now accepts exact
  `--sceneIndexes` so production retakes can replace only failed scenes.
  Production deployment `dpl_9L3LsRnN8mfJmDirFCpjnrBdeNJR` is READY at
  `https://mir2-web3-7ov6lp1xs-obelisk-labs.vercel.app` and visible through
  `https://mir2.obelisk.build`. Evidence:
  `docs/generated/player-qa/production-map-monsters/production-full-map-monsters-qa807-resource-strict-20260521/summary.aggregate.json`
  reports `ok=true`, 807/807 final captured scenes, failure count 0,
  zero-map-sprite scenes 0, broken images 0, network 404s 0, network failures
  0, and console errors 0. The aggregate combines the full run, 38 scene
  retakes after the render-center fix, a focused GA1 retake, and 44 low
  concurrency retakes that removed high-concurrency resource load noise. It is
  the production resource-health gate rather than a guarantee that every
  heavy-map high-concurrency screenshot reached `imagesComplete=true`; for
  complete-pixel visual retakes the capture script now has QA-only
  `--fulfillOriginalMapFromPublic`, which still opens the production QA URL
  while fulfilling original-map requests from the restored local release. A
  focused `hyunwol1` retake with that mode verified `imagesComplete=true`,
  `pendingImageCount=0`, 588 rendered map images, and a nonblank terrain
  screenshot. The only durable missing asset found in that process was GA1
  `WemadeMir2/Objects10`
  frames; 27 frames `5172..5234` were exported from the full Crystal
  `Objects10.Lib`, uploaded to R2 prefix `mir2/v/37596e16d64fde7c` via
  `docs/generated/remote-assets/prod-ga1-objects10-patch-20260521/remote-asset-release.json`,
  direct CDN probes returned 200, and the GA1 retake finished with 99 map
  images, 0 pending, 0 broken images, 0 404s, and 0 console/network failures.
- 2026-05-21 production original-map runtime-data pass: representative live
  maps were rendering as flat fallback terrain because `/api/scene/crystal`
  could not read the full Crystal `Map/` and `Data/Map/*.Lib` source tree in
  Vercel, so it fell back to the packaged starter Bichon fragment or empty
  map regions. `crystal-map-loader.ts` now uses the local full-client root
  when available, and production falls back to generated compressed runtime
  map data under `lib/generated/crystal-map-pack` plus frame dimensions under
  `lib/generated/crystal-map-library-meta`. The scene blueprint cache schema
  was bumped so old fallback regions are not reused. Generated runtime data
  covers 1624 Crystal map files and 138 map libraries / 1,327,368 frame
  metadata entries, and the newly needed rendered PNG frames were uploaded to
  the active R2 release. Production deployment
  `dpl_CLp4KrpvspZaPHExjdjtazkRdFUs` is READY at
  `https://mir2-web3-5kzhyxrns-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. Evidence: `pnpm --dir apps/web exec tsc
  --noEmit --pretty false`, `node --check
  apps/web/scripts/generate-crystal-map-runtime-data.mjs`, and focused diff
  whitespace checks passed. Direct production probes returned non-empty
  regions for `0@271,259` (697 sprites / 4116 cells), `1@308,170`
  (132 / 2503), `D011@206,206` (231 / 5581), `D401@106,106`
  (397 / 5969), `D2042@156,56` (563 / 5169), and `D5063@39,15`
  (96 / 5070); previously missing sample images such as
  `/original-map/WemadeMir2/Tiles/1950.png` and
  `/original-map/WemadeMir2/Objects23/1966.png` returned 200. Playable
  Bichon screenshot evidence
  `docs/generated/player-qa/live-map-monsters/prod-map0-bichon-runtime-wait20-20260521Tnow.png`
  recorded `mapObjectSpriteCount=120` and `network404Count=0` where the prior
  live capture at the same coordinate had 0 map object sprites; the capture
  waits for the larger production map image set to settle. Production
  cross-map screenshot automation is intentionally blocked by the production
  player-command safety rule rejecting debug `crystal:<map>:<x>:<y>` transfer
  keys, so cross-map visual proof is API-level until a non-debug map routing
  or admin QA relocation path is available.
- 2026-05-21 original-ui metadata/exporter split pass: `/api/original-ui-meta`
  no longer imports `lib/original-ui-export-server.ts` in the production route.
  The route now reads already deployed/static `meta.json` from the app/player
  domain or configured R2/CDN base through
  `lib/original-ui-meta-server.ts`, and missing metadata returns
  `library_not_deployed` instead of doing request-time Crystal export. This
  removes the production build trace that scanned `public/original-ui`.
  Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`
  passed; Vercel production build emitted no
  `original-ui-export-server.ts` broad-pattern warning, leaving only the
  separate `crystal-map-loader.ts` / `public/original-map` warning. Production
  deployment `dpl_Fq8FkQb2JxjEmMAHwNXJCU4v7Xdi` is READY at
  `https://mir2-web3-ezaeeogvv-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`. Direct probes returned 200 for
  `/api/original-ui-meta?library=Items`, `/api/original-ui-meta?library=NPC/94`,
  representative R2-backed asset paths, debug samples, and Bevy wasm; invalid
  `Map/foo` returned `unsupported_library`. Production cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-prod-20260521.json`
  passed with `ok=true`, 387/387 prewarm ok, warm transfer 0 bytes, and no
  non-favicon 404s. Playable production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-meta-reader-split-playable-prod-20260521.json`
  passed with `ok=true`, cold first playable 13745.3ms, warm first playable
  14118.8ms, 387/387 prewarm ok, and no non-favicon 404s.
- 2026-05-21 CDN-first Vercel output pass: Player Web production deployment
  now keeps the Vercel artifact focused on the Next.js shell, route handlers,
  retained debug samples, and same-origin Bevy runtime, while Crystal
  `/original-ui`, `/original-map`, and `/generated/original-map-blend` media
  are served from the verified R2/CDN release through the player domain.
  `apps/web/scripts/prune-vercel-output-assets.mjs` prunes only those
  R2-backed paths from `.vercel/output` after `vercel build`; the final report
  `docs/generated/remote-assets/vercel-output-prune-resource-cdn-first-20260521.json`
  reduced output size from 420,957,251 bytes / 18,650 files to 43,478,680 bytes
  / 278 files. Production deployment `dpl_ieQqdaZMnnZYNe4wxksuoqsj7Sgg` is
  READY at `https://mir2-web3-js3ofmmod-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`; upload size was 15.7MB. Direct probes returned
  200 for R2-backed title/item/map/blend assets, retained
  `/debug/map-samples/smtile-72.png` and `smtile-80.png`, and same-origin Bevy
  wasm. Evidence: `node --check
  apps/web/scripts/prune-vercel-output-assets.mjs`,
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, production
  cache-maintenance smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-final-prod-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm transfer 900 bytes, no critical
  console errors, and no non-favicon 404s, plus playable production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-cdn-first-playable-final-prod-20260521.json`
  with `ok=true`, cold first playable 14212.5ms, warm first playable 14163.9ms,
  warm transfer 600 bytes, and no non-favicon 404s.
- 2026-05-21 resource cache-tier production pass: Player Web resource
  management now separates declared static asset packs into Service Worker
  cache tiers instead of one bulk static cache. `/api/asset-manifest` exposes
  per-tier budgets (`staticCriticalMaxEntries=3000`,
  `staticBackgroundMaxEntries=6000`, `staticRuntimeMaxEntries=16000`);
  `login`, `character-select`, and `hud-core` are tagged
  `cacheTier=critical`; `bichon-spawn` is tagged `cacheTier=background`;
  scene-frame prewarm sends best-effort tier hints so dynamic Bichon frames
  populate `mir2-asset-cache-static-background-*` while login/select/HUD stay
  in `mir2-asset-cache-static-critical-*`. Evidence: Web
  `node --check apps/web/public/mir2-asset-worker.js`,
  `node --check apps/web/scripts/smoke-cache-metrics.mjs`,
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`, and
  `pnpm --dir apps/web run build` passed. Local production Web
  `127.0.0.1:13021` passed
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-local-20260521.json`
  with `ok=true`, 387/387 prewarm ok, warm CacheStorage 3 caches / 383
  entries / 63.9MB, no critical console errors, and no non-favicon 404s.
  Production deployment `dpl_9qZP7jXVU1Q6BzUWZVyQKKkMgiaf` is READY at
  `https://mir2-web3-aefb2e729-obelisk-labs.vercel.app`, aliased to
  `https://mir2-web3-web.vercel.app`, and visible through
  `https://mir2.obelisk.build`; production manifest version
  `5d1ec8e93c1caa62` reports the new tier budgets and cache tags.
  Production smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-resource-tier-prod-20260521.json`
  passed with `ok=true`, 387/387 prewarm ok, warm CacheStorage 3 caches / 383
  entries / 51.1MB, after-cleanup caches
  `static-critical`, `static-background`, `scene`, and `api`, reset deleted 4
  caches/unregistered 1 scope, and all cache-budget, console, and network
  assertions true.
- 2026-05-21 production map-change entity cleanup: Player Web now treats
  `MapInformation` with a different map file as a hard scene boundary and
  clears old non-self entities, drops, projectiles, terrain/decor, selection,
  and active NPC dialog before applying the new map's object packets. This
  closes the live QA state where switching a production test account from
  Bichon to `WoomyonWoods(S)` could show the correct forest map and original
  forest monsters while stale Bichon `Royal_Guard` / `Royal_Archer` rows
  remained in the packet-first entity table. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed.
- 2026-05-21 scene backdrop edge/fallback pass: the Player Web main scene no
  longer disables its terrain fallback just because some original floor sprites
  are present. `GameSceneBackdrop` now always draws the synthetic terrain tile
  underlay behind original Crystal floor sprites, scene/UI image elements retry
  failed loads with a cache-busted same-origin URL and then the manifest remote
  asset base when available, visible scene asset preloading follows the same
  retry candidates, and scene blueprint requests prefetch a wider margin so
  chunk edges are refreshed before the player reaches the loaded play bounds.
  Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed, and
  local Web `127.0.0.1:13017` against live `wss://mir2.obelisk.build/ws`
  passed
  `docs/generated/player-qa/movement-jitter/map-scene-fallback-ui-retry-final-20260521.json`
  with `ok=true`, `sceneAssetReadiness=127/127`, no visual jumps/rollback,
  no route spam, no non-favicon 404s, and no critical console errors;
  screenshot:
  `docs/generated/player-qa/movement-jitter/map-scene-fallback-ui-retry-final-20260521.png`.
- 2026-05-21 shared NPC/monster sprite retention fix: Bichon NPC nameplates and
  minimap dots could appear while several NPC bodies were missing because a
  later shared-Zone packet refresh could lose the retained sprite image and
  re-emit `ObjectNpc` / `ObjectMonster` with `image=0`; the Web client then
  accepted that placeholder packet and replaced the correct `NPC/<image>` or
  `Monster/<image>` sprite from the world snapshot. Gateway now stores simple
  Crystal sprite snapshots when converting `ObjectNpc` / `ObjectMonster` into
  shared entities, preserves an existing sprite when merging later shared
  packets, and serializes the retained image back into shared spawn packets.
  Player Web now keeps an existing NPC/monster sprite when a packet lacks a
  sprite or carries a conflicting `image=0`, and it also falls back to the
  app-local Crystal actor sprite manifest when a live `worldSnapshot` arrives
  with NPC/monster `sprite=null`. The movement diagnostic script records compact
  entity sprite state plus rendered `.entity-sprite-stack` image load details
  for future live NPC visual checks. Evidence: focused Gateway shared object
  sprite regressions passed for `shared_zone_state_records_object_*`; Web
  typecheck and movement diagnostic syntax checks passed; local Web
  `127.0.0.1:13016` against live `wss://mir2.obelisk.build/ws` passed
  `docs/generated/player-qa/movement-jitter/npc-sprite-retention-local-live-gateway-20260521-freshdev.json`
  with 22 NPC/monster actors, `missingSprites=0`, 18 rendered sprite stacks,
  `emptyRendered=0`, no non-favicon 404s, and no critical console errors.
- 2026-05-20 production Items R2 completeness repair: Production image 404s
  for high item-icon frames such as `/original-ui/Items/2723.png` through
  `/original-ui/Items/2732.png` were traced to the live R2 release having only
  the curated `Items` export while the full Crystal `Items.Lib` has 5380
  frames. `apps/web/scripts/export-crystal-ui.mjs` now supports exporting a
  selected full library into a temporary staging root; `Items` was exported from
  `downloads/crystal-client-full/Data/Items.Lib` into `/tmp/mir2-original-ui-full`
  and uploaded to R2 prefix `mir2/v/37596e16d64fde7c` through the authenticated
  bulk upload Worker route `assets.mir2.obelisk.build/upload*`. The Cloudflare
  player-domain proxy now resolves `/api/original-ui-meta?library=...` from R2
  static `/original-ui/<library>/meta.json` when present, then falls back to
  Vercel. Evidence: production probes through the user's proxy returned 200 for
  `Items/2723-2732.png`, `Items/983.png`, `Items/984.png`, `Prguse/983.png`,
  `Monster/010/10,16,17,18,19.png`, `Monster/012/17,19.png`,
  `CArmour/00/832-835.png`, `CHair/00/16,18,19,832-835.png`,
  `CWeapon/00/17.png`, `NPC/05/11.png`, `Title/320.png`, `Title/321.png`,
  `Sound/Login2.wav`, `/api/asset-manifest`, `/api/scene/crystal?...`, and the
  Bevy wasm. Static and API `Items` metadata both report 5380 frames with
  frames 2723, 2730, and 5379 present. Production cache smoke
  `/tmp/mir2-cache-verify/items-full-r2-verify-120s.json` passed with
  `ok=true`, cold/warm `prewarmOk=387`, `prewarmFailed=0`, warm CacheStorage
  383 entries / 51.1MB, `noCriticalConsoleErrors=true`, and
  `noNonFavicon404s=true`.
- 2026-05-20 production browser-network follow-up: After the Items repair,
  Chrome/Browser runtime evidence was refreshed through login and character
  select with the default `demo/demo` account. The page asset observer recorded
  416 URLs across login, select, HUD, Bichon scene prewarm, wasm, audio, map
  tiles, and UI sprites; direct status probing found no durable Mir2 asset 404s.
  User-reported `/original-ui/ChrSel/12.png` and
  `/original-ui/Monster/010/3.png` both return 200 from the player domain, and
  the transient `Prguse/1932.png` fetch failure also returns 200 on direct
  recheck. All local `original-ui/**/meta.json` files plus generated original
  UI manifests were uploaded to the active R2 prefix so static metadata requests
  such as `/original-ui/ChrSel/meta.json` now return 200 without waiting for the
  Vercel API fallback. Production cache smoke
  `/tmp/mir2-cache-verify/chrome-network-post-rum-120s.json` passed with
  `ok=true`, `prewarmOk=387`, `prewarmFailed=0`, warm CacheStorage 383 entries
  / 51.1MB, `noCriticalConsoleErrors=true`, and `noNonFavicon404s=true`.
  `/cdn-cgi/rum?` noise is Cloudflare Analytics/RUM on a reserved Cloudflare
  path, not a Mir2 game asset route; the Mir2 asset Service Worker now responds
  to same-origin `/cdn-cgi/rum` with `204 no-store` once the player page is
  controlled by the SW so repeated DevTools sessions do not keep surfacing it as
  a missing game resource.
- 2026-05-22 live Chrome resource-error retry follow-up: The user's connected
  Chrome tab still showed preserved red `Img` rows after the production asset
  repairs, but direct player-domain probes for the current broken DOM URLs all
  returned `200 image/png`, including `generated/original-map-blend` torch
  blends, `CArmour/00/8.png`, `CHair/00/8.png`, `CWeapon/00/8.png`,
  `Monster/000/*`, `Monster/139/*`, `NPC/05/9.png`, `NPC/08/1.png`,
  `NPC/16/1.png`, `NPC/27/1.png`, `NPC/45/1.png`, `NPC/83/1.png`,
  `Prguse/983.png`, `Prguse/2044.png`, plus the user-reported
  `ChrSel/12.png` and `Monster/010/3.png`. Root cause for the remaining bad
  image state is transient first-load failure without a later retry, not durable
  missing PNGs. Scene/UI image error handling now keeps the existing same-origin
  and remote CDN fallback, schedules cache-busted delayed retries at
  0.5s/1.5s/3.5s/7s/12s after `onError`, and runs a game-scene stalled-image
  rescue pass every 1.5s so pending images that never fire `onError` are also
  cache-busted and retried. Deployment `dpl_4Uw447PEm7Y656TYHXNnHeVvnzi5` is
  READY at `https://mir2-web3-1ie43fh9n-obelisk-labs.vercel.app` and aliased
  through `https://mir2-web3-web.vercel.app` / `https://mir2.obelisk.build`.
  Verification: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`
  passed; production build/prune reduced `.vercel/output` from
  624,756,385 bytes / 80,196 files to 43,888,088 bytes / 283 files; production
  URL probes returned 200 for all sampled current broken URLs; the live
  `mir2.obelisk.build` bundle contains both `mir2DelayedRetryCount` and
  `mir2StalledRetryCount` retry markers; and after refreshing the user's
  connected Chrome game tab, the live DOM reported `brokenCount=0` for Mir2
  `/original-ui` and `/generated/original-map-blend` images.
- 2026-05-20 SoundList fallback closure: The Crystal source tree still does not
  contain exact upstream files for SoundList entries `10022 -> 22.wav`,
  `10109 -> 109.wav`, and `705 -> ZombieRevive.wav`, but the Web asset exporter
  now publishes explicit, audited fallback WAVs under the expected original
  paths so every SoundList id resolves to a playable URL. `22.wav` is copied
  from adjacent movement clip `23.wav`, `109.wav` from adjacent struck clip
  `110.wav`, and `ZombieRevive.wav` from nearby undead BoneFamiliar clip
  `64.wav`; each entry is marked with `fallback=true`,
  `exactSourceExists=false`, and a `fallbackReason` in
  `sound-index.generated.json`. Evidence: `node
  apps/web/scripts/smoke-crystal-assets.mjs` reports `exportedSoundCount=450`,
  `missingSoundCount=0`, and `failures=[]`; production player-domain probes for
  `/original-ui/Sound/22.wav`, `/original-ui/Sound/109.wav`,
  `/original-ui/Sound/ZombieRevive.wav`, and
  `/original-ui/sound-index.generated.json` all return 200 from the active R2
  prefix. Production deployment `dpl_F77Spi5brjxcRJqS6cbMqA7cChcm` is READY
  and aliased to `mir2-web3-web.vercel.app`; the Cloudflare player-domain proxy
  version `22639255-5371-4926-88b4-92fc02919ea8` appends `no-transform` to HTML
  responses, and the final production resource smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-sound-fallback-prod-final-20260520.json`
  passed with `ok=true`, `prewarmOk=387`, `prewarmFailed=0`,
  `noCriticalConsoleErrors=true`, and `noNonFavicon404s=true`.
- 2026-05-19 prewarm-latency and scene-object pruning pass: Player Web now
  treats cache prewarm as two phases. Login/select/HUD packs stay critical, but
  the Bichon scene pack is background-only, waits until the first playable frame
  plus a 20s idle window by default, and caps its sampled scene sprite frames at
  180. `/api/asset-manifest` includes the asset-cache-pack definition in the
  manifest hash, so phase/frame-cap changes rotate the runtime cache version.
  The first visible scene loader also prioritizes sprite URLs by distance from
  the current scene center, and original-map object sprites are mounted only
  when their rendered pixel bounds intersect the visible viewport margin. This
  reduces the focused Bichon first-scene visible asset set from the earlier
  217/218 range to 112/112 without changing packet-driven movement authority.
  Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`,
  `node --check apps/web/scripts/smoke-cache-metrics.mjs`,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, direct
  `pnpm --dir apps/web run build`, and targeted `git diff --check` passed.
  Local production Web `127.0.0.1:13015` against live
  `wss://mir2.obelisk.build/ws` passed playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-viewport-pruned-delay20-cache-local-20260519.json`
  with `ok=true`, cold first playable 11976.3ms, warm first playable 6022.1ms,
  387/387 prewarm ok, warm CacheStorage 439 entries / 65.9MB, no critical
  console errors, and no non-favicon 404s. The matching movement diagnostic
  `docs/generated/player-qa/movement-jitter/viewport-pruned-existing-settle9-local-20260519.json`
  has `ok=true`, 112/112 scene assets loaded, packet runtime
  `{"packetRefresh":58}`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no critical console errors,
  and no non-favicon 404s. Raw aborted image requests from viewport pruning are
  retained in the report as ignored non-critical `net::ERR_FAILED` entries.
  Production deployment `dpl_4YwqgqQdhA1HQQwPhFrA1KoTCpXP` is READY and aliased
  to `mir2-web3-web.vercel.app`; the Cloudflare domain manifest reports version
  `ecb5ff44ad1ad66b`, the matching `asset-cache-packs` hash, and
  `bichon-spawn` as background. Production playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-viewport-pruned-delay20-cache-existing-20260519-221410.json`
  passed with `ok=true`, cold first playable 11673.5ms, warm first playable
  13549.9ms, 387/387 prewarm ok, warm CacheStorage 437 entries / 54.5MB, no
  critical console errors, and no non-favicon 404s. Production movement
  diagnostic
  `docs/generated/player-qa/movement-jitter/prod-viewport-pruned-existing-settle9-20260519-221630.json`
  passed with `ok=true`, 124/124 scene assets loaded, packet runtime
  `{"packetRefresh":58}`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no critical console errors,
  and no non-favicon 404s.
- 2026-05-19 mobile landscape controls pass: Player Web now has a first-class
  phone landscape input layer instead of relying on desktop mouse/keyboard only.
  `nipplejs` is used only as the analog joystick sensor; a Web-owned Mir2
  semantic adapter converts stick vectors into Crystal 8-way `walk` / `run`
  direction intents and sends them through the existing packet runtime/Zone
  input path. The mobile layer keeps only the latest joystick intent, gates
  sends while the packet movement runtime still has pending prediction or
  correction state, de-duplicates repeated same-direction `Turn` packets when a
  tile is blocked, and adds a mobile-MMO style right-bottom circular action
  wheel for Run, Attack, approach, pickup, inventory, character, belt items, and
  known skills. Landscape CSS exposes the controls with a scaled Crystal stage;
  portrait shows a rotate prompt. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false` passed,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs` passed, and
  the live mobile viewport smoke
  `mobile-controls-joystick-longhold3-20260519` passed with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no console errors, and no
  non-favicon 404s. Screenshot/report:
  `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.png`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.json`.
  The circular right-bottom wheel layout was then verified on
  `mobile-controls-wheel-short-20260519` with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no stale
  prediction, no command queue warnings, no console errors, and no non-favicon
  404s; screenshot/report:
  `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.png`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.json`.
- 2026-05-19 scene-asset-ready movement-feel gate: Player Web now treats the
  first visible game scene as part of first-playable readiness. The client
  preloads the currently visible map/entity sprite URLs, hides the scene layers
  while that first scene is pending, blocks keyboard, mouse, and mobile movement
  input until `sceneInteractionReady=true`, and records `sceneAssetsStart`,
  `sceneAssetsReady`, and deferred-input milestones in cache diagnostics.
  Movement diagnostics now waits for `sceneInteractionReady` before sending
  input and supports `--skipStartTransfer` for production Gateway routes that
  correctly reject debug teleport. Evidence: Web
  `pnpm --dir apps/web exec tsc --noEmit --pretty false`,
  `node --check apps/web/scripts/capture-web-movement-jitter.mjs`,
  `node --check apps/web/scripts/smoke-cache-metrics.mjs`, and local Web
  `127.0.0.1:13015` against live `wss://mir2.obelisk.build/ws` passed
  keyboard-sequence movement with `ok=true`, `sceneInteractionReady=true`,
  218/218 visible scene assets loaded, `packetRuntimeModes={"packetRefresh":49}`,
  no visual jumps, no logical rollback, no route spam, no console errors, and
  no non-favicon 404s. Report:
  `docs/generated/player-qa/movement-jitter/scene-ready-local-skip-transfer-20260519.json`.
  Production rerun on `https://mir2.obelisk.build` also passed after allowing
  the production prewarm queue to settle: movement report
  `docs/generated/player-qa/movement-jitter/prod-scene-ready-prewarm-wait-20260519.json`
  has `ok=true`, 217/217 visible scene assets loaded, packet runtime
  `{"packetRefresh":59}`, no visual jumps/rollback/route spam, no console
  errors, and no non-favicon 404s. The matching production playable cache smoke
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-scene-ready-20260519-1815.json`
  has `ok=true`, cold first playable 11384.1ms, warm first playable 17644.4ms,
  527/527 prewarm ok, warm CacheStorage 570 entries / 54.9MB, no critical
  console errors, and no non-favicon 404s.
- 2026-05-19 production R2 custom asset domain and edge cache: The verified R2
  release now uses `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c`
  as the public asset base, with `infra/cloudflare/mir2-r2-asset-cache`
  deployed on `assets.mir2.obelisk.build/*` in front of bucket
  `mir2-web3-assets`. Production `/api/asset-manifest` returns
  `remoteAssets.assetBaseUrl="https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c"`
  and `remoteAssets.objectPrefix="mir2/v/37596e16d64fde7c"`. Repeated public
  GET probes for scene sprite frames return `x-mir2-edge-cache: HIT` and
  `cf-cache-status: HIT`, so repeat asset requests are served from Cloudflare
  edge cache instead of repeatedly fetching from R2. `/bevy-runtime/...` now
  stays same-origin with short cache headers and a build-version query on the
  JS/WASM pair, preventing R2 release prefixes from serving stale runtime files.
  Evidence: production playable smoke
  `codex-r2-assets-domain-prod-smoke-final` passed with `ok=true`, cold first
  playable 3563.4ms, warm first playable 3775.9ms, 517/517 prewarm ok, no
  critical console errors, and no non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-codex-r2-assets-domain-prod-smoke-final.json`.
- 2026-05-19 production R2 scene sprite closure: The live R2 release at
  `mir2/v/37596e16d64fde7c` now includes the generated `/original-ui` actor,
  NPC, and Monster scene sprite roots that live gameplay requests after first
  render. The published manifest reports 7,319 asset files, 6,807 scene sprite
  files, and 0 missing files; public probes for `Monster/003/52.png`,
  `Monster/003/57.png`, `NPC/03/0.png`, `CArmour/00/12.png`,
  `AWeapon/00%20L/12.png`, and `ARWeapon/00%20S/12.png` returned 200 with
  immutable cache headers. Production playable smoke on
  `https://mir2.obelisk.build` passed with `ok=true`, cold first playable
  4296.3ms, warm first playable 4049.6ms, 517/517 prewarm ok, 0 prewarm
  failures, no critical console errors, and no non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-codex-r2-actor-sprites-prod-smoke.json`.
- 2026-05-21 Sui wallet picker / Dubhe Wallet login pass: Player Web no longer
  auto-selects the first Sui wallet returned by Wallet Standard. The login
  dialog's `Wallet` action now opens a compact Sui wallet picker, lists all
  detected wallets that support `sui:signPersonalMessage`, prioritizes wallets
  whose id/name match Dubhe, and passes the selected wallet id into the existing
  Sui personal-message login token flow. If Dubhe Wallet is not registered in
  the browser, the picker keeps a direct `Dubhe Wallet` entry to
  `https://dubhe.obelisk.build/en/wallet`; when a Wallet Standard Dubhe wallet
  is registered, the picker shows it as the selectable wallet and hides the
  external entry. Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`
  passed; local Chrome/CDP smoke on `http://127.0.0.1:13010` verified the picker
  is visible, `aria-expanded=true`, the Dubhe link is present with no wallet
  installed, no critical console errors, and no overlap with the original login
  buttons. A second CDP smoke injected a standards-shaped `Dubhe Wallet` and
  verified it appeared as a selectable Dubhe-prioritized wallet with the install
  link hidden. Evidence files:
  `docs/generated/player-qa/wallet-picker/dubhe-wallet-picker-login.json`,
  `docs/generated/player-qa/wallet-picker/dubhe-wallet-picker-login.png`, and
  `docs/generated/player-qa/wallet-picker/dubhe-wallet-picker-registered.json`.
- 2026-05-18 production Vercel/Cloudflare playable smoke: `https://mir2.obelisk.build`
  now serves Player Web from Vercel project `obelisk-labs/mir2-web3-web` while
  routing `/ws` to the UCloud Gateway and `/original-map/*` to the R2 release
  prefix `mir2/v/37596e16d64fde7c`. Original scene sprite metadata now prefers
  deployed static `/original-ui/.../meta.json` for libraries already included in
  the Vercel static output, avoiding request-time `/api/original-ui-meta`
  exports on Vercel for `CHair/00`, `CWeapon/00`, `Monster/010`, `NPC/05`,
  `CArmour/00`, and `Monster/012`. Evidence:
  `npx tsc --noEmit --pretty false` passed in `apps/web`; targeted
  `git diff --check` passed; direct production probes returned 200 for those
  metadata APIs and static `original-ui`, R2 `original-map`, and Vercel blend
  assets; `npm run smoke:playable-metrics -- --baseUrl https://mir2.obelisk.build --runId prod-mir2-obelisk-final-002458 --waitTimeoutMs 300000`
  passed with `ok=true`, cold first playable 4612.5ms, warm first playable
  4684.3ms, 517/517 prewarm ok, 0 prewarm failures, no critical console errors,
  and no non-favicon 404s. Report:
  `docs/generated/player-qa/cache-metrics/cache-metrics-prod-mir2-obelisk-final-002458.json`.
- 2026-05-18 ranking-system UI pass: Player Web now opens a real Crystal-style `Ranking` social panel from the in-game System Menu instead of static placeholder rows. The panel requests Gateway `getRanking`, supports Overall, class tabs, Online, manual Refresh, selected-row details, and My Rank display, then renders typed `Rankings` payload data from the server. Evidence: Web `npx tsc --noEmit --pretty false` passed; Rust Simulation/Gateway fmt/check and focused ranking/Gateway tests passed; live Browser smoke on `http://127.0.0.1:13012/?gatewayWs=ws://127.0.0.1:7222/ws&movementDiag=1&codexBust=ranking-smoke-20260518` logged into Scout, opened Menu -> Ranking, verified Overall and Online tabs showing `Scout #1`, `Level 7 Warrior`, and `My rank: 1 / 1`; screenshot and state evidence are `docs/generated/player-qa/ranking-system/ranking-panel.png` and `docs/generated/player-qa/ranking-system/ranking-smoke.json`.
- 2026-05-18 character creation class picker pass: Player Web `NEW` now opens an original-select-screen creation panel instead of creating a hidden random male Warrior. The panel supports name entry, male/female gender selection, all five Crystal classes (`Warrior`, `Wizard`, `Taoist`, `Assassin`, `Archer`), localized Chinese labels, class icon buttons, and live select-portrait preview; `NewCharacter` now sends the selected class/gender/name and selects the newly created visible slot on success. Evidence: Web `npx tsc --noEmit --pretty false` passed in the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; targeted `git diff --check` passed; Browser smoke on `http://127.0.0.1:13010/?gatewayWs=ws://127.0.0.1:7210/ws&movementDiag=1` logged into `demo/demo`, opened the localized create panel, created a female Archer visible as `QAPAPAGKA 1 弓箭手`, then cleaned that demo QA character; protocol smoke wrote `docs/generated/player-qa/create-character-classes-20260518/class-protocol-smoke.json` with `ok=true` after creating all five classes across two temporary accounts, and Browser evidence lives in `docs/generated/player-qa/create-character-classes-20260518/browser-summary.json` plus screenshots.
- 2026-05-18 Web Packet Runtime movement pass: Player Web now treats Crystal typed packets as the live in-game state source after bootstrap/reconnect/map-change/scene bootstrap. Normal `worldSnapshot` refreshes enter `packetRefresh` mode and merge only durable metadata into the packet-owned entity/drop tables, so stale snapshot rows cannot overwrite current `UserLocation`, `ObjectWalk/ObjectRun`, `ObjectNpc/ObjectMonster`, `ObjectRemove`, or live ground-drop packet state. Removed objects are tombstoned for the refresh window to prevent snapshot reinsert. The movement harness now records `worldSnapshotRealtimeMode` and `packetRuntime` mode counts, and its center-sprite jump check ignores expected residual map-scroll offset when Crystal movement direction changes. The missing `NPC/94` source-library path is also closed by making `/api/original-ui-meta` trigger the existing on-demand library export path. Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, `curl http://127.0.0.1:13014/api/original-ui-meta?library=NPC%2F94`, `docs/generated/player-qa/movement-jitter/r-web-packet-runtime-keyseq-20260518b.json`, and `docs/generated/player-qa/movement-jitter/r-web-packet-runtime-holdspam-20260518d.json` all passed with `ok=true`, `packetRuntimeModes={"packetRefresh":...}`, no visual jumps, no logical tile rollback, no route spam, responsive movement queue, clean settle, no console errors, and no non-favicon 404s.
- 2026-05-18 Bichon click-route air-wall pass: Player Web target movement now uses a bounded local route search when the direct Crystal step is blocked by visible static map cells, visible live objects, or recent server correction memory. This keeps keyboard/directional movement unchanged while allowing right-click target movement to route around building/bridge/tree edges instead of stopping at the first non-monotonic detour. If the clicked target tile itself is blocked, the route settles on the nearest reachable tile toward the target rather than leaving a stale pending plan. Evidence: Web `npx tsc --noEmit --pretty false` passed in the main checkout and in the currently served `/private/tmp/mir2-main-human` web directory; targeted `git diff --check` passed; 13010 was restarted; Browser smoke on `http://localhost:13010/?gatewayWs=ws://127.0.0.1:7210/ws&movementDiag=1` logged into `demo/demo`, clicked through the Bichon shop/bridge area, and wrote `docs/generated/player-qa/airwall-route-20260518/airwall-route-summary.json` plus `airwall-route-after.png` with `consoleWarningsAndErrors=[]`.
- 2026-05-18 reconnect/resume grace pass: Player Web now has a committed `npm run smoke:reconnect-resume` harness for the unexpected in-game Gateway WebSocket close path, and Gateway keeps active sessions under a short reconnect grace lease instead of immediately dropping Zone presence on socket loss. The client still keeps the active auth/character slot snapshot, shows compact in-stage reconnect status, retries with bounded backoff, and replays `clientVersion`/`login`/`startGame` or a still-valid Sui token login; the backend now retains the active `GatewaySession` for `MIR2_GATEWAY_RECONNECT_GRACE_SECONDS` (default 15s, clamped 1-120s) and restores it on the next authenticated `StartGame` for the same account/character. Manual reset/logout and server `Disconnect` packets still clear reconnect state instead of looping. Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/scripts/smoke-reconnect-resume.mjs`, Gateway `cargo +1.89.0 fmt --check -p mir2-gateway`, focused reconnect store tests 2/2, production Web path tests 3/3, and route-lease stale-owner regression passed. Live CDP smoke on `http://127.0.0.1:13011/?gatewayWs=ws://127.0.0.1:7211/ws` wrote `docs/generated/player-qa/reconnect/reconnect-resume-codex-reconnect-grace-smoke-final.json` with `ok=true`: it entered `demo/demo`, invoked `window.__mir2Stage5.closeGatewayForReconnectSmoke()`, observed `reconnectStatus={mode:"scheduled",attempt:1}` and `Connection lost. Reconnecting in 1s.`, then returned to `screen=game`, `wsState=open`, `reconnectStatus=idle`, same map `0`, and player `{x:336,y:249}`. Gateway logs for that run showed the grace path retained and restored session `demo/0`; the smoke records existing optional `NPC/94` original-ui meta 404s as allowed non-reconnect asset noise, with no unexpected 404s or critical console errors.
- 2026-05-18 original audio settings pass: Player Web now has persisted Music and Effects toggles backed by `mir2.originalAudioSettings`. Login and character-select screens expose compact top-right controls, and the in-game chat Settings panel exposes the same audio controls alongside channel/transparency settings. The shared audio manager now suppresses/resumes looping login/select music from the stored Music flag and suppresses Crystal `PlaySound`/button effects from the stored Effects flag. Evidence: Web `npx tsc --noEmit --pretty false` passed, targeted `git diff --check` passed, and Playwright smoke on `http://127.0.0.1:13013/?skipRuntime=1` verified the login `Audio` region, toggled `Music` and `Effects` to `Off`, confirmed `localStorage` persisted `{"musicEnabled":false,"effectsEnabled":false}`, captured a browser screenshot, and verified Simplified Chinese labels render as `声音` / `音乐` / `音效`. The only browser console error observed was the expected WebSocket refusal after an intentional `Quick Enter` attempt without a gateway running, not from the audio settings path.
- 2026-05-18 game-grade asset cache foundation: Player Web now has production immutable HTTP caching for `/original-ui`, `/original-map`, and `/bevy-runtime`; a versioned `/api/asset-manifest`; opt-in-dev/auto-production `mir2-asset-worker.js` runtime caching for static game assets, scene blueprints, and metadata; and a server-side `.next/cache/mir2-scene-blueprints` memory/disk cache for `GET /api/scene/crystal`. Evidence: Web `npx tsc --noEmit --pretty false`, `node --check apps/web/public/mir2-asset-worker.js`, `git diff --check` on changed cache files, and direct `npx next build` all passed; dev server `http://127.0.0.1:13011` scene probe for `map=0&x=420&y=257&width=32&height=28` returned first-run `X-Mir2-Scene-Cache=miss` in 124ms and second-run `hit` in 10ms; production `next start` on `http://127.0.0.1:13012` served `/original-ui/Prguse/4.png` with `Cache-Control: public, max-age=31536000, immutable`, served `/api/asset-manifest` with `max-age=60, stale-while-revalidate=300`, served `/mir2-asset-worker.js` with `Service-Worker-Allowed: /`, and returned production scene `Cache-Control: public, max-age=300, stale-while-revalidate=3600` with cache hits at 42ms/9ms. Browser smoke on `http://127.0.0.1:13011/?assetCache=1&skipRuntime=1` loaded the login shell with `critical console errors=[]`; the Browser plugin's read-only page context did not expose Cache Storage internals, so SW internals were verified by route/header/build behavior rather than direct Cache API enumeration.
- 2026-05-18 cache metrics and critical prewarm: Player Web now exposes QA-only cache observability through `?cacheDebug=1` and `window.__mir2CacheMetrics`, with resource timing, transfer/encoded bytes, cache-like resource counts, scene cache hit/miss counts, slowest resource samples, and prewarm status. `/api/asset-manifest` now includes critical prewarm packs for `login`, `character-select`, `hud-core`, and `bichon-spawn`; the Bichon pack fetches the scene blueprint and first visible scene sprite frames instead of preloading the full Crystal source tree. Evidence: `curl http://127.0.0.1:13011/api/asset-manifest` returned 4 resource packs with 40 login URLs, 41 character-select URLs, 108 HUD URLs, and 1 Bichon scene prewarm; Browser smoke on `http://127.0.0.1:13011/?assetCache=1&cacheDebug=1&skipRuntime=1` showed the `Mir2 Cache Debug` panel, `SW: registered`, scene cache hits, and final `Prewarm: 511/511 ok, 0 failed`, with console `errors=[]`; Web `npx tsc --noEmit --pretty false` and targeted `git diff --check` passed after the metrics/prewarm implementation.
- 2026-05-18 cache metrics cold/warm smoke: Web now has repeatable `npm run smoke:cache-metrics`, which launches a fresh Chrome profile, enables `assetCache/cacheDebug/prewarm`, waits for `window.__mir2CacheMetrics` prewarm completion, runs a second warm pass in the same profile, and writes `docs/generated/player-qa/cache-metrics/latest-cache-metrics.json`. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-metrics -- --runId codex-cache-smoke-final` passed with `ok=true`; cold recorded 524 game resources, 2,669,943 transfer bytes, 503 cache-like resources, 2/2 scene hits, and 503/511 prewarm ok; warm recorded 524 game resources, 0 transfer bytes, 524 cache-like resources, 2/2 scene hits, and 511/511 prewarm ok; assertions `warmCompletedPrewarm`, `noPrewarmFailures`, `noCriticalConsoleErrors`, and `noNonFavicon404s` were all true. Follow-up verification passed `node --check apps/web/scripts/smoke-cache-metrics.mjs`, Web `npx tsc --noEmit --pretty false`, targeted `git diff --check`, and direct `npx next build`.
- 2026-05-18 real first-playable cache smoke: Cache metrics now include milestones for HTML ready, Bevy/runtime decision, scene blueprint, scene sprite readiness, Gateway connect/login/select, `StartGame`, `UserInformation`, game screen readiness, and `firstPlayableFrame`. `npm run smoke:playable-metrics` drives a fresh Chrome profile through `demo/demo` login, character select, real Gateway `StartGame`, Bichon scene load, first playable frame, complete prewarm, then repeats a warm pass in the same profile. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7210/ws npm run smoke:playable-metrics -- --runId codex-playable-smoke-final --waitTimeoutMs 90000` passed with `ok=true` and wrote `docs/generated/player-qa/cache-metrics/cache-metrics-codex-playable-smoke-final.json`; cold first playable was 1503.7ms with 839 game resources, 3,395,535 transfer bytes, 697 cache-like resources, 3/3 scene hits, and 511/511 prewarm ok; warm first playable was 1193.8ms with 740 game resources, 300 transfer bytes, 738 cache-like resources, 3/3 scene hits, and 511/511 prewarm ok. Assertions for first-playable presence/budgets, prewarm completion, no prewarm failures, no critical console errors, and no non-favicon 404s were all true.
- 2026-05-18 CacheStorage/quota diagnostics: `window.__mir2CacheMetrics.snapshot()` now includes Mir2 CacheStorage cache counts, entry counts, and `navigator.storage.estimate()` usage/quota values, and the QA overlay shows those values without adding player-facing UI. The smoke harness now asserts that the warm pass has populated Mir2 CacheStorage entries. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-metrics -- --runId codex-cache-storage-smoke-final --waitTimeoutMs 90000` passed with warm `cacheStorageCacheCount=2`, `cacheStorageEntryCount=510`, `storageUsageBytes=65338772`, 0 transfer bytes, 511/511 prewarm ok, and all assertions true; `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7210/ws npm run smoke:playable-metrics -- --runId codex-playable-storage-smoke-final --waitTimeoutMs 90000` passed with cold first playable 1659.6ms, warm first playable 2224.9ms, warm `cacheStorageCacheCount=2`, `cacheStorageEntryCount=555`, `storageUsageBytes=67045268`, 0 transfer bytes, 511/511 prewarm ok, no prewarm failures, no critical console errors, and no non-favicon 404s.
- 2026-05-18 Service Worker maintenance and QA reset: Player Web now exposes a QA-only `window.__mir2AssetCacheReset({ reload?: false })` helper that deletes all `mir2-asset-cache-*` CacheStorage buckets, unregisters the Mir2 asset Service Worker, marks the cache state as reset, refreshes cache metrics, and reloads by default unless `reload:false` is passed. The Service Worker also handles maintenance messages for explicit reset/status and reports stale-cache cleanup after each manifest config. Evidence: after restarting the stale 13011 dev server with current code, `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-maintenance -- --runId codex-cache-maintenance-smoke-final --waitTimeoutMs 90000` passed with `ok=true`; warm cache had 2 Mir2 caches / 510 entries / 65335069 usage bytes / 0 transfer bytes / 511/511 prewarm ok; the maintenance pass seeded `mir2-asset-cache-static-legacy-smoke`, verified manifest-version cleanup removed it, then `__mir2AssetCacheReset({ reload:false })` deleted 3 active caches, unregistered 1 Service Worker scope, and left `afterReset.cacheNames=[]`. All maintenance assertions (`maintenanceLegacyCacheSeeded`, `maintenanceLegacyCacheCleanedByVersion`, `maintenanceManualResetAvailable`, `maintenanceManualResetClearedCaches`) were true.
- 2026-05-18 cache persistence and budget guardrails: The cache metrics now record `navigator.storage.persisted()` and the result of the one-time `navigator.storage.persist()` request so QA can see whether a browser is likely to evict cached game assets. The Service Worker no longer writes `bootstrap` runtime caches before receiving the versioned manifest, and the frontend waits for the SW configured/cleanup ACK before refreshing storage metrics. The cache smoke now enforces anti-footgun budgets: prewarm requests <= 1000, warm CacheStorage entries <= 2500, and warm browser storage usage <= 256 MiB by default. Evidence: `MIR2_WEB_BASE_URL=http://127.0.0.1:13011 npm run smoke:cache-maintenance -- --runId codex-cache-budget-maintenance-smoke-final --waitTimeoutMs 90000` passed with `ok=true`; warm pass recorded 511/511 prewarm ok, 118 warm CacheStorage entries, 62272086 storage usage bytes, `storagePersisted=false`, `storagePersistGranted=false` in fresh headless Chrome, and budget assertions `prewarmWithinBudget`, `warmCacheStorageEntriesWithinBudget`, and `warmStorageUsageWithinBudget` all true. The maintenance pass seeded the legacy cache, version-cleaned it, deleted 3 active caches, unregistered 1 SW scope, and ended with zero Mir2 caches.
- 2026-05-18 R2/CDN remote asset release pass: Player Web now supports versioned remote static asset sourcing for the existing game cache. `/api/asset-manifest` exposes `remoteAssets` with `{version}`-resolved CDN base URL/object prefix, the asset Service Worker maps same-origin `/original-ui`, `/original-map`, and `/bevy-runtime` misses to the configured remote base before falling back to app origin, and the R2 release scripts stage/upload the exact manifest-declared critical packs instead of the full 7-8 GB source tree. Evidence: current-code Web on `127.0.0.1:13014` returned `remoteAssets.objectPrefix="mir2/v/37596e16d64fde7c"`, and with `MIR2_ASSET_BASE_URL=https://assets.example.com/mir2/v/{version}` it returned `remoteAssets.enabled=true` plus `assetBaseUrl="https://assets.example.com/mir2/v/37596e16d64fde7c"`; `npm run assets:remote:build -- --baseUrl http://127.0.0.1:13014 --assetBaseUrl https://assets.example.com/mir2/v/{version} --runId codex-r2-release-smoke` wrote `docs/generated/remote-assets/codex-r2-release-smoke/remote-asset-release.json` and `latest-remote-asset-release.json` with `stats.fileCount=512`, `stats.totalBytes=64626176`, `stats.missingCount=0`; `npm run assets:r2:dry-run -- --manifest docs/generated/remote-assets/codex-r2-release-smoke/remote-asset-release.json` reported `uploadCount=513`, `totalBytes=65000146`, and sample object keys under `mir2/v/37596e16d64fde7c/`. Live R2 upload is verified: bucket `mir2-web3-assets` has 513/513 objects under `mir2/v/37596e16d64fde7c`, public access is enabled at `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev`, GET/HEAD CORS allows `*`, public `original-ui/Prguse/4.png` returns 200 with immutable cache headers, and public `remote-asset-release.json` reports `assetBaseUrl="https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/37596e16d64fde7c"`, `stats.fileCount=512`, `stats.missingCount=0`. `node --check` passed for both new scripts and `mir2-asset-worker.js`.
- 2026-05-17 item/equipment hover info pass: Player Web now renders in-game Crystal-style item info tooltips for inventory, storage, belt, and character equipment slots instead of relying on native browser `title` text or showing no panel. Tooltips use the live snapshot fields already provided by Gateway (`name`, `description`, stack quantity, durability, attack, defence) and are anchored per slot to avoid right-edge overflow. Evidence: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; 13010 was restarted so the updated tooltip CSS is served; Browser DOM inspection after `demo/demo` game entry confirmed visible belt items include `.original-item-tooltip` content such as `Red Potion`, its description, and `Quantity`; Browser console error check returned `[]`. Browser hover synthesis did not set `:hover` in the in-app automation surface, so final pixel/feel acceptance remains manual.
- 2026-05-17 login Web3 action integration: Player Web now places the Passkey/Wallet alternatives inside the original login dialog's dark credential well instead of floating them below the panel, using compact gold-brown button treatment that reads as a secondary login action alongside ID/PASS. Evidence: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; the 13010 Web dev server was restarted so the updated CSS was served; Browser screenshot inspection on `http://localhost:13010/?gatewayWs=ws://127.0.0.1:7210/ws` confirmed the actions sit inside the login dialog with no text overlap; Browser console error check returned `[]`.
- 2026-05-17 login error-state hardening: Player Web now treats Gateway `type="error"` messages during login like terminal login failures, clearing pending password/new-account/Sui-login refs, ending the login busy overlay, and surfacing the Gateway error on the login panel instead of leaving the client stuck on `Logging in...` / `正在登录...`. The same close-path cleanup remains in place for disconnected sockets. Evidence: Web `npx tsc --noEmit --pretty false` passed in both the main checkout and the currently served `/private/tmp/mir2-main-human` web directory; an initial live Gateway WS probe confirmed the previous running binary emitted a `type="error"` response for unsupported `passkeyLogin`; a Browser login smoke on `http://localhost:13010/?gatewayWs=ws://127.0.0.1:7210/ws` reached character select with `demo/demo`; and after refreshing the running Gateway with the coherent temp build, a live HMAC-token WS probe confirmed `passkeyLogin` returns `LoginSuccess`. The main source checkout still has broader Gateway/Simulation WIP to reconcile before rebuilding cleanly from main source.
- 2026-05-16 strict Crystal CurrentLocation movement sync: Player Web now commits each local self Walk/Run action target into the local `WorldEntity` at action start, matching Crystal `PlayerObject.SetAction()` where `CurrentLocation` is advanced before `OffSetMove` draws the sprite back from the source tile. Self `UserLocation` packets and periodic `worldSnapshot` self entries are now allowed to confirm/stale-echo the active local action without overwriting that local CurrentLocation; only true corrections can hard-reset the self transform. Evidence: Web `npx tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, high-frequency Shift+`D/A` capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-230738.json`, 10s Shift+`D` capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-long-230835.json`, and held-run-plus-spam-click capture `docs/generated/player-qa/movement-jitter/r-strict-actionfeed-current-location-clickspam-231926.json`, all with `ok=true`, `noVisualJumps=true`, `noLogicalTileRollback=true`, `movementCommandQueueResponsive=true`, clean settle, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s.
- 2026-05-16 all-map resource/gameplay audit closure: Web map coverage and runtime map semantics now treat Crystal empty/out-of-range map sprite frames as Crystal `MLibrary.GetSize/Draw` no-draw behavior instead of frontend fallback risk. `audit:crystal-map-coverage` records 463/463 maps present and parseable, unsupported map types 0, parse errors 0, missing minimap indices `[]`, missing sampled map libraries 0, `visualFallbackRisk.mapCount=0`, and 453 Crystal-ignored no-draw frame references tracked separately. The new `audit:crystal-map-gameplay` records 1999 movement rows checked with 1906 direct transfers, 93 Crystal-ignored/deferred/special transfers, movement failures 0, 6341 respawn rows with 6293 candidate-backed and 48 Crystal-inert no-candidate warnings, respawn failures 0, 375 NPC rows with scripts found, 7 empty placeholder warnings, unimplemented NPC commands 0, and static map semantic failures 0 across safe zones, safe-zone spell flags, doors, cell lights, fishing cells, drop rules, and light/feature flags. Runtime fixes in Simulation also make local full-client map lookup work on this checkout, correct type-1 map cell stride parsing, suppress invalid/special Crystal movement transfers from runtime `transfer_map`, and avoid spawning monsters at invalid origins when Crystal would leave a respawn inert. Evidence: `CRYSTAL_CLIENT_ROOT=/Users/henryliu/obelisk/ai/numeron/mir2/downloads/crystal-client-full node apps/web/scripts/audit-crystal-map-coverage.mjs`, `CRYSTAL_CLIENT_ROOT=/Users/henryliu/obelisk/ai/numeron/mir2/downloads/crystal-client-full node apps/web/scripts/audit-crystal-map-gameplay.mjs`, Web `npx tsc --noEmit`, `cargo +1.89.0 fmt --check -p mir2-simulation`, focused Simulation `crystal_manifest_movements` 2/2, and focused Simulation `spread_slots` 2/2.
- 2026-05-15 local CurrentLocation movement closure: Player Web now promotes visually completed self Walk/Run actions into the local self `WorldEntity` / `sceneView.center`, matching Crystal's local `CurrentLocation` update instead of leaving the completed tile in a long-lived predicted/anchor layer while waiting for delayed snapshots. Stale `worldSnapshot` self positions are ignored when the local self transform is still a plausible forward Crystal action, so old `UserLocation` / snapshot echoes cannot pull the rendered player back after high-frequency input. Evidence: Web `npx tsc --noEmit`, direct `npx next build`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, `docs/generated/player-qa/movement-jitter/r-highfreq-keyseq-da-after-local-current-location-16ms.json`, `docs/generated/player-qa/movement-jitter/r-long-shiftd-after-local-current-location-16ms.json`, and `docs/generated/player-qa/movement-jitter/r-right-left-after-local-current-location-16ms.json` all record `ok=true`, `noVisualJumps=true`, `noLogicalTileRollback=true`, `movementCommandQueueResponsive=true`, `movementSettledWithoutResidualPlan=true`, `pendingPlanAtEnd=null`, no console errors, and no non-favicon 404s.
- 2026-05-15 high-frequency keyboard movement closure: Player Web now keeps a separate service-confirmation ledger for self Walk/Run actions, distinct from the visual ActionFeed. When WASD/Arrow input reverses direction while older movement commands are still waiting for `UserLocation`, the client updates the latest intent but does not derive a new opposite-direction command from stale speculative tiles. This prevents old server confirmations from pulling the rendered player backward during high-frequency run/turn input. The movement harness now supports strict `keyboardSequence` captures, pre-input warmup, WebSocket movement frame tails, and direction-step source-aware route-spam classification. Evidence: `docs/generated/player-qa/movement-jitter/r-highfreq-keyseq-da-after-outstanding-gate-170756.json` records 36 rapid Shift+`D/A` taps with `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `outstandingSelfMovementActions=[]`, no console errors, and no non-favicon 404s; `docs/generated/player-qa/movement-jitter/r-highfreq-right-then-left-after-outstanding-gate-170859.json` records a right-run then left-run reversal with the same zero-warning result and final `direction=Left`; `docs/generated/player-qa/movement-jitter/r-long-shiftd-after-outstanding-gate-170946.json` keeps the 12s Shift+`D` long-run regression green.
- 2026-05-15 long continuous run/walk rollback closure: Player Web now treats repeated same-tile `UserLocation` confirmations during held direction input as a Crystal-style blocked action instead of letting the client keep sending stale Walk/Run intents into the blocked tile. The movement input loop records no-progress self acks, marks both walk/run blocked at the authoritative source tile, suppresses the held direction for the route-block memory window, clears local action/render anchors on true correction, and keeps the separate local render lead window at two tiles while action feed lookahead can remain wider for delayed Zone confirmations. Evidence: `docs/generated/player-qa/movement-jitter/r-long-shiftd-fresh-after-first-block.json` records a 12s Shift+`D` run from `{330,270}` to `{345,270}` with `ok=true`, `noVisualJumps=true`, `noLogicalTileRollback=true`, `noRouteSpamWarnings=true`, `movementCommandQueueResponsive=true`, `movementSettledWithoutResidualPlan=true`, no console errors, and no non-favicon 404s; screenshot `r-long-shiftd-fresh-after-first-block.png`. Verification also passed Web `npx tsc --noEmit`, direct `npx next build`, focused Simulation `continuous_run_extends_run_grace_after_successful_run`, and `cargo +1.89.0 fmt --check -p mir2-simulation`.
- 2026-05-15 Crystal ActionFeed movement semantic alignment: Player Web now keeps a local self-action feed matching Crystal's `QueuedAction` / `ActionFeed` split. Local Walk/Run actions record source tile, target tile, direction, mode, sent time, and visual window; self `UserLocation` packets are first classified as confirmed action, stale echo, partial run confirmation, or true correction before any hard rollback is allowed. Rendering and debug `state.player` now fall back to the latest local self action target while authoritative packets catch up, so held keyboard/run movement does not snap back to an older server tile during normal confirmation lag. The Bevy runtime boot path also supports `skipRuntime=1` for DOM-only movement harnesses and avoids duplicate same-page boot after HMR. Evidence: Web `npx tsc --noEmit`, direct `npx next build`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and the CDP mini smoke `docs/generated/player-qa/movement-jitter/r-crystal-actionfeed-mini-smoke3.json` record Shift+`D` from `{330,270}` to `{345,270}` with `ok=true`, `rollbackCount=0`, `staleSampleCount=0`, final `feed=[]`, final `queue=[]`, plus screenshot `r-crystal-actionfeed-mini-smoke3.png`.
- 2026-05-15 Crystal chat control-bar closure: Player Web now treats the `Prguse/2034` chat control row like Crystal `ChatControlBar` instead of using it as display-only filters. All/Shout/Whisper/Lover/Mentor/Group/Guild buttons set the outgoing chat prefix (`""`, `!`, `/`, `:)`, `!#`, `!!`, `!~`), preserve/reset the input after send, and no longer hide the feed themselves; the Settings panel now owns Crystal-style channel visibility filters plus transparency; Trade sends the real `tradeRequest` browser command; Size collapses/expands chat; Report opens/closes the report dialog. The row's sprite buttons now have real 24x13 hit boxes and sit above the HUD, closing the prior "looks clickable but nothing happens" class. Evidence: `MIR2_STAGE5_ACCOUNT_MODE=demo MIR2_STAGE5_SMOKE_CHAT_ONLY=1 MIR2_WEB_BASE_URL=http://127.0.0.1:13010/?gatewayWs=ws://127.0.0.1:7210/ws node apps/web/scripts/smoke-stage5-ui.mjs` wrote `docs/stage5-screenshots/stage5-chat-controls-smoke-manifest.json` with `mode="chat-controls-only"`, 13 screenshots, every chat-control hit test `topMatches=true`, verified prefixes for all seven channel buttons, `lastCommand.type="chat"` with `!Codex shout smoke`, `lastCommand.type="tradeRequest"`, settings normal-filter and transparency toggles, collapse/expand, report open/close, and `criticalConsoleErrors=[]`. Verification also passed Web `node --check apps/web/scripts/smoke-stage5-ui.mjs`, `npx tsc --noEmit`, and direct `npx next build`.
- 2026-05-14 Crystal asset pipeline productionization: Player Web now has explicit asset scripts for the full Crystal client instead of relying on ad hoc exported folders. `npm run generate:crystal-asset-index` now fails fast when the full client root is incomplete, `npm run export:crystal-sounds` parses Crystal `Sound/SoundList.lst`, copies all available referenced wavs into `public/original-ui/Sound`, and writes `public/original-ui/sound-index.generated.json` plus `docs/generated/assets/latest-sound-assets.json`. Web runtime now consumes `ServerPacket::PlaySound` through `original-audio.ts` / `original-sound-index.ts`, shares one audio manager for login/select music and effects, and plays the Crystal button effect from shared sprite buttons. The on-demand sprite API now validates against `source-libraries.generated.json`, rate-limits very large libraries by frame count, returns typed status codes (`unsupported_library`, `library_not_indexed`, `library_too_large`, `crystal_data_missing`, `library_missing`), and keeps production builds warning-free under Turbopack. Evidence: `npm run smoke:crystal-assets` records 1,440 libraries, 2,143,132 frames, 1,624 maps, 1,607 source sounds, 450 SoundList entries, 447 available sound mappings, and no failures at `docs/generated/assets/latest-crystal-asset-pipeline-smoke.json`; live Next API smoke returned 200 for source-only `NPC/223` on-demand export, 200 for `sound-index.generated.json`, 200 for `Sound/100.wav`, and 400 for unsupported `Map/*`; Web `npx tsc --noEmit`, direct `npx next build`, `smoke:crystal-minimap-assets`, and `audit:crystal-map-coverage` passed. The three missing SoundList wavs (`22.wav`, `109.wav`, `ZombieRevive.wav`) are absent from the Crystal source and recorded as non-blocking missing references.
- 2026-05-14 full Crystal client resource sync: downloaded the MirFiles Crystal patch manifest into `downloads/crystal-client-full` from `https://ftp.mirfiles.co.uk/resources/mir2/crystal/patch/`, verified all 4,698 manifest entries with 0 missing files and 0 size mismatches, and refreshed Web original UI assets from that full client root. A full asset index now records 1,440 `.Lib` libraries, 2,143,132 source frames, 1,624 map files, and 1,607 sound files at `docs/generated/assets/full-crystal-client-index.json`; `public/original-ui/source-libraries.generated.json` lets Player Web treat every non-map Crystal sprite library as available and convert a missing library on demand through `/api/original-ui-meta`. The on-demand path was verified with previously unexported `AArmour/02`, generating 1,024 frames plus `meta.json`. `Data/mmap.Lib` still stops at index 449, so minimap indices 450/451 were sourced from the Crystal database preview BMPs and exported as `public/original-ui/MMap/450.png` and `451.png`. Evidence: `docs/generated/assets/latest-minimap-assets.json` now records `missingMiniMapIndices=[]`, and `docs/generated/map/latest-crystal-map-coverage.json` records `miniMapCoverage.missingMiniMapIndices=[]` with 463/463 source maps still present/parseable.
- 2026-05-13 shared Zone live two-client browser smoke: Player Web was run against a temporary Gateway/account-store on `127.0.0.1:7210` / `127.0.0.1:13010` with two independent browser pages, then the flow was committed as repeatable `npm run smoke:two-client-zone`. Account `zonea20260513053629` / character `ZA053629` and account `zoneb20260513053629` / character `ZB053629` both reached the game screen on Crystal map `0`; after Zone placement and ticks, page A saw B as a `player` entity, page B saw A as a `player` entity, and page B received A's movement broadcast (`ObjectWalk` / `ObjectRun` evidence in WebSocket frames). Evidence: `docs/generated/player-qa/two-client-zone/two-client-zone-20260513053629.json` records `ok=true`, `bothGame=true`, `aSeesB=true`, `bSeesA=true`, `bSawMovementBroadcast=true`, `noConsoleErrors=true`, and `noNonFavicon404s=true`, with screenshots `two-client-zone-20260513053629-a.png` and `two-client-zone-20260513053629-b.png`. The repeatable script run `docs/generated/player-qa/two-client-zone/two-client-zone-script-135930.json` also records `ok=true`, `aSawChatBroadcast=true`, no console errors, and no non-favicon 404s, with screenshots `two-client-zone-script-135930-a.png` and `two-client-zone-script-135930-b.png`. The lower-level WebSocket two-client smoke `docs/generated/load/two-client-zone-smoke-133316.json` records 2/2 ready clients, 0 errors, 38 commands sent, and 1,241 received messages.
- 2026-05-12 Crystal movement rollback/cadence follow-up: Player Web no longer starts the next movement cooldown from `UserLocation` receive time; direction and target-route movement now keep the Crystal 600ms cadence anchored to the command/action window, with only a short confirm tick after authoritative packets. Recent attack/skill actions now block local movement prediction while still allowing movement packets through, and route handoff no longer preserves a future predicted tile during that combat action window. This closes the repro where repeated right-clicks on/near `Training Dummy` mixed attack packets with movement, causing the client to render `{331,270}` or `{333,270}` before the server was allowed to move and then snap back. Evidence: `docs/generated/player-qa/movement-jitter/r-crystal-input-stress-route-preserve-block-102429.json` records held right-run plus eight repeated target clicks with interleaved `attack` commands, `ok=true`, `logicalRollbackWarnings=[]`, `jumps=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, and `holdThenSpamClickTargetQueueStrict pass=true`; `r-crystal-input-keyboard-final-102458.json` records held Shift+`D`/run with `ok=true`, four movement commands, final player `{337,270}`, no rollback, no stale prediction, no queue warnings, no browser console errors, and no non-favicon 404s. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, live local Gateway/Web captures, and screenshot inspection at `r-crystal-input-stress-route-preserve-block-102429.png` plus `r-crystal-input-keyboard-final-102458.png`.
- 2026-05-11 Crystal input-loop hard-standard follow-up: Player Web now treats Crystal's local action display as the source of truth while server confirmations are still in flight. Same-source `UserLocation`/snapshot echoes inside the action window no longer hard-correct the client, blocked-step hints are de-duplicated before reroute, blocked run predictions cannot be restored as the same pending run, and accepted self movement packets bridge the React state frame so the visible player does not momentarily fall back to the previous server tile. Direction pending now releases after Crystal's 600ms action window plus 400ms correction window once the local predicted tile is visible, keeping WASD/Arrow held movement responsive without queue residue. Evidence: `docs/generated/player-qa/movement-jitter/r-crystal-input-align-monotonic-182603.json` records held-run plus repeated right-click target stress with `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, and `holdThenSpamClickTargetQueueStrict pass=true`; `r-crystal-input-align-keyboard-d-release-182950.json` records held Shift+`D`/run with four `run Right` commands, no rollback, no stale prediction, and no queue warning; `r-crystal-input-align-keyboard-arrow-183040.json` records held `ArrowRight`/run with the same zero-warning movement feel. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, live local Gateway/Web captures, and screenshot inspection at `r-crystal-input-align-monotonic-182603.png`.
- 2026-05-11 movement input-latency/tick-cadence follow-up: Player Web now starts direction-step and click-target prediction on the next Crystal tile in the same input frame instead of only turning in place while waiting for server `UserLocation`. Gateway tick handling now avoids 100ms movement-time flooding: idle ticks stay slow, and movement commands schedule one 320ms confirmation tick to unlock the next queued step without letting tick traffic swallow player movement confirms. Evidence: `docs/generated/player-qa/movement-jitter/r-input-latency-keyboard-d-0511i.json` records keyboard `D` first prediction at 52ms (`330,270 -> predicted 331,270`), first `UserLocation` at 374ms, and final `{333,270}`; `r-input-latency-shift-run-0511j.json` records Shift+`D` first prediction at 58ms to `{332,270}`, first `UserLocation` at 402ms, and final `{336,270}`; `r-input-latency-click-target-0511k.json` records click-target arrival at `{333,270}` with first prediction at capture start and first `UserLocation` at 307ms. All three record `ok=true`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web captures, and screenshot inspection.
- 2026-05-11 keyboard movement input follow-up: Player Web now accepts WASD and Arrow-key movement in the game stage while preserving Crystal's existing click movement path. Held keyboard directions reuse the same Crystal direction-step queue as mouse-hold input, support diagonal combinations, ignore editable chat/login/panel fields, and use Shift as run intent by targeting the two-tile Crystal run step. The old selected-target approach shortcut moved from `A` to `F` so `A` can be left movement; Space/Enter remain primary target action. Evidence: `docs/generated/player-qa/movement-jitter/r-keyboard-wasd-0511k.json` records keyboard `D` walk `330,270 -> 332,270`, `r-keyboard-arrow-0511l.json` records `ArrowRight` walk with no rollback or queue residue, and `r-keyboard-shift-run-0511m.json` records Shift+`D` as `run Right` ending at `332,270`; all three have `ok=true`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, movement harness syntax check, live local Gateway/Web keyboard captures, and screenshot inspection.
- 2026-05-11 Crystal input/NPC marker hard-standard follow-up: Player Web now recovers stale in-flight movement actions before the 1200ms responsiveness threshold by re-anchoring the target plan to the authoritative server tile, clearing pending state, and retrying after a short Crystal correction delay instead of letting held-run plus repeated target clicks age into a stalled queue. The renderer also keeps unconfirmed target/direction pending tiles out of the visible player position while still reflecting immediate facing, so automated checks distinguish true visual rollback from debug queue state. Evidence: `docs/generated/player-qa/movement-jitter/r-click-target-crystal-input-final-090309.json`, `r-route-spam-obstacle-crystal-input-final-090355.json`, `r-blocked-target-crystal-input-final-090443.json`, and `r-input-queue-held-run-spam-click-crystal-input-final-090527.json` all record `ok=true`, `jumps=[]`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `stalePredictionWarnings=[]`, `commandQueueWarnings=[]`, `pendingPlanAtEnd=null`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; the held-run stress path ends settled at `{335,270}` with `movementCommandQueueResponsive pass=true`. A temporary isolated Gateway/account-store marker fixture verified NPC click and marker geometry without touching the main local account data: `docs/generated/player-qa/npc-click/r-npc-click-marker-crystal-anchor-final-090830.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `crystalLeftDeltaPx=0`, `crystalTopDeltaPx=0`, no browser errors, and no non-favicon 404s. The marker anchor follows Crystal `Client/MirObjects/NPCObject.cs` draw math: `DrawLocation + BodyLibrary.GetOffSet(BaseIndex) + (size.Width / 2 - 28, -40)`, so the icon's right edge aligns to the NPC body center as in Crystal. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, movement/NPC script syntax checks, four live local Gateway/Web movement captures, isolated marker capture, and screenshot inspection.
- 2026-05-11 movement feel rollback follow-up: `capture-web-movement-jitter.mjs` now records and fails on logical tile rollback during an active Crystal movement direction, and it asserts `walk`/`run`/`turn` direction commands are reflected by the predicted/self sprite direction within a 260ms Crystal input-loop window. The live repro `docs/generated/player-qa/movement-jitter/r-direction-lag-logical-rollback-0511b.json` caught the old failure (`noVisualJumps=false`, `noLogicalTileRollback=false`, `logicalRollbackCount=1`) when a held run plus repeated target clicks cleared prediction from `{338,270}` back to confirmed `{334,270}` before the server caught up. Player Web now keeps the local predicted anchor through the server-lag window, does not count already-confirmed same-tile prediction as pending, and avoids converting a still-in-flight sent source into a hard route correction. Fixed evidence: `docs/generated/player-qa/movement-jitter/r-direction-lag-logical-rollback-0511-fix-bust-063119.json` records `ok=true`, `settle.status="settled"`, `pendingPlanAtEnd=null`, final player `{338,270}`, `predictedPlayer=null`, `jumps=[]`, `logicalRollbackWarnings=[]`, `directionLagWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Regression evidence: `r-route-spam-obstacle-regression-063209.json` and `r-blocked-target-regression-063209.json` both record `ok=true`, no visual/logical jumps, no route-spam warnings, and explicit `targetBlocked` non-failure status. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and live local Gateway/Web captures at `127.0.0.1:7210` / `127.0.0.1:13010`.
- 2026-05-10 NPC marker/click quest follow-up: the NPC click harness now runs both out-of-range and adjacent MirGuide scenarios and records marker/body/nameplate geometry. Evidence: `docs/generated/player-qa/npc-click/r-npc-click-marker-quest-0511a-summary.json` records `ok=true`; out-of-range `dialogTitle=MirGuide_Peter`, `moveCount=2`, `interactCount=1`; adjacent `dialogTitle=MirGuide_Peter`, `moveCount=0`, `interactCount=1`; marker rect `456,225 28x29`, NPC body rect `440,265 60x80`, `horizontalDeltaPx=0`, `iconBottomToNpcTopPx=-11`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-npc-click.mjs`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, focused `guild_` 15/15, Hero AI 25/25, full locked Simulation 855/855 plus Hero AI 25/25, Gateway shared registry 15/15, package fmt/check, and targeted diff checks.
- 2026-05-10 blocked-target movement settle pass: Player Web now carries short-lived blocked-step memory across repeated target clicks and delays same-source retries until server confirmation/correction, preventing unreachable targets from endlessly resending stale movement vectors or leaving a stale predicted player. The movement jitter harness adds `blockedTarget`, treats blocked/unreachable target residue as an explicit non-failure only when identified, and now asserts no jumps, no route-spam warnings, no console errors, and no non-favicon 404s. Evidence: `docs/generated/player-qa/movement-jitter/r-blocked-target-nonfailure-0511-fixed6.json` records `ok=true`, `settle.status="settled"`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPending=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web blocked-target capture, focused `guild_` 14/14, Hero AI 23/23, full locked Simulation 854/854 plus Hero AI 23/23, Gateway shared registry 15/15, package fmt/check, and targeted diff checks.
- 2026-05-10 route-spam settle follow-up: route-spam obstacle captures now wait for a final settle phase and record `pendingPlanAtEnd`, distinguishing real unresolved blocked targets from harness window cutoffs. Player Web now rechecks early server corrections after the reroute delay instead of waiting for the long correction grace, and movement packet handling updates the local world ref synchronously so the animation/input loop does not send one extra stale action from an older server tile. Evidence: `docs/generated/player-qa/movement-jitter/r-route-spam-obstacle-settle-followup5.json` records `ok=true`, `settle.status="settled"`, `waitedMs=7`, player `{334,273} -> {334,271}`, `pendingPlanAtEnd=null`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web capture, package fmt/check, Gateway shared registry 15/15, and full locked Simulation 852/852 plus Hero AI 20/20.
- 2026-05-10 route-spam/obstacle input pass: Player Web now stores short-lived blocked route steps after server correction, uses those hints plus visible entity occupancy to choose a walk fallback or nearby reroute direction instead of repeatedly resending the same stale target vector, and reanchors target plans from confirmed server positions after a bounded delay. Self dash/push/attack-move packets and object dash/push packets now flow through the same movement reconciliation path as walk/run/backstep. The movement harness adds `routeSpamObstacle`, runtime exception details, sent-packet probes, and `routeSpamWarnings`. Evidence: `docs/generated/player-qa/movement-jitter/r-route-spam-obstacle-final4.json` records `sampleCount=119`, player `{334,273} -> {334,271}`, `movementPlan=null`, `predictedPlayer=null`, `jumps=[]`, `routeSpamWarnings=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; coordinator reruns `r-route-spam-obstacle-coordinator*.json` also recorded `jumps=[]` and `routeSpamWarnings=[]` with no browser errors. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, live local Gateway/Web captures, Simulation/Gateway fmt, locked four-package check, and full locked Simulation 850/850 plus Hero AI 17/17.
- 2026-05-10 client input Crystal-feel pass: Player Web now keeps the local predicted player anchor through the Crystal 600ms direction-step visual window when the server confirms the same tile, instead of clearing prediction immediately and snapping the draw source back to the server tile mid-OffSetMove. The movement jitter harness also makes fixed-map center-sprite jump detection direction-aware, so diagonal `UpRight` Crystal map displacement is not mistaken for rollback while true opposite-sign jumps still fail. Evidence: `docs/generated/player-qa/movement-jitter/r-client-input-crystal-feel-final-diag.json` records a held right-run switched into eight repeated right-click target updates toward `338,266`, final player `338,266`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; early samples show the first confirmed `332,268 UpRight` step retaining `predictedPlayer={x:332,y:268,direction:"UpRight"}` after the server `UserLocation` arrives, preserving Crystal `CurrentLocation + OffSetMove` continuity. Verification passed `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and the live local Gateway/Web capture at `127.0.0.1:13010` / `127.0.0.1:7210`.
- 2026-05-10 client action-feel follow-up: target-click movement plans now inherit the current local action source from an in-flight held-direction queue or predicted position instead of snapping their planning source back to the last server tile, and target plans now apply the same bounded local-lead gate before sending the next run/walk packet. The movement jitter harness exposes `directionStepPendingQueue` and adds `holdThenSpamClickTarget` to stress the hold-to-repeated-click transition. Evidence: `docs/generated/player-qa/movement-jitter/r-client-action-feel-hold-spam-arrive-222639.json` records random account `QA222639`, held-run plus eight repeated right-click target updates, final player `338,270`, `movementPlan=null`, `predictedPlayer=null`, `directionStepPendingQueue=[]`, `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `npx tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, targeted `git diff --check`, and the live local Gateway/Web capture at `127.0.0.1:13010` / `127.0.0.1:7210`.
- 2026-05-10 NPC click/quest-marker follow-up: Player Web now treats an out-of-range NPC click as a Crystal-style approach-and-call target instead of losing the interaction, then calls the NPC once adjacent; the same-NPC repeat guard was shortened so a failed/no-op click does not swallow the next valid interaction. Quest markers are centered over the NPC body anchor instead of being shifted left by a full icon width. Evidence: `docs/generated/player-qa/npc-click/r-npc-click-after-mirguide-113448.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit` plus the live local Gateway/Web NPC click capture.
- 2026-05-10 Crystal input queue refresh: Player Web now treats target-click movement plans and held-direction movement as separate Crystal-style action queues, clearing stale direction confirmations when a new target plan starts, reconciling direction-step confirmations from both live packets and world snapshots, pre-queueing the next input 100ms before the 600ms action window completes, and allowing a bounded four-run-tile local lead while blocking any next command that would exceed that lead. The movement QA harness now includes `spamClickTarget` to stress repeated right-click destination updates. After a clean 13010 Next restart, `docs/generated/player-qa/movement-jitter/r-input-queue-fresh-092543.json` records held right-run sending four `run Right` commands in 1.8s with predicted movement `330,270 -> 338,270` and `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-click-target-fresh-092652.json` records right-click target arrival `330,270 -> 338,270`, `movementPlan=null`, and `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-spam-click-target-before-094503.json` records ten repeated right-clicks on the same target reaching `338,270` with `jumps=[]`. Fresh NPC click evidence at `docs/generated/player-qa/npc-click/r-npc-click-fresh-093512.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`, and live local Gateway/Web movement/NPC captures.
- 2026-05-10 Crystal dash-packet bridge pass: Gateway now serializes `UserDashAttack`, `ObjectDashAttack`, and `UserAttackMove`, and Player Web consumes Crystal self/object dash, dash-fail, dash-attack, attack-move, and push packets through the same server-reconciliation path as walk/run/backstep. Dash and dash-attack packets now drive running-style movement animation instead of being ignored or parsed as `0,0` nested-location payloads. Verification passed Web `pnpm --dir apps/web exec tsc --noEmit` plus focused Simulation `ShoulderDash`, `FlashDash`, and `SlashingBurst` regressions.
- 2026-05-09 component-boundary cleanup pass: Player Web's original-client shell was split into focused component modules without changing gameplay state or movement timing. `apps/web/app/components/original-client-overlays.tsx` now owns login/select/HUD/SpriteButton overlays, `original-client-panels.tsx` owns chat filter/feed, belt, and durability panels, and `original-client-dialogs.tsx` owns Mail, Report, and NPC dialog surfaces. The main shell dropped from 8,795 lines to 7,587 lines while keeping the movement/scene renderer and command queue in one place for the Crystal timing work. Verification passed Web `tsc --noEmit`, direct `next build`, targeted `git diff --check`, `node --check apps/web/scripts/capture-web-npc-click.mjs`, HTTP 200 for `http://127.0.0.1:13010/?gatewayWs=ws%3A%2F%2F127.0.0.1%3A7210%2Fws`, Gateway `/health`, and live browser captures: `docs/generated/player-qa/movement-jitter/r-component-split-input.json` records held right-run `330,270 -> 336,270` with `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-component-split-route.json` records the four-step click route with `jumps=[]`; `docs/generated/player-qa/npc-click/r-component-split-mirguide-click.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`.
- 2026-05-09 Crystal input/NPC anchor follow-up pass: Player Web now treats NPC activation like Crystal's `CallNPC` path: clicking an NPC sends one immediate `interact` command with same-NPC throttling while an NPC dialog is open / within the short repeat guard, and no longer starts a client-side auto-approach `moveTo` plan. The client-side movement lead is capped to one pending run/walk action (two tiles) instead of retaining a two-action queue, reducing continuous-click rollback while keeping Crystal's 600ms action cadence. NPC quest-marker CSS now uses a top-left Crystal `DrawLocation + BodyLibrary.GetOffSet + (width/2 - 28, -40)` style anchor instead of centering above the nameplate. Added `apps/web/scripts/capture-web-npc-click.mjs` to verify real DOM NPC clicks. Evidence after a clean Next restart: `docs/generated/player-qa/npc-click/r-crystal-mirguide-click-after.json` records `dialogTitle=MirGuide_Peter`, `interactCount=1`, `moveCount=0`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `docs/generated/player-qa/movement-jitter/r-crystal-input-after-restart.json` records held right-run `330,270 -> 336,270` with `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-crystal-route-click-after-restart.json` records run/right, run/right, walk/down, walk/left route arrival with `jumps=[]`. Verification passed Web `tsc --noEmit`, `node --check apps/web/scripts/capture-web-npc-click.mjs`, focused Simulation `crystal_manifest_gtmerchant_interact_opens_dialog_when_adjacent`, and live local Gateway/Web browser captures.
- 2026-05-09 movement queued-local confirmation pass: Web held/continuous movement now mirrors Crystal's local `QueuedAction` consumption more closely by allowing the next walk/run action to start on the Crystal 600ms cadence from the last local action position while keeping a bounded two-action / four-tile lead over server confirmation. Server `UserLocation` packets now drain the queued confirmations in order and only clear prediction on timeout/correction, removing the visible pause/rollback that happened when the client waited for the previous confirmation before visually starting the next step. Evidence: `docs/generated/player-qa/movement-jitter/r-live-hold-right-after-queued-local.json` records held right-run moving from `330,270` to `340,270` with `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `docs/generated/player-qa/movement-jitter/r-live-click-target-8-after-queued-local.json` records a right-click target path reaching `338,270` with `jumps=[]`; `docs/generated/player-qa/movement-jitter/r-live-click-route-after-queued-local.json` records a run/right, run/right, walk/down, walk/left click route with each segment arriving and `jumps=[]`. Verification passed Web `npx tsc --noEmit`, `git diff --check`, and live local Gateway/Web movement captures.
- 2026-05-08 Stage 5 dirty-save full-smoke hardening pass: Player Web smoke now defaults to a fresh throwaway account, keeping `demo/Scout` clean for human acceptance, while explicit `MIR2_STAGE5_ACCOUNT_MODE=demo` still covers accumulated demo-save state. The smoke validates the real command path for inventory split/use/drop, ground item/gold pickup, storage store/take-back, belt use, NPC services, compact layouts, and late-system System Menu panels. The harness no longer assumes fixed Red/Blue Potion slots or pristine storage: it seeds missing items, uses exact item `uniqueId` / drop `objectId` assertions, checks all inventory containers plus belt where Crystal auto-stack rules can move consumables, and falls back to direct object pickup when a ground marker is not clickable. Evidence updated at `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`: 114 screenshots, `criticalConsoleErrorCount=0`, `compactPanelCount=9`, `compactTextNodeCount=22`, `compactMatrixCount=3`, `systemMenuSocial=44`, `storageTakeBackFlow=4`, `inventorySplitFlow=3`, `groundPickupFlow=3`, and `groundGoldPickupFlow=3`. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, and the live Gateway/Web smoke at `http://127.0.0.1:13010/?gatewayWs=ws%3A%2F%2F127.0.0.1%3A7210%2Fws`.
- 2026-05-08 late-dialog command/readiness pass: Player Web System Menu now exposes command-backed Hero and Item Rental panels, and Creature/Mount/Fishing buttons now send real Gateway browser commands instead of being visual-only. The fast Stage 5 smoke verifies recent command history for `updateIntelligentCreature`, equipment `useItem` on mount slot, `fishingCast`, `fishingChangeAutocast`, `newHero`, and `itemRentalRequest`; the System Menu social coverage now includes Hero and ItemRental rows, raising fast-smoke social states from 36 to 44. Simulation also exposes `stage5Systems.itemRental` in world snapshots so the rental panel can read partner, fee, period, deposited item, lock state, and rented records. Evidence: live local Gateway/Web smoke passed with 22 screenshots, `systemMenuFeature=10`, `systemMenuSocial=44`, and `systemMenuQaTransfer=3`. Verification passed Web `node --check scripts/smoke-stage5-ui.mjs`, Web `npx tsc --noEmit`, focused Simulation `item_rental_` 3/3, locked Simulation/Gateway check, and Gateway browser-command mapping 7/7. Human Crystal dialog/pixel acceptance remains open.
- 2026-05-08 map torch/fire light offset pass: Web map object rendering now preserves Crystal Lib frame offset metadata for exported/dynamically loaded map frames and applies the Crystal offset mode to the Bichon `Objects/2723-2732` torch/fire blend frames in the packaged starter-map fallback. This moves the visible torch/fire light layer from the right/down neighboring cell back onto the red torch head, and routes those blend frames through generated additive-clean PNGs so the original dark edge pixels no longer render as a black ring. Evidence: `docs/generated/player-qa/light-offset/r-light-offset-torch-clean-state.json` records player `0:336,278`, torch body `Objects/2733` cell `335,274` rect `left=422 top=118`, and torch fire/light `Objects/2730` cell `336,275` rect `left=420 top=88` with `renderPath=/generated/original-map-blend/WemadeMir2/Objects/2730.png`, `mixBlendMode=screen`, `consoleErrorCount=0`, and `nonFaviconNetwork404s=[]`; screenshot evidence is `docs/generated/player-qa/light-offset/r-light-offset-torch-clean.png`. Verification passed Web `npx tsc --noEmit`, blend-asset generation/script syntax checks, and focused live capture against local Gateway/Web.
- 2026-05-08 movement feel cleanup pass: Web scene motion now uses Crystal-style six-frame walk/run displacement for the camera/entity offset, quantizes movement offsets to the same even-pixel cadence as Crystal `OffSetMove`, and caps held-direction local prediction to one unconfirmed walk/run action instead of letting repeated right-hold samples drift many tiles ahead of the server. The movement capture harness can now create isolated QA accounts/characters and records command/gateway movement tails for diagnosis. Evidence: `docs/generated/player-qa/movement-jitter/r-movement-direction-pending-cleared-after.json` records held right-run with `jumps=[]`, derived maximum prediction lead of 2 tiles, integer/even `centerSprite` deltas, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `docs/generated/player-qa/movement-jitter/r-movement-click-direct-crystal-frame-after.json` records direct run/walk route replay with `jumps=[]`, derived maximum prediction lead of 0 tiles, and no browser errors. Verification passed Web `npx tsc --noEmit`, movement capture script syntax, and live local Gateway/Web captures.
- 2026-05-07 Stage 5 frontend 2/4/5/6 closure pass: Player Web now consumes live Crystal magic/combat visual packets (`Magic`, `MagicCast`, `MagicDelay`, `MagicLeveled`, `ObjectMagic`, `ObjectProjectile`, `MapEffect`, `AddBuff`, `RemoveBuff`, `PauseBuff`) instead of relying only on snapshots, applies Crystal-like action timing for melee/range/struck/death windows, and keeps spell/buff cooldown state visible through the HUD skill surface. Late-system UI coverage now includes a real `trade` chat filter, dynamic System Menu social panels for ranking/friend/group/guild/trade/market/marriage/mentor/relationship, and command dispatch for supported Stage 5 social/trade/market actions. NPC/quest smoke now opens InnKeeper_Brittney through the Crystal dialog path, strips raw script markup from visible dialog text, exposes dialog/quest state to QA, routes selected-NPC primary actions through approach handling, and verifies Quest Diary row title/stage/progress/reward content. Responsive coverage now runs a compact matrix across 900x640, 768x640, and 820x540, adds overflow-safe CSS for mail/storage/system/social/quest text, and anchors the screenshot output directory to the repo path. Evidence: full live Stage 5 UI smoke against an isolated Gateway captured 113 screenshots with `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `systemMenuFeature=6`, `storagePassword=9`, `npcDialogFlow=11`, and `combatFlow=2`; verification passed Web `npx tsc --noEmit`, smoke script syntax, and the live isolated-Gateway smoke.
- 2026-05-07 movement/animation Crystal-timing pass: Player Web now records walk/run action windows from `UserLocation` / `ObjectWalk` / `ObjectRun` packets and drives entity sprite frames from the action start time instead of the global scene frame. Player, monster, and NPC standing frames now use the Crystal frame intervals (`Player`/monster 4 frames at 500ms, NPC 4 frames at 450ms), movement actions use the 6-frame/600ms Crystal timing window, and the browser-only attack bounce transform was removed so attack motion comes from the source sprite frames. Evidence: `docs/generated/player-qa/movement-jitter/r-movement-animation-crystal-timing.json` against local Gateway/Web with `demo/demo` records held right-run from `330,270` to `338,270`, `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`; `r-movement-animation-crystal-timing-live-tick.json` keeps automatic Gateway ticks enabled and records concurrent `ObjectWalk` monster movement packets with `jumps=[]`, `consoleErrors=[]`, and `nonFaviconNetwork404s=[]`. Verification passed Web `npx tsc --noEmit` and movement capture script syntax.
- 2026-05-07 Stage 5 full-smoke stabilization pass: the deterministic Player Web smoke now disables automatic keep-alive/tick flooding with `autoTick=0`, captures WebSocket frame diagnostics for timeout triage, opens a real Crystal NPC dialog through `qa.openNpcDialog`, and drives combat through a Crystal-backed `BugBat` event spawn placed on a nearby spawnable map tile. Full `smoke:stage5-ui` against an isolated local Gateway captured 102 screenshots with `criticalConsoleErrorCount=0`; manifest evidence records `storagePassword=9`, `storageStoreFlow=4`, `storageTakeBackFlow=4`, `characterRepairFlow=8`, `gameShopFlow=4`, `beltUseFlow=7`, `beltMouseUseFlow=3`, `npcDialogFlow=11`, and `combatFlow=2`. Verification passed Web `npx tsc --noEmit`, smoke script syntax, Rust fmt/check for Simulation/Gateway, full locked `mir2-gateway` 107/107 plus packet-trace bin 17/17, full locked `mir2-simulation` 731/731, shared in-process registry 11/11, and the live isolated-Gateway Stage 5 UI smoke.
- 2026-05-07 service-backed storage/repair, GameShop, and belt full-hotkey pass: Player Web can now target an isolated Gateway through `?gatewayWs=` / `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL`, allowing deterministic Stage 5 UI smoke against a fresh account store without mutating the developer's default `7110` store. Full `smoke:stage5-ui` captured 101 screenshots with `criticalConsoleErrorCount=0`; the manifest records `storagePassword=9`, `storageStoreFlow=4`, `storageTakeBackFlow=4`, `beltUseFlow=7`, `characterRepairFlow=8`, and `gameShopFlow=4`. The smoke opens InnKeeper_Brittney's Crystal `@Storage` service, verifies service-backed storage password set/unlock/change/remove including persisted last-set timestamp exposure, stores Dagger into warehouse slot 4, takes Red Potion back from storage into inventory, damages equipped Dagger through deterministic QA setup, repairs it through Blacksmith_Smith `@Repair` and Blacksmith_Bill `@SRepair` with durability/gold assertions, seeds gold through real Mail claim, buys `AccuracyPotion` from GameShop with Gold and verifies carry-slot delivery plus purchase feedback, presses belt hotkeys `1..6`, and confirms the Crystal Mail row no longer emits nested-button hydration errors. Evidence: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json` plus new screenshots including `stage5-storage-service-npc.png`, `stage5-storage-password-set.png`, `stage5-storage-password-unlocked.png`, `stage5-storage-password-changed.png`, `stage5-storage-password-removed.png`, `stage5-storage-service-return.png`, `stage5-repair-service-npc.png`, `stage5-special-repair-service-npc.png`, `stage5-gameshop-gold-open.png`, and `stage5-gameshop-gold-buy.png`.
- 2026-05-07 P1/P2 social System Menu pass: the Crystal-style narrow Menu social branch now opens player-facing group, guild, mentor, relationship, ranking, keyboard/help, and report/admin surfaces without visible Web/QA placeholder wording. The backend packet-runtime round that backs these surfaces also added typed Group utility, Quest, Refine, Market, OpenDoor, and request-info behavior. Verification passed Web `npx tsc --noEmit`, script syntax check, and a fast live Stage 5 UI smoke against local Gateway/Web with 17 screenshots, including `systemMenuSocial=24`, `systemMenuFeature=6`, and `systemMenuQaTransfer=3` manifest counts. Human Crystal visual/feel acceptance for exact dialog bitmaps remains open.
- 2026-05-01 R327 Gameshop purchase and map-click arrival pass: Gameshop Buy is now wired from the Crystal product cell to `gameShop.buyCredit` / `gameShop.buyGold`, with Web state exposing account credit and backend handling purchases from the generated Crystal game-shop manifest. Browser evidence for `QA0429A / QA0429Hero` verifies the first product (`AccuracyPotion`) sends `gameShop.buyCredit` with args `20,1`; because the QA account has `credit=0`, the expected insufficient-currency message is shown. Positive credit delivery is covered by the focused simulation test, which deducts credit and delivers the manifest-backed item by Stage 5 mail. Map click-to-arrive now holds only one pending target step ahead of server confirmation, reconciles self `ObjectRun` / `ObjectWalk` packets immediately, and removes the 180ms movement-time tick flood that was delaying queued `moveTo` packets behind monster updates. Evidence: `docs/generated/player-qa/r327-gameshop-buy-click-final-clean-state.json` records `network404Count=0`, `consoleErrorCount=0`, `gameShop.visible=true`, and the `gameShop.buyCredit` command; `docs/generated/player-qa/movement-jitter/r327-map-click-target-arrival-fixed3.json` records right-click target `338,270`, final player `338,270`, `movementPlan=null`, and `jumps=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax checks, focused game-shop simulation test, `mir2-gateway` check, and targeted CDP captures. `NPC/25` was exported from the Crystal client to remove the prior resource 404 in this scene.
- 2026-05-01 R326 held-mouse queued-action input fix: Web now separates Crystal-style held input sampling from action execution. The scene still samples the held pointer every 100ms, but `page.tsx` keeps the latest pending direction request instead of dropping samples during the 600ms walk/run action window. When the current movement action can be consumed, Web uses the latest queued direction to send one Crystal-like `Walk` / `Run` packet, matching the original client's `User.QueuedAction` overwrite/consume model without sending movement packets at a 100ms speed. Evidence: `docs/generated/player-qa/movement-jitter/r326-web-hold-run-queued-direction.json` records held right-run from `332,270` to `344,270`, `movementPlan=null`, fixed map-sprite continuity, and `jumps=[]`; `r326-web-hold-run-ws-send-probe.json` records WebSocket `sentMoveTail` entries for repeated `{"type":"run","direction":"Right"}` with `jumps=[]`. Gateway movement logs with `MIR2_GATEWAY_MOVE_LOG=1` show continuous `Run direction=Right -> UserLocation=(332..344,270)`. Verification passed: web `tsc --noEmit`, capture-script syntax check, and targeted CDP movement captures. The prior `original-ui/NPC/25/meta.json` 404 seen in this area was removed by the R327 asset export.
- 2026-05-01 R325 held-run visual jitter fix: fixed the remaining user-reported movement stutter/backtrack during held right-button running. The root cause was a one-frame mismatch between the newly predicted player tile and the previous motion snapshot: map sprites were rebuilt around the predicted tile while camera interpolation still used the old snapshot, so fixed floor sprites could jump right by roughly 2 tiles at server-confirmation boundaries. Web now keeps predicted movement in the render pipeline, uses the predicted player as the viewport/map/entity basis, and refreshes entity motion snapshots synchronously before rendering so map/camera math cannot use mixed old/new movement targets. The movement capture now records fixed map-sprite keys to catch background backtracking. Evidence: `docs/generated/player-qa/movement-jitter/r325-web-hold-run-final-4s.json` records held right-run from `332,270` to `344,270`, `movementPlan=null`, fixed sprite `sprite-3:330:270:0` moving monotonically, and `jumps=[]`; `r325-web-hold-run-sync-motion-snapshot.json` also records `jumps=[]` after the synchronous snapshot fix. Gateway move logging remains gated by `MIR2_GATEWAY_MOVE_LOG=1` and shows continuous `Run direction=Right -> UserLocation=(332..344,270)`. Verification passed: web `tsc --noEmit`, `node --check apps\web\scripts\capture-web-movement-jitter.mjs`, `cargo +1.89.0 check --locked -p mir2-gateway`, and targeted CDP movement captures. The known unrelated `original-ui/NPC/25/meta.json` 404 remains in movement captures.
- 2026-05-01 R322 movement correction loop fix: fixed the user-reported severe movement back-and-forth loop by removing predicted coordinates as the logical source for the next movement step. Prediction is now visual-only, and the movement plan records the pending server step; if `UserLocation` / snapshot correction does not land on that pending tile after the step window, Web clears the plan and prediction instead of repeatedly chasing the old target. This prevents client prediction and server correction from pulling the player in opposite directions around blocked/partially blocked Bichon tiles. Evidence: `docs/generated/player-qa/movement-jitter/r322-web-movement-no-predict-source.json`, `r322-web-movement-correction-stop.json`, and `r322-web-movement-open-area.json`; all record `jumps=[]`. Verification passed: web `tsc --noEmit` and targeted movement captures. Remaining movement feel work is Crystal-like continuous held-button `QueuedAction` behavior, but the oscillating loop is fixed.
- 2026-05-01 R323 held-mouse `QueuedAction` movement pass: Web now mirrors the original client's held mouse loop more closely. The scene input layer samples the held pointer every 100ms, stores coordinates in the Crystal 1024x768 stage coordinate system, and held walk/run dispatches Crystal-like `Walk` / `Run` direction packets instead of repeatedly feeding absolute `moveTo` path targets. The frontend keeps a local action coordinate for the next queued step, while server `UserLocation` / snapshots still reconcile or clear prediction on correction. Evidence: `docs/generated/player-qa/movement-jitter/r323-web-hold-run-direct-direction.json` records a 2.2s right-hold run from `330,270` to final `340,270` with `jumps=[]`; `r323-web-hold-walk-direct-direction.json` records left-hold walk to `335,270` with `jumps=[]`; `r323-web-packet-run-right.json` verifies repeated raw `Run Right` packets also reach `340,270`. Known non-movement warning in the captures: missing `original-ui/NPC/25/meta.json` still produces a 404. Remaining movement acceptance is human feel comparison against original Crystal, plus deeper collision/blocked-tile edge parity.
- 2026-05-01 R321 original/Web movement-control baseline: added `apps/web/scripts/capture-original-movement.ps1` so automation can bring the original Crystal `Legend of Mir 2` window to the foreground, send client-coordinate left/right mouse actions, and archive timed client-area screenshots next to Web movement evidence. R321 evidence under `docs/generated/player-qa/movement-jitter/` confirms original control captured 15 frames at 1024x768 for `QA0429Hero`, Web direct gateway `moveTo` moved cleanly through run/walk steps with `jumps=[]`, and Web real tile click/right-click had `jumps=[]`. A real hit-test bug was fixed by moving `.tile-hit` with the same `playerCameraMotionOffset` as the rendered map/entities; after the fix `r321-web-movement-click-hitoffset.json` shows the first right-click immediately advances from `330,270` to `332,270`. Crystal source comparison confirms the original client drives movement from `GameScene.Process` 100ms `CanMove` ticks plus `PlayerObject.SetAction()` consuming `QueuedAction`, immediately updating `CurrentLocation` and using server packets as correction; Web still uses a simplified target-plan loop, so the remaining movement-feel issue is continuous input/action-queue parity rather than DOM jitter or backend traversal failure.
- 2026-04-30 R319 label/BigMap/Mail/cursor parity pass: tightened the latest user-reported visual gaps against Crystal source. Entity nameplates now keep Crystal-style object-centered labels: NPC/monster underscore names split into stacked lines (`Teleport` / `Gilbert`) instead of web-normalized prose, and selected target HP/action hints no longer enlarge the name label. BigMap NPC rows now use the whole-map Crystal NPC manifest, render `MapLinkIcon` sprites, and format names like `(Teleport)Gilbert`; Mail empty state no longer shows Web `No mail`; and the client stage uses Crystal cursor files (`Cursor_Default.CUR`, `Cursor_Npc.cur`, `Cursor_Normal_Atk.CUR`, `Cursor_TextPrompt.CUR`). Evidence: `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final.png` and `docs/generated/player-qa/r319-label-bigmap-mail-cursor/r319-label-bigmap-mail-cursor-final-state.json`; state records `mailPanel.emptyVisible=false`, `bigMap.npcRowCount=18`, first BigMap rows `(Teleport)Gilbert`, `(BorderVillage)Board`, `(Assistant)Jane` with `/original-ui/MapLinkIcon/*.png`, Crystal `.CUR` cursor CSS for stage/NPC/monster hits, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture script syntax check, and focused CDP capture with `--openMail true --openBigMap true`.
- 2026-04-30 R318 BigMap/MailList parity pass: fixed the user-reported BigMap and Mail UI mismatch by replacing Web-style surfaces with Crystal source-aligned dialogs. The minimap BigMap button now toggles a real `BigMapDialog` using exported `Title/820`, `Title/821-829`, `Prguse2/1340-1342`, `Prguse2/1350`, and world-map `Prguse2/1360/1365/1366` assets; Web stores `MapInformation.bigMapIndex` and draws the current Crystal big-map raster with source dialog controls, title, coordinate label, NPC rows, and radar dots. The Mail button now opens the Crystal `MailListDialog` frame `Title/670` at the original 1024x768 position with `Title/7`, close/help/page/action buttons, 10-row layout, row icons/flags, and no visible Web overlay header. Evidence: `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final.png` and `docs/generated/player-qa/r318-mail-bigmap/r318-mail-bigmap-final-state.json`; state records `mailPanel.bounds=562,5,312,444`, `mailPanel.hasFrame=true`, `mailPanel.visibleOverlayHead=false`, `mailPanel.oldOverlayRowCount=0`, `bigMap.bounds=132,134,760,500`, `bigMap.viewport=146,186,568,380`, `bigMap.hasFrame=true`, `bigMap.hasRaster=true`, `bigMap.title="BichonProvince"`, `bigMap.coordinate="[ 287, 618 ]"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: UI asset export, web `tsc --noEmit`, capture/smoke script syntax checks, focused CDP capture with `--openMail true --openBigMap true`, and `git diff --check`.
- 2026-04-30 R317 Gameshop product-grid parity pass: replaced the remaining Web placeholder Gameshop interior with Crystal-backed product data and original cell/button assets. Web now renders the 105-item generated Crystal Gameshop manifest through the original `Title/750` cell frame, `Title/778-783` buy/preview buttons, `Prguse2/240-245` quantity controls, real category filters, class tabs, search, page labels, payment checkboxes, stock/count/price labels, and item icons exported from `Items.Lib`. Evidence: `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products.png` and `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json`; state records `gameShop.bounds=164,70,696,476`, `cellCount=8`, `firstCellName="AccuracyPotion"`, `pageLabel="1 / 14"`, `categoryCount=10`, `loadedIconCount=8`, `buyButtonCount=8`, `previewButtonCount=1`, `oldPlaceholderCellCount=0`, `inventoryVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax check, R317 CDP capture with `--openGameShop true`, UI asset export, and `git diff --check`.
- 2026-04-30 R316 Gameshop/Menu parity pass: fixed the user-reported HUD Gameshop/Menu mismatch. Crystal source confirms the HUD Gameshop button toggles `GameShopDialog` and the Menu button toggles the narrow `MenuDialog`; Web had Gameshop incorrectly wired to `onOpenInventoryTab("quest")` and rendered Menu as a large debug/transfer panel. Web now opens a Crystal-framed `GameShopDialog` shell from the Gameshop button without opening Inventory, and Menu renders the exported `Title/567` 36x282 vertical icon strip with 13 original sprite buttons at Crystal offsets. The QA transfer form is still available to automation but is moved offscreen so it no longer appears as the normal player menu. Exported missing Crystal UI assets include Gameshop frame/tabs/buttons and Menu frame/icon triples. Evidence: `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-open.png`, `docs/generated/player-qa/r316-gameshop-menu/r316-menu-open.png`, and `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json`; state records `shopVisible=true`, `inventoryVisible=false`, `shopBounds=164,70,696,476`, `menuBounds=988,349,36,282`, `iconCount=13`, `oldOverlayHeadVisible=false`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: web `tsc --noEmit`, capture-script syntax check, focused CDP click capture, and `git diff --check`.
- 2026-04-30 R315 empty new-character panel-state pass: closed the data-state cause behind the latest character/equipment, inventory, spells, quest, and storage screenshots. Crystal source confirms new character arrays for inventory/equipment/quest inventory/magics are empty and account gold/storage start empty unless real `StartItems` exist. Web runtime now creates real `NewCharacter` saves with empty bag/belt/storage/equipment/quest/skill state and `gold=0`, no longer treats empty save arrays as a signal to refill Web demo seeds, migrates old level-1 exact Web seed saves to empty Crystal state, and preserves `demo/Scout` Stage 5 seed data for automation. Character Spells no longer fills empty magic rows with Web hints/buffs, and the web-only repair/special-repair buttons were removed from the Character page. Evidence: `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels.png` and `docs/generated/player-qa/r315-empty-new-character-panels/r315-empty-new-character-panels-state.json`; state records `gold=0`, `inventoryItemCount=0`, `beltItemCount=0`, `storageItemCount=0`, `equipmentItemCount=0`, `questCount=0`, `skillCount=0`, `hudHealthOnlyLabel="HP 18/18"`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: focused `mir2-simulation start_game_` 16/16, `mir2-gateway` build, web `tsc --noEmit`, R315 CDP capture, `fmt --check`, and capture-script syntax check. Remaining panel visual work is exact Quest Diary/Storage bitmap layout and character paperdoll base sprite details.
- 2026-04-30 R314 HUD/chat/belt/vitals parity pass: closed the user-reported same-scene HUD text/value gap for the aligned Bichon comparison. Web now uses Crystal `MainDialog` HP-only behavior for low-level Warriors with exported `Prguse` frame 6, renders the HP label as `HP current/max`, keeps the Crystal bitmap orb fill from R311, and layers the belt bar from `Prguse` 1932 plus the 0.5-opacity 1933 overlay with 32px slots at Crystal offsets. Chat now follows Crystal's 4 visible rows, 13px row height, Arial 8pt-like sizing, white/blue/red row backgrounds, and Crystal-style hint/server colors. The backend default/legacy-save vitals now come from Crystal `BaseStats` formulas, so `QA0429A / QA0429Hero` at level 1 records `playerHp=18`, `playerMaxHp=18`, `playerMp=14`, and `hudHealthOnlyLabel="HP 18/18"`. Evidence: `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud.png` and `docs/generated/player-qa/r314-crystal-vitals-hud/r314-bichon-287-618-vitals-hud-state.json`, with exact 1024x768 stage/HUD bounds, 4 chat lines, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Verification passed: focused `mir2-simulation start_game_` 15/15, `cargo +1.89.0 build --locked -p mir2-gateway`, web `tsc --noEmit`, R314 CDP capture, `cargo +1.89.0 fmt --check`, and `git diff --check`.
- 2026-04-30 R312 entity projection/nameplate source alignment: reconciled the R311 same-scene framing change against Crystal source. Web map floor/object sprites keep Crystal `MapControl` map-layer math, while entity sprites/nameplates now use the Crystal `DrawLocation` origin (`OffSetX * 48`, `OffSetY * 32`) and `DisplayRectangle`-relative name/health placement instead of centering entity stacks on tile centers or applying a web-only self-nameplate offset. The vertical viewport offset is restored to Crystal's `Settings.ScreenHeight / 2 / CellHeight - 1` formula, and the focused capture at `BichonProvince` map `0`, `287,618` records `QA0429Hero` nameplate `top=275` with exact `1024x768` stage bounds, HUD `0,616,1024,768`, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Evidence: `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor.png` and `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json`. R312 supersedes the R311 playfield-centered camera experiment for projection math; the R311 Crystal bitmap HUD orb fill remains in place.
- 2026-04-30 R311 playfield camera/HUD orb cleanup: the Web Bichon comparison camera now centers on the Crystal playfield height above the 152px HUD instead of the full 768px client stage, moving `QA0429Hero` from the R310 web nameplate `top=389` to `top=325` at `BichonProvince` map `0`, `287,618`, much closer to the original-client same-scene framing. The main HUD HP/MP orb fill now uses the exported Crystal `Prguse` frame 4 left/right orb halves instead of CSS gradients; `Prguse` frames 4 and 6 were added to the UI export manifest. Evidence at `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json` records exact `1024x768` stage bounds, `hud=0,616,1024,768`, `questMarkerCount=0`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshots are `r311-bichon-287-618-playfield-camera.png` and `r311-bichon-287-618-hud-orb.png`. This reduces the user-reported same-scene visual mismatch; remaining visual acceptance still includes exact dynamic placement, lighting/effects, chat/HUD text feel, and human final acceptance.
- 2026-04-29 R310 original/Web visual-watch bootstrap: added repeatable same-scene Web capture at `apps/web/scripts/capture-crystal-parity.mjs` and a six-hour-capable original/Web sampler at `apps/web/scripts/r310-visual-watch.ps1`. R310 fixes the login-success transition leak so the Mir login animation is cleared before `screen=game`, and scopes NPC quest markers to NPCs whose server snapshot `questIds` match the active quest instead of painting every NPC. Evidence at `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618` with `transitionOverlayVisible=false`, `questMarkerCount=0`, `stage=0,0,1024,768`, `hud=0,616,1024,768`, `miniMap.right=1024`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`; screenshot evidence is `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene.png`. A one-sample original/Web watch test wrote `watch-20260429-042013-original.png`, `watch-20260429-042013-web.png`, and `r310-visual-watch-log.jsonl` with no errors. This is automated comparison evidence only; human final visual acceptance remains open.
- 2026-04-29 R309 Bichon minimap/HUD bounds cleanup: the aligned Bichon desktop minimap frame no longer overflows the 1024x768 Crystal-size stage by 2px. `.mini-map-panel` now sits at `left=896`, `right=1024`, `width=128` in desktop evidence, while compact `820x640` evidence keeps minimap and core HUD bounds inside the viewport. Evidence at `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` records `desktopOverflows=[]`, `compactOverflows=[]`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`; screenshots are `docs/generated/player-qa/r309-minimap-bounds-web-page.png` and `docs/generated/player-qa/r309-minimap-bounds-compact-web-page.png`. This closes the measured minimap boundary overflow; exact dynamic animal density/placement and human visual acceptance remain open.
- 2026-04-29 R308 Bichon viewport/resource scale cleanup: the web game stage no longer applies the 0.9 browser-only downscale at original comparison sizes, the outer page/frame background is now plain black with no decorative shadow, and compact scaling is reserved for viewports smaller than the 1024x768 Crystal client frame. The same Bichon comparison point also now has exported original sprite meta/assets for previously missing `NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, and `Monster/005`. Evidence at `docs/generated/player-qa/r308-stage-scale-web-page-state.json` records `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`; desktop stage bounds are exactly `0,0,1024,768` with transform scale 1, compact bounds are `798.72x599.04` inside `820x640`, `nonFaviconNetwork404s=[]`, and `consoleErrors=[]`. Screenshots: `docs/generated/player-qa/r308-stage-scale-web-page.png` and `docs/generated/player-qa/r308-stage-scale-compact-web-page.png`. This closes the browser-only stage-scale/frame decoration gap for the comparison viewport and removes the visible-object sprite 404s; exact dynamic animal density/placement and human visual acceptance remain open.
- 2026-04-29 R307 Bichon guard/archer comparison point evidence: added a focused simulation regression for the aligned Bichon `0:287,618` comparison point so imported fixed respawns must expose `Guard` at `291,620` and `ArcherGuard` at `295,624` through both `ObjectMonster` packets and `worldSnapshot`. Browser evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png` plus `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `hasGuard=true`, `hasArcherGuard=true`, `monsterCount=7`, `npcCount=5`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `287,618`. Verified with focused `mir2-simulation` regression and CDP browser capture with zero console errors. This closes the ordinary Guard/ArcherGuard visibility evidence at the user's second Bichon comparison point; exact dynamic animal density/placement, HUD scale/letterboxing, and human visual acceptance remain open.
- 2026-04-29 R306 Bichon same-scene display cleanup: default game view no longer renders the web-only left quest tracker panel over the Crystal playfield, and NPC/monster nameplates now display Crystal-style space-separated names while keeping raw runtime entity names unchanged for packets/tests. Browser evidence at `docs/generated/player-qa/r306-bichon-display-web-page.png` plus `docs/generated/player-qa/r306-bichon-display-web-page-state.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, `npcSpriteElementCount=8`, `monsterSpriteElementCount=8`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false` for `QA0429A / QA0429Hero` at `BichonProvince` map `0`, `284,607`. Verified with web `tsc --noEmit` and CDP browser capture with zero console errors. Exact object placement/density, HUD scale/letterboxing, and human visual acceptance remain open.
- 2026-04-29 R305 Bichon same-scene visible respawn fix: current-map Crystal visible respawns now enter the ECS world snapshot, not only the bootstrap `ObjectMonster` packet stream. Same-coordinate WS evidence at `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json` records `entityCount=17`, `npcCount=8`, `monsterCount=8`, including `Deer`, `Scarecrow`, `Hen`, and two `Royal_Guard` entities around `QA0429Hero` at `0:284,607`. Browser evidence at `docs/generated/player-qa/r305-bichon-visible-web-page.png` plus `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` records `npcSpriteElementCount=8` and `monsterSpriteElementCount=8`. Verified with focused R305 simulation regression, the existing visible-respawn density regression, `fmt --check`, `mir2-gateway` build, live WS probe, browser state/screenshot capture, gateway health, and web HTTP 200. This closes the first obvious Deer/Royal Guard gap from the Bichon screenshots; broader visual 1:1 remains open for exact density, ordinary guard/archer placement, name normalization, quest tracker/HUD scale, and human screenshot acceptance.
- 2026-04-29 R304 Bichon same-scene NPC population fix: starting a saved web character on a real Crystal map now rebuilds the runtime world from the current map and instantiates Crystal NPC-info manifest entries before the web snapshot is emitted. Same-coordinate WS verification for `QA0429A / Mir2test1 / QA0429Hero` at `BichonProvince` map `0`, `284,607` is archived at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json`: `entityCount=9`, `npcCount=8`, and `Assistant_Jane` plus `Merchant_Ruben` are present. Browser CDP evidence is archived at `docs/generated/player-qa/r304-bichon-npc-web-page.png` plus `docs/generated/player-qa/r304-bichon-npc-web-page-state.json`; the page state records `npcCount=8`, `npcSpriteElementCount=8`, and visible nameplates for the expected Bichon NPCs. Verified with focused/adjacent `mir2-simulation` tests, `cargo +1.89.0 fmt --check`, `cargo +1.89.0 build --locked -p mir2-gateway`, a live WS probe against gateway `127.0.0.1:7110`, and browser state/screenshot capture against `http://127.0.0.1:3002`. R305 later added visible respawns; visual 1:1 still remains open for exact object density, NPC display-name normalization, quest tracker overlay, HUD scale/letterboxing, and human screenshot acceptance.
- 2026-04-29 login/select audio bootstrap: exported Crystal `Sound/Login2.wav`, `Sound/Select2.wav`, and `Sound/100.wav` into the web public assets. Web now loops login music on the login scene, keeps it through the login-success transition while playing the login effect, then switches to select music. Browser autoplay may defer the first play until a user click/key gesture. Verified with `.\node_modules\.bin\tsc.cmd --noEmit` from `apps\web` plus HTTP 200 checks for all three WAV assets.
- 2026-04-29 login transition fix: web login now holds the first `ChrSel` frame while idle and plays the 19-frame login transition once when leaving the login screen after successful entry. Verified with `.\node_modules\.bin\tsc.cmd --noEmit` from `apps\web`; this does not close Crystal pixel/feel acceptance.
- 2026-04-29 same-scene manual-comparison setup: original Crystal `QA0429A / Mir2test1 / QA0429Hero` is mirrored into the web account store, with the web character aligned to `BichonProvince` map `0` at `287,618`. Frontend CDP verification reached `screen=game`, `mapFileName=0`, `mapTitle=BichonProvince`, `player=287,618` and archived `docs/generated/player-qa/latest-web-align-qa0429a.png` plus `docs/generated/player-qa/latest-web-align-qa0429a-frontend.json`. This opens an apples-to-apples human comparison point; it does not close visual 1:1 because NPC/monster population, quest panel visibility, outer scale/letterboxing, and HUD details still need judgment/fixes.
- 2026-04-29 R303 map-resource audit: `npm.cmd run audit:crystal-map-coverage --prefix apps\web` wrote `docs/generated/map/latest-crystal-map-coverage.json` and archived `docs/generated/map/r303-crystal-map-coverage.json`. This first audit confirmed 463/463 Crystal manifest maps had local source map files, 0 unsupported map types, 0 parse errors, and 463/463 sampled viewports with source frames. Its then-open source-frame and minimap warnings are superseded by the 2026-05-16 all-map resource/gameplay audit above.
- 2026-04-28 R302 original-client comparison: original Crystal `Server.exe` and visible `Client.exe` were launched locally, a retained Crystal QA character was created, and select/game screenshots were archived under `docs/generated/player-qa/r302-original-client/`. Web Stage 5 UI smoke was refreshed from `http://127.0.0.1:3002` with 88 screenshots and 0 critical console errors. R302 confirms original-client visual-reference capture is possible; it does not close the frontend rows because same-scene visual/feel acceptance is still human-blocked or must be explicitly accepted.
- 2026-04-28 R301 automation refresh: final Candidate acceptance pack passed and is summarized in `docs/generated/player-qa/r301-summary.json`. Web `tsc --noEmit`, web build, map API smoke 18/18 with 0 failures, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64 ready with 0 errors, and Stage 5 UI smoke 88 screenshots with 0 critical console errors all passed. The Stage 5 manifest checked 32 compact text nodes with no overflow. Frontend rows below remain Candidate/human-acceptance rows; R301 does not close the final Crystal visual/feel pass.
- 2026-04-28 R300 parity context: backend/server tracked-slice packet parity is now accepted under explicit stable-diff packet acceptance. Frontend rows below remain Candidate/human-acceptance rows; R300 does not close the final Crystal visual/feel pass.
- 2026-04-26 R225 regression refresh: `smoke:stage5-ui` still captures 88 screenshots and now writes manifest summary counts (8 compact panel bounds, 34 compact text nodes, 0 critical console errors, major flow counts). Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, Rust package regressions, `fmt --check`, and `diff --check` passed. The remaining frontend rows below are Candidate/human-acceptance rows, not unverified automatable gaps on this Mac.
- 2026-04-26 R224 integration evidence: `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`. Frontend/global automation remains **100% Candidate**; 100% Accepted still requires human Crystal visual/feel acceptance.
- 2026-04-26 R223 Candidate evidence: `smoke:stage5-ui` now captures 88 screenshots and records advanced Stage 5 systems state plus compact Mail/Report panel bounds. Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, full Rust package regressions, `fmt --check`, and `diff --check` passed.
- `npm.cmd run build`
- `npm.cmd run audit:crystal-map-coverage`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run load:gateway-ws`
- screenshot manifest: `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- load evidence: `docs/generated/load/latest-ws.json`, `docs/generated/load/latest-tcp.json`
- map/API evidence: `docs/generated/map/latest-crystal-map-api.json`
- all-map source-resource audit evidence: `docs/generated/map/latest-crystal-map-coverage.json`
- minimap asset evidence: `docs/generated/assets/latest-minimap-assets.json`
- 2026-04-26 R184 evidence: direct `next build`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui` (10 screenshots), and `load:gateway-ws` 64/64 ready passed locally on macOS with gateway on `127.0.0.1:7110`.
- 2026-04-26 R185 evidence: `smoke:stage5-ui` now captures 11 screenshots across desktop 1024x768 and compact 820x640 viewports, writes viewport metadata and compact layout bounds to `stage5-ui-smoke-manifest.json`, and includes `stage5-compact-game.png`.
- 2026-04-26 R186 evidence: `smoke:stage5-ui` now checks 33 visible compact text nodes for overflow, writes `compactTextLayout`, and the compact minimap title/Safe Zone label is fixed.
- 2026-04-26 R187 evidence: `smoke:stage5-ui` now captures 14 screenshots, exercises minimap collapse, BigMap re-expand, and Mail open paths, and writes `minimapFlow`.
- 2026-04-26 R188 evidence: `smoke:stage5-ui` now captures 17 screenshots, exercises belt rotate/close states, writes `beltFlow`, and asserts belt labels stay in-bounds without Quest overlap.
- 2026-04-26 R189 evidence: `smoke:stage5-ui` now captures 18 screenshots, presses belt hotkey `1`, verifies Red Potion quantity drops from 5 to 4, and writes `beltUseFlow`.
- 2026-04-26 R190 evidence: `smoke:stage5-ui` now captures 21 screenshots, switches inventory bag1/bag2/quest/bag1, and writes `inventoryFlow`.
- 2026-04-26 R191 evidence: `smoke:stage5-ui` now captures 25 screenshots, switches character char/stats1/stats2/spells/char, and writes `characterFlow`.
- 2026-04-26 R192 evidence: `smoke:stage5-ui` now captures 27 screenshots, switches storage page1/page2-locked/page1, and writes `storageFlow`.
- 2026-04-26 R193 evidence: `smoke:stage5-ui` now captures 31 screenshots, exercises chat Shout filter, All restore, Settings, collapse/restore, and Report paths, and writes `chatFlow`.
- 2026-04-26 R194 evidence: `smoke:stage5-ui` now captures 35 screenshots, opens the system menu, routes Character/Inventory/Quest actions, and writes `systemMenuFlow`.
- 2026-04-26 R195 evidence: `smoke:stage5-ui` now captures 36 screenshots, rents expanded storage from locked page 2, verifies unlocked page 2 plus 160-slot capacity, and writes the rented state into `storageFlow`.
- 2026-04-26 R196 evidence: `smoke:stage5-ui` now captures 37 screenshots, clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, and writes `inventoryUseFlow`.
- 2026-04-26 R197 evidence: `smoke:stage5-ui` now captures 38 screenshots, clicks Dagger from inventory bag1, verifies it moves into the weapon equipment slot, and writes `inventoryEquipFlow`.
- 2026-04-26 R198 evidence: `smoke:stage5-ui` now captures 40 screenshots, opens Character Spells from HUD Skill and Stats II from HUD Option, and writes `hudButtonFlow`.
- 2026-04-26 R199 evidence: `smoke:stage5-ui` now captures 42 screenshots, opens Drop Gold, confirms 100 gold, verifies gold drops from 1280 to 1180 plus a ground-drop label, and writes `inventoryGoldFlow`.
- 2026-04-26 R200 evidence: `smoke:stage5-ui` now captures 43 screenshots, context-clicks Wooden Sword in bag1, verifies it moves from slot 4 to slot 10, and writes `inventoryMoveFlow`.
- 2026-04-26 R201 evidence: `smoke:stage5-ui` now captures 45 screenshots, opens Split Item for Red Potion, verifies the split stack lands in the belt while total Red Potion quantity is preserved, and writes `inventorySplitFlow`.
- 2026-04-26 R202 evidence: `smoke:stage5-ui` now captures 47 screenshots, opens Delete Item for Blue Potion, verifies quantity drops from 3 to 2 plus a ground-drop label, and writes `inventoryDropFlow`.
- 2026-04-26 R203 evidence: `smoke:stage5-ui` now captures 48 screenshots, verifies Character Dagger removal back to bag1 slot 4, fixes RemoveItem target/grid wiring, and writes `characterRemoveFlow`.
- 2026-04-26 R204 evidence: `smoke:stage5-ui` now captures 49 screenshots, clicks Red Potion directly in the belt, verifies quantity drops from 5 to 4 before hotkey `1` drops it from 4 to 3, and writes `beltMouseUseFlow`.
- 2026-04-26 R205 evidence: `smoke:stage5-ui` now captures 51 screenshots, opens Sell Item for Dagger, confirms without active sell service, verifies Dagger/gold are preserved, and writes `inventorySellFlow`.
- 2026-04-26 R206 evidence: `smoke:stage5-ui` now captures 54 screenshots, opens Store Item for Dagger, selects a warehouse slot without active storage service, verifies Dagger/storage contents are preserved, and writes `storageStoreFlow`.
- 2026-04-26 R207 evidence: `smoke:stage5-ui` now captures 57 screenshots, opens Take Back for stored Red Potion, selects an inventory slot without active storage service, verifies inventory/storage quantities are preserved, and writes `storageTakeBackFlow`.
- 2026-04-26 R208 evidence: `smoke:stage5-ui` now captures 58 screenshots, opens/closes Set Storage Password without submitting credentials, verifies panel state, and writes `storagePasswordFlow`.
- 2026-04-26 R209 evidence: `smoke:stage5-ui` now captures 60 screenshots, fills Set Storage Password, verifies mismatch disables submit, submits matching `Safe123` without active storage service, verifies no password is set with no-service feedback, and extends `storagePasswordFlow`.
- 2026-04-26 R210-R218 evidence: `smoke:stage5-ui` now captures 71 screenshots, records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds.
- 2026-04-26 R219-R222 evidence: `smoke:stage5-ui` now captures 85 screenshots, records login/select lifecycle flows, compact inventory/storage/character/system-menu/chat-settings bounds, and existing broad gameplay/system flows. Map API smoke writes 18/18 successful requests, minimap asset smoke writes 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, and WS load refresh reports 64/64 ready with 0 errors.
- 2026-04-26 R223 evidence: `smoke:stage5-ui` now captures 88 screenshots, records advanced Stage 5 systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state, and adds compact Mail/Report panel bounds.

## Open Gap Matrix

| Status | Area | Gap | Evidence Needed |
| --- | --- | --- | --- |
| [~] | Login/select | Language switching, View Key, Enter-key login submit, Credits, Delete cancel, New Character, confirmed Delete Character, recreate, slot selection, Start, login music, login effect, and select music are implemented or smoke-verified; pixel/audio comparison against Crystal login/select screens still open | screenshots and human acceptance |
| [~] | Game shell | First viewport now has desktop/compact automated route screenshots; R303 confirms all 463 manifest map files are source-present/parseable for sampled frontend loading; R304 proves the aligned Bichon web runtime snapshot includes current-map Crystal NPCs; R305 proves the same view includes first-pass visible respawns; R306 removes the default quest tracker overlay while normalizing visible NPC/monster nameplates; R307 proves the second Bichon comparison point includes ordinary Guard plus ArcherGuard; R308 removes browser-only original-size stage downscaling/frame decoration while exporting the missing Bichon NPC/animal sprite libs; R309 removes the measured desktop minimap 2px overflow; R310 clears the login transition before game capture while removing over-broad NPC quest markers; R311 adds Crystal bitmap HUD orb fills; R312 restores Crystal source projection anchors for entity sprites/nameplates/health bars; the 2026-05-08 pass aligns Bichon torch/fire blend frames back onto the red torch head; the 2026-05-14 full-resource sync closes minimap asset coverage; and the 2026-05-16 all-map audit closes automated source/fallback risk with Crystal no-draw frame classification. Remaining open items are exact dynamic density/placement and human Crystal-like visual judgment | screenshot comparison at accepted viewports |
| [~] | HUD/chat | Crystal chat control-bar semantics are now smoke-verified: All/Shout/Whisper/Lover/Mentor/Group/Guild set outgoing prefixes, Trade sends `tradeRequest`, Settings owns channel visibility plus transparency, Size collapses/restores, Report opens/closes, hit boxes are 24x13 and topmost over the HUD; latest-line auto-follow, scroll-knob behavior, 4-line Crystal chat feed, and bitmap HP/MP orb fills are also implemented or verified. Remaining panel-level acceptance is Crystal visual/feel comparison, especially exact text placement and HUD/chat feel | targeted chat-control smoke, UI smoke/capture passed; human pass remains |
| [~] | Belt | Slots 1-6, rotate/close, occupied/empty visuals, in-bounds labels, no Quest overlap, mouse Red Potion use, and hotkeys `1..6` are smoke-verified; consumable hotkeys decrement and empty/non-consumable slots are recorded as no-op coverage. Full Crystal feel remains open | automated command path plus human pass |
| [~] | Minimap | Compact map title/Safe Zone text no longer overflows, collapse/BigMap re-expand/Mail open paths are smoke-verified, and the 2026-05-14 full-resource sync exports minimap ids 450/451 from Crystal database preview BMPs; direct Crystal visual comparison remains open | smoke plus screenshot comparison |
| [~] | Inventory | bag1/bag2/quest tabs, Red Potion item use/split, Blue Potion item drop, Dagger equip/remove, Sell Item no-service preserve, service-backed Store Item, service-backed Take Back, Drop Gold, and Wooden Sword move are smoke-verified with screenshots and state evidence; item merge/full service-backed sell and deeper panel acceptance remain open | UI route plus backend packets |
| [~] | Character | char/stats1/stats2/spells tabs, known skill display, HUD Skill/Option button routes, Battle Focus cast/buff/cooldown, Dagger equipment remove, equipped-slot normal repair through Blacksmith_Smith `@Repair`, equipped-slot special repair through Blacksmith_Bill `@SRepair`, durability restoration/max-loss behavior, gold deduction, and corrected Belt/Boots equipment-slot ids are smoke/test-verified; deeper paperdoll and durability UI feel acceptance remains open | screenshot plus interaction route |
| [~] | Combat/skills/effects | Battle Focus cast/buff/cooldown, targeted combat, live Crystal magic packets, projectile packets, buff add/remove/pause deltas, map-effect fallback, and Crystal-like visual action timing are smoke/type-verified; remaining work is full per-class skill visual/effect fidelity and human feel comparison against Crystal | packet/event smoke plus human visual pass |
| [~] | NPC/shop/storage | storage page 1, locked expanded page 2, expanded-storage rent/unlock, restored page 1, InnKeeper_Brittney `@Storage` service open through the real Crystal dialog path, service-backed Dagger store, service-backed Red Potion take-back, Blacksmith `@Repair` / `@SRepair` service-backed repairs, GameShop Gold buy with carry-slot delivery, NPC dialog markup sanitization, and dialog link rendering are smoke/build-verified; input, sell/craft/refine panels and GameShop preview still need Crystal comparison | route screenshots and packet trace |
| [~] | Storage password | expanded storage confirmation, Set Storage Password panel entry, mismatch validation, and service-backed set/unlock/change/remove password flows are smoke-verified through InnKeeper_Brittney `@Storage`, including last-set timestamp exposure; exact Crystal dialog bitmap and invalid-password edge visual acceptance remain open | UI route and persistence check |
| [~] | Quest/mail/report/menu | Mail open/close state, real Mail claim for seeded GameShop gold, nested-button-free Mail row DOM, Report open/close state, compact Mail/Report bounds, system menu QA Jump, transfer-list routing, trade chat filtering, dynamic social panels, Quest Diary title/stage/progress/reward rows, and repo-stable Stage 5 smoke screenshots are verified. R316 replaces the visible Web debug menu with Crystal `MenuDialog` frame `Title/567` and icon buttons, and Gameshop now opens the Crystal-framed dialog instead of Inventory/Quest. R317 replaces Gameshop placeholder cells with Crystal product manifest data, original cell/buttons, item icons, categories, pagination, prices, stock, and payment controls. Quest/mail/report exact dialog bitmaps and service-backed Gameshop preview behavior still need Crystal-like layout and interaction review | screenshot and human pass |
| [~] | Scene interaction | tile buttons avoid scene pointer double-dispatch and now track the same camera motion offset as rendered map/entities; added-stat ground drops render with server-provided Crystal Cyan name colour; selected scene targets route keyboard approach/primary actions; Blue Potion and gold ground pickup plus combat target selection are smoke-verified; R321 can now control original Crystal and Web for movement comparison; R322 prevents prediction/server-correction movement loops by making prediction visual-only and stopping plans on server correction; R323 adds held-mouse 100ms input sampling with direct `Walk` / `Run` direction packets for Crystal-like queued movement; R325 fixes held-run visual background backtracking by synchronizing predicted render basis and motion snapshots; R326 keeps the latest held-direction request queued during the 600ms action window and consumes it like Crystal `QueuedAction`; the 2026-05-07 movement/animation pass anchors player/monster/NPC frame cycles to Crystal action timing; the 2026-05-08 movement pass ties displacement to Crystal six-frame/even-pixel `OffSetMove` cadence and caps held-direction prediction to one unconfirmed action; the 2026-05-09 direction/queue pass carries predicted facing with predicted coordinates, keeps target prediction until the server reaches the target, and avoids click-route rollback in headless captures; the 2026-05-22 production pass clears confirmed ACK actions from the local movement feed and keeps the motion clock live under headless throttling; the 2026-05-23 production Crystal action-queue pass drives self Walk/Run/Turn through local `QueuedAction`/ActionFeed ACK/correction semantics with strict production walk/run evidence green. Remaining movement acceptance is final human feel plus broader blocked-tile/collision edge parity | route replay and human pass |
| [~] | Responsive/layout | R308 records exact 1024x768 stage bounds at original comparison size and keeps the 820x640 compact stage inside viewport bounds; R309 records minimap/core HUD bounds with no desktop or compact overflow; compact inventory/storage/character/system-menu/chat-settings/Mail/Report/social bounds are smoke-verified; the current compact matrix covers 900x640, 768x640, and 820x540. The 2026-05-19 mobile landscape pass adds nipplejs joystick controls, Mir2 direction/run semantic mapping, a right-bottom circular action wheel, a portrait rotate prompt, and strict mobile movement smoke evidence. The 2026-05-22 production pass dynamically scales the 1024x768 stage to the actual viewport and verifies a 150x647 DevTools-width login/game path with stage bounds inside the viewport. Broader human mobile feel, especially sustained high-frequency run/turn behavior, remains open | screenshot checks |
| [~] | Language/text | Compact visible core quest/HUD/minimap/belt/chat/entity/mail/storage/system/social text is smoke-checked with overflow-safe selectors and CSS; login/select language switches remain smoke-covered, while full language-by-panel visual acceptance remains open | screenshot and DOM checks |

Candidate note: as of R301, all rows above have automated evidence for the available route. They intentionally remain `[~]` until direct Crystal screenshots/live comparisons or human visual/feel acceptance close them; automation should not flip them to `[x]` by itself.

## Recent Frontend Fixes

- 2026-07-13: Crystal asset semantic-source phase 1 now preserves `.Lib` v3 `FrameSet` records instead of skipping the header seek. The shared parser exposes all original action fields (`Start`, `Count`, `Skip`, `Interval`, effect fields, `Reverse`, and `Blend`), validates truncated/corrupt ranges, and the UI exporter now writes this FrameSet into per-library and aggregate metadata while selective exports merge existing libraries instead of erasing the manifest. A deterministic Source Snapshot streams SHA-256 over the full local Crystal Data tree without decoding image payloads. Full-source evidence at `docs/generated/assets/crystal-source-snapshot.generated.json`: 1,440/1,440 libraries parsed, 7,638,253,548 source bytes, 2,143,132 frame slots, 585 v2 plus 855 v3 libraries, 703 non-empty FrameSets, 3,643 actions, and zero parse failures, invalid offsets, unknown actions, duplicate actions, or reported issues; two consecutive full generations produced byte-identical file SHA-256 `C3480F6689CF27C3CECC81ED86787BEBF5283B089B58644529E76E4AA09197F9`. Focused `test:crystal-library`, legacy `test:magic-effect-export`, syntax checks, synthetic selective-export merge, and a real `NPC/00.Lib` export passed. This closes source/metadata preservation only; Bevy runtime consumption of generated FrameSets, masks, and effect recipes remains open.
- 2026-05-24: Production movement visual closeout deployed as Player Web `dpl_8wQigG43KBLpaZY5oPPWHwNhz3QK`. Rapid discrete keyboard taps now carry a bounded same-direction input debt across server ACK latency, so six quick D taps send six walk packets and receive six ordered `UserLocation` ACKs instead of collapsing to two steps. The original-map renderer now keeps a textured floor fallback under still-loading map tiles and alpha-keys black-background map Object images before showing them, preventing the black rectangle flash that made movement feel broken even after the packet path was correct. Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`; scoped diff check; Vercel prebuilt build/prune/deploy; custom-domain `/health`; production `docs/generated/player-qa/movement-jitter/prod-underlay-keyboard-d-20260524T112642.json`; and headed Chrome `docs/generated/player-qa/movement-jitter/prod-underlay-headed-keyboard-d-20260524T112744.json`, both `ok=true` with no visual jumps, no route spam, no logical rollback, no scene blackouts, no console errors, no non-favicon 404s, and screenshot evidence without black map holes.
- 2026-05-23: Crystal action-queue movement pass landed across Web/Gateway/Simulation and was production-verified. Player Web now treats self `UserLocation` as an ordered ACK/correction surface for local `QueuedAction`/ActionFeed state, not as a fresh walk/run animation; correction snaps clear packet motion, same-tile confirmations preserve the active animation, and server `UserLocation` no longer re-seeds predicted motion when it is only confirming the local queue. Packet Walk/Run rendering now uses one Crystal 600ms action window even for two-tile Run, matching the original sprite cadence instead of stretching Run over two tiles. Backend Zone consumes bounded ordered Walk/Run/Turn actions on Crystal `ActionTime`; the later local rollback correction changes raw standstill Run from an origin correction into an effective one-tile Walk. The production follow-up capped local ActionFeed lead to two tiles and treats non-matching `UserLocation` as correction, closing the residual visual jump found after the first deploy. Evidence: Web `pnpm --dir apps/web exec tsc --noEmit --pretty false`; Web `pnpm --dir apps/web exec next build`; Simulation/Gateway fmt-check; Simulation `shared_zone` 78/78; focused Gateway Walk+Run/Turn regressions; local captures `docs/generated/player-qa/movement-jitter/crystal-action-queue-local-shiftd-20260523.json` and `docs/generated/player-qa/movement-jitter/crystal-action-queue-local-da2-20260523.json`; action-queue verification deployment `dpl_HmHQ4CXfy7d895kHFMfiNLHWespN`; and production captures `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-walk-fix2-20260523T1331.json` plus `docs/generated/player-qa/movement-jitter/prod-action-queue-keyboard-run-fix2-20260523T1332.json`, both `ok=true` with no visual jumps, logical rollback, scene blackouts, critical console errors, or non-favicon 404s.
- 2026-05-22: Production movement/layout hardening landed for the user-reported Chrome/DevTools failures. The original 1024x768 client stage now uses a viewport-driven CSS scale so very narrow DevTools layouts keep login/select/game controls inside the visible viewport; `/health` is a no-store Next route instead of a Vercel 404; hydration risk is reduced with deterministic initial motion state plus layout-level hydration suppression; and self movement ACKs now confirm local pending/fed actions even when the optimistic client state already matches the incoming `UserLocation`, pruning `outstandingSelfMovementActions` after visual settlement. A requestAnimationFrame plus 100ms fallback keeps `motionNow` live in headless/throttled Chrome. Production deployment `dpl_Gr9WgZX275rpfDfk9f4SdzAshogb` is live behind `https://mir2.obelisk.build/`; custom-domain `/health` returned 200, and `Monster/000/51.png` returned 200. Evidence: `pnpm --dir apps/web exec tsc --noEmit --pretty false`; `pnpm --dir apps/web exec next build`; `docs/generated/player-qa/movement-jitter/prod-final-narrow-stage-scale-20260522.json` (`ok=true`, 150x647 stage bounds `left=-0.01`, `width=150.02`, no console errors, no non-favicon 404s, no residual movement queues); and `docs/generated/player-qa/movement-jitter/prod-final-movement-ack-prune-skip-transfer-20260522.json` (`ok=true`, strict movement checks green, `directionStepPending=null`, `outstandingSelfMovementActions=[]`, no visual jumps, no route spam, no logical rollback, no camera-offset stair-step warnings). The failed `prod-final-movement-ack-prune-20260522` run is expected evidence that production correctly rejects debug `crystal:<map>:<x>:<y>` transfer commands for normal clients.
- 2026-05-20: Production self-movement rendering now preserves local/packet movement animation state even when the authoritative player tile already equals the predicted tile, so the self player no longer drops back to a standing frame during walk/run confirmation. Camera/entity motion offsets now keep fractional pixels instead of truncating every frame to integer pixels, removing the stair-step feel during tile interpolation. The movement QA harness also records self movement animation fields and retries an explicit navigation when Chrome creates a target before hydration completes. Evidence: production Vercel deployment `dpl_ArWKGQbfwi5F3viVUsNsoumktTuD`; web `pnpm --dir apps/web exec tsc --noEmit --pretty false`; `node --check apps/web/scripts/capture-web-movement-jitter.mjs`; production headed capture `docs/generated/player-qa/movement-jitter/prod-movement-animation-fix-20260520-click-target.json` with `ok=true`, `noVisualJumps=true`, `cameraOffsetMovesContinuously=true`, `directionAnimationWithinCrystalWindow=true`, `noConsoleErrors=true`, `noNonFaviconNetwork404s=true`, `97` samples, `65` fractional-offset samples, and live `walk/run` packets through `packetRefresh`. A focused packet-run probe `prod-movement-animation-fix-20260520-packet-run-left-rerun.json` additionally captured `movementAnimation="running"` on self samples and running motion snapshots after real `UserLocation` packets; its overall `ok=false` is expected because the probe deliberately sends repeated packetRun commands and trips spam/direction-window assertions.
- 2026-05-16: MiniMapDialog crop/title/collapse alignment now follows the Crystal source frame more closely. The 120x108 minimap viewport now crops the full minimap raster with Crystal-style negative source offsets instead of scaling a cropped image over a transparent frame, preventing the main scene from leaking into the top-right minimap. The large minimap title now renders one centered map-name line without appending Safe Zone, collapsed mode switches to the small `Prguse/2091` frame and hides title/scene content, mini-map button hit boxes remain stable in both expanded/collapsed modes, and radar colors match Crystal player/NPC/monster dots. A dedicated minimap-only Stage 5 smoke now covers expanded/collapsed/BigMap/Mail states without running the whole 100+ screenshot suite, and the movement prediction state update no longer emits React `flushSync` lifecycle warnings during smoke login/bootstrap. Evidence: Web `node --check scripts/smoke-stage5-ui.mjs`, `npx tsc --noEmit`, direct `npx next build`, and live `MIR2_STAGE5_SMOKE_MINIMAP_ONLY=1 MIR2_WEB_BASE_URL=http://127.0.0.1:13010/?gatewayWs=ws://127.0.0.1:7210/ws node scripts/smoke-stage5-ui.mjs`, which wrote `docs/stage5-screenshots/stage5-minimap-smoke-manifest.json` with `mode="minimap-only"`, 17 screenshots, `criticalConsoleErrors=[]`, expanded `nameText="BichonProvince"`, `titleCount=1`, `sceneHidden=false`, `sceneHasRaster=true`, `sceneHasFallback=false`, collapsed `titleCount=0`, `sceneHidden=true`, `smallMode=true`, BigMap/Mail open states, and all minimap button hit tests uncovered.
- 2026-05-15: Closed the remaining high-frequency movement residual/rollback path by committing completed local self movement into the client-side CurrentLocation and shielding it from stale snapshot echoes. Verification evidence is `r-highfreq-keyseq-da-after-local-current-location-16ms.json`, `r-long-shiftd-after-local-current-location-16ms.json`, and `r-right-left-after-local-current-location-16ms.json`, all strict-green with no visual jumps, no logical rollback, no queue residue, and no browser errors.
- 2026-05-15: Closed the chat control-bar button loop. The row beneath the belt now follows Crystal `ChatControlBar`: channel buttons set outgoing prefixes, Trade dispatches `tradeRequest`, Settings controls visible chat channels and transparency, Size collapses/expands, and Report opens/closes. A dedicated smoke uses real mouse events and hit-testing to prove the controls are not covered by the HUD: `docs/stage5-screenshots/stage5-chat-controls-smoke-manifest.json` records every button `topMatches=true`, all prefixes verified, chat send with `!Codex shout smoke`, trade command dispatch, settings toggles, report open/close, and zero critical console errors.
- 2026-05-11: Movement input-latency investigation matched Crystal's client/server loop more closely: Web movement confirmation ticks now run only while movement is busy, target-click plans preserve the local action anchor when switching from held direction input, early same-tile `UserLocation` echoes are retried instead of treated as immediate rollback, and route corrections use the originally sent direction when deciding whether to hold predicted motion. The simulation now queues one over-early Walk/Run retry for the next world tick, matching Crystal's `_retryList` behavior instead of dropping the packet. Evidence: web `npx tsc --noEmit`, `cargo +1.89.0 fmt --check -p mir2-simulation -p mir2-gateway`, focused simulation `crystal_packet_walk_timing_rejects_repeat_until_world_tick_advances`, and captures `r-manual-jitter-fix-0511j` / `r-keyboard-after-retry-0511a`. Remaining stress failures are blocked/dynamic-entity cases where the client still predicts into a tile the server later rejects; those now avoid visual rollback but still need collision/blocked-target feel parity.
- 2026-05-09: Original-client frontend shell was split into smaller ownership modules instead of keeping the Crystal client in one multi-thousand-line component. `original-client-shell.tsx` is now the input/state orchestrator (970 lines), with shared display contracts, shell flow constants/props, HUD/window composition, scene layout, scene motion timing, scene map rendering, scene sprite rendering, visual layers, inventory action/password panels, and social-system definitions split under `apps/web/app/components/`. Evidence: web `tsc --noEmit`, `next build`, `git diff --check`, live 13010 HTTP 200 after dev-server restart, `r-final-component-split-input-234609.json` (`jumps=[]`, held run 330,270 -> 336,270), `r-final-component-split-route-234706.json` (`jumps=[]` for route replay), and `r-final-component-split-mirguide-click-235004.json` (`dialogTitle="MirGuide_Peter"`, zero console errors).
- 2026-05-09: NPC quest markers now anchor to the rendered NPC sprite center instead of the tile fallback/name offset, with stable 28x29 Crystal icon dimensions and NPC sprite/name hitboxes using the real frame bounds. Closing an NPC dialog now sends `@Exit`, so the next click can reopen the same NPC instead of staying hidden behind a stale dismissed dialog key. Evidence: web `npx tsc --noEmit`, live Gateway WS new-account/new-character `Village Guide` interact/`@Exit` probe, and headless DOM click probe opening/closing `Village Guide` from the sprite hitbox with zero console errors.
- 2026-05-09: Movement prediction now includes facing direction, so local run/walk prediction turns the sprite in the same frame as the predicted tile. Target-click movement no longer clears the local target prediction while the server is still confirming an earlier tile, and queued move dispatch starts 50ms before the 600ms visual action boundary to reduce input-lag feel without changing the frame animation length. Evidence: web `npx tsc --noEmit`; `r-direction-queue-after.json` with held-run `jumps=[]`; `r-click-direction-after-restart.json` with right-click target prediction held at `338,270` while the server was still at `336,270`, `direction="Right"`, and `jumps=[]`; `r-route-direction-after.json` with route replay `jumps=[]`.
- 2026-05-08: Movement feel cleanup aligns Web walk/run displacement with Crystal's frame-driven `OffSetMove`: camera/entity offsets advance on the six 100ms movement frames, snap to even integer pixels, and held-direction prediction now waits for the prior action to be confirmed or timed out before adding another local run/walk step. Evidence: `r-movement-direction-pending-cleared-after.json` and `r-movement-click-direct-crystal-frame-after.json`, both with `jumps=[]`, no browser errors, and no non-favicon 404s.
- 2026-05-07: Closed the frontend 2/4/5/6 automation slice. Live Crystal magic/projectile/buff packets now update Web combat visuals and skill/buff state; late-system System Menu social panels include trade/market/marriage plus state-backed rows/actions; NPC/quest smoke uses real Crystal dialog links without QA storage fallback and strips Crystal script markup; compact smoke now runs a three-viewport matrix with repo-stable screenshot output. Evidence: 113-screenshot Stage 5 UI smoke, `criticalConsoleErrorCount=0`, `compactMatrixCount=3`, `systemMenuSocial=36`, `npcDialogFlow=11`, and `combatFlow=2`.
- 2026-05-07: Entity movement and idle animation now follow Crystal timing more closely. Walk/run packets create a 600ms action window for sprite motion, walking/running frames start from the packet time, standing player/monster/NPC frames animate at their Crystal idle intervals, and the extra CSS attack hop was removed. Evidence: `docs/generated/player-qa/movement-jitter/r-movement-animation-crystal-timing.json` / `.png` and the live-tick companion capture.
- 2026-05-07: Player Web gateway targeting is no longer hardcoded for test harnesses: `?gatewayWs=ws://host/ws` or `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` can point the client at an isolated Gateway, while the default remains `ws://127.0.0.1:7110/ws`.
- 2026-05-07: Stage 5 UI smoke now activates InnKeeper_Brittney's Crystal `@Storage` service, then verifies service-backed storage password set/unlock/change/remove, Dagger store, and Red Potion take-back. The refreshed manifest captures 101 screenshots and reports 0 critical console errors.
- 2026-05-07: Equipped-item normal/special repair is now backed by real Crystal NPC services. The smoke damages equipped Dagger, repairs through Blacksmith_Smith `@Repair` and Blacksmith_Bill `@SRepair`, and verifies durability/gold mutations; focused simulation regressions cover equipped-slot repair ids and the QA damage setup.
- 2026-05-07: GameShop now has a positive Gold-purchase smoke path. The script seeds Gold through real Mail claim, buys `AccuracyPotion`, verifies the Gold deduction, carry-slot delivery, and purchase chat feedback, then archives `stage5-gameshop-gold-open.png` and `stage5-gameshop-gold-buy.png`.
- 2026-05-07: Mail rows no longer nest action buttons inside a row button. The Crystal-style row is now `role="button"` with keyboard activation, leaving claim/delete as real child buttons and keeping full smoke `criticalConsoleErrorCount=0`.
- 2026-05-07: Stage 5 UI smoke now presses belt hotkeys `1..6`, requiring Red/Blue Potion slots to decrement and recording empty or non-consumable occupied slots as no-op coverage instead of leaving broader hotkeys untested.
- 2026-05-01: R326 held-mouse queued-action fix keeps the latest 100ms held-pointer direction request instead of returning during the current 600ms walk/run action window. This matches Crystal's `QueuedAction` overwrite/consume behavior while preserving one movement packet per completed action. Evidence: `docs/generated/player-qa/movement-jitter/r326-web-hold-run-queued-direction.json` final `344,270`, `movementPlan=null`, `jumps=[]`, `r326-web-hold-run-ws-send-probe.json` with repeated WebSocket `run Right` entries, plus gateway move logs showing `Run Right` through `344,270`; web `tsc --noEmit` passed.
- 2026-05-01: R327 wires Gameshop Buy to manifest-backed backend purchase commands and fixes right-click map-click arrival. Evidence: `r327-gameshop-buy-click-final-clean-state.json` sends `gameShop.buyCredit(20,1)` with the expected zero-credit rejection and no browser 404/errors; `r327-map-click-target-arrival-fixed3.json` reaches `338,270` with `movementPlan=null` and `jumps=[]`. Verified by web typecheck, script syntax checks, focused simulation game-shop test, gateway check, and CDP captures.
- 2026-05-01: R325 held-run visual jitter fix keeps predicted movement in the render basis and refreshes motion snapshots synchronously before paint, preventing map/camera backtracking when service snapshots confirm earlier tiles. Evidence: `docs/generated/player-qa/movement-jitter/r325-web-hold-run-final-4s.json` records final `344,270`, `movementPlan=null`, fixed map sprite continuity, and `jumps=[]`; verified with web `tsc --noEmit`, capture-script syntax check, `mir2-gateway` check, and targeted movement captures.
- 2026-05-01: R322 movement correction cleanup removes predicted coordinates from the logical movement source and clears the movement plan when the server corrects to a different tile. This fixes the severe back-and-forth loop around blocked or partially blocked tiles. Evidence: `docs/generated/player-qa/movement-jitter/r322-web-movement-correction-stop.json` and `r322-web-movement-open-area.json`, both with `jumps=[]`; verified with web `tsc --noEmit`.
- 2026-05-01: R323 held-mouse movement now samples held scene input every 100ms and sends Crystal-like `Walk` / `Run` direction packets for queued movement instead of feeding absolute `moveTo` targets. Evidence: `r323-web-hold-run-direct-direction.json` final `340,270`, `r323-web-hold-walk-direct-direction.json` final `335,270`, and `r323-web-packet-run-right.json` final `340,270`; all have `jumps=[]`. The later R327 asset export removed the missing `original-ui/NPC/25/meta.json` warning from this scene.
- 2026-05-01: R321 movement-control diagnostics add original-client Win32 mouse/screenshot automation and richer Web movement sampling. Evidence at `docs/generated/player-qa/movement-jitter/r321-web-movement-direct.json`, `r321-web-movement-click-actions.json`, `r321-web-movement-click-hitoffset.json`, and `r321-original-movement-control.json` separates backend traversal from input feel. Direct `moveTo` advances each step cleanly with no jumps, and moving the transparent tile hit layer with `playerCameraMotionOffset` fixes the initial click/right-click hit-test delay; remaining movement feel work is Crystal-like continuous queued input handling.
- 2026-04-30: R317 Gameshop product-grid cleanup replaces placeholder cells with the generated Crystal Gameshop manifest, original `Title/750` item-cell frame, buy/preview button sprites, real item icons, categories, pagination, stock/count/price labels, and gold/credit payment controls. Evidence at `docs/generated/player-qa/r317-gameshop-products/r317-gameshop-products-state.json` records 8 visible cells, `pageLabel="1 / 14"`, `loadedIconCount=8`, zero placeholder cells, zero non-favicon 404s, and zero console errors.
- 2026-04-30: R316 Gameshop/Menu cleanup fixes the HUD Gameshop miswire from Inventory/Quest to a Crystal-framed Gameshop shell and replaces the visible large Web menu/debug panel with the 36x282 Crystal `MenuDialog` icon strip. Evidence at `docs/generated/player-qa/r316-gameshop-menu/r316-gameshop-menu-state.json` records `shopVisible=true`, `inventoryVisible=false`, `menuBounds=988,349,36,282`, `iconCount=13`, no old overlay header, zero non-favicon 404s, and zero console errors.
- 2026-04-30: R312 entity projection/nameplate alignment restores Crystal source `MapControl.OffSetY` math and moves entity sprite/nameplate/health anchors to Crystal `DrawLocation` / `DisplayRectangle` placement. Evidence at `docs/generated/player-qa/r312-entity-crystal-anchor/r312-bichon-287-618-entity-anchor-state.json` records `QA0429Hero` nameplate `top=275`, exact stage/HUD bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-30: R311 playfield camera/HUD orb cleanup centers the Web map viewport on the Crystal playfield height above the HUD and replaces CSS-gradient HP/MP orb fills with Crystal `Prguse` frame 4 bitmap slices. R311 evidence at `docs/generated/player-qa/r311-playfield-camera/r311-bichon-287-618-hud-orb-state.json` records `QA0429Hero` nameplate `top=325`, exact stage/HUD bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-29: R310 same-scene visual-watch bootstrap fixes the login transition overlay leaking into game screenshots, scopes NPC quest icons by server-provided `questIds`, adds `capture-crystal-parity.mjs` for deterministic Web game captures, and adds `r310-visual-watch.ps1` for original/Web long-run sampling. R310 evidence at `docs/generated/player-qa/r310-visual-watch/r310-final-web-scene-state.json` records `transitionOverlayVisible=false`, `questMarkerCount=0`, exact 1024x768 stage/HUD bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-29: Bichon minimap/HUD bounds cleanup moves `.mini-map-panel` from `right=-2px` to `right=0`, closing the measured desktop `right=1026` overflow. R309 evidence at `docs/generated/player-qa/r309-minimap-bounds-web-page-state.json` records desktop minimap `right=1024`, empty overflow arrays, zero non-favicon 404s, and zero console errors.
- 2026-04-29: Bichon viewport/resource cleanup removes the 0.9 desktop downscale, removes decorative page/frame background effects, keeps compact-only 0.78 scaling, and exports missing `NPC/00`, `NPC/01`, `NPC/03`, `NPC/11`, `NPC/15`, `Monster/003`, `Monster/004`, and `Monster/005` sprite libraries. R308 evidence at `docs/generated/player-qa/r308-stage-scale-web-page-state.json` records exact desktop stage bounds, compact bounds, zero non-favicon 404s, and zero console errors.
- 2026-04-29: Bichon `0:287,618` guard/archer evidence now has a focused simulation regression and browser capture. R307 evidence at `docs/generated/player-qa/r307-bichon-guard-archer-web-page.png` and `docs/generated/player-qa/r307-bichon-guard-archer-web-page-state.json` records `hasGuard=true`, `hasArcherGuard=true`, `hasUnderscoreNameplate=false`, and `questTrackerVisible=false`.
- 2026-04-29: Bichon same-scene display cleanup removes the default web quest tracker overlay from the game playfield and displays NPC/monster nameplates without underscores. R306 evidence at `docs/generated/player-qa/r306-bichon-display-web-page.png` and `docs/generated/player-qa/r306-bichon-display-web-page-state.json` keeps 8 NPC sprite elements plus 8 monster sprite elements and records `hasUnderscoreNameplate=false`, `questTrackerVisible=false`.
- 2026-04-29: Bichon same-scene visible respawns now enter the world snapshot and page state. R305 evidence at `docs/generated/player-qa/r305-bichon-visible-world-snapshot.json`, `docs/generated/player-qa/r305-bichon-visible-web-page.png`, and `docs/generated/player-qa/r305-bichon-visible-web-page-state.json` shows 8 NPC sprite elements plus 8 monster sprite elements, including Deer and Royal_Guard.
- 2026-04-29: Bichon same-scene runtime population now includes current-map Crystal NPC-info manifest entries on saved-character start and transfer. Live WS evidence at `docs/generated/player-qa/r304-bichon-npc-world-snapshot.json` shows 8 NPCs around `0:284,607`; browser evidence at `docs/generated/player-qa/r304-bichon-npc-web-page.png` and `docs/generated/player-qa/r304-bichon-npc-web-page-state.json` shows 8 NPC sprite elements rendered in page state. Focused/adjacent simulation tests, gateway build, live WS probe, and browser capture passed.
- 2026-04-22: `LoginOverlay` account/password inputs now submit on Enter through the existing login handler; scene tile hit buttons now mark themselves UI-interactive and stop pointer bubbling so tile actions are handled once while empty-space scene clicks remain available. `npm.cmd run build --prefix E:\mir2\mir2-web3\apps\web` passed.
- 2026-04-22: Ground-drop labels now preserve and render server `nameColourArgb`, including Crystal Cyan for added-stat item drops. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-22: Selected scene targets now expose localized action/distance nameplate feedback and keyboard approach/primary-action routing through the existing target handlers. `npm.cmd run build --prefix apps\web` passed.
- 2026-04-26: Chat now opens on the newest filtered lines, follows new messages while at the bottom, preserves scrollback when the user scrolls up, and moves the Crystal scroll knob with position. Headless/no-WebGL UI smoke now stays in DOM mode, Crystal map API locally falls back to packaged starter-region data when Crystal map files are absent, and Stage 5 UI smoke detects macOS Chrome. Direct `next build`, map/minimap smokes, Stage 5 UI smoke, and WS load passed.
- 2026-04-26: Stage 5 UI smoke now archives named desktop and compact viewport evidence, captures `stage5-compact-game.png`, and asserts compact core UI bounds before writing the screenshot manifest.
- 2026-04-26: Stage 5 UI smoke now asserts visible compact core text does not overflow. The new check found and fixed compact minimap title wrapping by splitting the map title and Safe Zone label into a stable two-line header.
- 2026-04-26: Stage 5 UI smoke now clicks minimap collapse, BigMap re-expand, and Mail open paths, archives three minimap screenshots, and records `minimapFlow` state.
- 2026-04-26: Stage 5 UI smoke now rotates and closes the belt, archives three belt screenshots, records `beltFlow`, and checks that slot labels remain inside the belt and the vertical belt does not overlap Quest.
- 2026-04-26: Stage 5 UI smoke now presses belt hotkey `1`, verifies Red Potion quantity decreases, archives `stage5-belt-hotkey-use.png`, and records `beltUseFlow`.
- 2026-04-26: Stage 5 UI smoke now switches inventory bag1, bag2, quest, and back to bag1, archives three tab screenshots, and records `inventoryFlow`.
- 2026-04-26: Stage 5 UI smoke now switches character char, stats1, stats2, spells, and back to char, archives four tab screenshots, and records `characterFlow`.
- 2026-04-26: Stage 5 UI smoke now switches storage page 1, locked expanded page 2, and back to page 1, archives two page-state screenshots, and records `storageFlow`.
- 2026-04-26: Stage 5 UI smoke now exercises chat Shout filter, All restore, Settings, collapse/restore size, and Report paths, archives four chat-control screenshots, and records `chatFlow`.
- 2026-04-26: Stage 5 UI smoke now opens the system menu, verifies transfer/action labels, routes Character, Inventory, and Quest actions, archives four system-menu screenshots, and records `systemMenuFlow`.
- 2026-04-26: Stage 5 UI smoke now rents expanded storage from locked page 2, verifies page 2 unlocks with expanded capacity/expiry copy, archives `stage5-storage-page2-rented.png`, and records the rented state in `storageFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Red Potion from inventory bag1, verifies quantity drops from 5 to 4, archives `stage5-inventory-use-red-potion.png`, and records `inventoryUseFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Dagger from inventory bag1, verifies it moves into the weapon equipment slot, archives `stage5-inventory-equip-dagger.png`, and records `inventoryEquipFlow`.
- 2026-04-26: Stage 5 UI smoke now routes HUD Skill to Character Spells and HUD Option to Stats II, archives two HUD-button screenshots, and records `hudButtonFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Drop Gold, confirms 100 gold, verifies gold decreases and a ground-drop label appears, archives two gold-drop screenshots, records `inventoryGoldFlow`, and fixes missing `ui.confirm` fallback text.
- 2026-04-26: Stage 5 UI smoke now context-clicks Wooden Sword in bag1, moves it from slot 4 to slot 10, archives `stage5-inventory-move-wooden-sword.png`, and records `inventoryMoveFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Split Item for Red Potion, confirms count 1, verifies Crystal-style belt placement with total quantity preserved, archives two split screenshots, and records `inventorySplitFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Delete Item for Blue Potion, confirms the drop, verifies quantity decreases and a ground-drop label appears, archives two item-drop screenshots, and records `inventoryDropFlow`.
- 2026-04-26: Character RemoveItem now sends the Crystal-shaped inventory-grid target with the first free bag slot, and Stage 5 UI smoke verifies Dagger leaves equipment and returns to bag1 slot 4, archives `stage5-character-remove-dagger.png`, and records `characterRemoveFlow`.
- 2026-04-26: Stage 5 UI smoke now clicks Red Potion directly in the belt, verifies quantity decreases before the existing hotkey path, archives `stage5-belt-mouse-use-red-potion.png`, and records `beltMouseUseFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Sell Item for Dagger, confirms without an active sell service, verifies Dagger and gold are preserved, archives two sell screenshots, and records `inventorySellFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Store Item for Dagger, selects a warehouse slot without an active storage service, verifies Dagger and existing storage contents are preserved, archives three store screenshots, and records `storageStoreFlow`.
- 2026-04-26: Stage 5 UI smoke now opens Take Back for stored Red Potion, selects an inventory slot without an active storage service, verifies inventory/storage quantities are preserved, archives three take-back screenshots, and records `storageTakeBackFlow`.
- 2026-04-26: Storage Protect is now reachable before a password exists, and Stage 5 UI smoke opens/closes Set Storage Password without submitting credentials, archives `stage5-storage-password-panel.png`, and records `storagePasswordFlow`.
- 2026-04-26: Stage 5 UI smoke now fills Set Storage Password, archives mismatch and no-service submit screenshots, verifies mismatched confirmation keeps submit disabled, and verifies matching submit without an active storage service leaves `hasStoragePassword=false` with no-service feedback.
- 2026-04-26: Stage 5 UI smoke now captures 71 screenshots and records Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA and transfer-list routing, Battle Focus spell casting, and compact inventory panel bounds.
- 2026-04-26: Stage 5 UI smoke now captures 85 screenshots and records login/select lifecycle, confirmed character delete/recreate, compact inventory/storage/character/system-menu/chat-settings bounds, NPC dialog link-capable state, and the existing broad gameplay/system matrix. Map API and minimap asset smoke outputs are archived under `docs/generated`, and WS load refresh is 64/64 ready with 0 errors.
- 2026-04-26: Stage 5 UI smoke now captures 88 screenshots and records advanced Stage 5 systems state for trade item/cancel, shop gold purchase, auction buy/cancel, conquest end, hero behaviour, mining/craft, and mail delete state. Compact Mail and Report panel bounds are now asserted and archived as `stage5-compact-mail.png` and `stage5-compact-report.png`.

## Human-Only Acceptance Boundary

Automation can verify crashes, route completion, DOM state, screenshots, packet traces, and data snapshots.

Human acceptance is still required for:

- whether the screen visually feels like Crystal;
- whether mouse targeting and item interaction feel right;
- whether combat feedback, animation pacing, and panel layering are acceptable;
- whether small visual differences should be fixed or accepted.

## 2026-07-23 Deterministic Visual-Parity Closeout

- The current same-account gate uses overlay-free Crystal/Web `1024x768`
  captures at Bichon `0 @ 328,275` with explicit server light pairing.
- Dawn improved from r29 full/world changed pixels `36.4%/40.2%` to final r33
  `24.2%/26.1%`; world MAE fell from `18.845` to `11.987`.
- Final Night r32 remains at full/world `12.5%/12.6%`, matching the r26
  `12.4%/12.6%` baseline and proving the Dawn/Evening correction is scoped.
- HUD experience fill now follows Crystal's `(1004 - 3) * ratio` clipping,
  HP uses the native source crop, chat scrollbar rows match native geometry,
  minimap projection follows Crystal's reverse quantization, and AI 6 monsters
  retain the native green radar colour through observer re-seeding.
- The capture/login path now waits for `Connected`, retains one latest pending
  action, ignores stale-socket events, verifies cursor parking, and redacts
  serialized secrets. r33 records zero critical errors and zero 404s.
- Final strict headed captures pass 28/28 assertions on both selected WebGPU
  and forced WebGL2. Each sends and acknowledges four ordered moves, settles
  at `328,275`, and leaves no pending movement/map transaction or browser
  error. The dual-backend runtime smoke also remains fully green.
- The captured Bichon source closure adds 555 deterministic Crystal map PNGs.
  Release preflight sees 39,401 manifest assets, 12,015 original map PNGs,
  8,228 packed entity sprites, and 99.76% renderable map-frame coverage.
- Remaining visual gaps are GDI typography, deterministic chat content, and
  independently moving entity/animation phases. They are no longer classified
  as movement, camera, or map-transaction failures.

## 2026-07-23 Quest Marker GPU Occlusion Regression

- Live inspection confirmed that all four nearby Scarecrow entities resolve
  through `Monster/005`; the full pack contains 234 drawable frames and the
  separated one-tile capture shows the native thin straw/skeletal body. The
  reported invisibility was player overlap and low contrast, not a missing
  atlas or failed entity render.
- NPC quest markers were still mounted inside `viewport-sprite-overlay`.
  With the Bevy entity renderer active, the GPU canvas at z-index 2 covered
  that DOM layer even though the quest state and marker image were present.
- Quest markers now live in the independent `viewport-entity-overlay`, share
  the imperative entity-motion registry, retain Crystal sprite offsets and
  NPC activation, and carry the NPC object id for deterministic QA lookup.
- The live account currently has the available stage, so Crystal correctly
  shows yellow exclamation markers. Accepted/in-progress and ready-to-turn-in
  stages continue to select white and yellow question markers respectively.
- Evidence:
  `docs/generated/player-qa/r41-quest-marker-overlay/r41-quest-marker-overlay-final.jpg`
  shows three visible yellow exclamation markers above the WebGPU scene.
  `elementsFromPoint` reports the marker above the drop overlay and canvas.
- Verification passed:
  `node apps/web/scripts/test-player-frames.mjs`,
  `npm run typecheck --prefix apps/web`, and `git diff --check`.
