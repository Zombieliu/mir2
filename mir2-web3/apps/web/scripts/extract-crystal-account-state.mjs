import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_DB_PATH = path.resolve(REPO_ROOT, "..", "Crystal", "Build", "Server", "Debug", "Server.MirADB");
const DEFAULT_ITEM_MANIFEST_PATH = path.resolve(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_item_manifest.json",
);
const FLAG_INDEX_COUNT = 1999;
const BELT_SIZE = 6;
const EQUIPMENT_SLOTS = [
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

const args = parseArgs(process.argv.slice(2));
const dbPath = path.resolve(args.db ?? args.accountDb ?? DEFAULT_DB_PATH);
const itemManifestPath = path.resolve(args.items ?? args.itemManifest ?? DEFAULT_ITEM_MANIFEST_PATH);
const accountFilter = args.account ?? process.env.MIR2_QA_ACCOUNT ?? null;
const characterFilter = args.character ?? args.characterName ?? process.env.MIR2_QA_CHARACTER ?? null;
const outputPath = args.output ? path.resolve(args.output) : null;

async function main() {
  const [dbBuffer, itemManifest] = await Promise.all([
    fs.readFile(dbPath),
    readJson(itemManifestPath),
  ]);
  const itemByIndex = new Map((itemManifest.items ?? []).map((item) => [item.item_index, item]));
  const reader = new BinaryReader(dbBuffer);
  const parsed = readAccountDatabase(reader, itemByIndex);
  const account = findAccount(parsed.accounts, accountFilter, characterFilter);
  const result = {
    ok: Boolean(account),
    generatedAt: new Date().toISOString(),
    source: {
      dbPath,
      itemManifestPath,
      finalOffset: parsed.finalOffset,
      byteLength: dbBuffer.length,
      fullyConsumed: parsed.finalOffset === dbBuffer.length,
    },
    header: parsed.header,
    accountCount: parsed.accountCount,
    filters: {
      account: accountFilter,
      character: characterFilter,
    },
    account: account ? sanitizeAccount(account, characterFilter) : null,
  };

  const json = `${JSON.stringify(result, null, 2)}\n`;
  if (outputPath) {
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, json, "utf8");
  }
  process.stdout.write(json);
  if (!account) process.exitCode = 1;
}

function readAccountDatabase(reader, itemByIndex) {
  const header = {
    version: reader.i32(),
    customVersion: reader.i32(),
    nextAccountId: reader.i32(),
    nextCharacterId: reader.i32(),
    nextUserItemId: String(reader.u64()),
  };
  if (header.version > 98) header.nextHeroId = reader.i32();
  header.guildCount = reader.i32();
  header.nextGuildId = reader.i32();
  if (header.version > 102) {
    const heroCount = reader.i32();
    header.heroCount = heroCount;
    for (let index = 0; index < heroCount; index += 1) {
      throw new Error("Global hero-list parsing is not implemented yet.");
    }
  }

  const accountCount = reader.i32();
  const accounts = [];
  for (let index = 0; index < accountCount; index += 1) {
    accounts.push(readAccount(reader, header.version, header.customVersion, itemByIndex));
  }

  const tail = {
    nextAuctionId: String(reader.u64()),
    auctionCount: reader.i32(),
  };
  for (let index = 0; index < tail.auctionCount; index += 1) {
    skipAuction(reader, header.version, itemByIndex);
  }
  tail.nextMailId = String(reader.u64());
  tail.gameShopLogCount = reader.i32();
  for (let index = 0; index < tail.gameShopLogCount; index += 1) {
    reader.i32();
    reader.i32();
  }
  tail.savedSpawnCount = reader.i32();
  for (let index = 0; index < tail.savedSpawnCount; index += 1) {
    reader.i32();
    reader.i64();
    reader.bool();
  }

  return {
    header: { ...header, tail },
    accountCount,
    accounts,
    finalOffset: reader.offset,
  };
}

