import { parseGatewayJson } from "../lossless-json.mjs";

export const PURCHASE_CAPABILITY = "durableNpcPurchaseOwnerV1";
const MAX_U64 = 18446744073709551615n;
const REJECTIONS = new Set(["invalidRequest", "playerDead", "serviceUnavailable", "unsupportedService", "unknownGood", "invalidQuantity", "insufficientCurrency", "clockUnavailable", "invalidDelivery"]);
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const invalid = () => { throw new Error("Invalid durable NPC purchase message."); };
export function exact(value, keys) {
  if (!record(value) || Object.keys(value).length !== keys.length || keys.some(key => !Object.hasOwn(value, key))) invalid();
  return value;
}
export function u64(value, positive = false, terminal = false) {
  if (typeof value !== "string" || !/^(?:0|[1-9][0-9]{0,19})$/.test(value)) invalid();
  const number = BigInt(value);
  if (number > MAX_U64 || positive && number === 0n || terminal && number === MAX_U64) invalid();
  return value;
}
export function opaque(value) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value) || /^0+$/.test(value)) invalid();
  return value;
}
const integer = (value, max, min = 0) => { if (!Number.isSafeInteger(value) || value < min || value > max) invalid(); return value; };
const canonical = value => Array.isArray(value) ? `[${value.map(canonical).join(",")}]` : record(value) ? `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${canonical(value[k])}`).join(",")}}` : JSON.stringify(value);
export const same = (left, right) => canonical(left) === canonical(right);

export function purchaseRequest(value) {
  exact(value, ["itemIndex", "count", "panelType"]);
  u64(value.itemIndex); integer(value.count, 65535, 1); integer(value.panelType, 255);
  return structuredClone(value);
}
export function purchaseIntent(value) {
  exact(value, ["request", "currency", "source", "serviceCatalogProof"]);
  purchaseRequest(value.request); opaque(value.serviceCatalogProof);
  if (!["gold", "pearls"].includes(value.currency) || !["trade", "buyBack", "used"].includes(value.source)) invalid();
  return structuredClone(value);
}
export function purchaseOperation(value) {
  exact(value, ["actor", "requestScope", "sequence", "intent"]);
  opaque(value.actor); opaque(value.requestScope); u64(value.sequence, true); purchaseIntent(value.intent);
  return structuredClone(value);
}
function producer(value) {
  exact(value, ["actor", "producerScope", "serverRevision"]);
  opaque(value.actor); opaque(value.producerScope); u64(value.serverRevision, false, true);
}
export function purchaseReceipt(value, operation) {
  exact(value, ["producerScope", "entry"]); opaque(value.producerScope);
  exact(value.entry, ["operation", "serverRevision", "outcome"]);
  purchaseOperation(value.entry.operation); u64(value.entry.serverRevision, true, true);
  if (operation && !same(operation, value.entry.operation)) invalid();
  const outcome = value.entry.outcome;
  if (record(outcome) && Object.hasOwn(outcome, "committed")) {
    exact(outcome, ["committed"]);
    const paid = exact(outcome.committed, ["request", "currency", "source", "charged", "admittedCount", "incomingUniqueId"]);
    purchaseRequest(paid.request); integer(paid.charged, 0xffff_ffff); integer(paid.admittedCount, 65535, 1); u64(paid.incomingUniqueId, true);
    if (!same(paid.request, value.entry.operation.intent.request) || paid.currency !== value.entry.operation.intent.currency || paid.source !== value.entry.operation.intent.source || paid.admittedCount > paid.request.count) invalid();
  } else {
    exact(outcome, ["rejected"]); const rejected = exact(outcome.rejected, ["request", "reason"]);
    purchaseRequest(rejected.request);
    if (!same(rejected.request, value.entry.operation.intent.request) || !REJECTIONS.has(rejected.reason)) invalid();
  }
  return structuredClone(value);
}

export function purchaseAction(value) {
  if (!record(value)) invalid();
  if (value.kind === "begin") exact(value, ["kind"]);
  else if (value.kind === "quote") { exact(value, ["kind", "request"]); purchaseRequest(value.request); }
  else if (["purchase", "query"].includes(value.kind)) { exact(value, ["kind", "operation"]); purchaseOperation(value.operation); }
  else invalid();
  return structuredClone(value);
}

// JSON.parse discards duplicate keys. Owner receipts must not acquire meaning
// from an overwritten action, request identity, outcome, or authority field.
function rejectDuplicateKeys(raw) {
  let at = 0;
  const ws = () => { while (/\s/.test(raw[at] ?? "") && at < raw.length) at++; };
  const string = () => {
    const start = at++;
    while (at < raw.length) { const c = raw[at++]; if (c === "\\") at++; else if (c === '"') return JSON.parse(raw.slice(start, at)); }
    invalid();
  };
  const value = depth => {
    if (depth > 64) invalid(); ws();
    if (raw[at] === "{") {
      at++; ws(); const keys = new Set(); if (raw[at] === "}") { at++; return; }
      while (at < raw.length) { if (raw[at] !== '"') invalid(); const key = string(); if (keys.has(key)) invalid(); keys.add(key); ws(); if (raw[at++] !== ":") invalid(); value(depth + 1); ws(); const c = raw[at++]; if (c === "}") return; if (c !== ",") invalid(); ws(); }
      invalid();
    } else if (raw[at] === "[") {
      at++; ws(); if (raw[at] === "]") { at++; return; }
      while (at < raw.length) { value(depth + 1); ws(); const c = raw[at++]; if (c === "]") return; if (c !== ",") invalid(); } invalid();
    } else if (raw[at] === '"') string();
    else { const match = /^(?:null|true|false|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(raw.slice(at)); if (!match) invalid(); at += match[0].length; }
  };
  value(0); ws(); if (at !== raw.length) invalid();
}

/** Shape and correlation only. Physical authenticated transport and applied
 * complete snapshot are checked separately by GatewayClient. */
export function parsePurchaseFrame(raw, request) {
  if (typeof raw !== "string" || Buffer.byteLength(raw) > 2 * 1024 * 1024) invalid();
  rejectDuplicateKeys(raw);
  const frame = exact(parseGatewayJson(raw), ["type", "protocolVersion", "requestId", "reply", "snapshot", "authority"]);
  if (frame.type !== "npcPurchaseOwner" || frame.protocolVersion !== 1) invalid();
  u64(frame.requestId, true);
  if (frame.requestId !== request.requestId) invalid();
  const action = purchaseAction(request.action), reply = frame.reply;
  if (!record(reply)) invalid();
  if (reply.kind === "producer") {
    exact(reply, ["kind", "producer"]); producer(reply.producer); if (action.kind !== "begin") invalid();
  } else if (reply.kind === "quote") {
    exact(reply, ["kind", "intent"]); purchaseIntent(reply.intent);
    if (action.kind !== "quote" || !same(reply.intent.request, action.request)) invalid();
  } else if (reply.kind === "purchase") {
    exact(reply, ["kind", "receipt", "replayed"]);
    if (action.kind !== "purchase" || typeof reply.replayed !== "boolean") invalid(); purchaseReceipt(reply.receipt, action.operation);
  } else if (reply.kind === "recovery") {
    exact(reply, ["kind", "receipt"]); if (action.kind !== "query") invalid();
    if (reply.receipt !== null) purchaseReceipt(reply.receipt, action.operation);
  } else if (reply.kind === "failure") {
    exact(reply, ["kind", "state", "receipt"]);
    if (!["beforeExecution", "unknown", "postCommit"].includes(reply.state)) invalid();
    if (reply.receipt !== null) { if (!action.operation) invalid(); purchaseReceipt(reply.receipt, action.operation); }
  } else invalid();
  if ((frame.snapshot === null) !== (frame.authority === null) || frame.snapshot !== null && !record(frame.snapshot)) invalid();
  if (frame.authority !== null) {
    producer(frame.authority);
    if (reply.kind === "producer" && !same(reply.producer, frame.authority)) invalid();
    if (reply.receipt && (reply.receipt.entry.operation.actor !== frame.authority.actor || reply.receipt.producerScope !== frame.authority.producerScope || BigInt(reply.receipt.entry.serverRevision) > BigInt(frame.authority.serverRevision))) invalid();
  }
  return frame;
}
