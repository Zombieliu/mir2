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
10. travel several legs → render stays stable
11. wrap → dump diagnostics

Each beat is best-effort: a failure is recorded and the loop keeps going.

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

### Issue categories

- **render** — black/blank canvas (luma), stuck "Loading map…", scene never ready, dialog open in state but not in DOM, render frozen during movement
- **movement** — server moved but client didn't (desync), teleport/jump, or low camera scroll rate vs frame rate (judder). Note: the time between *logical tile changes* is the walk/run cadence (movement speed), NOT jank, so it is recorded (`tileCadenceMs`) but never flagged. Fine-grained movement-feel analysis (prediction staleness, command-queue latency, camera continuity) is `capture-web-movement-jitter.mjs`.
- **quest** — NPC click opened no dialog, no quest added after clicking dialog links, no NPC on map
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

# against this worktree's own dev server
npm run dev            # in another terminal; note the port it prints
npm run qa:playthrough -- --headed --baseUrl http://127.0.0.1:<port>
```

Useful flags: `--account NAME --password PW`, `--createAccount false` (reuse an
existing account), `--runId my-run`, `--startMap 0 --startX 330 --startY 330`
(force a spawn point), `--moveWindowMs`, `--sceneReadyTimeoutMs`.

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
