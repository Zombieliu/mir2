# Run-stutter ("奔跑卡") investigation — continuation brief

> **Status (2026-06-27):** the *dominant* stutter was found+fixed (**PR #165**). The
> residual "一顿一顿地停住" has now been **instrumented and pinned to the RENDER /
> main-thread layer — it is NOT a movement correction** (`window.__corr` all-zero across
> real held-key runs; see **§5**). The whole correction-source class (§5 candidates 1–4)
> is **ruled out**. Next focus = render frame-pacing (a separate render-perf effort), not
> the movement pipeline. The `?mir2Debug=1` `window.__corr` counters that proved this are
> landed (gated, zero prod cost).

## 1. Symptom (user-reported, verbatim)

Running with **按住 Shift + 方向键** (held Shift + arrow keys) feels like
**"奔跑两步一卡"** — runs a couple steps, then a brief **freeze / stop-and-go**, repeat.
Walking feels fine. The user is on a **120 Hz display**.

When asked to characterise the *remaining* stutter after the PR #165 fixes, the user
chose **"一顿一顿地停住"** (stop-and-go freezes) — i.e. the residual is the **standing
stalls**, NOT a continuous jitter.

## 2. What was RULED OUT (do not re-litigate)

The stutter is **not a rendering / frame problem**. Measured, repeatedly:

- **Not dev-build tax** — reproduced identically on a **prod build** (`next build
  --webpack` + `next start`). (`next dev` adds its own ~25 % jsxDEV cost, so all perf is
  judged on prod — see memory `client-perf-judge-on-prod-build`.)
- **Not frame drops** — a frame-time sampler during a held run showed **120 fps, 0
  hitches >20 ms, max frame 14 ms**.
- **Not the 33 Hz React camera alone** — `?bevySelfCamera=1` (Bevy interpolates the
  self-camera at display Hz) did **not** fix the felt stutter.
- **Not scene chunk-thrash** (already fixed, PR #163) and **not the gateway clone-spin**
  (PR #162).

The render/camera fixes attempted before finding the real cause (render-tile forward-ease;
`bevySelfCamera`; 33→60 Hz motion clock) each measurably moved a metric but **none removed
the felt stutter**, because the stutter is in the **movement pipeline**, not rendering.

## 3. The real root cause (found by measuring the user's REAL keypresses)

**The held run never sustains a clean run — it collapses into a walk/standing stutter.**

Measured with the user's real keys (8 s held run, **before** the fix):

```
mode sequence: walk → STAND → walk → run → STAND → walk → walk → run → walk → STAND → walk → run
counts: walking 6 · running 3 · standing 3
distance: 12 tiles in 8 s = 1.5 tiles/s  ← SLOWER than a plain walk (1.67)!  (clean run ≈ 3.3)
```

The 3 `standing` stalls are the felt "卡". They are **false movement corrections** that
reset the run prime (`runPrimedUntil → 0`, so the next step degrades to a walk) **and**
block input (`inputBlockedUntil = now + CRYSTAL_CORRECTION_BLOCK_MS`, a ~400 ms stall).

Two correction sources were identified and fixed in PR #165:

1. **worldSnapshot reconciliation** (`page.tsx reconcileSelfMovementSnapshot` →
   `controller reconcileMovementSnapshot`). The server's periodic full-state snapshot
   **always lags** a prediction that legitimately leads the server by up to
   `MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES (2)` tiles during a run, so **every** snapshot was
   flagged a "correction". Walking survives because between 1-tile steps the server catches
   up (lead 0) so the snapshot matches.
   **Fix:** skip the correction when the pending target / prediction is still on the
   predicted path *at-or-ahead* of the snapshot (`crystalMovementCandidateNotBehindServer`).
   The per-move ACK path owns real corrections.

2. **Run degradation** (`controller reconcileMovementAck`). A run predicts +2 tiles, but
   the **server degrades a run to a 1-tile walk when the second tile cannot continue the
   same direction** (Crystal-faithful — `apps/simulation/src/runtime/movement.rs`
   `pathfind_next_step`, the "Running covers two tiles, but only extend … when the route
   continues in the same direction" branch). Running through obstacle-dense town (trees /
   fences / NPCs) this fires constantly: server acks +1, client predicted +2 → mismatch →
   hard correction → prime reset + input block → standing stall.
   **Fix:** treat a degraded run ack (lands on `from + 1` in the same direction) as a
   **confirm that keeps the run primed**, not a correction.

Also in PR #165: tighten the non-imperative motion clock **33 → 60 Hz** while the
self-camera is gliding (`original-client-shell.tsx`, `selfCameraGliding`) so the map scroll
keeps up with the display during movement (120 fps, 0 added hitches; 30 Hz when idle).

**Result (measured, real keys, after the fix):** **23 tiles / 8 s** (near clean-run
speed), running steps **3 → 9**. The run is mostly sustained now.

## 4. Measurement tooling (REUSE THIS — it is the only reliable signal)

- **`window.__mir2SceneMotionDebug`** is the live per-render movement state — gated on
  **`?mir2Debug=1`** (`original-client-shell.tsx` `isSceneMotionDebugMode`). Exposes
  `renderPlayer` (`.x/.y` tile, `.movementUntil`), `playerMotionSnapshot`
  (`.animationState` = walking|running|standing, `.startedAt/.expiresAt`, `.fromX/.toX…`),
  and `playerCameraMotionOffset`.
- **Armed recorder pattern** (drop into the page console / CDP `javascript_tool`): arm a
  `requestAnimationFrame` loop that starts recording when `renderPlayer` tile first changes,
  records 8 s, then stores `window.__runResult` = `{ tilesMoved, modeCounts (walk/run/
  standing transitions), pctCameraStill }`. The exact script is in the session transcript;
  re-derive it from the fields above.
- **⚠️ Synthetic `KeyboardEvent` dispatch is UNRELIABLE for driving a held run** — one 4.5 s
  synthetic hold emitted only **1** `walk` command. The walk/run/standing mix it produced
  is partly its own flakiness. **Measure the USER's real held keys**, not synthetic ones.
  (Real CDP `Input.dispatchKeyEvent` held-down — as in `qa-load-stress.mjs` — would also be
  reliable; Chrome-MCP `computer key` is only a tap.)
- The game viewport must have **focus** (a real click into it) before keys register.
- `pctCameraStill` is inflated by legit standing periods — judge camera smoothness on the
  *running* frames only.

## 5. ROOT CAUSE FOUND (2026-06-27) — residual is RENDER-perf, NOT a correction

The §5 candidates below were the hypotheses *before* instrumenting. They are **all wrong** —
the residual is not in the movement/correction pipeline at all.

### What was instrumented (landed, `?mir2Debug=1`-gated, zero prod cost)

`window.__corr` correction-source counters in `app/page.tsx` (`bumpCorrectionCounter`),
bumped at **every** stall-causing site — not just the two the old brief suggested:

| counter | site | meaning |
|---|---|---|
| `snapshot` / `snapshotSuppressed` | `reconcileSelfMovementSnapshot` | worldSnapshot corrected vs guard caught a lagging echo |
| `ack` / `dashFail` | `reconcileSelfMovementAck` (correction branch) | per-move ACK off-path vs `UserDashFail` |
| `confirm` / `confirmDegraded` | `reconcileSelfMovementAck` (confirm branch) | clean confirm vs run→from+1 degrade (PR #165 keep-prime path) |
| `legacyInput` | `applyCrystalInputCorrection` | the **second**, legacy direction-step/movement-plan reconciler |
| `pendingTimeout` | `trySendQueuedCrystalMove` | a pending move aged out (>1.5 s) with no ACK |

`samples` keeps the last ~24 mismatch shapes (ack vs predicted from/to, mode, direction).

### The verdict (two real held-key runs, prod build, user's own keyboard)

```
window.__corr  →  snapshot 0 · snapshotSuppressed 0 · ack 0 · dashFail 0
                  legacyInput 0 · pendingTimeout 0 · confirmDegraded 0 · confirm 11–16
__mir2MovementSentCommands  →  clean ~600 ms cadence, 46 run / 3 walk
recorder  →  ~24–31 tiles / 8 s (clean-run speed) BUT user still feels "一顿一顿"
             frame deltas: avg ~17 ms, max 84 ms, ~4 frames >33 ms / 8 s, ~59 fps
```

**ZERO corrections of any kind fired, yet the user still felt the stutter.** Movement sends
are a clean 600 ms cadence; glide duration == send cadence (`RUN/WALK_STEP_INTERVAL_MS =
movementCommandDelayMs = 600`, `LEAD = 0`, no inter-step gap); the sub-tile interpolation is
linear (`original-client-scene-motion.ts` `movementProgressRatio` = `elapsed/duration`). So
the **movement pipeline is clean**. The only anomaly is **periodic frame hitches** (~4 ×
33–84 ms per 8 s ≈ one every ~2 s) at **~59 fps on a 120 Hz display**.

`?bevySelfCamera=1&bevyEntityInterp=1` (the old "secondary camera" lever, re-tested now the
run is sustained) **did NOT fix it and regressed**: entity name labels (DOM overlays) drift
because the imperative path drops the React clock to ~10 Hz while Bevy pans at display Hz.
Crucially it **still measured ~59 fps** with React at 10 Hz → the fps ceiling is **not** the
React re-render; it is Bevy/WebGPU/compositor-bound (or the test context is 60 Hz).

### ⚠️ Test-bed caveat (rule this out FIRST next session)

The local `:3080` prod build was built with `npx next build` directly, which **skips the
`npm run build` asset-gen steps** (`generate:original-asset-manifest` + `assets:map-atlas:build`).
Symptom: `/api/asset-manifest` 500s (missing `original-asset-manifest.generated.json`,
`required` in prod) and the Bevy map-atlas is absent → **black floor**. Fixed this session by
running both generators + a real rebuild. BUT the resulting asset **version digest ≠ the
published R2 release prefix**, so the SW's remote-prewarm targets a non-matching prefix. This
may add main-thread churn/hitches **not present in the deployed client**. Before chasing the
render-perf residual, re-measure on a clean bed: build via `npm run build` (or disable remote
prewarm — all assets are on local disk after the generators run) so the local hitch profile
is trustworthy.

## 5b. Next focus — render frame-pacing (separate effort)

Not a movement fix. Profile a real held run on a clean prod bed (memory
`client-perf-judge-on-prod-build`, `scripts/qa-cpu-profile.mjs`) and find the source of the
~84 ms periodic hitch:

- **Periodic heavy `setWorld` re-render** of the ~12.7k-line `page.tsx` monolith (memory
  `client-render-perf`: "setWorld-per-packet still open"). A periodic full merge → monolith
  re-render → dropped frame is the leading suspect.
- **Bevy/WebGPU per-frame cost** (entity/map atlas upload or decode spikes) — `__corr` proved
  it is not React-clock-bound (10 Hz React still 59 fps).
- **GC / asset-decode** spikes during play.

Levers already ruled out: `?bevySelfCamera=1` (regresses labels, no fix); pushing the React
motion clock 60→120 Hz is unviable (per-render already ~17 ms > the 8.3 ms 120 Hz budget).
The fix likely means making the per-frame render cheaper (decouple movement render from the
monolith), i.e. the known architectural render-perf work — not a one-line change.

### Measurement note (Chrome-MCP hidden-tab trap)

A CDP/automation tab that is not the foreground window has `document.hidden = true` → rAF (and
the React `motionNow` clock) throttle to ~0, freezing `renderPlayer`/the armed recorder. But
`window.__corr` is **packet-driven** (WebSocket `onmessage`) and stays reliable regardless of
visibility. So: read `__corr` any time; for the rAF recorder / fps, the **user** must run with
the game window genuinely foreground (their real keyboard — synthetic keys remain unreliable,
§4).

## 6. Local repro setup

- **Gateway**: `:7141` kept alive by `/tmp/mir2-gw-watchdog.sh` (launchd-parented) running
  MAIN's `target/release/mir2-gateway` with `MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7141`,
  `MIR2_ACCOUNT_STORE_PATH=/tmp/mir2-verify-accounts.json`, GM creds `gmtest/gmtest123`.
- **Client (prod build)**: from `mir2-web3/apps/web`,
  `MIR2_R2_PROXY_BASE=https://mir2.obelisk.build npx next build --webpack` then
  `… npx next start -p 3080`. The `MIR2_R2_PROXY_BASE` same-origin proxy (PR #164, still
  open) kills the local CORS/hotlink storm so the map renders without blue blocks.
  **Must be `--webpack`** (Turbopack panics on the worktree `node_modules` symlink).
- **Enter**: `http://localhost:3080/?mir2Debug=1&assetCache=0&gatewayWs=ws://127.0.0.1:7141/ws`
  → login `demo`/`demo` (pre-filled) → OK → select **Scout** (Lv 7) → START. Click into the
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
commits/paths English. CI is billing-blocked (red ≠ bug) → `gh pr merge --admin --squash`.
