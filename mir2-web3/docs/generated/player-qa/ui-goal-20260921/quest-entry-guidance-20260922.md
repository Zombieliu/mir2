# Quest entrance guidance

The user's Enter Oma Cave screenshot only names OmaCave_1F and offers a generic
map button. It does not explain how to travel there. This is a live usability
failure, not a passed quest-guidance page.

The candidate resolves V2 authored destination maps through the imported,
directional Crystal movement graph. It shows the current map/position and the
next entrance coordinates, with a `前往入口 · 自动寻路` button. Normal visible
map entrances are preferred; hole, movement-gated and conquest entrances are
excluded. Unknown routes do not fabricate coordinates. Every map change requires
a newly resolved next step; this is not automatic multi-map travel.

For BichonProvince (map 1) to OmaCave_1F (map 39), the next entrance is (147,33).
The D001 arrival location (151,362) is not used as a Bichon walking destination.
Windows reuses the existing Big Map A* planner and ordinary Walk/Run packets.
There is no client teleport, debug command or direct save mutation.

The UI validates the current primary quest, epoch, map and recomputed entrance.
The host validates map identity/epoch and loaded dimensions. Held pointer input
does not fall through to the world: release consumes the queued route. Escape,
manual movement, modal/focus/session changes cancel pending navigation.

Code evidence under C:/mir2-ui-repair-20260921:

- quest-route-client-full.log: native UI library 1045/1045.
- quest-route-windows-input.log: Windows input 83/83.
- quest-route-windows-bridge.log: held press/release and world-click isolation.
- quest-route-windows-escape.log: cancel prevents delayed restart.
- map-floor-route-rust.log: imported Mir3 small-object classification regression
  passes, supporting the paired floor-resource repair.

Release build, normal-exit switch and same-version visual/ordinary walking
verification must be recorded separately. These tests do not prove an actual
player arrived at the cave or that every map route is collision-reachable.
