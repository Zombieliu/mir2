import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_ACCOUNT_STORE_PATH = path.join(REPO_ROOT, ".mir2-data", "accounts.json");
const DEFAULT_ITEM_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_item_manifest.json",
);
const DEFAULT_RESPAWN_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const DEFAULT_QUEST_MANIFEST_PATH = path.join(
  REPO_ROOT, "packages", "game-data", "data", "generated", "crystal_quest_packet_manifest.json",
);
const DEFAULT_MAGIC_MANIFEST_PATH = path.join(
  REPO_ROOT, "packages", "game-data", "data", "generated", "crystal_magic_manifest.json",
);
const DEFAULT_STARTER_SERVER_DATA_PATH = path.join(
  REPO_ROOT, "packages", "game-data", "data", "starter_server_data.json",
);
const DEFAULT_EXP_LIST_PATH = path.resolve(
  REPO_ROOT,
  "..",
  "Crystal",
  "Build",
  "Server",
  "Debug",
  "Configs",
  "ExpList.ini",
);

const CLASS_NAMES = ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"];
const GENDER_NAMES = ["Male", "Female"];
const DIRECTION_NAMES = ["Up", "UpRight", "Right", "DownRight", "Down", "DownLeft", "Left", "UpLeft"];
const CRYSTAL_BELT_SIZE = 6;
const CRYSTAL_BASE_INVENTORY_CAPACITY = 46;
const CRYSTAL_MAX_INVENTORY_CAPACITY = 86;
const STORAGE_SIZE = 80;
const BUFF_KEY_BY_TYPE = new Map([
  [1, "temporal-flux"], [2, "hiding"], [3, "haste"], [4, "swift-feet"], [5, "battle-focus"],
  [6, "soul-shield"], [7, "blessed-armour"], [8, "light-body"], [9, "ultimate-enhancer"],
  [10, "protection-field"], [11, "rage"], [12, "curse"], [13, "moon-light"], [14, "dark-body"],
  [15, "concentration"], [16, "vampire-shot"], [17, "poison-shot"], [18, "counter-attack"],
  [19, "mental-state"], [20, "energy-shield"], [21, "magic-booster"], [22, "pet-enhancer"],
  [23, "immortal-skin"], [24, "magic-shield"], [25, "elemental-barrier"],
  [101, "general"], [102, "exp"], [103, "drop"], [104, "gold"], [105, "bag-weight"],
  [106, "transform"], [107, "lover"], [108, "mentee"], [109, "mentor"], [110, "guild"],
  [111, "prison"], [112, "rested"], [113, "skill"], [114, "clear-ring"], [115, "newbie"],
  [200, "impact"], [201, "magic"], [202, "taoist"], [203, "storm"], [204, "health-aid"],
  [205, "mana-aid"], [206, "defence"], [207, "magic-defence"], [208, "wonder-drug"], [209, "knapsack"],
]);
const SPELL_NAME_BY_ID = new Map([
  [0, "None"],
  ...numberedNames(1, ["Fencing", "Slaying", "Thrusting", "HalfMoon", "ShoulderDash", "TwinDrakeBlade", "Entrapment", "FlamingSword", "LionRoar", "CrossHalfMoon", "BladeAvalanche", "ProtectionField", "Rage", "CounterAttack", "SlashingBurst", "Fury", "ImmortalSkin"]),
  ...numberedNames(31, ["FireBall", "Repulsion", "ElectricShock", "GreatFireBall", "HellFire", "ThunderBolt", "Teleport", "FireBang", "FireWall", "Lightning", "FrostCrunch", "ThunderStorm", "MagicShield", "TurnUndead", "Vampirism", "IceStorm", "FlameDisruptor", "Mirroring", "FlameField", "Blizzard", "MagicBooster", "MeteorStrike", "IceThrust", "FastMove", "StormEscape"]),
  [61, "Healing"], [62, "SpiritSword"], [63, "Poisoning"], [64, "SoulFireBall"], [65, "SummonSkeleton"],
  [67, "Hiding"], [68, "MassHiding"], [69, "SoulShield"], [70, "Revelation"], [71, "BlessedArmour"],
  [72, "EnergyRepulsor"], [73, "TrapHexagon"], [74, "Purification"], [75, "MassHealing"],
  [76, "Hallucination"], [77, "UltimateEnhancer"], [78, "SummonShinsu"], [79, "Reincarnation"],
  [80, "SummonHolyDeva"], [81, "Curse"], [82, "Plague"], [83, "PoisonCloud"], [84, "EnergyShield"],
  [85, "PetEnhancer"], [86, "HealingCircle"],
  ...numberedNames(91, ["FatalSword", "DoubleSlash", "Haste", "FlashDash", "LightBody", "HeavenlySword", "FireBurst", "Trap", "PoisonSword", "MoonLight", "MPEater", "SwiftFeet", "DarkBody", "Hemorrhage", "CrescentSlash", "MoonMist", "CatTongue"]),
  ...numberedNames(121, ["Focus", "StraightShot", "DoubleShot", "ExplosiveTrap", "DelayedExplosion", "Meditation", "BackStep", "ElementalShot", "Concentration", "Stonetrap", "ElementalBarrier", "SummonVampire", "VampireShot", "SummonToad", "PoisonShot", "CrippleShot", "SummonSnakes", "NapalmShot", "OneWithNature", "BindingShot", "MentalState"]),
  ...numberedNames(151, ["Blink", "Portal", "BattleCry", "FireBounce", "MeteorShower"]),
]);
const CRYSTAL_EXPERIENCE_FALLBACK = [
  100, 200, 300, 400, 600, 900, 1200, 1700, 2500, 6000,
  8000, 10000, 15000, 30000, 40000, 50000, 70000, 100000, 120000, 140000,
  250000, 300000, 350000, 400000, 500000, 700000, 1000000, 1400000, 1800000, 2000000,
  2400000, 2800000, 3200000, 3600000, 4000000, 4800000, 5600000, 8200000, 9000000, 12000000,
  16000000, 30000000, 50000000, 80000000, 120000000, 160000000, 200000000, 250000000, 300000000, 350000000,
];

