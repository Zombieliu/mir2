# Full monster asset audit

All imported monster definitions, all respawn references, profile whitelist plus overrides. Static workspace asset reachability only; no running package or visual acceptance claim.

`node scripts/audit-all-monster-assets.mjs <Crystal Data directory>` regenerates this report without exporting assets.

```json
{
  "definitions": {
    "monsters": 555,
    "libraries": 296,
    "missingPublicLibrary": 435,
    "missingPublicLibraryCount": 259,
    "incompletePublicAnimation": 523,
    "noNativeAtlas": 530,
    "noNativeAtlasLibraryCount": 287,
    "incompleteNativeAnimation": 532
  },
  "respawnReferenced": {
    "monsters": 441,
    "libraries": 242,
    "missingPublicLibrary": 342,
    "missingPublicLibraryCount": 206,
    "incompletePublicAnimation": 416,
    "noNativeAtlas": 422,
    "noNativeAtlasLibraryCount": 233,
    "incompleteNativeAnimation": 424
  },
  "profileWhitelist": {
    "monsters": 174,
    "libraries": 103,
    "missingPublicLibrary": 120,
    "missingPublicLibraryCount": 71,
    "incompletePublicAnimation": 159,
    "noNativeAtlas": 163,
    "noNativeAtlasLibraryCount": 94,
    "incompleteNativeAnimation": 164
  },
  "profileSpawned": {
    "monsters": 169,
    "libraries": 98,
    "missingPublicLibrary": 115,
    "missingPublicLibraryCount": 66,
    "incompletePublicAnimation": 154,
    "noNativeAtlas": 158,
    "noNativeAtlasLibraryCount": 89,
    "incompleteNativeAnimation": 159
  },
  "bichon": {
    "monsters": 21,
    "libraries": 17,
    "missingPublicLibrary": 1,
    "missingPublicLibraryCount": 1,
    "incompletePublicAnimation": 5,
    "noNativeAtlas": 10,
    "noNativeAtlasLibraryCount": 8,
    "incompleteNativeAnimation": 12
  },
  "profileLevel1to30": {
    "monsters": 83,
    "libraries": 45,
    "missingPublicLibrary": 48,
    "missingPublicLibraryCount": 23,
    "incompletePublicAnimation": 73,
    "noNativeAtlas": 74,
    "noNativeAtlasLibraryCount": 38,
    "incompleteNativeAnimation": 74
  },
  "respawnRecords": 6341,
  "atlasPages": 8,
  "missingAtlasPages": 0,
  "newcomerV2TargetsAndBoneFamiliar": {
    "monsters": 11,
    "libraries": 11,
    "missingPublicLibrary": 4,
    "missingPublicLibraryCount": 4,
    "incompletePublicAnimation": 9,
    "noNativeAtlas": 9,
    "noNativeAtlasLibraryCount": 9,
    "incompleteNativeAnimation": 9
  },
  "profileBichon": {
    "monsters": 15,
    "libraries": 13,
    "missingPublicLibrary": 0,
    "missingPublicLibraryCount": 0,
    "incompletePublicAnimation": 3,
    "noNativeAtlas": 6,
    "noNativeAtlasLibraryCount": 4,
    "incompleteNativeAnimation": 7
  }
}
```

Native Monster frames require atlas membership: platform-windows/src/atlas.rs build_entity_layer and player_frame_path_parts exclude Monster from standalone PNG fallback. All eight directions of every imported action are enumerated, including count + skip direction stride. Zero-size source frames are listed separately, not automatically treated as export defects.

## Bichon actual respawn definitions

