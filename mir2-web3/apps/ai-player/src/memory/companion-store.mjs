import { DatabaseSync } from "node:sqlite";
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { archiveId, canonicalJson, nonnegativeInteger, payloadHash, plainRecord, signArchive, verifyArchive } from "./archive-codec.mjs";

const SCHEMA_VERSION = 2;
const HARD_PERMISSIONS = Object.freeze({ spendLimit: 0, mapAllowlist: Object.freeze(["0", "0141"]) });
const AUTH_KEYS = ["source", "operationId", "expectedPolicyVersion", "ownerRef", "archiveId", "worldEpoch"];
const FEEDBACK_KEYS = [...AUTH_KEYS, "feedbackId", "context", "feedbackText"];
const EPISODE_KEYS = ["episodeId", "observedAt", "kind", "action", "outcome", "summary", "actorRef", "targetRef", "objectRef", "locationRef", "context"];
const CONTEXT_KEYS = ["mode", "leaderRef", "locationRef", "sourceEventRef", "partySize"];
const PARTY_KEYS = ["assistBeforeLoot", "lootOnlyAfterCombat", "askBeforeLeaving"];
const ARCHIVE_KEYS = ["archiveId", "residentId", "ownerRef", "createdAt", "displayName", "realmId", "worldEpoch", "characterName", "characterIndex", "appearance", "baseSetting"];
const IMPORT_DATA = Symbol("verified-import-data");
const MAX_ROWS = 10_000;
const clone = value => structuredClone(value);

function object(value, keys, label) {
  if (!value || typeof value !== "object" || Array.isArray(value) || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    throw new TypeError(`${label} must be a plain object`);
  }
  if (Object.keys(value).some(key => !keys.includes(key))) throw new TypeError(`${label} contains unsupported fields`);
  return value;
}

