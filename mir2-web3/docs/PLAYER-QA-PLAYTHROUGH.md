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
