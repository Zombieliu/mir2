import { DatabaseSync } from "node:sqlite";
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const COST_KINDS = ["model", "hosting", "payment", "revenueShare"];
const ENTRY_SOURCES = ["unprompted", "reminder", "scheduled", "unknown"];
const CHOICE_REASONS = ["shared-history", "stronger", "setup-ease", "curiosity", "other"];
const MAX_VISIT_MS = 60 * 60 * 1000;
const MAX_MONEY = 9_007_199_254_740_991n;
const copy = value => structuredClone(value);
const hash = value => createHash("sha256").update(JSON.stringify(value)).digest("hex");

function plain(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some(key => !keys.includes(key))) throw new TypeError("Unsupported experience fields");
  return value;
}
function ref(value) {
  if (typeof value !== "string" || !/^[A-Za-z0-9._:/@+-]{1,160}$/.test(value)) throw new TypeError("Invalid experience reference");
  return value;
}
function note(value) {
  if (value === undefined) return undefined;
  if (typeof value !== "string" || [...value].length > 300 || /[\p{Cc}\p{Cf}\p{Cs}]/u.test(value) || /(?:password|token|secret|api[ _-]?key)\s*[:=]|\bbearer\s|\bsk-[a-z0-9]{8,}/iu.test(value)) throw new TypeError("Invalid experience note");
  return value.trim();
}
function number(value, maximum = Number.MAX_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) throw new TypeError("Invalid nonnegative integer");
  return value;
}
function micros(value) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,15})$/.test(value) || BigInt(value) > MAX_MONEY) throw new TypeError("Money must be nonnegative decimal micros");
  return value;
}
function offerOf(input, currency) {
  if (input == null) return null;
  plain(input, ["offerId", "currency", "priceMicros", "durationDays", "adventureCount", "paymentInstructions"]);
  if (input.currency !== currency || BigInt(micros(input.priceMicros)) <= 0n) throw new TypeError("Offer needs a fixed positive price in the experiment currency");
  const durationDays = number(input.durationDays, 365), adventureCount = number(input.adventureCount, 1000);
  if (!durationDays || !adventureCount) throw new TypeError("Offer needs a service duration and adventure allowance");
  return { offerId: ref(input.offerId), currency, priceMicros: input.priceMicros, durationDays, adventureCount, paymentInstructions: note(input.paymentInstructions) ?? "" };
}

export function participantEventOf(input) {
  const shapes = {
    visit: ["kind", "visitId", "entrySource", "reminderCount"],
    visitEnd: ["kind", "visitId"],
    choice: ["kind", "visitId", "reason", "reasonText"],
    teachingCheck: ["kind", "visitId", "understood", "reasonText"],
    purchaseIntent: ["kind", "visitId", "offerId"],
  };
  if (!input || !shapes[input.kind]) throw new TypeError("Unknown participant event");
  plain(input, shapes[input.kind]);
  const value = { kind: input.kind, visitId: ref(input.visitId) };
  if (input.kind === "visit") {
    if (!ENTRY_SOURCES.includes(input.entrySource)) throw new TypeError("Unknown return source");
    Object.assign(value, { entrySource: input.entrySource, reminderCount: number(input.reminderCount, 100) });
  }
  if (input.kind === "choice") {
    if (!CHOICE_REASONS.includes(input.reason)) throw new TypeError("Unknown choice reason");
    value.reason = input.reason;
  }
  if (input.kind === "teachingCheck") {
    if (typeof input.understood !== "boolean") throw new TypeError("Teaching response must be explicit");
    value.understood = input.understood;
  }
  if (input.reasonText !== undefined) value.reasonText = note(input.reasonText);
  if (input.kind === "purchaseIntent") value.offerId = ref(input.offerId);
  return value;
}

/** Local trial bookkeeping. Participant intent, measured activity and operator
 * confirmations have different origins and can never substitute for each other. */
export class ExperienceLedger {
  #db;
  #metadata;
  #clock;

