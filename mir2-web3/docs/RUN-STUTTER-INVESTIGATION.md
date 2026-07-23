# Run-stutter ("奔跑卡") investigation - continuation brief

> **Status (2026-07-12 final presentation-clock closure):** Walk/Run arithmetic
> was small, but five independent owners made the visible action non-atomic.
> React eased a one-tile logical center while Bevy interpolated pixels again;
> map and entity effects submitted separate center revisions; one rejected pose
> immediately switched the scene to a TypeScript clock; Bevy shadow assumed a
> fixed Run distance instead of the command target; and local phase 0 advanced
> on the nearest shared 100ms pulse, so a command near that pulse could display
> its first frame for only about 20ms.
>
> The final pipeline has one presentation owner and one transaction boundary.
> Page state gives Bevy raw command/ACK endpoints, map and entity submissions
> publish one coherent provenance tuple, the DOM holds the last accepted pose
> for a bounded 250ms watchdog, shadow validation consumes the explicit target,
> and local phases advance from `started + 100ms` at most once per display
> iteration. A stalled renderer extends the action instead of replaying missed
> phases. Crystal source remains the reference: mounted Walk is eight 100ms
> phases with rightward offsets `-6..-48px`; mounted Run is six phases over
> three cells at `-24..-144px`; logical location is already the destination
> while the render anchor finishes the residual pixels.
>
> WebGPU r12 passes 33/33 with 2/6ms ACKs; WebGL2 r16 passes 33/33 with 7/2ms
> ACKs. Both observe every exact phase, a pinned self sprite, zero split or
> synthetic centers, zero phase/shadow/pose warnings, and no critical console
> or network errors on `bevy-bd9004a17f2873ea`. Runtime 99/99, frontend logic,
> TypeScript, dual-backend smoke, shared Zone 152/152, and focused Gateway
> movement tests pass. Therefore Web is not intrinsically incapable and a
> PC-only Bevy rewrite is not required for this movement defect. Remaining
> visual dissatisfaction belongs to light/effect composition, scene contents,
> and human acceptance, not to an unresolved Walk/Run clock split.

> **Status (2026-07-12 mounted movement closure):** the remaining mounted feel
> defect was not WebGPU throughput, map rendering, or an unavoidable browser
> limitation. Movement semantics were split across four authorities: Zone
> distance/cooldown, Web prediction/routing, scene motion, and Bevy Pose. Web
> correctly predicted a three-cell mounted Run, but personal Session
> `MountUpdate` never reached the shared Zone, which therefore ACKed only two
> cells. Separately, the Pose parser hard-coded six frames and rejected mounted
> Walk frame indexes 6/7, producing a 200ms atomic-overlay fallback window.
>
> Gateway now forwards owner mount/sneak/buff packets into Zone state, Zone
> retains PauseBuff, and `phaseCount` travels through Web, Bevy, Pose JSON, and
> TypeScript parsing. Strict Release evidence
> `docs/generated/player-qa/movement-jitter/movement-mounted-walk8-run3-webgpu-20260712-r6.json`
> passes 27/27: Walk 1 tile/8 phases, Run 3 tiles/6 phases, ACK 18/22ms, Pose
> 2/2 within 26ms, final `(4,0)`, and zero rollback, stair-step, atomicity,
> console, or 404 warnings. This establishes that PC-only Bevy is not required
> to fix the run pipeline; consistent authority and metadata were required.

> **Status (2026-07-12 Zone-owned cadence/live-outbound closure):** the local
> scheduler fix now extends through shared-world observation. The per-Zone owner
> combines bounded Walk/Run/Turn execution with one monotonic 300ms global Tick;
> late ticks coalesce instead of catching up in a burst. Personal Session Tick
> no longer advances global Zone time, player movement, or shared-drop expiry.
> Realtime `UserLocation`, player appearance/removal, Turn, Walk, and Run use a
> bounded token-fenced socket channel and therefore do not wait for the observer
> Session mailbox. Full/closed channels fall back to that mailbox, preserving
> reliability.
>
> Strict Release proof uses a deliberately hostile configuration: personal Tick
> is 5000ms and observer pulses are disabled. Report
> `docs/generated/player-qa/two-client-zone/two-client-zone-zone-owned-cadence-tick5000-release-20260712.json`
> passes with 12ms observer movement latency, 16 entities on both clients, one
> Bevy remote-motion event, 29 packed-offset matches, and zero decode errors,
> queue drops, console errors, or 404s. Blocked-private runtime, unique cadence,
> queued-Run-without-Session-Tick, fencing/fallback, and delayed combat tests are
> green; Simulation remains 148/148. Earlier local Release baselines remain
> 15/14ms degradation, 17/21ms keyboard ACK with 23/24ms pose/sink, and 11/2ms
> event-observed ACK. Remaining stutter work is full non-movement command
> actorization plus mounted/sprint fidelity, not observer mailbox polling.
> Debug `0xc0000005` still predates this work and correlates with WHEA corrected
> machine checks; hardware stabilization remains required for a trustworthy soak.

