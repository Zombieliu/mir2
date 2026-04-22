# Crystal NPC Command Summary

Generated at: 2026-04-21T18:11:19.738Z

Source scripts: 634

Runtime command coverage: 81/81 command names implemented, 7044/7044 command occurrences covered by the current Rust baseline.

## Unimplemented Commands

No unimplemented command names found.

## Implemented Commands

| Kind | Command | Count | Scripts | Status | Example |
| --- | --- | ---: | ---: | --- | --- |
| action | `GOTO` | 1014 | 350 | implemented | AncientCaves/AncientNatural-D003:10 |
| condition | `LEVEL` | 801 | 26 | implemented | AncientCaves/AncientNatural-D003:28 |
| action | `MOVE` | 670 | 141 | implemented | AncientCaves/AncientNatural-D003:31 |
| action | `SET` | 394 | 47 | implemented | AncientCaves/AncientNatural-D003:19 |
| action | `GIVEPET` | 384 | 8 | implemented | GuildTerritory/GA0/GTPetNPC:51 |
| condition | `CHECKGOLD` | 352 | 33 | implemented | BichonProvince/BichonWall/Bichon_Teleport1:36 |
| action | `TAKEGOLD` | 346 | 32 | implemented | BichonProvince/BichonWall/Bichon_Teleport1:39 |
| condition | `CHECKITEM` | 321 | 38 | implemented | AncientCaves/AncientPrajna-D2074:27 |
| action | `TAKEITEM` | 309 | 35 | implemented | AncientCaves/AncientPrajna-D2074:29 |
| condition | `CHECKPKPOINT` | 294 | 294 | implemented | BichonProvince/BichonWall/14Wr-0:3 |
| action | `GIVEITEM` | 201 | 18 | implemented | BichonProvince/BichonWall/14Wr-0:59 |
| condition | `CHECK` | 198 | 36 | implemented | AncientCaves/AncientNatural-D003:17 |
| action | `GIVESKILL` | 183 | 2 | implemented | GM/GM-Book:27 |
| action | `MONGEN` | 168 | 3 | implemented | BichonProvince/Event/Proceeder30-EM001:319 |
| condition | `PETCOUNT` | 168 | 8 | implemented | GuildTerritory/GA0/GTPetNPC:19 |
| action | `CLEARPETS` | 129 | 9 | implemented | GM/GM-Manager:44 |
| condition | `CHECKPET` | 128 | 8 | implemented | GuildTerritory/GA0/GTPetNPC:575 |
| condition | `PETLEVEL` | 128 | 8 | implemented | GuildTerritory/GA0/GTPetNPC:576 |
| action | `LOCALMESSAGE` | 98 | 31 | implemented | AncientCaves/AncientStone-D715:6 |
| action | `BREAK` | 61 | 30 | implemented | BichonProvince/BichonWall/Hairdresser:32 |
| action | `PARAM1` | 60 | 3 | implemented | BichonProvince/Event/Proceeder30-EM001:316 |
| action | `PARAM2` | 60 | 3 | implemented | BichonProvince/Event/Proceeder30-EM001:317 |
| action | `PARAM3` | 60 | 3 | implemented | BichonProvince/Event/Proceeder30-EM001:318 |
| condition | `CHECKMON` | 60 | 3 | implemented | BichonProvince/Event/Proceeder30-EM001:324 |
| action | `MOV` | 42 | 14 | implemented | BichonProvince/BichonWall/Lottery-0:20 |
| action | `CLOSE` | 40 | 21 | implemented | MongchonProvince/14Qas-D604:11 |
| action | `CONQUESTGUARD` | 36 | 2 | implemented | GM/GM-Manager:185 |
| condition | `RANDOM` | 36 | 6 | implemented | BichonProvince/BichonWall/Lottery-0:18 |
| action | `GIVEGOLD` | 32 | 16 | implemented | BichonProvince/BichonWall/Lottery-0:134 |
| condition | `CHECKQUEST` | 32 | 30 | implemented | AncientCaves/AncientNatural-D003:4 |
| condition | `ISADMIN` | 24 | 24 | implemented | GM/GM-Armour:2 |
| action | `LINEMESSAGE` | 22 | 3 | implemented | GM/GM-Manager:45 |
| condition | `AFFORDGUARD` | 12 | 1 | implemented | MongchonProvince/SabukWall/Conquest:197 |
| condition | `CHECKCLASS` | 12 | 9 | implemented | GuildTerritory/GA0/GTPetNPC:2 |
| action | `EXTENDGT` | 10 | 10 | implemented | GuildTerritory/GA0/GTAdmin-GA10:33 |
| action | `GTALLRECALL` | 10 | 10 | implemented | GuildTerritory/GA0/GTAdmin-GA10:38 |
| action | `GTRECALL` | 10 | 10 | implemented | GuildTerritory/GA0/GTAdmin-GA10:46 |
| condition | `GENDER` | 10 | 2 | implemented | WasteLand/20Qah-NAMMAND_1:39 |
| condition | `HASGT` | 10 | 10 | implemented | GuildTerritory/GA0/GTAdmin-GA10:31 |
| action | `CONQUESTWALL` | 9 | 2 | implemented | GM/GM-Manager:158 |
| condition | `CONQUESTOWNER` | 7 | 1 | implemented | MongchonProvince/SabukWall/Conquest:2 |
| action | `CHANGEHAIR` | 6 | 1 | implemented | BichonProvince/BichonWall/Hairdresser:31 |
| action | `ADDNAMELIST` | 5 | 3 | implemented | BichonProvince/BichonWall/Board:121 |
| condition | `CHECKPERMISSION` | 5 | 1 | implemented | MongchonProvince/SabukWall/Conquest:29 |
| action | `GLOBALMESSAGE` | 4 | 1 | implemented | MongchonProvince/SabukWall/Conquest:49 |
| action | `SETCONQUESTRATE` | 4 | 1 | implemented | MongchonProvince/SabukWall/Conquest:48 |
| condition | `CHECKCALC` | 4 | 4 | implemented | BichonProvince/BichonWall/Lottery-0:119 |
| condition | `DAYOFWEEK` | 4 | 2 | implemented | SerpentValley/ConnectionPath/MysteriousYu:10 |
| condition | `HOUR` | 4 | 2 | implemented | SerpentValley/ConnectionPath/MysteriousYu:11 |
| condition | `MIN` | 4 | 2 | implemented | SerpentValley/ConnectionPath/MysteriousYu:12 |
| action | `BREAKTIMERECALL` | 3 | 3 | implemented | BichonProvince/Event/Proceeder30-EM001:908 |
| action | `ELSESAY` | 3 | 1 | implemented | Test:11 |
| action | `MONCLEAR` | 3 | 3 | implemented | BichonProvince/Event/MissDo-EM000:61 |
| action | `REVIVEHERO` | 3 | 3 | implemented | BichonProvince/BichonWall/Board:78 |
| action | `SEALHERO` | 3 | 3 | implemented | BichonProvince/BichonWall/Board:82 |
| action | `TIMERECALL` | 3 | 3 | implemented | BichonProvince/Event/MissDo-EM000:62 |
| condition | `AFFORDWALL` | 3 | 1 | implemented | MongchonProvince/SabukWall/Conquest:135 |
| condition | `CHECKHUM` | 3 | 3 | implemented | BichonProvince/Event/MissDo-EM000:17 |
| action | `ADDTOGUILD` | 2 | 2 | implemented | BichonProvince/BorderVillage/BountyBoard-0:28 |
| action | `BUYGT` | 2 | 2 | implemented | BichonProvince/BichonWall/GTMerchant-0:36 |
| action | `CHANGELEVEL` | 2 | 2 | implemented | GM/GM-Manager:28 |
| action | `CLOSEGATE` | 2 | 2 | implemented | GM/GM-Manager:153 |
| action | `CONQUESTGATE` | 2 | 2 | implemented | GM/GM-Manager:180 |
| action | `DELNAMELIST` | 2 | 1 | implemented | BichonProvince/BichonWall/Board:153 |
| action | `GIVEBUFF` | 2 | 2 | implemented | BichonProvince/BichonWall/Luke-0:39 |
| action | `GROUPGOTO` | 2 | 2 | implemented | MongchonProvince/FoxCave/Rock:4 |
| action | `GROUPTELEPORT` | 2 | 2 | implemented | MongchonProvince/FoxCave/Rock:41 |
| action | `OPENGATE` | 2 | 2 | implemented | GM/GM-Manager:148 |
| condition | `CHECKBUFF` | 2 | 2 | implemented | BichonProvince/BorderVillage/Prison-0127:2 |
| condition | `CHECKMAP` | 2 | 2 | implemented | MongchonProvince/FoxCave/Rock:13 |
| condition | `CHECKRANGE` | 2 | 2 | implemented | MongchonProvince/FoxCave/Rock:14 |
| condition | `GROUPCOUNT` | 2 | 2 | implemented | MongchonProvince/FoxCave/Rock:12 |
| condition | `GROUPLEADER` | 2 | 2 | implemented | MongchonProvince/FoxCave/Rock:2 |
| condition | `INGUILD` | 2 | 2 | implemented | BichonProvince/BorderVillage/BountyBoard-0:22 |
| action | `CHECKITEM` | 1 | 1 | implemented | GM/GM-Manager:35 |
| action | `CONQUESTREPAIRALL` | 1 | 1 | implemented | GM/GM-Manager:261 |
| action | `STARTCONQUEST` | 1 | 1 | implemented | GM/GM-Manager:105 |
| action | `TAKECONQUESTGOLD` | 1 | 1 | implemented | MongchonProvince/SabukWall/Conquest:32 |
| action | `TELEPORTGT` | 1 | 1 | implemented | BichonProvince/BichonWall/GTMerchant:51 |
| condition | `AFFORDGATE` | 1 | 1 | implemented | MongchonProvince/SabukWall/Conquest:126 |
| condition | `CHECKEXACTMON` | 1 | 1 | implemented | WhiteValley/SnowMonument:11 |
