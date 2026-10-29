function firstDefined(...values) {
  return values.find((value) => value !== undefined && value !== null);
}

function finiteNumber(value, fallback = null) {
  if (value === undefined || value === null || value === "") return fallback;
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function normalizedString(value) {
  return value === undefined || value === null ? null : String(value);
}

function normalizedEnum(value) {
  const string = normalizedString(value);
  return string === null ? null : string.trim().toLowerCase();
}

function parseStateList(value, label) {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  return value.map((entry, index) => {
    if (entry && typeof entry === "object" && !Array.isArray(entry)) return entry;
    if (typeof entry !== "string") {
      throw new Error(`${label}[${index}] must be a JSON object or encoded JSON object`);
    }
    try {
      const decoded = JSON.parse(entry);
      if (!decoded || typeof decoded !== "object" || Array.isArray(decoded)) {
        throw new Error("decoded value is not an object");
      }
      return decoded;
    } catch (error) {
      throw new Error(`${label}[${index}] is malformed: ${error instanceof Error ? error.message : String(error)}`);
    }
  });
}

function normalizeItem(item) {
  return {
    key: normalizedString(firstDefined(item?.key, item?.itemKey)) ?? "",
    uniqueId: finiteNumber(firstDefined(item?.uniqueId, item?.unique_id), 0),
    slot: finiteNumber(item?.slot, 0),
    container: normalizedEnum(item?.container) ?? "",
    quantity: finiteNumber(firstDefined(item?.quantity, item?.count), 1),
    icon: finiteNumber(item?.icon, 0),
    durabilityCurrent: finiteNumber(
      firstDefined(item?.durabilityCurrent, item?.durability_current),
      null,
    ),
    durabilityMax: finiteNumber(
      firstDefined(item?.durabilityMax, item?.durability_max),
      null,
    ),
  };
}

function normalizeEquipment(item) {
  return {
    key: normalizedString(item?.key) ?? "",
    uniqueId: finiteNumber(
      firstDefined(item?.uniqueId, item?.unique_id, item?.user_item_unique_id),
      null,
    ),
    slot: normalizedEnum(item?.slot) ?? "",
    quantity: finiteNumber(firstDefined(item?.quantity, item?.count), 1),
    icon: finiteNumber(item?.icon, 0),
    shape: finiteNumber(item?.shape, null),
    durabilityCurrent: finiteNumber(
      firstDefined(item?.durabilityCurrent, item?.durability_current),
      0,
    ),
    durabilityMax: finiteNumber(
      firstDefined(item?.durabilityMax, item?.durability_max),
      0,
    ),
  };
}

function normalizeQuest(quest) {
  return {
    questId: finiteNumber(firstDefined(quest?.questId, quest?.quest_id), 0),
    title: normalizedString(quest?.title) ?? "",
    stage: normalizedString(quest?.stage) ?? "",
    current: finiteNumber(quest?.current, 0),
    required: finiteNumber(quest?.required, 0),
  };
}

function normalizeSkill(skill) {
  return {
    key: normalizedString(skill?.key) ?? "",
    name: normalizedString(skill?.name) ?? "",
    level: finiteNumber(skill?.level, 0),
    experience: finiteNumber(skill?.experience, 0),
    hotkey: finiteNumber(skill?.hotkey, 0),
    delayMs: finiteNumber(firstDefined(skill?.delayMs, skill?.delay_ms), 0),
    castTimeMs: finiteNumber(firstDefined(skill?.castTimeMs, skill?.cast_time_ms), 0),
  };
}

function normalizeStats(stats) {
  if (!Array.isArray(stats)) return [];
  return stats
    .map((stat) => ({
      stat: finiteNumber(stat?.stat, 0),
      value: finiteNumber(stat?.value, 0),
    }))
    .sort((left, right) => left.stat - right.stat || left.value - right.value);
}

function normalizeBuff(buff) {
  return {
    key: normalizedString(buff?.key) ?? "",
    name: normalizedString(buff?.name) ?? "",
    attackBonus: finiteNumber(firstDefined(buff?.attackBonus, buff?.attack_bonus), 0),
    defenceBonus: finiteNumber(firstDefined(buff?.defenceBonus, buff?.defence_bonus), 0),
    stats: normalizeStats(buff?.stats),
  };
}

function sortByIdentity(values, identity) {
  return values.map(identity).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}

function normalizeCharacter(character) {
  if (!character || typeof character !== "object") return null;
  return {
    name: normalizedString(character.name) ?? "",
    level: finiteNumber(character.level, 0),
    class: normalizedEnum(firstDefined(character.classKey, character.class)) ?? "",
    gender: normalizedEnum(firstDefined(character.genderKey, character.gender)) ?? "",
  };
}

export function buildQaParityExpectation(payload) {
  const character = payload?.character ?? {};
  return {
    mapFileName: normalizedString(firstDefined(payload?.mapFileName, payload?.map_file_name)),
    position: {
      x: finiteNumber(firstDefined(payload?.position?.x, payload?.x), null),
      y: finiteNumber(firstDefined(payload?.position?.y, payload?.y), null),
    },
    direction: normalizedEnum(payload?.direction),
    character: normalizeCharacter(character),
    hp: finiteNumber(payload?.hp, null),
    maxHp: finiteNumber(firstDefined(payload?.maxHp, payload?.max_hp), null),
    mp: finiteNumber(payload?.mp, null),
    maxMp: finiteNumber(firstDefined(payload?.maxMp, payload?.max_mp), null),
    experience: finiteNumber(payload?.experience, null),
    maxExperience: finiteNumber(
      firstDefined(payload?.maxExperience, payload?.max_experience),
      null,
    ),
    gold: finiteNumber(payload?.gold, null),
    inventoryCapacity: finiteNumber(
      firstDefined(payload?.inventoryCapacity, payload?.inventory_capacity),
      46,
    ),
    inventoryItems: sortByIdentity(
      parseStateList(
        firstDefined(payload?.inventoryItemsJson, payload?.inventory_items_json),
        "inventoryItemsJson",
      ),
      normalizeItem,
    ),
    beltItems: sortByIdentity(
      parseStateList(firstDefined(payload?.beltItemsJson, payload?.belt_items_json), "beltItemsJson"),
      normalizeItem,
    ),
    storageItems: sortByIdentity(
      parseStateList(
        firstDefined(payload?.storageItemsJson, payload?.storage_items_json),
        "storageItemsJson",
      ),
      normalizeItem,
    ),
    equipmentItems: sortByIdentity(
      parseStateList(
        firstDefined(payload?.equipmentItemsJson, payload?.equipment_items_json),
        "equipmentItemsJson",
      ),
      normalizeEquipment,
    ),
    quests: sortByIdentity(
      parseStateList(firstDefined(payload?.questStatesJson, payload?.quest_states_json), "questStatesJson"),
      normalizeQuest,
    ),
    skills: sortByIdentity(
      parseStateList(firstDefined(payload?.skillStatesJson, payload?.skill_states_json), "skillStatesJson"),
      normalizeSkill,
    ),
    buffs: sortByIdentity(
      parseStateList(firstDefined(payload?.buffStatesJson, payload?.buff_states_json), "buffStatesJson"),
      normalizeBuff,
    ),
    stage5: {
      hair: finiteNumber(payload?.hair, 0),
      attackMode: finiteNumber(firstDefined(payload?.attackMode, payload?.attack_mode), 0),
      petMode: finiteNumber(firstDefined(payload?.petMode, payload?.pet_mode), 0),
      allowGroup: Boolean(firstDefined(payload?.allowGroup, payload?.allow_group, false)),
    },
  };
}

function canonicalActual(actual) {
  return {
    mapFileName: normalizedString(actual?.mapFileName),
    position: {
      x: finiteNumber(firstDefined(actual?.authoritativePlayer?.x, actual?.player?.x), null),
      y: finiteNumber(firstDefined(actual?.authoritativePlayer?.y, actual?.player?.y), null),
    },
    direction: normalizedEnum(
      firstDefined(actual?.authoritativePlayer?.direction, actual?.selfPlayer?.direction),
    ),
    character: normalizeCharacter(actual?.selfPlayer),
    hp: finiteNumber(actual?.playerHp, null),
    maxHp: finiteNumber(actual?.playerMaxHp, null),
    mp: finiteNumber(actual?.playerMp, null),
    maxMp: finiteNumber(actual?.playerMaxMp, null),
    experience: finiteNumber(actual?.playerExperience, null),
    maxExperience: finiteNumber(actual?.playerMaxExperience, null),
    gold: finiteNumber(actual?.gold, null),
    inventoryCapacity: finiteNumber(actual?.inventoryCapacity, null),
    inventoryItems: sortByIdentity(actual?.inventoryItems ?? [], normalizeItem),
    beltItems: sortByIdentity(actual?.beltItems ?? [], normalizeItem),
    storageItems: sortByIdentity(actual?.storageItems ?? [], normalizeItem),
    equipmentItems: sortByIdentity(actual?.equipmentItems ?? [], normalizeEquipment),
    quests: sortByIdentity(actual?.quests ?? actual?.questLog ?? [], normalizeQuest),
    skills: sortByIdentity(actual?.skills ?? actual?.knownSkills ?? [], normalizeSkill),
    buffs: sortByIdentity(actual?.buffs ?? actual?.activeBuffs ?? [], normalizeBuff),
    stage5: {
      hair: finiteNumber(firstDefined(actual?.stage5?.hair, actual?.stage5Systems?.appearance?.hair), 0),
      attackMode: finiteNumber(
        firstDefined(actual?.stage5?.attackMode, actual?.stage5Systems?.attackMode),
        0,
      ),
      petMode: finiteNumber(firstDefined(actual?.stage5?.petMode, actual?.stage5Systems?.petMode), 0),
      allowGroup: Boolean(
        firstDefined(actual?.stage5?.allowGroup, actual?.stage5Systems?.group?.allowGroup, false),
      ),
    },
  };
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

export function compareQaParityState(expected, actual) {
  const canonical = canonicalActual(actual);
  const mismatches = [];
  const compare = (path, expectedValue, actualValue, { optional = false } = {}) => {
    if (optional && (expectedValue === null || expectedValue === undefined)) return;
    if (stableJson(expectedValue) !== stableJson(actualValue)) {
      mismatches.push({ path, expected: expectedValue, actual: actualValue });
    }
  };

  compare("mapFileName", expected.mapFileName, canonical.mapFileName, { optional: true });
  compare("position", expected.position, canonical.position);
  compare("direction", expected.direction, canonical.direction, { optional: true });
  compare("character", expected.character, canonical.character, { optional: true });
  compare("hp", expected.hp, canonical.hp, { optional: true });
  compare("maxHp", expected.maxHp, canonical.maxHp, { optional: true });
  compare("mp", expected.mp, canonical.mp, { optional: true });
  compare("maxMp", expected.maxMp, canonical.maxMp, { optional: true });
  compare("experience", expected.experience, canonical.experience, { optional: true });
  compare("maxExperience", expected.maxExperience, canonical.maxExperience, { optional: true });
  compare("gold", expected.gold, canonical.gold, { optional: true });
  compare("inventoryCapacity", expected.inventoryCapacity, canonical.inventoryCapacity);
  compare("inventoryItems", expected.inventoryItems, canonical.inventoryItems);
  compare("beltItems", expected.beltItems, canonical.beltItems);
  compare("storageItems", expected.storageItems, canonical.storageItems);
  compare("equipmentItems", expected.equipmentItems, canonical.equipmentItems);
  compare("quests", expected.quests, canonical.quests);
  compare("skills", expected.skills, canonical.skills);
  compare("buffs", expected.buffs, canonical.buffs);
  compare("stage5", expected.stage5, canonical.stage5);

  return { ok: mismatches.length === 0, mismatches, actual: canonical };
}

export const __test = {
  parseStateList,
  normalizeItem,
  normalizeEquipment,
  normalizeQuest,
  normalizeSkill,
  normalizeBuff,
};
