/** A document owns one non-resettable shared economic host. All actual peer
 * checks and full application still belong to the caller; this facade only
 * binds private Core tokens to exact physical sockets and retained producers. */
export type NpcPurchaseReceiptModule = {
  npc_purchase_receipt_abi_version?: () => number;
  NpcPurchaseReceiptBridge?: new () => { transact(input: string): string };
};
export type NpcPurchaseReceiptDecision = Readonly<Record<string, unknown>> & { ok: boolean };
export type NpcPurchaseReceiptRuntime = {
  attachProducer(source: object): boolean;
  withdrawProducer(source: object): boolean;
  transportFor(source: object, socket: object, current: object | null, open: boolean): string | null;
  transact(source: object, socket: object, input: Readonly<Record<string, unknown>>): NpcPurchaseReceiptDecision;
};
const unavailable = (): NpcPurchaseReceiptDecision => Object.freeze({
  ok: false, error: "Shared NPC purchase recovery is unavailable; reload the client",
});
function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function decision(raw: string): NpcPurchaseReceiptDecision {
  if (typeof raw !== "string" || raw.length > 16 * 1024 * 1024) throw Error("Invalid receipt ABI output");
  const value: unknown = JSON.parse(raw);
  if (!record(value) || typeof value.ok !== "boolean"
    || (!value.ok && (typeof value.error !== "string" || Object.keys(value).sort().join() !== "error,ok"))) {
    throw Error("Invalid receipt ABI response");
  }
  return Object.freeze(value) as NpcPurchaseReceiptDecision;
}
// Snapshot only own plain data. Accessors/proxies may cause reentry during
// reflection, so callers also fence the host generation after serialization.
function plainSnapshot(value: unknown, depth = 0): unknown {
  if (depth > 64) throw Error("Deep economic facade input");
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isSafeInteger(value)) return value;
  if (typeof value !== "object") throw Error("Non-JSON economic facade input");
  const array = Array.isArray(value), prototype = Object.getPrototypeOf(value);
  if (prototype !== null && prototype !== (array ? Array.prototype : Object.prototype)) {
    throw Error("Non-plain economic facade input");
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const keys = Reflect.ownKeys(descriptors);
  const output: Record<string, unknown> | unknown[] = array ? [] : Object.create(null);
  if (array) Object.setPrototypeOf(output, null);
  let count = 0;
  for (const key of keys) {
    if (typeof key !== "string" || ["toJSON", "__proto__", "constructor", "prototype"].includes(key)) {
      throw Error("Invalid economic facade field");
    }
    const descriptor = descriptors[key];
    if (!("value" in descriptor)) throw Error("Accessor economic facade input");
    if (array && key === "length") continue;
    if (!descriptor.enumerable || array && (!/^(?:0|[1-9][0-9]*)$/.test(key) || Number(key) !== count++)) {
      throw Error("Noncanonical economic facade fields");
    }
    (output as Record<string, unknown>)[key] = plainSnapshot(descriptor.value, depth + 1);
  }
  if (array && count !== descriptors.length.value) throw Error("Sparse economic facade input");
  return Object.freeze(output);
}
function token(value: unknown): value is string {
  return typeof value === "string" && /^[0-9a-f]{64}$/.test(value) && /[1-9a-f]/.test(value);
}
export function persistentNpcPurchaseReceiptHost(
  module: NpcPurchaseReceiptModule, documentOwner: object, version: string,
): NpcPurchaseReceiptRuntime {
  const key = Symbol.for("mir2.clientCore.npcPurchaseReceipt.v1");
  const contract = "npcPurchaseActorCustody.v1";
  const descriptor = Object.getOwnPropertyDescriptor(documentOwner, key);
  if (descriptor) {
    if (!("value" in descriptor) || !record(descriptor.value)
      || Object.keys(descriptor.value).sort().join() !== "contract,poison,runtime,version"
      || typeof descriptor.value.poison !== "function" || !record(descriptor.value.runtime)) {
      throw Error("Invalid persistent NPC economic host");
    }
    const holder = descriptor.value as {
      contract: string; version: string; poison(): void; runtime: NpcPurchaseReceiptRuntime;
    };
    if (holder.contract !== contract) {
      holder.poison();
      throw Error("Incompatible NPC economic facade; reload the client");
    }
    if (holder.version !== version) holder.poison();
    return holder.runtime;
  }
  let bridge: { transact(input: string): string } | null = null;
  let poisoned = false, active: object | null = null, currentSocket: object | null = null;
  let generation = 0n;
  const known = new WeakSet<object>(), retired = new WeakSet<object>();
  const transports = new WeakMap<object, { token: string; source: object }>(), retiredSockets = new WeakSet<object>();
  const call = (input: Record<string, unknown>, guard?: () => boolean): NpcPurchaseReceiptDecision => {
    if (poisoned || !bridge) return unavailable();
    try {
      const body = JSON.stringify(plainSnapshot(input));
      if (body.length > 16 * 1024 * 1024) throw Error("Oversized receipt ABI input");
      if (guard && !guard()) return unavailable();
      return decision(bridge.transact(body));
    } catch { poison(); return unavailable(); }
  };
  const poison = () => {
    if (poisoned) return;
    generation++;
    // Withdraw only availability. Never free/recreate the Core or its Actor map.
    if (bridge && currentSocket) {
      const old = transports.get(currentSocket);
      if (old) {
        try { bridge.transact(JSON.stringify(plainSnapshot({ op: "withdraw", token: old.token, disconnect: true }))); }
        catch { /* The retained host still owns every original operation. */ }
      }
    }
    poisoned = true;
    currentSocket = null;
  };
  try {
    if (module.npc_purchase_receipt_abi_version?.() === 1
      && typeof module.NpcPurchaseReceiptBridge === "function") {
      const candidate = new module.NpcPurchaseReceiptBridge();
      if (decision(candidate.transact('{"op":"status"}')).ok) bridge = { transact: candidate.transact.bind(candidate) };
    }
  } catch { poisoned = true; }
  const runtime: NpcPurchaseReceiptRuntime = Object.freeze({
    attachProducer(source: object) {
      if (poisoned || !bridge || retired.has(source)) return false;
      if (active === source) return true;
      generation++;
      if (active) {
        if (currentSocket) {
          const old = transports.get(currentSocket);
          if (old) call({ op: "withdraw", token: old.token, disconnect: true });
          retiredSockets.add(currentSocket);
          currentSocket = null;
        }
        retired.add(active);
      }
      if (poisoned) return false;
      active = source; known.add(source); return true;
    },
    withdrawProducer(source: object) {
      if (active !== source) return false;
      generation++;
      if (currentSocket) {
        const old = transports.get(currentSocket);
        if (old) call({ op: "withdraw", token: old.token, disconnect: true });
        retiredSockets.add(currentSocket); currentSocket = null;
      }
      active = null; retired.add(source); return !poisoned;
    },
    transportFor(source: object, socket: object, current: object | null, open: boolean) {
      if (poisoned || !bridge || active !== source || socket !== current || !open || retiredSockets.has(socket)) return null;
      if (currentSocket === socket) {
        const prior = transports.get(socket);
        return prior?.source === source ? prior.token : null;
      }
      if (transports.has(socket)) return null;
      generation++;
      if (currentSocket) {
        const old = transports.get(currentSocket);
        if (old) call({ op: "withdraw", token: old.token, disconnect: true });
        retiredSockets.add(currentSocket); currentSocket = null;
      }
      const result = call({ op: "openConnection" });
      if (!result.ok || !token(result.token) || poisoned || active !== source) return null;
      transports.set(socket, { token: result.token, source }); currentSocket = socket;
      return result.token;
    },
    transact(source: object, socket: object, input: Readonly<Record<string, unknown>>) {
      const observedGeneration = generation;
      const stable = plainSnapshot(input);
      if (!record(stable) || typeof stable.op !== "string"
        || Object.prototype.hasOwnProperty.call(stable, "token") || stable.op === "openConnection") {
        throw Error("Invalid economic facade request");
      }
      const operation = stable.op;
      const owned = transports.get(socket);
      if (!known.has(source) || !owned || owned.source !== source) return unavailable();
      const offlineCustody = operation === "unknown" || operation === "cancelUnsent";
      const live = () => observedGeneration === generation && transports.get(socket) === owned
        && owned.source === source && known.has(source)
        && (offlineCustody || operation === "status"
          || active === source && currentSocket === socket && !retiredSockets.has(socket));
      if (!live()) return unavailable();
      return operation === "status" ? call({ op: "status" }, live)
        : call({ ...stable, token: owned.token }, live);
    },
  });
  Object.defineProperty(documentOwner, key, {
    value: Object.freeze({ contract, version, runtime, poison }),
    enumerable: false, configurable: false, writable: false,
  });
  return runtime;
}