function readAccount(reader, version, customVersion, itemByIndex) {
  const account = {
    index: reader.i32(),
    accountID: reader.string(),
  };
  reader.string(); // password hash
  reader.bytes(reader.i32()); // password salt
  account.requirePasswordChange = reader.bool();
  const storagePassword = reader.string();
  reader.bytes(reader.i32()); // storage salt
  reader.i64(); // storage password last set

  account.hasStoragePassword = storagePassword.length > 0;
  account.userName = reader.string();
  reader.i64(); // birth date
  reader.string(); // secret question
  reader.string(); // secret answer
  account.email = reader.string();
  account.creationIP = reader.string();
  reader.i64(); // creation date
  account.banned = reader.bool();
  account.banReason = reader.string();
  reader.i64(); // expiry date
  account.lastIP = reader.string();
  reader.i64(); // last date

  const characterCount = reader.i32();
  account.characters = [];
  for (let index = 0; index < characterCount; index += 1) {
    account.characters.push(readCharacter(reader, version, customVersion, itemByIndex));
  }

  account.hasExpandedStorage = reader.bool();
  account.expandedStorageExpiryDateBinary = String(reader.i64());
  account.gold = reader.u32();
  account.credit = reader.u32();
  account.storage = readItemArray(reader, version, customVersion, itemByIndex, "storage");
  account.adminAccount = reader.bool();
  return account;
}

function readCharacter(reader, version, customVersion, itemByIndex) {
  const character = {
    index: reader.i32(),
    name: reader.string(),
    level: reader.u16(),
    class: reader.u8(),
    gender: reader.u8(),
    hair: reader.u8(),
  };
  character.creationIP = reader.string();
  reader.i64(); // creation date
  character.banned = reader.bool();
  character.banReason = reader.string();
  reader.i64(); // expiry date
  character.lastIP = reader.string();
  character.lastLogoutDateBinary = String(reader.i64());
  character.lastLoginDateBinary = String(reader.i64());
  character.deleted = reader.bool();
  reader.i64(); // delete date

  character.currentMapIndex = reader.i32();
  character.currentLocation = { x: reader.i32(), y: reader.i32() };
  character.direction = reader.u8();
  character.bindMapIndex = reader.i32();
  character.bindLocation = { x: reader.i32(), y: reader.i32() };
  character.hp = reader.i32();
  character.mp = reader.i32();
  character.experience = String(reader.i64());
  character.attackMode = reader.u8();
  character.petMode = reader.u8();
  character.pkPoints = reader.i32();

  character.inventory = readItemArray(reader, version, customVersion, itemByIndex, "inventory");
  character.beltItems = character.inventory.items.filter((item) => item.slot >= 0 && item.slot < BELT_SIZE);
  character.bagItems = character.inventory.items.filter((item) => item.slot >= BELT_SIZE);
  character.equipment = readItemArray(reader, version, customVersion, itemByIndex, "equipment");
  character.equipment.items = character.equipment.items.map((item) => ({
    ...item,
    equipmentSlot: EQUIPMENT_SLOTS[item.slot] ?? String(item.slot),
  }));
  character.questInventory = readItemArray(reader, version, customVersion, itemByIndex, "questInventory");

  character.magicCount = reader.i32();
  for (let index = 0; index < character.magicCount; index += 1) skipUserMagic(reader);
  character.thrusting = reader.bool();
  character.halfMoon = reader.bool();
  character.crossHalfMoon = reader.bool();
  character.doubleSlash = reader.bool();
  character.mentalState = reader.u8();
  character.petCount = reader.i32();
  for (let index = 0; index < character.petCount; index += 1) skipPet(reader);
  character.allowGroup = reader.bool();
  for (let index = 0; index < FLAG_INDEX_COUNT; index += 1) reader.bool();
  character.guildIndex = reader.i32();
  character.allowTrade = reader.bool();
  character.allowObserve = reader.bool();
  character.questCount = reader.i32();
  for (let index = 0; index < character.questCount; index += 1) skipQuestProgress(reader);
  character.buffCount = reader.i32();
  for (let index = 0; index < character.buffCount; index += 1) skipBuff(reader, version, customVersion);
  character.mailCount = reader.i32();
  for (let index = 0; index < character.mailCount; index += 1) skipMail(reader, version, customVersion, itemByIndex);
  character.intelligentCreatureCount = reader.i32();
  for (let index = 0; index < character.intelligentCreatureCount; index += 1) skipIntelligentCreature(reader);
  character.pearlCount = reader.i32();
  character.completedQuestCount = reader.i32();
  for (let index = 0; index < character.completedQuestCount; index += 1) reader.i32();
  character.currentRefine = reader.bool() ? readUserItem(reader, version, customVersion, itemByIndex, null) : null;
  character.refineTimeRemaining = String(reader.i64());
  character.friendCount = reader.i32();
  for (let index = 0; index < character.friendCount; index += 1) skipFriend(reader);
  character.rentedItemCount = reader.i32();
  for (let index = 0; index < character.rentedItemCount; index += 1) skipItemRental(reader);
  character.hasRentedItem = reader.bool();
  character.married = reader.i32();
  reader.i64(); // married date
  character.mentor = reader.i32();
  reader.i64(); // mentor date
  character.isMentor = reader.bool();
  character.mentorExp = String(reader.i64());
  character.gameShopPurchaseCount = reader.i32();
  for (let index = 0; index < character.gameShopPurchaseCount; index += 1) {
    reader.i32();
    reader.i32();
  }
  character.maximumHeroCount = reader.i32();
  character.heroes = [];
  for (let index = 0; index < character.maximumHeroCount; index += 1) {
    character.heroes.push(reader.i32());
  }
  character.currentHeroIndex = reader.i32();
  character.heroSpawned = reader.bool();
  character.heroBehaviour = reader.u8();
  return character;
}