const args = parseArgs(process.argv.slice(2));
const nativeStatePath = path.resolve(
  args.nativeState ?? args.nativeAccountState ?? args.input ?? "native-account-state.json",
);
const accountStorePath = path.resolve(args.accountStore ?? process.env.MIR2_ACCOUNT_STORE_PATH ?? DEFAULT_ACCOUNT_STORE_PATH);
const itemManifestPath = path.resolve(args.items ?? args.itemManifest ?? DEFAULT_ITEM_MANIFEST_PATH);
const respawnManifestPath = path.resolve(args.respawnManifest ?? DEFAULT_RESPAWN_MANIFEST_PATH);
const questManifestPath = path.resolve(args.questManifest ?? DEFAULT_QUEST_MANIFEST_PATH);
const magicManifestPath = path.resolve(args.magicManifest ?? DEFAULT_MAGIC_MANIFEST_PATH);
const starterServerDataPath = path.resolve(args.starterServerData ?? DEFAULT_STARTER_SERVER_DATA_PATH);
const expListPath = path.resolve(args.expList ?? args.expListPath ?? process.env.MIR2_CRYSTAL_EXP_LIST_PATH ?? DEFAULT_EXP_LIST_PATH);
const outputPath = args.output ? path.resolve(args.output) : null;
const qaStateOutputPath = args.qaStateOutput ? path.resolve(args.qaStateOutput) : null;
const accountFilter = args.account ?? process.env.MIR2_QA_ACCOUNT ?? null;
const characterFilter = args.characterName ?? args.character ?? process.env.MIR2_QA_CHARACTER ?? null;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? null;
const mapOverride = args.map ?? null;
const xOverride = numberArg(args.x, null);
const yOverride = numberArg(args.y, null);
const writeStore = booleanArg(args.writeStore ?? args.write, false);
const strictUiState = booleanArg(args.strictUiState ?? args.strict, false);

const isMainModule = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMainModule) await main();

