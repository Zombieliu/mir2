# Daily and weekly commissions v1

This is additive game content, not a claim of original Crystal quest parity.
The configuration is [daily-weekly-v1.json](../config/quest-guidance/daily-weekly-v1.json).
It defines two Task Stewards and 20 quests for the current `platinum_176`
level 10–50 content. It must work alongside `newcomer-v2` without changing
the accepted main quest chain or existing imported quests.

On 2026-10-03 the user replaced the initial workload/reward brief: reduce
every kill target to one tenth, rounded up, and award about five levels for
the complete daily set and ten levels for the complete weekly set. Gold,
species, maps, routes and reset rules remain unchanged. Natural play time
under these new quantities has not been measured; the original 30–60 minute
target and its historical results are retained in the goal/evidence records.

## Player and reward rules

- Both stewards offer the same quest IDs and share one character's progress
  and claim history. Accepting in Bichon and handing in in Mongchon is valid.
- Each level band has three daily slots and two weekly slots. The claim key
  includes cadence and slot across **all** bands and both cities. A level-up
  or change of city cannot grant a second reward for a slot in the same period.
- Daily reset is 00:00 UTC+8. Weekly reset is Monday 00:00 UTC+8. Server wall
  time decides the period; client time and packet movement ticks do not.
- In-progress and ready quests survive a reset. A successful hand-in consumes
  the period in which the hand-in commits, not the acceptance period.
- Accepting locks that quest's band, absolute EXP and gold. A later
  level-up, reset or EXP-buff expiry cannot change an active quest's reward.
  Abandoning forfeits unclaimed progress and permits a fresh acceptance subject
  to the same slot limit. A new period clears a completed quest for another
  acceptance; its previous claim watermark is retained.
- This content update reduces active saved targets to the current catalog
  while retaining earned species counters and the original locked reward.
  Old completed rows display the current offer after reset and refresh their
  descriptions and required count when reaccepted. No old claim is repaid.
- Rewards contain only EXP and gold. There are no item rewards, random drops,
  purchase requirements, profession-only practice objectives or boss kills.
- A credited player or player-owned pet kill may advance an accepted daily and
  an accepted weekly objective together. Unaccepted quests, unrelated NPC
  kills, another player's private kill and the correct species in an unlisted
  map do not advance progress. Party credit follows the existing authoritative
  shared kill award, rather than an additional client-side counter.
- Reward, EXP-induced level/stat changes and claim metadata must commit in one
  durable full-character save before a success packet is returned. Duplicate
  Finish, reconnect and stale sessions must not produce an additional award.

For accepted level `L`, slot `s`, and configured `rewardLevelSpan`:

```text
N = 5 for daily, 10 for weekly
K = 3 daily slots, 2 weekly slots
budget = sum(Crystal_ExperienceList[level - 1] for level in L..L+N)
base_exp = floor(budget * (s + 1) / K) - floor(budget * s / K)
locked_exp = apply_current_social_and_general_exp_rate(base_exp)
locked_gold = configured_gold
```

Use checked u64 arithmetic for the full budget and prefix shares before
converting each quest's reward to the protocol's u32 representation. The
monster EXP multipliers are not applied to this reward. The runtime locks
the final EXP after the accepted
social/general rate, displays that absolute value in quest details and pays
it without applying a second rate at hand-in. Capturing that value is a
server action, not a value submitted by the client.

If the complete set is accepted at the same level without an EXP bonus,
its total exactly covers the consecutive five or ten level thresholds;
integer remainders are retained across the slots. Kills grant extra EXP.
Quests accepted at different levels retain their respective acceptance
values, so the actual level gain varies with that timing and EXP buffs.
At level 30 daily rewards are 4,666,666 / 4,666,667 / 4,666,667 EXP (14 million
total); weekly rewards are 22,800,000 each (45.6 million total). At level 50
the daily total is 2.43 billion and weekly total 7.16 billion; each weekly
share is 3.58 billion and fits u32. Existing social/general rate saturation
at u32::MAX is preserved per quest, rather than overflowing.

The current profile thresholds include level 10: 6,000; 15: 40,000; 20:
140,000; 25: 500,000; 30: 2,000,000; 35: 4,000,000; 40: 12,000,000; 45:
120,000,000; 50: 350,000,000. This large variation is why reward EXP is locked
from the acceptance level instead of using one fixed amount for a broad band.
Level 35–50 is the final configured band; new acceptance above level 50 is
not part of this version. Already accepted quests may be finished after
leveling above that band. The engine's Crystal progression curve continues
beyond the content profile's 50-level table and is used for these rewards.

