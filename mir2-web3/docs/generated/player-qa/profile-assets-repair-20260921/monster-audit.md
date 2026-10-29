# Monster asset repair audit

Static workspace reachability of every imported action and all eight directions; drawable-frame closure is separate from source-empty slots and source-descriptor out-of-range references. No package, live rendering or visual acceptance claim.

Run: node scripts/audit-all-monster-assets.mjs --sourceRoot <Crystal Data directory> --outDir <report directory>. The earlier all-monster-assets-20260921 baseline is preserved.

Atlas first; otherwise canonical Monster/NNN, Gate/00..03, Pet/00..14 standalone requires valid PNG signature/IHDR, matching metadata path/geometry, source geometry and recorded PNG SHA256 when present. PNG decoding/pixel equivalence is covered separately by the profile exporter verifier.

Source-empty slots are counted and excluded from missing exports. Out-of-range action indices are retained separately and are not counted as drawable closure failures. Unknown sources, absent actions, descriptor mismatches and pending library mappings cannot pass closure.

```json
{
  "definitions": {
    "monsters": 555,
    "libraries": 296,
    "missingPublicLibrary": 227,
    "incompletePublicDrawableAnimation": 230,
    "incompleteNativeDrawableAnimation": 230,
    "unknownAnimationOrMapping": 19,
    "monstersWithSourceEmptyActions": 1,
    "monstersWithDescriptorOutOfRange": 19
  },
  "respawnReferenced": {
    "monsters": 441,
    "libraries": 242,
    "missingPublicLibrary": 168,
    "incompletePublicDrawableAnimation": 170,
    "incompleteNativeDrawableAnimation": 170,
    "unknownAnimationOrMapping": 5,
    "monstersWithSourceEmptyActions": 0,
    "monstersWithDescriptorOutOfRange": 15
  },
  "profileWhitelist": {
    "monsters": 174,
    "libraries": 103,
    "missingPublicLibrary": 0,
    "incompletePublicDrawableAnimation": 0,
    "incompleteNativeDrawableAnimation": 0,
    "unknownAnimationOrMapping": 0,
    "monstersWithSourceEmptyActions": 1,
    "monstersWithDescriptorOutOfRange": 2
  },
  "profileSpawned": {
    "monsters": 169,
    "libraries": 98,
    "missingPublicLibrary": 0,
    "incompletePublicDrawableAnimation": 0,
    "incompleteNativeDrawableAnimation": 0,
    "unknownAnimationOrMapping": 0,
    "monstersWithSourceEmptyActions": 0,
    "monstersWithDescriptorOutOfRange": 2
  },
  "newcomerV2TargetsAndBoneFamiliar": {
    "monsters": 11,
    "libraries": 11,
    "missingPublicLibrary": 0,
    "incompletePublicDrawableAnimation": 0,
    "incompleteNativeDrawableAnimation": 0,
    "unknownAnimationOrMapping": 0,
    "monstersWithSourceEmptyActions": 0,
    "monstersWithDescriptorOutOfRange": 0
  },
  "bichon": {
    "monsters": 21,
    "libraries": 17,
    "missingPublicLibrary": 1,
    "incompletePublicDrawableAnimation": 1,
    "incompleteNativeDrawableAnimation": 1,
    "unknownAnimationOrMapping": 0,
    "monstersWithSourceEmptyActions": 0,
    "monstersWithDescriptorOutOfRange": 0
  },
  "profileBichon": {
    "monsters": 15,
    "libraries": 13,
    "missingPublicLibrary": 0,
    "incompletePublicDrawableAnimation": 0,
    "incompleteNativeDrawableAnimation": 0,
    "unknownAnimationOrMapping": 0,
    "monstersWithSourceEmptyActions": 0,
    "monstersWithDescriptorOutOfRange": 0
  },
  "atlasPages": 8,
  "missingAtlasPages": 0
}
```

## Profile whitelist

