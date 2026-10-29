# Natural equipment sprite closure — 2026-09-15

The active native r2 package lacked the Warrior's `CArmour/03` and `CWeapon/09`. Its existing `CHair/00` could render independently, consistent with the head-only R20 screenshot. This change supplies the original body/weapon libraries and adds an equipment-derived package guard. It does not certify native visual acceptance.

## Read-only save evidence

Equipment was selected from `C:/mir2-protocol-journey-20260911/gateway/accounts.json`. No account was changed; no credentials or full account records are reproduced here.

| Class | Level / observed save revision | Armour item → library | Weapon item → library |
|---|---|---|---|
| Warrior | 30 / 7430 | 1221 ThickArmour(M) → CArmour/03 | 1216 MartialSabre → CWeapon/09 |
| Wizard | 25 / 7085 | 1223 FireMagicRobe(M) → CArmour/04 | 1217 SpearWithHook → CWeapon/10 |
| Taoist | 23 / 8202 | 1190 SolidArmour(M) → CArmour/02 | 268 Scimitar → CWeapon/07 |

These are observed save revisions, not claims about subsequent running sessions. All three inspected characters use the ordinary male C sprite family and hair 0. Requirement keys are catalogue item IDs; library shapes come from `packages/game-data/data/generated/crystal_item_manifest.json`, not item names.

## Original source and installed output

The actual source is `E:/mir2/Crystal/Build/Client/Debug/Data`. Every source byte length and SHA-256 matches `docs/generated/assets/crystal-source-snapshot.generated.json`.

| Library | Source bytes | Valid exported frames | Original SHA-256 |
|---|---:|---:|---|
| CArmour/02 | 4366813 | 1616 | b274a07d0bf8cf0019171b64982e93c25322f6574576aed225b0ccf09f3d89aa |
| CArmour/03 | 3540581 | 1616 | df3dd27f8f33f12bb4a13a26f8b72c2b2175b449b1ef85f0eb4fed1dea3e8fbe |
| CArmour/04 | 3660285 | 1616 | 98797e3a3f4722a57fe0836f9ba165b2de4c98e157e2d88db1bfaceba0b06594 |
| CWeapon/07 | 355802 | 832 | 5299d552707c24c6e12df28f94f8e5e8a295bc4e57d7d8dffda3449fc37cae78 |
| CWeapon/09 | 283116 | 832 | 53578d4295468075c7d5b7bc1c096b7efa4f162a1db77edc4d99b45b7dd83161 |
| CWeapon/10 | 439818 | 832 | fa22fe5a76a7605ce9e5380150e47047009d7031ddeb16e12d9471595a4e3964 |

Installed package root: `C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-assets`.

Under `original-ui/`, the six new directories contain 7,344 PNGs and six `meta.json` files; `original-ui/manifest.generated.json` incorporates their source/frame identities. The existing exporter and decoder preserve original RGBA pixels and frame offsets. No original source file or existing baseline PNG was replaced.

The existing entity atlas was not rebuilt. All eight existing atlas page files still match their manifest SHA-256 and byte lengths. Its unchanged manifest SHA-256 is `a6f57f316c350221e30b95a006f20807a06ca4341ef4a61a65f23714922b2a40`. The Windows client's existing `verified_player_frame_geometry` / standalone-frame path can resolve the newly installed files; Rust drawing and fallback code were not changed in this task.

## Reproduction and verification

Run from `apps/web`:

```powershell
node scripts/export-crystal-ui.mjs --dataDir E:/mir2/Crystal/Build/Client/Debug/Data --outputDir C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-assets/original-ui --libraries CArmour/02,CArmour/03,CArmour/04,CWeapon/07,CWeapon/09,CWeapon/10 --fullLibraries CArmour/02,CArmour/03,CArmour/04,CWeapon/07,CWeapon/09,CWeapon/10 --concurrency 16
node scripts/build-bevy-entity-atlas-pack.mjs --verifyEquipmentClosure --assetRoot C:/mir2-natural-journey-20260911/hookingcat-resource-package-r2/mir2-assets --sourceDataDir E:/mir2/Crystal/Build/Client/Debug/Data
node --test scripts/test-equipment-sprite-closure.mjs scripts/test-item-icon-closure.mjs
```

- `export.log`: all 7,344 original present frames exported.
- `closure-source-verification.log`: zero errors; all 7,344 standalone PNG hashes, decoded RGBA hashes, dimensions and offsets match originals; 416 required existing hair atlas frames pass page integrity and geometry checks.
- `equipment-closure-tests.log`: 19/19 passed, including eight new equipment checks and eleven existing item-icon checks.
- `package-evidence.json`: source/metadata identities, representative frame geometry, selected save revisions and all eight existing atlas page identities.
- `equipment-closure-tests.initial-failure.log`: preserved first run, 6/7. One assertion exposed that capped diagnostics omitted later failing libraries; diagnostics now reserve a sample per library. The integrity rejection itself was already working.

## Code changes and limits

`natural-equipment-sprites.json` declares the inspected item IDs. `equipment-sprite-closure.mjs` derives the seven required libraries and checks complete 416-frame ordinary action blocks, including movement/combat/death/revive. Female profiles use Crystal's 808 body/hair and 416 weapon offsets. Missing files or metadata cannot reduce the denominator; source identity, geometry, PNG/RGBA digests and used atlas page integrity must pass.

`crystal-ui-export-manifest.json` now includes complete original ranges for the six new libraries so normal exports reproduce the closure. `build-bevy-entity-atlas-pack.mjs` enforces closure for a base build or an explicitly requested check. Isolated custom-root atlas builds retain their existing scope unless equipment requirements are requested. The starter packing roots and texture budget remain unchanged. Future generated atlas metadata carries page hashes and original offsets; only a tiny test fixture was rebuilt.

The new generated PNGs are installed in the external running r2 package. They are not copied into the versioned `apps/web/public` tree in this task; a future package prepared there must export these configured libraries before the new closure guard accepts it.

This evidence covers the inspected ordinary equipment profiles and full exports of their six libraries. It does not establish all equipment/transform/mount/class asset coverage, live rendered frame correctness, sustained native memory stability, or a complete visual 0→30 journey. No UI input, native process launch/restart, account write, code commit or publication occurred.