function readItemArray(reader, version, customVersion, itemByIndex, label) {
  const count = reader.i32();
  const items = [];
  for (let slot = 0; slot < count; slot += 1) {
    if (!reader.bool()) continue;
    items.push(readUserItem(reader, version, customVersion, itemByIndex, slot));
  }
  return { label, count, items };
}

function readUserItem(reader, version, customVersion, itemByIndex, slot) {
  const item = {
    slot,
    uniqueId: String(reader.u64()),
    itemIndex: reader.i32(),
    currentDura: reader.u16(),
    maxDura: reader.u16(),
    count: reader.u16(),
  };
  item.soulBoundId = reader.i32();
  const flags = reader.u8();
  item.identified = Boolean(flags & 0x01);
  item.cursed = Boolean(flags & 0x02);
  const slotCount = reader.i32();
  item.socketItems = [];
  for (let index = 0; index < slotCount; index += 1) {
    const empty = reader.bool();
    if (!empty) item.socketItems.push(readUserItem(reader, version, customVersion, itemByIndex, index));
  }
  item.gemCount = reader.u16();
  item.addedStats = readStats(reader);
  item.awake = readAwake(reader);
  item.refinedValue = reader.u8();
  item.refineAdded = reader.u8();
  item.refineSuccessChance = reader.i32();
  item.weddingRing = reader.i32();
  item.expireInfo = reader.bool() ? { expiryDateBinary: String(reader.i64()) } : null;
  item.rentalInformation = reader.bool() ? readRentalInformation(reader) : null;
  item.isShopItem = reader.bool();
  item.sealedInfo = reader.bool()
    ? { expiryDateBinary: String(reader.i64()), nextSealDateBinary: String(reader.i64()) }
    : null;
  item.gmMade = reader.bool();

  const manifestItem = itemByIndex.get(item.itemIndex);
  item.name = manifestItem?.name ?? null;
  item.image = manifestItem?.image ?? null;
  item.itemType = manifestItem?.item_type ?? null;
  item.stackSize = manifestItem?.stack_size ?? null;
  return item;
}

function readStats(reader) {
  const count = reader.i32();
  const values = [];
  for (let index = 0; index < count; index += 1) {
    values.push({ stat: reader.u8(), value: reader.i32() });
  }
  return values;
}

function readAwake(reader) {
  const type = reader.u8();
  const count = reader.i32();
  for (let index = 0; index < count; index += 1) reader.u8();
  return { type, count };
}

function readRentalInformation(reader) {
  return {
    ownerName: reader.string(),
    bindingFlags: reader.u16(),
    expiryDateBinary: String(reader.i64()),
    rentalLocked: reader.bool(),
  };
}

function skipUserMagic(reader) {
  reader.u8();
  reader.u8();
  reader.u8();
  reader.u16();
  reader.bool();
  reader.i64();
}

function skipPet(reader) {
  reader.i32();
  reader.i32();
  reader.u32();
  reader.u8();
  reader.u8();
}

function skipQuestProgress(reader) {
  reader.i32();
  reader.i64();
  reader.i64();
  let count = reader.i32();
  for (let index = 0; index < count; index += 1) {
    reader.i32();
    reader.i32();
  }
  count = reader.i32();
  for (let index = 0; index < count; index += 1) {
    reader.i32();
    reader.i32();
  }
  count = reader.i32();
  for (let index = 0; index < count; index += 1) {
    reader.i32();
    reader.bool();
  }
}

function skipBuff(reader, version, customVersion) {
  reader.u8();
  reader.u32();
  reader.i64();
  readStats(reader, version, customVersion);
  let count = reader.i32();
  for (let index = 0; index < count; index += 1) {
    reader.string();
    reader.bytes(reader.i32());
  }
  count = reader.i32();
  for (let index = 0; index < count; index += 1) reader.i32();
}

