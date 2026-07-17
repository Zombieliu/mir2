# Player QA Script

Last updated: 2026-07-18

Purpose: keep final human frontend validation focused. The project can be driven to **100% Candidate** automatically, then this script is used to decide whether the build becomes **100% Accepted**.

## Latest Deterministic Same-Scene Gate

- Prefer live r16 under
  `docs/generated/player-qa/visual-parity/cwp-20260718-r16-live-clean/` for the
  current post-NPC-fix layout/effect trend. It records `live-window-cycle`, 24
  native candidates, Day setting 2, target `0 @ 332,275`, 0 critical errors,
  0 404s, 7.1% full-window / 6.0% world changed pixels, world MAE 4.499, and
  Belt MAE 10.765. Its native top-left includes a Codex Computer Use status
  bubble; exclude that area from product conclusions and recapture without the
  external overlay before final human acceptance.
- A locked-effect capture must include `effectPixelContribution` with at least
  100 changed pixels. r16 records 28 visible TrapHexagon nodes and 57,282
  changed pixels; forced WebGL2 r09 records 55,462. The paired hidden-effects
  PNG must remove the cross without black compositor tiles.
- Long native raw paths are supported by Buffer-based decode. r15 validates a
  271-character source path and 24-candidate phase selection; do not shorten
  evidence prefixes merely to work around `MAX_PATH`.
- r03/r04 under
  `docs/generated/player-qa/visual-parity/crystal-web-pack-20260718-same-state-deterministic-r0*/`
  prove Edge 150 CDP capture works through Next 16.2.1's compiled `ws`.
- Treat r04's 100% weighted score as an automated Candidate trend only. Its
  raw thresholded differences remain 10% full-window and 9% world, with HUD UI
  87%, chat 82%, and MiniMap 87%; human visual/feel acceptance remains open.
- r04 must have `criticalConsoleErrorCount=0`, zero non-favicon 404s, requested
  and server `lightSetting=2`, primary NPC labels `rgb(0, 255, 0)`, and
  underscore-delimited secondary labels `rgb(255, 255, 255)`.
- Ignore only the exact Edge extension message-port closure classified by the
  CDP helper. Any JavaScript exception, application error, or other network
  failure remains critical.
- Capture a fresh native reference after the 2026-07-18 NPC packet-colour fix;
  do not use the older native White primary-name pixels as the color baseline.

## Local Playability Preflight

- Before judging visuals or movement, verify the Player Web and Gateway agree
  on a WebSocket port. The current local stack uses Web `3002` and Gateway
  `7111`; ignored `apps/web/.env.local` must contain
  `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL=ws://127.0.0.1:7111/ws` unless the Gateway is
  moved back to the source fallback port `7110`.
- Test the bare `http://127.0.0.1:3002/` URL, not only a `?gatewayWs=` override.
  Acceptance requires account login, character list, Start Game, a visible map,
  one movement input changing coordinates, one active Gateway session, and no
  browser error/warning logs. The 2026-07-13 verification moved
  `MountQaR19` from `333,274` to `334,274` with `D`.

## Full Pack / Low-End Gate

- Force the constrained path with
  `?renderTier=low&bevyBackend=webgl2&bevyEntities=1&bevyAtlas=1` and keep
  `?mir2Debug=1` while collecting evidence. The runtime debug snapshot must
  report tier `low`, atlas mode `packed`, and no DOM entity fallback.
- Enter Bichon, wait for `sceneInteractionReady`, and move normally through at
  least four acknowledged tile steps. Require no residual movement plan,
  visual jump, logical rollback, scene blackout, critical console error, or
  non-favicon 404.
- On a 2 GiB low-tier profile, decoded resident entity-atlas bytes must remain
  below 64 MiB. The accepted local baseline used 13 pages, 1,598 rects, and
  58,379,430 bytes while passing all 28 movement/render assertions.
- Low-tier scene prewarm must stay below 1,000 requests with no failures;
  background prewarm is off by default. CacheStorage must remain below 256 MiB
  and first playable below 15 seconds in the automated cold and warm runs. The
  accepted baseline completed 403/403 requests, used 69,027,432 bytes after the
  warm run, and transferred 18,993,684 cold bytes versus 600 warm bytes.
- Review
  `docs/generated/player-qa/full-asset-pack-low-tier/full-pack-low-tier-summary.json`
  and `full-pack-low-tier-webgl2-clean.png`. This desktop/local-network gate is
  necessary but not sufficient for Brazil release: repeat it on physical 2 GiB
  and 4 GiB Android devices with throttled 4G, map transitions, background
  resume, and memory pressure.

## Crystal/Web Temporal Pack

- Use the Codex desktop trusted runner with
  `capture-crystal-temporal-pack.mjs` for the canonical Bichon `332,275`
  four-step comparison. `npm run qa:crystal-temporal-pack` is the equivalent
  CLI entry point, but its native phase requires
  `MIR2_COMPUTER_USE_CLIENT_MODULE`; a plain shell cannot acquire Computer Use
  by itself. Supply the existing QA account, password, and control token
  through `MIR2_QA_ACCOUNT`, `MIR2_QA_PASSWORD`, and
  `MIR2_QA_CONTROL_TOKEN`; never add them to the scenario JSON. Set
  `MIR2_WEB_BASE_URL` when the Gateway is not reachable through the Web build's
  default URL.
- Run the native client against a Release Gateway. The Debug Gateway's initial
  world bootstrap can exceed Crystal's five-second handshake timeout. Log the
  native character into Bichon at `332,275`, keep its game viewport at
  1024x768, and then run the pack. The Web phase uses CDP screenshots at exact
  1024x768 rather than OS window bounds.
- A fresh baseline requires `native`, `web`, and `report` to pass in
  `manifest.json`. A focused Web repair rerun may leave native `skipped` only
  when it reuses the unchanged validated native artifact and the report phase
  verifies that input. The Web artifact must show four eligible and four
  matched local-command pose events, zero dropped sink events, exact Crystal
  phase pixels, atomic pose commits, and final delta `(-4,0)`. After rebuilding
  WASM under `next start`, rebuild and restart `.next` too, then verify the
  captured runtime version changed.
- The accepted 2026-07-13 WebGPU baseline is runtime
  `bevy-90fb96239f221a47`, 4/4 pose coverage, and 41ms maximum sink latency.
  Its paired artifacts are under
  `docs/generated/player-qa/movement-jitter/temporal-packs/bichon-332275-left4/`.
  A report pass proves geometry, timing, and evidence generation. It does not
  waive the observed 75.0%-76.8% full-window visual delta; inspect the overlay
  and heatmap PNGs before human acceptance.

## Compact-Window Pixel Stability Gate

- Crystal updates movement on a 100ms phase lattice, but renders integer/even
  `OffSetMove` values directly into the selected backbuffer. It does not scale a
  fixed 1024x768 frame through a fractional compositor transform.
