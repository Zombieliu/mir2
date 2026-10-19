import { npcPurchaseDecimal, parseNpcPurchaseJson } from "./npc-purchase-client";

/** Private identity custody, never a view, URL parameter or persisted value. */
export type HuntingRepairWalletIdentity = Readonly<{
  token: string; sessionId: string; accountId: string; expiresAtMs: number;
}>;
export type HuntingRepairWalletBalance = Readonly<{
  characterIndex: number; initialized: boolean; enabled: boolean;
  boundGold: string; unboundGold: string; walletRevision: string;
  permitStockAtoms: string; policyVersion: string; settledThroughDay: number;
}>;
type Options<Owner extends object> = Readonly<{
  owner: Owner; actorScope: object; identity: HuntingRepairWalletIdentity; characterIndex: number;
  isCurrent(owner: Owner, actorScope: object, identity: HuntingRepairWalletIdentity): boolean;
  onChange?(): void;
  fetch?: (input: string, init: RequestInit) => Promise<Response>;
  now?(): number;
}>;
const fields = ["characterIndex", "initialized", "enabled", "boundGold", "unboundGold", "walletRevision",
  "permitStockAtoms", "policyVersion", "settledThroughDay"];
const safe = (value: unknown, minimum = 0): value is number => typeof value === "number"
  && Number.isSafeInteger(value) && value >= minimum;
const validToken = (value: unknown): value is string => typeof value === "string"
  && value.length > 0 && value.length <= 4089 && /^[A-Za-z0-9._~-]+$/.test(value);

/** Read-only observation of the new ledger. A balance never authorizes repair. */
export function parseHuntingRepairWallet(raw: string, characterIndex: number): HuntingRepairWalletBalance {
  if (typeof raw !== "string" || new TextEncoder().encode(raw).length > 16_384) throw Error("Invalid wallet size");
  const value = parseNpcPurchaseJson(raw);
  if (!value || typeof value !== "object" || Array.isArray(value)) throw Error("Invalid wallet");
  const row = value as Record<string, unknown>;
  if (Object.keys(row).length !== fields.length || fields.some(name => !Object.hasOwn(row, name))
    || !safe(row.characterIndex) || row.characterIndex > 0x7fffffff || row.characterIndex !== characterIndex
    || typeof row.initialized !== "boolean" || typeof row.enabled !== "boolean"
    || ![row.boundGold, row.unboundGold, row.walletRevision, row.permitStockAtoms].every(npcPurchaseDecimal)
    || typeof row.policyVersion !== "string" || row.policyVersion.length === 0
    || new TextEncoder().encode(row.policyVersion).length > 256 || /[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(row.policyVersion)
    || !Number.isSafeInteger(row.settledThroughDay)) throw Error("Invalid wallet");
  if (!row.initialized && [row.boundGold, row.unboundGold, row.walletRevision].some(amount => amount !== "0"))
    throw Error("Uninitialized wallet contains assets");
  return Object.freeze({ ...row }) as HuntingRepairWalletBalance;
}

export class HuntingRepairWalletClient<Owner extends object> {
  private readonly options: Options<Owner>;
  private readonly fetcher: (input: string, init: RequestInit) => Promise<Response>;
  private value: HuntingRepairWalletBalance | null = null;
  private control: object | null = null;
  private abort: AbortController | null = null;
  private retired = false;
  private notifying = false;
  private highestRevision: bigint | null = null;
  constructor(options: Options<Owner>) {
    const identity = options.identity;
    if (!options.owner || typeof options.owner !== "object" || !options.actorScope || typeof options.actorScope !== "object"
      || !Object.isFrozen(options.actorScope) || !identity || !Object.isFrozen(identity)
      || !validToken(identity.token) || !identity.sessionId || !identity.accountId || !safe(identity.expiresAtMs)
      || !safe(options.characterIndex) || options.characterIndex > 0x7fffffff || typeof options.isCurrent !== "function")
      throw Error("Invalid wallet custody");
    this.options = Object.freeze({ ...options });
    this.fetcher = options.fetch ?? ((input, init) => fetch(input, init));
  }
  private current(): boolean {
    if (this.retired) return false;
    try {
      const now = (this.options.now ?? Date.now)();
      const active = safe(now) && now < this.options.identity.expiresAtMs
        && this.options.isCurrent(this.options.owner, this.options.actorScope, this.options.identity) === true;
      return active && !this.retired;
    } catch { return false; }
  }
  private notify(): void {
    if (this.notifying) return;
    this.notifying = true;
    try { this.options.onChange?.(); } catch { /* Display callbacks have no asset authority. */ }
    finally { this.notifying = false; }
  }
  balance(): HuntingRepairWalletBalance | null { return this.current() ? this.value : null; }
  /** Invalidate before a possibly submitted mutation; never display its old balance as current. */
  invalidate(): void {
    this.control = null;
    this.abort?.abort();
    this.abort = null;
    this.value = null;
    this.notify();
  }
  async refresh(): Promise<boolean> {
    if (this.notifying || !this.current()) return false;
    this.invalidate();
    if (!this.current()) return false;
    const control = Object.freeze({}), abort = new AbortController();
    this.control = control;
    this.abort = abort;
    const current = () => this.control === control && !abort.signal.aborted && this.current()
      && this.control === control && !abort.signal.aborted && !this.retired;
    try {
      const response = await this.fetcher(`/api/economy/hunting-wallet?characterIndex=${this.options.characterIndex}`, {
        method: "GET", headers: { accept: "application/json", authorization: `Bearer ${this.options.identity.token}` },
        cache: "no-store", credentials: "same-origin", redirect: "error", signal: abort.signal,
      });
      if (!current() || response.status !== 200) return false;
      const raw = await response.text();
      if (!current()) return false;
      const balance = parseHuntingRepairWallet(raw, this.options.characterIndex);
      const revision = BigInt(balance.walletRevision);
      if (this.highestRevision !== null && revision < this.highestRevision || !current()) return false;
      this.value = balance;
      this.highestRevision = revision;
      this.notify();
      return current() && this.value === balance;
    } catch { return false; }
    finally {
      if (this.control === control) { this.control = null; this.abort = null; }
    }
  }
  retire(): void { this.retired = true; this.invalidate(); }
}