> **Status (2026-07-12 degradation and release-gateway pass):** the first raw
> standstill/expired Run transition is now deterministic. Web uses one ACK
> classifier before render mutation and during controller reconciliation; the
> one-cell first Run ACK is confirmed, while genuinely off-path ACKs still take
> the native-like snap plus 400ms input lock. Shared Zone degrades an unprimed
> standstill Run to Walk and passes 148/148 tests. Release evidence
> `docs/generated/player-qa/movement-jitter/movement-protocol-expired-run-degrades-release-202607120745.json`
> reports 16/99ms ACKs, one degradation, no correction, and `(2,0)` total;
> normal UI evidence
> `docs/generated/player-qa/movement-jitter/movement-normal-walk-run-chain-release-202607120750.json`
> reports 22/28ms ACKs, 17/1ms pose latency, no degradation/correction, and
> `(3,0)` total. The prior Debug 2375ms tail co-arrived with monster packets and
> identified the remaining scheduler defect: private world Tick runs inside the
> same socket future. Release removes the practical repro but not the race, so
> the active fix is independent bounded Zone ingress, not another finite grace
> extension.

> **Status (2026-07-12 default shared-clock and additive pass):** clean local
> movement no longer waits for React publication or movement ACK before visual
> progress. Bevy local self/camera ownership and synchronous pose commit are now
> the default, driven by one Crystal-compatible 100ms scene pulse across six
> walk phases. Default continuous capture reached a 10ms maximum
> command-to-pose delay; the committed keyboard capture reached 15ms across 4/4
> commands and returned exactly to `328,275`. Both had zero main-thread long
> tasks, failed assertions, input pollution, console errors, and 404s. The
> native and default-Web four-action windows each span exactly 2701ms. Explicit
> rollback via `?bevyLocalMotion=0&bevyPoseCommit=0` is also strict-green with
> both flags inactive and 2/2 command/ACK matches. The previous normal-grid
> capture produced 11 long tasks at 59-69ms; the no-grid/shared-clock captures
> produce zero. The last 25 additive map sprites have moved from DOM to Bevy's
> `SrcAlpha + One` material, so both WebGPU and WebGL2 smokes finish with zero
> DOM world sprites. Evidence:
> `docs/generated/player-qa/movement-jitter/movement-default-shared-clock-keyboard-committed-ref-202607120617.json`,
> `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-default-shared-clock-horizontal-20260712-001.md`,
> `docs/generated/player-qa/movement-jitter/movement-explicit-legacy-rollback-202607120623.json`,
> and
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-default-shared-clock-202607120620.json`.
> Remaining stutter/fidelity work is bounded to real correction/degraded-run,
> mounted eight-frame movement, sprint prediction, and non-player scene/effect
> parity. Full-window visual-delta ratios include different world contents and
> capture geometry, so they are not a standalone movement regression signal.

> **Status (2026-07-10 release early-pose and map-incremental pass):** the new
> in-page command-to-accepted-sink probe removes CDP sampling distortion and
> pairs each walk/run only inside its own command interval. On release WebGPU,
> the exact four-left route reached the synchronous `localCommand` pose sink in
> `14/18/32/16ms` (maximum 32ms, hard budget 75ms), ended exactly at `328,275`,
> and produced no jumps, provenance failures, drops, console errors, or 404s:
> `docs/generated/player-qa/bevy-movement-shadow/bevy-movement-shadow-webgpu-20260710220403-44ba1f45-report.json`.
> The earlier debug-WASM runs at roughly 109-133ms were build-profile overhead,
> not evidence against WebGPU; normal `npm run dev` therefore builds release WASM,
> with `npm run dev:debug-runtime` reserved for diagnostics. Separately, map-state
> semantic deduplication and retained Rust tile generations reduced the same route
> from about 70 producer revisions/second to five sampled states total (initial
> plus four real center changes). The strict 50ms-sampled default-off route still
> exposed one 182ms TypeScript camera-offset hold, while the compatibility rerun
> at 75ms sampling remained functionally green. This supports the Bevy route as
> the performance candidate but does not yet justify making it default: exact
> native clean/correction/degraded-run temporal capture and human feel remain the
> acceptance gate. Dual-backend runtime evidence is green at
> `docs/generated/player-qa/bevy-runtime-backends/bevy-runtime-backends-20260710221430.json`.

> **Status (2026-07-10 post-stepped-motion native/Web rerun):** the latest
> automated comparison is valid and close-cadence on both sides. The earlier
> native black-screen capture was discarded; Crystal was relaunched from
> `E:\mir2\Crystal\Build\Client\Debug\Client.exe`, logged in with
> `cdx0708235326`, and captured through Computer Use +
> `capture-original-window-frames.ps1` as
> `docs/generated/player-qa/movement-jitter/original-crystal-valid-step-route-20260710.json`
> with 90 JPEG frames at `50.12ms` average cadence and four real clicks. Web
> was rerun against the live `7111` Gateway using a fresh QA account, natural
> spawn, and headed window-frame capture:
> `docs/generated/player-qa/movement-jitter/web-crystal-window-fresh-step-route-20260710.json`
> with 86 JPEG frames at `50.11ms`, 3/3 walk ACKs, average ACK `233ms`, max
> ACK `457ms`, 0 failed assertions, 0 interaction pollution, 0 critical console
> errors, and 0 non-favicon 404s. Final report
> `docs/generated/player-qa/movement-jitter/temporal-crystal-native-vs-web-window-20260710.md`
> is `ok=true` and reports normalized visual delta/sec Crystal `68.0367` vs Web
> `37.9166` (Web ratio `0.5573`). Important nuance: the Web movement/ACK path is
> green; the remaining visible gap is now a render/scene-motion energy gap, not
> a rollback/correction failure. Also confirmed during the run: production Web
> correctly rejects `debug crystal transfer`, so future parity captures must use
> natural accounts, admin-only setup, or a safe QA harness rather than
> production debug teleports.


> **Status (2026-07-07 combat/effect-heavy probe):** the next fidelity lane is
> open but red. Combat QA
> `docs/generated/player-qa/combat-survival-default-selfcamera-20260707/report.md`
> produced 11 screenshots and completed all harness beats on the default Bevy
> WebGL2 route, but it ran through `7110` because Rust `7111` was unavailable.
> Attack-kill and damage-floater checks did not pass, death/revive failed, and
> field transfer/monster engagement needs harness hardening. The useful pass is
> player HP dropping `18 -> 9`, proving at least one incoming-damage surface.
> Next: run the same probe on `7111` and stabilize attack-kill,
> `.scene-damage-floater`, death/revive, loot, and XP evidence.

> **Status (2026-07-07 default self-camera held/chorded evidence):** default
> self-camera now has strict-green keyboard evidence, not only click-route
> evidence. Chorded/cardinal Web capture
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-default-selfcamera-windowfps-content-jpeg-20260707-2000.json`
> is `ok=true`, 148 JPEG frames, 8 movement commands, no rollback, no
> interaction pollution, and Bevy WebGL2 packed/no DOM fallback. Held
> Shift+Right first reproduced one non-render logical rollback between run ACKs
> (`predicted 332,270 -> server 331,270`); the fix treats fresh, unconsumed
> direction `queuedMoveIntent` as movement transport evidence so sustained
> held-key prediction is not cleared between ACKs. Verified rerun
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-default-selfcamera-windowfps-content-queuedintentfix-jpeg-20260707-2000.json`
> is `ok=true`, 122 JPEG frames at ~50ms cadence, 8 movement commands, average
> ACK `198.5ms`, max ACK `439ms`, final `345,270`, 0 rollback warnings, 0
> failed assertions, 0 capture errors, and no console/network failures. Next
> investigation step: equal-duration native held/video capture and
> combat/effect-heavy scenes.

> **Status (2026-07-07 default self-camera temporal evidence):** native Crystal
> and default-URL headed Web now both have near-50ms content-only evidence for
> the current four-click Bichon route. `capture-original-window-frames.ps1`
> captured native Crystal while real Computer Use clicks drove the client:
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-highfps-20260707-2000.json`
> with 104 JPEG frames, average sample delta `50.17ms`, and four real clicks.
> Web now requests Bevy self-camera + per-entity interpolation by default when
> the Bevy entity/map renderer is live, and the residual DOM self overlay
> cancels the parent camera transform so nameplate/health overlays stay pinned.
> Default Web evidence
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-samedir-4click-windowfps-content-default-selfcamera-jpeg-20260707-2000.json`
> is strict-green with 105 JPEG frames at ~50ms cadence, 4/4 Walk ACKs,
> average ACK `139.25ms`, max `369ms`, no visual jumps, no interaction
> pollution, and no console/network failures. The final report
> `docs/generated/player-qa/movement-jitter/temporal-native-highfps-route-vs-web-windowfps-content-default-selfcamera-clicksequence-bichon-20260707.md`
> records normalized visual delta/sec Crystal `63.7831` vs Web `62` (Web ratio
> `0.972`) and changed-pixel/sec Crystal `1.718936` vs Web `1.7788` (Web ratio
> `1.0348`). Next stutter/fidelity step: repeat this evidence style on
> held/chorded movement and combat/effect-heavy scenes, then tune HUD/chat and
> effect-layer temporal polish.

> **Status (2026-07-07 native/Web 4-click temporal):** the real-input native
> capture path now supports sustained Crystal movement routes. Native evidence
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-route-bichon-4click-20260707-2000.json`
> captured 23 frames and 4 real clicks through Computer Use. Web evidence now
> has explicit `clickSequence` route replay; the first same-area sample
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-4click-left-jpeg-20260707-2000.json`
> correctly failed after hitting `Teleport_Gilbert`, while the clean route
> `docs/generated/player-qa/movement-jitter/web-motion-clicksequence-bichon-leftclean-4click-jpeg-20260707-2000.json`
> passed with `ok=true`, 29 JPEG frames, 4/4 ACKs, average ACK `204.25ms`, max
> `590ms`, and 0 interaction pollution. Report
> `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-route-vs-web-clicksequence-bichon-leftclean-20260707.md`
> reports aggregate visual delta `Crystal 11.42` vs `Web 10.11` (ratio
> `0.8853`). Next stutter/fidelity step: raise native capture cadence or
> derive frames from video, then replay the exact clean route on both clients.

> **Status (2026-07-07 native Computer Use movement):** the native movement
> capture blocker has a working path. `capture-original-computer-use.mjs` uses
> Computer Use window capture/input to drive the real `Legend of Mir 2` window
> and save frame evidence:
> `docs/generated/player-qa/movement-jitter/original-motion-computeruse-click-620-520-20260707-2000.json`.
> The matched Web `clickTarget` sample
> `docs/generated/player-qa/movement-jitter/web-motion-clicktarget-bichon-287-611-plus1-left-jpeg-1800ms-20260707-2000.json`
> reached `288,612` with a single clean `walk DownRight`. The aligned report
> `docs/generated/player-qa/movement-jitter/temporal-native-computeruse-click-vs-web-clicktarget-bichon-1800ms-20260707.md`
> is `ok=true`, with native mean visual delta `7.09` vs Web `4.51`. Next
> stutter/fidelity step: repeat with longer held/run routes or video-derived
> frame extraction so the comparison is based on equivalent sustained motion.

> **Status (2026-07-07 frame-cadence automation):** Web held-key movement now
> has real per-frame evidence instead of only movement ACK/final-state
> telemetry. JPEG full-stage capture
> `docs/generated/player-qa/movement-jitter/web-motion-keyhold-right-jpeg-cadence-20260707-2000.json`
> is `ok=true`, captures 23 frames at about 98ms average spacing, reaches
> `335,270`, and records no frame-capture/assertion/pollution failures.
> `docs/generated/player-qa/movement-jitter/temporal-keyhold-native-static-vs-webjpeg-cadence-20260707.md`
> adds frame-diff scoring and reports aggregate visual delta `Crystal 0.37` vs
> `Web 7.09`. The native Crystal side is not accepted yet because current
> Win32 synthetic keyboard/click attempts captured frames but did not reliably
> move the real client. SendInput scan-code keyboard, right-click target, and
> left-click target probes also stayed near static deltas (`0.43`, `0.33`,
> `0.46`). Next investigation step: get native real input or video-capture
> automation working, then compare Crystal's actual animation cadence against
> the clean Web JPEG trace.

> **Status (2026-07-07 held/chorded WebGL2 follow-up):** the next Bichon
> keyboard repro found and closed a server data/config issue, not a render
> hitch. Held Shift+Right reached `0:339,270`, then the full Crystal world
> runtime applied the leftover starter demo `starter-east-field-gate`, batched
> transfer/reset packets, delayed ACKs by `7481/4066ms`, and visually looked
> like a rollback/stutter. `with_crystal_world_runtime()` now clears starter
> demo transfers, matching the generated-Crystal-transfer source of truth. The
> fixed held-run evidence
> `docs/generated/player-qa/movement-jitter/web-motion-heldrun-bichon-right-worldtransferfix-20260707.json`
> is `ok=true`, 8/8 ACKs at `359/152/200/247/91/57/92/146ms`, no rollback,
> no ACK warnings, and Bevy WebGL2 packed/no DOM fallback. The cardinal
> chorded rerun
> `docs/generated/player-qa/movement-jitter/web-motion-keyseq-bichon-cardinal-worldtransferfix-rerun-20260707.json`
> is also `ok=true`. Remaining investigation target: native Crystal vs Web
> animation frame cadence, now that this long-run server rollback class is
> covered by automated evidence.

> **Status (2026-07-07 local Bichon follow-up):** the crowded-town click-route
> repro is now green under clean interaction diagnostics. The earlier Bichon
> failures separated into entity-hit pollution, self-sprite/nameplate click
> interception, a ready-after-input Zone ACK edge, and a too-tight 1.2s
> post-ACK Gateway input window. Current fix: self UI layers do not intercept
> own ground clicks, shared Zone movement ACKs late-ready input immediately,
> and Gateway post-movement grace is 1.5s (Crystal run grace + one tick).
> Evidence:
> `docs/generated/player-qa/movement-jitter/web-motion-clickroute-bichon-leftclean-postgrace1500-20260707.json`
> with `ok=true`, ACKs `490/164/33/5ms`, no interaction pollution, and Bevy
> WebGL2 packed/no DOM fallback.

> **Status (2026-07-06 local Web/Gateway):** the current local click repro is
> fixed and verified. This was a server scheduling/input-priority race, not a
> PR #123 merge issue: after a movement `UserLocation` ACK, a heavy
> shared-Zone `WorldCommand::Tick` could occupy the same WebSocket task before
> the browser's next chained movement packet arrived, pushing a Run outside
> Crystal grace and producing stop/go one-tile movement. Shared in-process Zone
> now drains `TickPlayerMovement` before heavy ticks and yields heavy world
> ticks during pending movement plus a 1.2s post-ACK input window. The passing
> evidence is
> `docs/generated/player-qa/startgame-debug-20260706-213036/current-web-jitter-r2-gateway-postackgrace1200-click.json`
> with `ok=true`, Run ACK about 205ms, no logical rollback, no residual pending
> plan, and Bevy WebGL2 packed/no-DOM-fallback rendering.

> **Status (2026-06-27):** the *dominant* stutter was found+fixed (**PR #165**). The
> residual "濠电偞鍨堕幐鎾磻閹炬剚娓婚柕鍫濇噷娴犳帞绱掗弬璺ㄦ憼闁靛洤瀚板畷褰掝敊閽樺鐟庨梻浣侯焾椤戝棛鍒掗崼銏㈢? has now been **instrumented and pinned to the RENDER /
> main-thread layer 闂?it is NOT a movement correction** (`window.__corr` all-zero across
> real held-key runs; see **闂?**). The whole correction-source class (闂? candidates 1闂?)
> is **ruled out**. Next focus = render frame-pacing (a separate render-perf effort), not
> the movement pipeline. The `?mir2Debug=1` `window.__corr` counters that proved this are
> landed (gated, zero prod cost).

## 1. Symptom (user-reported, verbatim)

Running with **闂備礁婀遍…鍫ニ囬鍓х?Shift + 闂備礁鎼崐濠氬箠閹捐绠栨繝濠傜墛閻?* (held Shift + arrow keys) feels like
**"濠电娀娼ч崐鐟拔涙担铏圭幓闁搞儯鍔庨埢鏃堟煏閸繃锛嶉柛鏂垮閳藉骞橀姘闂?** 闂?runs a couple steps, then a brief **freeze / stop-and-go**, repeat.
Walking feels fine. The user is on a **120 Hz display**.

When asked to characterise the *remaining* stutter after the PR #165 fixes, the user
chose **"濠电偞鍨堕幐鎾磻閹炬剚娓婚柕鍫濇噷娴犳帞绱掗弬璺ㄦ憼闁靛洤瀚板畷褰掝敊閽樺鐟庨梻浣侯焾椤戝棛鍒掗崼銏㈢?** (stop-and-go freezes) 闂?i.e. the residual is the **standing
stalls**, NOT a continuous jitter.

## 2. What was RULED OUT (do not re-litigate)

The stutter is **not a rendering / frame problem**. Measured, repeatedly:

- **Not dev-build tax** 闂?reproduced identically on a **prod build** (`next build
  --webpack` + `next start`). (`next dev` adds its own ~25 % jsxDEV cost, so all perf is
  judged on prod 闂?see memory `client-perf-judge-on-prod-build`.)
- **Not frame drops** 闂?a frame-time sampler during a held run showed **120 fps, 0
  hitches >20 ms, max frame 14 ms**.
- **Not the 33 Hz React camera alone** 闂?`?bevySelfCamera=1` (Bevy interpolates the
  self-camera at display Hz) did **not** fix the felt stutter.
- **Not scene chunk-thrash** (already fixed, PR #163) and **not the gateway clone-spin**
  (PR #162).

The render/camera fixes attempted before finding the real cause (render-tile forward-ease;
`bevySelfCamera`; 33闂?0 Hz motion clock) each measurably moved a metric but **none removed
the felt stutter**, because the stutter is in the **movement pipeline**, not rendering.

## 3. The real root cause (found by measuring the user's REAL keypresses)

**The held run never sustains a clean run 闂?it collapses into a walk/standing stutter.**

Measured with the user's real keys (8 s held run, **before** the fix):

```
mode sequence: walk 闂?STAND 闂?walk 闂?run 闂?STAND 闂?walk 闂?walk 闂?run 闂?walk 闂?STAND 闂?walk 闂?run
counts: walking 6 闁?running 3 闁?standing 3
distance: 12 tiles in 8 s = 1.5 tiles/s  闂?SLOWER than a plain walk (1.67)!  (clean run 闂?3.3)
```

The 3 `standing` stalls are the felt "闂?. They are **false movement corrections** that
reset the run prime (`runPrimedUntil 闂?0`, so the next step degrades to a walk) **and**
block input (`inputBlockedUntil = now + CRYSTAL_CORRECTION_BLOCK_MS`, a ~400 ms stall).

Two correction sources were identified and fixed in PR #165:

1. **worldSnapshot reconciliation** (`page.tsx reconcileSelfMovementSnapshot` 闂?   `controller reconcileMovementSnapshot`). The server's periodic full-state snapshot
   **always lags** a prediction that legitimately leads the server by up to
   `MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES (2)` tiles during a run, so **every** snapshot was
   flagged a "correction". Walking survives because between 1-tile steps the server catches
   up (lead 0) so the snapshot matches.
   **Fix:** skip the correction when the pending target / prediction is still on the
   predicted path *at-or-ahead* of the snapshot (`crystalMovementCandidateNotBehindServer`).
   The per-move ACK path owns real corrections.

2. **Run degradation** (`controller reconcileMovementAck`). A run predicts +2 tiles, but
   the **server degrades a run to a 1-tile walk when the second tile cannot continue the
   same direction** (Crystal-faithful 闂?`apps/simulation/src/runtime/movement.rs`
   `pathfind_next_step`, the "Running covers two tiles, but only extend 闂?when the route
   continues in the same direction" branch). Running through obstacle-dense town (trees /
   fences / NPCs) this fires constantly: server acks +1, client predicted +2 闂?mismatch 闂?   hard correction 闂?prime reset + input block 闂?standing stall.
   **Fix:** treat a degraded run ack (lands on `from + 1` in the same direction) as a
   **confirm that keeps the run primed**, not a correction.

Also in PR #165: tighten the non-imperative motion clock **33 闂?60 Hz** while the
self-camera is gliding (`original-client-shell.tsx`, `selfCameraGliding`) so the map scroll
keeps up with the display during movement (120 fps, 0 added hitches; 30 Hz when idle).

**Result (measured, real keys, after the fix):** **23 tiles / 8 s** (near clean-run
speed), running steps **3 闂?9**. The run is mostly sustained now.

## 4. Measurement tooling (REUSE THIS 闂?it is the only reliable signal)

- **`window.__mir2SceneMotionDebug`** is the live per-render movement state 闂?gated on
  **`?mir2Debug=1`** (`original-client-shell.tsx` `isSceneMotionDebugMode`). Exposes
  `renderPlayer` (`.x/.y` tile, `.movementUntil`), `playerMotionSnapshot`
  (`.animationState` = walking|running|standing, `.startedAt/.expiresAt`, `.fromX/.toX闂備胶鍋ㄩ崕鍝勨枖?,
  and `playerCameraMotionOffset`.