| Monster | Level | Library | Respawns | PNG animation | Native atlas / animation |
|---|---:|---|---:|---|---|
| ArcherGuard | 0 | Monster/139 | 36 | MISSING | 80 / MISSING |
| Guard | 0 | Monster/000 | 39 | complete | 128 / complete |
| Hen | 2 | Monster/003 | 5 | complete | 232 / complete |
| Deer | 12 | Monster/004 | 15 | complete | 232 / complete |
| Yob | 12 | Monster/008 | 6 | complete | 0 / MISSING |
| Scarecrow | 10 | Monster/005 | 9 | complete | 234 / complete |
| HookingCat | 13 | Monster/006 | 9 | complete | 224 / complete |
| RakingCat | 13 | Monster/007 | 8 | complete | 448 / complete |
| SpittingSpider | 16 | Monster/012 | 17 | complete | 224 / complete |
| CannibalPlant | 20 | Monster/010 | 9 | complete | 164 / complete |
| Oma | 15 | Monster/009 | 19 | MISSING | 0 / MISSING |
| ForestYeti | 16 | Monster/011 | 9 | MISSING | 0 / MISSING |
| FrostTiger | 47 | Monster/102 | 7 | MISSING | 0 / MISSING |
| ChestnutTree | 60 | Monster/013 | 76 | complete | 0 / MISSING |
| ChestnutTree1 | 60 | Monster/013 | 28 | complete | 0 / MISSING |
| ChestnutTree2 | 60 | Monster/013 | 25 | complete | 0 / MISSING |
| EbonyTree | 60 | Monster/014 | 77 | complete | 0 / MISSING |
| CherryTree | 60 | Monster/016 | 74 | complete | 0 / MISSING |
| LargeMushroom | 60 | Monster/015 | 77 | complete | 0 / MISSING |
| Royal_Guard | 255 | Monster/000 | 27 | complete | 128 / complete |
| Royal_Archer | 255 | Monster/139 | 26 | MISSING | 80 / MISSING |

## Profile levels 1–30 (monster level, not quest route level)