async function main() {
  const [nativeState, itemManifest, respawnManifest, questManifest, magicManifest, starterServerData, existingStore, expListText] = await Promise.all([
    readJson(nativeStatePath),
    readJson(itemManifestPath),
    readJson(respawnManifestPath),
    readJson(questManifestPath),
    readJson(magicManifestPath),
    readJson(starterServerDataPath),
    readJsonIfExists(accountStorePath),
    readTextIfExists(expListPath),
  ]);
  const experienceList = parseCrystalExperienceList(expListText);

  const nativeAccount = nativeState.account;
  if (!nativeState.ok || !nativeAccount) {
    throw new Error(`Native account state is not usable: ${nativeStatePath}`);
  }

  const accountId = accountFilter ?? nativeAccount.accountID;
  if (!accountFilter && /^\[redacted:/.test(String(accountId))) {
    throw new Error("A redacted native state requires an explicit --account target.");
  }
  const nativeCharacter = selectNativeCharacter(nativeAccount, characterFilter);
  const itemByIndex = new Map((itemManifest.items ?? []).map((item) => [item.item_index, item]));
  const mapByIndex = new Map((respawnManifest.maps ?? []).map((map) => [map.map_index, map]));
  const questByIndex = new Map((questManifest.quests ?? []).map((quest) => [Number(quest.index), quest]));
  const magicBySpell = new Map((magicManifest.magics ?? []).map((magic) => [String(magic.spell), magic]));
  const starterSkillBySpell = new Map(
    (starterServerData.skills ?? [])
      .filter((skill) => skill.crystal_spell)
      .map((skill) => [String(skill.crystal_spell), skill]),
  );
  const strictBlockers = strictUiStateBlockers(
    nativeState,
    nativeAccount,
    nativeCharacter,
    itemByIndex,
    questByIndex,
    magicBySpell,
  );
  if (strictUiState && strictBlockers.length) {
    throw new Error(`Native UI state is not strictly representable: ${strictBlockers.join("; ")}`);
  }
  const mapInfo = mapByIndex.get(nativeCharacter.currentMapIndex) ?? {};
  const mapFileName = String(mapOverride ?? mapInfo.map_file_name ?? nativeCharacter.currentMapIndex ?? "0");
  const mapTitle = mapInfo.map_title ?? mapFileName;
  const position = {
    x: xOverride ?? nativeCharacter.currentLocation?.x ?? 0,
    y: yOverride ?? nativeCharacter.currentLocation?.y ?? 0,
  };
  const characterName = characterFilter ?? nativeCharacter.name;
  const characterClass = className(nativeCharacter.class);
  const characterGender = genderName(nativeCharacter.gender);
  const characterDirection = directionName(nativeCharacter.direction);
  const [maxHp, maxMp] = crystalBaseVitals(characterClass, nativeCharacter.level);
  const maxExperience = crystalMaxExperienceForLevel(nativeCharacter.level, experienceList);

  const store = normalizeAccountStore(existingStore);
  const existingAccountRecord = store.accounts[accountId] ?? null;
  if (writeStore && !existingAccountRecord && !password) {
    throw new Error(
      "Writing a new Candidate account requires MIR2_QA_PASSWORD or --password; no default password is used.",
    );
  }
  const accountRecord = existingAccountRecord ?? newAccountRecord(password ?? "");
  const existingCharacter = (accountRecord.characters ?? []).find((character) =>
    String(character.name ?? "").toLowerCase() === String(characterName).toLowerCase(),
  );
  const characterIndex = existingCharacter?.index ?? nextCharacterIndex(store);
  store.nextCharacterIndex = Math.max(Number(store.nextCharacterIndex ?? 0), characterIndex + 1);

  const characterRecord = {
    index: characterIndex,
    name: characterName,
    level: Math.max(1, Number(nativeCharacter.level) || 1),
    class: characterClass,
    gender: characterGender,
  };

  const inventoryItems = (nativeCharacter.bagItems ?? []).map((item) =>
    itemStateFromNative(item, itemByIndex, {
      ...candidateBagLocationFromCrystalSlot(item.slot),
    }),
  );
  inventoryItems.push(
    ...(nativeCharacter.questInventoryItems ?? []).map((item) =>
      itemStateFromNative(item, itemByIndex, {
        container: "quest",
        slot: clampSlot(Number(item.slot) || 0, 0, 39),
      }),
    ),
  );
  const beltItems = (nativeCharacter.beltItems ?? []).map((item) =>
    itemStateFromNative(item, itemByIndex, {
      container: "belt",
      slot: clampSlot(Number(item.slot) || 0, 0, 5),
    }),
  );
  const storageItems = (nativeAccount.storage?.items ?? nativeAccount.storageItems ?? []).map((item) =>
    itemStateFromNative(item, itemByIndex, {
      container: "storage",
      slot: clampSlot(Number(item.slot) || 0, 0, 159),
    }),
  );
  const equipmentItems = (nativeCharacter.equipmentItems ?? nativeCharacter.equipment?.items ?? []).map((item) =>
    equipmentStateFromNative(item, itemByIndex),
  );
  const questStates = questStatesFromNative(nativeCharacter, questByIndex);
  const skillStates = (nativeCharacter.magics ?? []).map((magic) =>
    skillStateFromNative(magic, magicBySpell, starterSkillBySpell),
  );
  const npcFlagStates = (nativeCharacter.flags ?? []).map((index) => ({ index, value: true }));
  const buffStates = (nativeCharacter.buffs ?? []).map(buffStateFromNative);
  const stage5Systems = stage5SystemsFromExisting(
    accountRecord.saves?.[String(characterIndex)]?.stage5_systems_json,
    nativeCharacter,
  );

  const save = {
    character: characterRecord,
    map_file_name: mapFileName,
    map_title: mapTitle,
    position,
    direction: characterDirection,
    hp: clampNumber(nativeCharacter.hp, 1, maxHp),
    max_hp: maxHp,
    mp: clampNumber(nativeCharacter.mp, 0, maxMp),
    max_mp: maxMp,
    experience: numberFromString(nativeCharacter.experience, 0),
    max_experience: maxExperience,
    gold: Math.max(0, Number(nativeAccount.gold ?? 0) || 0),
    credit: Math.max(0, Number(nativeAccount.credit ?? 0) || 0),
    city_currencies: {},
    pk_points: Math.trunc(Number(nativeCharacter.pkPoints ?? 0) || 0),
    chat_banned: false,
    chat_ban_until_ms: null,
    inventory_capacity: Number(
      nativeCharacter.inventoryCapacity ?? CRYSTAL_BASE_INVENTORY_CAPACITY,
    ),
    inventory_items_json: inventoryItems.map((item) => JSON.stringify(item)),
    belt_items_json: beltItems.map((item) => JSON.stringify(item)),
    hero_inventory_items_json: [],
    storage_items_json: storageItems.map((item) => JSON.stringify(item)),
    equipment_items_json: equipmentItems.map((item) => JSON.stringify(item)),
    equipment_items_explicit_empty: true,
    quest_states_json: questStates.map((state) => JSON.stringify(state)),
    skill_states_json: skillStates.map((state) => JSON.stringify(state)),
    buff_states_json: buffStates.map((state) => JSON.stringify(state)),
    npc_flag_states_json: npcFlagStates.map((state) => JSON.stringify(state)),
    npc_saved_values_json: [],
    npc_buy_back_items_json: [],
    npc_used_goods_items_json: [],
    item_rental_records_json: [],
    has_rented_item: Boolean(nativeCharacter.hasRentedItem),
    stage5_systems_json: JSON.stringify(stage5Systems),
  };

  if (!accountRecord.password && password) accountRecord.password = password;
  accountRecord.storage_size = Number(nativeAccount.storageCapacity ?? STORAGE_SIZE) || STORAGE_SIZE;
  accountRecord.has_expanded_storage = Boolean(nativeAccount.hasExpandedStorage);
  accountRecord.expanded_storage_expiry_time_binary_datetime = numberFromString(
    nativeAccount.expandedStorageExpiryDateBinary,
    0,
  );
  accountRecord.storage_password = accountRecord.storage_password ?? "";
  accountRecord.storage_password_last_set_binary_datetime =
    accountRecord.storage_password_last_set_binary_datetime ?? 0;
  accountRecord.is_banned = Boolean(accountRecord.is_banned);
  accountRecord.ban_reason = accountRecord.ban_reason ?? "";
  accountRecord.ban_until_ms = accountRecord.ban_until_ms ?? null;
  accountRecord.banned_at_ms = accountRecord.banned_at_ms ?? null;
  accountRecord.gm_level = accountRecord.gm_level ?? 0;
  accountRecord.characters = upsertByIndex(accountRecord.characters ?? [], characterRecord);
  accountRecord.saves = accountRecord.saves ?? {};
  accountRecord.saves[String(characterIndex)] = save;
  store.accounts[accountId] = accountRecord;

  if (writeStore) {
    await writeJson(accountStorePath, store);
  }

  const qaCharacterState = {
    character: characterRecord,
    mapFileName,
    mapTitle,
    position,
    direction: characterDirection,
    hp: save.hp,
    maxHp,
    mp: save.mp,
    maxMp,
    experience: save.experience,
    maxExperience: save.max_experience,
    gold: save.gold,
    credit: save.credit,
    pkPoints: save.pk_points,
    inventoryItemsJson: save.inventory_items_json,
    beltItemsJson: save.belt_items_json,
    storageItemsJson: save.storage_items_json,
    equipmentItemsJson: save.equipment_items_json,
    questStatesJson: save.quest_states_json,
    skillStatesJson: save.skill_states_json,
    npcFlagStatesJson: save.npc_flag_states_json,
    buffStatesJson: save.buff_states_json,
    hair: Number(nativeCharacter.hair ?? 0) || 0,
    inventoryCapacity: Number(nativeCharacter.inventoryCapacity ?? 46) || 46,
    attackMode: Number(nativeCharacter.attackMode ?? 0) || 0,
    petMode: Number(nativeCharacter.petMode ?? 0) || 0,
    allowGroup: Boolean(nativeCharacter.allowGroup),
  };

  const result = {
    ok: true,
    generatedAt: new Date().toISOString(),
    nativeStatePath,
    accountStorePath,
    wroteAccountStore: writeStore,
    createdAccount: !existingAccountRecord,
    requiresPasswordForWrite: !existingAccountRecord && !password,
    account: accountId,
    characterName,
    characterIndex,
    map: { fileName: mapFileName, title: mapTitle, position, direction: characterDirection },
    summary: {
      level: characterRecord.level,
      hp: `${save.hp}/${save.max_hp}`,
      mp: `${save.mp}/${save.max_mp}`,
      experience: `${save.experience}/${save.max_experience}`,
      gold: save.gold,
      inventoryItemCount: inventoryItems.length,
      beltItemCount: beltItems.length,
      storageItemCount: storageItems.length,
      equipmentItemCount: equipmentItems.length,
      questInventoryItemCount: nativeCharacter.questInventoryItems?.length ?? 0,
      questStateCount: questStates.length,
      skillStateCount: skillStates.length,
      buffStateCount: buffStates.length,
      hair: Number(nativeCharacter.hair ?? 0) || 0,
    },
    strictUiState: {
      enabled: strictUiState,
      passed: strictBlockers.length === 0,
      blockers: strictBlockers,
    },
    experienceList: {
      path: expListPath,
      loaded: Boolean(expListText),
      fallback: !expListText,
      count: experienceList.length,
    },
    qaCharacterState,
  };

  if (qaStateOutputPath) {
    await writeJson(qaStateOutputPath, qaCharacterState);
  }
  if (outputPath) {
    await writeJson(outputPath, result);
  }
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

function numberedNames(start, names) {
  return names.map((name, index) => [start + index, name]);
}

function candidateBagLocationFromCrystalSlot(rawSlotValue) {
  const rawSlot = Number(rawSlotValue);
  if (!Number.isInteger(rawSlot) || rawSlot < CRYSTAL_BELT_SIZE || rawSlot >= CRYSTAL_MAX_INVENTORY_CAPACITY) {
    throw new Error(`Crystal bag slot ${rawSlotValue} is outside raw inventory slots 6..85`);
  }
  const logicalSlot = rawSlot - CRYSTAL_BELT_SIZE;
  return logicalSlot < 40
    ? { container: "bag1", slot: logicalSlot }
    : { container: "bag2", slot: logicalSlot - 40 };
}

function strictUiStateBlockers(nativeState, nativeAccount, nativeCharacter, itemByIndex, questByIndex, magicBySpell) {
  const blockers = [];
  const version = Number(nativeState.header?.version);
  const customVersion = Number(nativeState.header?.customVersion ?? 0);
  if (nativeState.schemaVersion !== "mir2-crystal-account-state-v2") {
    blockers.push(`schemaVersion must be mir2-crystal-account-state-v2, got ${nativeState.schemaVersion ?? "missing"}`);
  }
  if (nativeState.source?.fullyConsumed !== true) blockers.push("source database was not fully consumed");
  if (version !== 117 || customVersion !== 0) {
    blockers.push(`only audited Crystal database version 117/0 is supported, got ${version}/${customVersion}`);
  }

  const capacity = Number(nativeCharacter.inventoryCapacity);
  const legalCapacity = capacity === CRYSTAL_BASE_INVENTORY_CAPACITY
    || (capacity >= 54 && capacity <= CRYSTAL_MAX_INVENTORY_CAPACITY && (capacity - 54) % 4 === 0);
  if (!legalCapacity) blockers.push(`illegal Crystal inventory capacity ${nativeCharacter.inventoryCapacity}`);
  if (Number(nativeCharacter.questInventoryCapacity) !== 40) {
    blockers.push(`quest inventory capacity must be 40, got ${nativeCharacter.questInventoryCapacity ?? "missing"}`);
  }
  const storageCapacity = Number(nativeAccount.storageCapacity);
  if (![80, 160].includes(storageCapacity)) {
    blockers.push(`storage capacity must be 80 or 160, got ${nativeAccount.storageCapacity ?? "missing"}`);
  }
  if (nativeAccount.hasStoragePassword) blockers.push("storage password value is intentionally unavailable to the importer");

  const occupied = new Set();
  const uniqueIds = new Set();
  const checkItems = (items, container, minSlot, maxSlot) => {
    for (const item of items ?? []) {
      const slot = Number(item.slot);
      if (!Number.isInteger(slot) || slot < minSlot || slot > maxSlot) {
        blockers.push(`${container} slot ${item.slot} is outside ${minSlot}..${maxSlot}`);
      }
      const location = `${container}:${slot}`;
      if (occupied.has(location)) blockers.push(`duplicate occupied slot ${location}`);
      occupied.add(location);
      validateNativeItem(item, `${container}[${slot}]`, itemByIndex, blockers, uniqueIds);
    }
  };
  checkItems(nativeCharacter.beltItems, "belt", 0, 5);
  checkItems(nativeCharacter.bagItems, "inventory", 6, Math.max(5, capacity - 1));
  checkItems(nativeCharacter.questInventoryItems, "quest", 0, 39);
  checkItems(nativeCharacter.equipmentItems, "equipment", 0, 13);
  checkItems(nativeAccount.storageItems, "storage", 0, Math.max(0, storageCapacity - 1));

  for (const quest of nativeCharacter.currentQuests ?? []) {
    const template = questByIndex.get(Number(quest.index));
    if (!template) blockers.push(`current quest ${quest.index} is absent from the Crystal quest manifest`);
    else validateQuestTaskIdentity(quest, template, blockers);
  }
  for (const questId of nativeCharacter.completedQuests ?? []) {
    if (!questByIndex.has(Number(questId))) blockers.push(`completed quest ${questId} is absent from the Crystal quest manifest`);
  }
  for (const magic of nativeCharacter.magics ?? []) {
    const spellName = SPELL_NAME_BY_ID.get(Number(magic.spell));
    if (!spellName) blockers.push(`unknown Crystal spell id ${magic.spell}`);
    else if (!magicBySpell.has(spellName)) blockers.push(`spell ${spellName} is absent from the Crystal magic manifest`);
    if (magic.isTempSpell) blockers.push(`temporary spell ${spellName ?? magic.spell} cannot be durably projected`);
  }
  for (const buff of nativeCharacter.buffs ?? []) {
    if (!BUFF_KEY_BY_TYPE.has(Number(buff.type))) blockers.push(`unknown Crystal buff type ${buff.type}`);
    if ((buff.data ?? []).some((entry) => String(entry.valueBase64 ?? "").length > 0)) {
      blockers.push(`buff type ${buff.type} contains opaque data not represented by Candidate`);
    }
    if ((buff.values ?? []).length > 0) blockers.push(`buff type ${buff.type} contains legacy values not represented by Candidate`);
    validateSafeInteger(buff.expireTimeMs, `buff ${buff.type} expireTimeMs`, blockers, { allowNegative: false });
  }
  for (const index of nativeCharacter.flags ?? []) {
    if (!Number.isInteger(Number(index)) || Number(index) < 0 || Number(index) >= 1999) {
      blockers.push(`NPC flag index ${index} is outside 0..1998`);
    }
  }
  for (const [label, count] of [
    ["mail", nativeCharacter.mailCount], ["pet", nativeCharacter.petCount],
    ["intelligent creature", nativeCharacter.intelligentCreatureCount], ["friend", nativeCharacter.friendCount],
    ["rented item", nativeCharacter.rentedItemCount],
  ]) {
    if (Number(count ?? 0) > 0) blockers.push(`${label} records are not present in the exact Candidate projection`);
  }
  if (nativeCharacter.currentRefine) blockers.push("active refine state is not present in the exact Candidate projection");
  if ((nativeCharacter.heroes ?? []).some((hero) => hero !== null && Number(hero) > 0)) {
    blockers.push("hero state is not present in the exact Candidate projection");
  }
  if (nativeCharacter.thrusting || nativeCharacter.halfMoon || nativeCharacter.crossHalfMoon || nativeCharacter.doubleSlash) {
    blockers.push("Crystal persistent skill-toggle state is not yet represented by Candidate");
  }
  if (Number(nativeCharacter.mentalState ?? 0) !== 0) blockers.push("Crystal mental state is not yet represented by Candidate");
  if (nativeCharacter.allowTrade) blockers.push("Crystal allowTrade=true is not yet represented by Candidate");
  if (nativeCharacter.allowObserve) blockers.push("Crystal allowObserve=true is not yet represented by Candidate");
  validateSafeInteger(nativeAccount.expandedStorageExpiryDateBinary, "expanded storage expiry", blockers);
  return [...new Set(blockers)];
}

function validateNativeItem(item, label, itemByIndex, blockers, uniqueIds) {
  const itemIndex = Number(item.itemIndex);
  if (!Number.isInteger(itemIndex) || !itemByIndex.has(itemIndex)) blockers.push(`${label} has unknown item index ${item.itemIndex}`);
  validateSafeInteger(item.uniqueId, `${label} uniqueId`, blockers, { allowNegative: false });
  const uniqueId = String(item.uniqueId ?? "0");
  if (uniqueId !== "0") {
    if (uniqueIds.has(uniqueId)) blockers.push(`duplicate item uniqueId ${uniqueId}`);
    uniqueIds.add(uniqueId);
  }
  if (!Array.isArray(item.socketSlots)) blockers.push(`${label} is missing the exact socketSlots layout`);
  for (let index = 0; index < (item.socketSlots ?? []).length; index += 1) {
    const socket = item.socketSlots[index];
    if (!socket) continue;
    if (Number(socket.slot) !== index) blockers.push(`${label} socket ${index} reports slot ${socket.slot}`);
    validateNativeItem(socket, `${label}.socket[${index}]`, itemByIndex, blockers, uniqueIds);
  }
  for (const [field, value] of [
    ["expireInfo.expiryDateBinary", item.expireInfo?.expiryDateBinary],
    ["rentalInformation.expiryDateBinary", item.rentalInformation?.expiryDateBinary],
    ["sealedInfo.expiryDateBinary", item.sealedInfo?.expiryDateBinary],
    ["sealedInfo.nextSealDateBinary", item.sealedInfo?.nextSealDateBinary],
  ]) {
    if (value !== undefined && value !== null) validateSafeInteger(value, `${label} ${field}`, blockers);
  }
}

function validateSafeInteger(value, label, blockers, { allowNegative = true } = {}) {
  if (value === undefined || value === null || value === "") return;
  try {
    const integer = BigInt(value);
    if (!allowNegative && integer < 0n) blockers.push(`${label} must not be negative`);
    if (integer > BigInt(Number.MAX_SAFE_INTEGER) || integer < BigInt(Number.MIN_SAFE_INTEGER)) {
      blockers.push(`${label} cannot round-trip through JSON Number without precision loss`);
    }
  } catch {
    blockers.push(`${label} is not an integer`);
  }
}

function validateQuestTaskIdentity(progress, template, blockers) {
  const expectedKills = new Set((template.kill_tasks ?? []).map((task) => Number(task.monster_index)));
  const expectedItems = new Set((template.item_tasks ?? []).map((task) => Number(task.item_index)));
  const expectedFlags = new Set((template.flag_tasks ?? []).map((task) => Number(task.number)));
  for (const task of progress.killTasks ?? []) {
    if (!expectedKills.has(Number(task.monsterId))) blockers.push(`quest ${progress.index} has unknown kill task ${task.monsterId}`);
  }
  for (const task of progress.itemTasks ?? []) {
    if (!expectedItems.has(Number(task.itemId))) blockers.push(`quest ${progress.index} has unknown item task ${task.itemId}`);
  }
  for (const task of progress.flagTasks ?? []) {
    if (!expectedFlags.has(Number(task.number))) blockers.push(`quest ${progress.index} has unknown flag task ${task.number}`);
  }
}

function questStatesFromNative(nativeCharacter, questByIndex) {
  const states = [];
  const seen = new Set();
  for (const progress of nativeCharacter.currentQuests ?? []) {
    const questId = Number(progress.index);
    const template = questByIndex.get(questId);
    if (!template) continue;
    states.push(questStateFromNative(progress, template));
    seen.add(questId);
  }
  for (const rawQuestId of nativeCharacter.completedQuests ?? []) {
    const questId = Number(rawQuestId);
    if (seen.has(questId)) continue;
    const template = questByIndex.get(questId);
    if (!template) continue;
    const required = questRequired(template);
    states.push({
      quest_id: questId, title: template.name ?? `Quest ${questId}`,
      summary: template.group ?? template.name ?? `Quest ${questId}`,
      reward_preview: "Original Crystal quest reward.", required, current: required,
      stage: "completed", task_progress: completedQuestTaskProgress(template),
    });
  }
  return states.sort((left, right) => left.quest_id - right.quest_id);
}

function questStateFromNative(progress, template) {
  const taskProgress = {};
  for (const task of progress.killTasks ?? []) taskProgress[`kill:${Number(task.monsterId)}`] = Math.max(0, Number(task.count) || 0);
  for (const task of progress.itemTasks ?? []) taskProgress[`item:${Number(task.itemId)}`] = Math.max(0, Number(task.count) || 0);
  for (const task of progress.flagTasks ?? []) taskProgress[`flag:${Number(task.number)}`] = task.state ? 1 : 0;
  const required = questRequired(template);
  return {
    quest_id: Number(progress.index), title: template.name ?? `Quest ${progress.index}`,
    summary: template.group ?? template.name ?? `Quest ${progress.index}`,
    reward_preview: "Original Crystal quest reward.", required,
    current: progress.completed ? required : questCurrent(template, taskProgress),
    stage: progress.completed ? "readyToTurnIn" : "inProgress",
    task_progress: progress.completed ? completedQuestTaskProgress(template, taskProgress) : taskProgress,
  };
}

function questRequired(template) {
  const kill = (template.kill_tasks ?? []).reduce((sum, task) => sum + Math.max(1, Number(task.count) || 0), 0);
  const items = (template.item_tasks ?? []).reduce((sum, task) => sum + Math.max(1, Number(task.count) || 0), 0);
  return Math.max(1, kill + items + (template.flag_tasks ?? []).length);
}

function questCurrent(template, progress) {
  let total = 0;
  for (const task of template.kill_tasks ?? []) total += Math.min(Math.max(0, progress[`kill:${task.monster_index}`] ?? 0), Math.max(1, Number(task.count) || 0));
  for (const task of template.item_tasks ?? []) total += Math.min(Math.max(0, progress[`item:${task.item_index}`] ?? 0), Math.max(1, Number(task.count) || 0));
  for (const task of template.flag_tasks ?? []) total += Math.min(Math.max(0, progress[`flag:${task.number}`] ?? 0), 1);
  return Math.min(total, questRequired(template));
}

function completedQuestTaskProgress(template, existing = {}) {
  const progress = { ...existing };
  for (const task of template.kill_tasks ?? []) progress[`kill:${task.monster_index}`] = Math.max(1, Number(task.count) || 0);
  for (const task of template.item_tasks ?? []) progress[`item:${task.item_index}`] = Math.max(1, Number(task.count) || 0);
  for (const task of template.flag_tasks ?? []) progress[`flag:${task.number}`] = 1;
  return progress;
}

function skillStateFromNative(magic, magicBySpell, starterSkillBySpell = new Map()) {
  const spellName = SPELL_NAME_BY_ID.get(Number(magic.spell));
  const metadata = magicBySpell.get(spellName) ?? {};
  const starter = starterSkillBySpell.get(spellName);
  const level = clampSlot(magic.level, 0, 255);
  const delayMs = Math.max(1, (Number(metadata.delayBase) || 1) - (Number(metadata.delayReduction) || 0) * level);
  return {
    key: starter?.key ?? normalizeCrystalSkillKey(spellName ?? `spell-${magic.spell}`),
    name: starter?.name ?? metadata.name ?? spellName ?? `Spell ${magic.spell}`,
    description: starter?.description ?? `Crystal learned skill ${spellName ?? magic.spell}.`,
    level, experience: clampSlot(magic.experience, 0, 65535), hotkey: clampSlot(magic.key, 0, 255),
    cooldown_ticks: starter ? Math.max(1, Number(starter.cooldown_ticks) || 1) : Math.ceil(delayMs / 1000),
    delay_ms: delayMs, cooldown_ends_at: 0, cast_time_ms: 0,
  };
}

function normalizeCrystalSkillKey(spellName) {
  return String(spellName).split("")
    .map((character) => /[A-Za-z0-9]/.test(character) ? character.toLowerCase() : "-")
    .join("").replace(/^-+|-+$/g, "");
}

function buffStateFromNative(buff) {
  const key = BUFF_KEY_BY_TYPE.get(Number(buff.type)) ?? `crystal-buff-${buff.type}`;
  const remainingMs = Math.max(0, numberFromString(buff.expireTimeMs, 0));
  return {
    key,
    name: key.split("-").map((part) => part ? part[0].toUpperCase() + part.slice(1) : part).join(" "),
    description: `Crystal ${key} buff.`, expires_at_tick: Math.ceil(remainingMs / 1000),
    attack_bonus: 0, defence_bonus: 0,
    stats: (buff.stats ?? []).map((stat) => ({ stat: clampSlot(stat.stat, 0, 255), value: Math.trunc(Number(stat.value) || 0) })),
  };
}

function stage5SystemsFromExisting(encoded, nativeCharacter) {
  let systems = null;
  if (typeof encoded === "string" && encoded.trim()) {
    try { systems = JSON.parse(encoded); } catch { systems = null; }
  }
  if (!systems || typeof systems !== "object" || Array.isArray(systems)) systems = defaultStage5Systems();
  systems.group = systems.group && typeof systems.group === "object" ? systems.group : { allowGroup: true, members: [], lootMode: "free" };
  systems.group.allowGroup = Boolean(nativeCharacter.allowGroup);
  systems.appearance = systems.appearance && typeof systems.appearance === "object" ? systems.appearance : { hair: 0 };
  systems.appearance.hair = clampSlot(nativeCharacter.hair, 0, 255);
  systems.attackMode = clampSlot(nativeCharacter.attackMode, 0, 255);
  systems.petMode = clampSlot(nativeCharacter.petMode, 0, 255);
  return systems;
}

function defaultStage5Systems() {
  return {
    group: { allowGroup: true, members: [], lootMode: "free" },
    guild: { name: "", members: [], rank: "", permissions: [], chatLog: [], knownGuilds: [], activeWars: [], activeWarTicksRemaining: {}, alliedGuilds: [], allyCount: 0, allianceBroadcasts: [], warBroadcasts: [], notice: [], storageGold: 0, storageItems: {}, storageItemStates: {}, storageItemUsers: {} },
    social: { friends: [], blocked: [], memos: {} },
    relationship: { allowMarriage: true, partnerName: "", marriedDateBinaryDatetime: 0, mapName: "", marriedDays: 0, pendingRequestFrom: null, pendingDivorceFrom: null },
    mentor: { allowMentor: true, name: "", level: 0, online: false, menteeExp: 0, pendingRequestFrom: null, pendingRequestLevel: 0 },
    mail: [], gameShopIndividualPurchases: {}, economyProjectionEventIds: [], trade: null, auction: [],
    refine: { slots: {}, currentItem: null, refining: false, ready: false, pendingUniqueId: 0, pendingChance: 0, pendingStat: 0 },
    conquest: { castleOwner: "", activeWars: [], eventLog: [], taxRatePercent: 0, gold: 0, guards: [], walls: [], gates: [], openGates: [] },
    guildTerritory: { owned: false, mapFileName: "GA0", owner: "", leader: "", leader2: "", price: 0, rentalDaysLeft: 0, begin: 0, recallLog: [] },
    hero: null, heroLearnedMagics: [], profession: { miningLevel: 0, ore: 0, craftedItems: [] },
    appearance: { hair: 0 }, nameLists: [], intelligentCreatures: [],
    itemRental: { partnerName: null, fee: 0, days: 0, hasDepositedItem: false, depositedItemName: null, goldLocked: false, itemLocked: false, recordCount: 0, rentedItems: [] },
    attackMode: 0, petMode: 0, pkDecayElapsedTicks: 0,
  };
}

function selectNativeCharacter(account, filter) {
  const characters = account.characters ?? [];
  if (!characters.length) throw new Error(`Native account ${account.accountID} has no characters`);
  if (!filter) return characters[0];
  const match = characters.find((character) => String(character.name).toLowerCase() === String(filter).toLowerCase());
  if (!match) throw new Error(`Native account ${account.accountID} has no character named ${filter}`);
  return match;
}

function normalizeAccountStore(store) {
  const normalized = store && typeof store === "object" ? store : {};
  normalized.schemaVersion = Number(normalized.schemaVersion ?? normalized.schema_version ?? 2) || 2;
  normalized.nextCharacterIndex = Number(normalized.nextCharacterIndex ?? normalized.next_character_index ?? 0) || 0;
  normalized.accounts = normalized.accounts && typeof normalized.accounts === "object" ? normalized.accounts : {};
  return normalized;
}

function newAccountRecord(accountPassword) {
  return {
    password: accountPassword,
    storage_size: STORAGE_SIZE,
    has_expanded_storage: false,
    expanded_storage_expiry_time_binary_datetime: 0,
    storage_password: "",
    storage_password_last_set_binary_datetime: 0,
    is_banned: false,
    ban_reason: "",
    ban_until_ms: null,
    banned_at_ms: null,
    gm_level: 0,
    characters: [],
    saves: {},
  };
}

function nextCharacterIndex(store) {
  const configured = Number(store.nextCharacterIndex ?? 0) || 0;
  const used = Object.values(store.accounts ?? {}).flatMap((account) =>
    (account.characters ?? []).map((character) => Number(character.index) || 0),
  );
  return Math.max(configured, ...used.map((value) => value + 1), 0);
}

function itemStateFromNative(nativeItem, itemByIndex, options) {
  const template = itemByIndex.get(Number(nativeItem.itemIndex)) ?? {};
  const key = crystalItemKey(nativeItem);
  const name = nativeItem.name ?? template.name ?? key;
  const durabilityCurrent = durabilityValue(nativeItem.currentDura, template.durability);
  const durabilityMax = durabilityValue(nativeItem.maxDura ?? template.durability, template.durability);
  const templateStats = templateStatsFor(template);
  const addedStats = addedStatsFor(nativeItem);
  const socketed = (nativeItem.socketSlots ?? nativeItem.socketItems ?? []).filter(Boolean).map((socketItem) =>
    itemStateFromNative(socketItem, itemByIndex, {
      container: options.container,
      slot: Number(socketItem.slot) || 0,
      capturedSocketPosition: Number(socketItem.slot) || 0,
    }),
  );

  return {
    key,
    name,
    icon: Number(nativeItem.image ?? template.image ?? 0) || 0,
    slot: clampSlot(options.slot, 0, 255),
    unique_id: uniqueId(nativeItem),
    container: options.container,
    quantity: Math.max(1, Number(nativeItem.count ?? 1) || 1),
    description: crystalItemDescription(name),
    durability_current: durabilityCurrent,
    durability_max: durabilityMax,
    weight: Math.max(0, Number(template.weight ?? 0) || 0),
    equip_slot: equipSlotForItem(nativeItem, template),
    grade: itemGrade(template.grade),
    added_attack: statValue(addedStats, 5),
    added_defence: statValue(addedStats, 1),
    added_stats: addedStats,
    socketed,
    user_item_metadata: userItemMetadataFromNative(nativeItem, options.capturedSocketPosition),
    cursed: Boolean(nativeItem.cursed),
    socket_slots: Math.max((nativeItem.socketSlots ?? []).length, Number(template.slots ?? 0) || 0),
    gem_count: Math.max(0, Number(nativeItem.gemCount ?? 0) || 0),
    identified: typeof nativeItem.identified === "boolean" ? nativeItem.identified : null,
    soul_bound_id: normalizeOptionalId(nativeItem.soulBoundId),
    sealed_expiry_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.expiryDateBinary, 0),
    sealed_next_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.nextSealDateBinary, 0),
    rental_binding_flags: Number(nativeItem.rentalInformation?.bindingFlags ?? 0) || 0,
    rental_owner_name: nativeItem.rentalInformation?.ownerName ?? "",
    rental_expiry_binary_datetime: numberFromString(nativeItem.rentalInformation?.expiryDateBinary, 0),
    rental_locked: Boolean(nativeItem.rentalInformation?.rentalLocked),
    attack: statValue(templateStats, 5),
    defence: statValue(templateStats, 1),
    heal_hp: statValue(templateStats, 12),
    heal_mp: statValue(templateStats, 13),
  };
}

