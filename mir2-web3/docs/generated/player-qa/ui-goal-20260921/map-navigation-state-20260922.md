# Map navigation and transfer state repair

User observations on the quest-entry-floor candidate:

- Closing the Big Map cancels an already-running path.
- GO TO stays disabled; NPC selection can be erased by the next snapshot.
- After entering DeadMineEntrance, the quest card still names BichonProvince.

The close path was treated as world input: HUD toggles, Escape and the panel's
X could call stop_auto_path. The candidate preserves an existing map route for
those map-panel controls, while retaining cancellation for world clicks outside
the panel, manual movement, Escape without the map, focus loss, death and modals.
Clicking the map image deliberately replaces the destination as before.

Packet MapInformation precedes destination UserLocation. A delayed source-map
world snapshot could subsequently call set_player_location(Some(old_map), ...),
rolling the BigMapModel identity backwards. Snapshot identity/position updates
now reject a conflicting current identity. Bootstrap snapshots and matching
destination snapshots still update normally. A raw protocol-envelope regression
covers the map packet, location packet and delayed old-map snapshot sequence.

Ordinary same-scene snapshots now preserve valid local map controls instead of
replacing NPC selection, search draft, scroll and browsing state wholesale.
Fresh search results have their own packet-driven revision and take priority;
map/session boundaries or removed targets invalidate old selection.

Crystal BigMapDialog.cs:399 implements GO TO as server-gated, paid NPC teleport,
not walking. Its enabled condition is current-map NPC CanTeleportTo. The unrelated
WorldMapSetup.Enabled gate has been removed; the current-map, nonzero object ID
and authoritative NPC permission checks remain. No forced enabling or client
teleport was added. The previous user-facing explanation calling it walking was
incorrect and has been corrected.

The real Bichon entrance (147,33) is statically walkable; Crystal movement checks
the exact source tile. Dynamic occupancy remains enforced and now reports a
temporary occupied destination instead of implying a permanent invalid route.
No neighbouring fake entrance, movement-budget increase or QA transfer is used.

Tests, build identity and live evidence are separate. Local regression logs:
big-map-route-close-tests.log, input-route-close-full.log,
map-identity-gameplay-bridge.log, map-identity-input.log,
big-map-reconcile-client.log and big-map-reconcile-platform.log under
C:/mir2-ui-repair-20260921. Actual close-and-continue walking, map transfer and NPC
selection require same-version visual acceptance after a normal-exit switch.
