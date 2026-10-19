import { completeNpcPurchaseSnapshot, npcPurchaseDecimal, parseNpcPurchaseJson } from "./npc-purchase-client";

type Data = Readonly<Record<string, unknown>>;
export type NpcRepairMode = "legacy" | "hunting-v1";
export type HuntingRepairSelection = Readonly<{ uniqueId: string; special: boolean }>;
export type HuntingRepairQuote = Readonly<HuntingRepairSelection & {
  quoteId: string; costBoundGold: string; createdAtMs: number; expiresAtMs: number;
}>;
/** Original receipt only. It is never a current wallet or an item projection. */
export type HuntingRepairReceipt = Readonly<{
  quoteId: string; costBoundGold: string; free: boolean; operationKey: string;
  requestDigest: string; businessDay: number; committedAtMs: number;
}>;
export type HuntingRepairRejection = "disabled" | "unauthenticated" | "unavailable" | "invalidRequest"
  | "unsupported" | "expired" | "itemChanged" | "notSubmitted";
export type HuntingRepairReply =
  | Readonly<{ kind: "quoted"; quote: HuntingRepairQuote }>
  | Readonly<{ kind: "pending"; quoteId: string }>
  | Readonly<{ kind: "unknown"; quoteId: string | null }>
  | Readonly<{ kind: "committed"; receipt: HuntingRepairReceipt; replayed: boolean; reloadRequired: boolean }>
  | Readonly<{ kind: "rejected"; quoteId: string | null; code: HuntingRepairRejection }>;
export type HuntingRepairFrame = Readonly<{
  type: "huntingRepairOwner"; protocolVersion: 1; requestId: string;
  reply: HuntingRepairReply; snapshot: Data | null;
}>;
export type HuntingRepairView = Readonly<{
  phase: "idle" | "quoting" | "quoted" | "pending" | "committed" | "rejected" | "retired";
  visible: boolean; quote: HuntingRepairQuote | null; quoteId: string | null;
  receipt: HuntingRepairReceipt | null; reason: string | null; reloadRequired: boolean;
}>;
export type HuntingRepairClientOptions<Owner extends object> = Readonly<{
  /** The stable physical socket/transport object itself, reused for its whole
   * lifetime. Never allocate a fresh owner wrapper for each action or window. */
  owner: Owner;
  /** A frozen, caller-owned actor identity captured with this physical owner. */
  actorScope: object;
  mode: NpcRepairMode;
  isCurrent(owner: Owner, actorScope: object, mode: NpcRepairMode): boolean;
  selectionMatches(owner: Owner, selection: HuntingRepairSelection): boolean;
  send(owner: Owner, raw: string): void;
  applySnapshot(snapshot: Data): boolean;
  onChange?(view: HuntingRepairView): void;
  now?(): number;
  timeoutMs?: number;
}>;

const MAX_FRAME_BYTES = 2 * 1024 * 1024;
const MAX_U64 = 18446744073709551615n;
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const fields = (value: unknown, names: readonly string[]): value is Record<string, unknown> => record(value)
  && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
const safe = (value: unknown, minimum = 0): value is number => typeof value === "number"
  && Number.isSafeInteger(value) && value >= minimum;
const quoteId = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{64}$/.test(value) && /[1-9a-f]/.test(value);
const nonzeroDecimal = (value: unknown): value is string => npcPurchaseDecimal(value) && value !== "0";
const text = (value: unknown): value is string => typeof value === "string" && value.length > 0
  && new TextEncoder().encode(value).length <= 1024 && !/[\u0000-\u001f\u007f-\u009f]/.test(value);
const selectionValid = (value: unknown): value is HuntingRepairSelection => fields(value, ["uniqueId", "special"])
  && npcPurchaseDecimal(value.uniqueId) && typeof value.special === "boolean";