| Monster | Library | Public drawable | Native drawable | Empty action frames | Out-of-range | Unknown |
|---|---|---|---|---:|---:|---|
| ArcherGuard | Monster/139 | complete | complete | 0 | 0 | false |
| Guard | Monster/000 | complete | complete | 0 | 0 | false |
| Hen | Monster/003 | complete | complete | 0 | 0 | false |
| Deer | Monster/004 | complete | complete | 0 | 0 | false |
| Sheep | Monster/103 | complete | complete | 0 | 7 | false |
| Wolf | Monster/104 | complete | complete | 0 | 0 | false |
| Currish | Monster/104 | complete | complete | 0 | 0 | false |
| Scarecrow | Monster/005 | complete | complete | 0 | 0 | false |
| HookingCat | Monster/006 | complete | complete | 0 | 0 | false |
| HookingCat0 | Monster/006 | complete | complete | 0 | 0 | false |
| RakingCat | Monster/007 | complete | complete | 0 | 0 | false |
| RakingCat0 | Monster/007 | complete | complete | 0 | 0 | false |
| SpittingSpider | Monster/012 | complete | complete | 0 | 0 | false |
| CannibalPlant | Monster/010 | complete | complete | 0 | 0 | false |
| Oma | Monster/009 | complete | complete | 0 | 0 | false |
| Oma0 | Monster/009 | complete | complete | 0 | 0 | false |
| OmaFighter | Monster/017 | complete | complete | 0 | 0 | false |
| OmaFighter0 | Monster/017 | complete | complete | 0 | 0 | false |
| OmaWarrior | Monster/018 | complete | complete | 0 | 0 | false |
| ForestYeti | Monster/011 | complete | complete | 0 | 0 | false |
| ForestYeti0 | Monster/011 | complete | complete | 0 | 0 | false |
| ChestnutTree | Monster/013 | complete | complete | 0 | 0 | false |
| ChestnutTree1 | Monster/013 | complete | complete | 0 | 0 | false |
| ChestnutTree2 | Monster/013 | complete | complete | 0 | 0 | false |
| EbonyTree | Monster/014 | complete | complete | 0 | 0 | false |
| RedSnake | Monster/111 | complete | complete | 0 | 0 | false |
| RedSnake0 | Monster/111 | complete | complete | 0 | 0 | false |
| RedViper | Monster/111 | complete | complete | 0 | 0 | false |
| TigerSnake | Monster/112 | complete | complete | 0 | 0 | false |
| TigerSnake0 | Monster/112 | complete | complete | 0 | 0 | false |
| TigerViper | Monster/112 | complete | complete | 0 | 0 | false |
| Keratoid | Monster/106 | complete | complete | 0 | 0 | false |
| Keratoid0 | Monster/106 | complete | complete | 0 | 0 | false |
| ShellNipper | Monster/105 | complete | complete | 0 | 0 | false |
| ShellNipper0 | Monster/105 | complete | complete | 0 | 0 | false |
| SkyStinger | Monster/108 | complete | complete | 0 | 0 | false |
| SkyStinger0 | Monster/108 | complete | complete | 0 | 0 | false |
| VisceralWorm | Monster/110 | complete | complete | 0 | 0 | false |
| SandWorm | Monster/109 | complete | complete | 0 | 0 | false |
| GiantKeratoid | Monster/107 | complete | complete | 0 | 0 | false |
| CaveBat | Monster/019 | complete | complete | 0 | 0 | false |
| CaveBat0 | Monster/019 | complete | complete | 0 | 0 | false |
| CaveMaggot | Monster/020 | complete | complete | 0 | 0 | false |
| Scorpion | Monster/021 | complete | complete | 0 | 0 | false |
| Skeleton | Monster/022 | complete | complete | 0 | 0 | false |
| Skeleton0 | Monster/022 | complete | complete | 0 | 0 | false |
| AxeSkeleton | Monster/024 | complete | complete | 0 | 0 | false |
| AxeSkeleton0 | Monster/024 | complete | complete | 0 | 0 | false |
| BoneFighter | Monster/023 | complete | complete | 0 | 0 | false |
| BoneFighter0 | Monster/023 | complete | complete | 0 | 0 | false |
| BoneWarrior | Monster/025 | complete | complete | 0 | 0 | false |
| BoneWarrior0 | Monster/025 | complete | complete | 0 | 0 | false |
| BoneElite | Monster/026 | complete | complete | 0 | 0 | false |
| Zombie1 | Monster/073 | complete | complete | 0 | 0 | false |
| Zombie10 | Monster/073 | complete | complete | 0 | 0 | false |
| Zombie2 | Monster/069 | complete | complete | 0 | 0 | false |
| Zombie20 | Monster/069 | complete | complete | 0 | 0 | false |
| Zombie3 | Monster/070 | complete | complete | 0 | 0 | false |
| Zombie30 | Monster/070 | complete | complete | 0 | 0 | false |
| Zombie4 | Monster/071 | complete | complete | 0 | 0 | false |
| Zombie40 | Monster/071 | complete | complete | 0 | 0 | false |
| Zombie5 | Monster/072 | complete | complete | 0 | 0 | false |
| Zombie50 | Monster/072 | complete | complete | 0 | 0 | false |
| Ghoul | Monster/074 | complete | complete | 0 | 0 | false |
| CrawlerZombie | Monster/072 | complete | complete | 0 | 0 | false |
| RotNdZombie | Monster/071 | complete | complete | 0 | 0 | false |
| RotShamanZombie | Monster/073 | complete | complete | 0 | 0 | false |
| Zombie51 | Monster/073 | complete | complete | 0 | 0 | false |
| HiGreatGhoul | Monster/074 | complete | complete | 0 | 0 | false |
| WhiteSerpent | Monster/114 | complete | complete | 0 | 0 | false |
| BlackMaggot | Monster/038 | complete | complete | 0 | 0 | false |
| BlackMaggot0 | Monster/038 | complete | complete | 0 | 0 | false |
| Centipede | Monster/037 | complete | complete | 0 | 0 | false |
| Centipede0 | Monster/037 | complete | complete | 0 | 0 | false |
| GiantWorm | Monster/036 | complete | complete | 0 | 0 | false |
| GiantWorm0 | Monster/036 | complete | complete | 0 | 0 | false |
| WhimperingBee | Monster/035 | complete | complete | 0 | 0 | false |
| WhimperingBee0 | Monster/035 | complete | complete | 0 | 0 | false |
| Tongs | Monster/039 | complete | complete | 0 | 0 | false |
| Tongs0 | Monster/039 | complete | complete | 0 | 0 | false |
| EvilTongs | Monster/040 | complete | complete | 0 | 0 | false |
| EvilTongs2 | Monster/040 | complete | complete | 0 | 0 | false |
| EvilCentipede | Monster/041 | complete | complete | 0 | 0 | false |
| WedgeMoth | Monster/044 | complete | complete | 0 | 0 | false |
| BugBatMaggot | Monster/043 | complete | complete | 0 | 0 | false |
| RedBoar | Monster/045 | complete | complete | 0 | 0 | false |
| RedBoar0 | Monster/045 | complete | complete | 0 | 0 | false |
| RedBoar3 | Monster/045 | complete | complete | 0 | 0 | false |
| BlackBoar | Monster/046 | complete | complete | 0 | 0 | false |
| BlackBoar0 | Monster/046 | complete | complete | 0 | 0 | false |
| BlackBoar3 | Monster/046 | complete | complete | 0 | 0 | false |
| WhiteBoar | Monster/048 | complete | complete | 0 | 0 | false |
| SnakeScorpion | Monster/047 | complete | complete | 0 | 0 | false |
| SnakeScorpion0 | Monster/047 | complete | complete | 0 | 0 | false |
| SnakeScorpion3 | Monster/047 | complete | complete | 0 | 0 | false |
| Ancient_SnakeScorpion | Monster/047 | complete | complete | 0 | 0 | false |
| Ancient_SnakeScorpion0 | Monster/047 | complete | complete | 0 | 0 | false |
| Ancient_RedBoar | Monster/045 | complete | complete | 0 | 0 | false |
| Ancient_RedBoar0 | Monster/045 | complete | complete | 0 | 0 | false |
| Ancient_BlackBoar | Monster/046 | complete | complete | 0 | 0 | false |
| Ancient_BlackBoar0 | Monster/046 | complete | complete | 0 | 0 | false |
| Ancient_WhiteBoar | Monster/048 | complete | complete | 0 | 0 | false |
| GiantRat | Monster/063 | complete | complete | 0 | 0 | false |
| GiantRat0 | Monster/063 | complete | complete | 0 | 0 | false |
| ZumaArcher | Monster/064 | complete | complete | 0 | 0 | false |
| ZumaArcher0 | Monster/064 | complete | complete | 0 | 0 | false |
| ZumaArcher3 | Monster/064 | complete | complete | 0 | 0 | false |
| ZumaStatue | Monster/065 | complete | complete | 0 | 0 | false |
| ZumaStatue0 | Monster/065 | complete | complete | 0 | 0 | false |
| ZumaStatue3 | Monster/065 | complete | complete | 0 | 0 | false |
| ZumaGuardian | Monster/066 | complete | complete | 0 | 0 | false |
| ZumaGuardian0 | Monster/066 | complete | complete | 0 | 0 | false |
| ZumaGuardian3 | Monster/066 | complete | complete | 0 | 0 | false |
| ZumaGuardian00 | Monster/066 | complete | complete | 0 | 0 | false |
| RedThunderZuma | Monster/067 | complete | complete | 0 | 0 | false |
| ZumaTaurus | Monster/068 | complete | complete | 0 | 0 | false |
| SpiderFrog | Monster/081 | complete | complete | 0 | 0 | false |
| BlueHoroBlaster | Monster/083 | complete | complete | 0 | 0 | false |
| KekTal | Monster/084 | complete | complete | 0 | 0 | false |
| VioletKekTal | Monster/085 | complete | complete | 0 | 0 | false |
| Dung | Monster/027 | complete | complete | 0 | 0 | false |
| WoomaSoldier | Monster/029 | complete | complete | 0 | 0 | false |
| WoomaSoldier0 | Monster/029 | complete | complete | 0 | 0 | false |
| WoomaFighter | Monster/030 | complete | complete | 0 | 0 | false |
| WoomaFighter0 | Monster/030 | complete | complete | 0 | 0 | false |
| WoomaWarrior | Monster/031 | complete | complete | 0 | 0 | false |
| WoomaWarrior0 | Monster/031 | complete | complete | 0 | 0 | false |
| FlamingWooma | Monster/032 | complete | complete | 0 | 0 | false |
| FlamingWooma0 | Monster/032 | complete | complete | 0 | 0 | false |
| WoomaGuardian | Monster/033 | complete | complete | 0 | 0 | false |
| WoomaTaurus | Monster/034 | complete | complete | 0 | 0 | false |
| CursedPriest | Monster/069 | complete | complete | 0 | 0 | false |
| CursedZombie | Monster/070 | complete | complete | 0 | 0 | false |
| HungryZombie | Monster/071 | complete | complete | 0 | 0 | false |
| ShiZombie | Monster/070 | complete | complete | 0 | 0 | false |
| RootSpider | Monster/051 | complete | complete | 0 | 110 | false |
| SpiderBat | Monster/052 | complete | complete | 0 | 0 | false |
| VenomSpider | Monster/053 | complete | complete | 0 | 0 | false |
| GangSpider | Monster/054 | complete | complete | 0 | 0 | false |
| GreatSpider | Monster/055 | complete | complete | 0 | 0 | false |
| LureSpider | Monster/056 | complete | complete | 0 | 0 | false |
| BigApe | Monster/057 | complete | complete | 0 | 0 | false |
| BigApe3 | Monster/057 | complete | complete | 0 | 0 | false |
| EvilApe | Monster/058 | complete | complete | 0 | 0 | false |
| EvilApe0 | Monster/058 | complete | complete | 0 | 0 | false |
| GreyEvilApe | Monster/059 | complete | complete | 0 | 0 | false |
| RedEvilApe | Monster/060 | complete | complete | 0 | 0 | false |
| CrystalSpider | Monster/061 | complete | complete | 0 | 0 | false |
| RedMoonEvil | Monster/062 | complete | complete | 0 | 0 | false |
| EvilApeSobo | Monster/058 | complete | complete | 0 | 0 | false |
| EvilBigApe | Monster/058 | complete | complete | 0 | 0 | false |
| ToxicGhoul | Monster/088 | complete | complete | 0 | 0 | false |
| RoninGhoul | Monster/087 | complete | complete | 0 | 0 | false |
| BoneArcher | Monster/092 | complete | complete | 0 | 0 | false |
| BoneBlademan | Monster/091 | complete | complete | 0 | 0 | false |
| BoneSpearman | Monster/090 | complete | complete | 0 | 0 | false |
| Minotaur | Monster/094 | complete | complete | 0 | 0 | false |
| IceMinotaur | Monster/095 | complete | complete | 0 | 0 | false |
| WindMinotaur | Monster/097 | complete | complete | 0 | 0 | false |
| FireMinotaur | Monster/098 | complete | complete | 0 | 0 | false |
| RightGuard | Monster/099 | complete | complete | 0 | 0 | false |
| LeftGuard | Monster/100 | complete | complete | 0 | 0 | false |
| GhastlyLeecher | Monster/152 | complete | complete | 0 | 0 | false |
| MutatedManworm | Monster/154 | complete | complete | 0 | 0 | false |
| CrazyManworm | Monster/155 | complete | complete | 0 | 0 | false |
| CyanoGhast | Monster/153 | complete | complete | 0 | 0 | false |
| DreamDevourer | Monster/163 | complete | complete | 0 | 0 | false |
| BloodyLureSpider | Monster/056 | complete | complete | 0 | 0 | false |
| ChainGhoul | Monster/074 | complete | complete | 0 | 0 | false |
| ArcherGuard3 | Monster/378 | complete | complete | 0 | 0 | false |
| SabukGate | Gate/00 | complete | complete | 7 | 0 | false |
| PalaceWallLeft | Gate/01 | complete | complete | 0 | 0 | false |
| PalaceWall1 | Gate/02 | complete | complete | 0 | 0 | false |
| PalaceWall2 | Gate/03 | complete | complete | 0 | 0 | false |

