# Player-QA playthrough loop

`apps/web/scripts/qa-playthrough.mjs` drives the **real web client over Chrome
DevTools Protocol** through a full "real player" journey and records every
problem it sees into a structured report. It is the browser-level complement to
the protocol-level bots (`load-gateway-ws.mjs`, `smoke-two-client-zone.mjs`):
those are blind to rendering, this one sees the actual Bevy canvas.

## The journey (beats)

1. open client → login screen
2. register a fresh account
3. log in → character select
4. create a character → start game
5. enter world → **map renders** (scene-ready + canvas not black + no stuck "Loading map…")
6. move → **render keeps up** (server moved us but canvas didn't = render bug)
7. **camera update-rate probe** → quantify the scroll content rate during a sustained walk (judder = low `cameraUpdateHz` vs `rafHz`)
8. find an NPC → walk to it → open dialog (+ assert `.npc-dialog-panel` rendered)
9. accept a quest through the dialog (`questLog` grows)
10. **cross-map** → transfer to a SECOND map and assert it renders (interaction-ready + canvas not black; flags stuck "Loading map…" / black)
11. **combat** → find a `kind:"monster"` near the player (hopping to a hunting field — Woomyon "1" / Serpent Valley "2" — if the current map is empty), walk melee-adjacent, and attack the way the client does (click its tile → `activateEntity` → `attackTarget` → `send({type:"attack"})`). Verify its HP drops and/or it dies, and that damage indicators (`.scene-damage-floater`) appear; flags "attacking produced no damage/death/indicators"
12. **inventory** → click `.hud-button.inventory button` and assert the inventory window (`.inventory-window`) rendered
13. travel several legs → render stays stable
14. wrap → dump diagnostics

Each beat is best-effort: a failure is recorded and the loop keeps going.

### Camera A/B mode (`--cameraAb true`)

A separate flow (instead of the journey above) that A/Bs the Bevy self-camera fix
(PR #125): it runs the camera update-rate probe **with and without
`?bevySelfCamera=1`** across 2–3 maps (Bichon "0", Woomyon "1", Serpent Valley
"2") and reports `cameraUpdateHz` for each into a comparison table in `report.md`
(and `camera-ab.json`) — the evidence for a default-on decision. Each variant
reloads the page (the flag is read once at load), re-logs in, and re-enters.

## What it records

Output lands in `apps/web/docs/generated/player-qa/playthrough-<runId>/`:

| File | Contents |
|---|---|
| `report.md` | human-readable: issue table by severity, per-beat journey, evidence |
| `report.json` | machine-readable issues + beats |
| `summary.json` | counts by severity/category |
| `frames/NN-<beat>.png` | screenshot after each beat (the visual timeline) |
| `console.json` | console errors/warnings |
| `network-failures.json` | every ≥400 / `net::ERR_*` request (sprite/atlas 404s) |
| `ws-timeline.json` | last WS frames sent/received (server truth) |
| `camera-ab.json` | (`--cameraAb` only) per-map `cameraUpdateHz` with/without `?bevySelfCamera=1` |

### Issue categories

- **render** — black/blank canvas (luma), stuck "Loading map…", scene never ready, dialog open in state but not in DOM, render frozen during movement, cross-map transfer that never lands/renders
- **movement** — server moved but client didn't (desync), teleport/jump, or low camera scroll rate vs frame rate (judder). Note: the time between *logical tile changes* is the walk/run cadence (movement speed), NOT jank, so it is recorded (`tileCadenceMs`) but never flagged. Fine-grained movement-feel analysis (prediction staleness, command-queue latency, camera continuity) is `capture-web-movement-jitter.mjs`.
- **combat** — attacking a monster produced no damage/death/damage-indicators, or no monster could be found/spawned to fight
- **quest** — NPC click opened no dialog, no quest added after clicking dialog links, no NPC on map
- **ui** — a HUD window (e.g. inventory) did not open/render when its button was clicked
- **network** — failed sprite/atlas/UI requests, grouped by kind (a wall of identical 404s = one issue). _If files exist in git, the R2 release is likely stale — see `ASSET-RELEASE-RUNBOOK.md`._
- **console** — critical console errors/exceptions (deduped)
- **flow** — a beat threw (blocks progression)

## Run it

Prereqs: gateway on `:7110` + simulation running, and a web client served
somewhere (a running `next dev`, e.g. `:3001`).

```bash
cd apps/web

# watch it play (headed; real GPU — best for render fidelity)
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:3001

# headless (CI-style)
npm run qa:playthrough -- --baseUrl http://127.0.0.1:3001

# camera A/B (compare ?bevySelfCamera=1 on/off across maps) instead of the journey
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:3001 --cameraAb true

# against this worktree's own dev server
npm run dev            # in another terminal; note the port it prints
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:<port>
```

Useful flags: `--account NAME --password PW`, `--createAccount false` (reuse an
existing account), `--runId my-run`, `--startMap 0 --startX 330 --startY 330`
(force a spawn point), `--moveWindowMs`, `--combatWindowMs`, `--sceneReadyTimeoutMs`,
`--cameraAb true` (run the camera A/B comparison instead of the normal journey).

> **One run at a time:** a playthrough drives the live stack (gateway + simulation
> + client) exclusively — don't start a second loop against the same stack while
> one is running.

## Gotchas

- **Headed uses real GPU**; headless falls back to SwiftShader which can produce
  a falsely "black" canvas — prefer `--headed` when judging render health.
- Background-throttling is disabled via Chrome flags, so rAF keeps running even
  when the window is not focused.
- The harness launches its **own** fresh Chrome (separate profile + debug port);
  it does not touch your normal browser.
- It registers a **new account each run** (unique id) so runs are reproducible
  from scratch; pass `--account`/`--createAccount false` to reuse one.

## Fix loop

The report is the input to the fix phase: triage by severity, locate with
codegraph, fix, then **re-run the same loop** (`--runId`) to confirm the issue is
gone (regression).

---

# Soak loop (`qa-soak.mjs`)

`apps/web/scripts/qa-soak.mjs` is the **long-running stability** complement to the
journey loops above. The playthrough/combat/social loops each run a short scripted
journey and exit; the soak answers the one thing they can't — does a **sustained
"play all day" session stay healthy, or slowly leak / degrade / desync?** It keeps
the real client **continuously busy** and samples a **time-series** of memory / GPU /
DOM / FPS / WebSocket / error health every ~10–15s for the whole run (minutes →
hours), then fits trends and emits a **PASS / LEAK / DEGRADED** verdict.

## Activity driver (active soak, not idle)

An idle client never allocates, so a leak that only grows under play would never
show. On a fast cycle (`--activityIntervalMs`, default 2.5s) the harness alternates
real player activity, keeping the client busy the whole run:

1. **click-to-move** — click an in-viewport tile a few steps away (production move path)
2. **held-keyboard** — sustain a real held **arrow key** via CDP `Input.dispatchKeyEvent`
   (also the only loop that exercises the keyboard SEND pipeline the click loops miss)
3. **combat** — find a `kind:"monster"`, walk adjacent, swing
4. **cross-map travel** — `transferMap` across a ring of maps (Bichon "0" / Woomyon "1" / Serpent Valley "2")
5. **inventory churn** — open then close the inventory HUD (a window mount/unmount → DOM-leak probe)

## What it samples (the time-series)

| Group | Source | Fields |
|---|---|---|
| JS heap | `performance.memory` (**GC-forced** each sample → retained heap) | `usedJSHeapSize`, `totalJSHeapSize` |
| Runtime / GPU | `window.__mir2BevyEntityRendererDebug` + `sceneAssetRuntime` | `atlasPixelBytes`, `atlasCount`, `alphaKeyedBlobBytes` |
| Cache | `window.__mir2CacheMetrics.snapshot().summary` | `transferBytes`, `cacheStorageEntryCount`, `domImageCount` |
| DOM | `document` | `querySelectorAll("*").length`, `images.length` |
| FPS / cadence | in-page rAF recorder (the `rafGaps` approach from `qa-load-stress.mjs`) | median fps, p95 frame-time, worst no-frame gap |
| WS health | CDP `Network.*` | reconnect count, frames-received rate, `wsState` |
| Errors | CDP `Runtime.consoleAPICalled` / `exceptionThrown` + `Network` ≥400 / `net::ERR_*` | cumulative + per-window delta |

> Heap is read **after a forced `HeapProfiler.collectGarbage`**, so the series is
> *retained* memory — a monotonic climb is a real leak, not uncollected garbage.

## Detectors / verdict

- **LEAK** — retained JS heap **or** bevy `atlasPixelBytes` **or** DOM node count trends
  monotonically up (least-squares slope over the **steady-state** windows, past warm-up)
  beyond a threshold GC doesn't reclaim.
- **FPS DEGRADED** — last-window median fps materially below the first window.
- **ERROR ACCUM** — console/network error rate growing across the run.
- **RECONNECT STORM** — repeated gateway reconnects (or sustained non-`open` `wsState`).
- **FREEZE** — a long no-frame gap (rAF starved) / zero-frame window / renderer crash.

Verdict = **LEAK** (any leak detector) → else **DEGRADED** (any other) → else **PASS**.
Exit code is `1` on LEAK / FREEZE / RECONNECT-STORM (hard failures), else `0`.

## What it writes

Output lands in `apps/web/docs/generated/soak-qa/run-<runId>/`:

| File | Contents |
|---|---|
| `timeseries.json` | the full per-window sample series (written incrementally — survives a mid-run crash) |
| `report.md` | verdict, detector table, trend table with **sparklines**, per-window numbers, issues |
| `report.json` / `summary.json` | machine-readable verdict + detectors + trends |
| `console.json` / `network-failures.json` | accumulated errors |
| `frames/00-start.png`, `frames/99-end.png` | first/last screenshots |

## Run it

> **Prefer an ISOLATED gateway.** The shared `:7110` node-proxy sim **depletes** over
> long runs (project memory), so a soak pointed at it measures shared-sim exhaustion,
> not the client. Spin up a private gateway on fresh ports with a temp account store
> and point the client at it via the localhost-only `?gatewayWs=` override:

```bash
cd apps/web

# 1) isolated gateway + sim (fresh ports + temp account store)
MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7311 MIR2_ACCOUNT_STORE=$(mktemp -d)/acct.json \
  cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway

# 2) reuse a running `next dev` (note its port) and point the soak at the isolated gateway
npm run qa:soak -- --headed --durationMin 120 \
  --baseUrl http://127.0.0.1:3001 --gatewayWs ws://127.0.0.1:7311/ws

# quick smoke (5 min) — proves the time-series + verdict + report pipeline
npm run qa:soak -- --headed --durationMin 5 --sampleMs 8000 \
  --baseUrl http://127.0.0.1:3001 --gatewayWs ws://127.0.0.1:7111/ws
```

If you run against the **shared** `:7110` stack the report flags it (`gateway.shared`)
so a leak/degrade there isn't mistaken for a client bug.

Useful flags: `--durationMin` (default 20; supports multi-hour), `--headed`
(default true — real GPU for true render memory), `--baseUrl`, `--account`/`--password`,
`--sampleMs` (default 12000), `--activityIntervalMs` (default 2500), and detector
thresholds (`--leakHeapSlope`, `--leakDomSlope`, `--fpsDegradeRatio`, `--reconnectMax`,
`--freezeGapMs`, `--warmupFraction`). `Ctrl-C` flushes a partial report.