function equipmentStateFromNative(nativeItem, itemByIndex) {
  const template = itemByIndex.get(Number(nativeItem.itemIndex)) ?? {};
  const key = crystalItemKey(nativeItem);
  const name = nativeItem.name ?? template.name ?? key;
  const templateStats = templateStatsFor(template);
  const addedStats = addedStatsFor(nativeItem);
  const socketed = (nativeItem.socketSlots ?? nativeItem.socketItems ?? []).filter(Boolean).map((socketItem) =>
    itemStateFromNative(socketItem, itemByIndex, {
      container: "bag1",
      slot: Number(socketItem.slot) || 0,
      capturedSocketPosition: Number(socketItem.slot) || 0,
    }),
  );
  return {
    key,
    slot: normalizeEquipmentSlot(nativeItem.equipmentSlot ?? equipSlotForItem(nativeItem, template) ?? nativeItem.slot),
    quantity: Math.max(1, Number(nativeItem.count ?? 1) || 1),
    name,
    icon: Number(nativeItem.image ?? template.image ?? 0) || 0,
    shape: template.shape === undefined || Number(template.shape) < 0 ? null : Number(template.shape),
    description: crystalItemDescription(name),
    durability_current: durabilityValue(nativeItem.currentDura, template.durability) ?? 0,
    durability_max: durabilityValue(nativeItem.maxDura ?? template.durability, template.durability) ?? 0,
    grade: itemGrade(template.grade),
    added_attack: statValue(addedStats, 5),
    added_defence: statValue(addedStats, 1),
    added_luck: statValue(addedStats, 15),
    added_stats: addedStats,
    socketed,
    cursed: Boolean(nativeItem.cursed),
    socket_slots: Math.max((nativeItem.socketSlots ?? []).length, Number(template.slots ?? 0) || 0),
    gem_count: Math.max(0, Number(nativeItem.gemCount ?? 0) || 0),
    awake_type: Math.max(0, Number(nativeItem.awake?.type ?? 0) || 0),
    awake_values: (nativeItem.awake?.values ?? []).map((value) => clampSlot(value, 0, 255)),
    user_item_metadata: userItemMetadataFromNative(nativeItem, null),
    user_item_unique_id: uniqueId(nativeItem),
    identified: typeof nativeItem.identified === "boolean" ? nativeItem.identified : null,
    soul_bound_id: normalizeOptionalId(nativeItem.soulBoundId),
    sealed_expiry_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.expiryDateBinary, 0),
    sealed_next_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.nextSealDateBinary, 0),
    rental_binding_flags: Number(nativeItem.rentalInformation?.bindingFlags ?? 0) || 0,
    rental_owner_name: nativeItem.rentalInformation?.ownerName ?? "",
    rental_expiry_binary_datetime: numberFromString(nativeItem.rentalInformation?.expiryDateBinary, 0),
    rental_locked: Boolean(nativeItem.rentalInformation?.rentalLocked),
    attack: statValue(templateStats, 5),
    defence: statValue(templateStats, 1),
  };
}

