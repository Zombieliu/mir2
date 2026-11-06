# Map Environment Data Slice E0

Status: bounded data/packet closure; whole map-event parity remains open.

## Crystal baseline

- `Crystal/Server/MirDatabase/MapInfo.cs`: binary `Load`/`Save` order for
  `Music`, `Fire`, `FireDamage`, `Lightning`, `LightningDamage`,
  `FireWallLimit`, and `FireWallCount`.
- `Crystal/Server/MirEnvir/Map.cs`: 3-15 second fire/lightning scheduling,
  per-player strike selection, one-second spell-object lifetime, and 500 ms
  tick speed.
- `Crystal/Shared/ServerPackets.cs`: `MapInformation` environment fields.

## Generated evidence

- Source: `Crystal/Build/Server/Debug/Server.MirDB`, version 117.
- Source map records: 464.
- Named maps: 463.
- Empty source placeholder: map index 477; retained as source evidence and not
  counted as a playable map.
- Hazard-enabled maps: 12.
- Lightning fixture: `D2081` / `LightningCave`, damage cap 100.
- Fire fixture: `D2082` / `MoltenRockCave`, damage cap 100.
- Enabled fire-wall-limit maps: 0.
- Nonzero map-music records: 0.

## Runtime closure

- Crystal map/world profiles derive `SimulationConfig.map_hazards` from the
  generated manifest.
- Selected-map metadata sets exact `MapInformation` fire/lightning bits and
  music while retaining light, dark-light, and weather values.
- No fallback hazard rows or fabricated values are used for this path.

## Verification

- `mir2-game-data::tests::crystal_respawn_manifest_loads`: 1 passed.
- `apps/simulation/tests/crystal_map_hazards.rs`: 3 passed.
- Existing personal/shared hazard behavior filters: 5 passed.
- Package and Web manifests are byte-identical at SHA-256
  `E885C879A8FDA4BF1967C41D4F271B1C82F6CBE9CB499505E9C3C7CE036015B6`.
- Web manifest sync gate: 464 source records / 463 named maps, passed.
- Web `npm run typecheck`, including the sync gate: passed.
- Exact changed-file Rustfmt: passed.
- Exact changed-file `git diff --check`: passed.

## Explicitly open

- Crystal `System.Random` trace equivalence and replay ledger.
- General `Events/*.txt` delayed action execution.
- Ordinary door open/5-second close packet order.
- Castle gate, wall, and blocking-object script semantics.
- Gateway cross-map/AOI end-to-end evidence for every event binding.

This report must not be used to claim full map, backend, Candidate, or Accepted
parity.
