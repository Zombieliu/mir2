import { mkdirSync, readFileSync, writeFileSync, existsSync, readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const serverDbPath = process.argv[2] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Server.MirDB");
const routesRoot = process.argv[3] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Envir", "Routes");
const questsRoot = process.argv[4] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Envir", "Quests");
const npcsRoot = process.argv[5] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Envir", "NPCs");
const recipeRoot = process.argv[6] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Envir", "Recipe");
const mapRoot = process.argv[7] ?? resolve(dirname(serverDbPath), "Maps");
const configRoot = resolve(dirname(serverDbPath), "Configs");
const respawnOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const monsterOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_monster_manifest.json",
);
const itemOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_item_manifest.json",
);
const npcInfoOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_npc_info_manifest.json",
);
const questPacketOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_quest_packet_manifest.json",
);
const recipePacketOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_recipe_packet_manifest.json",
);
const gameShopPacketOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_game_shop_packet_manifest.json",
);
const baseStatsPacketOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_base_stats_packet_manifest.json",
);
const guildBuffPacketOutputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_guild_buff_packet_manifest.json",
);

const MIR_DIRECTIONS = [
  "Up",
  "UpRight",
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
];

function main() {
  const reader = new BinaryReader(readFileSync(serverDbPath));
  const version = reader.readInt32();
  const customVersion = reader.readInt32();
  const settings = readCrystalServerSettings();

  if (version <= 84) {
    throw new Error(`Unsupported Crystal DB version ${version}. This script expects the current v85+ save layout.`);
  }

  for (let index = 0; index < 8; index += 1) {
    reader.readInt32();
  }

  const maps = parseMaps(reader);
  const items = parseItems(reader);
  const monsterByIndex = parseMonsters(reader);
  const npcs = parseNpcs(reader, maps);
  const quests = parseQuests(reader, version, items, npcs, maps, settings);
  const recipes = parseRecipes(items);
  const gameShopInfos = parseGameShopInfos(reader, version, items);
  const baseStatsPackets = parseBaseStatsPackets();
  const guildBuffPacket = parseGuildBuffListPacket();
  const mapObjectInfo = buildMapObjectInfo(maps, npcs, settings);

  const manifestMaps = maps
    .map((map) => {
      const objectInfo = mapObjectInfo.get(map.map_index) ?? { safe_zone_spells: [] };
      const respawns = map.respawns.map((respawn) => {
        const monster = monsterByIndex.get(respawn.monster_index);
        return {
          ...respawn,
          monster_name: monster?.name ?? `Unknown#${respawn.monster_index}`,
          monster_image: monster?.image ?? 0,
          monster_ai: monster?.ai ?? 0,
          monster_view_range: monster?.view_range ?? 0,
          monster_hp: monster?.hp ?? 0,
          monster_attack_speed: monster?.attack_speed ?? 0,
          monster_move_speed: monster?.move_speed ?? 0,
          monster_can_push: monster?.can_push ?? false,
          monster_can_tame: monster?.can_tame ?? false,
          monster_auto_rev: monster?.auto_rev ?? false,
          monster_undead: monster?.undead ?? false,
          route: loadRoute(respawn.route_path),
        };
      });

      return {
        map_index: map.map_index,
        map_file_name: map.map_file_name,
        map_title: map.map_title,
        mini_map: map.mini_map,
        big_map: map.big_map,
        light: map.light,
        no_throw_item: map.no_throw_item,
        no_drop_player: map.no_drop_player,
        no_drop_monster: map.no_drop_monster,
        safe_zones: map.safe_zones,
        safe_zone_spells: objectInfo.safe_zone_spells,
        movement_count: map.movements.length,
        movements: map.movements,
        respawn_count: respawns.length,
        respawns,
      };
    });

  const manifestMonsters = [...monsterByIndex.values()].sort(
    (left, right) => left.monster_index - right.monster_index,
  );
  const manifestItems = items.sort((left, right) => left.item_index - right.item_index);

  const respawnManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    source_routes_dir: "Crystal/Build/Server/Debug/Envir/Routes",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_maps: manifestMaps.length,
    total_respawns: manifestMaps.reduce((sum, map) => sum + map.respawn_count, 0),
    maps: manifestMaps,
  };

  const monsterManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_monsters: manifestMonsters.length,
    monsters: manifestMonsters,
  };
  const itemManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_items: manifestItems.length,
    items: manifestItems,
  };
  const npcInfoManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_npcs: npcs.length,
    npcs,
  };
  const questPacketManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    source_quests_dir: "Crystal/Build/Server/Debug/Envir/Quests",
    source_npcs_dir: "Crystal/Build/Server/Debug/Envir/NPCs",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_quests: quests.length,
    quests,
  };
  const recipePacketManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    source_recipe_dir: "Crystal/Build/Server/Debug/Envir/Recipe",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_recipes: recipes.length,
    recipes,
  };
  const gameShopPacketManifest = {
    generated_at: new Date().toISOString(),
    source_file: "Crystal/Build/Server/Debug/Server.MirDB",
    crystal_db_version: version,
    crystal_db_custom_version: customVersion,
    total_items: gameShopInfos.length,
    items: gameShopInfos,
  };
  const baseStatsPacketManifest = {
    generated_at: new Date().toISOString(),
    source_configs_dir: "Crystal/Build/Server/Debug/Configs",
    total_classes: baseStatsPackets.length,
    classes: baseStatsPackets,
  };
  const guildBuffPacketManifest = {
    generated_at: new Date().toISOString(),
    source_config_file: "Crystal/Build/Server/Debug/Configs/GuildSettings.ini",
    payload_len: guildBuffPacket.length,
    payload_hex: guildBuffPacket.toString("hex"),
  };

  mkdirSync(dirname(respawnOutputPath), { recursive: true });
  writeFileSync(
    respawnOutputPath,
    `${JSON.stringify(respawnManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    monsterOutputPath,
    `${JSON.stringify(monsterManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    itemOutputPath,
    `${JSON.stringify(itemManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    npcInfoOutputPath,
    `${JSON.stringify(npcInfoManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    questPacketOutputPath,
    `${JSON.stringify(questPacketManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    recipePacketOutputPath,
    `${JSON.stringify(recipePacketManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    gameShopPacketOutputPath,
    `${JSON.stringify(gameShopPacketManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    baseStatsPacketOutputPath,
    `${JSON.stringify(baseStatsPacketManifest, null, 2)}\n`,
    "utf8",
  );
  writeFileSync(
    guildBuffPacketOutputPath,
    `${JSON.stringify(guildBuffPacketManifest, null, 2)}\n`,
    "utf8",
  );
  console.log(`Wrote Crystal respawn manifest to ${respawnOutputPath}`);
  console.log(`Wrote Crystal monster manifest to ${monsterOutputPath}`);
  console.log(`Wrote Crystal item manifest to ${itemOutputPath}`);
  console.log(`Wrote Crystal NPC info manifest to ${npcInfoOutputPath}`);
  console.log(`Wrote Crystal quest packet manifest to ${questPacketOutputPath}`);
  console.log(`Wrote Crystal recipe packet manifest to ${recipePacketOutputPath}`);
  console.log(`Wrote Crystal game shop packet manifest to ${gameShopPacketOutputPath}`);
  console.log(`Wrote Crystal base stats packet manifest to ${baseStatsPacketOutputPath}`);
  console.log(`Wrote Crystal guild buff packet manifest to ${guildBuffPacketOutputPath}`);
}

function parseMaps(reader) {
  const mapCount = reader.readInt32();
  const maps = [];

  for (let index = 0; index < mapCount; index += 1) {
    const map_index = reader.readInt32();
    const map_file_name = reader.readString();
    const map_title = reader.readString();

    const mini_map = reader.readUInt16();
    const light = reader.readUInt8();
    const big_map = reader.readUInt16();

    const safeZoneCount = reader.readInt32();
    const safe_zones = [];
    for (let safeIndex = 0; safeIndex < safeZoneCount; safeIndex += 1) {
      safe_zones.push({
        location: {
          x: reader.readInt32(),
          y: reader.readInt32(),
        },
        size: reader.readUInt16(),
        start_point: reader.readBoolean(),
      });
    }

    const respawnCount = reader.readInt32();
    const respawns = [];
    for (let respawnIndex = 0; respawnIndex < respawnCount; respawnIndex += 1) {
      respawns.push(parseRespawn(reader));
    }

    const movementCount = reader.readInt32();
    const movements = [];
    for (let movementIndex = 0; movementIndex < movementCount; movementIndex += 1) {
      movements.push({
        map_index: reader.readInt32(),
        source: {
          x: reader.readInt32(),
          y: reader.readInt32(),
        },
        destination: {
          x: reader.readInt32(),
          y: reader.readInt32(),
        },
        need_hole: reader.readBoolean(),
        need_move: reader.readBoolean(),
        conquest_index: reader.readInt32(),
        show_on_big_map: reader.readBoolean(),
        icon: reader.readInt32(),
      });
    }

    reader.readBoolean();
    reader.readBoolean();
    reader.readString();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    const no_throw_item = reader.readBoolean();
    const no_drop_player = reader.readBoolean();
    const no_drop_monster = reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readInt32();
    reader.readBoolean();
    reader.readInt32();
    reader.readUInt8();

    const mineZoneCount = reader.readInt32();
    for (let mineIndex = 0; mineIndex < mineZoneCount; mineIndex += 1) {
      reader.readInt32();
      reader.readInt32();
      reader.readUInt16();
      reader.readUInt8();
    }

    reader.readUInt8();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readUInt16();
    reader.readBoolean();
    reader.readBoolean();
    reader.readUInt16();
    reader.readBoolean();
    reader.readUInt8();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
    reader.readInt32();
    reader.readBoolean();
    reader.readBoolean();
    reader.readInt32();

    maps.push({
      map_index,
      map_file_name,
      map_title,
      mini_map,
      big_map,
      light,
      no_throw_item,
      no_drop_player,
      no_drop_monster,
      safe_zones,
      movements,
      respawns,
    });
  }

  return maps;
}

function parseRespawn(reader) {
  return {
    monster_index: reader.readInt32(),
    location: {
      x: reader.readInt32(),
      y: reader.readInt32(),
    },
    count: reader.readUInt16(),
    spread: reader.readUInt16(),
    delay_minutes: reader.readUInt16(),
    direction: MIR_DIRECTIONS[reader.readUInt8()] ?? "Up",
    route_path: nullableString(reader.readString()),
    random_delay_minutes: reader.readUInt16(),
    respawn_index: reader.readInt32(),
    save_respawn_time: reader.readBoolean(),
    respawn_ticks: reader.readUInt16(),
  };
}

function parseItems(reader) {
  const itemCount = reader.readInt32();
  const items = [];

  for (let index = 0; index < itemCount; index += 1) {
    const item_index = reader.readInt32();
    const name = reader.readString();
    const item_type = reader.readUInt8();
    const grade = reader.readUInt8();
    const required_type = reader.readUInt8();
    const required_class = reader.readUInt8();
    const required_gender = reader.readUInt8();
    const item_set = reader.readUInt8();
    const shape = reader.readInt16();
    const weight = reader.readUInt8();
    const light = reader.readUInt8();
    const required_amount = reader.readUInt8();
    const image = reader.readUInt16();
    const durability = reader.readUInt16();
    const stack_size = reader.readUInt16();
    const price = reader.readUInt32();
    const start_item = reader.readBoolean();
    const effect = reader.readUInt8();
    const bools = reader.readUInt8();
    const bind = reader.readInt16();
    const unique = reader.readInt16();
    const random_stats_id = reader.readUInt8();
    const can_fast_run = reader.readBoolean();
    const can_awakening = reader.readBoolean();
    const slots = reader.readUInt8();
    const stats = [...readStats(reader).entries()]
      .sort((left, right) => left[0] - right[0])
      .map(([stat, value]) => ({ stat, value }));
    const tooltip = reader.readBoolean() ? reader.readString() : null;

    items.push({
      item_index,
      name,
      item_type,
      grade,
      required_type,
      required_class,
      required_gender,
      item_set,
      shape,
      weight,
      light,
      required_amount,
      image,
      durability,
      stack_size,
      price,
      start_item,
      effect,
      need_identify: (bools & 0x01) === 0x01,
      show_group_pickup: (bools & 0x02) === 0x02,
      class_based: (bools & 0x04) === 0x04,
      level_based: (bools & 0x08) === 0x08,
      can_mine: (bools & 0x10) === 0x10,
      global_drop_notify: (bools & 0x20) === 0x20,
      bind,
      unique,
      random_stats_id,
      can_fast_run,
      can_awakening,
      slots,
      stats,
      tooltip,
    });
  }

  return items;
}

function parseMonsters(reader) {
  const monsterCount = reader.readInt32();
  const monsters = new Map();

  for (let index = 0; index < monsterCount; index += 1) {
    const monster_index = reader.readInt32();
    const name = reader.readString();
    const image = reader.readUInt16();
    const ai = reader.readUInt8();
    const effect = reader.readUInt8();
    const level = reader.readUInt16();
    const view_range = reader.readUInt8();
    const cool_eye = reader.readUInt8();
    const stats = readStats(reader);
    const hp = stats.get(12) ?? 0;
    const min_ac = stats.get(0) ?? 0;
    const max_ac = stats.get(1) ?? 0;
    const min_mac = stats.get(2) ?? 0;
    const max_mac = stats.get(3) ?? 0;
    const min_dc = stats.get(4) ?? 0;
    const max_dc = stats.get(5) ?? 0;
    const min_mc = stats.get(6) ?? 0;
    const max_mc = stats.get(7) ?? 0;
    const min_sc = stats.get(8) ?? 0;
    const max_sc = stats.get(9) ?? 0;
    const light = reader.readUInt8();
    const attack_speed = reader.readUInt16();
    const move_speed = reader.readUInt16();
    const experience = reader.readUInt32();
    const can_push = reader.readBoolean();
    const can_tame = reader.readBoolean();
    const auto_rev = reader.readBoolean();
    const undead = reader.readBoolean();
    const drop_path = nullableString(reader.readString());
    const can_recall = reader.readBoolean();
    const is_boss = reader.readBoolean();

    monsters.set(monster_index, {
      monster_index,
      name,
      image,
      ai,
      effect,
      level,
      view_range,
      cool_eye,
      hp,
      min_ac,
      max_ac,
      min_mac,
      max_mac,
      min_dc,
      max_dc,
      min_mc,
      max_mc,
      min_sc,
      max_sc,
      light,
      attack_speed,
      move_speed,
      experience,
      can_push,
      can_tame,
      auto_rev,
      undead,
      drop_path,
      can_recall,
      is_boss,
    });
  }

  return monsters;
}

function parseNpcs(reader, maps) {
  const mapFileByIndex = new Map(maps.map((map) => [map.map_index, map.map_file_name]));
  const npcCount = reader.readInt32();
  const npcs = [];

  for (let index = 0; index < npcCount; index += 1) {
    const npc_index = reader.readInt32();
    const map_index = reader.readInt32();
    const collect_quest_indexes = readInt32List(reader);
    const finish_quest_indexes = readInt32List(reader);
    const file_name = normalizePath(reader.readString());
    const name = reader.readString();
    const location = {
      x: reader.readInt32(),
      y: reader.readInt32(),
    };
    const image = reader.readUInt16();
    const rate = reader.readUInt16();
    const time_visible = reader.readBoolean();
    const hour_start = reader.readUInt8();
    const minute_start = reader.readUInt8();
    const hour_end = reader.readUInt8();
    const minute_end = reader.readUInt8();
    const min_level = reader.readInt16();
    const max_level = reader.readInt16();
    const day_of_week = reader.readString();
    const class_required = reader.readString();
    const conquest = reader.readInt32();
    const flag_needed = reader.readInt32();
    const show_on_big_map = reader.readBoolean();
    const big_map_icon = reader.readInt32();
    const can_teleport_to = reader.readBoolean();
    const conquest_visible = reader.readBoolean();

    npcs.push({
      npc_index,
      map_index,
      map_file_name: mapFileByIndex.get(map_index) ?? null,
      file_name,
      script_key: file_name,
      name,
      location,
      image,
      rate,
      price_rate: rate / 100,
      collect_quest_indexes,
      finish_quest_indexes,
      time_visible,
      hour_start,
      minute_start,
      hour_end,
      minute_end,
      min_level,
      max_level,
      day_of_week,
      class_required,
      conquest,
      flag_needed,
      show_on_big_map,
      big_map_icon,
      can_teleport_to,
      conquest_visible,
    });
  }

  return npcs.sort((left, right) => {
    const mapOrder = left.map_index - right.map_index;
    if (mapOrder !== 0) {
      return mapOrder;
    }
    return left.npc_index - right.npc_index;
  });
}

function parseQuests(reader, version, items, npcs, maps, settings) {
  const questCount = reader.readInt32();
  const itemByName = buildItemLookupByName(items);
  const npcObjectIdByIndex = assignNpcObjectIds(maps, npcs, settings);
  const questInfos = [];
  const questByIndex = new Map();

  for (let questIndex = 0; questIndex < questCount; questIndex += 1) {
    const index = reader.readInt32();
    const quest = {
      index,
      name: reader.readString(),
      group: reader.readString(),
      file_name: normalizePath(reader.readString()),
      required_min_level: reader.readInt32(),
      required_max_level: reader.readInt32(),
      required_quest: reader.readInt32(),
      required_class: reader.readUInt8(),
      type: reader.readUInt8(),
      goto_message: reader.readString(),
      kill_message: reader.readString(),
      item_message: reader.readString(),
      flag_message: reader.readString(),
      time_limit_in_seconds: version > 90 ? reader.readInt32() : 0,
      npc_index: 0,
      finish_npc_index: 0,
      description: [],
      task_description: [],
      return_description: [],
      completion_description: [],
      fixed_rewards: [],
      select_rewards: [],
      reward_gold: 0,
      reward_exp: 0,
      reward_credit: 0,
    };
    if (quest.required_max_level === 0) {
      quest.required_max_level = 65535;
    }
    loadQuestText(quest, itemByName);
    questInfos.push(quest);
    questByIndex.set(index, quest);
  }

  applyQuestNpcObjectIds(questByIndex, npcs, npcObjectIdByIndex);

  return questInfos.map((quest) => {
    const payload = encodeClientQuestInfo(quest);
    return {
      index: quest.index,
      name: quest.name,
      group: quest.group,
      file_name: quest.file_name,
      npc_index: quest.npc_index,
      finish_npc_index: quest.finish_npc_index || quest.npc_index,
      payload_len: payload.length,
      payload_hex: payload.toString("hex"),
    };
  });
}

function parseRecipes(items) {
  if (!existsSync(recipeRoot)) {
    return [];
  }

  const itemByName = buildItemLookupByName(items);
  let nextRecipeId = 0;
  return readdirSync(recipeRoot)
    .filter((fileName) => fileName.toLowerCase().endsWith(".txt"))
    .map((fileName) => {
      const name = fileName.replace(/\.txt$/i, "");
      const item = itemByName.get(normalizeItemLookupName(name));
      if (!item) {
        return null;
      }

      const recipe = {
        name,
        item,
        item_info_indices: [item.item_index],
        gold: 0,
        chance: 100,
        item_payload: createShopUserItem(item, ++nextRecipeId),
        tools: [],
        ingredients: [],
      };
      loadRecipeText(recipe, resolve(recipeRoot, fileName), itemByName);

      const payload = encodeClientRecipeInfo(recipe);
      return {
        name: recipe.name,
        item_index: recipe.item.item_index,
        item_name: recipe.item.name,
        gold: recipe.gold,
        chance: recipe.chance,
        item_info_indices: recipe.item_info_indices,
        tool_count: recipe.tools.length,
        ingredient_count: recipe.ingredients.length,
        payload_len: payload.length,
        payload_hex: payload.toString("hex"),
      };
    })
    .filter(Boolean);
}

function buildItemLookupByName(items) {
  const itemByName = new Map();
  for (const item of items) {
    const key = normalizeItemLookupName(item.name);
    if (!itemByName.has(key)) {
      itemByName.set(key, item);
    }
  }
  return itemByName;
}

function buildItemLookupByIndex(items) {
  const itemByIndex = new Map();
  for (const item of items) {
    itemByIndex.set(item.item_index, item);
  }
  return itemByIndex;
}

function parseGameShopInfos(reader, version, items) {
  skipDragonInfo(reader);
  skipMagicInfos(reader, version);

  const itemByIndex = buildItemLookupByIndex(items);
  const count = reader.readInt32();
  const gameShopInfos = [];
  for (let index = 0; index < count; index += 1) {
    const item = parseGameShopItem(reader, version, itemByIndex);
    if (!item?.info) {
      continue;
    }
    const stockLevel = item.stock === 0 ? 0 : item.stock;
    const payload = encodeGameShopInfo(item, stockLevel);
    gameShopInfos.push({
      item_index: item.item_index,
      game_shop_index: item.g_index,
      item_name: item.info.name,
      gold_price: item.gold_price,
      credit_price: item.credit_price,
      count: item.count,
      class: item.class,
      category: item.category,
      stock: item.stock,
      stock_level: stockLevel,
      payload_len: payload.length,
      payload_hex: payload.toString("hex"),
    });
  }
  return gameShopInfos;
}

const MIR_CLASSES = [
  { name: "Warrior", value: 0 },
  { name: "Wizard", value: 1 },
  { name: "Taoist", value: 2 },
  { name: "Assassin", value: 3 },
  { name: "Archer", value: 4 },
];

const STAT_VALUES = [
  ["MinAC", 0],
  ["MaxAC", 1],
  ["MinMAC", 2],
  ["MaxMAC", 3],
  ["MinDC", 4],
  ["MaxDC", 5],
  ["MinMC", 6],
  ["MaxMC", 7],
  ["MinSC", 8],
  ["MaxSC", 9],
  ["Accuracy", 10],
  ["Agility", 11],
  ["HP", 12],
  ["MP", 13],
  ["AttackSpeed", 14],
  ["Luck", 15],
  ["BagWeight", 16],
  ["HandWeight", 17],
  ["WearWeight", 18],
  ["Reflect", 19],
  ["Strong", 20],
  ["Holy", 21],
  ["Freezing", 22],
  ["PoisonAttack", 23],
  ["MagicResist", 30],
  ["PoisonResist", 31],
  ["HealthRecovery", 32],
  ["SpellRecovery", 33],
  ["PoisonRecovery", 34],
  ["CriticalRate", 35],
  ["CriticalDamage", 36],
  ["MaxACRatePercent", 40],
  ["MaxMACRatePercent", 41],
  ["MaxDCRatePercent", 42],
  ["MaxMCRatePercent", 43],
  ["MaxSCRatePercent", 44],
  ["AttackSpeedRatePercent", 45],
  ["HPRatePercent", 46],
  ["MPRatePercent", 47],
  ["HPDrainRatePercent", 48],
  ["ExpRatePercent", 100],
  ["ItemDropRatePercent", 101],
  ["GoldDropRatePercent", 102],
  ["MineRatePercent", 103],
  ["GemRatePercent", 104],
  ["FishRatePercent", 105],
  ["CraftRatePercent", 106],
  ["SkillGainMultiplier", 107],
  ["AttackBonus", 108],
  ["LoverExpRatePercent", 120],
  ["MentorDamageRatePercent", 121],
  ["MentorExpRatePercent", 123],
  ["DamageReductionPercent", 124],
  ["EnergyShieldPercent", 125],
  ["EnergyShieldHPGain", 126],
  ["ManaPenaltyPercent", 127],
  ["TeleportManaPenaltyPercent", 128],
  ["Hero", 129],
  ["Unknown", 255],
];

const STAT_BY_NAME = new Map(STAT_VALUES.map(([name, value]) => [name.toLowerCase(), value]));
const STAT_FORMULA_BY_NAME = new Map([
  ["health", 0],
  ["mana", 1],
  ["weight", 2],
  ["stat", 3],
]);

const GUILD_BUFF_STAT_KEYS = [
  ["BuffAc", "MaxAC"],
  ["BuffMAC", "MaxMAC"],
  ["BuffDc", "MaxDC"],
  ["BuffMc", "MaxMC"],
  ["BuffSc", "MaxSC"],
  ["BuffMaxHp", "HP"],
  ["BuffMaxMp", "MP"],
  ["BuffMineRate", "MineRatePercent"],
  ["BuffGemRate", "GemRatePercent"],
  ["BuffFishRate", "FishRatePercent"],
  ["BuffExpRate", "ExpRatePercent"],
  ["BuffCraftRate", "CraftRatePercent"],
  ["BuffSkillRate", "SkillGainMultiplier"],
  ["BuffHpRegen", "HealthRecovery"],
  ["BuffMpRegen", "SpellRecovery"],
  ["BuffAttack", "AttackBonus"],
  ["BuffDropRate", "ItemDropRatePercent"],
  ["BuffGoldRate", "GoldDropRatePercent"],
];

function parseBaseStatsPackets() {
  return MIR_CLASSES.map((job) => {
    const ini = readIniFile(resolve(configRoot, `BaseStats${job.name}.ini`));
    const baseStats = [];
    const caps = new Map();

    for (const [statName, statValue] of STAT_VALUES) {
      const formulaName = readIniString(ini, statName, "Formula", "");
      if (formulaName.length > 0) {
        const formula = STAT_FORMULA_BY_NAME.get(formulaName.toLowerCase());
        if (formula === undefined) {
          throw new Error(`Unsupported stat formula ${formulaName} in BaseStats${job.name}.ini`);
        }
        baseStats.push({
          stat: statValue,
          formula,
          base: readIniInt(ini, statName, "Base", 0),
          gain: readIniFloat(ini, statName, "Gain", 0),
          gain_rate: readIniFloat(ini, statName, "GainRate", 0),
          max: readIniInt(ini, statName, "Max", 0),
        });
      }

      const cap = readIniInt(ini, "Caps", statName, 0);
      if (cap !== 0) {
        caps.set(statValue, cap);
      }
    }

    const payload = encodeBaseStats(job.value, baseStats, caps);
    return {
      class: job.name,
      class_id: job.value,
      stat_count: baseStats.length,
      cap_count: caps.size,
      payload_len: payload.length,
      payload_hex: payload.toString("hex"),
    };
  });
}

function encodeBaseStats(job, baseStats, caps) {
  const writer = new BinaryWriter();
  writer.writeUInt8(job);
  writer.writeInt32(baseStats.length);
  for (const stat of baseStats) {
    writer.writeUInt8(stat.stat);
    writer.writeUInt8(stat.formula);
    writer.writeInt32(stat.base);
    writer.writeSingle(stat.gain);
    writer.writeSingle(stat.gain_rate);
    writer.writeInt32(stat.max);
  }
  writeStats(writer, caps);
  return writer.toBuffer();
}

function parseGuildBuffListPacket() {
  const ini = readIniFile(resolve(configRoot, "GuildSettings.ini"));
  const totalBuffs = readIniInt(ini, "Guilds", "TotalBuffs", 0);
  const writer = new BinaryWriter();
  writer.writeUInt8(0);
  writer.writeInt32(0);
  writer.writeInt32(totalBuffs);
  for (let index = 0; index < totalBuffs; index += 1) {
    const section = `Buff-${index}`;
    writer.writeInt32(readIniInt(ini, section, "Id", 0));
    writer.writeInt32(readIniInt(ini, section, "Icon", 0));
    writer.writeString(readIniString(ini, section, "Name", ""));
    writer.writeUInt8(readIniInt(ini, section, "LevelReq", 0));
    writer.writeUInt8(readIniInt(ini, section, "PointsReq", 1));
    writer.writeInt32(readIniInt(ini, section, "TimeLimit", 0));
    writer.writeInt32(readIniInt(ini, section, "ActivationCost", 0));
    writeStats(writer, parseGuildBuffStats(ini, section));
  }
  return writer.toBuffer();
}

function parseGuildBuffStats(ini, section) {
  const stats = new Map();
  for (const [key, statName] of GUILD_BUFF_STAT_KEYS) {
    const value = readIniInt(ini, section, key, 0);
    if (value === 0) {
      continue;
    }
    const stat = STAT_BY_NAME.get(statName.toLowerCase());
    if (stat === undefined) {
      throw new Error(`Unsupported guild buff stat ${statName}`);
    }
    stats.set(stat, value);
  }
  return stats;
}

function writeStats(writer, stats) {
  const entries = [...stats.entries()].sort((left, right) => left[0] - right[0]);
  writer.writeInt32(entries.length);
  for (const [stat, value] of entries) {
    writer.writeUInt8(stat);
    writer.writeInt32(value);
  }
}

function readIniFile(filePath) {
  const sections = new Map();
  let section = "";
  sections.set(section, new Map());

  if (!existsSync(filePath)) {
    return sections;
  }

  for (const rawLine of readFileSync(filePath, "utf8").split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith(";") || line.startsWith("#")) {
      continue;
    }
    if (line.startsWith("[") && line.endsWith("]")) {
      section = line.slice(1, -1);
      if (!sections.has(section.toLowerCase())) {
        sections.set(section.toLowerCase(), new Map());
      }
      continue;
    }
    const separator = line.indexOf("=");
    if (separator === -1) {
      continue;
    }
    sections
      .get(section.toLowerCase())
      .set(line.slice(0, separator).trim().toLowerCase(), line.slice(separator + 1).trim());
  }

  return sections;
}

function readIniString(ini, section, key, fallback) {
  return ini.get(section.toLowerCase())?.get(key.toLowerCase()) ?? fallback;
}

function readIniInt(ini, section, key, fallback) {
  const value = Number.parseInt(readIniString(ini, section, key, String(fallback)), 10);
  return Number.isFinite(value) ? value : fallback;
}

function readIniFloat(ini, section, key, fallback) {
  const value = Number.parseFloat(readIniString(ini, section, key, String(fallback)));
  return Number.isFinite(value) ? value : fallback;
}

function skipDragonInfo(reader) {
  reader.skip(1);
  reader.readString();
  reader.readString();
  reader.readString();
  reader.skip(6 * 4);
  reader.skip(12 * 8);
}

function skipMagicInfos(reader, version) {
  const count = reader.readInt32();
  for (let index = 0; index < count; index += 1) {
    reader.readString();
    reader.skip(7);
    reader.skip(3 * 2);
    reader.skip(2 * 4);
    reader.skip(4 * 2);
    if (version > 66) {
      reader.skip(1);
    }
    if (version > 70) {
      reader.skip(2 * 4);
    }
  }
}

function parseGameShopItem(reader, version, itemByIndex) {
  const item_index = reader.readInt32();
  const g_index = reader.readInt32();
  const gold_price = reader.readUInt32();
  const credit_price = reader.readUInt32();
  const count = version <= 84 ? reader.readUInt32() : reader.readUInt16();
  const item = {
    item_index,
    g_index,
    info: itemByIndex.get(item_index) ?? null,
    gold_price,
    credit_price,
    count,
    class: reader.readString(),
    category: reader.readString(),
    stock: reader.readInt32(),
    individual_stock: reader.readBoolean(),
    deal: reader.readBoolean(),
    top_item: reader.readBoolean(),
    date_binary: reader.readInt64(),
    can_buy_credit: false,
    can_buy_gold: false,
  };
  if (version > 105) {
    item.can_buy_credit = reader.readBoolean();
    item.can_buy_gold = reader.readBoolean();
  }
  return item;
}

function loadRecipeText(recipe, filePath, itemByName) {
  const lines = readFileSync(filePath, "utf8").split(/\r?\n/);
  let mode = "ingredients";

  for (const rawLine of lines) {
    if (rawLine.length === 0) {
      continue;
    }
    const line = rawLine.trim();
    if (line.length === 0) {
      continue;
    }
    if (line.startsWith("[")) {
      mode = line.substring(1, line.length - 1).toLowerCase();
      continue;
    }

    const data = line.split(/\s+/).filter(Boolean);
    if (data.length === 0) {
      continue;
    }

    switch (mode) {
      case "recipe":
        applyRecipeOption(recipe, data);
        break;
      case "tools":
        addRecipeTool(recipe, data, itemByName);
        break;
      case "ingredients":
        addRecipeIngredient(recipe, data, itemByName);
        break;
      default:
        break;
    }
  }
}

function applyRecipeOption(recipe, data) {
  if (data.length < 2) {
    return;
  }

  switch (data[0].toLowerCase()) {
    case "amount":
      recipe.item_payload.count = parseUInt16(data[1]);
      break;
    case "chance":
      recipe.chance = Math.min(parseUInt8(data[1]), 100);
      break;
    case "gold":
      recipe.gold = parseUInt32(data[1]);
      break;
    default:
      break;
  }
}

function addRecipeTool(recipe, data, itemByName) {
  const item = itemByName.get(normalizeItemLookupName(data[0]));
  if (!item) {
    return;
  }
  recipe.item_info_indices.push(item.item_index);
  recipe.tools.push(createShopUserItem(item, 0));
}

function addRecipeIngredient(recipe, data, itemByName) {
  const item = itemByName.get(normalizeItemLookupName(data[0]));
  if (!item) {
    return;
  }
  const ingredient = createShopUserItem(item, 0);
  const count = data.length >= 2 ? parseUInt16(data[1]) : 1;
  if (data.length >= 3) {
    ingredient.current_dura = parseUInt16(data[2]);
  }
  ingredient.count = Math.min(count, item.stack_size);
  recipe.item_info_indices.push(item.item_index);
  recipe.ingredients.push(ingredient);
}

function loadQuestText(quest, itemByName) {
  const filePath = resolve(questsRoot, `${quest.file_name}.txt`);
  if (!existsSync(filePath)) {
    return;
  }
  const lines = readFileSync(filePath, "utf8").split(/\r?\n/);
  const sections = [
    { key: "[@DESCRIPTION]", target: "description" },
    { key: "[@TASKDESCRIPTION]", target: "task_description" },
    { key: "[@COMPLETION]", target: "completion_description" },
    { key: "[@CARRYITEMS]", target: null },
    { key: "[@KILLTASKS]", target: null },
    { key: "[@ITEMTASKS]", target: null },
    { key: "[@FLAGTASKS]", target: null },
    { key: "[@FIXEDREWARDS]", target: "fixed_rewards" },
    { key: "[@SELECTREWARDS]", target: "select_rewards" },
    { key: "[@EXPREWARD]", target: "reward_exp" },
    { key: "[@GOLDREWARD]", target: "reward_gold" },
    { key: "[@CREDITREWARD]", target: "reward_credit" },
    { key: "[@RETURNDESCRIPTION]", target: "return_description" },
  ];

  for (const section of sections) {
    const start = lines.findIndex((line) => line.toUpperCase() === section.key);
    if (start === -1) {
      continue;
    }
    for (let lineIndex = start + 1; lineIndex < lines.length; lineIndex += 1) {
      const line = lines[lineIndex];
      if (line.startsWith("[") || line.startsWith("//")) {
        break;
      }
      if (line.length === 0) {
        continue;
      }
      switch (section.target) {
        case "description":
        case "task_description":
        case "return_description":
        case "completion_description":
          quest[section.target].push(line);
          break;
        case "fixed_rewards":
        case "select_rewards":
          parseQuestReward(quest[section.target], line, itemByName);
          break;
        case "reward_exp":
        case "reward_gold":
        case "reward_credit":
          quest[section.target] = parseUInt32(line);
          break;
        default:
          break;
      }
    }
  }
}

function parseQuestReward(target, line, itemByName) {
  if (line.length < 1) {
    return;
  }
  const split = line.split(" ");
  let count = 1;
  if (split.length > 1) {
    count = parseUInt16(split[1]);
  }
  const baseName = split[0];
  const direct = itemByName.get(normalizeItemLookupName(baseName));
  if (direct) {
    target.push({ item: direct, count });
    return;
  }
  const male = itemByName.get(normalizeItemLookupName(`${baseName}(M)`));
  if (male) {
    target.push({ item: male, count });
  }
  const female = itemByName.get(normalizeItemLookupName(`${baseName}(F)`));
  if (female) {
    target.push({ item: female, count });
  }
}

function assignNpcObjectIds(maps, npcs, settings) {
  const objectIdByNpcIndex = new Map();
  const cellCache = new Map();
  let objectId = 0;
  for (const map of maps) {
    const cells = loadCrystalMapCells(map.map_file_name, cellCache);
    if (!cells) {
      continue;
    }
    for (const npc of npcs) {
      if (npc.map_index !== map.map_index) {
        continue;
      }
      if (!validCrystalPoint(cells, npc.location.x, npc.location.y)) {
        continue;
      }
      objectId += 1;
      npc.loaded_object_id = objectId;
      objectIdByNpcIndex.set(npc.npc_index, objectId);
    }
    objectId += countSafeZoneSpellObjects(map.safe_zones, cells, settings);
  }
  return objectIdByNpcIndex;
}

function buildMapObjectInfo(maps, npcs, settings) {
  const mapObjectInfo = new Map();
  const cellCache = new Map();
  let objectId = 0;
  for (const map of maps) {
    const cells = loadCrystalMapCells(map.map_file_name, cellCache);
    if (!cells) {
      mapObjectInfo.set(map.map_index, { safe_zone_spells: [] });
      continue;
    }

    for (const npc of npcs) {
      if (npc.map_index !== map.map_index) {
        continue;
      }
      if (!validCrystalPoint(cells, npc.location.x, npc.location.y)) {
        continue;
      }
      objectId += 1;
      npc.loaded_object_id = objectId;
    }

    const safe_zone_spells = [];
    if (settings.safeZoneBorder) {
      for (const safeZone of map.safe_zones) {
        for (const location of collectSafeZoneBorderCells(safeZone, cells)) {
          objectId += 1;
          safe_zone_spells.push({
            object_id: objectId,
            location,
            spell: "TrapHexagon",
          });
        }
      }
    }
    if (settings.safeZoneHealing) {
      for (const safeZone of map.safe_zones) {
        for (const location of collectSafeZoneInteriorCells(safeZone, cells)) {
          objectId += 1;
          safe_zone_spells.push({
            object_id: objectId,
            location,
            spell: "Healing",
          });
        }
      }
    }
    mapObjectInfo.set(map.map_index, { safe_zone_spells });
  }
  return mapObjectInfo;
}

function readCrystalServerSettings() {
  const settings = {
    safeZoneBorder: false,
    safeZoneHealing: false,
  };
  const setupPath = resolve(dirname(serverDbPath), "Configs", "Setup.ini");
  if (!existsSync(setupPath)) {
    return settings;
  }

  for (const rawLine of readFileSync(setupPath, "utf8").split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith(";") || line.startsWith("#") || line.startsWith("[")) {
      continue;
    }
    const separator = line.indexOf("=");
    if (separator === -1) {
      continue;
    }
    const key = line.slice(0, separator).trim().toLowerCase();
    const value = parseCrystalBoolean(line.slice(separator + 1).trim());
    if (value === null) {
      continue;
    }
    if (key === "safezoneborder") {
      settings.safeZoneBorder = value;
    } else if (key === "safezonehealing") {
      settings.safeZoneHealing = value;
    }
  }

  return settings;
}

function parseCrystalBoolean(value) {
  if (/^true$/i.test(value)) {
    return true;
  }
  if (/^false$/i.test(value)) {
    return false;
  }
  return null;
}

function loadCrystalMapCells(mapFileName, cache) {
  const key = mapFileName.toLowerCase();
  if (cache.has(key)) {
    return cache.get(key);
  }

  const filePath = resolve(mapRoot, `${mapFileName}.map`);
  if (!existsSync(filePath)) {
    cache.set(key, null);
    return null;
  }

  const bytes = readFileSync(filePath);
  let cells = null;
  switch (findCrystalMapType(bytes)) {
    case 0:
      cells = loadMapCellsV0(bytes);
      break;
    case 1:
      cells = loadMapCellsV1(bytes);
      break;
    case 2:
      cells = loadMapCellsV2(bytes);
      break;
    case 3:
      cells = loadMapCellsV3(bytes);
      break;
    case 4:
      cells = loadMapCellsV4(bytes);
      break;
    case 5:
      cells = loadMapCellsV5(bytes);
      break;
    case 6:
      cells = loadMapCellsV6(bytes);
      break;
    case 7:
      cells = loadMapCellsV7(bytes);
      break;
    case 100:
      cells = loadMapCellsV100(bytes);
      break;
    default:
      break;
  }
  cache.set(key, cells);
  return cells;
}

function findCrystalMapType(bytes) {
  if (bytes[2] === 0x43 && bytes[3] === 0x23) {
    return 100;
  }
  if (bytes[0] === 0) {
    return 5;
  }
  if (bytes[0] === 0x0f && bytes[5] === 0x53 && bytes[14] === 0x33) {
    return 6;
  }
  if (bytes[0] === 0x15 && bytes[4] === 0x32 && bytes[6] === 0x41 && bytes[19] === 0x31) {
    return 4;
  }
  if (bytes[0] === 0x10 && bytes[2] === 0x61 && bytes[7] === 0x31 && bytes[14] === 0x31) {
    return 1;
  }
  if (
    bytes[4] === 0x0f ||
    (bytes[4] === 0x03 && bytes[18] === 0x0d && bytes[19] === 0x0a)
  ) {
    const width = bytes[0] + (bytes[1] << 8);
    const height = bytes[2] + (bytes[3] << 8);
    return bytes.length > 52 + width * height * 14 ? 3 : 2;
  }
  if (bytes[0] === 0x0d && bytes[1] === 0x4c && bytes[7] === 0x20 && bytes[11] === 0x6d) {
    return 7;
  }
  return 0;
}

function makeCellMap(width, height) {
  return {
    width,
    height,
    valid: new Uint8Array(width * height),
  };
}

function setCellValid(cells, x, y, valid) {
  cells.valid[x * cells.height + y] = valid ? 1 : 0;
}

function validCrystalPoint(cells, x, y) {
  return (
    x >= 0 &&
    x < cells.width &&
    y >= 0 &&
    y < cells.height &&
    cells.valid[x * cells.height + y] === 1
  );
}

function loadMapCellsV0(bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = makeCellMap(width, height);
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      let blocked = (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 4;
      offset += 3;
      offset += 1;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV1(bytes) {
  let offset = 21;
  const widthXor = bytes.readInt16LE(offset);
  offset += 2;
  const xor = bytes.readInt16LE(offset);
  offset += 2;
  const heightXor = bytes.readInt16LE(offset);
  const width = widthXor ^ xor;
  const height = heightXor ^ xor;
  const cells = makeCellMap(width, height);
  offset = 54;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      let blocked = ((bytes.readInt32LE(offset) ^ 0xaa38aa38) & 0x20000000) !== 0;
      offset += 6;
      blocked ||= ((bytes.readInt16LE(offset) ^ xor) & 0x8000) !== 0;
      offset += 2;
      offset += 5;
      offset += 1;
      offset += 1;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV2(bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = makeCellMap(width, height);
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      let blocked = (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      offset += 5;
      offset += 1;
      offset += 2;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV3(bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = makeCellMap(width, height);
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      let blocked = (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      offset += 12;
      offset += 1;
      offset += 17;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV4(bytes) {
  let offset = 31;
  const widthXor = bytes.readInt16LE(offset);
  offset += 2;
  const xor = bytes.readInt16LE(offset);
  offset += 2;
  const heightXor = bytes.readInt16LE(offset);
  const width = widthXor ^ xor;
  const height = heightXor ^ xor;
  const cells = makeCellMap(width, height);
  offset = 64;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      let blocked = (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 4;
      offset += 6;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV5(bytes) {
  let offset = 22;
  const width = bytes.readInt16LE(offset);
  offset += 2;
  const height = bytes.readInt16LE(offset);
  const cells = makeCellMap(width, height);
  offset = 28 + 3 * (Math.trunc(width / 2) + (width % 2)) * Math.trunc(height / 2);
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      const flag = bytes[offset];
      const blocked = (flag & 0x01) !== 1 || (flag & 0x02) !== 2;
      offset += 13;
      offset += 1;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV6(bytes) {
  let offset = 16;
  const width = bytes.readInt16LE(offset);
  offset += 2;
  const height = bytes.readInt16LE(offset);
  const cells = makeCellMap(width, height);
  offset = 40;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      const flag = bytes[offset];
      const blocked = (flag & 0x01) !== 1 || (flag & 0x02) !== 2;
      offset += 20;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV7(bytes) {
  let offset = 21;
  const width = bytes.readInt16LE(offset);
  offset += 4;
  const height = bytes.readInt16LE(offset);
  const cells = makeCellMap(width, height);
  offset = 54;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      let blocked = (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 6;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      offset += 4;
      offset += 1;
      offset += 2;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function loadMapCellsV100(bytes) {
  if (bytes[0] !== 1 || bytes[1] !== 0) {
    return null;
  }
  const width = bytes.readInt16LE(4);
  const height = bytes.readInt16LE(6);
  const cells = makeCellMap(width, height);
  let offset = 8;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      offset += 2;
      let blocked = (bytes.readInt32LE(offset) & 0x20000000) !== 0;
      offset += 10;
      blocked ||= (bytes.readInt16LE(offset) & 0x8000) !== 0;
      offset += 2;
      offset += 11;
      offset += 1;
      setCellValid(cells, x, y, !blocked);
    }
  }
  return cells;
}

function countSafeZoneSpellObjects(safeZones, cells, settings) {
  let count = 0;
  if (settings.safeZoneBorder) {
    for (const safeZone of safeZones) {
      count += countSafeZoneBorderCells(safeZone, cells);
    }
  }
  if (settings.safeZoneHealing) {
    for (const safeZone of safeZones) {
      count += countSafeZoneInteriorCells(safeZone, cells);
    }
  }
  return count;
}

function countSafeZoneBorderCells(safeZone, cells) {
  return collectSafeZoneBorderCells(safeZone, cells).length;
}

function collectSafeZoneBorderCells(safeZone, cells) {
  const locations = [];
  const { x: centerX, y: centerY } = safeZone.location;
  const size = safeZone.size;
  for (let y = centerY - size; y <= centerY + size; y += 1) {
    if (y < 0) {
      continue;
    }
    if (y >= cells.height) {
      break;
    }
    const step = Math.abs(y - centerY) === size ? 1 : size * 2;
    for (let x = centerX - size; x <= centerX + size; x += step) {
      if (x < 0) {
        continue;
      }
      if (x >= cells.width) {
        break;
      }
      if (validCrystalPoint(cells, x, y)) {
        locations.push({ x, y });
      }
    }
  }
  return locations;
}

function countSafeZoneInteriorCells(safeZone, cells) {
  return collectSafeZoneInteriorCells(safeZone, cells).length;
}

function collectSafeZoneInteriorCells(safeZone, cells) {
  const locations = [];
  const { x: centerX, y: centerY } = safeZone.location;
  const size = safeZone.size;
  for (let y = centerY - size; y <= centerY + size; y += 1) {
    if (y < 0) {
      continue;
    }
    if (y >= cells.height) {
      break;
    }
    for (let x = centerX - size; x <= centerX + size; x += 1) {
      if (x < 0) {
        continue;
      }
      if (x >= cells.width) {
        break;
      }
      if (validCrystalPoint(cells, x, y)) {
        locations.push({ x, y });
      }
    }
  }
  return locations;
}

function applyQuestNpcObjectIds(questByIndex, npcs, npcObjectIdByIndex) {
  for (const npc of npcs) {
    const objectId = npcObjectIdByIndex.get(npc.npc_index);
    if (!objectId) {
      continue;
    }
    const scriptPath = resolve(npcsRoot, `${npc.file_name}.txt`);
    if (!existsSync(scriptPath)) {
      continue;
    }
    const lines = readFileSync(scriptPath, "utf8").split(/\r?\n/);
    const start = lines.findIndex((line) => line.toUpperCase() === "[QUESTS]");
    if (start === -1) {
      continue;
    }
    for (let lineIndex = start + 1; lineIndex < lines.length; lineIndex += 1) {
      const line = lines[lineIndex].trim();
      if (line.startsWith("[") || line.startsWith("//")) {
        break;
      }
      if (line.length === 0) {
        continue;
      }
      const questIndex = Number.parseInt(line, 10);
      if (!Number.isFinite(questIndex) || questIndex === 0) {
        continue;
      }
      const quest = questByIndex.get(Math.abs(questIndex));
      if (!quest) {
        continue;
      }
      if (questIndex > 0) {
        quest.npc_index = objectId;
      } else {
        quest.finish_npc_index = objectId;
      }
    }
  }
}

function encodeClientQuestInfo(quest) {
  const writer = new BinaryWriter();
  writer.writeInt32(quest.index);
  writer.writeUInt32(quest.npc_index);
  writer.writeString(quest.name);
  writer.writeString(quest.group);
  writeStringList(writer, quest.description);
  writeStringList(writer, quest.task_description);
  writeStringList(writer, quest.return_description);
  writeStringList(writer, quest.completion_description);
  writer.writeInt32(quest.required_min_level);
  writer.writeInt32(quest.required_max_level);
  writer.writeInt32(quest.required_quest);
  writer.writeUInt8(quest.required_class);
  writer.writeUInt8(quest.type);
  writer.writeInt32(quest.time_limit_in_seconds);
  writer.writeUInt32(quest.reward_gold);
  writer.writeUInt32(quest.reward_exp);
  writer.writeUInt32(quest.reward_credit);
  writeQuestRewards(writer, quest.fixed_rewards);
  writeQuestRewards(writer, quest.select_rewards);
  writer.writeUInt32(quest.finish_npc_index || quest.npc_index);
  return writer.toBuffer();
}

function writeStringList(writer, values) {
  writer.writeInt32(values.length);
  for (const value of values) {
    writer.writeString(value);
  }
}

function writeQuestRewards(writer, rewards) {
  writer.writeInt32(rewards.length);
  for (const reward of rewards) {
    writeItemInfo(writer, reward.item);
    writer.writeUInt16(reward.count);
  }
}

function encodeClientRecipeInfo(recipe) {
  const writer = new BinaryWriter();
  writer.writeUInt32(recipe.gold);
  writer.writeUInt8(recipe.chance);
  writeUserItem(writer, recipe.item_payload);
  writer.writeInt32(recipe.tools.length);
  for (const tool of recipe.tools) {
    writeUserItem(writer, tool);
  }
  writer.writeInt32(recipe.ingredients.length);
  for (const ingredient of recipe.ingredients) {
    writeUserItem(writer, ingredient);
  }
  return writer.toBuffer();
}

function createShopUserItem(item, uniqueId) {
  return {
    unique_id: BigInt(uniqueId),
    item_index: item.item_index,
    current_dura: item.durability,
    max_dura: item.durability,
    count: 1,
    soul_bound_id: -1,
    identified: false,
    cursed: false,
    slot_count: userItemSlotCount(item),
    gem_count: 0,
    added_stats: [],
    awake_type: 0,
    awake_values: [],
    refined_value: 0,
    refine_added: 0,
    refine_success_chance: 0,
    wedding_ring: -1,
    expire_info: null,
    rental_information: null,
    is_shop_item: true,
    sealed_info: null,
    gm_made: false,
  };
}

function userItemSlotCount(item) {
  if (item.item_type === 19) {
    if (item.shape < 7) {
      return 4;
    }
    if (item.shape < 12) {
      return 5;
    }
  }
  if (item.item_type === 1 && (item.shape === 49 || item.shape === 50)) {
    return 5;
  }
  return item.slots;
}

function writeUserItem(writer, item) {
  writer.writeUInt64(item.unique_id);
  writer.writeInt32(item.item_index);
  writer.writeUInt16(item.current_dura);
  writer.writeUInt16(item.max_dura);
  writer.writeUInt16(item.count);
  writer.writeInt32(item.soul_bound_id);

  let bools = 0;
  if (item.identified) bools |= 0x01;
  if (item.cursed) bools |= 0x02;
  writer.writeUInt8(bools);

  writer.writeInt32(item.slot_count);
  for (let index = 0; index < item.slot_count; index += 1) {
    writer.writeBoolean(true);
  }

  writer.writeUInt16(item.gem_count);
  writer.writeInt32(item.added_stats.length);
  for (const stat of item.added_stats) {
    writer.writeUInt8(stat.stat);
    writer.writeInt32(stat.value);
  }

  writer.writeUInt8(item.awake_type);
  writer.writeInt32(item.awake_values.length);
  for (const value of item.awake_values) {
    writer.writeUInt8(value);
  }

  writer.writeUInt8(item.refined_value);
  writer.writeUInt8(item.refine_added);
  writer.writeInt32(item.refine_success_chance);
  writer.writeInt32(item.wedding_ring);

  writer.writeBoolean(item.expire_info !== null);
  writer.writeBoolean(item.rental_information !== null);
  writer.writeBoolean(item.is_shop_item);
  writer.writeBoolean(item.sealed_info !== null);
  writer.writeBoolean(item.gm_made);
}

function encodeGameShopInfo(item, stockLevel) {
  const writer = new BinaryWriter();
  writeGameShopItem(writer, item);
  writer.writeInt32(stockLevel);
  return writer.toBuffer();
}

function writeGameShopItem(writer, item) {
  writer.writeInt32(item.item_index);
  writer.writeInt32(item.g_index);
  writeItemInfo(writer, item.info);
  writer.writeUInt32(item.gold_price);
  writer.writeUInt32(item.credit_price);
  writer.writeUInt16(item.count);
  writer.writeString(item.class);
  writer.writeString(item.category);
  writer.writeInt32(item.stock);
  writer.writeBoolean(item.individual_stock);
  writer.writeBoolean(item.deal);
  writer.writeBoolean(item.top_item);
  writer.writeInt64(item.date_binary);
  writer.writeBoolean(item.can_buy_credit);
  writer.writeBoolean(item.can_buy_gold);
}

function writeItemInfo(writer, item) {
  writer.writeInt32(item.item_index);
  writer.writeString(item.name);
  writer.writeUInt8(item.item_type);
  writer.writeUInt8(item.grade);
  writer.writeUInt8(item.required_type);
  writer.writeUInt8(item.required_class);
  writer.writeUInt8(item.required_gender);
  writer.writeUInt8(item.item_set);
  writer.writeInt16(item.shape);
  writer.writeUInt8(item.weight);
  writer.writeUInt8(item.light);
  writer.writeUInt8(item.required_amount);
  writer.writeUInt16(item.image);
  writer.writeUInt16(item.durability);
  writer.writeUInt16(item.stack_size);
  writer.writeUInt32(item.price);
  writer.writeBoolean(item.start_item);
  writer.writeUInt8(item.effect);
  let bools = 0;
  if (item.need_identify) bools |= 0x01;
  if (item.show_group_pickup) bools |= 0x02;
  if (item.class_based) bools |= 0x04;
  if (item.level_based) bools |= 0x08;
  if (item.can_mine) bools |= 0x10;
  if (item.global_drop_notify) bools |= 0x20;
  writer.writeUInt8(bools);
  writer.writeInt16(item.bind);
  writer.writeInt16(item.unique);
  writer.writeUInt8(item.random_stats_id);
  writer.writeBoolean(item.can_fast_run);
  writer.writeBoolean(item.can_awakening);
  writer.writeUInt8(item.slots);
  writer.writeInt32(item.stats.length);
  for (const stat of item.stats) {
    writer.writeUInt8(stat.stat);
    writer.writeInt32(stat.value);
  }
  writer.writeBoolean(item.tooltip !== null);
  if (item.tooltip !== null) {
    writer.writeString(item.tooltip);
  }
}

function normalizeItemLookupName(name) {
  return name.replaceAll(" ", "").toLowerCase();
}

function parseUInt16(value) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.min(parsed, 0xffff) : 0;
}

function parseUInt8(value) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.min(parsed, 0xff) : 0;
}

function parseUInt32(value) {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.min(parsed, 0xffff_ffff) : 0;
}

function readInt32List(reader) {
  const count = reader.readInt32();
  const values = [];
  for (let index = 0; index < count; index += 1) {
    values.push(reader.readInt32());
  }
  return values;
}

function readStats(reader) {
  const count = reader.readInt32();
  const values = new Map();
  for (let index = 0; index < count; index += 1) {
    values.set(reader.readUInt8(), reader.readInt32());
  }
  return values;
}

function skipStats(reader) {
  const count = reader.readInt32();
  reader.skip(count * 5);
}

function loadRoute(routePath) {
  if (!routePath) {
    return [];
  }

  const routeFilePath = `${resolve(routesRoot, ...routePath.split(/[\\/]+/))}.txt`;
  if (!existsSync(routeFilePath)) {
    return [];
  }

  return readFileSync(routeFilePath, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map(parseRouteLine)
    .filter(Boolean);
}

function parseRouteLine(line) {
  const [xRaw, yRaw, delayRaw] = line.split(",").map((value) => value.trim());
  const x = Number.parseInt(xRaw, 10);
  const y = Number.parseInt(yRaw, 10);
  if (Number.isNaN(x) || Number.isNaN(y)) {
    return null;
  }

  const delay = delayRaw === undefined ? 0 : Number.parseInt(delayRaw, 10);
  return {
    x,
    y,
    delay: Number.isNaN(delay) ? 0 : delay,
  };
}

function nullableString(value) {
  return value.length === 0 ? null : value;
}

function normalizePath(value) {
  return value.replace(/\\/g, "/").replace(/\.txt$/i, "");
}

class BinaryReader {
  constructor(buffer) {
    this.buffer = buffer;
    this.offset = 0;
  }

  readInt16() {
    const value = this.buffer.readInt16LE(this.offset);
    this.offset += 2;
    return value;
  }

  readUInt16() {
    const value = this.buffer.readUInt16LE(this.offset);
    this.offset += 2;
    return value;
  }

  readInt32() {
    const value = this.buffer.readInt32LE(this.offset);
    this.offset += 4;
    return value;
  }

  readUInt32() {
    const value = this.buffer.readUInt32LE(this.offset);
    this.offset += 4;
    return value;
  }

  readInt64() {
    const value = this.buffer.readBigInt64LE(this.offset);
    this.offset += 8;
    return value;
  }

  readUInt8() {
    const value = this.buffer.readUInt8(this.offset);
    this.offset += 1;
    return value;
  }

  readBoolean() {
    return this.readUInt8() !== 0;
  }

  readString() {
    const byteLength = this.read7BitEncodedInt();
    const value = this.buffer.toString("utf8", this.offset, this.offset + byteLength);
    this.offset += byteLength;
    return value;
  }

  read7BitEncodedInt() {
    let count = 0;
    let shift = 0;
    let byte = 0;

    do {
      byte = this.readUInt8();
      count |= (byte & 0x7f) << shift;
      shift += 7;
    } while ((byte & 0x80) !== 0);

    return count;
  }

  skip(byteLength) {
    this.offset += byteLength;
  }
}

class BinaryWriter {
  constructor() {
    this.chunks = [];
  }

  writeInt16(value) {
    const buffer = Buffer.alloc(2);
    buffer.writeInt16LE(value);
    this.chunks.push(buffer);
  }

  writeUInt16(value) {
    const buffer = Buffer.alloc(2);
    buffer.writeUInt16LE(value);
    this.chunks.push(buffer);
  }

  writeInt32(value) {
    const buffer = Buffer.alloc(4);
    buffer.writeInt32LE(value);
    this.chunks.push(buffer);
  }

  writeSingle(value) {
    const buffer = Buffer.alloc(4);
    buffer.writeFloatLE(value);
    this.chunks.push(buffer);
  }

  writeUInt32(value) {
    const buffer = Buffer.alloc(4);
    buffer.writeUInt32LE(value);
    this.chunks.push(buffer);
  }

  writeUInt64(value) {
    const buffer = Buffer.alloc(8);
    buffer.writeBigUInt64LE(BigInt(value));
    this.chunks.push(buffer);
  }

  writeInt64(value) {
    const buffer = Buffer.alloc(8);
    buffer.writeBigInt64LE(BigInt(value));
    this.chunks.push(buffer);
  }

  writeUInt8(value) {
    const buffer = Buffer.alloc(1);
    buffer.writeUInt8(value);
    this.chunks.push(buffer);
  }

  writeBoolean(value) {
    this.writeUInt8(value ? 1 : 0);
  }

  writeString(value) {
    const buffer = Buffer.from(value ?? "", "utf8");
    this.write7BitEncodedInt(buffer.length);
    this.chunks.push(buffer);
  }

  write7BitEncodedInt(value) {
    let remaining = value;
    while (remaining >= 0x80) {
      this.writeUInt8((remaining & 0x7f) | 0x80);
      remaining >>= 7;
    }
    this.writeUInt8(remaining);
  }

  toBuffer() {
    return Buffer.concat(this.chunks);
  }
}

main();
