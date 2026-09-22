# Ordinary entrance metadata repair — 2026-09-23

The user rejected the previous client-only map-identity candidate: the actual
scene and HUD title were DeadMineEntrance, but the minimap remained Bichon
and the quest card still used Bichon's entrance coordinates. Native visual
acceptance remains open. This record supersedes the assumption that the
2026-09-22 client patch alone resolved this report.

## Root cause and repair

The running native trace `20260923-000717-726-render.jsonl` records an ordinary
`0 -> D401` MapInformation at unix 1790093382890 with miniMapIndex **101**.
The imported destination is index **47**, minimap/big-map image **8**;
Bichon is index **1**, image **101**. `apply_map_transfer` changed only filename
and title using `..current_map`, retaining the source index and presentation
fields. Consequently the client could retain Bichon's quest route and image
even though the map scene correctly loaded D401. The previous synthetic
client fixtures supplied correct destination data (and an invented index
401), so they did not reproduce this server defect.

Ordinary entrance transfer now resolves all known Crystal destination metadata
through the existing `apply_crystal_map_metadata` function before relocation:
index, title, minimap, big map, lighting, music, weather and flags. Unknown
custom maps retain their configured values. Entry conditions, movement,
collision, character state and quest completion are unchanged. Entering
D401 is not completion of the separate D001 / Enter Oma Cave objective.

## Verification

- Simulation ordinary imported entrances: Bichon <-> D401 and Bichon <-> D001
  fail **0/2** before the repair, pass **2/2** after it. These tests declare
  in-memory starting-position fixtures; they are not ordinary fresh journey
  evidence. Broader map-transfer regressions pass **7/7**.
- Gateway map-transfer/cadence regressions pass **6/6**.
- Windows map-identity regressions pass **5/5**, using real manifest indices
  and actual `miniMapIndex` wire naming. Destination map definition responses
  follow the transfer invalidation, as when opening the ordinary big map.
- Standard Cargo Gateway release build succeeds. A locked old root build
  executable prevented output replacement; an alternate target root reuses
  only the development cache directories. No live executable was interrupted.

The live protocol comparison uses two isolated file stores initially copied
byte-for-byte from the same existing save. Source copy SHA256:
`A1789431540E8FB145F025CABDD87E564635CA06CA94F1747E2911A9653EA20C`.
It uses public login, StartGame, collision-valid Walk, clientVersion snapshots
and acknowledged LogOut. The user's live service/store is never written by
the probe. There are no grants, save edits or debug/QA teleports.

| Ordinary transfer | Old Gateway packet | Repaired Gateway packet |
|---|---|---|
| D401 -> Bichon | index 47, images 8, light 4: wrong | index 1, images 101, light 0: correct |
| Bichon -> D401 | source fields retained; coincidentally correct after starting in D401 | index 47, images 8, light 4: correct |

Both runs acknowledge normal logout; final snapshot and persistent save agree
on character name, map/title, position/direction, gold and experience. Before
the final baseline capture, probe-development attempts exposed a direction
spelling error and incorrect report field aliases; those records are retained
separately. The final baseline therefore begins at the normally saved D401
(24,181), while the repaired run starts from the original copy at (26,180).
This is packet/save verification, not timing, a fresh level-30 journey or
native visual acceptance. The comparison preserves both initial positions.

Machine-readable result: [comparison](map-transfer-server-metadata-20260923.json).
Full local evidence: `C:/mir2-ui-repair-20260921/map-transfer-live-20260923`.
Logs include `map-transfer-metadata-simulation-regression.log`,
`map-transfer-metadata-gateway-regression.log`,
`map-actual-identity-client-tests-final.log` and
`map-transfer-metadata-gateway-build-final.log` in the same QA root.

Reproduction commands from the project root:

```powershell
cargo +1.95.0 test --release --offline -p mir2-simulation --lib map_transfer -- --test-threads=1
cargo +1.95.0 test --release --offline -p mir2-gateway --lib map_transfer -- --test-threads=1
cargo +1.95.0 test --offline --manifest-path apps/game-client/platform-windows/Cargo.toml gateway::map_identity_tests -- --test-threads=1
# With credentials supplied through MIR2_PROBE_ACCOUNT / MIR2_PROBE_PASSWORD,
# and an isolated service using an unmodified copy of a near-entrance save:
node apps/web/scripts/quest-agent/probe-map-transfer-metadata.mjs ws://127.0.0.1:19781/ws OUTPUT_DIRECTORY 7
```

## Prepared handoff

Package: `C:/numeron-legend-of-rebirth-20260923-map-transfer-metadata`.
Gateway SHA256:
`355E8461074A130C9E9B7CE2AA6039DC3DFBE25B2F58F8677EA29A74E7FD65D6`.
The previous native client/source `192e14ce1`, atlas edge fix, fixed daylight,
movement settings and diagnostics remain paired with the repaired server.
Client SHA256:
`B94737CB547F574C77BE609E1F44F5FC230A18C91327390A22E70FD0B4B5D476`.
Live deployment is pending normal user logout and close. The original
account store and identity/recovery keys must be retained. The user retains
gameplay control; no native input was taken during this repair.