const quoteValid = (value: unknown): value is HuntingRepairQuote => fields(value,
  ["quoteId", "uniqueId", "special", "costBoundGold", "createdAtMs", "expiresAtMs"])
  && quoteId(value.quoteId) && npcPurchaseDecimal(value.uniqueId) && typeof value.special === "boolean"
  && npcPurchaseDecimal(value.costBoundGold) && safe(value.createdAtMs) && safe(value.expiresAtMs)
  && value.createdAtMs <= Number.MAX_SAFE_INTEGER - 30_000 && value.createdAtMs + 30_000 === value.expiresAtMs;
const receiptValid = (value: unknown): value is HuntingRepairReceipt => fields(value,
  ["quoteId", "costBoundGold", "free", "operationKey", "requestDigest", "businessDay", "committedAtMs"])
  && quoteId(value.quoteId) && npcPurchaseDecimal(value.costBoundGold) && typeof value.free === "boolean"
  && value.free === (value.costBoundGold === "0") && text(value.operationKey) && text(value.requestDigest)
  && safe(value.businessDay, Number.MIN_SAFE_INTEGER) && safe(value.committedAtMs);
const rejections: readonly string[] = ["disabled", "unauthenticated", "unavailable", "invalidRequest", "unsupported", "expired", "itemChanged", "notSubmitted"];
/** Only fresh JSON graphs and our own view records reach this boundary. Keep
 * nested authority immutable independently of the shared decoder's freezing. */
function freezeGraph<T>(value: T): T {
  const pending: unknown[] = [value];
  while (pending.length) {
    const current = pending.pop();
    if (current !== null && typeof current === "object") {
      for (const child of Object.values(current)) pending.push(child);
      Object.freeze(current);
    }
  }
  return value;
}
function unicodeValid(value: unknown): boolean {
  const wellFormed = (text: string): boolean => {
    for (let index = 0; index < text.length; index++) {
      const unit = text.charCodeAt(index);
      if (unit >= 0xd800 && unit <= 0xdbff) {
        const next = text.charCodeAt(++index);
        if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      } else if (unit >= 0xdc00 && unit <= 0xdfff) return false;
    }
    return true;
  };
  const pending = [value];
  while (pending.length) {
    const current = pending.pop();
    if (typeof current === "string" && !wellFormed(current)) return false;
    if (current !== null && typeof current === "object") {
      for (const [key, child] of Object.entries(current)) {
        if (!wellFormed(key)) return false;
        pending.push(child);
      }
    }
  }
  return true;
}
function replyValid(value: unknown): value is HuntingRepairReply {
  if (!record(value)) return false;
  switch (value.kind) {
    case "quoted": return fields(value, ["kind", "quote"]) && quoteValid(value.quote);
    case "pending": return fields(value, ["kind", "quoteId"]) && quoteId(value.quoteId);
    case "unknown": return fields(value, ["kind", "quoteId"]) && (value.quoteId === null || quoteId(value.quoteId));
    case "rejected": return fields(value, ["kind", "quoteId", "code"])
      && (value.quoteId === null || quoteId(value.quoteId)) && typeof value.code === "string" && rejections.includes(value.code);
    case "committed": return fields(value, ["kind", "receipt", "replayed", "reloadRequired"])
      && receiptValid(value.receipt) && typeof value.replayed === "boolean" && typeof value.reloadRequired === "boolean";
    default: return false;
  }
}

/** Route the original namespace even when duplicate top-level type fields will
 * make strict parsing fail. Nested strings and escaped quotes are not markers. */
export function isHuntingRepairOwnerFrame(raw: unknown): raw is string {
  if (typeof raw !== "string" || raw.length > 16 * 1024 * 1024) return false;
  let offset = 0;
  const whitespace = () => { while (/[\t\n\r ]/.test(raw[offset] ?? "!")) offset++; };
  const string = (): string => {
    const start = offset++;
    while (offset < raw.length) {
      const char = raw[offset++];
      if (char === "\\") { offset++; continue; }
      if (char === '"') return JSON.parse(raw.slice(start, offset)) as string;
    }
    throw Error("Unclosed repair marker");
  };
  try {
    whitespace(); if (raw[offset++] !== "{") return false;
    let depth = 1;
    while (offset < raw.length && depth > 0) {
      const char = raw[offset];
      if (char === '"') {
        const key = string();
        if (depth === 1 && key === "type") {
          whitespace();
          if (raw[offset] === ":") {
            offset++; whitespace();
            if (raw[offset] === '"' && string() === "huntingRepairOwner") return true;
          }
        }
        continue;
      }
      offset++;
      if (char === "{" || char === "[") depth++;
      else if (char === "}" || char === "]") depth--;
    }
    return false;
  } catch { return false; }
}

