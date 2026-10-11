// Source-owned five-class effect contracts. These are resources, not permission
// to render a stateful effect without its authoritative Crystal packet/buff.
const PLAYER = "Crystal/Client/MirObjects/PlayerObject.cs";
const GAME = "Crystal/Client/MirScenes/GameScene.cs";
const WORLD = "Crystal/Client/MirObjects/SpellObject.cs";
const MONSTER = "Crystal/Client/MirObjects/MonsterObject.cs";
const EFFECT = "Crystal/Client/MirObjects/Effect.cs";
const phase = (library, base, count, interval, kind, extra = {}) => ({
  library, base, count, interval, kind, blend: true, light: 6, repeat: false,
  offset: { x: 0, y: 0 }, ...extra,
});
const directions = (base, count, directionCount = 16, directionStride = 10) => ({
  directionCount, directionStride,
  directionRanges: Array.from({ length: directionCount }, (_, direction) => ({
    direction, base: base + direction * directionStride,
    end: base + direction * directionStride + count - 1,
  })),
});
const arrow = (base, count = 5, interval = 10) => phase("Magic3", base, count, interval, "projectile", directions(base, count));
const ranged = (spell, spellId, projectile, extra = {}) => ({
  spell, spellId, ...projectile, projectile, ...extra,
  provenance: { source: `${PLAYER}::MirAction.AttackRange2`, symbol: `Spell.${spell}` },
});
const client = (effect, library, base, count, interval, source, extra = {}) => ({
  effect, ...phase(library, base, count, interval, "target", extra),
  provenance: { source, symbol: effect },
});