- **Armed recorder pattern** (drop into the page console / CDP `javascript_tool`): arm a
  `requestAnimationFrame` loop that starts recording when `renderPlayer` tile first changes,
  records 8 s, then stores `window.__runResult` = `{ tilesMoved, modeCounts (walk/run/
  standing transitions), pctCameraStill }`. The exact script is in the session transcript;
  re-derive it from the fields above.
- **闂備礁鐤囧▔鏇熷垔鐎靛摜绠?Synthetic `KeyboardEvent` dispatch is UNRELIABLE for driving a held run** 闂?one 4.5 s
  synthetic hold emitted only **1** `walk` command. The walk/run/standing mix it produced
  is partly its own flakiness. **Measure the USER's real held keys**, not synthetic ones.
  (Real CDP `Input.dispatchKeyEvent` held-down 闂?as in `qa-load-stress.mjs` 闂?would also be
  reliable; Chrome-MCP `computer key` is only a tap.)
- The game viewport must have **focus** (a real click into it) before keys register.
- `pctCameraStill` is inflated by legit standing periods 闂?judge camera smoothness on the
  *running* frames only.

## 5. ROOT CAUSE FOUND (2026-06-27) 闂?residual is RENDER-perf, NOT a correction

The 闂? candidates below were the hypotheses *before* instrumenting. They are **all wrong** 闂?the residual is not in the movement/correction pipeline at all.