| Monster | Level | Library | Respawns | PNG animation | Native atlas / animation |
|---|---:|---|---:|---|---|
| Hen | 2 | Monster/003 | 5 | complete | 232 / complete |
| Deer | 12 | Monster/004 | 15 | complete | 232 / complete |
| Sheep | 13 | Monster/103 | 2 | MISSING | 0 / MISSING |
| Wolf | 16 | Monster/104 | 6 | MISSING | 0 / MISSING |
| Currish | 16 | Monster/104 | 1 | MISSING | 0 / MISSING |
| Scarecrow | 10 | Monster/005 | 9 | complete | 234 / complete |
| HookingCat | 13 | Monster/006 | 9 | complete | 224 / complete |
| HookingCat0 | 13 | Monster/006 | 2 | complete | 224 / complete |
| RakingCat | 13 | Monster/007 | 8 | complete | 448 / complete |
| RakingCat0 | 13 | Monster/007 | 2 | complete | 448 / complete |
| SpittingSpider | 16 | Monster/012 | 17 | complete | 224 / complete |
| CannibalPlant | 20 | Monster/010 | 9 | complete | 164 / complete |
| Oma | 15 | Monster/009 | 19 | MISSING | 0 / MISSING |
| Oma0 | 15 | Monster/009 | 3 | MISSING | 0 / MISSING |
| OmaFighter | 22 | Monster/017 | 10 | MISSING | 0 / MISSING |
| OmaFighter0 | 22 | Monster/017 | 3 | MISSING | 0 / MISSING |
| OmaWarrior | 28 | Monster/018 | 7 | MISSING | 0 / MISSING |
| ForestYeti | 16 | Monster/011 | 9 | MISSING | 0 / MISSING |
| ForestYeti0 | 16 | Monster/011 | 2 | MISSING | 0 / MISSING |
| RedSnake | 17 | Monster/111 | 14 | MISSING | 0 / MISSING |
| RedSnake0 | 17 | Monster/111 | 2 | MISSING | 0 / MISSING |
| RedViper | 17 | Monster/111 | 2 | MISSING | 0 / MISSING |
| TigerSnake | 17 | Monster/112 | 15 | MISSING | 0 / MISSING |
| TigerSnake0 | 17 | Monster/112 | 3 | MISSING | 0 / MISSING |
| TigerViper | 18 | Monster/112 | 2 | MISSING | 0 / MISSING |
| Keratoid | 16 | Monster/106 | 3 | MISSING | 0 / MISSING |
| Keratoid0 | 16 | Monster/106 | 1 | MISSING | 0 / MISSING |
| ShellNipper | 16 | Monster/105 | 5 | MISSING | 0 / MISSING |
| ShellNipper0 | 16 | Monster/105 | 1 | MISSING | 0 / MISSING |
| SkyStinger | 16 | Monster/108 | 2 | MISSING | 0 / MISSING |
| SkyStinger0 | 16 | Monster/108 | 1 | MISSING | 0 / MISSING |
| VisceralWorm | 16 | Monster/110 | 1 | MISSING | 0 / MISSING |
| SandWorm | 17 | Monster/109 | 2 | MISSING | 0 / MISSING |
| GiantKeratoid | 27 | Monster/107 | 2 | MISSING | 0 / MISSING |
| CaveBat | 20 | Monster/019 | 72 | MISSING | 0 / MISSING |
| CaveBat0 | 20 | Monster/019 | 10 | MISSING | 0 / MISSING |
| Scorpion | 18 | Monster/021 | 11 | MISSING | 0 / MISSING |
| Skeleton | 18 | Monster/022 | 11 | MISSING | 0 / MISSING |
| Skeleton0 | 18 | Monster/022 | 5 | MISSING | 0 / MISSING |
| AxeSkeleton | 18 | Monster/024 | 8 | MISSING | 0 / MISSING |
| AxeSkeleton0 | 18 | Monster/024 | 3 | MISSING | 0 / MISSING |
| BoneFighter | 20 | Monster/023 | 12 | MISSING | 0 / MISSING |
| BoneFighter0 | 20 | Monster/023 | 8 | MISSING | 0 / MISSING |
| BoneWarrior | 19 | Monster/025 | 8 | MISSING | 0 / MISSING |
| BoneWarrior0 | 19 | Monster/025 | 8 | MISSING | 0 / MISSING |
| Zombie1 | 25 | Monster/073 | 14 | MISSING | 0 / MISSING |
| Zombie10 | 25 | Monster/073 | 13 | MISSING | 0 / MISSING |
| Zombie2 | 25 | Monster/069 | 18 | MISSING | 0 / MISSING |
| Zombie20 | 25 | Monster/069 | 13 | MISSING | 0 / MISSING |
| Zombie3 | 12 | Monster/070 | 15 | MISSING | 0 / MISSING |
| Zombie30 | 12 | Monster/070 | 8 | MISSING | 0 / MISSING |
| Zombie4 | 25 | Monster/071 | 16 | MISSING | 0 / MISSING |
| Zombie40 | 25 | Monster/071 | 7 | MISSING | 0 / MISSING |
| Zombie5 | 25 | Monster/072 | 16 | MISSING | 0 / MISSING |
| Zombie50 | 25 | Monster/072 | 9 | MISSING | 0 / MISSING |
| CrawlerZombie | 25 | Monster/072 | 1 | MISSING | 0 / MISSING |
| RotNdZombie | 25 | Monster/071 | 0 | MISSING | 0 / MISSING |
| RotShamanZombie | 25 | Monster/073 | 1 | MISSING | 0 / MISSING |
| Zombie51 | 25 | Monster/073 | 1 | MISSING | 0 / MISSING |
| BlackMaggot | 28 | Monster/038 | 162 | MISSING | 0 / MISSING |
| BlackMaggot0 | 28 | Monster/038 | 25 | MISSING | 0 / MISSING |
| Centipede | 26 | Monster/037 | 58 | MISSING | 0 / MISSING |
| Centipede0 | 26 | Monster/037 | 18 | MISSING | 0 / MISSING |
| GiantWorm | 26 | Monster/036 | 36 | MISSING | 0 / MISSING |
| GiantWorm0 | 26 | Monster/036 | 10 | MISSING | 0 / MISSING |
| WhimperingBee | 26 | Monster/035 | 42 | MISSING | 0 / MISSING |
| WhimperingBee0 | 26 | Monster/035 | 14 | MISSING | 0 / MISSING |
| BugBatMaggot | 30 | Monster/043 | 189 | complete | 0 / MISSING |
| SpiderFrog | 27 | Monster/081 | 32 | MISSING | 0 / MISSING |
| BlueHoroBlaster | 26 | Monster/083 | 22 | MISSING | 0 / MISSING |
| KekTal | 28 | Monster/084 | 27 | MISSING | 0 / MISSING |
| VioletKekTal | 28 | Monster/085 | 28 | MISSING | 0 / MISSING |
| Dung | 26 | Monster/027 | 2 | MISSING | 0 / MISSING |
| WoomaSoldier | 30 | Monster/029 | 2 | MISSING | 0 / MISSING |
| WoomaSoldier0 | 30 | Monster/029 | 2 | MISSING | 0 / MISSING |
| WoomaFighter | 30 | Monster/030 | 11 | MISSING | 0 / MISSING |
| WoomaFighter0 | 30 | Monster/030 | 3 | MISSING | 0 / MISSING |
| WoomaWarrior | 30 | Monster/031 | 11 | MISSING | 0 / MISSING |
| WoomaWarrior0 | 30 | Monster/031 | 2 | MISSING | 0 / MISSING |
| CursedPriest | 30 | Monster/069 | 15 | MISSING | 0 / MISSING |
| CursedZombie | 30 | Monster/070 | 24 | MISSING | 0 / MISSING |
| HungryZombie | 30 | Monster/071 | 26 | MISSING | 0 / MISSING |
| ShiZombie | 30 | Monster/070 | 17 | MISSING | 0 / MISSING |