function templateStatsFor(template) {
  return (template.stats ?? []).map((stat) => ({
    stat: Number(stat.stat) || 0,
    value: Number(stat.value) || 0,
  })).filter((stat) => stat.stat >= 0 && stat.value !== 0);
}

function addedStatsFor(nativeItem) {
  return (nativeItem.addedStats ?? []).map((stat) => ({
    stat: Number(stat.stat) || 0,
    value: Number(stat.value) || 0,
  })).filter((stat) => stat.stat >= 0 && stat.value !== 0);
}

function userItemMetadataFromNative(nativeItem, capturedSocketPosition) {
  const rental = nativeItem.rentalInformation;
  const sealed = nativeItem.sealedInfo;
  const expire = nativeItem.expireInfo;
  return {
    item_index: Number(nativeItem.itemIndex),
    awake_type: clampSlot(nativeItem.awake?.type, 0, 255),
    awake_values: (nativeItem.awake?.values ?? []).map((value) => clampSlot(value, 0, 255)),
    refined_value: clampSlot(nativeItem.refinedValue, 0, 255),
    refine_added: clampSlot(nativeItem.refineAdded, 0, 255),
    refine_success_chance: Math.trunc(Number(nativeItem.refineSuccessChance) || 0),
    wedding_ring: Math.trunc(Number(nativeItem.weddingRing ?? -1)),
    expire_info: expire ? { expiry_binary_datetime: numberFromString(expire.expiryDateBinary, 0) } : null,
    rental_information: rental ? {
      owner_name: String(rental.ownerName ?? ""),
      binding_flags: Math.trunc(Number(rental.bindingFlags) || 0),
      expiry_binary_datetime: numberFromString(rental.expiryDateBinary, 0),
      rental_locked: Boolean(rental.rentalLocked),
    } : null,
    sealed_info: sealed ? {
      expiry_binary_datetime: numberFromString(sealed.expiryDateBinary, 0),
      next_seal_binary_datetime: numberFromString(sealed.nextSealDateBinary, 0),
    } : null,
    slots: [],
    is_shop_item: Boolean(nativeItem.isShopItem),
    gm_made: Boolean(nativeItem.gmMade),
    live_socketed_at_capture: true,
    socket_layout_hydrated: true,
    captured_socket_positions: (nativeItem.socketSlots ?? []).map((socket) => socket ? {
      unique_id: uniqueId(socket),
      item_index: Number(socket.itemIndex),
    } : null),
    captured_socket_position: capturedSocketPosition === undefined ? null : capturedSocketPosition,
  };
}

