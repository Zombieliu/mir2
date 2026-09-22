# Sustained-run cadence and confirmed-step handoff

The user still reports a pause at each step with client `67d18345e` and the
older Gateway `3b8bd01f9`. Earlier client-only smooth/retained-center fixes did
not establish smoothness acceptance. This repair covers server timing and the
client handoff together.

## Measured failure

Live native input trace:
`C:/mir2-ui-repair-20260921/render-live/20260922-211618-871-movement.jsonl`.
Its last seven Run receipts take 646.38, 675.68, 651.35, 659.70, 675.32,
665.67 and 652.44 ms. The client correctly keeps one command in flight, so
the next step waits after its 600 ms visual travel finishes. There is no
collision correction in this selection.

The old owner scheduled each 300 ms maintenance wake from the previous tick's
finish. Tick work accumulates into the phase; a command at/before its movement
ready boundary waits for maintenance. Two drifting ticks eventually exceed
the client's 600 ms step, keeping each later command behind its ACK.

The render trace separately exposes a confirmed previous step followed by a
new command while the previous applied map/entity center remains. At
1790083488378 the ACK is (290,362), successor target (288,360), and retained
center (292,364). Rejecting that exact predecessor center drops the camera
offset to zero for a frame, visibly reversing the world position. This is
distinct from server cooldown waiting.

## Changes

- The single Zone owner wakes at the earliest pending movement deadline.
  This wake runs only movement and normal outbound dispatch. AI, regeneration,
  drops and maintenance counters keep the 300 ms maintenance cadence.
- Maintenance stays on an absolute phase; late ticks skip missed periods
  without replaying a burst. Existing movement cooldown, collision, run
  eligibility, teardown fences and mutation authorization remain enforced.
- Native processing explicitly orders clock, packet/input producers, then
  local/remote motion consumers in the same PreUpdate schedule.
- A confirmed predecessor permits its exact old center only until the new
  center commits. Correction/reset/unconfirmed paths cannot grant that handoff.
- Scene reset clears old motion and camera state. A same-frame new-map
  command retains only its own command origin and ACK entry, stripping old
  fractional position, handoff and confirmation. Session reset clears all.

## Public protocol A/B evidence

Both runs use independent fresh ordinary accounts, public registration/login,
normal Run commands and confirmed LogOutSuccess. No grants, debug transfer,
save editing, death/revival allowance or newcomer clock changes occur.
These are movement probes, separate from V1/V2 journey evidence.

| Measure | Old Gateway | Repaired Gateway |
| --- | ---: | ---: |
| Consecutive commands | 64 | 64 |
| First 16 ACK mean | 0.695 ms | 0.616 ms |
| Last 16 ACK mean | 653.234 ms | 0.699 ms |
| Last 16 ACK maximum | 654.228 ms | 1.366 ms |
| All send intervals mean | 617.113 ms | 600.070 ms |
| Corrections | 0 | 0 |
| Normal logout acknowledged | yes | yes |

Reports: [before](movement-cadence-before.json),
[after](movement-cadence-after.json). The old binary SHA256 is
`ADC7B40FB6E3734D6C57E3C069625F72EC464E4C8C71DFB316CBBDB2406CD651`;
the repaired Gateway SHA256 is
`457C9EDAA5305D23B15C6F2CC15BE829F702711F99AC7E2FF619A7C93307B773`.
Ports 19410/19510 and stores under `C:/mir2-ui-repair-20260921/cadence-*-server`
were isolated from the live 19910 store. Both probe gateways stopped after
their clients logged out.

The first [coarse-timer control](movement-cadence-coarse-control.json) did
**not** reproduce: Windows timers spaced sends around 606 ms, always after
the ready boundary. The repeatable probe uses a 600 ms monotonic deadline
with a bounded <=20 ms spin tail only in the isolated Node timing probe.
Actual intervals are recorded; production client/server code never spins.

Command (repository root `mir2-web3`):
`node apps/web/scripts/quest-agent/probe-movement-cadence.mjs --endpoint ws://127.0.0.1:PORT/ws --output PATH --count 64`.
Credentials remain in the local private directory and are not committed.

## Validation and remaining gate

Runtime full regression passes 271/271, including late-ACK handoff,
correction rejection, producer order and overlapping-coordinate scene reset.
Simulation movement-deadline tests pass 3/3; Gateway deadline tests pass 3/3
and existing shared-zone regressions pass 55/55. Windows full regression
passes 696/696 again after the scene-reset additions. Both final native-client
and Gateway release builds pass. These builds do not establish visual acceptance.

Logs are under `C:/mir2-ui-repair-20260921/`: `owner-movement-deadline-*.log`,
`movement-chain-runtime-final.log`, `movement-chain-windows-final.log`,
`movement-chain-client-build.log` and `movement-chain-gateway-build.log`.

The protocol A/B proves removal of the reproduced server cadence drift. It
does not measure GPU presentation, native frame pacing, or human feel.
The paired new client/Gateway package still needs native continuous-running
and map-change verification. `visualAccepted=false`, whole UI goal incomplete.