- For Web viewports smaller than 1024x768, verify `.client-stage-frame` has an
  integer 4:3 bounding rectangle and integer `left`/`top`. A fractional outer
  rectangle resamples every map and entity pixel together and can look like
  NPC/player flicker even when entity count, atlas state, and AOI are stable.
- 2026-07-13 A/B evidence is under
  `docs/generated/player-qa/flicker-ab/`. The pre-fix `current-820.json` records
  stage `(-0.01, 102.49, 820.02x615.01)`; `aligned-820.json` records exact
  `(0, 103, 820x615)` across 93 movement frames, with zero scene blackouts,
  pose-commit warnings, critical console errors, and non-favicon 404s.
- A compact Web window is still a downsampled presentation. For final Crystal
  visual/feel acceptance, use at least a 1024x768 content viewport so the
  1024x768 canvas presents at 1:1; do not compare a 0.8x browser pane against a
  native 1:1 backbuffer and attribute the resulting sampling difference to the
  server movement pipeline.

## Mounted Movement Gate

- The current cross-backend automated baseline is WebGPU
  `movement-mounted-scene-transaction-full-phases-webgpu-20260712-r12.json`
  at 33/33 and WebGL2
  `movement-mounted-scene-transaction-full-phases-webgl2-20260712-r16.json` at
  33/33. Require exactly two movement frames, all Walk phases `0..7` at
  `-6,-12,-18,-24,-30,-36,-42,-48px`, all mounted Run phases `0..5` at
  `-24,-48,-72,-96,-120,-144px`, a pinned self sprite, equal map/entity
  centers, endpoint-only logical centers, and zero shadow/pose/provenance/
  console/network warnings.
- The scene transaction must stay atomic and one rejected pose must retain the
  last coherent pose until the 250ms watchdog. Local phase 0 must last a full
  100ms from command start, and delayed display frames must extend the action
  rather than catch up multiple phases. These checks prevent the former
  mixed-clock jerk even when ACK latency remains low.
- Do not assign a routine `--debugPort`. The harness asks Chrome for an
  ephemeral port and verifies the `DevToolsActivePort` file in that run's
  isolated profile. Use an explicit port only for diagnosis; an occupied value
  must fail immediately instead of attaching to another browser. The unattended
  reference is `movement-mounted-autocdp-cleanup-webgpu-20260712-r19.json` at
  33/33 with zero new Chrome profiles after completion.
- The semantic predecessor baseline is
  `movement-mounted-walk8-run3-webgpu-20260712-r6.json`. It must retain 27/27
  assertions, exactly two movement WebSocket frames, one-cell Walk, three-cell
  Run, phase counts 8/6, 2/2 Pose coverage, and zero atomic Pose warnings.
- Use short keyboard taps for deterministic sequencing. Pass
  `--keyPressMs 80`; a 600ms key hold intentionally exercises repeat movement
  and is not a two-command Walk/Run proof.
- Mounted setup is token-gated: `--mountItem crystal-item-769
  --mountRequiredLevel 22 --expectMounted true`. The harness raises only its QA
  character through `qa.applyNativeState`, grants the real item, equips slot 13,
  then toggles riding through normal `UseItem`. It does not bypass equipment or
  movement validation.
- Crystal expectations: mounted Walk is eight 100ms phases and one tile;
  mounted Run is six phases and three tiles. Server MoveDelay remains 600ms.
  A two-cell mounted Run means Session-to-Zone state propagation regressed. Pose
  warnings only in phases 6/7 mean the `phaseCount` Pose contract regressed.

## Latest Movement Ingress Gate

- Start movement captures only after the QA transfer has produced a newer
  `window.__mir2PacketRuntime.lastSnapshotAt`; seeing the target
  `UserLocation` alone is not enough because the serial transfer response can
  still be publishing its world snapshot.
- `capture-web-movement-jitter.mjs` now fails any unresponsive CDP command after
  15 seconds. A stuck headless WebGPU renderer must fail and clean up rather
  than hold unattended QA forever.
- Current Release baselines are 15/14ms for expired Walk/Run degradation,
  17/21ms ACK plus 23/24ms pose/sink for strict keyboard Walk/Run, and 11/2ms
  for the gameplay-event-observed protocol run. Expected final deltas are
  `(2,0)`, `(3,0)`, and `(3,0)` respectively, with no corrections.
- The final normal-port keyboard baseline is
  `movement-zone-owned-cadence-final-release-keyboard-20260712.json`: Walk/Run
  ACKs are 23/6ms, pose/sink maxima are 12/12ms, final delta is `(3,0)`, and all
  strict assertions pass. Use `keyboardSequence` for local presentation
  acceptance; `packetSequence` deliberately bypasses keyboard/controller pose
  production and is protocol evidence only.
- When validating observability, enable `MIR2_GAMEPLAY_EVENT_LOG=1`; the
  movement run must produce exactly one `client.Walk` and one `client.Run`, and
  health must keep `accepted == published` with zero `failed` and `dropped`.
- The shared-world cadence/observer automation gate is now accepted. Run
  `smoke-two-client-zone.mjs` with `observerPulseAfterMove=false`; for the
  strongest regression set `MIR2_GATEWAY_RUNTIME_TICK_MS=5000` on the Gateway
  while leaving the Zone owner at its fixed 300ms cadence. The observer movement
  frame must arrive within 250ms without sending Tick, and Bevy remote-motion,
  packed-offset, decode/drop, console, and network assertions must all pass.
  Latest strict evidence is
  `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`
  at 12ms with 16 entities on both clients, one remote-motion event, 29 offset
  matches, and zero failures. Its matching `-a.png`/`-b.png` screenshots are
  scene-ready because the harness foregrounds each page before capture.
- StartGame and every QA transfer must observe a newer
  `window.__mir2PacketRuntime.lastSnapshotAt`. Read map/entities/tick through the
  live Stage5 QA getters, not a React render snapshot, so a background tab's rAF
  throttling cannot produce a false AOI failure.

## Acceptance States

| State | Meaning |
| --- | --- |
| 100% Candidate | Automated checks, docs, traces, screenshots, and implementation tasks are complete against current standards. |
| 100% Accepted | Human gameplay review passes, or remaining differences are explicitly accepted. |

## Human Time Budget

Recommended final human QA budget: **35-70 hours** total.

The target is to keep routine development review small and reserve most human time for the final Candidate build.

## Evidence Gate

Before starting human acceptance for a Candidate build, the Coordinator should provide fresh evidence for:

- `npm.cmd run build`
- `npm.cmd run audit:crystal-map-coverage`
- `npm.cmd run audit:crystal-map-gameplay`
- `npm.cmd run smoke:crystal-minimap-assets`
- `npm.cmd run smoke:crystal-map-api`
- `npm.cmd run smoke:stage5-ui`
- `npm.cmd run smoke:cache-metrics`
- `npm.cmd run smoke:bevy-runtime-backends`
- `npm.cmd run test:local-command-pose-latency`
- `npm.cmd run smoke:cache-maintenance`
- `npm.cmd run smoke:playable-metrics`
- `npm.cmd run qa:visual-parity`
- `npm.cmd run assets:remote:build`
- `npm.cmd run assets:r2:dry-run`
- `npm.cmd run load:gateway-ws`
- backend focused/regression commands for the latest changed systems

Existing frontend evidence sources:

- `apps/web/package.json` scripts: `build`, `audit:crystal-map-coverage`, `smoke:crystal-minimap-assets`, `smoke:crystal-map-api`, `smoke:stage5-ui`, `smoke:cache-metrics`, `smoke:bevy-runtime-backends`, `test:local-command-pose-latency`, `smoke:cache-maintenance`, `smoke:playable-metrics`, `qa:visual-parity`, `assets:remote:build`, `assets:r2:dry-run`, `load:gateway-ws`
- `docs/generated/player-qa/cache-metrics/latest-cache-metrics.json`
- `docs/generated/player-qa/bevy-runtime-backends/latest-bevy-runtime-backends.json`
- `docs/generated/player-qa/visual-parity/*-report.md`
- `docs/generated/remote-assets/latest-remote-asset-release.json`
- `docs/stage5-screenshots/stage5-ui-smoke-manifest.json`
- `docs/generated/load/latest-ws.json`
- `docs/generated/load/latest-tcp.json`
- `docs/generated/map/latest-crystal-map-coverage.json`
- `docs/generated/map/latest-crystal-map-gameplay.json`
- `docs/generated/map/latest-crystal-map-api.json`
- `docs/generated/assets/latest-minimap-assets.json`

Latest automated frontend evidence:

- 2026-07-12: Run transition automation now distinguishes a Crystal-style
  one-cell degradation from a real correction. The raw expired Walk -> Run
  route records 16/99ms Release ACKs, one degradation, zero corrections, and
  delta `(2,0)` in
  `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`.
  The normal UI Walk -> Run chain records 22/28ms ACKs, 17/1ms pose latency,
  zero degradation/correction, and delta `(3,0)` in
  `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`.
  Human QA should verify that the first Shift+direction action visibly walks
  one cell, the immediately primed follow-up runs two, and neither transition
  flashes backward or triggers a 400ms lock. Mounted movement and true
  three-cell sprint are still open and must not be accepted from this evidence.
- 2026-07-12: local self/camera Bevy ownership and synchronous pose commit are
  now enabled on the normal URL. Human QA should first compare the default URL,
  then use `?bevyLocalMotion=0&bevyPoseCommit=0` only as an A/B rollback; both
  modes are automation-green. Default continuous movement stayed within 10ms
  command-to-pose latency, committed keyboard movement stayed within 15ms for
  4/4 commands, and both native and Web four-action spans measured 2701ms.
  Evidence:
  `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-continuous-202607120610.json`,
  `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-keyboard-committed-ref-202607120617.json`,
  `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-default-shared-clock-horizontal-20260712-001.md`,
  and rollback
  `docs/generated/player-qa/movement-jitter/movement-explicit-legacy-rollback-202607120623.json`.
  The final 25 additive map sprites are also Bevy-owned; WebGPU/WebGL2 smokes
  report zero DOM world sprites. Runtime `bevy-630a77b3535f95bd` passes 94/94
  Rust tests and
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-default-shared-clock-202607120620.json`.
  Keep correction/degraded-run, mounted movement, sprint, crowded/AOI scene
  motion, ambient lighting/effects, and combat VFX unaccepted until their own
  native/Web captures are green. Do not use the full-window visual-delta ratio
  as an actor-only movement score when world contents or capture geometry differ.
- 2026-07-10: release WebGPU local-command presentation has an in-page latency
  gate that is independent of CDP/DOM sampling. Exact route report
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710220403-44ba1f45-report.json`
  records 4/4 accepted synchronous pose commits at `14/18/32/16ms`, a 32ms
  maximum under the 75ms budget, exact final tile `328,275`, five map states
  (initial plus four real centers), and zero visual/provenance/console/network
  failures. Default-off compatibility report
  `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710221024-ce1066ce-report.json`
  and dual-backend report
  `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710221430.json`
  are also green. This was the historical pre-promotion A/B gate: reproducing
  that exact 2026-07-10 setup requires both `?bevyLocalMotion=1` and
  `?bevyPoseCommit=1`. The 2026-07-12 entry above supersedes its default-off
  instruction; current human QA starts from the normal URL and uses the
  explicit `=0` switches only for rollback comparison.
- 2026-07-08: local QA automation now has an explicit token-gated control lane.
  Gateway `qaControl` commands require `MIR2_GATEWAY_QA_CONTROL_TOKEN`; the Web
  harness sends them only when `MIR2_QA_CONTROL_TOKEN` is configured. Normal
  production player commands still reject debug transfer and raw Stage5
  commands. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-qacontrol2-20260708/report.md`
  ran with production command safety enabled and passed incoming damage
  (`playerHp 18 -> 0`) plus `townRevive` (`0 -> 18`). It also exposes the next
  automated QA gaps: QA transfer/spawn packets can settle late without an
  explicit ack, seeded Blue Potion pickup did not reach the drop tile, server
  damage packets did not create a `.scene-damage-floater`, and normal
  attack-kill/XP/drop remain red.
- 2026-07-08: Rust `7111` combat-survival attack-trace harness now records
  target map/object id, sent attack frames, melee approach trace, and delayed
  `ObjectAttack` / `ObjectStruck` / `DamageIndicator` packets before scoring
  incoming damage. It also retries the first `StartGame` race and uses normal
  packet movement for combat approach instead of over-clicking a moving target.
  Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace5-20260708/report.md`
  proves a real-client hostile-retaliation chain: the player reached melee
  range with `ForestYeti` object `258949`, sent 24 attack frames, received 7
  target `ObjectAttack` frames plus `ObjectStruck` / `DamageIndicator`, and HP
  fell `18 -> 3`. Follow-up evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivalattacktrace8-20260708/report.md`
  shifts the active gap to QA/admin control: `transferMap` commands return
  sent but do not change map/position, `event.spawn RakingCat0` produces no
  visible hostile Raking, and death/revive is not stable when the player
  remains beside a live hostile. Next gate: repair or replace that test-control
  lane, then rerun normal kill/XP/drop evidence.
- 2026-07-08: Rust `7111` combat-survival harness follow-up now separates the
  pickup route fix from the remaining retaliation proof. `qa-combat-survival.mjs`
  waits longer after a real drop click before injecting fallback movement, records
  pickup-route progress, supports `--qaSeedWindowMs`, `--allowQaCombatSeed`,
  `--preferQaCombatSeed`, and filters passive animals out of retaliation probes.
  Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-pickupwait5s-20260708/report.md`
  passed deterministic Blue Potion pickup (`GainedItem x1`, carried `0 -> 1`)
  and death/revive (`playerHp 0 -> 18`, respawn `0:330,270`) against
  `ws://127.0.0.1:7111/ws`. Follow-up evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-survivaltick-20260708/report.md`
  kept pickup/death green with clean StartGame, but the retaliation beat still
  cannot be accepted: RakingCat0 appeared late and no reliable attack/monster
  damage packet sequence reached the player within the probe window. Keep
  incoming-damage feel, hostile chase/retaliation, and real kill/XP/drop as
  active QA gates.
- 2026-07-08: Rust `7111` default self-camera pickup/death evidence now has a
  deterministic green lane. `page.tsx` exposes `state.authoritativePlayer`
  from packet-level self movement ACKs and no longer lets snapshot/render self
  unlock pickup/action gates ahead of the authoritative position. The combat
  QA harness now records inventory and belt carried items, WS URL/frames,
  pickup attempts, and an opt-in QA pickup seed. Backend routing now syncs
  personal-session drops into the shared Zone before pickup and forces
  position-sensitive personal commands to the current Zone transform; shared
  Zone chat still broadcasts normal chat, while GM chat commands such as
  `@DIE` fall back to the personal session. Evidence
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-authpickupseed7-20260708/report.md`
  connected to `ws://127.0.0.1:7111/ws`, completed 11/11 beats, passed
  deterministic Blue Potion pickup (`carried 0 -> 1`, `GainedItem x1`) and
  passed death/revive (`playerHp 0 -> 18`, respawn `0:330,270`). Remaining
  unaccepted gates: attack-kill/XP remain skipped in this seeded run, monster
  retaliation still failed to reduce HP, and missing `Sound/103.wav` /
  `Sound/144.wav` asset requests remain visible.