function equipSlotForItem(nativeItem, template) {
  if (nativeItem.equipmentSlot) return normalizeEquipmentSlot(nativeItem.equipmentSlot);
  switch (Number(template.item_type ?? nativeItem.itemType)) {
    case 1:
      return "weapon";
    case 2:
      return "armour";
    case 4:
      return "helmet";
    case 5:
      return "necklace";
    case 6:
      return "braceletLeft";
    case 7:
      return "ringLeft";
    case 8:
      return "amulet";
    case 9:
      return "belt";
    case 10:
      return "boots";
    case 11:
      return "stone";
    case 12:
      return "torch";
    case 14:
      return "mount";
    default:
      return null;
  }
}

function normalizeEquipmentSlot(value) {
  if (typeof value === "string") return value;
  const slots = [
    "weapon",
    "armour",
    "helmet",
    "torch",
    "necklace",
    "braceletLeft",
    "braceletRight",
    "ringLeft",
    "ringRight",
    "amulet",
    "belt",
    "boots",
    "stone",
    "mount",
  ];
  return slots[Number(value)] ?? "weapon";
}

function crystalItemKey(nativeItem) {
  return `crystal-item-${Number(nativeItem.itemIndex ?? nativeItem.item_index ?? 0) || 0}`;
}

function crystalItemDescription(name) {
  return `Crystal native account item: ${name}.`;
}

