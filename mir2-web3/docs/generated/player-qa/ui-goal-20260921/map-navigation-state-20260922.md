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

Verified final candidate: source 19c23c262, Windows release build completed in
1m53s (map-complete-navigation-release.log). Big Map model 17/17, gameplay bridge
91/91 and Windows input 86/86 pass. Prepared package:
C:/numeron-legend-of-rebirth-20260922-map-navigation.
Client SHA256: 5C72AE630673D1CD2D52700A8281F3EC153D8E4FED92832114636B2A57630D4E.
Its manifest pins the six-map asset manifest hashes from the resource-repair
record, while documenting the shared mutable junction. Deployment and visual
acceptance were false at preparation. After the user's normal-exit/desktop
handoff, this client was launched (PID37312); the unchanged paired Gateway
remains PID26712 from the quest-entry-floor directory. The previous client log
records a successful exit outside the game. The user entered a1 normally.

Live D401 (19,156) evidence is saved at
C:/mir2-ui-repair-20260921/d401-19-156-live-before-minimap.png.
The task card now correctly says DeadMineEntrance and points to the return
entrance (24,182), but the minimap is absent and the self character is obscured
by cave roofs. Therefore this is only a narrow map-identity observation, not
visual acceptance. Close-and-continue walking and NPC selection remain open.

The minimap renderer incorrectly restricts its profile to Bichon titles despite
the D401 source having MMap index8 (300x199 image for a 200x200 map). A repair
must carry authoritative map index/dimensions through the model and replace the
image/geometry on transfer. Original Objects2 cave roofs contain opaque black
pixels; removing their black alpha would corrupt the source. Crystal's separate
40-percent self-body/head/wings redraw is absent in the native renderer and is
being repaired independently. Neither repair is included in this 19c23c262 EXE.