export const FIVE_CLASS_ADDITIONAL_SPELL_EFFECTS = [
  ranged("StraightShot", 122, arrow(1210, 5, 30), {
    impact: phase("Magic3", 1370, 7, 85, "target"),
  }),
  ranged("DoubleShot", 123, arrow(1030, 5, 30), { launchFrames: [5, 7] }),
  { ...ranged("CatTongue", 107, arrow(260, 6, 30), {
    impact: phase("Magic3", 420, 8, 100, "target"),
  }), provenance: { source: `${PLAYER}::MirAction.Spell/Complete`, symbol: "Spell.CatTongue" } },
  ranged("VampireShot", 133, arrow(1930), {
    impact: phase("Magic3", 2090, 6, 166, "target"),
    returnEffect: phase("Magic3", 2100, 8, 125, "return"),
  }),
  ranged("CrippleShot", 136, arrow(2330), {
    // Base impact is for neither enhancement buff. Other impacts must be
    // selected from typed caster buffs; PoisonShot wins over VampireShot.
    impact: phase("Magic3", 2490, 7, 142, "target"),
  }),
  ...[["BindingShot", 140], ["SummonVampire", 132], ["SummonSnakes", 137]]
    .map(([name, id]) => ranged(name, id, arrow(2750))),
  ranged("Stonetrap", 130, arrow(2750, 5, 20), { explodeAtGround: true }),
];
export const FIVE_CLASS_POISON_SHOT_PHASES = {
  projectile: arrow(2130), impact: phase("Magic3", 2290, 6, 166, "target"),
};
export const FIVE_CLASS_ADDITIONAL_CLIENT_EFFECTS = [
  client("AssassinDarkBodyDisappear", "Magic2", 2600, 10, 120, `${GAME}::ObjectDied/Type=2`, { kind: "ground" }),
  ...[["AssassinDoubleSlashAttack1", 1960, "Attack1"], ["AssassinDoubleSlashAttack4", 2050, "Attack4"]]
    .map(([name, base, action]) => client(name, "Magic2", base, 6, 46, `${PLAYER}::DrawEffects/MirAction.${action}`, {
      kind: "attackOverlay", rate: 0.7, light: 0, ...directions(base, 6, 8),
    })),
  client("MPEaterSecondary", "Magic2", 2411, 19, 100, `${GAME}::ObjectEffect/SpellEffect.MPEater`),
  client("HemorrhageLayer2", "Magic3", 28, 6, 100, `${GAME}::ObjectEffect/SpellEffect.Hemorrhage`),
  client("HemorrhageLayer3", "Magic3", 46, 8, 100, `${GAME}::ObjectEffect/SpellEffect.Hemorrhage`),
  client("ArcherConcentration", "Magic3", 1860, 8, 100, `${GAME}::SetConcentration`, { kind: "attackOverlay", repeat: true }),
  ...[["Green", 1630], ["Blue", 1640], ["Red", 1650], ["Mixed", 1660]]
    .map(([colour, base]) => client(`ArcherElement${colour}`, "Magic3", base, 10, 100, `${GAME}::SetElemental`, { kind: "attackOverlay", repeat: true })),
  client("ArcherElementalShotCast", "Magic3", 1681, 5, 160, `${PLAYER}::MirAction.AttackRange2/Spell.ElementalShot`, { kind: "attackOverlay" }),
  client("ArcherElementalShotArrow", "Magic3", 1690, 6, 30, `${PLAYER}::MirAction.AttackRange2/Spell.ElementalShot`, { kind: "projectile", ...directions(1690, 6) }),
  client("ArcherBindingDrop", "MagicC", 0, 8, 87, `${GAME}::SetBindingShot`),
  client("ArcherBindingDown", "MagicC", 7, 1, 1000, `${GAME}::SetBindingShot`, { repeat: true }),
  client("ArcherBindingFall", "MagicC", 8, 8, 87, `${GAME}::SetBindingShot`),
  ...[0, 1, 2].map((stage) => client(`ArcherDelayedExplosionStage${stage}`, "Magic3", 1590 + stage * 10, 8, 150,
    `${GAME}::ObjectEffect/SpellEffect.DelayedExplosion`, { repeat: stage !== 2 })),
  client("ArcherDelayedExplosionArrow", "Magic3", 1030, 5, 30, `${PLAYER}::MirAction.AttackRange2/Spell.DelayedExplosion`, { kind: "projectile", ...directions(1030, 5) }),
  client("ArcherCripplePoisonImpact", "Magic3", 2500, 7, 142, `${PLAYER}::MirAction.AttackRange2/Spell.CrippleShot`),
  client("ArcherCrippleVampireImpact", "Magic3", 2510, 7, 142, `${PLAYER}::MirAction.AttackRange2/Spell.CrippleShot`),
  client("ArcherVampireReturn", "Magic3", 2100, 8, 125, `${PLAYER}::MirAction.AttackRange2/Complete`, { kind: "return" }),
  client("ArcherBarrierPersistent16", "Magic3", 1890, 16, 200, `${PLAYER}::ElementalBarrier/CurrentEffect`, { kind: "attackOverlay", repeat: true }),
  client("ArcherBarrierStruck", "Magic3", 1910, 5, 120, `${PLAYER}::ElementalBarrier/MirAction.Struck`, { kind: "attackOverlay" }),
  client("ArcherExplosiveTrapDetonation", "Magic3", 1570, 9, 100, `${WORLD}::Load/Spell.ExplosiveTrap/info.Param`, { kind: "ground", light: 0, rate: 0.8 }),
  client("ArcherTrapRockImpact", "Monster/130", 26, 10, 60, `${MONSTER}::ProcessFrames/Monster.TrapRock`),
  client("ArcherVampireSpiderDeath", "Monster/359", 296, 5, 200, `${MONSTER}::MirAction.Die/Monster.VampireSpider`),
  client("ArcherSnakeTotemAura", "Monster/361", 2, 10, 150, `${MONSTER}::MirAction.Standing/Monster.SnakeTotem`, { repeat: true }),
  client("ArcherCharmedSnakeDeath", "Monster/362", 40, 8, 100, `${MONSTER}::MirAction.Die/Monster.CharmedSnake`),
];
export const FIVE_CLASS_ADDITIONAL_WORLD_EFFECTS = [
  ...[["Trap", 98, "Magic2", 2360, 8], ["ExplosiveTrap", 124, "Magic3", 1560, 10]]
    .map(([spell, spellId, library, base, count]) => ({
      spell, spellId, ...phase(library, base, count, 100, "ground", { light: 0, rate: 0.8, repeat: true }),
      provenance: { source: `${WORLD}::Load`, symbol: `Spell.${spell}` },
    })),
];
export const FIVE_CLASS_ADDITIONAL_OBJECT_EFFECTS = [{
  effect: "Hemorrhage", effectId: 17, ...phase("Magic3", 0, 4, 100, "target"),
  provenance: { source: `${GAME}::ObjectEffect`, symbol: "SpellEffect.Hemorrhage" },
}];