function text(value, label, maxLength = 500, allowEmpty = false) {
  if (typeof value !== "string") throw new TypeError(`${label} must be text`);
  const result = value.normalize("NFKC").trim();
  if ((!result && !allowEmpty) || [...result].length > maxLength || /[\p{Cc}\p{Cf}\p{Cs}]/u.test(value)) {
    throw new TypeError(`${label} is empty, too long, or contains control characters`);
  }
  // Never accept raw transport/configuration payloads disguised as a summary.
  if (/(?:password|passwd|api[ _-]?key|(?:access[ _-]?|refresh[ _-]?)?token|authorization|secret)["']?\s*(?:[:=]|\bis\b)\s*\S+|\bbearer\s+\S+|\b(?:sk|ghp|github_pat|xox[baprs])[-_][a-z0-9_-]{8,}|密码\s*(?:是|为|[:=：])\s*\S+|[a-z][a-z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]+@/iu.test(result)) {
    throw new TypeError(`${label} cannot contain credentials`);
  }
  return result;
}

function ref(value, label) {
  const result = text(value, label, 160);
  if (!/^[\p{L}\p{N}._:/@,+-]+$/u.test(result)) throw new TypeError(`${label} must be an opaque text reference`);
  return result;
}

function bindingOf(value) {
  object(value, ["realmId", "residentId", "characterName", "characterIndex", "operatorRef"], "binding");
  const result = {
    realmId: text(value.realmId, "realmId", 120),
    residentId: ref(value.residentId, "residentId"),
    characterName: text(value.characterName, "characterName", 64),
  };
  if (value.characterIndex !== undefined) {
    if (!Number.isSafeInteger(value.characterIndex) || value.characterIndex < 0) throw new TypeError("characterIndex must be a nonnegative safe integer");
    result.characterIndex = value.characterIndex;
  }
  if (value.operatorRef !== undefined) result.operatorRef = ref(value.operatorRef, "operatorRef");
  return result;
}

function contextOf(value) {
  if (value === undefined || value === null) return null;
  object(value, CONTEXT_KEYS, "context");
  const result = {};
  if (value.mode !== undefined) {
    if (!["party", "solo"].includes(value.mode)) throw new TypeError("context.mode must be party or solo");
    result.mode = value.mode;
  }
  for (const key of ["leaderRef", "locationRef", "sourceEventRef"]) {
    if (value[key] !== undefined) result[key] = ref(value[key], `context.${key}`);
  }
  if (value.partySize !== undefined) {
    if (!Number.isSafeInteger(value.partySize) || value.partySize < 1 || value.partySize > 100) throw new TypeError("context.partySize must be from 1 to 100");
    result.partySize = value.partySize;
  }
  return result;
}

function identifier(value, label) {
  const result = text(value, label, 64);
  if (!/^[a-z][a-z0-9._-]*$/u.test(result)) throw new TypeError(`${label} must be a bounded identifier`);
  return result;
}

function episodeOf(value) {
  object(value, EPISODE_KEYS, "episode");
  const result = { kind: identifier(value.kind, "episode.kind") };
  for (const key of ["action", "outcome"]) if (value[key] !== undefined) result[key] = identifier(value[key], `episode.${key}`);
  if (value.summary !== undefined) result.summary = text(value.summary, "episode.summary");
  for (const key of ["actorRef", "targetRef", "objectRef", "locationRef"]) {
    if (value[key] !== undefined) result[key] = ref(value[key], `episode.${key}`);
  }
  if (value.context !== undefined) result.context = contextOf(value.context);
  if (value.observedAt !== undefined) {
    if (!Number.isSafeInteger(value.observedAt) || value.observedAt < 0) throw new TypeError("observedAt must be a nonnegative safe integer");
    result.observedAt = value.observedAt;
  }
  return result;
}

function defaultProfile(binding) {
  return { schemaVersion: SCHEMA_VERSION, policyVersion: 1, style: "collection", binding: clone(binding),
    solo: { considerLootWhenSafe: true }, party: { assistBeforeLoot: false, lootOnlyAfterCombat: false, askBeforeLeaving: false }, permissions: clone(HARD_PERMISSIONS) };
}
function habitsOf(habits) {
  plainRecord(habits, ["solo", "party"], "habits");
  plainRecord(habits.solo, ["considerLootWhenSafe"], "solo habits");
  plainRecord(habits.party, PARTY_KEYS, "party habits");
  if (typeof habits.solo.considerLootWhenSafe !== "boolean" || PARTY_KEYS.some(key => typeof habits.party[key] !== "boolean")) throw new TypeError("Invalid companion habits");
  return clone(habits);
}
function profileOf(profile, binding, version = SCHEMA_VERSION) {
  object(profile, ["schemaVersion", "policyVersion", "style", "binding", "solo", "party", "permissions"], "profile");
  if (profile.schemaVersion !== version || !Number.isSafeInteger(profile.policyVersion) || profile.policyVersion < 1 ||
      canonicalJson(profile.binding) !== canonicalJson(binding) || canonicalJson(profile.permissions) !== canonicalJson(HARD_PERMISSIONS)) throw new Error("Companion profile identity or hard permissions are invalid");
  habitsOf({ solo: profile.solo, party: profile.party });
  if (profile.style !== "collection") throw new Error("Unsupported companion preference schema");
  return profile;
}
function appearanceOf(value = { class: "Warrior", gender: "Male" }) {
  plainRecord(value, ["class", "gender"], "appearance");
  if (!["Warrior", "Wizard", "Taoist", "Assassin", "Archer"].includes(value.class) || !["Male", "Female"].includes(value.gender)) throw new TypeError("Invalid base appearance");
  return clone(value);
}
function characterArchiveOf(value, { trusted = false } = {}) {
  plainRecord(value, ARCHIVE_KEYS, "characterArchive");
  const result = {
    archiveId: archiveId(value.archiveId, "archiveId"), residentId: ref(value.residentId, "residentId"),
    ownerRef: value.ownerRef === null ? null : archiveId(value.ownerRef, "ownerRef"),
    createdAt: nonnegativeInteger(value.createdAt, "createdAt"), displayName: text(value.displayName, "displayName", 64),
    realmId: text(value.realmId, "realmId", 120), worldEpoch: value.worldEpoch === null ? null : archiveId(value.worldEpoch, "worldEpoch"),
    characterName: text(value.characterName, "characterName", 64),
    characterIndex: value.characterIndex === null ? null : nonnegativeInteger(value.characterIndex, "characterIndex"),
    appearance: appearanceOf(value.appearance), baseSetting: text(value.baseSetting, "baseSetting", 1000, true),
  };
  if (trusted && (!result.ownerRef || !result.worldEpoch || result.characterIndex === null)) throw new Error("Trusted owner, world epoch and game character binding are required for portability");
  return result;
}
function feedbackPayloadOf(value) {
  object(value, ["source", "context", "feedbackText"], "feedback payload");
  if (!["local-owner", "gateway-owner"].includes(value.source)) throw new Error("Teaching requires the local-owner or trusted gateway-owner control source");
  return { source: value.source, context: contextOf(value.context), feedbackText: value.feedbackText === null ? null : text(value.feedbackText, "feedbackText") };
}
function feedbackEpisodeOf(value) {
  object(value, ["kind", "action", "outcome", "feedbackId", "source", "context", "feedbackText"], "teaching episode");
  if (value.kind !== "feedback" || value.action !== "teach-support-before-loot" || value.outcome !== "accepted") throw new Error("Unsupported teaching episode");
  return { kind: "feedback", action: "teach-support-before-loot", outcome: "accepted", feedbackId: archiveId(value.feedbackId, "feedbackId"), ...feedbackPayloadOf({ source: value.source, context: value.context, feedbackText: value.feedbackText }) };
}
function arrayOf(value, label) {
  if (!Array.isArray(value) || value.length > MAX_ROWS) throw new TypeError(label + " exceeds the supported record limit");
  return value;
}
function validateGrowth(value) {
  plainRecord(value, ["growthRevision", "policyVersion", "habits", "feedback", "episodes", "relations", "operations"], "playerGrowth");
  nonnegativeInteger(value.growthRevision, "growthRevision");
  if (!Number.isSafeInteger(value.policyVersion) || value.policyVersion < 1) throw new Error("Invalid policy version");
  const habits = habitsOf(value.habits), episodeIds = new Set(), feedbackIds = new Set(), operationIds = new Set();
  const episodes = arrayOf(value.episodes, "episodes").map(row => {
    plainRecord(row, ["episodeId", "sequence", "recordedAt", "policyVersion", "status", "payloadHash", "payload"], "episode row");
    archiveId(row.episodeId, "episodeId"); nonnegativeInteger(row.recordedAt, "recordedAt");
    if (!Number.isSafeInteger(row.sequence) || row.sequence < 1 || !Number.isSafeInteger(row.policyVersion) || row.policyVersion < 1 || row.policyVersion > value.policyVersion ||
        !["active", "revoked", "deleted"].includes(row.status) || !/^[a-f0-9]{64}$/.test(row.payloadHash) || episodeIds.has(row.episodeId)) throw new Error("Invalid or duplicate episode");
    episodeIds.add(row.episodeId);
    const payload = row.status === "deleted" ? null : row.payload?.kind === "feedback" ? feedbackEpisodeOf(row.payload) : episodeOf(row.payload);
    if (row.status === "deleted" && row.payload !== null || payload && payloadHash(payload) !== row.payloadHash) throw new Error("Invalid episode content hash");
    return { ...row, payload };
  });
  if (episodes.some((row, index) => index && row.sequence <= episodes[index - 1].sequence)) throw new Error("Episode order is invalid");
  const byEpisode = new Map(episodes.map(row => [row.episodeId, row])), feedbackSources = new Set();
  const feedback = arrayOf(value.feedback, "feedback").map(row => {
    plainRecord(row, ["feedbackId", "episodeId", "status", "payloadHash", "payload"], "feedback row");
    archiveId(row.feedbackId, "feedbackId"); archiveId(row.episodeId, "episodeId");
    if (!["active", "revoked", "deleted"].includes(row.status) || !/^[a-f0-9]{64}$/.test(row.payloadHash) || feedbackIds.has(row.feedbackId)) throw new Error("Invalid or duplicate feedback");
    feedbackIds.add(row.feedbackId);
    const episode = byEpisode.get(row.episodeId);
    if (!episode || episode.status !== row.status || feedbackSources.has(row.episodeId)) throw new Error("Feedback source episode is missing, shared or inactive");
    feedbackSources.add(row.episodeId);
    const payload = row.status === "deleted" ? null : feedbackPayloadOf(row.payload);
    if (row.status === "deleted" && row.payload !== null || payload && payloadHash(payload) !== row.payloadHash) throw new Error("Invalid feedback content hash");
    if (payload && (episode.payload?.feedbackId !== row.feedbackId || canonicalJson(payload) !== canonicalJson({ source: episode.payload.source, context: episode.payload.context, feedbackText: episode.payload.feedbackText }))) throw new Error("Feedback and source episode disagree");
    return { ...row, payload };
  });
  if (episodes.some(row => row.payload?.kind === "feedback" && !feedbackIds.has(row.payload.feedbackId))) throw new Error("Teaching episode has no feedback source");
  const hasTeaching = feedback.some(row => row.status === "active");
  if (PARTY_KEYS.some(key => habits.party[key] !== hasTeaching)) throw new Error("Imported habits disagree with active teaching");
  const operations = arrayOf(value.operations, "operations").map(row => {
    plainRecord(row, ["operationId", "kind", "payloadHash"], "operation row");
    archiveId(row.operationId, "operationId"); identifier(row.kind, "operation kind");
    if (!["teach", "revoke-feedback", "delete-feedback", "delete-episode", "clear-growth"].includes(row.kind) || !/^[a-f0-9]{64}$/.test(row.payloadHash) || operationIds.has(row.operationId)) throw new Error("Invalid or duplicate operation");
    operationIds.add(row.operationId); return clone(row);
  });
  const relationIds = new Set();
  const relations = arrayOf(value.relations, "relations").map(row => {
    plainRecord(row, ["relationId", "subjectRef", "kind", "sourceEpisodeIds", "summary"], "relation");
    archiveId(row.relationId, "relationId"); ref(row.subjectRef, "subjectRef");
    if (!["session-observation", "configured-owner"].includes(row.kind) || relationIds.has(row.relationId) || !Array.isArray(row.sourceEpisodeIds) || !row.sourceEpisodeIds.length ||
        row.sourceEpisodeIds.length > 100 || new Set(row.sourceEpisodeIds).size !== row.sourceEpisodeIds.length ||
        row.sourceEpisodeIds.some(id => !byEpisode.has(id) || byEpisode.get(id).status !== "active")) throw new Error("Invalid relation evidence");
    relationIds.add(row.relationId);
    return { relationId: row.relationId, subjectRef: row.subjectRef, kind: row.kind, sourceEpisodeIds: [...row.sourceEpisodeIds], ...(row.summary === undefined ? {} : { summary: text(row.summary, "relation summary") }) };
  });
  return { growthRevision: value.growthRevision, policyVersion: value.policyVersion, habits, feedback, episodes, relations, operations };
}
export class CompanionConflictError extends Error {
  constructor(view) { super("Companion policy version conflict"); this.code = "conflict"; this.currentView = view; }
}

/** Identity and growth only. Runtime settings and game assets never enter this database. */
export class CompanionStore {
  #db;
  #binding;

  constructor(options) {
    const { path, binding, ...settings } = options;
    const imported = settings[IMPORT_DATA]; delete settings[IMPORT_DATA];
    object(settings, ["archiveId", "ownerRef", "worldEpoch", "displayName", "appearance", "baseSetting", "adoptLegacy"], "archive configuration");
    if (settings.adoptLegacy !== undefined && typeof settings.adoptLegacy !== "boolean") throw new TypeError("adoptLegacy must be an explicit boolean");
    if (typeof path !== "string" || !path.trim() || path === ":memory:") throw new TypeError("A durable companion database path is required");
    this.#binding = bindingOf(binding);
    mkdirSync(dirname(resolve(path)), { recursive: true });
    this.#db = new DatabaseSync(resolve(path));
    try {
      this.#db.exec("PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;");
      const version = this.#db.prepare("PRAGMA user_version").get().user_version;
      if (imported && version !== 0) throw new Error("Import requires a fresh destination");
      if (![0, 1, SCHEMA_VERSION].includes(version)) throw new Error("Unsupported companion schema version");
      if (version === 0) {
        if (this.#db.prepare("SELECT COUNT(*) AS count FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").get().count) throw new Error("Unsupported unversioned companion database");
        this.#transaction(() => { this.#createSchema(); this.#initialize(settings, imported); });
      } else if (version === 1) {
        if ((settings.ownerRef !== undefined || settings.worldEpoch !== undefined) && settings.adoptLegacy !== true) throw new Error("Explicit legacy adoption is required");
        this.#transaction(() => this.#migrate(settings));
      } else {
        this.#readProfile();
        this.#transaction(() => this.#checkArchive(settings));
      }
      this.#db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;");
    } catch (error) { this.#db.close(); this.#db = null; throw error; }
  }

  #open() { if (!this.#db) throw new Error("CompanionStore is closed"); return this.#db; }

  #transaction(operation) {
    const db = this.#open(); db.exec("BEGIN IMMEDIATE");
    try { const result = operation(); db.exec("COMMIT"); return result; }
    catch (error) { db.exec("ROLLBACK"); throw error; }
  }

  #createSchema() {
    this.#db.exec([
      "CREATE TABLE companion_state (singleton INTEGER PRIMARY KEY CHECK(singleton=1),schema_version INTEGER NOT NULL,binding_json TEXT NOT NULL,profile_json TEXT NOT NULL);",
      "CREATE TABLE episodes (sequence INTEGER PRIMARY KEY AUTOINCREMENT,episode_id TEXT NOT NULL UNIQUE,recorded_at INTEGER NOT NULL,policy_version INTEGER NOT NULL,payload_json TEXT NOT NULL,status TEXT NOT NULL DEFAULT 'active',payload_hash TEXT NOT NULL);",
      "CREATE TABLE feedback (feedback_id TEXT PRIMARY KEY,payload_json TEXT NOT NULL,result_profile_json TEXT NOT NULL,episode_id TEXT NOT NULL UNIQUE REFERENCES episodes(episode_id),status TEXT NOT NULL DEFAULT 'active',payload_hash TEXT NOT NULL);",
      "CREATE TABLE character_archive (singleton INTEGER PRIMARY KEY CHECK(singleton=1),archive_json TEXT NOT NULL,growth_revision INTEGER NOT NULL);",
      "CREATE TABLE operations (operation_id TEXT PRIMARY KEY,operation_kind TEXT NOT NULL,payload_hash TEXT NOT NULL);",
      "CREATE TABLE relations (relation_id TEXT PRIMARY KEY,payload_json TEXT NOT NULL);",
      "CREATE TABLE import_receipts (export_id TEXT PRIMARY KEY,envelope_hash TEXT NOT NULL,growth_hash TEXT NOT NULL);",
      "PRAGMA user_version=2;",
    ].join("\n"));
  }

  #newArchive(settings) {
    if (settings.ownerRef !== undefined && this.#binding.operatorRef !== undefined && settings.ownerRef !== this.#binding.operatorRef) throw new Error("Existing operator binding cannot be replaced");
    return characterArchiveOf({
      archiveId: settings.archiveId ?? randomUUID(), residentId: this.#binding.residentId,
      ownerRef: settings.ownerRef ?? this.#binding.operatorRef ?? null, createdAt: Date.now(),
      displayName: settings.displayName ?? this.#binding.characterName, realmId: this.#binding.realmId,
      worldEpoch: settings.worldEpoch ?? null, characterName: this.#binding.characterName, characterIndex: this.#binding.characterIndex ?? null,
      appearance: appearanceOf(settings.appearance), baseSetting: settings.baseSetting ?? "",
    });
  }

  #initialize(settings, imported) {
    const archive = imported?.characterArchive ?? this.#newArchive(settings), profile = defaultProfile(this.#binding);
    if (imported) { profile.policyVersion = imported.playerGrowth.policyVersion; profile.solo = imported.playerGrowth.habits.solo; profile.party = imported.playerGrowth.habits.party; }
    this.#db.prepare("INSERT INTO companion_state VALUES (1,?,?,?)").run(SCHEMA_VERSION, JSON.stringify(this.#binding), JSON.stringify(profile));
    this.#db.prepare("INSERT INTO character_archive VALUES (1,?,?)").run(JSON.stringify(archive), imported?.playerGrowth.growthRevision ?? 0);
    if (!imported) return;
    for (const row of imported.playerGrowth.episodes) this.#db.prepare("INSERT INTO episodes(sequence,episode_id,recorded_at,policy_version,payload_json,status,payload_hash) VALUES(?,?,?,?,?,?,?)")
      .run(row.sequence, row.episodeId, row.recordedAt, row.policyVersion, row.payload === null ? "{}" : JSON.stringify(row.payload), row.status, row.payloadHash);
    for (const row of imported.playerGrowth.feedback) this.#db.prepare("INSERT INTO feedback VALUES(?,?,?,?,?,?)")
      .run(row.feedbackId, row.payload === null ? "{}" : JSON.stringify(row.payload), "{}", row.episodeId, row.status, row.payloadHash);
    for (const row of imported.playerGrowth.operations) this.#db.prepare("INSERT INTO operations VALUES(?,?,?)").run(row.operationId, row.kind, row.payloadHash);
    for (const row of imported.playerGrowth.relations) this.#db.prepare("INSERT INTO relations VALUES(?,?)").run(row.relationId, JSON.stringify(row));
    this.#db.prepare("INSERT INTO import_receipts VALUES(?,?,?)").run(imported.exportId, imported.envelopeHash, payloadHash(imported.playerGrowth));
  }

  #migrate(settings) {
    const row = this.#db.prepare("SELECT * FROM companion_state WHERE singleton=1").get();
    if (!row || row.schema_version !== 1 || canonicalJson(JSON.parse(row.binding_json)) !== canonicalJson(this.#binding)) throw new Error("Companion database binding does not match this resident");
    const profile = profileOf(JSON.parse(row.profile_json), this.#binding, 1);
    const episodes = this.#db.prepare("SELECT * FROM episodes ORDER BY sequence").all();
    const feedback = this.#db.prepare("SELECT * FROM feedback").all();
    for (const value of episodes) value.payload = JSON.parse(value.payload_json).kind === "feedback" ? feedbackEpisodeOf(JSON.parse(value.payload_json)) : episodeOf(JSON.parse(value.payload_json));
    for (const value of feedback) value.payload = feedbackPayloadOf(JSON.parse(value.payload_json));
    if (PARTY_KEYS.some(key => profile.party[key] !== (feedback.length > 0))) throw new Error("Legacy policy and feedback evidence disagree");
    const archive = this.#newArchive(settings);
    this.#db.exec([
      "ALTER TABLE episodes ADD COLUMN status TEXT NOT NULL DEFAULT 'active';",
      "ALTER TABLE episodes ADD COLUMN payload_hash TEXT NOT NULL DEFAULT '';",
      "ALTER TABLE feedback ADD COLUMN status TEXT NOT NULL DEFAULT 'active';",
      "ALTER TABLE feedback ADD COLUMN payload_hash TEXT NOT NULL DEFAULT '';",
      "CREATE TABLE character_archive (singleton INTEGER PRIMARY KEY CHECK(singleton=1),archive_json TEXT NOT NULL,growth_revision INTEGER NOT NULL);",
      "CREATE TABLE operations (operation_id TEXT PRIMARY KEY,operation_kind TEXT NOT NULL,payload_hash TEXT NOT NULL);",
      "CREATE TABLE relations (relation_id TEXT PRIMARY KEY,payload_json TEXT NOT NULL);",
      "CREATE TABLE import_receipts (export_id TEXT PRIMARY KEY,envelope_hash TEXT NOT NULL,growth_hash TEXT NOT NULL);",
    ].join("\n"));
    for (const value of episodes) this.#db.prepare("UPDATE episodes SET payload_hash=? WHERE episode_id=?").run(payloadHash(value.payload), value.episode_id);
    for (const value of feedback) this.#db.prepare("UPDATE feedback SET payload_hash=? WHERE feedback_id=?").run(payloadHash(value.payload), value.feedback_id);
    profile.schemaVersion = SCHEMA_VERSION;
    this.#db.prepare("UPDATE companion_state SET schema_version=?,profile_json=? WHERE singleton=1").run(SCHEMA_VERSION, JSON.stringify(profile));
    this.#db.prepare("INSERT INTO character_archive VALUES(1,?,?)").run(JSON.stringify(archive), episodes.length);
    this.#db.exec("PRAGMA user_version=2");
    validateGrowth(this.growth());
  }

  #checkArchive(settings) {
    const archive = this.archive();
    const adopting = archive.worldEpoch === null && settings.adoptLegacy === true;
    for (const key of ["archiveId", "ownerRef", "worldEpoch", "displayName", "appearance", "baseSetting"]) {
      if (settings[key] === undefined || canonicalJson(archive[key]) === canonicalJson(settings[key])) continue;
      if (adopting && key !== "archiveId" && (key !== "ownerRef" || archive.ownerRef === null)) archive[key] = clone(settings[key]);
      else throw new Error("Companion archive binding does not match trusted configuration");
    }
    if (this.#binding.operatorRef !== undefined && archive.ownerRef !== null && archive.ownerRef !== this.#binding.operatorRef) throw new Error("Existing operator binding cannot be replaced");
    if (canonicalJson(archive) !== canonicalJson(this.archive())) this.#db.prepare("UPDATE character_archive SET archive_json=? WHERE singleton=1").run(JSON.stringify(characterArchiveOf(archive)));
  }

  #readProfile() {
    const row = this.#open().prepare("SELECT * FROM companion_state WHERE singleton=1").get();
    if (!row || row.schema_version !== SCHEMA_VERSION || canonicalJson(JSON.parse(row.binding_json)) !== canonicalJson(this.#binding)) throw new Error("Companion database binding does not match this resident");
    return profileOf(JSON.parse(row.profile_json), this.#binding);
  }

  profile() { return clone(this.#readProfile()); }

  archive() {
    const row = this.#open().prepare("SELECT archive_json FROM character_archive WHERE singleton=1").get();
    if (!row) throw new Error("Companion archive is missing");
    const archive = characterArchiveOf(JSON.parse(row.archive_json));
    if (archive.realmId !== this.#binding.realmId || archive.residentId !== this.#binding.residentId || archive.characterName !== this.#binding.characterName ||
        archive.characterIndex !== (this.#binding.characterIndex ?? null)) throw new Error("Companion archive character binding mismatch");
    return archive;
  }

  growth() {
    const profile = this.#readProfile();
    return {
      growthRevision: this.#open().prepare("SELECT growth_revision FROM character_archive WHERE singleton=1").get().growth_revision,
      policyVersion: profile.policyVersion, habits: { solo: clone(profile.solo), party: clone(profile.party) },
      episodes: this.#db.prepare("SELECT * FROM episodes ORDER BY sequence").all().map(row => ({
        episodeId: row.episode_id, sequence: row.sequence, recordedAt: row.recorded_at, policyVersion: row.policy_version,
        status: row.status, payloadHash: row.payload_hash, payload: row.status === "deleted" ? null : JSON.parse(row.payload_json),
      })),
      feedback: this.#db.prepare("SELECT * FROM feedback ORDER BY rowid").all().map(row => ({
        feedbackId: row.feedback_id, episodeId: row.episode_id, status: row.status, payloadHash: row.payload_hash, payload: row.status === "deleted" ? null : JSON.parse(row.payload_json),
      })),
      relations: this.#db.prepare("SELECT payload_json FROM relations ORDER BY relation_id").all().map(row => JSON.parse(row.payload_json)),
      operations: this.#db.prepare("SELECT * FROM operations ORDER BY operation_id").all().map(row => ({ operationId: row.operation_id, kind: row.operation_kind, payloadHash: row.payload_hash })),
    };
  }

  #authorize(input) {
    const source = input.source ?? "local-owner";
    if (!["local-owner", "gateway-owner"].includes(source)) throw new Error("Teaching requires the local-owner or trusted gateway-owner control source");
    const archive = this.archive();
    if (source === "gateway-owner" && (!archive.ownerRef || !archive.worldEpoch || input.ownerRef !== archive.ownerRef || input.archiveId !== archive.archiveId || input.worldEpoch !== archive.worldEpoch)) throw new Error("Trusted companion owner binding mismatch");
    if (input.expectedPolicyVersion !== undefined && (!Number.isSafeInteger(input.expectedPolicyVersion) || input.expectedPolicyVersion < 1)) throw new TypeError("Invalid expected policy version");
    if (source === "gateway-owner" && input.expectedPolicyVersion === undefined) throw new TypeError("Gateway owner mutation requires an expected policy version");
    if (source === "gateway-owner" && input.operationId === undefined) throw new TypeError("Gateway owner mutation requires an operation ID");
  }

  #mutate(input, kind, fingerprint, change) {
    this.#authorize(input);
    const operationId = input.operationId === undefined ? null : archiveId(input.operationId, "operationId");
    const hash = payloadHash({ kind, ...fingerprint, ...(input.expectedPolicyVersion === undefined ? {} : { expectedPolicyVersion: input.expectedPolicyVersion }) });
    return this.#transaction(() => {
      if (operationId) {
        const old = this.#db.prepare("SELECT * FROM operations WHERE operation_id=?").get(operationId);
        if (old) {
          if (old.operation_kind !== kind || old.payload_hash !== hash) throw new Error("operationId conflicts with a different payload");
          return false;
        }
      }
      if (input.expectedPolicyVersion !== undefined && this.#readProfile().policyVersion !== input.expectedPolicyVersion) throw new CompanionConflictError(this.ownerView());
      const changed = change();
      if (operationId) this.#db.prepare("INSERT INTO operations VALUES(?,?,?)").run(operationId, kind, hash);
      if (changed || operationId) this.#bumpGrowth();
      return changed;
    });
  }

  #bumpGrowth() {
    const revision = this.#db.prepare("SELECT growth_revision FROM character_archive WHERE singleton=1").get().growth_revision;
    if (revision === Number.MAX_SAFE_INTEGER) throw new Error("Companion growth revision is exhausted");
    this.#db.prepare("UPDATE character_archive SET growth_revision=growth_revision+1 WHERE singleton=1").run();
  }

  #afterCommit(view) {
    try { return view(); }
    catch (error) { error.committed = true; throw error; }
  }

  #recompute({ clear = false } = {}) {
    const profile = this.#readProfile();
    const active = !clear && this.#db.prepare("SELECT COUNT(*) AS count FROM feedback WHERE status='active'").get().count > 0;
    const party = Object.fromEntries(PARTY_KEYS.map(key => [key, active]));
    const solo = clear ? { considerLootWhenSafe: true } : profile.solo;
    if (canonicalJson(profile.party) !== canonicalJson(party) || canonicalJson(profile.solo) !== canonicalJson(solo)) {
      if (profile.policyVersion === Number.MAX_SAFE_INTEGER) throw new Error("Companion policy version is exhausted");
      profile.policyVersion += 1;
    }
    profile.party = party; profile.solo = solo;
    this.#db.prepare("UPDATE companion_state SET profile_json=? WHERE singleton=1").run(JSON.stringify(profile));
    return profile;
  }

  teachSupportBeforeLoot(input) {
    object(input, FEEDBACK_KEYS, "feedback");
    const feedbackId = archiveId(input.feedbackId ?? input.operationId ?? randomUUID(), "feedbackId");
    const payload = feedbackPayloadOf({ source: input.source ?? "local-owner", context: contextOf(input.context), feedbackText: input.feedbackText === undefined ? null : input.feedbackText });
    this.#mutate(input, "teach", { feedbackId, payload }, () => {
      const existing = this.#db.prepare("SELECT * FROM feedback WHERE feedback_id=?").get(feedbackId);
      if (existing) {
        if (existing.payload_hash !== payloadHash(payload)) throw new Error("feedbackId conflicts with a different feedback payload");
        return false;
      }
      const before = this.#readProfile();
      const changed = PARTY_KEYS.some(key => !before.party[key]);
      if (changed && before.policyVersion === Number.MAX_SAFE_INTEGER) throw new Error("Companion policy version is exhausted");
      const version = before.policyVersion + (changed ? 1 : 0), episodeId = randomUUID();
      const event = { kind: "feedback", action: "teach-support-before-loot", outcome: "accepted", feedbackId, ...payload };
      this.#db.prepare("INSERT INTO episodes(episode_id,recorded_at,policy_version,payload_json,status,payload_hash) VALUES(?,?,?,?,?,?)")
        .run(episodeId, Date.now(), version, JSON.stringify(event), "active", payloadHash(event));
      this.#db.prepare("INSERT INTO feedback VALUES(?,?,?,?,?,?)").run(feedbackId, JSON.stringify(payload), "{}", episodeId, "active", payloadHash(payload));
      this.#recompute();
      return true;
    });
    return this.#afterCommit(() => this.profile());
  }

  #eraseFeedback(id, status) {
    const row = this.#db.prepare("SELECT * FROM feedback WHERE feedback_id=?").get(id);
    if (!row) throw new Error("Feedback was not found");
    if (row.status === "deleted" || row.status === status) return false;
    if (status === "deleted") {
      this.#db.prepare("UPDATE feedback SET status='deleted',payload_json='{}',result_profile_json='{}' WHERE feedback_id=?").run(id);
      this.#db.prepare("UPDATE episodes SET status='deleted',payload_json='{}' WHERE episode_id=?").run(row.episode_id);
    } else {
      this.#db.prepare("UPDATE feedback SET status='revoked' WHERE feedback_id=?").run(id);
      this.#db.prepare("UPDATE episodes SET status='revoked' WHERE episode_id=?").run(row.episode_id);
    }
    this.#invalidateRelations(row.episode_id);
    this.#recompute();
    return true;
  }

  #invalidateRelations(episodeId) {
    for (const row of this.#db.prepare("SELECT * FROM relations").all()) {
      if (JSON.parse(row.payload_json).sourceEpisodeIds.includes(episodeId)) this.#db.prepare("DELETE FROM relations WHERE relation_id=?").run(row.relation_id);
    }
  }

  revokeFeedback(input) {
    object(input, [...AUTH_KEYS, "feedbackId"], "revoke feedback");
    const id = archiveId(input.feedbackId, "feedbackId");
    this.#mutate(input, "revoke-feedback", { feedbackId: id }, () => this.#eraseFeedback(id, "revoked"));
    return this.#afterCommit(() => this.ownerView());
  }

  deleteFeedback(input) {
    object(input, [...AUTH_KEYS, "feedbackId"], "delete feedback");
    const id = archiveId(input.feedbackId, "feedbackId");
    this.#mutate(input, "delete-feedback", { feedbackId: id }, () => this.#eraseFeedback(id, "deleted"));
    return this.#afterCommit(() => this.ownerView());
  }

  deleteEpisode(input) {
    object(input, [...AUTH_KEYS, "episodeId"], "delete episode");
    const id = archiveId(input.episodeId, "episodeId");
    this.#mutate(input, "delete-episode", { episodeId: id }, () => {
      const row = this.#db.prepare("SELECT * FROM episodes WHERE episode_id=?").get(id);
      if (!row) throw new Error("Episode was not found");
      if (row.status === "deleted") return false;
      const feedback = this.#db.prepare("SELECT feedback_id FROM feedback WHERE episode_id=?").get(id);
      if (feedback) return this.#eraseFeedback(feedback.feedback_id, "deleted");
      this.#db.prepare("UPDATE episodes SET status='deleted',payload_json='{}' WHERE episode_id=?").run(id);
      this.#invalidateRelations(id);
      return true;
    });
    return this.#afterCommit(() => this.ownerView());
  }

  clearGrowth(input) {
    object(input, [...AUTH_KEYS, "confirmArchiveId"], "clear growth");
    if (input.confirmArchiveId !== this.archive().archiveId) throw new Error("Explicit archive confirmation is required to clear growth");
    this.#mutate(input, "clear-growth", { confirmArchiveId: input.confirmArchiveId }, () => {
      const current = this.#readProfile();
      const changed = this.#db.prepare("SELECT COUNT(*) AS count FROM episodes WHERE status!='deleted'").get().count > 0 || this.#db.prepare("SELECT COUNT(*) AS count FROM relations").get().count > 0 ||
        !current.solo.considerLootWhenSafe || PARTY_KEYS.some(key => current.party[key]);
      this.#db.exec("UPDATE feedback SET status='deleted',payload_json='{}',result_profile_json='{}'; UPDATE episodes SET status='deleted',payload_json='{}'; DELETE FROM relations;");
      this.#recompute({ clear: true });
      return changed;
    });
    return this.#afterCommit(() => this.ownerView());
  }

  recordEpisode(input) {
    const payload = episodeOf(input), episodeId = archiveId(input.episodeId ?? randomUUID(), "episodeId"), hash = payloadHash(payload);
    return this.#transaction(() => {
      const old = this.#db.prepare("SELECT * FROM episodes WHERE episode_id=?").get(episodeId);
      if (old) {
        if (old.payload_hash !== hash) throw new Error("episodeId conflicts with a different episode payload");
        return this.#episodeView(old);
      }
      const profile = this.#readProfile();
      this.#db.prepare("INSERT INTO episodes(episode_id,recorded_at,policy_version,payload_json,status,payload_hash) VALUES(?,?,?,?,?,?)")
        .run(episodeId, Date.now(), profile.policyVersion, JSON.stringify(payload), "active", hash);
      this.#bumpGrowth();
      return this.#episodeView(this.#db.prepare("SELECT * FROM episodes WHERE episode_id=?").get(episodeId));
    });
  }

  #episodeView(row) {
    return { episodeId: row.episode_id, recordedAt: row.recorded_at, policyVersion: row.policy_version, status: row.status, ...(row.status === "deleted" ? {} : JSON.parse(row.payload_json)) };
  }

  recentEpisodes(limit = 20) {
    if (!Number.isSafeInteger(limit) || limit < 0) throw new TypeError("Episode limit must be a nonnegative safe integer");
    return this.#open().prepare("SELECT * FROM episodes WHERE status!='deleted' ORDER BY sequence DESC LIMIT ?").all(Math.min(limit, 100)).map(row => this.#episodeView(row));
  }

  ownerView({ cursor = null, limit = 20 } = {}) {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 50 || cursor !== null && (typeof cursor !== "string" || !/^[1-9][0-9]{0,15}$/.test(cursor) || !Number.isSafeInteger(Number(cursor)))) throw new TypeError("Invalid owner view pagination");
    const rows = this.#open().prepare("SELECT * FROM episodes WHERE sequence<? ORDER BY sequence DESC LIMIT ?").all(cursor === null ? Number.MAX_SAFE_INTEGER : Number(cursor), limit + 1);
    const selected = rows.slice(0, limit), feedback = [];
    const preview = value => [...value].slice(0, 160).join("");
    for (const row of selected) {
      const item = this.#db.prepare("SELECT * FROM feedback WHERE episode_id=?").get(row.episode_id);
      if (item) feedback.push({ feedbackId: item.feedback_id, status: item.status, recordedAt: row.recorded_at,
        ...(item.status === "deleted" ? {} : { summary: preview(JSON.parse(item.payload_json).feedbackText ?? "组队先支援，战斗后拾取") }) });
    }
    const profile = this.#readProfile();
    return { archive: this.archive(), profile: { policyVersion: profile.policyVersion, style: profile.style, party: clone(profile.party) },
      growthRevision: this.#db.prepare("SELECT growth_revision FROM character_archive WHERE singleton=1").get().growth_revision,
      feedback, episodes: selected.map(row => {
        const value = this.#episodeView(row), visible = {};
        for (const key of ["episodeId", "recordedAt", "policyVersion", "status", "kind", "action", "outcome"]) if (value[key] !== undefined) visible[key] = value[key];
        if (value.summary !== undefined) visible.summary = preview(value.summary);
        return visible;
      }), nextCursor: rows.length > limit ? String(selected.at(-1).sequence) : null };
  }

  exportArchive({ key }) {
    return this.#transaction(() => {
      const characterArchive = characterArchiveOf(this.archive(), { trusted: true });
      return signArchive({ characterArchive, playerGrowth: validateGrowth(this.growth()), key });
    });
  }

  static importArchive({ path, binding, archiveId: expectedId, ownerRef, worldEpoch, key, envelope }) {
    const verified = verifyArchive(envelope, key);
    const characterArchive = characterArchiveOf(verified.characterArchive, { trusted: true });
    const playerGrowth = validateGrowth(verified.playerGrowth), expectedBinding = bindingOf(binding);
    archiveId(expectedId, "expected archiveId"); archiveId(ownerRef, "expected ownerRef"); archiveId(worldEpoch, "expected worldEpoch");
    if (characterArchive.archiveId !== expectedId || characterArchive.ownerRef !== ownerRef || characterArchive.worldEpoch !== worldEpoch ||
        characterArchive.realmId !== expectedBinding.realmId || characterArchive.residentId !== expectedBinding.residentId ||
        characterArchive.characterName !== expectedBinding.characterName || characterArchive.characterIndex !== expectedBinding.characterIndex ||
        expectedBinding.operatorRef !== undefined && expectedBinding.operatorRef !== ownerRef) throw new Error("Archive does not match trusted owner/world/character binding");
    if (typeof path !== "string" || !path.trim() || path === ":memory:") throw new TypeError("A fresh durable archive destination is required");
    const filename = resolve(path), existed = existsSync(filename), envelopeHash = payloadHash(verified);
    if (existed) {
      const db = new DatabaseSync(filename, { readOnly: true });
      let receipt = null, tables;
      try {
        tables = db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").all();
        if (tables.some(row => row.name === "import_receipts")) receipt = db.prepare("SELECT * FROM import_receipts WHERE export_id=?").get(verified.exportId);
      } finally { db.close(); }
      if (tables.length) {
        if (!receipt || receipt.envelope_hash !== envelopeHash) throw new Error("Import requires a fresh destination; existing growth cannot be overwritten");
        const store = new CompanionStore({ path, binding, archiveId: expectedId, ownerRef, worldEpoch });
        if (payloadHash(store.growth()) !== receipt.growth_hash) { store.close(); throw new Error("Import cannot roll back existing character growth"); }
        return store;
      }
    }
    return new CompanionStore({ path, binding, archiveId: expectedId, ownerRef, worldEpoch,
      [IMPORT_DATA]: { ...verified, characterArchive, playerGrowth, envelopeHash } });
  }

  close() { if (this.#db) { this.#db.close(); this.#db = null; } }
}