- 2026-07-07: Rust-gateway combat/effect evidence improved materially but is
  still not human-acceptance ready. The latest report
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-floaterfix30s-20260707/report.md`
  ran against `ws://127.0.0.1:7111/ws` (`gatewayIsRust=true`) and completed
  11/11 beats. The Web client now schedules targeted combat-confirm ticks for
  attack/range/cast commands, `qa-combat-survival.mjs` can approach targets via
  normal `walk` packets when DOM tile hitboxes are absent, and
  `DamageIndicator` now renders through `.scene-damage-floater`. Green signal:
  melee damage landed, target HP dropped (`minPercent=95`), 4 server damage
  indicators were observed, and DOM floater peak reached 1. Remaining fail/skip
  gates: the monster did not die within the 30s window, so XP and loot remain
  unproven; survival damage was not re-proven in that specific run because no
  second monster was available; `@DIE` still did not enter a dead/revive state;
  and UI/sound metadata 404s remain. Human acceptance must keep combat
  completion, death/revive, loot, XP, and full effect polish unaccepted until a
  fresh Rust `7111` run turns those gates green or explicitly accepted.
- 2026-07-07: Rust-gateway combat/effect anchor evidence is now valid red
  evidence instead of a `7110` or safe-zone false positive. The hardened
  `qa-combat-survival.mjs` path writes partial reports per beat, writes final
  reports atomically, avoids known Crystal field safe-zone circles, and starts
  combat from Woomyon anchor `1:315,100`. Report
  `docs/generated/player-qa/combat-survival-default-selfcamera-rust7111-anchor-20260707/report.md`
  connected to `ws://127.0.0.1:7111/ws` with `gatewayIsRust=true`, ran outside
  the safe zone, completed 11 beats with 10 ok, and then failed the real
  combat gates: melee attacks against `ForestYeti` produced no
  `ObjectStruck` / `DamageIndicator` / target `ObjectHealth` drop /
  `ObjectDied`; `RakingCat0` retaliation did not reduce player HP; `@DIE` did
  not enter a dead/revive state. Human acceptance must keep combat/effects
  unaccepted until gateway/Zone attack damage, incoming damage, death/revive,
  loot, XP, and damage-floater evidence are rerun green or explicitly accepted
  as remaining gaps. Same run also records missing `original-ui/Sound/103.wav`
  and missing Monster `007` original-ui metadata.
- 2026-07-07: Combat/effect-heavy evidence is now started but red, so human
  acceptance should not treat combat feel as accepted. The report
  `docs/generated/player-qa/combat-survival-default-selfcamera-20260707/report.md`
  contains 11 screenshots and a completed harness run, but attack-kill,
  damage-floater, and death/revive checks failed or skipped while the run used
  `7110` instead of Rust `7111`. The useful green signal is incoming damage:
  player HP dropped `18 -> 9`. Magic/effect QA attempts currently stall before
  report generation and need harness repair before they count as evidence.
- 2026-07-07: Default self-camera keyboard movement now covers held and
  chorded/cardinal repros. The chorded cardinal capture
  `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
  passed with 148 JPEG frames, 8 movement commands, final `329,270`, no failed
  assertions, no logical rollback, and no interaction pollution. A first held
  Shift+Right capture found one prediction cleanup rollback between run ACKs;
  the fixed rerun
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-queuedintentfix-jpeg-20260707-2000.json`
  passed with 122 JPEG frames, 8 movement commands, average ACK `198.5ms`, max
  `439ms`, final `345,270`, 0 logical rollback warnings, 0 failed assertions,
  and no console/network failures. Human acceptance should still request
  equal-duration native held/video evidence before treating sustained keyboard
  feel as accepted.
- 2026-07-07: Native/Web high-cadence temporal evidence is now like-for-like,
  and the default Web route uses Bevy self-camera + per-entity interpolation
  when the Bevy entity/map renderer is live. The DOM self overlay now cancels
  parent camera motion, so nameplate/health overlays stay pinned without visual
  jumps. Native evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-highfps-20260707-2000.json`
  passed with 104 JPEG frames, average sample delta `50.17ms`, and 4 native
  clicks. Matching default-URL Web same-direction evidence
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-samedir-4click-windowfps-content-default-selfcamera-jpeg-20260707-2000.json`
  passed with `ok=true`, 105 JPEG frames at ~50ms cadence, 4/4 walk ACKs,
  average ACK `139.25ms`, max `369ms`, no visual jumps, no interaction
  pollution, and no browser/network errors. Report
  `docs/generated/player-qa/movement-jitter/temporal-native-highfps-route-vs-web-windowfps-content-default-selfcamera-clicksequence-bichon-20260707.md`
  records normalized visual delta/sec `Crystal 63.7831` vs `Web 62` (Web ratio
  `0.972`) and changed-pixel/sec `Crystal 1.718936` vs `Web 1.7788` (Web ratio
  `1.0348`). Human acceptance should treat this as the current temporal-feel
  baseline for the Bichon route; broader held/chorded and combat/effect scenes
  still need the same evidence style.
