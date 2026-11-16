import type { SocialReplyOwner } from "./social-incoming-replies";

/** Transport custody only. Mapping, eight-slot admission and receipt completion
 * are implemented by the same Rust SocialModel used by Windows. */
export type SocialBudgetRuntime = { processMir2SocialBudget?: (input: string) => string };
type NativeOwner = Omit<SocialReplyOwner, "socket"> & { generation: number };
type Pending = Readonly<{ token: number; kind: string; entered: boolean }>;
export type SocialBudgetState = Readonly<{
  version: 1; ok: boolean; handled: boolean; token: number | null; kind: string | null;
  pendingCount: number; pending: readonly Pending[]; released: readonly number[]; error?: string;
}>;
export type SocialBudgetTicket = Readonly<{ token: number; kind: string }>;
type Custody = { binding: Binding; owner: NativeOwner; token: number; entered: boolean };
type Binding = {
  source: SocialReplyOwner; owner: NativeOwner; bridge: ((input: string) => string) | null;
  queued: string[]; queuedBytes: number; failed: boolean; state: SocialBudgetState | null;
};
const socialCommands = new Set(["switchGroup", "addMember", "delMember", "groupInvite", "requestGuildInfo",
  "editGuildMember", "editGuildNotice", "guildInvite", "guildStorageGoldChange", "guildStorageItemChange",
  "tradeGold", "depositTradeItem", "retrieveTradeItem", "tradeCancel"]);
const kinds = new Set(["groupSwitch", "groupAdd", "groupRemove", "groupInviteAccept", "guildInfo", "guildRosterRead",
  "guildMember", "guildRankRename", "guildRankPermission", "guildNotice", "guildInviteAccept", "guildStorageGold",
  "guildStorageItem", "tradeRequest", "tradeReply", "tradeGold", "tradeDeposit", "tradeRetrieve", "tradeConfirm", "tradeCancel"]);
const socialPackets = new Set(["UserInformation", "SwitchGroup", "AddMember", "DeleteMember", "DeleteGroup",
  "GroupInvite", "GroupMemberInfo", "GroupMembersMap", "GroupInviteResult", "GuildInvite", "GuildInviteResult", "GuildStatus", "GuildNoticeChange",
  "GuildNoticeResult", "GuildMemberChange", "GuildRequestResult", "GuildStorageGoldChange", "GuildStorageList",
  "GuildStorageItemChange", "TradeRequest", "TradeAccept", "TradeReply", "TradeItem", "TradeGold", "TradeConfirm",
  "TradeCancel", "DepositTradeItem", "RetrieveTradeItem"]);
const safe = (n: unknown): n is number => typeof n === "number" && Number.isSafeInteger(n) && n >= 0;
function validOwner(o: SocialReplyOwner | null): o is SocialReplyOwner {
  return !!o && !!o.socket && typeof o.socket === "object" && safe(o.connectionGeneration) && o.connectionGeneration > 0
    && safe(o.sessionGeneration) && o.sessionGeneration > 0 && safe(o.playerObjectId) && o.playerObjectId > 0
    && o.playerObjectId <= 0xffff_ffff && safe(o.sceneRevision) && typeof o.mapFileName === "string"
    && !!o.mapFileName && new TextEncoder().encode(o.mapFileName).length <= 512 && !/[\x00-\x1f\x7f]/.test(o.mapFileName);
}
function physical(a: SocialReplyOwner, b: SocialReplyOwner): boolean {
  return a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId;
}
function sameOwner(a: NativeOwner, b: NativeOwner): boolean {
  return a.generation === b.generation && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId
    && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
}
function nextGeneration(): number | null {
  const host = globalThis as typeof globalThis & { __mir2SocialBudgetGeneration?: number };
  const last = host.__mir2SocialBudgetGeneration ?? 0;
  if (!safe(last) || last >= Number.MAX_SAFE_INTEGER) return null;
  return host.__mir2SocialBudgetGeneration = last + 1;
}
function decode(raw: string): SocialBudgetState | null {
  const r = JSON.parse(raw) as SocialBudgetState;
  if (!r || r.version !== 1 || typeof r.ok !== "boolean" || typeof r.handled !== "boolean"
    || !(r.token === null || safe(r.token) && r.token > 0) || !(r.kind === null || kinds.has(r.kind))
    || !safe(r.pendingCount) || r.pendingCount > 8 || !Array.isArray(r.pending) || r.pending.length !== r.pendingCount
    || !r.pending.every(p => p && safe(p.token) && p.token > 0 && kinds.has(p.kind) && typeof p.entered === "boolean")
    || new Set(r.pending.map(p => p.token)).size !== r.pendingCount || !Array.isArray(r.released)
    || !r.released.every(t => safe(t) && t > 0) || !r.ok && typeof r.error !== "string") return null;
  return r;
}