## Stewards and verified placement

| Steward | Object ID | Map file / index | Position | NPC image |
| --- | --- | --- | --- | --- |
| Bichon Task Steward | 2180 | `0` / 1, BichonProvince | 335,266 | 27 |
| Mongchon Task Steward | 2181 | `3` / 172, MongchonProvince | 334,330 | 16 |

These positions are inside the existing imported safe zones at 328,264
(size 10) and 330,330 (size 10). Static collision/door checks find both NPC
tiles and all eight immediately adjacent tiles open. No imported NPC occupies
either chosen tile. Bichon 335,265 is deliberately avoided because the
existing GTMerchant_Jamie occupies it. This does not assert that dynamic
players or monsters will never temporarily block an approach tile.

The imported NPC manifest's maximum loaded object ID is 2163. Code, fixtures
and entity content do not reserve 2180/2181; equally numbered effect/image
frames are a different namespace. Imported quest IDs are 1–154, and the
assigned quest range 92001–92020 does not replace them. Crystal respawn
objects start from a 200,000 base and shared dynamic objects start from
1,000,000.

Image 27 is already used by Examiner_Tony, and image 16 by ordinary merchants
and MongchonDelegate_Michael. The exported `NPC/27` and `NPC/16` metadata and
frame-set catalogs contain Standing actions. Their PNG frames exist and have
nonzero dimensions; visual inspection confirms distinct character sprites.
They reuse existing sprite libraries instead of adding unverified art.

## Quest configuration

The map list is checked for **each kill target**, not merely against the
union of a quest's maps. The first map is the preferred navigation map.
Coordinates identify a hunting guide point; kills anywhere in a listed map
count. Guidance coordinates are not an artificial small kill-radius gate.

| Levels | IDs | Daily slots 0 / 1 / 2 | Weekly slots 0 / 1 | Daily / weekly gold |
| --- | --- | --- | --- | --- |
| 10–14 | 92001–92005 | Scarecrow 8 / RakingCat 6 / HookingCat 6, `0` | Scarecrow 20 / RakingCat 16 + HookingCat 16, `0` | 12,000 total / 25,000 each |
| 15–24 | 92006–92010 | Oma 7, `0` / Skeleton 6 / Scorpion 6, `D001` or `D011` | Oma 20, `0` / Skeleton 15 + Scorpion 15, `D001` or `D011` | 24,000 total / 60,000 each |
| 25–34 | 92011–92015 | Centipede 5 / BlackMaggot 5 / WhimperingBee 5, `D601` | Centipede 15 + BlackMaggot 10 / WhimperingBee 12 + GiantWorm 12, `D601` | 60,000 total / 150,000 each |
| 35–50 | 92016–92020 | RedBoar 6 / BlackBoar 5, `D711` / GiantRat 3, `D501` | RedBoar 18 + BlackBoar 15, `D711` / GiantRat 10 + WedgeMoth 12, `D501` | 102,000 total / 250,000 each |

The complete three-quest daily sets require 20, 19, 15 and 14 credited kills respectively
for the four bands. These are workload counts, not measured durations. Weekly
progress overlaps the corresponding daily hunts where species/maps match,
so players do not need to redo identical kills just for the weekly counter.

## Imported monster and spawn evidence

The table is based on the tracked Crystal monster/respawn manifests, with
profile whitelist checks. Totals are sums of imported spawn-group `count`,
not an observation of simultaneously living monsters. Spread, collision,
already killed mobs and other players affect live availability.

| Species / index | Level / HP | Listed map | Imported total | First selected group and refresh |
| --- | --- | --- | --- | --- |
| Scarecrow / 39 | 10 / 20 | `0` | 190 | 200,400; 20 mobs, spread 50, 6 min |
| RakingCat / 44 | 13 / 32 | `0` | 110 | 340,550; 40 mobs, spread 50, 6 min |
| HookingCat / 42 | 13 / 30 | `0` | 110 | 340,550; 40 mobs, spread 50, 6 min |
| Oma / 48 | 15 / 30 | `0` | 210 | 420,95; 20 mobs, spread 50, 3 min |
| Skeleton / 85 | 18 / 90 | `D001` / `D011` | 90 / 60 | 200,200; 80 / 60 mobs, spread 200, 4 / 3 min |
| Scorpion / 84 | 18 / 45 | `D001` / `D011` | 60 / 50 | 200,200; 50 / 50 mobs, spread 200, 4 / 3 min |
| Centipede / 152 | 26 / 210 | `D601` | 82 | 40,27; 12 mobs, spread 80, 2 min |
| BlackMaggot / 150 | 28 / 200 | `D601` | 24 | 40,27; 4 mobs, spread 80, 2 min |
| WhimperingBee / 156 | 26 / 180 | `D601` | 36 | 40,27; 12 mobs, spread 80, 2 min |
| GiantWorm / 154 | 26 / 180 | `D601` | 36 | 84,104; 12 mobs, spread 80, 2 min |
| RedBoar / 168 | 32 / 300 | `D711` | 126 | 251,41; 7 mobs, spread 30, 8 min |
| BlackBoar / 171 | 35 / 280 | `D711` | 90 | 251,41; 5 mobs, spread 30, 8 min |
| GiantRat / 192 | 34 / 1000 | `D501` | 100 | 200,200; 20 mobs, spread 100, 3 min |
| WedgeMoth / 165 | 32 / 220 | `D501` | 100 | 200,200; 20 mobs, spread 100, 5 min |