- 2026-07-07: Native/Web 4-click temporal evidence is now repeatable. Native
  Crystal Computer Use route evidence
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-20260707-2000.json`
  passed with 23 captured frames and 4 real native clicks. Web now supports
  explicit `clickSequence` routes; the polluted first sample
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-4click-left-jpeg-20260707-2000.json`
  failed as expected because the final click hit `Teleport_Gilbert`, while the
  clean route
  `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-leftclean-4click-jpeg-20260707-2000.json`
  passed with `ok=true`, 29 JPEG frames, 4/4 walk ACKs, average ACK `204.25ms`,
  max ACK `590ms`, no interaction pollution, and no browser errors. The
  temporal report
  `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-route-vs-web-clicksequence-bichon-leftclean-20260707.md`
  records aggregate visual delta `Crystal 11.42` vs `Web 10.11` (ratio
  `0.8853`). Human acceptance should still wait for native higher-cadence or
  video-derived frame capture on the exact clean route.
- 2026-07-07: Native Crystal real-input temporal evidence is now automated for
  the first one-step click movement. `capture-original-computer-use.mjs`
  captured native Crystal frames at
  `docs/generated/player-qa/movement-jitter/original-motion-computeruse-click-620-520-20260707-2000.json`.
  The matched Web same-scene click-target evidence
  `docs/generated/player-qa/movement-jitter/web-motion-clicktarget-bichon-287-611-plus1-left-jpeg-1800ms-20260707-2000.json`
  passed with `ok=true`, one `walk DownRight`, final `288,612`, 10 JPEG
  frames, 0 failed assertions, 0 capture errors, and 0 interaction pollution.
  The aligned temporal report
  `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-click-vs-web-clicktarget-bichon-1800ms-20260707.md`
  records native mean visual delta `7.09` vs Web `4.51`. This is stronger than
  static screenshot evidence, but final acceptance still needs longer route/run
  samples and human review.
- 2026-07-07: Frame-cadence evidence is now part of the movement feel gate.
  Web keyboard movement capture
  `docs/generated/player-qa/movement-jitter/web-motion-keyhold-right-jpeg-cadence-20260707-2000.json`
  passed with `ok=true`, 23 full-stage JPEG frames, about 98ms average sample
  spacing, `Walk, Run, Run` movement to `335,270`, 0 frame capture errors, 0
  failed assertions, and 0 interaction pollution. The temporal report
  `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-static-vs-webjpeg-cadence-20260707.md`
  compares this Web trace with the current native Crystal synthetic-input
  sample and records aggregate visual delta `Crystal 0.37` vs `Web 7.09`.
  This should be treated as automation progress only: the native Crystal
  sample did not reliably move the real client, and follow-up SendInput
  keyboard/right-click/left-click probes remained near static deltas (`0.43`,
  `0.33`, `0.46`). Final human acceptance still needs native real-input/video
  cadence evidence.
- 2026-07-07: Held/chorded Bichon keyboard movement now has clean automated
  WebGL2 evidence after removing a backend starter-transfer rollback from full
  Crystal world runtime. The red repro
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-webgl2-movelog-20260707.json`
  hit `0:339,270`, batched transfer/reset packets, delayed ACKs by
  `7481/4066ms`, and rolled the player back toward `0:330,270`. Post-fix
  evidence
  `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
  passed with `ok=true`, 8/8 movement ACKs at
  `359/152/200/247/91/57/92/146ms`, final `345,270`, no visual/logical
  rollback, no ACK/stale-prediction/command-queue warnings, no interaction
  pollution, and Bevy WebGL2 packed/no DOM fallback. The cardinal chorded
  rerun
  `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
  also passed with 8/8 expected ACKs under 300ms. This closes the first
  long-held/chorded Web server-movement repro; human/automation acceptance
  should now compare native Crystal animation cadence and frame timing against
  these clean movement traces.
- 2026-07-07: Crowded Bichon click-route movement now has clean post-fix
  evidence, superseding the earlier red Bichon sample. The movement harness can
  avoid entity hit targets (`--avoidEntityHits true`), fail on interaction
  pollution (`--failOnInteractionPollution true`), choose route patterns such
  as `--routePattern leftHook`, and wait for the Bevy WebGL2 entity renderer
  before final assertions. Evidence:
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
  passed with `ok=true`, 4/4 movement ACKs, ACK latencies
  `490/164/33/5ms`, no entity-hit clicks, no non-movement gameplay frames,
  Bevy WebGL2 packed atlas, no DOM entity fallback, and no browser errors.
  The generated temporal summary
  `docs/generated/player-qa/movement-jitter/temporal-clickroute-postgrace1500-20260707.md`
  is now the current Crystal/Web movement evidence pointer for this slice. A
  no-frame-image rerun
  `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-rerun-20260707.json`
  also passed with ACK latencies `582/78/109/7ms`.
- 2026-07-07: Temporal movement comparison now has a first repeatable
  Crystal/Web evidence loop. Native Crystal was sampled through
  `apps/web/scripts/capture-original-movement.ps1` at
  `docs/generated/player-qa/movement-jitter/original-motion-frames-20260707-183007.json`
  with 16 frame images. Web movement capture now supports per-sample frame
  images plus mouse timing alignment (`--captureFrameImages true`,
  `--routeStepMs`, `--clickHoldMs`), and
  `apps/web/scripts/report-movement-temporal-parity.mjs` writes a compact
  JSON/Markdown summary. Current summary:
  `docs/generated/player-qa/movement-jitter/temporal-clickroute-runfix-20260707-183748.md`.
  It records the right-click run-prime fix: Web open-map click-route evidence
  `web-motion-clickroute-runfix-woods-20260707-183748.json` is `ok=true`,
  8/8 self ACKs, average ACK 164.75ms, max ACK 301ms, no console errors, and no
  non-favicon 404s. The Bichon route evidence
  `web-motion-clickroute-runfix-clean-20260707-183601.json` remains
  non-green with a missing self ACK after the first run, so crowded Bichon
  mouse-route feel remains a human/automation follow-up.
- 2026-07-07: Same-scene movement/resource evidence is clean for local Bichon
  `0:286,610`. The movement harness now seeds parity localStorage before login,
  waits for login controls, suppresses the Web-only tutorial overlay, and the
  asset set includes Crystal `NPC/09`, `Monster/011`, and `Monster/013` so the
  populated town scene does not produce missing-sprite noise. Evidence at
  `docs/generated/player-qa/movement-jitter/local-crystal-visual-baseline-keyseq-clean-20260707-181953.json`
  passed with `ok=true`, `strictStatus="settled"`, 4 sent movement frames,
  4 `UserLocation` ACKs, all 15 movement assertions green, no visual jumps, no
  logical rollback, no route spam, no residual movement plan, Bevy WebGL2
  gameplay layers drawn, 0 critical console errors, and 0 non-favicon 404s.
  This turns the prior 367-resource-404 movement run into a closed harness/data
  issue and leaves temporal Crystal-vs-Web feel recording as the next QA slice.
- 2026-07-07: Same-scene Crystal/Web visual parity evidence is now repeatable.
  The new `qa:visual-parity` report analyzes `r310-visual-watch` pairs and
  scores runtime health, 1024x768 layout, entity/nameplate coverage, and pixel
  similarity across world/HUD/minimap/chat regions. Current local Bichon
  evidence at
  `docs/generated/player-qa/visual-parity/current-20260707-181734-report.md`
  reports weighted 95%, runtime/layout/entities 100%, pixel trend 86%, and an
  estimated human visual/feel parity band of 88-100%. The previous Web-only
  objective tracker P1 is closed: the top-center tracker now defaults off and
  is available only through `?objectiveTracker=1` or
  `localStorage["mir2:objectiveTracker"]="1"`. The report has no recurring
  automated top gaps, so the next acceptance work should focus on movement
  recording, animation cadence, Crystal lighting/shadow timing, and live HUD
  state differences.
- 2026-05-19: Mobile landscape controls are now covered by live strict smoke.
  Player Web uses `nipplejs` as the touch joystick sensor and a Mir2 semantic
  adapter to send Crystal 8-way `walk` / `run` intents through the existing
  packet runtime/Zone movement path. The mobile overlay includes a left joystick,
  a right-bottom circular Run/Attack/Go/Pick/Bag/Char action wheel, belt/skill
  quick actions, and a portrait rotate prompt. Evidence:
  `pnpm --dir apps/web exec tsc --noEmit
  --pretty false`, `node --check apps/web/scripts/capture-web-movement-jitter.mjs`,
  and live mobile viewport smoke
  `mobile-controls-joystick-longhold3-20260519` passed with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no route spam,
  no stale prediction, no command queue warnings, no console errors, and no
  non-favicon 404s. Report/screenshot:
  `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.json`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-joystick-longhold3-20260519.png`.
  The circular right-bottom wheel refinement was additionally verified by
  `mobile-controls-wheel-short-20260519` with `ok=true`,
  `strictStatus="settled"`, no visual jumps, no logical rollback, no stale
  prediction, no command queue warnings, no console errors, and no non-favicon
  404s. Report/screenshot:
  `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.json`
  and `docs/generated/player-qa/movement-jitter/mobile-controls-wheel-short-20260519.png`.