const ids = (spell, ...variants) => variants.map((variant) => 20000 + spell * 10 + variant);
const skill = (spell, className, spellId, refs, sounds, trigger, source, actors = [], noBitmapReason = null) => ({
  spell, class: className, spellId, refs, soundIds: sounds, trigger, source, actors,
  ...(noBitmapReason ? { noBitmapReason } : {}),
});
const orbRefs = ["Green", "Blue", "Red", "Mixed"].map((colour) => `client_effects:ArcherElement${colour}`);
const assassin = (name, id, refs, sounds, trigger, source, noBitmapReason) => skill(name, "Assassin", id, refs, sounds, trigger, source, [], noBitmapReason);
const archer = (name, id, refs, sounds, trigger, source, actors, noBitmapReason) => skill(name, "Archer", id, refs, sounds, trigger, source, actors, noBitmapReason);
export const FIVE_CLASS_SKILL_SOURCE = [
  assassin("FatalSword", 91, ["object_effects:FatalSword"], ids(91, 0), "ObjectEffect.FatalSword on target", `${GAME}:4729-4732`),
  assassin("DoubleSlash", 92, ["client_effects:AssassinDoubleSlashAttack1", "client_effects:AssassinDoubleSlashAttack4"], ids(92, 0, 1), "Attack1 followed by queued Attack4, frame interval truncates body interval * 0.46", `${PLAYER}:1540-1548,1581-1585,4959-4960,4983-4984`),
  assassin("Haste", 93, ["spell_effects:Haste"], ids(93, 0), "MirAction.Spell; directional caster", `${PLAYER}:1765-1768`),
  assassin("FlashDash", 94, [], ids(94, 0, 1), "DashAttack / Attack4 source player action; gender selects sound", `${PLAYER}:1591-1596,2132-2140`, "No additional Magic bitmap in source; real A/C player action closure is required"),
  assassin("LightBody", 95, ["spell_effects:LightBody"], ids(95, 0), "MirAction.Spell caster", `${PLAYER}:1936-1938`),
  assassin("HeavenlySword", 96, ["spell_effects:HeavenlySword"], ids(96, 0), "MirAction.Spell directional caster", `${PLAYER}:2181-2184`),
  assassin("FireBurst", 97, ["spell_effects:FireBurst"], ids(97, 0), "MirAction.Spell caster", `${PLAYER}:1847-1849`),
  assassin("Trap", 98, ["spell_effects:Trap", "ground_effects:Trap"], ids(98, 0, 1), "Cast then authoritative SpellObject.Trap; source world lifetime", `${PLAYER}:1909-1911,3339-3341;${WORLD}:132-137`),
  assassin("PoisonSword", 99, ["spell_effects:PoisonSword"], ids(99, 0), "MirAction.Spell directional caster", `${PLAYER}:1946-1948`),
  assassin("MoonLight", 100, ["spell_effects:MoonLight"], ids(100, 0), "MirAction.Spell caster", `${PLAYER}:1918-1920`),
  assassin("MPEater", 101, ["object_effects:MPEater", "client_effects:MPEaterSecondary"], ids(91, 0), "ObjectEffect.MPEater; secondary owner is packet EffectType object id", `${GAME}:4751-4757`),
  assassin("SwiftFeet", 102, ["spell_effects:SwiftFeet"], ids(102, 0), "MirAction.Spell caster", `${PLAYER}:1927-1929`),
  assassin("DarkBody", 103, ["spell_effects:DarkBody", "client_effects:AssassinDarkBodyDisappear"], ids(103, 0, 1), "Cast and source stealth removal; ObjectDied.Type=2 at removed clone's last location, never unconditional invisibility", `${PLAYER}:1955-1957;${GAME}:3790-3812`),
  assassin("Hemorrhage", 104, ["object_effects:Hemorrhage", "client_effects:HemorrhageLayer2", "client_effects:HemorrhageLayer3", "object_effects:Bleeding"], ids(104, 0), "Hemorrhage packet starts three simultaneous target layers; Bleeding separate packet", `${GAME}:4758-4772`),
  assassin("CrescentSlash", 105, ["spell_effects:CrescentSlash"], ids(105, 0, 1, 2), "Directional caster; gender cast sound then completion sound", `${PLAYER}:2121-2125,3348-3349`),
  assassin("CatTongue", 107, ["spell_effects:CatTongue"], ids(107, 0, 1), "Cast completion projectile; target must still be alive for impact", `${PLAYER}:3142-3153`),
  archer("Focus", 121, ["client_effects:ArcherFocusProc"], ids(121, 5), "AttackRange1 body frame6 only when authoritative Focus state is set", `${PLAYER}:2553-2566`),
  archer("StraightShot", 122, ["spell_effects:StraightShot"], ids(122, 0, 2), "AttackRange2 completion and Cast; living target impact", `${PLAYER}:2577-2615`),
  archer("DoubleShot", 123, ["spell_effects:DoubleShot", "client_effects:ArcherNormalArrow"], ids(121, 0, 2), "AttackRange2 launches separately at body frames5 and7", `${PLAYER}:2617-2634`),
  archer("ExplosiveTrap", 124, ["ground_effects:ExplosiveTrap", "client_effects:ArcherExplosiveTrapDetonation"], ids(124, 0, 5), "Harvest action sound, then SpellObject Param=false armed / true detonation", `${PLAYER}:1474-1478;${WORLD}:112-130`),
  archer("DelayedExplosion", 125, ["client_effects:ArcherDelayedExplosionArrow", ...[0, 1, 2].map((n) => `client_effects:ArcherDelayedExplosionStage${n}`)], [...ids(121, 0, 2), ...ids(125, 0, 5)], "AttackRange2 body5 arrow; ObjectEffect.DelayedExplosion increasing EffectType stages0/1/2; remove packet/death", `${PLAYER}:2693-2708;${GAME}:4828-4842;${EFFECT}:321-368`),
  archer("Meditation", 126, orbRefs, ids(126, 5, 6, 7, 8), "Passive gather; SetElemental Enabled/ElementType1..4 selects orb; typed state only", `${GAME}:5569-5583;${EFFECT}:269-317`),
  archer("BackStep", 127, [], ids(127, 5, 6, 7), "Jump body action, sound frame1 gender and frame7 landing", `${PLAYER}:1213-1225,2335-2353`, [], "Source uses player Jump animation; no independent Magic bitmap"),
  archer("ElementalShot", 128, ["client_effects:ArcherElementalShotCast", "client_effects:ArcherElementalShotArrow", ...orbRefs], [...ids(128, 0, 2), ...ids(126, 5, 6, 7, 8)], "HasElements && !ElementCasted; body1 cast, body7 arrow; no generic cast if authoritative state absent", `${PLAYER}:1228-1240,2636-2653;${GAME}:5569-5583`),
  archer("Concentration", 129, ["client_effects:ArcherConcentration"], ids(129, 0), "SetConcentration Enabled && !Interrupted; pause/hide if interrupted, remove when disabled", `${GAME}:5553-5565;${EFFECT}:221-266`),
  archer("Stonetrap", 130, ["spell_effects:Stonetrap", "client_effects:ArcherTrapRockImpact"], [...ids(121, 0), ...ids(130, 7), 1306], "AttackRange2 body7; ground exploding projectile; server Monster.TrapRock actor and hit", `${PLAYER}:2676-2687;${MONSTER}:2691-2696`, [130]),
  archer("ElementalBarrier", 131, ["spell_effects:ElementalBarrier", "object_effects:ElementalBarrierUp", "object_effects:ElementalBarrierDown", "client_effects:ArcherBarrierPersistent16", "client_effects:ArcherBarrierStruck"], ids(131, 0, 5), "Cast requires HasElements && !ElementalBarrier; packet Up10/Down7, player persistent16/struck5 variants", `${PLAYER}:2190-2196,2254-2272,5008;${GAME}:4803-4825`),
  archer("SummonVampire", 132, ["spell_effects:SummonVampire", "client_effects:ArcherVampireSpiderDeath"], [...ids(121, 0), ...ids(132, 7)], "AttackRange2 body7 and server ObjectMonster VampireSpider image359", `${PLAYER}:2656-2671;${MONSTER}:1122-1123`, [359]),
  archer("VampireShot", 133, ["spell_effects:VampireShot", "client_effects:ArcherVampireReturn"], [...ids(121, 0), ...ids(133, 2), ...ids(45, 2)], "AttackRange2 body7; impact on target and return on caster only after impact completion", `${PLAYER}:2710-2763`),
  archer("PoisonShot", 135, ["spell_effects:PoisonShot"], [...ids(121, 0), ...ids(135, 2)], "AttackRange2 body7 arrow/impact; MirAction.Spell caster branch2300–2307 is distinct", `${PLAYER}:2201-2203,2710-2758`),
  archer("CrippleShot", 136, ["spell_effects:CrippleShot", "client_effects:ArcherCripplePoisonImpact", "client_effects:ArcherCrippleVampireImpact", "client_effects:ArcherVampireReturn"], [...ids(136, 0, 5, 6, 7), ...ids(45, 2)], "Typed caster buffs select impact; PoisonShot overrides VampireShot; return only Vampire variant", `${PLAYER}:2710-2752`),
  archer("SummonSnakes", 137, ["spell_effects:SummonSnakes", "client_effects:ArcherSnakeTotemAura", "client_effects:ArcherCharmedSnakeDeath"], [...ids(121, 0), ...ids(137, 7)], "AttackRange2 body7; server SnakeTotem image361 and CharmedSnake image362; Dead removes snake", `${PLAYER}:2656-2671;${MONSTER}:617-626,1125-1126,1488-1490`, [361, 362]),
  archer("BindingShot", 140, ["spell_effects:BindingShot", "client_effects:ArcherBindingDrop", "client_effects:ArcherBindingDown", "client_effects:ArcherBindingFall"], [...ids(121, 0), ...ids(140, 7), ...ids(130, 5, 6)], "AttackRange2 body7; SetBindingShot only monster target; authoritative duration minus1500; remove on expiry", `${PLAYER}:2656-2671;${GAME}:5595-5617;${MONSTER}:1224-1251`),
  archer("MentalState", 141, [], [], "Passive/typed mental mode alters attack logic, no standalone player effect", "Crystal/Server/MirObjects/HumanObject.cs:5504-5523", [], "No independent bitmap or sound in source MentalState mode"),
];

