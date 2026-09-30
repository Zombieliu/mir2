# V2 search priority and cumulative recovery evidence

The fresh ordinary Taoist paused on N7 at 2026-09-17T20:46:44.863Z.
Both 30-second searches and the single 5-second respawn observation were used.
Each search stopped before finishing its first waypoint, although accepted
owner locations showed progress. Old ForestYeti observations farther away
were prioritized ahead of the nearest manifest-backed field; the navigation
consumed the bounded search window. No extra retry or spawn budget is added.

V2 now explicitly disables historical ordinary-monster spawn hints. Current
live AOI targets keep priority and CannibalPlant retains its close-reveal
special case. Legacy V1 keeps its existing historical-hint behavior. The
implementation worker ran the affected combat suite: 156/156 pass, including
stale-hint, live-target and default-V1 regressions.

A separate audit found that normal process resume retained the original clock
but reset recovery rows, allowing the three-recovery cap to be reused. A strict
V2-only local ledger now clones confirmed death/town-revive evidence and starts
the next controller with the accumulated rows. The original ordinary start
time is unchanged. Invalid/incomplete existing V2 evidence fails closed rather
than granting a fresh clock or recovery allowance. These records never write
server character or quest state.

Ledger tests pass 14/14; affected V2 controller tests pass 38/38, including
cross-resume recovery limits and clone isolation. Syntax and scoped diff
checks pass. Root independently loaded the real paused Taoist report: its one
confirmed recovery and 20:35:16.489Z original start survive, leaving two
recoveries before the unchanged 22:35:16.489Z deadline.

The same ordinary account will resume after integration. Full route, dynamic
survival, clean timing and visual acceptance remain open.