- 2026-05-19: Production R2 asset delivery is on the custom Cloudflare domain
  `https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c` instead of the
  raw `r2.dev` public URL. `infra/cloudflare/mir2-r2-asset-cache` is deployed on
  `assets.mir2.obelisk.build/*` with an R2 binding and edge Cache API; repeated
  GET probes for gameplay sprite frames return `x-mir2-edge-cache: HIT` and
  `cf-cache-status: HIT`. Production `/api/asset-manifest` reports
  `remoteAssets.assetBaseUrl="https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c"`
  and `remoteAssets.objectPrefix="mir2/v/37596e16d64fde7c"`. `/bevy-runtime`
  remains same-origin with a build-versioned JS/WASM query so R2 runtime copies
  cannot mismatch the current Vercel build. Evidence:
  `MIR2_WEB_BASE_URL=https://mir2.obelisk.build/?codexBust=domain-smoke-final-... npm run smoke:playable-metrics -- --runId codex-r2-assets-domain-prod-smoke-final --waitTimeoutMs 240000`
  passed with `ok=true`, cold first playable 3563.4ms, warm first playable
  3775.9ms, 517/517 prewarm ok, warm transfer bytes 727,992, no critical
  console errors, and no non-favicon 404s.
- 2026-05-18: Remote R2/CDN asset release chain is implemented and live R2 verified. `/api/asset-manifest` now exposes versioned `remoteAssets`, the Service Worker can fetch static game assets from a configured CDN base on cache miss while storing them under the original same-origin CacheStorage key, and `assets:remote:build` stages the manifest-declared critical packs plus Bichon scene frames for R2. Evidence: `npm run assets:remote:build -- --baseUrl http://127.0.0.1:13014 --assetBaseUrl https://assets.example.com/mir2/v/{version} --runId codex-r2-release-smoke` wrote `docs/generated/remote-assets/codex-r2-release-smoke/remote-asset-release.json` and `latest-remote-asset-release.json` with `stats.fileCount=512`, `stats.totalBytes=64626176`, `stats.missingCount=0`, and object prefix `mir2/v/37596e16d64fde7c`. Live upload then created/used bucket `mir2-web3-assets`, uploaded 513/513 objects remotely totaling 65,000,146 bytes, enabled public r2.dev access at `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev`, applied GET/HEAD CORS, and republished the release manifest with CDN base `https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev/mir2/v/37596e16d64fde7c`. Public CORS probes for `original-ui/Prguse/4.png` returned 200 with `Access-Control-Allow-Origin: *`, and the public release manifest reports `stats.fileCount=512`, `stats.missingCount=0`.
- 2026-05-18: Game cache, cache maintenance, persistence diagnostics, budget guardrails, and first-playable evidence passed. `smoke:cache-metrics` validates cold/warm resource caching, critical prewarm, scene cache hits, populated Mir2 CacheStorage entries, storage usage/quota diagnostics, and no critical browser errors. `smoke:cache-maintenance` seeds a legacy Mir2 CacheStorage bucket, verifies manifest-version cleanup removes it, then calls the QA reset API to clear active caches and unregister the Service Worker. Latest maintenance evidence at `docs/generated/player-qa/cache-metrics/cache-metrics-codex-cache-budget-maintenance-smoke-final.json` records `ok=true`, 511/511 prewarm ok, warm 118 CacheStorage entries, 62272086 storage usage bytes, storage persistence requested but not granted in fresh headless Chrome, all default budget assertions true (`<=1000` prewarm requests, `<=2500` entries, `<=256MiB` usage), legacy cleanup true, reset API available true, 3 caches deleted, 1 SW scope unregistered, and `afterReset.cacheNames=[]`. `smoke:playable-metrics` drives `demo/demo` through Gateway login, character select, `StartGame`, `UserInformation`, Bichon scene readiness, and first playable frame before repeating a warm pass in the same Chrome profile. Latest playable storage evidence at `docs/generated/player-qa/cache-metrics/cache-metrics-codex-playable-storage-smoke-final.json` records `ok=true`, cold first playable 1659.6ms, warm first playable 2224.9ms, 3/3 scene hits in both passes, 511/511 prewarm ok in both passes, warm CacheStorage 2 caches / 555 entries / 67045268 usage bytes, no prewarm failures, no critical console errors, and no non-favicon 404s.
- 2026-05-16: All-map resource/gameplay evidence passed on the full Crystal client root. `audit:crystal-map-coverage` now records 463/463 maps present/parseable, unsupported map types 0, parse errors 0, missing minimap indices `[]`, missing sampled map libraries 0, `visualFallbackRisk.mapCount=0`, and Crystal empty/out-of-range source frame references tracked separately as no-draw behavior. `audit:crystal-map-gameplay` records 1999 movement rows checked with 1906 direct transfers, 93 Crystal-ignored/deferred/special rows, movement failures 0, 6341 respawns with 6293 walkable-candidate rows and 48 Crystal-inert no-candidate warnings, respawn failures 0, 375 NPC rows with scripts found, 7 empty placeholder warnings, unimplemented NPC commands 0, and static map semantic failures 0 across safe zones, safe-zone spell flags, doors, cell lights, fishing cells, drop rules, and light/feature flags. Web `npx tsc --noEmit`, Simulation fmt check, focused `crystal_manifest_movements` 2/2, and focused `spread_slots` 2/2 also passed. Human visual/feel acceptance is still the final gate, but the prior automated map-source blocker is closed.
- 2026-04-29 R303: All-map frontend source-resource audit was added and passed via `npm.cmd run audit:crystal-map-coverage --prefix apps\web`, with evidence archived at `docs/generated/map/r303-crystal-map-coverage.json`. This was the first all-map source baseline; its then-open source-frame/minimap warnings are superseded by the 2026-05-16 full-client map coverage and gameplay audits above.
- 2026-04-28 R302: Windows original-client comparison evidence is archived at `docs/generated/player-qa/r302-original-client/summary.json`. Original Crystal `Server.exe` listened on `127.0.0.1:7000`, visible `Client.exe` reached select/game with retained character `R302HeroB`, and screenshots were captured for login rejection, select, game welcome, and unobstructed game HUD. Matching web evidence was refreshed through Stage 5 UI smoke at `http://127.0.0.1:3002` with 88 screenshots and 0 critical console errors, archived as `docs/generated/player-qa/r302-original-client/web-stage5-ui-smoke-manifest.json`. This is visual-reference evidence only; it does not close whole-project 100% Accepted because the comparison is not yet a deterministic same-scene human visual/feel pass.
- 2026-04-28 R301: Windows final automated Candidate acceptance pack passed and is summarized in `docs/generated/player-qa/r301-summary.json`. Web `tsc --noEmit` and `npm.cmd run build` passed; map API smoke wrote `docs/generated/map/r301-crystal-map-api.json` with 18/18 requests and 0 failures; minimap smoke wrote `docs/generated/assets/r301-minimap-assets.json` with 0 failures and a historical preview-index warning later closed by the 2026-05-16 full-client map audit; WS load wrote `docs/generated/load/r301-ws.json` with 64/64 ready, 0 errors, and keepalive p95 637 ms; Stage 5 UI smoke wrote `docs/generated/player-qa/r301/stage5-ui-smoke-manifest.json` with 88 screenshots, 0 critical console errors, and 32 compact text nodes checked without overflow. Rust verification passed `mir2-game-data` 27/27, `mir2-gateway` 55/55 plus packet-trace bin 15/15, `mir2-admin-api` 22/22, and `mir2-simulation` 674/674. Temporary gateway/web services were stopped and ports 7000/7110/3002 verified closed. Human Crystal visual/feel acceptance remains required for whole-project 100% Accepted.
- 2026-04-28 R300: backend/server tracked-slice packet parity is accepted under the explicit stable-diff policy. R298 supplies the 9/9 stable-clean live Crystal matrix, R299 identifies strict exact dirtiness as Crystal dynamic state, and R300 records/enforces the stable-diff acceptance mode in `docs/PACKET-PARITY-ACCEPTANCE.md` and `docs/generated/packet-traces/r300-stable-acceptance.json`. This does not replace the human Crystal visual/feel pass required for whole-project 100% Accepted.
- 2026-04-28 R297: Windows automated player QA refresh passed with `CRYSTAL_CLIENT_ROOT=E:\mir2\Crystal\Build\Client\Debug`. Web `npm.cmd run build`, `tsc --noEmit`, map API smoke 18/18, minimap smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 full-client map audit, WS load 64/64 ready with 0 errors and keepalive p95 632 ms, and Stage 5 UI smoke 88 screenshots with 0 critical console errors passed. This round also exported missing original scene sprite libs (`NPC/07`, `NPC/08`, `NPC/16`, `NPC/27`, `NPC/45`, `NPC/52`, `NPC/83`, `Monster/000`, `Monster/139`), fixed map-transfer minimap state from gateway `MapInformation`, and hardened concurrent account-store file saves for Windows load. Rust validation passed `mir2-simulation` 674/674, `mir2-gateway` 55/55 plus packet-trace bin 14/14, `mir2-admin-api` 22/22, `fmt --check`, and `git diff --check`. Human Crystal visual/feel acceptance remains required for 100% Accepted.
- 2026-04-26 R224: Local packet trace matrix evidence restored. `packet_trace --list-flows` works, `mir2-gateway` passes 53/53 including packet trace bin tests 6/6, and require-local `packet_trace --matrix` wrote 9 TCP-traceable artifacts under `docs/generated/packet-traces/r224-matrix` with `localOk=true`. Live Crystal trace comparison was later refreshed by R298 and accepted under R300 stable-diff policy.
- 2026-04-26 R223: **100% Candidate** automated gate passed. Stage 5 UI smoke captured 88 screenshots with advanced Stage 5 systems state and compact Mail/Report bounds. Direct `next build`, `tsc --noEmit`, map API smoke 18/18, minimap asset smoke 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, WS load 64/64, `mir2-game-data` 22/22, `mir2-gateway` 47/47, full `mir2-simulation` 664/664, `fmt --check`, and `diff --check` passed. Human Crystal visual/feel acceptance remains required for 100% Accepted.
- 2026-04-26 R184: direct `next build`, Crystal minimap smoke, Crystal map API smoke, Stage 5 UI smoke with 10 screenshots, gateway health on `127.0.0.1:7110`, and websocket load 64/64 ready passed. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R185: Stage 5 UI smoke passed with 11 screenshots, named desktop 1024x768 and compact 820x640 viewport metadata, compact layout bounds assertions, and `stage5-compact-game.png`. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R186: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with compact visible-text overflow checks for 33 core text nodes. The compact minimap title/Safe Zone header is fixed. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R187: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 14 screenshots and minimap collapse, BigMap re-expand, and Mail open flow evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R188: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 17 screenshots and belt horizontal, vertical, rotate-back, and close evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R189: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 18 screenshots and belt hotkey `1` Red Potion use evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R190: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 21 screenshots and inventory bag1/bag2/quest tab evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R191: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 25 screenshots and character char/stats1/stats2/spells tab evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R192: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 27 screenshots and storage page1/page2-locked/page1 evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R193: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 31 screenshots and chat Shout filter, All restore, Settings, collapse/restore, and Report evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R194: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 35 screenshots and system menu open plus Character/Inventory/Quest action evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R195: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 36 screenshots and expanded storage rent/unlock evidence from storage page 2. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R196: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 37 screenshots and inventory Red Potion use evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R197: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 38 screenshots and inventory Dagger equip evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R198: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 40 screenshots and HUD Skill/Option character-panel routing evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R199: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 42 screenshots and inventory Drop Gold evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R200: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 43 screenshots and inventory Wooden Sword move evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R201: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 45 screenshots and inventory Red Potion Split Item evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R202: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 47 screenshots and inventory Blue Potion item-drop evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R203: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 48 screenshots and Character Dagger remove evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R204: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 49 screenshots and belt mouse-use Red Potion evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R205: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 51 screenshots and Sell Item no-service preservation evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R206: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 54 screenshots and Store Item no-service preservation evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R207: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 57 screenshots and Take Back no-service preservation evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R208: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 58 screenshots and Set Storage Password panel evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R209: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 60 screenshots and Set Storage Password mismatch/no-service submit evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R210-R218: web `tsc --noEmit`, direct `next build`, and Stage 5 UI smoke passed with 71 screenshots and Mail/Report/NPC panel state, broad Stage 5 systems state, guild/group chat filters, Character repair/special-repair, ground item/gold pickup, combat target state, system-menu QA/transfer routes, Battle Focus spell casting, and compact inventory panel bounds evidence. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R219-R222: web `tsc --noEmit`, direct `next build`, Stage 5 UI smoke passed with 85 screenshots, map API smoke archived 18/18 successful requests, minimap asset smoke archived 0 failures with a historical preview-index warning later closed by the 2026-05-16 map audit, and WS load refreshed at 64/64 ready with 0 errors. Login/select lifecycle, confirmed character delete/recreate, compact multi-panel bounds, compact system-menu overflow fix, and NPC dialog link-capable rendering are covered. Human Crystal visual/feel acceptance remains required.
- 2026-04-26 R223: Stage 5 UI smoke passed with 88 screenshots, adding advanced systems state and compact Mail/Report bounds. This is the automated Candidate gate; human Crystal visual/feel acceptance remains required.