// Source gaps stay explicit; never replace them with a neighbouring sound.
export const FIVE_CLASS_MISSING_WAVS = ["M94-0.wav", "M94-1.wav", "M107-0.wav", "M107-1.wav", "M137-7.wav", "M140-7.wav"];
export const FIVE_CLASS_SKILL_SOUNDS = [...new Set(FIVE_CLASS_SKILL_SOURCE.flatMap((entry) => entry.soundIds))]
  .sort((a, b) => a - b).map((id) => {
    const fileName = id >= 20000 ? `M${Math.floor((id - 20000) / 10)}-${id % 10}.wav` : `${Math.floor(id / 10)}-${id % 10}.wav`;
    return { id, fileName, expectedSourceAbsent: FIVE_CLASS_MISSING_WAVS.includes(fileName) };
  });
export const FIVE_CLASS_SKILL_ACTORS = [
  { image: 130, library: "Monster/130", name: "TrapRock", frameCount: 36 },
  { image: 359, library: "Monster/359", name: "VampireSpider", frameCount: 301 },
  { image: 361, library: "Monster/361", name: "SnakeTotem", frameCount: 12 },
  { image: 362, library: "Monster/362", name: "CharmedSnake", frameCount: 52,
    sourceMissingFrames: [52, 53, 54, 55, 56, 57, 58, 59],
    sourceLifecycle: "Die has real effect frames40–47; source body frames52–59 are absent. MirAction.Dead removes CharmedSnake immediately." },
];
