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
const STORAGE_SIZE = 80;
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
const expListPath = path.resolve(args.expList ?? args.expListPath ?? process.env.MIR2_CRYSTAL_EXP_LIST_PATH ?? DEFAULT_EXP_LIST_PATH);
const outputPath = args.output ? path.resolve(args.output) : null;
const qaStateOutputPath = args.qaStateOutput ? path.resolve(args.qaStateOutput) : null;
const accountFilter = args.account ?? process.env.MIR2_QA_ACCOUNT ?? null;
const characterFilter = args.characterName ?? args.character ?? process.env.MIR2_QA_CHARACTER ?? null;
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Test123";
const mapOverride = args.map ?? null;
const xOverride = numberArg(args.x, null);
const yOverride = numberArg(args.y, null);
const writeStore = booleanArg(args.writeStore ?? args.write, true);

await main();

async function main() {
  const [nativeState, itemManifest, respawnManifest, existingStore, expListText] = await Promise.all([
    readJson(nativeStatePath),
    readJson(itemManifestPath),
    readJson(respawnManifestPath),
    readJsonIfExists(accountStorePath),
    readTextIfExists(expListPath),
  ]);
  const experienceList = parseCrystalExperienceList(expListText);

  const nativeAccount = nativeState.account;
  if (!nativeState.ok || !nativeAccount) {
    throw new Error(`Native account state is not usable: ${nativeStatePath}`);
  }

  const accountId = accountFilter ?? nativeAccount.accountID;
  const nativeCharacter = selectNativeCharacter(nativeAccount, characterFilter);
  const itemByIndex = new Map((itemManifest.items ?? []).map((item) => [item.item_index, item]));
  const mapByIndex = new Map((respawnManifest.maps ?? []).map((map) => [map.map_index, map]));
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
  const accountRecord = store.accounts[accountId] ?? newAccountRecord(password);
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
      container: "bag1",
      slot: clampSlot(Number(item.slot) || 0, 0, 79),
    }),
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
    inventory_items_json: inventoryItems.map((item) => JSON.stringify(item)),
    belt_items_json: beltItems.map((item) => JSON.stringify(item)),
    hero_inventory_items_json: [],
    storage_items_json: storageItems.map((item) => JSON.stringify(item)),
    equipment_items_json: equipmentItems.map((item) => JSON.stringify(item)),
    equipment_items_explicit_empty: true,
    quest_states_json: [],
    skill_states_json: [],
    npc_flag_states_json: [],
    npc_saved_values_json: [],
    npc_buy_back_items_json: [],
    npc_used_goods_items_json: [],
    item_rental_records_json: [],
    has_rented_item: Boolean(nativeCharacter.hasRentedItem),
    stage5_systems_json: null,
  };

  accountRecord.password = accountRecord.password || password;
  accountRecord.storage_size = Math.max(Number(accountRecord.storage_size ?? 0) || 0, STORAGE_SIZE);
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
    inventoryItemsJson: save.inventory_items_json,
    beltItemsJson: save.belt_items_json,
    storageItemsJson: save.storage_items_json,
    equipmentItemsJson: save.equipment_items_json,
  };

  const result = {
    ok: true,
    generatedAt: new Date().toISOString(),
    nativeStatePath,
    accountStorePath,
    wroteAccountStore: writeStore,
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
  const stats = statsFor(nativeItem, template);
  const socketed = (nativeItem.socketItems ?? []).map((socketItem, index) =>
    itemStateFromNative(socketItem, itemByIndex, {
      container: options.container,
      slot: index,
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
    added_attack: 0,
    added_defence: 0,
    added_stats: statsFor(nativeItem, { stats: [] }),
    socketed,
    cursed: Boolean(nativeItem.cursed),
    socket_slots: Math.max(socketed.length, Number(template.slots ?? 0) || 0),
    gem_count: Math.max(0, Number(nativeItem.gemCount ?? 0) || 0),
    identified: typeof nativeItem.identified === "boolean" ? nativeItem.identified : null,
    soul_bound_id: normalizeOptionalId(nativeItem.soulBoundId),
    sealed_expiry_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.expiryDateBinary, 0),
    sealed_next_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.nextSealDateBinary, 0),
    rental_binding_flags: Number(nativeItem.rentalInformation?.bindingFlags ?? 0) || 0,
    rental_owner_name: nativeItem.rentalInformation?.ownerName ?? "",
    rental_expiry_binary_datetime: numberFromString(nativeItem.rentalInformation?.expiryDateBinary, 0),
    rental_locked: Boolean(nativeItem.rentalInformation?.locked),
    attack: statValue(stats, 5),
    defence: statValue(stats, 1),
    heal_hp: statValue(stats, 12),
    heal_mp: statValue(stats, 13),
  };
}