## Remediation

1. Export missing public libraries from the recorded original Crystal Data source, preserving complete metadata, source frame geometry and all action directions. Review zero-size source frames separately.
2. Add scoped, lazy native Monster PNG support analogous to verified player frames, or build bounded zone-specific atlases for every profile-used library. Existing eight atlas pages alone cannot cover the profile.
3. Re-run this full manifest/reference audit after changes, then compare authenticated scene screenshots and movement/attack/death animations with Crystal. Static presence is not visual parity.
4. Include summons and authored newcomer monsters in a further runtime snapshot union; this report includes all 555 definitions but spawn subsets represent imported respawns and profile overrides, not exhaustive dynamic summons.

## Newcomer V2 exact configured targets plus BoneFamiliar

| Monster | Level | Library | Respawns | PNG animation | Native atlas / animation |
|---|---:|---|---:|---|---|
| BoneFamiliar | 15 | Monster/078 | 0 | MISSING | 0 / MISSING |
| Scarecrow | 10 | Monster/005 | 9 | complete | 234 / complete |
| RakingCat | 13 | Monster/007 | 8 | complete | 448 / complete |
| Oma | 15 | Monster/009 | 19 | MISSING | 0 / MISSING |
| ForestYeti | 16 | Monster/011 | 9 | MISSING | 0 / MISSING |
| Skeleton | 18 | Monster/022 | 11 | MISSING | 0 / MISSING |
| Zombie2 | 25 | Monster/069 | 18 | MISSING | 0 / MISSING |
| Zombie3 | 12 | Monster/070 | 15 | MISSING | 0 / MISSING |
| Dung | 26 | Monster/027 | 2 | MISSING | 0 / MISSING |
| WoomaSoldier | 30 | Monster/029 | 2 | MISSING | 0 / MISSING |
| WoomaFighter | 30 | Monster/030 | 11 | MISSING | 0 / MISSING |

## Original-library mapping caveat

sourceExists tests the runtime Monster/{image} path, not all alternative Crystal libraries. Crystal MonsterObject.cs maps 900/904 to Dragon, 950..953 to Gate/00..03 and 10000+ to Pet/00+; these direct Monster paths must not be reported as missing original artwork.

The server snapshot currently emits Monster/{image:03} in apps/simulation/src/runtime/packets.rs:8114. Crystal's client switches library family in Client/MirObjects/MonsterObject.cs:149. Profile SabukGate and PalaceWallLeft/1/2 need a dedicated Gate-family mapping check before any export plan; exporting nonexistent Monster/950.Lib is not a repair.
