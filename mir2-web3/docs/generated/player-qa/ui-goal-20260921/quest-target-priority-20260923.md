# Arrival route replaced by a monster hint — 2026-09-23

The user reported the entrance navigation button disappearing. The task card
still showed `Enter Oma Cave` / `Reach the Oma Cave entrance 0/1`, but displayed
`Oma (420,91) · 9` instead of the route. The live client was source `841da03dc`
/ PID29732; its movement trace shows the player at `(429,82)`. No native input,
account edits, or Gateway restart were used for this investigation.

## Cause and implementation

The actual arrival quest is **2110009**, whose authored configuration specifies
map `D001`, no kills, and server-event flag `omaCaveEntryReached`. The old
monster matcher searched all incomplete objective text, so the word `Oma` in
the cave name matched the nearby monster. The task card also selected nearby
monsters before checking the authored destination map. Consequently a nearby
Oma replaced `Place::Route` with `Place::Current` and removed the button.
Earlier entrance fixtures used quest 2110010 with no objective text and did
not exercise this collision between a destination name and a monster name.

V2 monster hints now use configured kill identities and the same-index
authoritative objective counters. The server orders snapshots as kills,
items, flags; native adaptation preserves that order, and current V2 main
definitions contain no item tasks. An arrival, equipment or class-practice
flag cannot become a kill target through its wording. Missing, zero-target
or completed kill progress produces no inferred monster target. Full normalized
species names are compared, so `Cat` cannot match the authored `RakingCat`.
Tasks absent from the V2 configuration retain the legacy packet-text matcher.

For a configured in-progress task, the card now checks the authoritative map
first. Outside its target maps, it keeps the next imported ordinary entrance
route even when a matching species is visible locally. Unknown map identity
does not claim a map-specific target is nearby. Once on the target map,
unfinished actual kill objectives may again choose nearby targets. Existing
NotStarted/ReadyToTurnIn NPC guidance remains unchanged. All of this is
read-only presentation; objective progress, rewards and movement rules are
owned by the existing server and native controller.

## Verification and limits

- Four new targeted regressions reproduce the previous implementation: 0/4.
  After repair: 4/4.
- The actual 2110009 card retains an enabled navigation intent for Bichon
  `(147,33) -> D001` as an Oma at `(420,91)` appears and disappears, with its
  original incomplete objective still visible.
- Arrival flags produce no nearest-monster/highlight target. Declared kills
  use their counter order, including translated objective text, completed
  kills, absent progress, partial species names, and an unrelated extra flag.
- A cave Skeleton task on Bichon retains its map route; authoritative map
  identity 39 then enables its actual nearby Skeleton hint. Unknown identity
  stays unknown.
- Shared-client native UI `quest_` filter: 151/151 pass, including original
  quest presentation, task switching, target actions and navigation intent
  regressions. This filter also includes some names containing `request_`.
- Independent read-only review checked server objective ordering, map changes,
  NPC accept/turn-in behavior, legacy matching and non-combat flags; no
  blocking regression was found.

Tests inspect the native UI entity tree and read models. They do not claim a
real native screenshot pass, server quest completion, or whole UI/Crystal
acceptance. Local logs under `C:/mir2-ui-repair-20260921`:

- `quest-target-priority-before.log`
- `quest-target-priority-after.log`
- `quest-target-priority-regression.log`
- `quest-target-priority-client-build.log`

## Package and runtime handoff

The offline native release build passed. Candidate package:
`C:/numeron-legend-of-rebirth-20260923-task-target`.

- Client SHA256: `C11511AD1EDAF1C1AB55B1FCA79E5AC27CCC3FB56CF0E26F1CD49433117285D7`.
- Retained Gateway source: `e65bdc4ebcd99b0c80658c087a1bf2f26c4cbc33`.
- Gateway SHA256: `355E8461074A130C9E9B7CE2AA6039DC3DFBE25B2F58F8677EA29A74E7FD65D6`.
- Config SHA256: `01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.

At 04:15 local time, repeated process/listener checks found neither prior client
PID29732 nor Gateway PID36484, with no listeners on 19900/19910. No process stop
was issued in this repair turn. Their termination cause and whether a normal
logout occurred are not established. The existing accounts store is present
(335097 bytes, modified 03:45:59); OS uptime shows no intervening reboot.

Before restoring the login screen, preserve a copy of the current store and
retain the original identity/recovery keys, profile and recovery directory.
Do not restore an older save or claim unsaved progress was verified. Native
visual acceptance remains open and the user retains gameplay control.