## Phase 1: Smoke Acceptance

Estimated human time: 2-4 hours.

Run after major backend/frontend milestones.

- Login with a fresh account.
- Create/select a character.
- Enter game and verify the first viewport looks coherent.
- Walk and run in four directions.
- Open/close inventory, character, belt, NPC dialog, and storage/shop panels where available.
- Fight a representative starter monster.
- Pick up gold and item drops.
- Use a potion from inventory and belt.
- Log out and reconnect.
- Confirm the first game viewport visually reads as a Crystal-like client rather than a generic web dashboard.

Pass criteria:

- No crash or broken panel.
- No unreadable or overlapping critical text.
- Core controls respond without obvious delay or wrong target behavior.

## Phase 2: System Matrix Acceptance

Estimated human time: 12-24 hours.

Run near 85-92% project completion.

Panel matrix:

- HUD: HP/MP bars, experience, gold/credit, target state, combat feedback.
- Chat: filtering, scroll, input, system messages, size/settings/report entry points.
- Belt: slots 1-6, rotation where available, hotkey item use, empty/full slot states.
- Minimap: collapse/expand, mail/map buttons, safe-zone/map readability.
- Inventory: bag1/bag2/quest tabs, item use/drop/equip/remove/move/merge/split/sell/drop gold/store/take back.
- Character: character/stats/spells tabs, equipment slots, durability display, repair/special repair entry points.
- NPC: dialog links, input submission, branch flow, buy/sell/repair/storage/craft/refine surfaces.
- Storage: unlock/set/change/remove password flows, expanded storage confirmation where available.
- Quest/mail/report/system menus: open/close, readable state, no critical overlap.
- Scene: target selection, approach/primary action, ground drop pickup, map transfer, logout/reconnect.

