# V2 quest hunting areas — 2026-09-20

Request: a new player cannot tell where to hunt RakingCat from the kill objective.

The current-map panel displays amber rectangles based on actual imported spawn
center/spread for unfinished V2 kill objectives, capped at three target species.
Labels show species, remaining count and center coordinates. The nearest spawn
is selected relative to the player. This does not expose exact live positions,
add teleport commands, or prove automated travel works. Imported spawn data
does not cover custom training-spawn overrides; full V2 map coverage is open.

The a1 regression fixture preserves authoritative N3 progress: Scarecrow 2/2,
RakingCat 0/2. It yields only RakingCat at (340,550), with radius 50. Completing
the second objective removes that marker. Unknown maps produce no marker.
The N3 detail hint explicitly directs the player northeast of the starter
village, with the coordinate and explanation of the amber areas.

Validation with Rust 1.95.0, offline:

- client-bevy native-ui `hunt_regions`: 2 passed, covering source/progress and
  spawned UI label (not a screenshot).
- client-bevy native-ui `big_map`: 19 passed, including the label test above.
- platform-windows release build passed.

Package: `C:/numeron-legend-of-rebirth-20260920-hunt-map`.
EXE SHA-256: `E5EBF2A311B4D986478A76740F78BC3CBF4F61C459D295DAE64A24C64B158954`.
It uses the existing asset junction and Gateway 19910 with persisted
`quest_guidance = "newcomer-v2"`. It is a local test package, not standalone
distribution. No server/store changes were made for this feature.

Pending: authenticated visual check of region alignment, label legibility,
progress removal and map interaction in this EXE. No Crystal 1:1 visual pass
or automatic-path completion is claimed.
