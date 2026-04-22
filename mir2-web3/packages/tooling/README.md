# packages/tooling

Migration and conversion tools live here.

## Scripts

- `scripts/generate-crystal-server-parity.mjs`
  - scans `../Crystal/Server`
  - emits `docs/generated/crystal-server-parity.json`
  - use it to keep a machine-readable inventory of Crystal server modules while the Rust backend catches up
- `scripts/generate-crystal-runtime-manifests.mjs`
  - scans Crystal source defaults in `MirEnvir/Envir.cs`, `MirDatabase/BuffInfo.cs`, `Build/Server/Debug/Envir/Drops`, and `Build/Server/Debug/Envir/NPCs`
  - emits `packages/game-data/data/generated/crystal_magic_manifest.json`
  - emits `packages/game-data/data/generated/crystal_buff_manifest.json`
  - emits `packages/game-data/data/generated/crystal_drop_manifest.json`
  - emits `packages/game-data/data/generated/crystal_npc_manifest.json`
  - use it to keep Rust-side runtime manifests aligned with Crystal spell, buff, drop-table, and NPC-script definitions
- `scripts/generate-crystal-respawn-manifest.mjs`
  - scans `Build/Server/Debug/Server.MirDB` plus `Build/Server/Debug/Envir/Routes`
  - emits `packages/game-data/data/generated/crystal_respawn_manifest.json`
  - emits `packages/game-data/data/generated/crystal_item_manifest.json`
  - emits `packages/game-data/data/generated/crystal_monster_manifest.json`
  - emits `packages/game-data/data/generated/crystal_npc_info_manifest.json`
  - use it to import Crystal map respawn metadata, route points, monster references, item rows, and NPC placement/service-rate data into Rust-side game data