function equipmentStateFromNative(nativeItem, itemByIndex) {
  const template = itemByIndex.get(Number(nativeItem.itemIndex)) ?? {};
  const key = crystalItemKey(nativeItem);
  const name = nativeItem.name ?? template.name ?? key;
  const stats = statsFor(nativeItem, template);
  const socketed = (nativeItem.socketItems ?? []).map((socketItem, index) =>
    itemStateFromNative(socketItem, itemByIndex, {
      container: "bag1",
      slot: index,
    }),
  );
  return {
    key,
    slot: normalizeEquipmentSlot(nativeItem.equipmentSlot ?? equipSlotForItem(nativeItem, template) ?? nativeItem.slot),
    name,
    icon: Number(nativeItem.image ?? template.image ?? 0) || 0,
    shape: template.shape === undefined || Number(template.shape) < 0 ? null : Number(template.shape),
    description: crystalItemDescription(name),
    durability_current: durabilityValue(nativeItem.currentDura, template.durability) ?? 0,
    durability_max: durabilityValue(nativeItem.maxDura ?? template.durability, template.durability) ?? 0,
    grade: itemGrade(template.grade),
    added_attack: 0,
    added_defence: 0,
    added_luck: statValue(statsFor(nativeItem, { stats: [] }), 15),
    added_stats: statsFor(nativeItem, { stats: [] }),
    socketed,
    cursed: Boolean(nativeItem.cursed),
    socket_slots: Math.max(socketed.length, Number(template.slots ?? 0) || 0),
    gem_count: Math.max(0, Number(nativeItem.gemCount ?? 0) || 0),
    awake_type: Math.max(0, Number(nativeItem.awake?.type ?? 0) || 0),
    awake_values: Array.from({ length: Math.max(0, Number(nativeItem.awake?.count ?? 0) || 0) }, () => 1),
    identified: typeof nativeItem.identified === "boolean" ? nativeItem.identified : null,
    soul_bound_id: normalizeOptionalId(nativeItem.soulBoundId),
    sealed_expiry_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.expiryDateBinary, 0),
    sealed_next_time_binary_datetime: numberFromString(nativeItem.sealedInfo?.nextSealDateBinary, 0),
    rental_binding_flags: Number(nativeItem.rentalInformation?.bindingFlags ?? 0) || 0,
    rental_owner_name: nativeItem.rentalInformation?.ownerName ?? "",
    rental_expiry_binary_datetime: numberFromString(nativeItem.rentalInformation?.expiryDateBinary, 0),
    rental_locked: Boolean(nativeItem.rentalInformation?.locked),
    attack: statValue(stats, 5),
    defence: statValue(stats, 1),
  };
}

function statsFor(nativeItem, template) {
  const templateStats = (template.stats ?? []).map((stat) => ({
    stat: Number(stat.stat) || 0,
    value: Number(stat.value) || 0,
  }));
  const addedStats = (nativeItem.addedStats ?? []).map((stat) => ({
    stat: Number(stat.stat) || 0,
    value: Number(stat.value) || 0,
  }));
  return [...templateStats, ...addedStats].filter((stat) => stat.stat >= 0 && stat.value !== 0);
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
