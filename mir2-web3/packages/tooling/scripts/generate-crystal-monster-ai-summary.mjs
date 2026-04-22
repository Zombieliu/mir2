import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const monsterManifestPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_monster_manifest.json",
);
const respawnManifestPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const crystalMonsterObjectPath =
  process.argv[2] ??
  resolve(repoRoot, "..", "Crystal", "Server", "MirObjects", "MonsterObject.cs");
const jsonOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_monster_ai_summary.json",
);
const markdownOutputPath = resolve(
  repoRoot,
  "docs",
  "generated",
  "crystal-monster-ai-summary.md",
);

const RUNTIME_COVERAGE = new Map([
  [
    0,
    {
      status: "implemented_special",
      notes: ["Default MonsterObject branch is covered as the imported generic runtime baseline: Crystal template stats, hostile movement/chase, adjacent ObjectAttack melee, delayed struck damage, respawn, drops, and packet visibility are covered; per-family subclasses remain tracked by their own AI entries."],
    },
  ],
  [
    1,
    {
      status: "implemented_special",
      notes: ["Hen/Pig/Bull style HarvestMonster baseline is covered: neutral/passive no-player-target behavior, no normal attacks, and Crystal two-pass harvest skin count."],
    },
  ],
  [
    2,
    {
      status: "implemented_special",
      notes: ["Deer/Sheep passive no-player-target baseline, Crystal five-pass harvest skin count, deterministic run-away mode selection, and flee movement are covered; exact Quality randomization remains pending with item/drop quality parity."],
    },
  ],
  [
    3,
    {
      status: "implemented_special",
      notes: ["Tree-style static object behavior is covered: neutral/passive targeting, no movement/route/chase/patrol, no normal attacks, fixed one-HP damage intake, and Crystal upward-facing runtime spawns."],
    },
  ],
  [
    4,
    {
      status: "implemented_special",
      notes: ["Two-tile line/diagonal attack range and delayed hit timing are covered."],
    },
  ],
  [
    5,
    {
      status: "implemented_special",
      notes: ["Hidden plant spawn, reveal, ObjectShow/ObjectHide, and untargetable hidden state are covered."],
    },
  ],
  [
    6,
    {
      status: "neutral_guard_baseline",
      notes: ["Neutral guard disposition, hostile-monster target selection, target-back melee packet shape, and deer/hen/tree ignore rules are covered."],
    },
  ],
  [
    7,
    {
      status: "implemented_special",
      notes: ["CaveMaggot Crystal melee timing, DC-based damage, 1/20 five-second paralysis status, and shared HarvestMonster two-pass corpse harvest are covered."],
    },
  ],
  [
    9,
    {
      status: "implemented_special",
      notes: ["HarvestMonster corpse-drop semantics are covered: no immediate death drops, Crystal Harvest/ObjectHarvested packets, two skin-count passes, and configured harvest rewards are resolved from the corpse."],
    },
  ],
  [
    10,
    {
      status: "implemented_special",
      notes: ["FlamingWooma Crystal melee baseline is covered: ObjectAttack packet shape, 300 ms delayed hit timing, and imported DC-based damage for the magic-melee branch."],
    },
  ],
  [
    8,
    {
      status: "implemented_special",
      notes: ["Six-tile ranged packet pressure and Crystal fear-window close/kite movement are covered."],
    },
  ],
  [
    11,
    {
      status: "implemented_special",
      notes: ["WoomaTaurus FlamingWooma melee timing/DC damage, HP-threshold mad speed phase, surrounded teleport baseline, and ObjectTeleportOut/ObjectTeleportIn effect packets are covered."],
    },
  ],
  [
    12,
    {
      status: "implemented_special",
      notes: ["BugBagMaggot summon timing, cap, and DB-backed BugBat body import are covered."],
    },
  ],
  [
    13,
    {
      status: "implemented_special",
      notes: ["RedMoonEvil static view-range ObjectAttack baseline is covered with 300 ms delayed imported DC damage, Crystal's multi-target fanout across opposing targets in view range, and ObjectEffect RedMoonEvil broadcasts for each target."],
    },
  ],
  [
    14,
    {
      status: "implemented_special",
      notes: ["EvilCentipede hidden reveal/hide baseline and static view-range ObjectAttack branch are covered with 500 ms delayed imported DC damage, seven-tile multi-target fanout, and Crystal green/paralysis poison rolls on successful player hit."],
    },
  ],
  [
    15,
    {
      status: "implemented_special",
      notes: ["Stoned Zuma extra state, wake, wake propagation, and respawn state reset are covered."],
    },
  ],
  [
    16,
    {
      status: "implemented_special",
      notes: ["RedThunderZuma stoned wake state, nine-tile range, non-adjacent ObjectRangeAttack, fixed ranged delay, and DC damage with zero-DC no-damage gating are covered."],
    },
  ],
  [
    17,
    {
      status: "implemented_special",
      notes: ["ZumaTaurus stoned wake state and adjacent ObjectAttack melee branch are covered with imported DC damage; HP-stage Zuma slave spawning now uses the Crystal 7-stage tracker, 8-per-wave / 40-slave cap, and configured Zuma minion set."],
    },
  ],
  [
    18,
    {
      status: "implemented_special",
      notes: ["Shinsu target-driven show/hide mode and two-tile pressure are covered."],
    },
  ],
  [
    19,
    {
      status: "implemented_special",
      notes: ["KingScorpion two-tile line/range branch is covered with ObjectRangeAttack, delayed MC damage, Crystal's line-shaped reach, line fanout, adjacent ObjectAttack DC fallback, and adjacent random/second-tile range override."],
    },
  ],
  [
    20,
    {
      status: "implemented_special",
      notes: ["DarkDevil three-tile ObjectRangeAttack area branch is covered with 500 ms delayed imported DC*3 damage, Crystal's 2-4 second area cooldown, and one-tile fanout around the point two tiles in front of the monster."],
    },
  ],
  [
    22,
    {
      status: "implemented_special",
      notes: ["IncarnatedZT non-stoned Zuma melee baseline is covered with ObjectAttack, 300 ms delayed imported DC damage, and Crystal's 1/12 paralysis poison chance."],
    },
  ],
  [
    24,
    {
      status: "implemented_special",
      notes: ["DigOutZombie hidden/non-blocking initial state and near-player ObjectShow reveal are covered."],
    },
  ],
  [
    25,
    {
      status: "implemented_special",
      notes: ["RevivingZombie delayed two-revival baseline and reduced-health ObjectRevived flow are covered."],
    },
  ],
  [
    26,
    {
      status: "implemented_special",
      notes: ["ShamanZombie six-tile line/diagonal ObjectRangeAttack and delayed hit timing are covered."],
    },
  ],
  [
    27,
    {
      status: "implemented_special",
      notes: ["Khazard four-tile line/diagonal ObjectRangeAttack pull branch is covered with no direct player damage, immediate player pull movement toward Khazard, and 5s PullTime cooldown; exact magic-resist checks remain pending."],
    },
  ],
  [
    28,
    {
      status: "implemented_special",
      notes: ["ToxicGhoul Crystal melee timing, DC-based damage, 1/8 five-second green poison status, and shared HarvestMonster two-pass corpse harvest are covered; death explosion remains data-gated because current imported AI28 rows have Effect=0."],
    },
  ],
  [
    29,
    {
      status: "implemented_special",
      notes: ["Two-tile line attack range and delayed damage are covered."],
    },
  ],
  [
    30,
    {
      status: "implemented_special",
      notes: ["BoneLord seven-tile ObjectRangeAttack branch is covered with distance-scaled delayed DC damage, and HP-stage ObjectAttack type-1 slave spawning now summons the Crystal bone minion set with the original 8-per-wave / 40-slave cap."],
    },
  ],
  [
    31,
    {
      status: "implemented_special",
      notes: ["RightGuard adjacent melee vs ranged packet switching is covered."],
    },
  ],
  [
    32,
    {
      status: "implemented_special",
      notes: ["LeftGuard ranged/near attack family is wired through the shared guard range logic."],
    },
  ],
  [
    33,
    {
      status: "implemented_special",
      notes: ["MinotaurKing RightGuard-derived six-tile ObjectRangeAttack branch is covered with 500 ms delayed imported DC damage and Crystal's three-tile range-damage fanout around the target."],
    },
  ],
  [
    34,
    {
      status: "implemented_special",
      notes: ["FrostTiger passive-until-targeted acquisition, six-tile range, non-adjacent ObjectRangeAttack, distance-scaled ranged delay, DC damage gating, effect-driven ranged bleeding/slow poison rolls, and ObjectSitDown sitting/standing presentation are covered."],
    },
  ],
  [
    35,
    {
      status: "implemented_special",
      notes: ["SandWorm SpittingSpider-style two-tile line ObjectAttack, 300 ms delayed hit timing, imported DC damage, forward-line multi-target fanout, and shared harvest-corpse baseline are covered."],
    },
  ],
  [
    36,
    {
      status: "implemented_special",
      notes: ["Yimoogi seven-tile ObjectRangeAttack branch is covered with 500 ms delayed imported DC damage; the four-tile type-1 red-poison branch, four-second sister child spawn, final teleport with WhiteSerpent spawns, and paired drop suppression are covered."],
    },
  ],
  [
    37,
    {
      status: "implemented_special",
      notes: ["CrystalSpider three-tile row/column/diagonal ObjectAttack type-1 branch, distance-scaled delayed DC damage, forward-line multi-target fanout, and 1/8 green poison baseline are covered."],
    },
  ],
  [
    38,
    {
      status: "implemented_special",
      notes: ["HolyDeva six-tile ObjectRangeAttack, summoned extra presentation, 500 ms delayed hit timing, imported DC damage, and Crystal fear-window kiting movement are covered."],
    },
  ],
  [
    39,
    {
      status: "implemented_special",
      notes: ["RootSpider delayed BombSpider summon with back-offset spawn semantics is covered."],
    },
  ],
  [
    40,
    {
      status: "implemented_special",
      notes: ["BombSpider adjacency/timeout self-destruct and delayed explosion damage are covered."],
    },
  ],
  [
    42,
    {
      status: "implemented_special",
      notes: ["Yin/Yang Devil Node immobility and no-player-attack support-node baseline are covered; friendly buff details remain pending."],
    },
  ],
  [
    43,
    {
      status: "implemented_special",
      notes: ["OmaKing seven-tile type-1 ObjectAttack magic branch is covered with 500 ms delayed imported MC damage; close Crystal split is also covered with type-0 push/paralysis handling and two-tile line fanout using imported DC damage."],
    },
  ],
  [
    44,
    {
      status: "implemented_special",
      notes: ["BlackFoxman two-tile type-1 line attack packet and delayed hit timing are covered."],
    },
  ],
  [
    45,
    {
      status: "implemented_special",
      notes: ["RedFoxman six-tile ObjectRangeAttack baseline is covered with imported DC damage, 500 ms delayed hit timing, deterministic coverage for Crystal's type-0/type-1 ranged packet split, fear-window kiting movement, and adjacent fear-window teleport with ObjectTeleportOut/ObjectTeleportIn effect type 2."],
    },
  ],
  [
    46,
    {
      status: "implemented_special",
      notes: ["WhiteFoxman six-tile ObjectRangeAttack baseline is covered with imported DC damage, delayed hit timing, the 1-in-8 type-1 slow branch as a delayed status-only action, and fear-window kiting movement."],
    },
  ],
  [
    47,
    {
      status: "implemented_special",
      notes: ["TrapRock hidden reveal baseline is covered: starts hidden/non-visible, blocks route/chase/patrol movement, reveals near a target after the Crystal visibility delay, uses deterministic SpawnCorner ordering to choose the adjacent target tile, emits ObjectShow, spawns the other three cardinal child rocks around the trapped target, applies reveal paralysis, parent TrapRock uses ObjectRangeAttack with no direct damage, child rocks use ObjectAttack, child hits clear the parent FirstAttack state, visible parent rocks die when the trapped target moves, the first player hit collapses the parent, and parent repeated attacks roll the Crystal 1-in-8 paralysis poison."],
    },
  ],
  [
    48,
    {
      status: "implemented_special",
      notes: ["GuardianRock static range-pull baseline is covered: no route/chase/patrol movement, normal damage immunity, 500 ms delayed ObjectRangeAttack visibility, no direct player damage, and Crystal pull distance toward the rock capped at four tiles; magic-resist handling remains pending."],
    },
  ],
  [
    49,
    {
      status: "implemented_special",
      notes: ["ThunderElement two-tile CompleteAttack baseline, delayed due-time ObjectAttack packet, DC-based damage, random near-target repositioning, nearby opposing-target fanout, and normal-damage immunity are covered; Repulsion push-damage from player skills remains pending."],
    },
  ],
  [
    50,
    {
      status: "implemented_special",
      notes: ["GreatFoxSpirit static seven-tile ObjectRangeAttack branch is covered with 300 ms delayed imported DC damage, Crystal FindAllTargets fanout, ObjectEffect GreatFoxSpirit broadcasts on ranged targets, slow/paralysis poison rolls, HP-stage extra-byte broadcasts, nearby GuardianRock activation/deactivation, and 10s-cooldown far-target recall-to-nearby-tile movement with ObjectTeleportOut/ObjectTeleportIn effect 11; multi-target/MagicResist recall details remain pending."],
    },
  ],
  [
    51,
    {
      status: "implemented_special",
      notes: ["HedgeKekTal near-vs-range switching is covered: eight-tile attack range, ObjectRangeAttack when non-adjacent, distance-scaled delayed ranged damage, adjacent ObjectAttack, and imported DC-based damage."],
    },
  ],
  [
    54,
    {
      status: "implemented_special",
      notes: ["MirStatue/DragonStatue static delayed ObjectRangeAttack baseline is covered with view-range reach, 500 ms due-time packet emission, imported DC damage, FindAllTargets(2)-style fanout around the player target, lethal-damage sleep instead of death, sleeping damage immunity, and 15-minute full-HP wake."],
    },
  ],
  [
    57,
    {
      status: "neutral_guard_baseline",
      notes: ["TownArcher/archer ranged packet behavior and neutral non-chasing baseline are covered."],
    },
  ],
  [
    86,
    {
      status: "implemented_special",
      notes: ["ManectricClaw/AI 86 three-tile ObjectRangeAttack thrust baseline is covered with Crystal's random pre-thrust step-toward-target branch, 500 ms delayed near-DC/far-MC damage, player slow/frozen poison rolls, and full three-column cone fanout to opposing monsters."],
    },
  ],
  [
    88,
    {
      status: "implemented_special",
      notes: ["ManectricKing/Master_DragonYang three-tile row/column/diagonal ObjectAttack magic line branch is covered with 500 ms delayed imported MC damage, the close type-1 DC push line branch is covered, and the low-HP ObjectRangeAttack mass attack now hits nearby targets with distance-delayed imported MC damage."],
    },
  ],
  [
    56,
    {
      status: "implemented_special",
      notes: ["Trainer static target-dummy baseline is covered: neutral/passive targeting, no movement/route/chase/patrol, no normal attacks, no HP loss/death from ordinary damage, and Crystal ChatType.Trainer damage/DPS plus idle average reporting."],
    },
  ],
  [
    58,
    {
      status: "neutral_guard_baseline",
      notes: ["Neutral TaoGuard disposition is imported; pet-specific Crystal targeting is pending."],
    },
  ],
  [
    60,
    {
      status: "implemented_special",
      notes: ["VampireSpider death explosion uses delayed summon-vs-monster damage path."],
    },
  ],
  [
    61,
    {
      status: "implemented_special",
      notes: ["SpittingToad long-range packet pressure is covered for friendly summon combat."],
    },
  ],
  [
    79,
    {
      status: "implemented_special",
      notes: ["HellKeeper static view-range ObjectAttack branches are covered with no route/chase/patrol movement, locked attack facing, 300 ms delayed imported DC damage for type 0, raw-MC gating plus defence mitigation and Dazed for nonzero-MC type 1, and FindAllTargets-style fanout to view-range opposing targets; current zero-MC HellKeeper data still emits type 1 without damage/dazed."],
    },
  ],
  [
    62,
    {
      status: "implemented_special",
      notes: ["SnakeTotem friendly summon ownership, minion cap, and CharmedSnake spawning are covered."],
    },
  ],
  [
    63,
    {
      status: "implemented_special",
      notes: ["CharmedSnake owner tracking, out-of-range cleanup, and death explosion are covered."],
    },
  ],
  [
    97,
    {
      status: "implemented_special",
      notes: ["HellKnight spawned extra presentation and HellLord stage-advance hook are covered."],
    },
  ],
  [
    98,
    {
      status: "implemented_special",
      notes: ["HellLord immobility, stage-gated damage immunity, knight spawning, bomb spawning, and stage update packets are covered."],
    },
  ],
  [
    99,
    {
      status: "implemented_special",
      notes: ["HellBomb immobility, damage immunity, timeout attack/death packets, delayed radius damage, and HellBomb1/HellBomb2/HellBomb3 Frozen/Dazed/Bleeding poison variants on successful player explosion hits are covered."],
    },
  ],
  [
    102,
    {
      status: "implemented_special",
      notes: ["IceGuard eight-tile range, adjacent ObjectAttack vs non-adjacent ObjectRangeAttack switching, fixed ranged delay, imported MC ranged damage, DC melee gating, fire ObjectRangeAttack type-1 branch, and ice-branch slow/frozen poison rolls are covered."],
    },
  ],
  [
    113,
    {
      status: "neutral_guard_baseline",
      notes: ["ArcherGuard neutral guard targeting, ranged attacks, and no-route behavior are covered."],
    },
  ],
  [
    115,
    {
      status: "implemented_special",
      notes: ["SandSnail adjacent attack split is covered: primary type-0 DC ObjectAttack, type-1 DC halfmoon arc fanout, and type-2 MC one-tile area fanout with Green poison on player hit, all using Crystal 300 ms delayed timing."],
    },
  ],
  [
    112,
    {
      status: "implemented_special",
      notes: ["DarkBeast primary Crystal melee timing and DC-based damage baseline are covered; secondary type-1 branch and bleeding hook are covered, with current spawned CatWidow data correctly gating damage/bleed because MC=0 and Effect=0."],
    },
  ],
  [
    116,
    {
      status: "implemented_special",
      notes: ["BlackHammerCat two-tile type-1 line attack packet and delayed hit timing are covered."],
    },
  ],
  [
    117,
    {
      status: "implemented_special",
      notes: ["StrayCat two-tile type-2 line attack packet and delayed hit timing are covered; the close type-1 push variant is covered and current imported MC=0 data correctly gates follow-up line damage."],
    },
  ],
  [
    118,
    {
      status: "implemented_special",
      notes: ["CatShaman six-tile ObjectRangeAttack baseline and delayed hit timing are covered; the 1-in-5 type-1 red-poison packet/hook is covered and current imported MC=0 data correctly gates damage and poison."],
    },
  ],
  [
    119,
    {
      status: "implemented_special",
      notes: ["Jar1 static one-tile attack baseline is covered, including no movement/chase and delayed regular-monster slave spawn on death; exact Crystal global RNG selection remains a deterministic runtime approximation."],
    },
  ],
  [
    120,
    {
      status: "implemented_special",
      notes: ["Jar2 static six-tile ObjectRangeAttack baseline is covered, including no movement/chase, zero-MC data gating, adjacent 1/3 DC ObjectAttack, adjacent 2/3 ObjectRangeAttack, and the Frozen poison hook for successful ranged hits; current generated Jar2 rows have MC=0 so ranged damage and poison remain data-gated."],
    },
  ],
  [
    121,
    {
      status: "implemented_special",
      notes: ["SeedingsGeneral two-tile ObjectRangeAttack magic branch is covered with 300 ms delayed imported MC damage, including type-0 Echo Shout slow poison and type-1 Stomp frozen poison plus adjacent opposing-target fanout; close mixed melee is covered with type-0 DC Blood Attack and type-1 MC Green Splash."],
    },
  ],
  [
    122,
    {
      status: "implemented_special",
      notes: ["RestlessJar static six-tile ObjectRangeAttack branch is covered, including no movement/chase, Crystal ProjectileAttack distance*50+500ms ranged timing, and zero-MC no-damage gating for current imported data; adjacent spin fanout, tornado/blindness, and low-HP stomp push/fanout are covered."],
    },
  ],
  [
    123,
    {
      status: "implemented_special",
      notes: ["GeneralMeowMeow twelve-tile range branch is covered with ObjectRangeAttack type 0, 500 ms delayed imported MC damage, two-tile target-area fanout, and close two-tile ObjectAttack/DC switching including the 1-in-9 type-1 triple-DC slam; HP shield windows now apply GeneralMeowMeowShield presentation and 100-AC damage absorption, shield-phase mass thunder emits ObjectSpell GeneralMeowMeowThunder packets with delayed MC damage to the player and opposing monsters near the target, and the 60s periodic slave spawn now uses the Crystal cat minion set with the original 3-per-wave / 6-slave cap."],
    },
  ],
  [
    124,
    {
      status: "implemented_special",
      notes: ["Armadillo DigOutZombie-style hidden reveal, DigOutArmadillo ObjectSpell presentation, primary ObjectAttack/DC melee branch, type-1 three-hit combo branch, retreat ObjectBackStep, retreat area range-damage, and run-away after failed retreat damage are covered with imported DC timing."],
    },
  ],
  [
    125,
    {
      status: "implemented_special",
      notes: ["ArmadilloElder DigOutZombie-style hidden reveal, DigOutArmadillo ObjectSpell presentation, primary ObjectAttack branch with imported DC*2 damage, type-1 two-tile push/no-direct-damage branch, retreat ObjectBackStep no-damage branch, and run-away movement are covered."],
    },
  ],
  [
    126,
    {
      status: "implemented_special",
      notes: ["TucsonMage three-tile ObjectAttack type-1 WideLine branch is covered, including zero-MC no-damage gating for current imported data, adjacent 1-in-3 branch selection, and fanout to the forward plus three-lane two-step line when MC is available."],
    },
  ],
  [
    127,
    {
      status: "implemented_special",
      notes: ["TucsonWarrior two-tile non-adjacent ObjectAttack type-1 smash branch is covered with imported MC damage; adjacent 4/5 halfmoon DC fanout, adjacent 1/5 type-1 MC smash, and target-area multi-target coverage are covered."],
    },
  ],
  [
    128,
    {
      status: "implemented_special",
      notes: ["TucsonEgg immobile/no-turn/no-walk baseline, fixed one-HP damage intake, and delayed death poison/spawn hook are covered; current generated TucsonEgg rows have Effect=0/DC=0 so slave spawn and poison damage remain data-gated."],
    },
  ],
  [
    130,
    {
      status: "implemented_special",
      notes: ["CannibalTentacles non-adjacent range branch is covered with view-range ObjectRangeAttack, distance-scaled delayed hit timing, and imported MC-based damage; adjacent Crystal type-1 halfmoon is also covered with fixed 500 damage, green poison on player hit, and arc fanout to adjacent opposing monsters."],
    },
  ],
  [
    131,
    {
      status: "implemented_special",
      notes: ["TucsonGeneral opening rage ObjectRangeAttack type-0 packet is covered with no direct damage plus 20-second rage cooldown/8-second attack pause; the rage branch now also spawns 15 delayed TucsonGeneralRock ObjectSpell objects with deterministic target-biased scatter and raw-DC impact damage. Normal >2 tile ObjectRangeAttack type-1, 1-in-4 type-2 ranged SC*2, and close type-1 MC stomp/paralysis area branches are covered; exact Crystal global RNG ordering remains approximated."],
    },
  ],
  [
    173,
    {
      status: "implemented_special",
      notes: ["TurtleGrass Zuma-style stone/wake state, two-tile Crystal attack shape, ObjectAttack packet baseline, imported DC damage, and the type-1 single-push branch with three-tile displacement plus delayed DC damage are covered."],
    },
  ],
  [
    174,
    {
      status: "implemented_special",
      notes: ["ManTree/FineSoul Zuma-style stone/wake state and adjacent ObjectAttack packet branches are covered, including type-0, type-1 halfmoon, and type-2 boulder/Stun hook; current generated FineSoul rows have DC=0/MC=0 so delayed damage, halfmoon fanout, and Stun remain data-gated."],
    },
  ],
  [
    179,
    {
      status: "implemented_special",
      notes: ["SnowWolf primary ObjectAttack branch is covered with 350 ms delayed timing and imported DC damage; the type-1 ObjectAttack branch follows raw-MC gating with defence mitigation, keeps current zero-MC data damage-free, and covers nonzero-MC Slow/Frozen poison plus FindAllTargets(2)-style fanout."],
    },
  ],
  [
    180,
    {
      status: "implemented_special",
      notes: ["FrozenWarewolf/SnowWolfKing ObjectAttack type 0/1/2/3 branches are covered with 500 ms delayed imported DC damage, the below-70% SnowWolf slave spawn is covered, and delayed one-tile death explosion damage is covered; teleport retarget and pet transfer remain pending."],
    },
  ],
  [
    181,
    {
      status: "implemented_special",
      notes: ["WaterDragon non-adjacent ObjectRangeAttack baseline, delayed hit timing, imported MC damage, and 1-in-7 five-tick green poison on successful ranged hits are covered."],
    },
  ],
  [
    187,
    {
      status: "implemented_special",
      notes: ["FrozenMiner primary ObjectAttack branch is covered with Crystal 600 ms delayed timing and imported DC damage; type-1 ObjectAttack area branch is covered with 1000 ms delayed imported 80% DC damage for the player target plus adjacent opposing-monster FindAllTargets(1)-style fanout."],
    },
  ],
  [
    188,
    {
      status: "implemented_special",
      notes: ["FrozenAxeman two-tile line/diagonal type-1 ObjectAttack branch is covered with 500 ms delayed timing and imported DC*2 damage; adjacent type-2 pull/push branch is covered with a Crystal-shaped 2/3 deterministic trigger, 10s cooldown, 2-4 tile player push, and 500 ms imported DC hit."],
    },
  ],
  [
    189,
    {
      status: "implemented_special",
      notes: ["FrozenMagician nine-tile non-adjacent ObjectRangeAttack branches are covered: type-0 uses distance-scaled 600 ms base delay with imported MC damage, and type-1 uses the boosted 750 ms base delay with imported MC*3/2 damage."],
    },
  ],
  [
    190,
    {
      status: "implemented_special",
      notes: ["SnowYeti nine-tile non-adjacent ObjectRangeAttack branch is covered with distance-scaled delayed timing, imported DC damage, and the Crystal frozen poison roll; the adjacent ObjectAttack double-hit branch is covered with type-0/type-1 packets plus 500/1500 ms imported DC hits."],
    },
  ],
  [
    192,
    {
      status: "implemented_special",
      notes: ["DarkWraith four-tile line/diagonal ObjectAttack type-2 branch is covered with imported DC*3 damage, four-tile line fanout, and Crystal's 3-7s line cooldown approximation; the adjacent type-1 area branch is covered with 600 ms imported DC damage plus adjacent opposing-target fanout."],
    },
  ],
  [
    182,
    {
      status: "implemented_special",
      notes: ["BlackTortoise non-adjacent ObjectRangeAttack baseline, delayed hit timing, green-poison hook with current SmallDrake MC=0 damage/poison gating, and close 1-in-5 type-1 halfmoon fanout are covered."],
    },
  ],
  [
    186,
    {
      status: "implemented_special",
      notes: ["Lamia/Kirin two-tile row/diagonal ObjectAttack baseline is covered with imported DC damage, including the type-1 500 ms DC branch; the nonzero-MC type-2 IceThrust cone, slow poison, and opposing-target fanout are supported, while current Lamia MC=0 data naturally gates that branch."],
    },
  ],
  [
    255,
    {
      status: "implemented_special",
      notes: ["StoneTrap immobility, trap-priority hostile targeting, owner binding, and struck immunity are covered."],
    },
  ],
]);

