# Class acceptance and supplies — 2026-09-24

## Acceptance scope

The player reports: "OK-0-30级我战士都验收完毕了。法师道士怎么办?中途药品道具什么的怎么版？"

Warrior 0–30 gameplay is accepted by the player. The current deployed candidate
is `abee09b21` (`20260924-practice-combat`). This is a player-reported journey
acceptance, not a new measured two-hour run or a replacement of the historical
cohort ledgers. Wizard and Taoist 0–30 gameplay remain unaccepted. Whole-game
stability, all-map performance and original Crystal visual parity remain separate.

## Current supply contract, read from source

`config/quest-guidance/newcomer-journey-v2.json` contains 22 main quests and
four growth claims per class. Summing their authored reward counts gives:

| Reward | Warrior | Wizard | Taoist |
| --- | ---: | ---: | ---: |
| Small HP medicine | 60 | 60 | 60 |
| Small MP medicine | 0 | 100 | 80 |
| TownTeleport | 6 | 6 | 6 |
| RandomTeleport | 4 | 4 | 4 |
| Amulet | 0 | 0 | 300 |
| GreenPoison / RedPoison | 0 / 0 | 0 / 0 | 50 / 50 |

These are cumulative configured counts across completion, not starting stock,
measured consumption or a guarantee of survival. Amulet and poison templates
have zero durability and stack limits of 500; consumption decreases quantity.
One configured reward unit is not a package of 500 uses.
The main quests award 25,000 gold and growth claims award 55,000 gold;
25,000 of that growth gold arrives only at level 30 after the final report.
It cannot finance an earlier expedition. Income must be checked at each
departure, including purchase prices, repairs, bag space and weight.

N13 (Dead Mine entry) and N17 (expedition preparation) still award small HP/MP
medicine. One-time quest rewards therefore do not establish a sustainable
late-journey supply loop.

Ordinary shop source data at rate 1 lists the following. This is a catalog/
script audit, not a new live purchase receipt:

| NPC | Location | Supply and unit price (gold) |
| --- | --- | --- |
| Alchemist Samuel | Bichon map `0`, (324,291) | Small/medium/large HP or MP medicine: 40/110/225 per bottle |
| Merchant Bull | Bichon map `0`, (374,296) | TownTeleport 1,000; RandomTeleport 100; Amulet 25 |
| Specialist Travis | Indoor map `0109`, (4,9) | GreenPoison and RedPoison: 40 per unit |

Samuel's script offers 1/5/10-bottle rows; unit prices must not be mistaken
for package prices. Current `platinum_176` admits Travis's map, Potion3 script
and materials; his NPC record has no level/class/time/flag gate. The ordinary
indoor entrance route and live purchase were not verified in this audit.

Source references (repository-relative):

- `packages/game-data/data/generated/crystal_npc_info_manifest.json:635`
  (Samuel), `:701` (Bull), `:1657` (Travis).
- `packages/game-data/data/generated/crystal_npc_manifest.json:22188`
  (Samuel goods), `:11465` (Bull goods), `:22555` (Travis goods).
- `packages/game-data/data/generated/crystal_item_manifest.json:32614`
  (small medicine), `:32692` (medium), `:32770` (large), `:34661`
  (green poison), `:34695` (red poison), `:34729` (Amulet).
- `packages/game-data/data/content_profiles/platinum_176.json:118` (map),
  `:701` (materials), `:827` (Potion3 script).
- `apps/simulation/src/runtime/npc.rs:1116` computes template unit price
  times quantity times NPC rate. `skills.rs:7577` checks material shape
  and consumes quantity; SoulFireBall, Poisoning and SummonSkeleton each
  require one eligible material per use. Amulet requires level 18;
  red/green poison require level 14.

## Gaps to close before caster acceptance

- `quest_practice.rs::supply_instructions` only appears for main quests at
  minimum level 16 or higher. It receives no class and lists only Samuel's
  medicine and Bull's scroll shops. Taoist material vendors and class-specific
  departure requirements are absent from this guidance.
- Existing class-practice instructions correctly name spells and distinguish
  equipping an Amulet from equipping poison, but do not show an inventory-aware
  resupply destination or departure checklist.
- The V2 runner's `ensureReady` in `run-protocol-journey.mjs` checks HP stock,
  conditional Amulets and Wizard escape scrolls. Its early ready result and
  post-purchase checks do not require caster MP medicine or poison stock.
  A test run can therefore begin under-supplied even though the general
  `restockInVillage` helper supports separate MP thresholds.
- N20/N21 have a specific ordinary-shop Amulet preflight. That bounded rule
  does not verify the complete Wizard/Taoist budget or every material.
- Bull's old question/answer text says TownTeleport only drops from monsters,
  while his Trade list sells it. Correct the contradictory source-derived
  player guidance when implementing the supply flow.

## Next class acceptance sequence

1. Complete the class-aware supply checks and player guidance, then use two
   independent fresh ordinary characters, one Wizard and one Taoist. Keep
   the player's accepted Warrior save intact.
2. Track each class through all 22 main quests and four growth claims to
   level 30, with actual learned skills, eligible equipment and normal
   purchases. Collect per-chapter medicine/material/scroll consumption,
   gold spent/remaining, deaths and time. Use ordinary return routes and
   legal scrolls when returning to restock.
3. Wizard gates: FireBall, GreatFireBall, legal reposition after spell damage,
   FireWall damage and Lightning; sustain MP while keeping enough HP and
   transport supplies for the return trip.
4. Taoist gates: self-Healing, SpiritSword, SoulFireBall with real material
   consumption, an owned skeleton causing damage, and Poisoning; validate
   switching equipped materials and restocking both types during the route.
5. Include one mid-route restock and save/relogin check per class. A route
   that needs an external item/gold grant to continue fails the normal
   supply acceptance. Reaching level 30 alone does not pass the class gates.
6. After functional and resource-flow verification, obtain the player's
   Wizard/Taoist native gameplay acceptance. Existing failed cohort clocks
   and death ledgers remain unchanged and separate from a new run.

This checkpoint records the human Warrior result and a source audit only.
No new caster playthrough, purchase, live-store edit, client replacement or
balance change was performed for this checkpoint.