/** Raw JSON is decoded before Number conversion. The shared decoder rejects
 * duplicate/escaped duplicate keys and preserves declared u64 snapshot leaves. */
export function parseHuntingRepairFrame(raw: string): HuntingRepairFrame {
  if (typeof raw !== "string" || raw.length === 0 || raw.length > MAX_FRAME_BYTES
    || new TextEncoder().encode(raw).length > MAX_FRAME_BYTES) throw Error("Invalid repair frame size");
  const value = parseNpcPurchaseJson(raw, true);
  if (!fields(value, ["type", "protocolVersion", "requestId", "reply", "snapshot"])
    || value.type !== "huntingRepairOwner" || value.protocolVersion !== 1 || !nonzeroDecimal(value.requestId)
    || !replyValid(value.reply) || !unicodeValid(value)) throw Error("Invalid repair frame");
  if (value.snapshot !== null && (value.reply.kind !== "committed" || value.reply.reloadRequired
    || !completeNpcPurchaseSnapshot(value.snapshot))) throw Error("Repair frame requires a complete current snapshot");
  return freezeGraph(value as HuntingRepairFrame);
}

type Action = Readonly<{ kind: "quote"; uniqueId: string; special: boolean }>
  | Readonly<{ kind: "confirm" | "query"; quoteId: string }>;
type Control = Readonly<{ requestId: string; action: Action; deadline: number }>;
type OwnerSequence = { next: bigint; active: object | null };
const sequences = new WeakMap<object, OwnerSequence>();

/** One adapter belongs to one physical owner and immutable actor scope. UI
 * closure calls resetView; only an actual authority/connection change retires
 * the adapter. A new connection may Query the retained view's original id. */
export class HuntingRepairClient<Owner extends object = object> {
  private readonly options: HuntingRepairClientOptions<Owner>;
  private readonly token = Object.freeze({});
  private readonly sequence: OwnerSequence;
  private control: Control | null = null;
  private selected: HuntingRepairSelection | null = null;
  private readonly confirmed = new Set<string>();
  private pending = false;
  private retired = false;
  private busy = false;
  private notifying = false;
  private state: HuntingRepairView = Object.freeze({ phase: "idle", visible: false, quote: null,
    quoteId: null, receipt: null, reason: null, reloadRequired: false });