function durabilityValue(value, templateDurability) {
  const parsed = Number(value ?? 0) || 0;
  const template = Number(templateDurability ?? 0) || 0;
  if (parsed <= 0 && template <= 0) return null;
  return clampSlot(parsed, 0, 65535);
}

function uniqueId(nativeItem) {
  const value = Number(nativeItem.uniqueId ?? nativeItem.unique_id ?? 0);
  return Number.isFinite(value) && value > 0 ? value : 0;
}

function normalizeOptionalId(value) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) return null;
  return Math.trunc(parsed);
}

function statValue(stats, statId) {
  return stats
    .filter((stat) => stat.stat === statId)
    .reduce((sum, stat) => sum + stat.value, 0);
}

function itemGrade(value) {
  switch (Number(value) || 0) {
    case 1:
      return "common";
    case 2:
      return "rare";
    case 3:
      return "legendary";
    case 4:
      return "mythical";
    case 5:
      return "heroic";
    default:
      return "none";
  }
}

function className(value) {
  return CLASS_NAMES[Number(value)] ?? "Warrior";
}

function genderName(value) {
  return GENDER_NAMES[Number(value)] ?? "Male";
}

function directionName(value) {
  return DIRECTION_NAMES[Number(value)] ?? "Down";
}

function crystalBaseVitals(classValue, levelValue) {
  const level = Math.max(1, Number(levelValue) || 1);
  let hp;
  switch (classValue) {
    case "Wizard":
      hp = 14 + (level / 15 + 1.8) * level;
      break;
    case "Taoist":
      hp = 14 + (level / 6 + 2.5) * level;
      break;
    case "Assassin":
    case "Archer":
      hp = 14 + (level / 4 + 3.25) * level;
      break;
    case "Warrior":
    default:
      hp = 14 + (level / 4 + 4.5 + level / 20) * level;
      break;
  }

  let mp;
  switch (classValue) {
    case "Wizard":
      mp = 13 + (level / 5 + 2) * 2.2 * level;
      break;
    case "Taoist":
      mp = 13 + (level / 8) * 2.2 * level;
      break;
    case "Assassin":
      mp = 11 + level * 5;
      break;
    case "Archer":
      mp = 11 + level * 4;
      break;
    case "Warrior":
    default:
      mp = 11 + level * 3.5;
      break;
  }
  return [Math.max(1, Math.trunc(hp)), Math.max(0, Math.trunc(mp))];
}