export class SocialOperationBudgetHost {
  private binding: Binding | null = null;
  private retired: SocialReplyOwner | null = null;
  private newest: SocialReplyOwner | null = null;
  private tickets = new WeakMap<SocialBudgetTicket, Custody>();
  constructor(private runtime: () => SocialBudgetRuntime | null) {}

  // Native's actual invitation/lock UI uses transient intents. Do not turn
  // TradeRequest/TradeReply/TradeConfirm into long-lived pending operations.
  static covers(command: Readonly<Record<string, unknown>>): boolean {
    return socialCommands.has(String(command.type))
      && !(["groupInvite", "guildInvite"].includes(String(command.type)) && command.acceptInvite === false)
      && !(command.type === "guildStorageItemChange" && command.changeType === 3);
  }
  private call(b: Binding, input: string): SocialBudgetState | null {
    if (!b.bridge || b.failed) return null;
    try {
      const result = decode(b.bridge(input));
      if (!result) b.failed = true;
      else b.state = result;
      return result;
    } catch { b.failed = true; return null; }
  }
  private deliver(b: Binding, operation: string, fields: Record<string, unknown> = {}): SocialBudgetState | null {
    if (b.failed) return null;
    let input: string;
    try { input = JSON.stringify({ version: 1, operation, owner: b.owner, ...fields }); }
    catch { b.failed = true; return null; }
    if (new TextEncoder().encode(input).length > 8 * 1024 * 1024) { b.failed = true; return null; }
    if (b.bridge) return this.call(b, input);
    // Only authenticated, relevant receives can queue before WASM is ready.
    // Overflow fails closed for this session instead of discarding a receipt.
    b.queuedBytes += new TextEncoder().encode(input).length;
    if (b.queued.length >= 128 || b.queuedBytes > 8 * 1024 * 1024) b.failed = true;
    else b.queued.push(input);
    return null;
  }
  private sync(source: SocialReplyOwner | null): Binding | null {
    if (!validOwner(source) || this.retired && physical(this.retired, source)) return null;
    // A late old socket must never be laundered into a higher Native generation.
    // The Page's authenticated connection/session counters are monotonic.
    const newest = this.newest;
    if (newest && (source.connectionGeneration < newest.connectionGeneration
      || source.connectionGeneration === newest.connectionGeneration && (source.socket !== newest.socket
        || source.sessionGeneration < newest.sessionGeneration
        || source.sessionGeneration === newest.sessionGeneration && source.playerObjectId !== newest.playerObjectId))) return null;
    let b = this.binding;
    if (!b || !physical(b.source, source)) {
      this.retire();
      const generation = nextGeneration();
      if (generation === null) return null;
      const { socket: _socket, ...identity } = source;
      b = { source: Object.freeze({ ...source }), owner: { ...identity, generation }, bridge: null,
        queued: [], queuedBytes: 0, failed: false, state: null };
      this.binding = b;
      this.newest = b.source;
      this.deliver(b, "reset");
    } else if (source.sceneRevision !== b.owner.sceneRevision || source.mapFileName !== b.owner.mapFileName) {
      if (source.sceneRevision < b.owner.sceneRevision
        || source.sceneRevision === b.owner.sceneRevision && source.mapFileName !== b.owner.mapFileName) return null;
      b.source = Object.freeze({ ...source });
      b.owner = { ...b.owner, sceneRevision: source.sceneRevision, mapFileName: source.mapFileName };
      const result = this.deliver(b, "reset");
      if (b.bridge && !result?.ok) b.failed = true;
    }
    if (!b.bridge && !b.failed) {
      const runtime = this.runtime();
      if (typeof runtime?.processMir2SocialBudget === "function") {
        // Keep this module's custody until physical retirement, even when the
        // renderer is replaced. Replaying projections into a new model loses ACK history.
        b.bridge = runtime.processMir2SocialBudget;
        const queued = b.queued; b.queued = []; b.queuedBytes = 0;
        for (const input of queued) if (!this.call(b, input)?.ok) { b.failed = true; break; }
      }
    }
    return b;
  }
  receive(source: SocialReplyOwner | null, packet: string, payload: unknown): void {
    if (!socialPackets.has(packet)) return;
    const b = this.sync(source);
    if (b) this.deliver(b, "packet", { packet, payload });
  }
  snapshot(source: SocialReplyOwner | null, snapshot: unknown, inventoryJson: unknown): void {
    const b = this.sync(source);
    if (b) this.deliver(b, "snapshot", { snapshot, inventoryJson });
  }
  reserve(command: Readonly<Record<string, unknown>>, source: SocialReplyOwner | null, nowMs: number):
    { ok: true; ticket: SocialBudgetTicket | null } | { ok: false } {
    if (!SocialOperationBudgetHost.covers(command)) return { ok: true, ticket: null };
    const b = this.sync(source);
    if (!b || b.failed || !b.bridge || !safe(nowMs) || nowMs > Number.MAX_SAFE_INTEGER - 5000) return { ok: false };
    if (!this.deliver(b, "expire", { nowMs })?.ok) return { ok: false };
    const result = this.deliver(b, "reserve", { command,
      ...(command.type === "editGuildMember" && command.changeType === 3 ? { deadlineMs: nowMs + 5000 } : {}) });
    if (!result?.ok || !result.handled || !result.token || !result.kind) return { ok: false };
    const ticket = Object.freeze({ token: result.token, kind: result.kind });
    this.tickets.set(ticket, { binding: b, owner: b.owner, token: result.token, entered: false });
    return { ok: true, ticket };
  }
  validate(ticket: SocialBudgetTicket, source: SocialReplyOwner | null, command: Readonly<Record<string, unknown>>): boolean {
    const custody = this.tickets.get(ticket), b = this.sync(source);
    return !!custody && !!b && b === custody.binding && !custody.entered && sameOwner(custody.owner, b.owner)
      && this.deliver(b, "validate", { token: custody.token, command })?.ok === true;
  }
  enter(ticket: SocialBudgetTicket, source: SocialReplyOwner | null, command: Readonly<Record<string, unknown>>): boolean {
    const custody = this.tickets.get(ticket), b = this.sync(source);
    if (!custody || !b || b !== custody.binding || custody.entered || !sameOwner(custody.owner, b.owner)
      || !this.deliver(b, "enter", { token: custody.token, command })?.ok) return false;
    custody.entered = true;
    return true;
  }
  cancelUnsent(ticket: SocialBudgetTicket): void {
    const custody = this.tickets.get(ticket);
    if (!custody || custody.entered) return;
    if (this.binding === custody.binding) this.deliver(custody.binding, "cancelUnsent", { token: custody.token });
    this.tickets.delete(ticket);
  }
  retire(): void {
    const b = this.binding;
    if (!b) return;
    this.retired = b.source;
    this.deliver(b, "retire");
    b.queued = []; b.queuedBytes = 0;
    this.binding = null;
  }
  inspect(source: SocialReplyOwner | null): SocialBudgetState | null {
    const b = this.sync(source);
    return b && this.deliver(b, "inspect");
  }
}