### What was instrumented (landed, `?mir2Debug=1`-gated, zero prod cost)

`window.__corr` correction-source counters in `app/page.tsx` (`bumpCorrectionCounter`),
bumped at **every** stall-causing site 闂?not just the two the old brief suggested:

| counter | site | meaning |
|---|---|---|
| `snapshot` / `snapshotSuppressed` | `reconcileSelfMovementSnapshot` | worldSnapshot corrected vs guard caught a lagging echo |
| `ack` / `dashFail` | `reconcileSelfMovementAck` (correction branch) | per-move ACK off-path vs `UserDashFail` |
| `confirm` / `confirmDegraded` | `reconcileSelfMovementAck` (confirm branch) | clean confirm vs run闂備焦鍓氶崑鍛叏娑旂皡m+1 degrade (PR #165 keep-prime path) |
| `legacyInput` | `applyCrystalInputCorrection` | the **second**, legacy direction-step/movement-plan reconciler |
| `pendingTimeout` | `trySendQueuedCrystalMove` | a pending move aged out (>3.0 s) with no ACK |

`samples` keeps the last ~24 mismatch shapes (ack vs predicted from/to, mode, direction).

### The verdict (two real held-key runs, prod build, user's own keyboard)

```
window.__corr  闂? snapshot 0 闁?snapshotSuppressed 0 闁?ack 0 闁?dashFail 0
                  legacyInput 0 闁?pendingTimeout 0 闁?confirmDegraded 0 闁?confirm 11闂?6
__mir2MovementSentCommands  闂? clean ~600 ms cadence, 46 run / 3 walk
recorder  闂? ~24闂?1 tiles / 8 s (clean-run speed) BUT user still feels "濠电偞鍨堕幐鎾磻閹炬剚娓婚柕鍫濇噷娴犳帞绱掗弬璺ㄦ憼闁?
             frame deltas: avg ~17 ms, max 84 ms, ~4 frames >33 ms / 8 s, ~59 fps
```

**ZERO corrections of any kind fired, yet the user still felt the stutter.** Movement sends
are a clean 600 ms cadence; glide duration == send cadence (`RUN/WALK_STEP_INTERVAL_MS =
movementCommandDelayMs = 600`, `LEAD = 0`, no inter-step gap); the sub-tile interpolation is
linear (`original-client-scene-motion.ts` `movementProgressRatio` = `elapsed/duration`). So
the **movement pipeline is clean**. The only anomaly is **periodic frame hitches** (~4 闂?
33闂?4 ms per 8 s 闂?one every ~2 s) at **~59 fps on a 120 Hz display**.

`?bevySelfCamera=1&bevyEntityInterp=1` (the old "secondary camera" lever, re-tested now the
run is sustained) **did NOT fix it and regressed**: entity name labels (DOM overlays) drift
because the imperative path drops the React clock to ~10 Hz while Bevy pans at display Hz.
Crucially it **still measured ~59 fps** with React at 10 Hz 闂?the fps ceiling is **not** the
React re-render; it is Bevy/WebGPU/compositor-bound (or the test context is 60 Hz).

### 闂備礁鐤囧▔鏇熷垔鐎靛摜绠?Test-bed caveat (rule this out FIRST next session)

The local `:3080` prod build was built with `npx next build` directly, which **skips the
`npm run build` asset-gen steps** (`generate:original-asset-manifest` + `assets:map-atlas:build`).
Symptom: `/api/asset-manifest` 500s (missing `original-asset-manifest.generated.json`,
`required` in prod) and the Bevy map-atlas is absent 闂?**black floor**. Fixed this session by
running both generators + a real rebuild. BUT the resulting asset **version digest 闂?the
published R2 release prefix**, so the SW's remote-prewarm targets a non-matching prefix. This
may add main-thread churn/hitches **not present in the deployed client**. Before chasing the
render-perf residual, re-measure on a clean bed: build via `npm run build` (or disable remote
prewarm 闂?all assets are on local disk after the generators run) so the local hitch profile
is trustworthy.

## 5b. CPU-profile attribution (2026-06-28) 闂?it is the asset-streaming pipeline

Built `scripts/qa-cpu-profile.mjs` (CDP V8 Profiler + a REAL held Shift+arrow run over
`Input.dispatchKeyEvent`; **proves the run actually moved via outbound move-packet count** 闂?an
earlier draft silently profiled the SELECT screen because the character name `Walk0704` has a
ZERO, not a letter o, so it fell back to a lease-locked Scout and never entered game). Profiled a
confirmed 14-tile run on the prod `:3080` build.

**Verdict 闂?the held-run main thread is ~47 % busy (vs ~13 % standing), with NO single >50 ms
hitch (`[Violation]` channel empty). The cost is DISTRIBUTED asset-streaming, not one stall:**

| self-time | where | what |
|---|---|---|
| ~8 % | `751.*.js` (`includeEntityPreloadPaths`, IndexedDB `onsuccess`) | the **asset-residency preload** system streaming assets as you move into new cells |
| ~5 % | `png::filter::paeth::unfilter` + `fdeflate::decompress` in `mir2_bevy_runtime_bg.wasm` | **Bevy decodes sprite/tile PNGs on the main thread** (WASM is main-thread) 闂?first-encounter decode as new sprites/tiles enter view |
| ~3.4 % | `onBevyEntityRenderStateChange` + `onBevyMapRenderStateChange` (`page-*.js`) + `serde_json` parse in wasm | the **React闂備焦鍓氶崑鍛櫠閻氱垝y render-state push** (serialise + hand the entity/map state to the runtime each change) |
| ~2.4 % | `writeTexture` (native) | GPU texture uploads for the new sprites/tiles |

So the earlier "~84 ms periodic hitch" was **load/prewarm time**, not the steady run; the steady
run has no single hitch 闂?it's the streaming/decode/push cost spiking past the 8.3 ms (120 Hz)
frame budget as new map regions + entities stream in (running outruns the resident set faster
than walking 闂?exactly why walking feels fine). NOT `setWorld`/packet-handling (every handler is
闂?.4 ms; no React commit frames appear in the profile).

**Levers (each a separate effort, by impact):**
- **Off-thread the Bevy WASM PNG decode** (the ~5 % `paeth`/`fdeflate`) 闂?the dominant decode is
  inside the Bevy runtime, not the JS path. Biggest single lever.
  - **Bevy loads the entity atlas by `imageUrl`** (`runtime/src/lib.rs` `sync_entity_render_atlas_layouts`
    ~1366 闂?`asset_server.load`), and `url_image` is preferred over uploaded RGBA (`atlas_assets.images`)
    at the `if let Some(..) = url_image { .. } else if uploaded` branch (~1370). The web feeds the atlas
    as `imageUrl` with empty `pixels` (entity `atlasPixelBytes:0`), so Bevy decodes the PNG in wasm.
    Both `setMir2EntityRenderAtlas` / `setMir2MapRenderAtlas` take raw RGBA 闂?the map already feeds RGBA
    (no wasm decode); only the entity atlas goes through `imageUrl`.
  - **TRIED & REVERTED (negative result):** decoding the entity-atlas `imageUrl` 闂?RGBA off-thread and
    swapping it in (drop `imageUrl` when pixels ready) **made it WORSE** 闂?the CPU profiler showed
    `createImageBitmap` jump to **~5.1 % main-thread** while `paeth`/`fdeflate` barely moved (2.8闂?.5,
    2.1闂?.8) and total busy rose 47闂?0 %. Cause: the entity atlas is a **per-visible-set packer whose key
    churns every few frames during a run**, so the async decode never lands before Bevy has already
    fetched+decoded the next `imageUrl`; you pay the new `createImageBitmap` AND the old wasm decode.
    The decode-then-swap shape cannot beat a fast-churning atlas.
  - **Correct fix (deeper):** make the packer emit **RGBA directly** (composite in a worker 闂?    `getImageData` 闂?`setMir2EntityRenderAtlas`, never a PNG round-trip) so Bevy never sees an `imageUrl`;
    OR prefetch/pin atlas pages before they enter view; OR flip the Rust precedence (prefer uploaded) so a
    one-shot prebuilt-atlas RGBA upload sticks. All are asset-residency-packer changes, not a swap.
- **Throttle / shrink the React闂備焦鍓氶崑鍛櫠閻氱垝y render-state push** (`onBevy*RenderStateChange` ~3.4 %) 闂?  push deltas, not full state, and coalesce per frame.
- **Defer/spread asset-residency preload during fast movement** (`751.js` ~8 %).
- This change **already off-threads the JS map-atlas page readback** (`lib/map-atlas-decode.ts`)
  闂?a load-time/new-region win (the profiler shows `createImageBitmap` engaging, the JS
  `getImageData` readback gone) but NOT the dominant run-time decode (which is the WASM path above).

Ruled out: `?bevySelfCamera=1` (regresses labels, no fix); React motion clock 60闂?20 Hz
(unviable). The remaining work is the architectural asset-streaming/decode pipeline, not a
one-liner.

### Measurement note (Chrome-MCP hidden-tab trap)

A CDP/automation tab that is not the foreground window has `document.hidden = true` 闂?rAF (and
the React `motionNow` clock) throttle to ~0, freezing `renderPlayer`/the armed recorder. But
`window.__corr` is **packet-driven** (WebSocket `onmessage`) and stays reliable regardless of
visibility. So: read `__corr` any time; for the rAF recorder / fps, the **user** must run with
the game window genuinely foreground (their real keyboard 闂?synthetic keys remain unreliable,
闂?).

## 6. Local repro setup

- **Gateway**: `:7141` kept alive by `/tmp/mir2-gw-watchdog.sh` (launchd-parented) running
  MAIN's `target/release/mir2-gateway` with `MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7141`,
  `MIR2_ACCOUNT_STORE_PATH=/tmp/mir2-verify-accounts.json`, GM creds `gmtest/gmtest123`.
- **Client (prod build)**: from `mir2-web3/apps/web`,
  `MIR2_R2_PROXY_BASE=https://mir2.obelisk.build npx next build --webpack` then
  `闂?npx next start -p 3080`. The `MIR2_R2_PROXY_BASE` same-origin proxy (PR #164, still
  open) kills the local CORS/hotlink storm so the map renders without blue blocks.
  **Must be `--webpack`** (Turbopack panics on the worktree `node_modules` symlink).
- **Enter**: `http://localhost:3080/?mir2Debug=1&assetCache=0&gatewayWs=ws://127.0.0.1:7141/ws`
  闂?login `demo`/`demo` (pre-filled) 闂?OK 闂?select **Scout** (Lv 7) 闂?START. Click into the
  viewport for key focus.
- **A worktree** needs `node_modules` symlinked from the main checkout for `tsc`; remove
  before committing. `cargo` needs `+1.89.0`.

## 7. Verify before landing any follow-up

```
cd mir2-web3/apps/web && npx tsc --noEmit          # 0
npm run test:frontend-logic                        # 18/0 (movement controller is pure-fn testable)
```

Conventions: 1:1 with Crystal (cite `Crystal/...cs` / `simulation/...rs file:line`); new
fields optional + backward-compatible; **no model identifiers** in commits/PRs/code;
commits/paths English. CI is billing-blocked (red 闂?bug) 闂?`gh pr merge --admin --squash`.