  constructor({ path, experimentId, realmId, worldEpoch, currency = "CNY", environment = "trial", offer = null, clock = Date.now }) {
    if (!["trial", "test"].includes(environment) || !/^[A-Z]{3}$/.test(currency)) throw new TypeError("Invalid experiment environment/currency");
    const metadata = { experimentId: ref(experimentId), realmId: ref(realmId), worldEpoch: ref(worldEpoch), currency, environment };
    if (typeof path !== "string" || !path || path === ":memory:") throw new TypeError("A durable experience ledger path is required");
    this.#clock = clock;
    mkdirSync(dirname(resolve(path)), { recursive: true });
    this.#db = new DatabaseSync(resolve(path));
    try {
      this.#db.exec("PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;");
      const version = this.#db.prepare("PRAGMA user_version").get().user_version;
      if (version !== 0 && version !== 1) throw new Error("Unsupported experience ledger schema");
      if (version === 0) {
        if (this.#db.prepare("SELECT COUNT(*) AS n FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").get().n) throw new Error("Unknown existing ledger");
        this.#transaction(() => {
          this.#db.exec(`
          CREATE TABLE metadata(singleton INTEGER PRIMARY KEY CHECK(singleton=1), binding TEXT NOT NULL, offer TEXT);
          CREATE TABLE participants(owner_ref TEXT PRIMARY KEY, participant_ref TEXT UNIQUE NOT NULL);
          CREATE TABLE events(sequence INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT UNIQUE NOT NULL, owner_ref TEXT NOT NULL REFERENCES participants(owner_ref), archive_id TEXT, kind TEXT NOT NULL, origin TEXT NOT NULL, recorded_at INTEGER NOT NULL, payload_hash TEXT NOT NULL, payload TEXT NOT NULL);
          CREATE TABLE runtime_sessions(session_id TEXT PRIMARY KEY, owner_ref TEXT NOT NULL REFERENCES participants(owner_ref), archive_id TEXT NOT NULL, runtime_id TEXT NOT NULL, started_at INTEGER NOT NULL, finished_at INTEGER, runtime_millis INTEGER NOT NULL DEFAULT 0, shared_millis INTEGER NOT NULL DEFAULT 0, model_calls INTEGER NOT NULL DEFAULT 0, model_micros TEXT NOT NULL DEFAULT '0', coverage_gaps INTEGER NOT NULL DEFAULT 0);
          PRAGMA user_version=1;
          `);
          this.#db.prepare("INSERT INTO metadata VALUES(1, ?, ?)").run(JSON.stringify(metadata), offer == null ? null : JSON.stringify(offerOf(offer, currency)));
        });
      }
      const saved = this.#db.prepare("SELECT binding, offer FROM metadata WHERE singleton=1").get();
      if (!saved || saved.binding !== JSON.stringify(metadata)) throw new Error("Experience ledger belongs to a different experiment/world/currency");
      const supplied = offerOf(offer, currency);
      if (supplied && saved.offer && JSON.stringify(supplied) !== saved.offer) throw new Error("The fixed offer cannot change within this experiment");
      if (supplied && !saved.offer) this.#db.prepare("UPDATE metadata SET offer=? WHERE singleton=1 AND offer IS NULL").run(JSON.stringify(supplied));
      this.#metadata = metadata;
      this.#db.exec("CREATE TABLE IF NOT EXISTS request_receipts(operation_id TEXT PRIMARY KEY, payload_hash TEXT NOT NULL)");
      this.#db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;");
    } catch (error) { this.#db.close(); this.#db = null; throw error; }
  }

  #transaction(fn) {
    this.#db.exec("BEGIN IMMEDIATE");
    try { const value = fn(); this.#db.exec("COMMIT"); return value; }
    catch (error) { this.#db.exec("ROLLBACK"); throw error; }
  }
  registerParticipant(ownerRef, participantRef = `P-${hash(ref(ownerRef)).slice(0, 10)}`) {
    ref(ownerRef); ref(participantRef);
    const row = this.#db.prepare("SELECT participant_ref FROM participants WHERE owner_ref=?").get(ownerRef);
    if (row && row.participant_ref !== participantRef) throw new Error("Participant label is already bound");
    if (!row) {
      this.#db.prepare("INSERT OR IGNORE INTO participants VALUES(?,?)").run(ownerRef, participantRef);
      if (this.#db.prepare("SELECT participant_ref FROM participants WHERE owner_ref=?").get(ownerRef)?.participant_ref !== participantRef) throw new Error("Participant label is already bound");
    }
    return participantRef;
  }
  #rows(ownerRef) {
    return this.#db.prepare("SELECT * FROM events WHERE owner_ref=? ORDER BY sequence").all(ref(ownerRef)).map(row => ({ id: row.event_id, sequence: row.sequence, archiveId: row.archive_id, kind: row.kind, origin: row.origin, at: row.recorded_at, ...JSON.parse(row.payload) }));
  }
  #existing(id, ownerRef, archiveId, kind, origin, payload) {
    const encoded = JSON.stringify({ ownerRef, archiveId, kind, origin, payload });
    const digest = createHash("sha256").update(encoded).digest("hex");
    const row = this.#db.prepare("SELECT payload_hash FROM events WHERE event_id=? UNION ALL SELECT payload_hash FROM request_receipts WHERE operation_id=? LIMIT 1").get(ref(id), id);
    if (row && row.payload_hash !== digest) throw new Error("Experience operation ID conflicts");
    return { exists: Boolean(row), digest };
  }
  #insert(id, ownerRef, archiveId, kind, origin, payload, digest) {
    this.#db.prepare("INSERT INTO events(event_id,owner_ref,archive_id,kind,origin,recorded_at,payload_hash,payload) VALUES(?,?,?,?,?,?,?,?)")
      .run(id, ownerRef, archiveId, kind, origin, number(this.#clock()), digest, JSON.stringify(payload));
    this.#remember(id, digest);
  }
  #remember(id, digest) { this.#db.prepare("INSERT INTO request_receipts VALUES(?,?)").run(id, digest); }
  #visits(rows) {
    return rows.filter(row => row.kind === "visit").map(row => {
      const end = rows.find(candidate => candidate.kind === "visitEnd" && candidate.visitId === row.visitId);
      const later = rows.find(candidate => candidate.kind === "visit" && candidate.sequence > row.sequence);
      return { ...row, endedAt: end?.at ?? later?.at ?? null, endReason: end ? "participant" : later ? "superseded" : null };
    });
  }
  #active(rows) {
    return this.#visits(rows).findLast(visit => visit.endedAt === null && this.#clock() - visit.at <= MAX_VISIT_MS) ?? null;
  }
  offer() {
    const value = this.#db.prepare("SELECT offer FROM metadata WHERE singleton=1").get()?.offer;
    return value ? JSON.parse(value) : null;
  }

  recordParticipantEvent({ operationId, ownerRef, archiveId, event }) {
    ref(ownerRef); ref(archiveId);
    const payload = participantEventOf(event);
    return this.#transaction(() => {
      const operation = this.#existing(operationId, ownerRef, archiveId, payload.kind, "participant-self-report", payload);
      if (!operation.exists) {
        const rows = this.#rows(ownerRef), visit = rows.find(row => row.kind === "visit" && row.visitId === payload.visitId);
        if (payload.kind === "visit") {
          if (visit) {
            if (visit.entrySource !== payload.entrySource || visit.reminderCount !== payload.reminderCount) throw new Error("Visit ID conflicts");
            this.#remember(operationId, operation.digest);
            return this.ownerView(ownerRef, archiveId);
          }
        } else {
          if (!visit || this.#active(rows)?.visitId !== payload.visitId) throw new Error("A current participant visit is required");
          if (payload.kind === "purchaseIntent" && this.offer()?.offerId !== payload.offerId) throw new Error("The service offer is not enabled");
        }
        this.#insert(operationId, ownerRef, archiveId, payload.kind, "participant-self-report", payload, operation.digest);
      }
      return this.ownerView(ownerRef, archiveId);
    });
  }

  recordOperator({ source, operationId, ownerRef, archiveId = null, kind, payload }) {
    if (source !== "local-operator") throw new Error("Operator bookkeeping cannot be recorded by a player request");
    ref(ownerRef); if (archiveId != null) ref(archiveId);
    const shapes = {
      payment: ["paymentId", "offerId", "amountMicros", "currency", "paidAt", "receiptRef"],
      refund: ["paymentId", "amountMicros", "currency", "occurredAt", "receiptRef"],
      grant: ["grantId", "paymentId", "startsAt"],
      cost: ["category", "amountMicros", "currency", "basis", "reference"],
      support: ["seconds", "reference"],
      invitation: ["channel", "occurredAt", "reference"],
      reminder: ["channel", "occurredAt", "reference"],
    };
    if (!shapes[kind]) throw new Error("Unknown operator record");
    plain(payload, shapes[kind]);
    const normalized = {};
    for (const key of shapes[kind]) {
      if (payload[key] === undefined) throw new Error("Incomplete operator record");
      normalized[key] = payload[key];
    }
    if (Object.hasOwn(payload, "amountMicros")) micros(payload.amountMicros);
    if (payload.currency !== undefined && payload.currency !== this.#metadata.currency) throw new Error("Mixed currencies cannot be combined");
    for (const key of ["paymentId", "offerId", "grantId", "receiptRef", "reference"]) if (payload[key] !== undefined) ref(payload[key]);
    for (const key of ["paidAt", "occurredAt", "startsAt"]) if (payload[key] !== undefined) number(payload[key]);
    if (kind === "cost" && (!COST_KINDS.includes(payload.category) || !["actual", "estimate"].includes(payload.basis))) throw new Error("Unknown cost category/basis");
    if (kind === "support") number(payload.seconds, 86_400);
    if (["invitation", "reminder"].includes(kind) && !["manual", "wechat", "email", "other"].includes(payload.channel)) throw new Error("Unknown contact channel");
    return this.#transaction(() => {
      const operation = this.#existing(operationId, ownerRef, archiveId, kind, "operator-confirmed", normalized);
      if (!operation.exists) {
        const rows = this.#rows(ownerRef), offer = this.offer();
        if (kind === "payment") {
          if (!offer || offer.offerId !== payload.offerId || offer.priceMicros !== payload.amountMicros || !archiveId) throw new Error("Payment must match the fixed offer and character");
          if (rows.some(row => row.kind === "payment" && row.paymentId === payload.paymentId)) throw new Error("Payment reference already recorded");
          if (this.#db.prepare("SELECT 1 FROM events WHERE kind='payment' AND json_extract(payload,'$.receiptRef')=?").get(payload.receiptRef)) throw new Error("Payment receipt already recorded");
        }
        if (["refund", "grant"].includes(kind)) {
          const payment = rows.find(row => row.kind === "payment" && row.paymentId === payload.paymentId);
          if (!payment || (archiveId && archiveId !== payment.archiveId)) throw new Error("A matching confirmed payment is required");
          if (kind === "refund") {
            const refunded = rows.filter(row => row.kind === "refund" && row.paymentId === payment.paymentId).reduce((sum, row) => sum + BigInt(row.amountMicros), 0n);
            if (refunded + BigInt(payload.amountMicros) > BigInt(payment.amountMicros)) throw new Error("Refund exceeds the actual payment");
          } else {
            if (!archiveId || rows.some(row => row.kind === "grant" && (row.grantId === payload.grantId || row.paymentId === payload.paymentId))) throw new Error("Service grant already exists or lacks its character");
            if (rows.some(row => row.kind === "refund" && row.paymentId === payload.paymentId)) throw new Error("Refunded payment cannot open a new service grant");
            Object.assign(normalized, { durationDays: offer.durationDays, adventureCount: offer.adventureCount });
          }
        }
        // Idempotency hashes bind supplied fields, not derived package terms.
        this.#insert(operationId, ownerRef, archiveId, kind, "operator-confirmed", normalized, operation.digest);
      }
      return this.ownerView(ownerRef, archiveId);
    });
  }

  startRuntime({ ownerRef, archiveId, runtimeId, sessionId = randomUUID() }) {
    for (const value of [ownerRef, archiveId, runtimeId, sessionId]) ref(value);
    this.#db.prepare("INSERT INTO runtime_sessions(session_id,owner_ref,archive_id,runtime_id,started_at) VALUES(?,?,?,?,?)").run(sessionId, ownerRef, archiveId, runtimeId, number(this.#clock()));
    return sessionId;
  }
  measureRuntime(sessionId, { elapsedMs, sharedParty = false, coverageGap = false } = {}) {
    number(elapsedMs, 60_000);
    this.#db.prepare("UPDATE runtime_sessions SET runtime_millis=runtime_millis+?,shared_millis=shared_millis+?,coverage_gaps=coverage_gaps+? WHERE session_id=? AND finished_at IS NULL")
      .run(elapsedMs, sharedParty ? elapsedMs : 0, coverageGap ? 1 : 0, ref(sessionId));
  }
  stopRuntime(sessionId) {
    this.#db.prepare("UPDATE runtime_sessions SET finished_at=? WHERE session_id=? AND finished_at IS NULL").run(number(this.#clock()), ref(sessionId));
  }
  recordObservedAdventure({ ownerRef, archiveId, evidenceRef }) {
    ref(ownerRef); ref(archiveId); ref(evidenceRef);
    return this.#transaction(() => {
      const rows = this.#rows(ownerRef), active = this.#active(rows);
      if (!active || rows.some(row => row.kind === "observedAdventure" && row.visitId === active.visitId && row.archiveId === archiveId)) return false;
      const choice = rows.findLast(row => row.kind === "choice" && row.visitId === active.visitId);
      if (choice?.archiveId !== archiveId) return false;
      const available = rows.filter(row => row.kind === "grant" && row.archiveId === archiveId && row.startsAt <= this.#clock() && row.startsAt + row.durationDays * 86_400_000 > this.#clock())
        .find(grant => rows.filter(row => row.kind === "observedAdventure" && row.grantId === grant.grantId).length < grant.adventureCount && !rows.some(row => row.kind === "refund" && row.paymentId === grant.paymentId));
      const payload = { visitId: active.visitId, evidenceRef, grantId: available?.grantId ?? null, coverage: "server-observed-own-monster-strike-with-owner-in-party" };
      const id = `usage:${hash({ ownerRef, archiveId, visitId: active.visitId })}`;
      const operation = this.#existing(id, ownerRef, archiveId, "observedAdventure", "measured", payload);
      this.#insert(id, ownerRef, archiveId, "observedAdventure", "measured", payload, operation.digest);
      return true;
    });
  }

  ownerView(ownerRef, archiveId = null) {
    const participant = this.#db.prepare("SELECT participant_ref FROM participants WHERE owner_ref=?").get(ref(ownerRef));
    if (!participant) throw new Error("Unknown participant");
    const rows = this.#rows(ownerRef), visits = this.#visits(rows), active = this.#active(rows);
    const runtimes = this.#db.prepare("SELECT * FROM runtime_sessions WHERE owner_ref=?").all(ownerRef);
    const total = (kind, field = "amountMicros") => rows.filter(row => row.kind === kind).reduce((sum, row) => sum + BigInt(row[field]), 0n);
    const revenue = total("payment") - total("refund");
    const costs = Object.fromEntries(COST_KINDS.map(category => {
      const matching = rows.filter(row => row.kind === "cost" && row.category === category && row.basis === "actual");
      const value = matching.length ? matching.reduce((sum, row) => sum + BigInt(row.amountMicros), 0n).toString() : category === "model" && runtimes.length ? "0" : null;
      return [`${category}Micros`, value];
    }));
    const complete = COST_KINDS.every(category => costs[`${category}Micros`] !== null);
    return copy({ experimentId: this.#metadata.experimentId, participantRef: participant.participant_ref, environment: this.#metadata.environment,
      scope: { kind: "participant", selectedArchiveId: archiveId, meaning: "All visits, purchases, runtime and costs for this participant across their enrolled companions." },
      visitCount: visits.length, activeVisitId: active?.visitId ?? null,
      selectionCount: new Set(rows.filter(row => row.kind === "choice").map(row => row.visitId)).size,
      selectedCompanionSelectionCount: new Set(rows.filter(row => row.kind === "choice" && (!archiveId || row.archiveId === archiveId)).map(row => row.visitId)).size,
      visits: visits.map(visit => ({ visitId: visit.visitId, startedAt: visit.at, endedAt: visit.endedAt, endReason: visit.endReason, entrySource: visit.entrySource, reminderCount: visit.reminderCount, source: "participant-self-report" })),
      choices: rows.filter(row => row.kind === "choice").map(row => ({ archiveId: row.archiveId, visitId: row.visitId, reason: row.reason, reasonText: row.reasonText ?? "", recordedAt: row.at })),
      teachingChecks: rows.filter(row => row.kind === "teachingCheck").map(row => ({ visitId: row.visitId, understood: row.understood, reasonText: row.reasonText ?? "", recordedAt: row.at })),
      offer: this.offer(),
      purchaseIntents: rows.filter(row => row.kind === "purchaseIntent").map(row => ({ visitId: row.visitId, offerId: row.offerId, recordedAt: row.at, status: "intent" })),
      purchases: rows.filter(row => row.kind === "payment").map(payment => ({ paymentId: payment.paymentId, archiveId: payment.archiveId, amountMicros: payment.amountMicros, currency: payment.currency, paidAt: payment.paidAt, source: "operator-confirmed", serviceOpened: rows.some(row => row.kind === "grant" && row.paymentId === payment.paymentId) })),
      contacts: rows.filter(row => ["invitation", "reminder"].includes(row.kind)).map(row => ({ kind: row.kind, occurredAt: row.occurredAt, channel: row.channel, source: "operator-confirmed" })),
      usage: { runtimeSeconds: Math.floor(runtimes.reduce((sum, row) => sum + row.runtime_millis, 0) / 1000), sharedPartySeconds: Math.floor(runtimes.reduce((sum, row) => sum + row.shared_millis, 0) / 1000), confirmedAdventureCount: rows.filter(row => row.kind === "observedAdventure").length, paidAdventureCount: rows.filter(row => row.kind === "observedAdventure" && row.grantId).length, modelCalls: 0, coverageGaps: runtimes.reduce((sum, row) => sum + row.coverage_gaps, 0), openRuntimeSessions: runtimes.filter(row => row.finished_at === null).length, coverage: "rules-only runtime; server-observed own strike with owner in party; no human-attention inference" },
      costs: { currency: this.#metadata.currency, revenueMicros: revenue.toString(), ...costs, complete, contributionMicros: complete ? (revenue - COST_KINDS.reduce((sum, category) => sum + BigInt(costs[`${category}Micros`]), 0n)).toString() : null,
        estimates: rows.filter(row => row.kind === "cost" && row.basis === "estimate").map(row => ({ category: row.category, amountMicros: row.amountMicros, reference: row.reference })), supportSeconds: rows.filter(row => row.kind === "support").reduce((sum, row) => sum + row.seconds, 0) },
    });
  }
  participantRows() {
    return this.#db.prepare("SELECT owner_ref FROM participants ORDER BY participant_ref").all().map(row => this.ownerView(row.owner_ref));
  }
  close() { this.#db?.close(); this.#db = null; }
}

export function participantCsv(rows) {
  const fields = ["participantRef", "environment", "visits", "contacts", "choices", "teachingChecks", "purchaseIntents", "purchases", "usage", "costs"];
  const cell = value => {
    let text = typeof value === "string" ? value : JSON.stringify(value);
    if (/^\s*[=+\-@\t\r]/.test(text)) text = `'${text}`;
    return `"${text.replaceAll('"', '""')}"`;
  };
  return fields.join(",") + "\r\n" + rows.map(row => fields.map(field => cell(row[field] ?? null)).join(",")).join("\r\n") + "\r\n";
}