function skipMail(reader, version, customVersion, itemByIndex) {
  reader.u64();
  reader.string();
  reader.i32();
  reader.string();
  reader.u32();
  const count = reader.i32();
  for (let index = 0; index < count; index += 1) {
    readUserItem(reader, version, customVersion, itemByIndex, null);
  }
  reader.i64();
  reader.i64();
  reader.bool();
  reader.bool();
  reader.bool();
}

function skipIntelligentCreature(reader) {
  reader.u8();
  reader.string();
  reader.i32();
  reader.i32();
  reader.i64();
  reader.i64();
  reader.u8();
  for (let index = 0; index < 9; index += 1) reader.bool();
  reader.u8();
  reader.i64();
}

function skipFriend(reader) {
  reader.i32();
  reader.bool();
  reader.string();
}

function skipItemRental(reader) {
  reader.u64();
  reader.string();
  reader.string();
  reader.i64();
}

function skipAuction(reader, version, itemByIndex) {
  reader.u64();
  readUserItem(reader, version, 0, itemByIndex, null);
  reader.u32();
  reader.i64();
  reader.i32();
}

function findAccount(accounts, account, character) {
  const accountNeedle = account?.toLowerCase() ?? null;
  const characterNeedle = character?.toLowerCase() ?? null;
  return (
    accounts.find((entry) => {
      const accountMatches = !accountNeedle || entry.accountID.toLowerCase() === accountNeedle;
      const characterMatches =
        !characterNeedle || entry.characters.some((candidate) => candidate.name.toLowerCase() === characterNeedle);
      return accountMatches && characterMatches;
    }) ?? null
  );
}

function sanitizeAccount(account, characterFilter) {
  const characterNeedle = characterFilter?.toLowerCase() ?? null;
  const characters = characterNeedle
    ? account.characters.filter((character) => character.name.toLowerCase() === characterNeedle)
    : account.characters;
  return {
    index: account.index,
    accountID: account.accountID,
    userName: account.userName,
    email: account.email,
    gold: account.gold,
    credit: account.credit,
    hasExpandedStorage: account.hasExpandedStorage,
    storageItemCount: account.storage.items.length,
    characters: characters.map((character) => ({
      index: character.index,
      name: character.name,
      level: character.level,
      class: character.class,
      gender: character.gender,
      currentMapIndex: character.currentMapIndex,
      currentLocation: character.currentLocation,
      direction: character.direction,
      hp: character.hp,
      mp: character.mp,
      experience: character.experience,
      beltItems: character.beltItems,
      bagItems: character.bagItems,
      equipmentItems: character.equipment.items,
      questInventoryItemCount: character.questInventory.items.length,
      magicCount: character.magicCount,
      buffCount: character.buffCount,
      mailCount: character.mailCount,
      petCount: character.petCount,
    })),
  };
}

async function readJson(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
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

class BinaryReader {
  constructor(buffer) {
    this.buffer = buffer;
    this.offset = 0;
  }

  ensure(byteCount) {
    if (this.offset + byteCount > this.buffer.length) {
      throw new Error(`Unexpected end of file at ${this.offset}; need ${byteCount} bytes`);
    }
  }

  u8() {
    this.ensure(1);
    return this.buffer[this.offset++];
  }

  bool() {
    return this.u8() !== 0;
  }

  i32() {
    this.ensure(4);
    const value = this.buffer.readInt32LE(this.offset);
    this.offset += 4;
    return value;
  }

  u16() {
    this.ensure(2);
    const value = this.buffer.readUInt16LE(this.offset);
    this.offset += 2;
    return value;
  }

  u32() {
    this.ensure(4);
    const value = this.buffer.readUInt32LE(this.offset);
    this.offset += 4;
    return value;
  }

  u64() {
    this.ensure(8);
    const value = this.buffer.readBigUInt64LE(this.offset);
    this.offset += 8;
    return value;
  }

  i64() {
    this.ensure(8);
    const value = this.buffer.readBigInt64LE(this.offset);
    this.offset += 8;
    return value;
  }

  bytes(byteCount) {
    this.ensure(byteCount);
    this.offset += byteCount;
  }

  string() {
    let length = 0;
    let shift = 0;
    let byte = 0;
    do {
      byte = this.u8();
      length |= (byte & 0x7f) << shift;
      shift += 7;
      if (shift > 35) throw new Error(`Invalid BinaryWriter string length at offset ${this.offset}`);
    } while (byte & 0x80);
    this.ensure(length);
    const value = this.buffer.toString("utf8", this.offset, this.offset + length);
    this.offset += length;
    return value;
  }
}

await main();