## Newcomer V2 targets plus BoneFamiliar

| Monster | Library | Public drawable | Native drawable | Empty action frames | Out-of-range | Unknown |
|---|---|---|---|---:|---:|---|
| BoneFamiliar | Monster/078 | complete | complete | 0 | 0 | false |
| Scarecrow | Monster/005 | complete | complete | 0 | 0 | false |
| RakingCat | Monster/007 | complete | complete | 0 | 0 | false |
| Oma | Monster/009 | complete | complete | 0 | 0 | false |
| ForestYeti | Monster/011 | complete | complete | 0 | 0 | false |
| Skeleton | Monster/022 | complete | complete | 0 | 0 | false |
| Zombie2 | Monster/069 | complete | complete | 0 | 0 | false |
| Zombie3 | Monster/070 | complete | complete | 0 | 0 | false |
| Dung | Monster/027 | complete | complete | 0 | 0 | false |
| WoomaSoldier | Monster/029 | complete | complete | 0 | 0 | false |
| WoomaFighter | Monster/030 | complete | complete | 0 | 0 | false |

## Non-profile mapping follow-up

Runtime now maps profile images 950..953 to Gate/00..03. Non-profile pending: Crystal maps 900 and 902 to Dragon (902 has direction-specific statue frames), 903..905 to Monster/247 with special HellBomb frames (904 is not Dragon), 901 has no independent body, siege 940..944 and gates 954..964 use their own libraries, and 10000..10014 use Pet/00..14. Unknown/mismatched paths remain pending and do not establish missing original artwork.

## Descriptor exceptions

| Monster | Library | Public drawable | Native drawable | Empty action frames | Out-of-range | Unknown |
|---|---|---|---|---:|---:|---|
| Sheep | Monster/103 | complete | complete | 0 | 7 | false |
| RootSpider | Monster/051 | complete | complete | 0 | 110 | false |

## Exact source exceptions

- Monster/051: source empty action indices []; descriptor out-of-range indices [66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175].
- Monster/103: source empty action indices []; descriptor out-of-range indices [225, 226, 227, 228, 229, 230, 231].
- Gate/00: source empty action indices [40, 41, 42, 50, 51, 52, 53]; descriptor out-of-range indices [].

These indices must not be repaired by inventing source artwork. Original action/state selection and permitted directions need separate runtime comparison, especially RootSpider and Sheep.