function crystalMaxExperienceForLevel(levelValue, experienceList) {
  const level = Math.max(1, Math.trunc(Number(levelValue) || 1));
  const list = Array.isArray(experienceList) && experienceList.length ? experienceList : CRYSTAL_EXPERIENCE_FALLBACK;
  const value = list[level - 1];
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 1;
}

function parseCrystalExperienceList(text) {
  if (!text) return CRYSTAL_EXPERIENCE_FALLBACK;
  const values = [];
  for (const rawLine of text.replace(/^\uFEFF/, "").split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith(";") || line.startsWith("#") || line.startsWith("[")) continue;
    const match = /^Level(\d+)\s*=\s*(\d+)/i.exec(line);
    if (!match) continue;
    const level = Number(match[1]);
    const value = Number(match[2]);
    if (Number.isInteger(level) && level >= 1 && Number.isFinite(value)) {
      values[level - 1] = Math.trunc(value);
    }
  }
  return values.length ? values : CRYSTAL_EXPERIENCE_FALLBACK;
}

function upsertByIndex(items, next) {
  const result = items.filter((item) => Number(item.index) !== Number(next.index));
  result.push(next);
  return result.sort((left, right) => Number(left.index) - Number(right.index));
}

function clampNumber(value, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return min;
  return Math.min(Math.max(Math.trunc(parsed), min), max);
}

function clampSlot(value, min, max) {
  return Math.min(Math.max(Math.trunc(Number(value) || 0), min), max);
}

function numberFromString(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.trunc(parsed) : fallback;
}

async function readJson(filePath) {
  return JSON.parse((await fs.readFile(filePath, "utf8")).replace(/^\uFEFF/, ""));
}

async function readJsonIfExists(filePath) {
  try {
    return await readJson(filePath);
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
}

async function readTextIfExists(filePath) {
  try {
    return await fs.readFile(filePath, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") return null;
    throw error;
  }
}

async function writeJson(filePath, value) {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = "true";
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function numberArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export {
  buffStateFromNative,
  candidateBagLocationFromCrystalSlot,
  defaultStage5Systems,
  itemStateFromNative,
  questStateFromNative,
  questStatesFromNative,
  skillStateFromNative,
  stage5SystemsFromExisting,
  strictUiStateBlockers,
  userItemMetadataFromNative,
};