The packed map's D001 spawn center 200,200 is blocked. Both Skeleton and
Scorpion guidance therefore uses 199,199, a walkable tile in **both** D001
and D011. This preserves the actual spawn pool while avoiding an unreachable
exact guide tile. The other configured guide points are open in the bundled
map collision data. A static open tile is not proof of a complete live
route; gateway navigation tests remain required.

Relevant ordinary routes include Bichon `0` 147,33 → OmaCave_1F `D001`
151,362; Bichon `0` 43,111 → NaturalCave `D011` 154,368; Mongchon `3`
139,85 → S_1FofDungeon `D601` 207,236; Mongchon `3` 305,324 →
AngledStoneTombEntrance `D710` 28,21 → `D711` 205,198. The guidance must use
the map route graph rather than attempting to walk in the current map to
another map's numeric coordinates.

No target requires a boss. D022 is avoided because `newcomer-v2` deliberately
reduces each imported D022 group to one mob. ZumaArcher is also avoided for
the daily pool: D501/D502 each import only ten archers with a ten-minute
refresh. The listed profile spawn overrides repair D421/D422 monsters and
do not change these selected pools.

## Supply budget and class pacing

Imported base shop prices are Small HP/MP 40; Medium HP/MP 110; Large HP/MP
225; XL HP/MP 250; TownTeleport 1,000; RandomTeleport 100; Amulet 25;
GreenPoison/RedPoison 40. Price-rate 1 is recorded for Bichon
Alchemist_Samuel at 324,291 and Mongchon Merchant_Daniel at 361,335.
Actual supplies are bought through their ordinary shop UI, not granted as
commission rewards.

| Levels | Planning basket at price-rate 1 | Basket cost | Daily gold |
| --- | --- | --- | --- |
| 10–14 | 20 Small HP + 20 Small MP + 1 Town + 5 Random | 3,100 | 12,000 |
| 15–24 | 25 Medium HP + 25 Medium MP + 1 Town + 5 Random + 20 Amulet + 20 Poison | 8,300 | 24,000 |
| 25–34 | 40 Large HP + 40 Large MP + 2 Town + 10 Random + 100 Amulet + 40 Poison | 25,100 | 60,000 |
| 35–50 | 60 XL HP + 60 XL MP + 2 Town + 10 Random + 150 Amulet + 80 Poison | 39,950 | 102,000 |

These are budgeting assumptions, not measured consumptions or guaranteed
net profit. Repair charges and replacement gear are not included. The larger
healing/mana PotionSmall and PotionMedium cost 1,100 and 2,200; if normal
high-band play needs those, actual spend can be substantially greater. A
cash reward paid at completion also does not replace the need for initial
supplies before departure. Low-stock guidance must retain the existing
class-aware ordinary merchant route.

Taoists below 18 cannot use SoulFireBall, and below 19 cannot summon Skeleton.
The level 10–14 pool therefore has no poison/amulet/spell practice requirement.
At 25–34, pet kill credit and amulet/poison consumption matter. Wizards gain
FireBang at 22, FireWall at 24 and Lightning at 26, so group hunting can be
much faster than ordinary Warrior melee. Higher-band equipment, armour,
magic resistance and the 35–50 level span also affect time. Daily counts
must be calibrated from all three classes, without weakening monster stats
or changing combat cooldowns to make a timing target pass.

## Required acceptance matrix

