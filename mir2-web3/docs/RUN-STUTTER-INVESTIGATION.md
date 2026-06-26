# Run-stutter ("奔跑卡") investigation — continuation brief

> **Status (2026-06-26):** root cause of the *dominant* stutter found and fixed
> (**PR #165**, on `main`). One residual symptom remains — see **§5 Open root cause**.
> This doc is the hand-off for a fresh session to finish it.

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

## 5. OPEN root cause — what the new session must find

After PR #165, the user's real run still shows **~3 residual `standing` stalls** ("一顿一顿
地停住"). Distance jumped to 23 tiles (near clean) so it is much better, but the brief
freezes remain. **The remaining correction source is not yet pinned.** Candidates:

1. **Snapshot-guard edge** — the guard only suppresses when the prediction is ≤2 tiles
   ahead (`MOVEMENT_LOCAL_ACTION_MAX_LEAD_TILES`). A snapshot that lags by >2 tiles, or a
   prediction with a stale `direction`, still corrects.
2. **ACK off-path / over-degraded** — the degradation guard only matches `from + 1` exactly.
   If the server lands somewhere else on the path (or the `from` is stale), it still hard-
   corrects.
3. **`UserDashFail`** — when the server refuses the run entirely (first tile blocked) it
   sends `UserDashFail` → `hardFailure` → correction. Running into a wall is *legitimate*,
   but verify the user's "open" runs are not hitting spurious `UserDashFail`.
4. **Held-direction re-arm gaps** — the queued-direction intent may lapse between steps
   (separate from the prime), inserting a standing frame.

### Recommended first move (instrument, don't guess)

Add cheap correction-source counters and read them after a real run:

- `reconcileSelfMovementSnapshot`: bump `window.__corr.snapshot` when it *actually* corrects
  (passes the new guard), and `…snapshotSuppressed` when the guard fires.
- `reconcileSelfMovementAck`: bump `window.__corr.ack` (correction, `!UserDashFail`) vs
  `…dashFail` (`packetName === "UserDashFail"`).

Rebuild, have the **user** run 8 s, read `window.__corr`. Whichever dominates is the
remaining source → fix that one. (Gate the counters on `?mir2Debug=1` or strip before
landing.)

### Deeper option (eliminate the degradation snap-back, the 1:1 way)

Crystal's client *predicts* the run degradation locally (it knows map collision, so it
predicts +1 when the 2nd tile is blocked and never mismatches). Our client predicts +2
unconditionally (`movementPointInDirection(from, dir, 2)` in the controller) — so even with
the keep-prime fix there is a 1-tile correction each degraded step. Predicting the
degradation client-side (needs a walkability check the controller currently lacks) would
remove the residual entirely. Bigger change; measure whether the standing stalls are worth
it first.

### Camera (secondary)

`pctCameraStill` was still ~74 % on the user's post-fix run (partly the standings). If, with
the stalls gone, the user still perceives a choppy *scroll*, the lever is display-Hz camera:
either push the React clock to ~120 Hz during movement (verify the ~14 ms re-render frames
don't drop on an 8.3 ms budget) or make `?bevySelfCamera=1` the default now that the run is
sustained (re-test it — it was rejected while the run was still broken).

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
