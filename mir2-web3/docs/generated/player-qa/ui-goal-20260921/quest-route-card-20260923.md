# Quest-card entrance navigation — 2026-09-23

The user's post-deployment screenshot correctly names DeadMineEntrance and
its next ordinary exit `(24,182) -> BichonProvince`, at player `(23,176)`, but
clicking the persistent task card does not start travel. The live client is
source `192e14ce1`, PID35936, paired with corrected Gateway `e65bdc4eb`,
PID36484. The live movement log records physical clicks with no route progress
at that point. No desktop input or live character manipulation was used in
this investigation.

## Cause and change

Two UI dependencies incorrectly guarded the Windows route consumer:

1. It consumed the task-card queue only when `ui.quest_open()` was true. That
   means the separate Quest Diary had to be open, even though the task card
   remains visible with it closed.
2. The planner required a cached current `NewMapInfo` and the browsed big map
   to equal the current map. `NewMapInfo` is fetched when the big map is opened;
   ordinary login/transfer supplies the current identity before that cache.

The consumer now accepts the visible task card with the diary closed. The
request still checks the authoritative current map and reset epoch; the loaded
scene filename must resolve to that same imported Crystal map. Planning uses
the existing full-map A* and actual collision file, without a Big Map view or
map-info-cache prerequisite. Held mouse input stays queued until release;
ordinary Walk/Run commands remain bounded by server acknowledgements. Starting
travel from the diary dismisses its world-input shield. Existing modal/death,
manual cancellation, static/dynamic collision and map-change fences remain.

The card now displays its existing success/error feedback. The consumer writes
planning failures into that same visible feedback, and emits one bounded
`questRouteNavigation` diagnostic event containing accepted/error, quest/map
identity and destination. Success text acknowledges that a route was set and
shows Esc cancellation; it does not claim an ongoing navigation status after
a later manual cancellation. The next map still requires its next ordinary
entrance action; this change does not teleport or automatically complete quests.

## Verification and limits

New Windows input-system regressions pass **0/3** on the prior implementation
and pass **3/3** after repair. They use the packaged D401 collision file and
the user's `(23,176) -> (24,182)` route, with no diary and no NewMapInfo cache.
They verify mouse-down/release ownership, subsequent real input-system
Walk/Run dispatch, no command flooding before an injected acknowledgement,
arrival within twelve steps, remote map browsing, and rejected stale epochs
with visible error feedback. These are in-memory controller fixtures with
injected ACKs, not live Gateway or native visual acceptance.

The whole Windows `input::` suite passes **97/97**. Shared-client `route_`
tests pass **9/9**, including an actual D401/2110010 button interaction into
the precise route queue and rendered success/error text/colors. Read-only
independent review found no blocking input or movement-authority regression.
The final success wording is a presentation-only follow-up to that test run.

Local logs under `C:/mir2-ui-repair-20260921`:

- `quest-route-card-before.log` and `quest-route-card-after.log`
- `quest-route-input-regression.log`
- `quest-route-ui-regression.log`
- `quest-route-card-client-build.log`

```powershell
cargo +1.95.0 test --offline --manifest-path apps/game-client/platform-windows/Cargo.toml quest_route_input_tests -- --test-threads=1
cargo +1.95.0 test --offline --manifest-path apps/game-client/platform-windows/Cargo.toml input:: -- --test-threads=1
cargo +1.95.0 test --offline --manifest-path apps/game-client/client-bevy/Cargo.toml --features native-ui --lib route_ -- --test-threads=1
cargo +1.95.0 build --release --offline --manifest-path apps/game-client/platform-windows/Cargo.toml
```

Native release build and package are ready at
`C:/numeron-legend-of-rebirth-20260923-quest-route-card`, with client source
`47efc7896bda9c855d6a410e663e511090ac67e7` and SHA256
`210DAE35C3C7552FFB5FBD627930835BEAA6E371BDD5CD38ACEF898BDE08EE7C`.
The local candidate manifest pins the client, Gateway and config hashes and
now records `deployed=true` / `visualAccepted=false`. Daylight, movement cadence
and atlas seam settings are retained.

## Deployment after normal exit

The user confirmed normal exit. The previous native client's log ends in a
confirmed Exit dialog and successful event-loop return; Gateway logs record an
immutable checkpoint committed before connection teardown. No old native
client process remained. The replacement launched as PID44352 at local
2026-09-23 02:01, with the expected executable/config hashes and window title
`numeron-legend of rebirth`. Startup logs confirm complete local assets,
fixed daylight, enabled movement/render tracing, and WebSocket connection to
`ws://127.0.0.1:19910/ws`; the process is responsive.

Gateway PID36484 was retained with its original store and keys; no service
restart, account edits or gameplay input were performed. Launch evidence:
`C:/mir2-ui-repair-20260921/render-live/quest-route-card-client-launch.json`;
startup and trace logs use prefix `20260923-020107-979` in the same directory.
The user retains gameplay control. Native physical-click-through-arrival
acceptance remains open and is not implied by successful startup.
