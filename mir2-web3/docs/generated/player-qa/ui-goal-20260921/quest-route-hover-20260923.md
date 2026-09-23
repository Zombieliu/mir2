# Route stopped near Bichon obstacles — 2026-09-23

The user reported the ongoing Oma entrance route stopped at Bichon `(205,62)`,
with destination `(147,33)`. Client `47efc7896` / PID44352 was still running;
no desktop input, account edits or service restart were used to investigate.

## Observed failure

The movement trace confirms the final ordinary Run to `(205,62)` at
`2385821.7315 ms`. At `2386274.4039 ms`, the client stops the route with
`worldInputBlocked`; the next mouse press is approximately 1.6 seconds later.
There is no collision correction at this stop. The old trace does not identify
which UI predicate was true, so the exact hovered control is not established.
The bounded event excerpt is saved locally as
`C:/mir2-ui-repair-20260921/quest-route-ui-block-repro.json`.

Code inspection and a failing native controller fixture reproduce a definite
defect in that branch: passive status/buff/skill/hero hover is conflated with
modal input and unconditionally discards the entire map route. A separate
fixture using the real Bichon collision map already reaches the entrance in
58 acknowledged Walk/Run steps while avoiding both static terrain and an
entity newly placed on the originally planned next tile. The route planner's
search budget and ordinary collision/acknowledgement rules were not changed.

## Repair

An established map route, with no held or freshly pressed mouse button,
uses a dedicated UI guard that excludes passive hover. The guard shares the
original full modal checks. Only the precise ordinary Inventory and BigMap
panel variants are exempted; Storage, NpcShop and Mail panels that also show
an inventory remain blocking. Chat focus, actual prompts, active drags/item
operations, NPC dialogs, death and consumed input still stop movement. Real
clicks continue through the existing click shield and manual cancellation;
Escape, movement keys, session/map changes and focus loss retain their rules.
The common guard does not clone the UI state every frame.

When a UI guard actually stops a map route, the task card now displays a stop
message and a bounded `autoPathUiBlocked` event records the blocker category
and relevant UI/press flags. This makes a future UI stop distinguishable from
route planning or server correction in the diagnostic logs.

## Verification

- New controller baseline: 2/3 pass, with passive status hover clearing the
  route. After repair: 3/3 pass.
- Six passive hover surfaces continue ordinary movement without sending a
  second command before the authoritative ACK: status, buffs, skill state,
  skill geometry, hero geometry and the ordinary inventory.
- Sixteen cancellation contexts retain protection, including service panels,
  modal/drag/consumed states, death, NPC, focus, Escape, a middle press, and
  a same-frame press/release.
- Bichon `(205,62) -> (147,33)` reaches the actual imported entrance after 58
  acknowledged steps, checking every traversed tile, including Run intermediate
  cells, against actual static collision and the newly occupied tile.
- Full Windows `input::` regression: 100/100 pass.
- Shared-client native UI `world_` guards: 12/12 pass, including inventory
  boundaries and mail, shop, storage, help, diary and generic window shields.
- Independent read-only review found no blocking regression in modal ownership,
  service panels, BigMap close preservation or map-image click replacement.

These controller tests inject authoritative ACKs into an in-memory native
input app; they are not a real Gateway roundtrip or native visual acceptance.
Release packaging and native handoff are recorded below when complete.
No whole-UI/Crystal acceptance percentage follows from this fix.

Logs under `C:/mir2-ui-repair-20260921`:

- `quest-route-obstacle-before.log`
- `quest-route-obstacle-after.log`
- `quest-route-hover-input-regression.log`
- `quest-route-hover-ui-guards.log`
- `quest-route-hover-client-build.log`

## Release handoff

Offline native release build succeeded. Candidate package:
`C:/numeron-legend-of-rebirth-20260923-route-hover`; client SHA256
`7540DAD6F825D70FC11F5786D92501A9AB6A16E70360D30AEAC092E15E9906C2`.
Gateway `e65bdc4eb` and original store/keys stay running without a restart.
Config hash remains `01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.
The assets junction, fixed daylight and movement/render diagnostics are retained.
After the user confirmed normal exit, no previous native client process
remained. Its log records the confirmed Exit dialog and successful event-loop
return. Client source `841da03dc87fd7cfde839e2ca3aa998981e17dae` launched as
PID29732 at local 2026-09-23 03:38 with matching executable/config hashes.
It is responsive with title `numeron-legend of rebirth`; startup confirms
complete local assets, fixed daylight and a WebSocket connection to the
retained Gateway PID36484 at `ws://127.0.0.1:19910/ws`.

Launch record: `C:/mir2-ui-repair-20260921/render-live/route-hover-client-launch.json`.
Logs use prefix `20260923-033815-735` in the same directory. The local manifest
now records `deployed=true`, while `visualAccepted=false` remains. No service
restart, account editing or gameplay input was performed. The user retains
control for actual path-following acceptance.