Backend-facing checks:

- movement and map transfer
- PvE melee/ranged attacks
- death/revive where available
- harvest monsters
- drop ownership and pickup
- item use/drop/split/merge/sell/buy/repair
- NPC dialog branches and input pages
- storage, shop, craft, repair pages
- save/reconnect persistence

Frontend-facing checks:

- login/select/game layout
- inventory/equipment/belt drag and click behavior
- tooltips and item metadata
- NPC link selection and input flow
- combat target feedback and HP/MP display
- map/minimap readability
- responsive layout at accepted desktop/mobile sizes

Pass criteria:

- Representative flows match Crystal behavior closely enough for Candidate status.
- Any accepted visual/feel differences are recorded in `docs/FRONTEND-1TO1-GAPS.md` or this script.

## Phase 3: Crystal Comparison Acceptance

Estimated human time: 8-16 hours.

Run near 92-97% project completion.

- Compare screenshots for login/select/game panels against Crystal references.
- Compare packet trace reports for representative flows when a live Crystal endpoint is configured.
- Play the same route in Crystal and `mir2-web3`:
  - start game
  - move to a nearby combat area
  - kill and loot monsters
  - use consumables
  - interact with NPC/shop/storage
  - transfer maps
  - reconnect
- Compare the panel matrix against Crystal screenshots or direct Crystal play for every implemented panel.

Pass criteria:

- No high-impact packet-visible mismatch remains untriaged.
- No major visual/layout mismatch blocks normal play.

## Phase 4: Final Candidate Acceptance

Estimated human time: 10-20 hours.

Run only after the Coordinator marks **100% Candidate**.

- Complete a 2-4 hour continuous play session.
- Complete one fresh-account route and one existing-account reconnect route.
- Visit representative maps from the current accepted map list.
- Exercise representative monsters, items, NPCs, shop/storage, and map transfer.
- Review the final known-gap list.
- Confirm frontend gaps in `docs/FRONTEND-1TO1-GAPS.md` are fixed, accepted, or explicitly deferred.

Pass criteria:

- No blocker or high-severity issue remains.
- Medium issues are either fixed or explicitly accepted.
- The user confirms `100% Accepted`.

## Reporting Format

For each issue, record:

```text
Area:
Route/step:
Expected Crystal behavior:
Actual mir2-web3 behavior:
Screenshot/trace:
Severity: blocker | high | medium | low
Decision: fix | accept | defer
```
