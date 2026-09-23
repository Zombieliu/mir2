# Quest diary, combat continuity and town binding — 2026-09-24

The level-20 diary mixed one active newcomer V2 main quest with imported
Crystal side quests grouped by map, all expanded in one 316×466 panel. The V2
diary now defaults to **主线** and offers **可交付** and **支线** tabs. All server-owned
quest states remain intact; side quests are paged at eight rows per page, with
left-click detail and right-click tracking retained. Crystal and newcomer V1
diaries keep their original group presentation. This V2 organization is an
onboarding UI choice, not a claim of original Crystal diary parity.

Long objective, route and other-map text in the task card previously wrapped
inside a fixed 18-pixel row and drew over later controls. Rows and buttons now
reserve the height of each pre-wrapped line. No quest objective or route is
discarded. The new layout test checks long Wooma route and side-map labels;
an additional long-word boundary test prevents adjacent labels from merging.
The diary test checks that 11 imported quests remain reachable on two pages
without obscuring the Crystal frame's close control.

The live movement trace has 983 sends, 972 confirmed moves and 11 corrections.
Four movement acknowledgements took more than three seconds, the longest about
5.86 seconds. The client did **not** accumulate an unbounded run queue: it held
one unconfirmed move. Combat nonetheless stopped because a pending move blocked
attack issuance. Passive HUD/skill hover could also cancel a selected monster.
The client now continues a selected in-range attack on its ordinary cadence
while that one move awaits the server, and it treats an omitted `dead` field in
a partial actor refresh as unknown rather than dead. Actual death, modal input,
manual movement and target changes still cancel combat. Failed attacks no
longer sit in the host retry FIFO ahead of a newer target; only the latest
attack in a batch is sent, and stale attack intents are removed from both UI
lanes. Movement collision, occupancy, ACK ownership and attack validation
remain authoritative on the server. Optional attack request/forward/block
trace events are available for the next gameplay reproduction.

The shared Zone had a separate ordering defect: an accepted attack could be
followed by a previously queued Walk/Run step when its movement deadline
arrived. After authoritative melee, range or magic admission, it now cancels
only movement queued before that action and corrects the owner's location.
Rejected attacks leave that movement intact. Later movement input remains
possible at the original cadence. Three tests cover accepted normal and
materialized melee, ranged and magic actions, rejection, and later fresh steps.

Original Crystal `SetBindSafeZone` sets `BindLocation` and `BindMapIndex` to
the most recently entered safe zone's center. The old Rust TownTeleport always
used the configured starter spawn. The personal session now retains a
backward-compatible `bind_point`, refreshes it after accepted local/shared-Zone
movement and map arrivals, and saves it on normal logout. TownTeleport,
DungeonEscape and TownRevive use it; Gateway syncs the authoritative Zone
position before immediate `UseItem`. The player explicitly chose the original
**most recently entered safe zone** rule. The current a1 save is at
BichonProvince `(283,608)`, inside the village safe area, and has no historic
bind field. No live save was edited. Once the revised server is active, a
normal visit to the Bichon city safe area binds the city center `(328,264)`;
a later visit to the village safe area binds the village again, as in Crystal.

## Validation

- Shared Bevy client library: **1,079 passed**, including V2 diary paging,
  task-card height, and stale attack queue tests.
- Native Windows test executable: **732 passed**, two existing GPU soak tests
  ignored. New tests cover delayed movement ACK, passive hover, explicit
  target death, manual supersession, a saturated host channel and latest-target
  forwarding. The test executable used an alternate suffix because the default
  one is locked by another process; no game process was stopped for tests.
- Simulation scroll integration: **7 passed**, including dynamic city/village
  binding, persistence, cross-map TownTeleport, DungeonEscape and old-save
  recovery when the character is currently inside the city safe area.
- Simulation town-revive unit case: **1 passed**. Its former spawn-tile
  expectation was updated to the imported safe-zone center.
- Shared Zone integration: **209 passed**, including the three new combat
  ordering cases. Two pre-existing stale assertions were corrected: death
  clears an old effect, and BoneFamiliar now uses its Crystal 12–23 damage
  range rather than one damage.
- Gateway immediate post-step TownTeleport: **1 passed**; shared-Zone
  town-revive synchronization: **1 passed** after updating the same
  safe-zone-center expectation.
- Release client and Gateway builds passed offline. Their SHA-256 hashes are
  `DEA0A1A05C30365D4F01663D9BBB20ED9C0E151EDABFE77B4FD21003DD810CAE`
  and `6B7B85AAFFCA44C1C9A2E9DF36F3C358B3AEC655CC2EF3B67E09B688AFFB3E9E`.
  Both files were copied and hash-verified into
  `C:\numeron-legend-of-rebirth-20260924-quest-combat-bind`; the live client
  and Gateway were left running until ordinary player logout.

Physical gameplay acceptance and a sustained combat replay remain open. The
test results establish command ordering and source behavior; they do not claim
that every visual or combat issue in the whole game is complete.