| Area | Required check |
| --- | --- |
| Content | Exact 20 unique IDs, four nonoverlapping bands, three daily/two weekly slots per band; all species/index/map pairs exist, are whitelisted and have imported spawns; safe NPC tiles, no old NPC overwrite, existing Standing sprite frames. |
| Default gateway | New NPCs are visible and callable in public shared zones for maps 0 and 3 under `newcomer-v2`; a native client can accept and finish through ordinary NPC packets. Private session spawning alone is insufficient. |
| Shared cities | Accept at either steward, move normally, complete at the other; IDs, progress and claim count remain the same. NPC data-range and exact active-dialog validation still apply. |
| Bands and slots | Boundary levels 9/10/14/15/24/25/34/35/50/51; active old-band quests survive a level-up; one accepted quest per cadence/slot; a same-period claim blocks all other bands/cities for that slot. |
| Combat | Correct species and map count once, wrong-map same-species does not; daily/weekly overlap advances both; Warrior, Wizard and Taoist-owned pet authoritative kills count; unrelated kills and duplicate shared kill envelopes do not. |
| Rewards | Locked absolute EXP/gold shown after acceptance, reconnect and hand-in; accept-level and active EXP-buff changes cannot silently alter the reward; no item grant or bag-space requirement. |
| Recurrence | UTC+8 daily boundary and Sunday→Monday boundary; in-progress/ready survives; claim charges hand-in period; Completed reopens once next period; clock rollback/future watermark does not reopen early; legacy metadata migration remains conservative. |
| Durability | Non-guild and guild success persist before ACK; duplicate Finish, retry at other steward, stale CAS, save failure and unknown publish outcome do not expose another success/reward; process restart/relogin retains the one committed claim. |
| UI and navigation | Correct current map, hunting point and return steward; static blocked-center adjustment; map transfer/obstacle recovery; original shop/bag/input layouts remain usable; localized daily/weekly text and locked numeric rewards fit. |
| Natural pacing | Four representative band levels × three classes through the default playtest gateway, with ordinary gear/learned skills and without injected kill/progress/EXP commands. Measure whole daily set duration, route time, combat/loot time, resupply, deaths, mana/HP/amulet/poison usage and real gold spend. Record low/high band boundary timing follow-ups where representative runs reveal large differences. |

The natural pacing target is the whole three-quest daily set in 30–60 minutes.
Weekly mechanics can be checked independently, but seeded ready-state reward
tests do not prove a naturally completable weekly hunt. Each report must state
its gear, starting level, active rate, class and whether its timing is fresh.

## Source references and current evidence scope

- `packages/game-data/data/generated/crystal_monster_manifest.json`: species
  indices, combat levels, HP and base kill EXP.
- `packages/game-data/data/generated/crystal_respawn_manifest.json`: map
  identities, safe zones, movement edges and grouped respawns.
- `packages/game-data/data/generated/crystal_npc_info_manifest.json`: NPC
  object reservations, placements, existing images and merchant price rates.
- `packages/game-data/data/generated/crystal_item_manifest.json`: supply prices.
- `packages/game-data/data/content_profiles/platinum_176.json`: classes,
  map/monster whitelists, EXP curve and respawn overrides.
- `apps/web/lib/generated/crystal-map-pack/*.map.gz`,
  `packages/game-data/data/generated/crystal_starter_map_collision.json`, and
  `apps/simulation/src/runtime/map.rs`: offline collision checks using the same
  v1/v100 wall/door bits and the runtime starter-map collision source.
- `apps/web/public/original-ui/NPC/27/meta.json`, `NPC/16/meta.json` and
  `frame-sets.generated.json`: existing images and Standing actions.
- `apps/simulation/src/runtime/quest_recurrence.rs`: existing UTC+8 period and
  active-progress preservation semantics; the new IDs need additive binding.
- `apps/simulation/src/runtime/quests.rs`, `npc_script.rs`, `save.rs` and the
  shared gateway paths: runtime integration and atomic hand-in requirements.

The 2026-10-02 offline content validation passed: 20 quest IDs, 26 kill
objectives, 16 distinct allowed species/map pairs with existing spawns, nine
distinct open hunting points, two collision/occupancy-safe steward positions,
and 36 existing Standing PNG frame files (four for image 27 and 32 for image
16 across eight directions). JSON keys, band/slot ordering, reward percentages
and gold totals match the specified schema. The first two NPC sprite PNGs
also have different hashes and were visually inspected.

This content revision is validated against tracked static data and sprite
files. It does not by itself establish runtime persistence, native GUI
acceptance, multiplayer spawn availability, localized UI acceptance or the
30–60 minute pacing target. Runtime and gateway evidence must be recorded
separately after integration. No human character save is changed by this
content work.
