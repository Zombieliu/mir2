# N5 arrival guidance repair

Reported tracker used tiny English absolute-positioned lines, no destination
and no distinction between the village and city safe zones.

Pending N5 (2110005) now gets a dedicated flow-layout card with Chinese labels,
14px Microsoft YaHei, 20px minimum text rows, automatic wrapping and a dark
background. It states Bichon city safe area (328,264), north from the starter
village, arrival-based progress and that the village safe zone does not count.
The local map button opens the existing big map without emitting a server
movement/teleport command. A cyan square marks the source safe-zone extent,
with a Chinese label; completed and wrong-map cases suppress it.

Authority: newcomer_v2_events.rs requires map 0, y<400, and the original
is_safe_zone_point test. Imported safe-zone center (328,264), size10 corresponds
to x318..338/y254..274. Manifest map index is 1 (file name is 0).
The destination itself is statically walkable and not an imported NPC cell.
Read-only protocol pathfinding from (278,604) found 342 ordinary steps with
the real 0.map collision and NPC occupancy. Dynamic occupancy is unverified.

Validation:

- `bichon_destination`: 2 tests pass (source geometry; ECS label/font and
  completed/wrong-map hiding).
- `bichon_arrival_card`: 1 test passes (coordinate, village exclusion,
  local-only button and Chinese font).
- No authenticated visual screenshot or live travel completion in this turn.

Scope is this arrival objective. Other quests retain their existing tracker;
this change is not complete-game localization or automated route acceptance.
