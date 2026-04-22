import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const serverDbPath = process.argv[2] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Server.MirDB");
const routesRoot = process.argv[3] ?? resolve(repoRoot, "..", "Crystal", "Build", "Server", "Debug", "Envir", "Routes");
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

  const manifestMaps = maps
    .map((map) => {
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
        safe_zones: map.safe_zones,
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
  console.log(`Wrote Crystal respawn manifest to ${respawnOutputPath}`);
  console.log(`Wrote Crystal monster manifest to ${monsterOutputPath}`);
  console.log(`Wrote Crystal item manifest to ${itemOutputPath}`);
  console.log(`Wrote Crystal NPC info manifest to ${npcInfoOutputPath}`);
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
    reader.readBoolean();
    reader.readBoolean();
    reader.readBoolean();
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

main();