function main() {
  const monsterManifest = readJson(monsterManifestPath);
  const respawnManifest = readJson(respawnManifestPath);
  const crystalSource = readFileSync(crystalMonsterObjectPath, "utf8");
  const sourceCases = parseCrystalMonsterObjectCases(crystalSource);

  const monsterStats = collectMonsterStats(monsterManifest.monsters);
  const respawnStats = collectRespawnStats(respawnManifest.maps);
  const allAiValues = new Set([
    ...sourceCases.keys(),
    ...monsterStats.keys(),
    ...respawnStats.keys(),
  ]);

  const families = [...allAiValues]
    .sort((left, right) => left - right)
    .map((ai) => buildFamily(ai, sourceCases, monsterStats, respawnStats));

  const summary = {
    generated_at: new Date().toISOString(),
    source_files: {
      monster_manifest: "packages/game-data/data/generated/crystal_monster_manifest.json",
      respawn_manifest: "packages/game-data/data/generated/crystal_respawn_manifest.json",
      crystal_monster_object: "Crystal/Server/MirObjects/MonsterObject.cs",
    },
    total_monsters: monsterManifest.total_monsters,
    total_ai_families: families.length,
    spawned_ai_families: families.filter((family) => family.respawn_entity_count > 0).length,
    implemented_runtime_families: families.filter((family) =>
      ["implemented_special", "neutral_guard_baseline"].includes(family.runtime_status),
    ).length,
    generic_runtime_families: families.filter((family) => family.runtime_status === "generic_baseline").length,
    data_only_families: families.filter((family) => family.runtime_status === "data_only").length,
    unknown_source_ai_values: families
      .filter((family) => family.crystal_class === "Unknown")
      .map((family) => family.ai),
    remaining_runtime_priorities: buildRemainingRuntimePriorities(families),
    families,
  };

  mkdirSync(dirname(jsonOutputPath), { recursive: true });
  mkdirSync(dirname(markdownOutputPath), { recursive: true });
  writeFileSync(jsonOutputPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  writeFileSync(markdownOutputPath, renderMarkdown(summary), "utf8");

  console.log(`Wrote Crystal monster AI summary to ${jsonOutputPath}`);
  console.log(`Wrote Crystal monster AI markdown summary to ${markdownOutputPath}`);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function parseCrystalMonsterObjectCases(source) {
  const switchStart = source.indexOf("switch (info.AI)");
  if (switchStart < 0) {
    throw new Error("Could not find MonsterObject.GetMonster AI switch.");
  }
  const defaultStart = source.indexOf("default:", switchStart);
  if (defaultStart < 0) {
    throw new Error("Could not find MonsterObject.GetMonster default case.");
  }

  const cases = new Map([
    [
      0,
      {
        crystal_class: "MonsterObject",
        crystal_notes: ["Default generic MonsterObject fallback."],
        crystal_todo: false,
      },
    ],
  ]);
  const lines = source.slice(switchStart, defaultStart).split(/\r?\n/);
  let pendingAiValues = [];
  let pendingNotes = [];

  for (const rawLine of lines) {
    const line = rawLine.trim();
    const caseMatch = line.match(/^case\s+(\d+)\s*:/);
    if (caseMatch) {
      pendingAiValues.push(Number.parseInt(caseMatch[1], 10));
      continue;
    }

    const commentIndex = line.indexOf("//");
    if (pendingAiValues.length > 0 && commentIndex >= 0) {
      const note = cleanCrystalComment(line.slice(commentIndex + 2));
      if (note.length > 0) {
        pendingNotes.push(note);
      }
    }

    const returnMatch = line.match(/return\s+new\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(info\)/);
    if (!returnMatch || pendingAiValues.length === 0) {
      continue;
    }

    const crystalClass = returnMatch[1];
    const notes = [...new Set(pendingNotes)];
    const todo = notes.some((note) => note.toUpperCase().includes("TODO"));
    for (const ai of pendingAiValues) {
      cases.set(ai, {
        crystal_class: crystalClass,
        crystal_notes: notes,
        crystal_todo: todo,
      });
    }
    pendingAiValues = [];
    pendingNotes = [];
  }

  return cases;
}

function cleanCrystalComment(value) {
  return value
    .replace(/^[-\s]+/, "")
    .replace(/\s+/g, " ")
    .trim();
}

function collectMonsterStats(monsters) {
  const stats = new Map();
  for (const monster of monsters) {
    const entry = stats.get(monster.ai) ?? {
      monster_count: 0,
      boss_monster_count: 0,
      example_monsters: [],
    };
    entry.monster_count += 1;
    if (monster.is_boss) {
      entry.boss_monster_count += 1;
    }
    if (entry.example_monsters.length < 8) {
      entry.example_monsters.push(monster.name);
    }
    stats.set(monster.ai, entry);
  }
  return stats;
}

function collectRespawnStats(maps) {
  const stats = new Map();
  for (const map of maps) {
    for (const respawn of map.respawns) {
      const entry = stats.get(respawn.monster_ai) ?? {
        respawn_rule_count: 0,
        respawn_entity_count: 0,
        maps: new Set(),
        example_maps: [],
      };
      entry.respawn_rule_count += 1;
      entry.respawn_entity_count += respawn.count;
      entry.maps.add(map.map_file_name);
      addUniqueLimited(entry.example_maps, `${map.map_file_name} ${map.map_title}`, 8);
      stats.set(respawn.monster_ai, entry);
    }
  }
  return stats;
}

function buildFamily(ai, sourceCases, monsterStats, respawnStats) {
  const numericAi = Number(ai);
  const source = sourceCases.get(numericAi) ?? {
    crystal_class: "Unknown",
    crystal_notes: [],
    crystal_todo: false,
  };
  const monsters = monsterStats.get(numericAi) ?? {
    monster_count: 0,
    boss_monster_count: 0,
    example_monsters: [],
  };
  const respawns = respawnStats.get(numericAi) ?? {
    respawn_rule_count: 0,
    respawn_entity_count: 0,
    maps: new Set(),
    example_maps: [],
  };
  const coverage = runtimeCoverageFor(numericAi, respawns.respawn_entity_count);

  return {
    ai: numericAi,
    crystal_class: source.crystal_class,
    crystal_notes: source.crystal_notes,
    crystal_todo: source.crystal_todo,
    runtime_status: coverage.status,
    runtime_notes: coverage.notes,
    monster_count: monsters.monster_count,
    boss_monster_count: monsters.boss_monster_count,
    respawn_rule_count: respawns.respawn_rule_count,
    respawn_entity_count: respawns.respawn_entity_count,
    map_count: respawns.maps.size,
    example_monsters: monsters.example_monsters,
    example_maps: respawns.example_maps,
  };
}

function runtimeCoverageFor(ai, respawnEntityCount) {
  const coverage = RUNTIME_COVERAGE.get(ai);
  if (coverage) {
    return coverage;
  }
  if (respawnEntityCount <= 0) {
    return {
      status: "data_only",
      notes: ["Imported in Crystal monster manifest, but not referenced by current respawn data."],
    };
  }
  return {
    status: "generic_baseline",
    notes: ["Spawned through imported respawn metadata with generic hostile movement/combat baseline; AI-specific Crystal behavior is pending."],
  };
}

function buildRemainingRuntimePriorities(families) {
  return families
    .filter(
      (family) =>
        family.respawn_entity_count > 0 &&
        ["generic_baseline", "wildlife_partial"].includes(family.runtime_status),
    )
    .map((family) => buildPriorityEntry(family))
    .sort(
      (left, right) =>
        right.priority_score - left.priority_score ||
        right.respawn_entity_count - left.respawn_entity_count ||
        right.map_count - left.map_count ||
        left.ai - right.ai,
    )
    .map((entry, index) => ({
      rank: index + 1,
      ...entry,
    }));
}

function buildPriorityEntry(family) {
  const reasons = [];
  let score = 0;

  const add = (bonus, reason) => {
    if (bonus <= 0) {
      return;
    }
    score += bonus;
    reasons.push(reason);
  };

  add(family.respawn_entity_count * 10, `${family.respawn_entity_count} live respawn entities`);
  add(family.respawn_rule_count * 5, `${family.respawn_rule_count} respawn rules`);
  add(family.map_count * 100, `${family.map_count} maps touched`);
  add(family.monster_count * 25, `${family.monster_count} monster rows share this AI`);
  add(family.boss_monster_count * 600, `${family.boss_monster_count} boss rows share this AI`);

  const text = `${family.crystal_class} ${family.crystal_notes.join(" ")} ${family.example_monsters.join(" ")}`.toLowerCase();
  if (/\b(range|projectile)\b/.test(text)) {
    add(1200, "packet-visible ranged/projectile attack semantics");
  }
  if (/\bmagic\b/.test(text)) {
    add(1000, "magic attack semantics");
  }
  if (/\bline\b/.test(text)) {
    add(900, "line attack geometry");
  }
  if (/\bpoison\b/.test(text)) {
    add(800, "poison side effects");
  }
  if (/\bfear\b/.test(text)) {
    add(700, "fear side effects");
  }
  if (/\bteleport\b/.test(text)) {
    add(700, "teleport side effects");
  }
  if (/\brush\b/.test(text)) {
    add(600, "rush timing side effects");
  }
  if (/\b(aoe|halfmoon|fullmoon)\b/.test(text)) {
    add(1000, "area attack geometry");
  }
  if (/\beffect\b/.test(text)) {
    add(500, "visible effect-state semantics");
  }
  if (/attack on death/.test(text)) {
    add(1000, "attack-on-death semantics");
  }
  if (/\b(king|lord|queen|master|general|elder|evil|spirit|guardian|boss)\b/.test(text)) {
    add(1200, "boss or progression-gate naming risk");
  }
  if (family.crystal_todo) {
    add(400, "Crystal source marks TODO behavior");
  }

  return {
    ai: family.ai,
    crystal_class: family.crystal_class,
    runtime_status: family.runtime_status,
    priority_score: score,
    respawn_entity_count: family.respawn_entity_count,
    respawn_rule_count: family.respawn_rule_count,
    map_count: family.map_count,
    monster_count: family.monster_count,
    boss_monster_count: family.boss_monster_count,
    example_monsters: family.example_monsters,
    example_maps: family.example_maps,
    priority_reasons: reasons,
  };
}

function renderMarkdown(summary) {
  const priorityRows = summary.remaining_runtime_priorities
    .map((entry) =>
      `| ${[
        entry.rank,
        entry.ai,
        md(entry.crystal_class),
        md(entry.runtime_status),
        entry.priority_score,
        entry.respawn_entity_count,
        entry.respawn_rule_count,
        entry.map_count,
        entry.boss_monster_count,
        md(entry.example_monsters.join(", ")),
        md(entry.example_maps.join(", ")),
        md(entry.priority_reasons.join(" / ")),
      ].join(" | ")} |`,
    )
    .join("\n");

  const rows = summary.families
    .map((family) =>
      `| ${[
        family.ai,
        md(family.crystal_class),
        md(family.runtime_status),
        family.monster_count,
        family.respawn_rule_count,
        family.respawn_entity_count,
        family.map_count,
        family.boss_monster_count,
        md(family.example_monsters.join(", ")),
        md(family.example_maps.join(", ")),
        md([...family.crystal_notes, ...family.runtime_notes].join(" / ")),
      ].join(" | ")} |`,
    )
    .join("\n");

  return `# Crystal Monster AI Summary

Generated at: ${summary.generated_at}

Source:

- Crystal AI switch: ${summary.source_files.crystal_monster_object}
- Monster manifest: ${summary.source_files.monster_manifest}
- Respawn manifest: ${summary.source_files.respawn_manifest}

Totals:

- Monster rows: ${summary.total_monsters}
- AI families: ${summary.total_ai_families}
- Spawned AI families: ${summary.spawned_ai_families}
- Runtime special/guard families: ${summary.implemented_runtime_families}
- Runtime generic families: ${summary.generic_runtime_families}
- Data-only families: ${summary.data_only_families}
- Unknown Crystal source AI values: ${summary.unknown_source_ai_values.length === 0 ? "none" : summary.unknown_source_ai_values.join(", ")}

Remaining runtime priority queue:

| Rank | AI | Crystal class | Runtime status | Score | Respawn count | Respawn rules | Maps | Boss rows | Examples | Map examples | Reasons |
| ---: | ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
${priorityRows}

Full AI family inventory:

| AI | Crystal class | Runtime status | Monsters | Respawn rules | Respawn count | Maps | Boss rows | Examples | Map examples | Notes |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
${rows}
`;
}

function addUniqueLimited(values, value, limit) {
  if (!values.includes(value) && values.length < limit) {
    values.push(value);
  }
}

function md(value) {
  return String(value).replace(/\|/g, "\\|").replace(/\r?\n/g, " ");
}

main();
