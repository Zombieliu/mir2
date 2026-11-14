import { payloadHash as hashPayload } from "../memory/archive-codec.mjs";

export const MAX_MODEL_CONTEXT_BYTES = 16 * 1024;
const MAX_MEMORIES = 8;
const MAX_WORLD_OBJECTS = 12;
const MEMORY_FIELDS = ["episodeId", "kind", "action", "outcome", "summary", "recordedAt", "policyVersion", "payloadHash"];
const PARTY_HABITS = ["assistBeforeLoot", "lootOnlyAfterCombat", "askBeforeLeaving"];
const HASH = /^[a-f0-9]{64}$/;
const ID = /^[A-Za-z0-9][A-Za-z0-9._:/@,+-]{0,99}$/;
const IDENTIFIER = /^[a-z][a-z0-9._-]{0,63}$/;
const CREDENTIAL = /(?:password|passwd|passphrase|api[ _-]?key|archive[ _-]?key|account(?:[ _-]?id)?|owner[ _-]?ref|credential|client[ _-]?secret|authorization|secret|(?:access[ _-]?|refresh[ _-]?)?token)["']?\s*(?:[:=：]|\bis\b)\s*\S+|\bbearer\s+\S+|\b(?:sk|ghp|github_pat|xox[baprs])[-_][a-z0-9_-]{8,}|\bAKIA[a-z0-9]{16}\b|(?:密码|密钥|账号|帐号|账户|令牌)\s*(?:是|为|[:=：])\s*\S+|[a-z][a-z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]+@/iu;
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const uint = value => Number.isSafeInteger(value) && value >= 0;
const point = value => record(value) && uint(value.x) && value.x <= 65_535 && uint(value.y) && value.y <= 65_535
  ? { x: value.x, y: value.y } : null;
const bytes = value => Buffer.byteLength(JSON.stringify(value), "utf8");

function safeText(value, maxBytes, maxInputCharacters = 2048) {
  if (typeof value !== "string" || value.length > maxInputCharacters * 2) return null;
  if (/[\p{Cc}\p{Cf}\p{Cs}]/u.test(value)) return null;
  const text = value.normalize("NFKC").trim();
  if (!text || [...text].length > maxInputCharacters || CREDENTIAL.test(text)) return null;
  let result = "", length = 0;
  for (const character of text) {
    const size = Buffer.byteLength(character, "utf8");
    if (length + size > maxBytes) break;
    result += character;
    length += size;
  }
  return result || null;
}

function tokens(value) {
  const text = safeText(value, 4096, 2048)?.toLowerCase();
  const result = new Set();
  if (!text) return result;
  for (const word of text.match(/[a-z][a-z0-9_-]{1,47}/g) ?? []) {
    result.add(word);
    if (result.size >= 128) return result;
  }
  for (const run of text.match(/[\p{Script=Han}]+/gu) ?? []) {
    const characters = [...run];
    if (characters.length === 1) result.add(run);
    for (let index = 0; index + 1 < characters.length; index += 1) {
      result.add(characters[index] + characters[index + 1]);
      if (result.size >= 128) return result;
    }
  }
  return result;
}

function episodeMemory(row) {
  if (!record(row) || row.status !== "active" || typeof row.episodeId !== "string" || !ID.test(row.episodeId) || CREDENTIAL.test(row.episodeId) ||
      typeof row.payloadHash !== "string" || !HASH.test(row.payloadHash) ||
      !uint(row.recordedAt) || !Number.isSafeInteger(row.policyVersion) || row.policyVersion < 1 || !record(row.payload)) return null;
  const payload = row.payload;
  if (!IDENTIFIER.test(payload.kind ?? "") || CREDENTIAL.test(payload.kind) || payload.kind === "feedback" || payload.kind === "model-call" ||
      payload.kind.startsWith("model-") && !["model-plan", "model-explanation"].includes(payload.kind) ||
      payload.kind === "objective-progress" && payload.outcome !== "observed") return null;
  if (["model-plan", "model-explanation"].includes(payload.kind) && ["observed", "executed", "completed", "succeeded"].includes(payload.outcome)) return null;
  if (["action", "outcome"].some(key => payload[key] !== undefined &&
      (typeof payload[key] !== "string" || !IDENTIFIER.test(payload[key]) || CREDENTIAL.test(payload[key])))) return null;
  const summary = payload.summary === undefined ? null : safeText(payload.summary, 512, 500);
  if (payload.summary !== undefined && summary === null) return null;
  try { if (hashPayload(payload) !== row.payloadHash) return null; }
  catch { return null; }
  return { episodeId: row.episodeId, kind: payload.kind, action: payload.action ?? null, outcome: payload.outcome ?? null,
    summary, recordedAt: row.recordedAt, policyVersion: row.policyVersion, payloadHash: row.payloadHash };
}

function worldObjects(values, player, mapFileName, { monsters = false } = {}) {
  if (!Array.isArray(values)) return [];
  const seen = new Set();
  const projected = [];
  for (const value of values) {
    if (!record(value) || !uint(value.objectId) || value.objectId > 0xffff_ffff || seen.has(value.objectId) ||
        monsters && (value.kind !== "monster" || value.dead === true || typeof value.hp === "number" && value.hp <= 0) ||
        value.mapFileName !== undefined && value.mapFileName !== mapFileName) continue;
    const position = point(value);
    if (!position) continue;
    seen.add(value.objectId);
    projected.push({ objectId: value.objectId, name: safeText(value.name, 96, 128), position });
  }
  const distance = value => player ? Math.max(Math.abs(value.position.x - player.x), Math.abs(value.position.y - player.y)) : Infinity;
  return projected.sort((a, b) => distance(a) - distance(b) || a.objectId - b.objectId).slice(0, MAX_WORLD_OBJECTS);
}

/**
 * A bounded JSON data projection, never a prompt instruction or a new world fact.
 * Names and summaries remain untrusted quoted data. owner-instruction and its
 * continuation are owner statements, model-plan/explanation are proposals, and
 * only objective-progress with outcome=observed claims an observed result.
 * The caller must retain those distinctions in its model system instructions.
 */
export function buildModelContext({ state = {}, store, text = "", leaderName, limit = MAX_MEMORIES }) {
  if (!Number.isSafeInteger(limit) || limit < 0) throw new TypeError("Memory limit must be a nonnegative safe integer.");
  if (!store || typeof store.profile !== "function" || typeof store.growth !== "function") throw new TypeError("A companion memory store is required.");
  const snapshot = record(state) ? state : {};
  const player = point(snapshot.player);
  const mapFileName = safeText(snapshot.mapFileName, 64, 64);
  const entities = Array.isArray(snapshot.entities) ? snapshot.entities : [];
  const owner = typeof leaderName === "string" && entities.find(value => record(value) && value.kind === "player" &&
    value.name === leaderName && value.dead !== true && (typeof value.hp !== "number" || value.hp > 0) && point(value) &&
    (value.mapFileName === undefined || value.mapFileName === snapshot.mapFileName));
  const members = Array.isArray(snapshot.party?.members) ? snapshot.party.members : [];
  const hp = uint(snapshot.playerHp) && snapshot.playerHp <= 0x7fff_ffff ? snapshot.playerHp : null;
  const maxHp = uint(snapshot.playerMaxHp) && snapshot.playerMaxHp > 0 && snapshot.playerMaxHp <= 0x7fff_ffff ? snapshot.playerMaxHp : null;
  const profile = store.profile();
  const context = {
    world: { mapFileName, position: player, health: { hp: maxHp !== null && hp > maxHp ? null : hp, maxHp },
      owner: { visible: Boolean(owner), position: owner ? point(owner) : null },
      party: { grouped: typeof leaderName === "string" && typeof snapshot.player?.name === "string" &&
        leaderName !== snapshot.player.name && members.includes(leaderName) && members.includes(snapshot.player.name) },
      monsters: worldObjects(entities, player, snapshot.mapFileName, { monsters: true }),
      nearbyDrops: worldObjects(snapshot.groundDrops, player, snapshot.mapFileName) },
    habits: Object.fromEntries(PARTY_HABITS.map(key => [key, profile?.party?.[key] === true])),
    memories: [], memoryStamp: [],
  };
  // Preserve the current position, health, owner, party and habits before optional
  // object detail. These loops also keep the bound safe if field limits grow.
  while (bytes(context) > MAX_MODEL_CONTEXT_BYTES && context.world.nearbyDrops.length) context.world.nearbyDrops.pop();
  while (bytes(context) > MAX_MODEL_CONTEXT_BYTES && context.world.monsters.length) context.world.monsters.pop();
  if (bytes(context) > MAX_MODEL_CONTEXT_BYTES) throw new Error("Required model context exceeds its byte limit.");
  const query = tokens(text);
  const episodes = store.growth().episodes;
  const candidates = (Array.isArray(episodes) ? episodes : []).flatMap((row, index) => {
    const memory = episodeMemory(row);
    if (!memory) return [];
    const words = tokens([memory.kind, memory.action, memory.outcome, memory.summary].filter(Boolean).join(" "));
    let relevance = 0;
    for (const word of query) if (words.has(word)) relevance += 1;
    return [{ memory, relevance, recency: uint(row.sequence) ? row.sequence : index }];
  }).sort((a, b) => b.relevance - a.relevance || b.recency - a.recency || a.memory.episodeId.localeCompare(b.memory.episodeId));
  for (const { memory } of candidates) {
    if (context.memories.length >= Math.min(limit, MAX_MEMORIES)) break;
    context.memories.push(memory);
    context.memoryStamp.push({ episodeId: memory.episodeId, payloadHash: memory.payloadHash });
    if (bytes(context) > MAX_MODEL_CONTEXT_BYTES) { context.memories.pop(); context.memoryStamp.pop(); }
  }
  return context;
}

/** Recheck current active content immediately before using model citations. */
export function validateMemoryReferences(ids, context, store) {
  if (!Array.isArray(ids) || ids.length > MAX_MEMORIES || new Set(ids).size !== ids.length ||
      ids.some(id => typeof id !== "string" || !ID.test(id)) || !record(context) ||
      !Array.isArray(context.memories) || !Array.isArray(context.memoryStamp) ||
      context.memories.length > MAX_MEMORIES || context.memoryStamp.length !== context.memories.length ||
      !store || typeof store.growth !== "function") return false;
  try {
    const memories = new Map(), stamps = new Map();
    for (const memory of context.memories) {
      if (!record(memory) || typeof memory.episodeId !== "string" || !ID.test(memory.episodeId) ||
          typeof memory.payloadHash !== "string" || !HASH.test(memory.payloadHash) ||
          Object.keys(memory).length !== MEMORY_FIELDS.length || MEMORY_FIELDS.some(key => !Object.hasOwn(memory, key)) ||
          memories.has(memory.episodeId)) return false;
      memories.set(memory.episodeId, memory);
    }
    for (const stamp of context.memoryStamp) {
      if (!record(stamp) || typeof stamp.episodeId !== "string" || !ID.test(stamp.episodeId) ||
          typeof stamp.payloadHash !== "string" || !HASH.test(stamp.payloadHash) ||
          Object.keys(stamp).length !== 2 || stamps.has(stamp.episodeId)) return false;
      stamps.set(stamp.episodeId, stamp.payloadHash);
    }
    const current = new Map(store.growth().episodes.map(row => [row.episodeId, row]));
    return ids.every(id => {
      const memory = memories.get(id), expected = episodeMemory(current.get(id));
      return memory && expected && stamps.get(id) === memory.payloadHash &&
        MEMORY_FIELDS.every(key => memory[key] === expected[key]);
    });
  } catch { return false; }
}