  constructor(options: HuntingRepairClientOptions<Owner>) {
    if (!options.owner || typeof options.owner !== "object" || !options.actorScope
      || typeof options.actorScope !== "object" || !Object.isFrozen(options.actorScope)
      || !["legacy", "hunting-v1"].includes(options.mode)
      || [options.isCurrent, options.selectionMatches, options.send, options.applySnapshot].some(fn => typeof fn !== "function")
      || options.timeoutMs !== undefined && (!safe(options.timeoutMs, 1) || options.timeoutMs > 30_000))
      throw Error("Invalid repair owner configuration");
    this.options = Object.freeze({ ...options });
    this.sequence = sequences.get(options.owner) ?? { next: 1n, active: null };
    if (this.sequence.active !== null) throw Error("Physical owner already has a repair adapter");
    this.sequence.active = this.token;
    sequences.set(options.owner, this.sequence);
  }
  view(): HuntingRepairView { return this.state; }
  private changed(patch: Partial<HuntingRepairView>): void {
    this.state = freezeGraph({ ...this.state, ...patch });
    if (this.notifying) return;
    this.notifying = true;
    try { this.options.onChange?.(this.state); } catch { /* Observation does not authorize execution. */ }
    finally { this.notifying = false; }
  }
  private current(): boolean {
    if (this.retired || this.options.mode !== "hunting-v1" || this.sequence.active !== this.token) return false;
    try {
      const accepted = this.options.isCurrent(this.options.owner, this.options.actorScope, this.options.mode) === true;
      return accepted && !this.retired && this.options.mode === "hunting-v1" && this.sequence.active === this.token;
    }
    catch { return false; }
  }
  private selectionCurrent(): boolean {
    if (!this.selected || !this.current()) return false;
    try { return this.options.selectionMatches(this.options.owner, this.selected) === true && this.current(); }
    catch { return false; }
  }
  private clock(): number | null {
    try { const now = (this.options.now ?? Date.now)(); return safe(now) ? now : null; }
    catch { return null; }
  }
  private dispatch(action: Action): boolean {
    const now = this.clock();
    if (!this.current() || this.control || now === null || this.sequence.next > MAX_U64) return false;
    const timeout = this.options.timeoutMs ?? 10_000;
    if (now > Number.MAX_SAFE_INTEGER - timeout) return false;
    const control = Object.freeze({ requestId: String(this.sequence.next++), action, deadline: now + timeout });
    this.control = control;
    if (action.kind === "confirm") {
      this.pending = true;
      this.confirmed.add(action.quoteId);
    }
    this.changed({ phase: action.kind === "quote" ? "quoting" : "pending", reason: null });
    // onChange and current-selection callbacks may synchronously close/rebind.
    if (this.control !== control || !this.current() || action.kind !== "query" && !this.selectionCurrent()) {
      if (this.control === control) this.control = null;
      if (!this.retired) this.changed({ phase: this.pending ? "pending" : "idle", reason: "ownerOrSelectionChanged" });
      return false;
    }
    // The callbacks above can synchronously time out/cancel this control.
    // This final custody check performs no further caller callback.
    if (this.control !== control || this.retired || this.sequence.active !== this.token) return false;
    try {
      this.options.send(this.options.owner, JSON.stringify({ type: "huntingRepairOwner", protocolVersion: 1,
        requestId: control.requestId, action }));
      return true;
    } catch {
      if (this.control === control) this.control = null;
      if (!this.retired) this.changed({ phase: this.pending ? "pending" : "idle", reason: "sendUnknown" });
      return false;
    }
  }
  private action(run: () => boolean): boolean {
    if (this.busy || this.notifying) return false;
    this.busy = true;
    try { return run(); } finally { this.busy = false; }
  }
  quote(selection: HuntingRepairSelection): boolean { return this.action(() => this.quoteOriginal(selection)); }
  private quoteOriginal(selection: HuntingRepairSelection): boolean {
    if (!selectionValid(selection) || !this.current() || this.pending || this.control) return false;
    this.selected = Object.freeze({ uniqueId: selection.uniqueId, special: selection.special });
    if (!this.selectionCurrent()) { this.selected = null; return false; }
    this.changed({ phase: "idle", visible: true, quote: null, quoteId: null, receipt: null, reason: null, reloadRequired: false });
    return this.dispatch(Object.freeze({ kind: "quote", ...this.selected }));
  }
  confirm(): boolean { return this.action(() => this.confirmOriginal()); }
  private confirmOriginal(): boolean {
    const quote = this.state.quote, now = this.clock();
    if (!quote || this.state.phase !== "quoted" || !this.state.visible || this.pending || this.control
      || this.confirmed.has(quote.quoteId) || !this.selectionCurrent() || now === null) return false;
    if (now >= quote.expiresAtMs) { this.changed({ phase: "rejected", reason: "expired" }); return false; }
    return this.dispatch(Object.freeze({ kind: "confirm", quoteId: quote.quoteId }));
  }
  query(originalQuoteId = this.state.quoteId): boolean { return this.action(() => this.queryOriginal(originalQuoteId)); }
  private queryOriginal(originalQuoteId: string | null): boolean {
    if (!quoteId(originalQuoteId) || !this.current() || this.control
      || this.pending && this.state.quoteId !== originalQuoteId) return false;
    if (this.state.quoteId !== originalQuoteId) {
      this.selected = null;
      this.changed({ quote: null, receipt: null, quoteId: originalQuoteId, reloadRequired: false });
    }
    this.pending = true;
    this.changed({ phase: "pending", reason: null });
    return this.dispatch(Object.freeze({ kind: "query", quoteId: originalQuoteId }));
  }
  receive(raw: string): boolean { return this.action(() => this.receiveOriginal(raw)); }
  private receiveOriginal(raw: string): boolean {
    let frame: HuntingRepairFrame;
    try { frame = parseHuntingRepairFrame(raw); } catch { return false; }
    const control = this.control;
    if (!control || frame.requestId !== control.requestId || !this.current() || this.control !== control) return false;
    const action = control.action, reply = frame.reply;
    if (action.kind === "quote") {
      if (reply.kind === "quoted") {
        if (reply.quote.uniqueId !== action.uniqueId || reply.quote.special !== action.special
          || this.confirmed.has(reply.quote.quoteId) || !this.selectionCurrent() || this.control !== control) return false;
        this.control = null;
        this.changed({ phase: "quoted", quote: reply.quote, quoteId: reply.quote.quoteId, reason: null });
        return true;
      }
      if ((reply.kind !== "rejected" && reply.kind !== "unknown") || reply.quoteId !== null) return false;
      this.control = null;
      this.changed({ phase: reply.kind === "rejected" ? "rejected" : "idle", reason: reply.kind === "rejected" ? reply.code : "quoteUnknown" });
      return true;
    }
    const actual = reply.kind === "committed" ? reply.receipt.quoteId : reply.kind === "quoted" ? null : reply.quoteId;
    if (actual !== action.quoteId) return false;
    if (reply.kind === "committed") {
      if (this.state.quote && reply.receipt.costBoundGold !== this.state.quote.costBoundGold
        || this.state.receipt && Object.keys(this.state.receipt).some(key =>
          this.state.receipt![key as keyof HuntingRepairReceipt] !== reply.receipt[key as keyof HuntingRepairReceipt])) return false;
      this.control = null;
      this.changed({ receipt: reply.receipt, quoteId: action.quoteId, phase: "pending", reason: "snapshotRequired" });
      if (reply.reloadRequired) {
        this.changed({ reloadRequired: true });
        this.retireConnection();
        return true;
      }
      if (frame.snapshot === null || !this.current()) return true;
      let applied = false;
      try { applied = this.options.applySnapshot(frame.snapshot) === true; } catch { /* The original remains pending. */ }
      if (!applied || !this.current() || this.state.quoteId !== action.quoteId || this.retired) return true;
      this.pending = false;
      this.changed({ phase: "committed", reason: null });
      return true;
    }
    if (reply.kind === "rejected" && action.kind === "query" && reply.code === "notSubmitted" && this.state.receipt === null) {
      this.control = null;
      this.pending = false;
      this.changed({ phase: "rejected", reason: "notSubmitted" });
      return true;
    }
    if (reply.kind !== "rejected" && reply.kind !== "unknown" && reply.kind !== "pending") return false;
    this.control = null;
    this.changed({ phase: "pending", reason: reply.kind === "rejected" ? reply.code : reply.kind });
    return true;
  }
  checkTimeout(): void {
    const now = this.clock(), control = this.control;
    if (now === null || !control || now < control.deadline) return;
    this.control = null;
    this.changed({ phase: this.pending ? "pending" : "idle", reason: "timeout" });
  }
  showView(): void { if (!this.retired) this.changed({ visible: true }); }
  resetView(): void {
    if (this.pending) { this.changed({ visible: false }); return; }
    this.control = null;
    this.selected = null;
    this.changed({ phase: this.retired ? "retired" : "idle", visible: false, quote: null, quoteId: null, receipt: null, reason: null });
  }
  retireConnection(): void {
    if (this.retired) return;
    this.retired = true;
    this.control = null;
    if (this.sequence.active === this.token) this.sequence.active = null;
    this.changed({ phase: "retired", visible: false, reason: this.state.reloadRequired ? "reloadRequired" : "connectionRetired" });
  }
}
