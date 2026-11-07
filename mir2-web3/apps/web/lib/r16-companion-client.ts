/** Owner-private transport. Never attach its snapshots or frames to World,
 * command history, console output, persistent UI settings, or browser storage. */
export type R16CompanionSource = Readonly<{
  socket: object; socketGeneration: number; sessionId: string; roleId: string;
  realmId: string; worldEpoch: string; contextEpoch: number;
}>;
export const R16_COMPANION_OPERATIONS = ["list", "view", "instruct", "proposeTeaching", "confirmTeaching", "cancelTeaching",
  "authorizeSupply", "cancelSupply", "authorizeLife", "resumeLife", "cancelLife", "requestLifeSupply", "teach",
  "revokeFeedback", "deleteFeedback", "deleteEpisode", "clearGrowth", "recall", "resume", "exportBegin", "exportChunk", "trialEvent"] as const;
export type R16CompanionOperation = typeof R16_COMPANION_OPERATIONS[number];
export type R16CompanionFrame = Readonly<{ type: "companionControl"; requestId: string; residentId?: string;
  operation: R16CompanionOperation; payload: Readonly<Record<string, unknown>> }>;
declare const renderLeaseBrand: unique symbol;
/** Identity is checked inside the controller; serialized/copied objects never authorize an action. */
export type R16CompanionRenderLease = Readonly<{ [renderLeaseBrand]: true }>;
export type R16CompanionHost = {
  /** Null unless the current authenticated character is in game and its own
   * realmInfo advertises companionControl plus a valid realm/world epoch. */
  captureSource(): R16CompanionSource | null;
  /** Final source check and direct send on the EXISTING authenticated socket.
   * not-sent means send was never entered; exceptions must mean unknown. */
  sendPrivate(source: R16CompanionSource, frame: R16CompanionFrame): "sent" | "not-sent" | "unknown";
  now?(): number; randomId?(): string;
  sha256?(bytes: Uint8Array): Promise<string>;
  makeDownload?(bytes: Uint8Array): string;
  revokeDownload?(url: string): void;
};
export type R16CompanionEnrollment = Readonly<{ residentId: string; archiveId: string; displayName: string; online: boolean }>;
export type R16CompanionFeedback = Readonly<{ feedbackId: string; status: string; summary: string; recordedAt: number | null }>;
export type R16CompanionEpisode = Readonly<{ episodeId: string; kind: string; summary: string; action: string; outcome: string; recordedAt: number | null }>;
export type R16CompanionTask = Readonly<Record<string, string | number | boolean | null>>;
export type R16CompanionOwnerView = Readonly<{
  archive: Readonly<{ archiveId: string; residentId: string; displayName: string; characterName: string; realmId: string; worldEpoch: string; baseSetting: string }>;
  profile: Readonly<{ policyVersion: number; style: string; party: Readonly<{ assistBeforeLoot: boolean; lootOnlyAfterCombat: boolean; askBeforeLeaving: boolean }> }>;
  growthRevision: number; feedback: readonly R16CompanionFeedback[]; episodes: readonly R16CompanionEpisode[]; nextCursor: string | null;
  runtime: Readonly<{ provider: string; model: string; configured: boolean; modelEnabled: boolean; modelCalls: number | null;
    recallRequested: boolean; teachingProposal: R16CompanionTask; teachingChanges: Readonly<Partial<Record<"assistBeforeLoot" | "lootOnlyAfterCombat" | "askBeforeLeaving", boolean>>>;
    modelTask: R16CompanionTask; supplyTask: R16CompanionTask; lifeTask: R16CompanionTask; resources: R16CompanionTask; giftTask: R16CompanionTask }>;
  costs: R16CompanionTask;
}>;
export type R16CompanionAction =
  | { type: "teach" | "recall" | "resume" | "authorizeSupply" | "cancelSupply" | "cancelLife" | "clearGrowth" }
  | { type: "instruct" | "proposeTeaching"; text: string }
  | { type: "confirmTeaching" | "cancelTeaching"; proposalId: string }
  | { type: "revokeFeedback" | "deleteFeedback"; feedbackId: string }
  | { type: "deleteEpisode"; episodeId: string }
  | { type: "authorizeLife"; durationSeconds: 1200 | 1800 }
  | { type: "resumeLife"; lifePermitId: string };
export type R16CompanionSnapshot = Readonly<{
  renderLease: R16CompanionRenderLease; contentEpoch: number; available: boolean; busy: boolean; uncertain: boolean;
  enrollments: readonly R16CompanionEnrollment[]; selected: string | null; view: R16CompanionOwnerView | null;
  notice: string; pendingOperation: R16CompanionOperation | null;
  unknownIntent: Readonly<{ requestId: string; operation: R16CompanionOperation; residentId?: string }> | null;
  download: Readonly<{ url: string; fileName: string }> | null;
}>;
const CHUNK = 16 * 1024, MAX_EXPORT = 2 * 1024 * 1024, EXPORT_TTL = 120_000, REQUEST_TTL = 18_000;
const statuses = ["ok", "denied", "offline", "unknown", "invalid", "conflict", "error"];
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === "object" && !Array.isArray(v) && [Object.prototype, null].includes(Object.getPrototypeOf(v));
const exact = (v: Record<string, unknown>, allowed: readonly string[]) => Object.keys(v).every(k => allowed.includes(k));
const ref = (v: unknown): v is string => typeof v === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(v);
const integer = (v: unknown, min = 0): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= min;
const cursorRef = (v: unknown): v is string => typeof v === "string" && /^[1-9][0-9]{0,15}$/.test(v) && Number.isSafeInteger(Number(v));
const text = (v: unknown, cap = 2000): string => typeof v === "string" ? Array.from(v).slice(0, cap).join("").replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, "") : "";
const number = (v: unknown) => integer(v) ? v : null;
const freeze = <T>(value: T): Readonly<T> => Object.freeze(value);
const emptyTask = (): R16CompanionTask => freeze({});
function task(value: unknown, keys: readonly string[]): R16CompanionTask {
  if (!record(value)) return emptyTask();
  const out: Record<string, string | number | boolean | null> = {};
  for (const key of keys) {
    const v = value[key];
    if (typeof v === "string") out[key] = text(v, 2000);
    else if (typeof v === "boolean" || v === null || integer(v)) out[key] = v as number | boolean | null;
  }
  return freeze(out);
}
const taskKeys = ["status", "step", "reason", "lastError", "permitId", "expiresAtMs", "receiptRevision", "budgetGold", "chargedGold", "acquiredQuantity", "receiptKnown", "returnedToAnchor",
  "phase", "lifePermitId", "maxPurchasedQuantity", "remainingBudgetGold", "reservedGold", "purchasedQuantity", "killsObserved", "pausedReason", "modelStatus", "decisionMode", "activationPending",
  "goalId", "instruction", "reply", "completedKills", "requestedKills"];
export function sameR16CompanionSource(a: R16CompanionSource | null, b: R16CompanionSource | null): boolean {
  return !!a && !!b && a.socket === b.socket && a.socketGeneration === b.socketGeneration && a.sessionId === b.sessionId && a.roleId === b.roleId
    && a.realmId === b.realmId && a.worldEpoch === b.worldEpoch && a.contextEpoch === b.contextEpoch;
}
function validSource(s: R16CompanionSource | null): s is R16CompanionSource {
  const identity = (v: unknown) => typeof v === "string" && v.length <= 256 && v.trim() === v && !!v && !/[\u0000-\u001F\u007F]/.test(v);
  return !!s && !!s.socket && typeof s.socket === "object" && integer(s.socketGeneration, 1) && integer(s.contextEpoch)
    && identity(s.sessionId) && identity(s.roleId) && ref(s.realmId) && ref(s.worldEpoch);
}
function ownerView(raw: unknown, selected: R16CompanionEnrollment, source: R16CompanionSource): R16CompanionOwnerView | null {
  if (!record(raw) || !record(raw.archive) || !record(raw.profile) || !record(raw.profile.party)) return null;
  const a = raw.archive, p = raw.profile, h = p.party as Record<string, unknown>;
  if (a.archiveId !== selected.archiveId || a.residentId !== selected.residentId || a.realmId !== source.realmId || a.worldEpoch !== source.worldEpoch
    || !integer(p.policyVersion, 1) || !integer(raw.growthRevision) || typeof a.characterName !== "string" || !a.characterName || a.characterName.length > 100
    || a.characterName.trim() !== a.characterName || /[\u0000-\u001F\u007F]/.test(a.characterName)
    || ![h.assistBeforeLoot, h.lootOnlyAfterCombat, h.askBeforeLeaving].every(v => typeof v === "boolean")) return null;
  if (!Array.isArray(raw.feedback) || raw.feedback.length > 256 || !Array.isArray(raw.episodes) || raw.episodes.length > 50
    || raw.nextCursor != null && !cursorRef(raw.nextCursor)) return null;
  const feedback: R16CompanionFeedback[] = [], episodes: R16CompanionEpisode[] = [];
  for (const row of raw.feedback) {
    if (!record(row) || !ref(row.feedbackId) || !["active", "revoked", "deleted"].includes(String(row.status))) return null;
    feedback.push(freeze({ feedbackId: row.feedbackId, status: String(row.status), summary: text(row.summary), recordedAt: number(row.recordedAt) }));
  }
  for (const row of raw.episodes) {
    if (!record(row) || !ref(row.episodeId)) return null;
    episodes.push(freeze({ episodeId: row.episodeId, kind: text(row.kind, 100), summary: text(row.summary), action: text(row.action, 100), outcome: text(row.outcome, 100), recordedAt: number(row.recordedAt) }));
  }
  if (new Set(feedback.map(r => r.feedbackId)).size !== feedback.length || new Set(episodes.map(r => r.episodeId)).size !== episodes.length) return null;
  const r = record(raw.runtime) ? raw.runtime : {}, proposal = record(r.teachingProposal) ? r.teachingProposal : {};
  const changes: Partial<Record<"assistBeforeLoot" | "lootOnlyAfterCombat" | "askBeforeLeaving", boolean>> = {};
  if (record(proposal.changes) && exact(proposal.changes, ["assistBeforeLoot", "lootOnlyAfterCombat", "askBeforeLeaving"])
    && Object.values(proposal.changes).every(v => typeof v === "boolean")) {
    for (const key of ["assistBeforeLoot", "lootOnlyAfterCombat", "askBeforeLeaving"] as const) if (typeof proposal.changes[key] === "boolean") changes[key] = proposal.changes[key];
  }
  return freeze({ archive: freeze({ archiveId: selected.archiveId, residentId: selected.residentId, displayName: text(a.displayName, 64), characterName: a.characterName,
    realmId: source.realmId, worldEpoch: source.worldEpoch, baseSetting: text(a.baseSetting) }),
    profile: freeze({ policyVersion: p.policyVersion, style: text(p.style, 100), party: freeze({ assistBeforeLoot: h.assistBeforeLoot as boolean,
      lootOnlyAfterCombat: h.lootOnlyAfterCombat as boolean, askBeforeLeaving: h.askBeforeLeaving as boolean }) }), growthRevision: raw.growthRevision,
    feedback: freeze(feedback), episodes: freeze(episodes), nextCursor: raw.nextCursor as string | null ?? null,
    runtime: freeze({ provider: text(r.provider, 100), model: text(r.model, 100), configured: r.configured === true, modelEnabled: r.modelEnabled === true,
      modelCalls: number(r.modelCalls), recallRequested: r.recallRequested === true,
      teachingProposal: task(proposal, ["proposalId", "status", "sourceVersion", "reply"]), teachingChanges: freeze(changes),
      modelTask: task(r.modelTask, taskKeys), supplyTask: task(r.supplyTask, taskKeys), lifeTask: task(r.lifeTask, taskKeys), giftTask: task(r.giftTask, ["status"]),
      resources: task(r.resources, ["known", "observedAtMs", "inventoryCurrent", "medicineUnresolved", "gold", "hp", "maxHp", "hpPotionCount", "usableHpPotionCount", "freeBagSlots", "currentWeight", "maxWeight"]) }),
    costs: task(record(raw.trial) ? raw.trial.costs : null, ["currency", "modelMicros", "hostingMicros", "paymentMicros", "revenueShareMicros", "contributionMicros", "complete", "supportSeconds"]) });
}
export function r16CompanionModelReady(v: R16CompanionOwnerView): boolean { return v.runtime.provider === "openai" && v.runtime.configured && v.runtime.modelEnabled; }
export function r16CompanionTeachingId(v: R16CompanionOwnerView): string | null {
  const p = v.runtime.teachingProposal;
  return p.status === "proposed" && p.sourceVersion === v.profile.policyVersion && ref(p.proposalId) && Object.keys(v.runtime.teachingChanges).length ? p.proposalId : null;
}
export function r16CompanionInstruction(value: string): string | null {
  // Native trims the editor before sending. Node's instructionText additionally
  // rejects all control/format/surrogate characters and checks normalized text.
  // Reject these before trimming too, so invisible input cannot disappear into a valid request.
  if (/[\p{Cc}\p{Cf}\p{Cs}]/u.test(value)) return null;
  const trimmed = value.trim();
  if (!trimmed || Array.from(trimmed).length > 1000 || new TextEncoder().encode(trimmed).length > 6000 || /[\u2028\u2029]/.test(trimmed)) return null;
  const normalized = trimmed.normalize("NFKC");
  return Array.from(normalized).length <= 1000 && !/[\p{Cc}\p{Cf}\p{Cs}]/u.test(normalized)
    && !/(?:password|passwd|api[ _-]?key|token|authorization|secret)["']?\s*(?:[:=：]|\bis\b)\s*\S+|\bbearer\s+\S+|\b(?:sk|ghp|github_pat|xox[baprs])[-_][a-z0-9_-]{8,}|(?:密码|密钥)\s*(?:是|为|[:=：])\s*\S+|[a-z][a-z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]+@/iu.test(normalized) ? normalized : null;
}
export function r16CompanionCancelTeachingId(v: R16CompanionOwnerView): string | null {
  const p = v.runtime.teachingProposal;
  return ref(p.proposalId) && ["planning", "proposed"].includes(String(p.status)) ? p.proposalId : null;
}
export function r16CompanionResumeLifeId(v: R16CompanionOwnerView, now = Date.now()): string | null {
  const t = v.runtime.lifeTask, g = v.runtime.giftTask;
  return (!g.status || ["idle", "completed", "cancelled"].includes(String(g.status))) && ref(t.lifePermitId) && t.status === "paused"
    && t.budgetGold === 50 && t.maxPurchasedQuantity === 1 && integer(t.expiresAtMs) && t.expiresAtMs > now ? t.lifePermitId : null;
}
export function r16CompanionSupplyComplete(t: R16CompanionTask): boolean {
  return t.status === "completed" && t.receiptKnown === true && t.returnedToAnchor === true && t.acquiredQuantity === 1 && t.budgetGold === 50 && integer(t.chargedGold) && t.chargedGold <= 50;
}
const supplyActive = (v: R16CompanionOwnerView) => ["authorized", "running", "paused", "unknown", "returning"].includes(String(v.runtime.supplyTask.status));
const lifeActive = (v: R16CompanionOwnerView) => !!v.runtime.lifeTask.status && !["idle", "ended", "completed", "cancelled", "failed", "expired"].includes(String(v.runtime.lifeTask.status));
export function r16CompanionCanAuthorizeLife(v: R16CompanionOwnerView): boolean {
  const t = v.runtime.lifeTask, g = v.runtime.giftTask;
  return (!g.status || ["idle", "completed", "cancelled"].includes(String(g.status))) && v.runtime.resources.medicineUnresolved !== true && !supplyActive(v)
    && (!t.status || t.status === "idle" || !lifeActive(v) && v.runtime.resources.medicineUnresolved === false && t.reservedGold === 0);
}
type Pending = { request: R16CompanionFrame; source: R16CompanionSource; deadline: number; mutation: boolean; archiveId: string | null };
type Assembly = { exportId: string; totalBytes: number; sha256: string; chunkCount: number; next: number; bytes: Uint8Array; offset: number; deadline: number; source: R16CompanionSource; archiveId: string };

export class R16CompanionClient {
  private source: R16CompanionSource | null = null;
  private listeners = new Set<() => void>();
  private pending: Pending | null = null;
  private assembly: Assembly | null = null;
  private downloadDeadline = 0;
  private viewCursor: string | undefined;
  private versions = new Map<string, { policy: number; growth: number }>();
  private nonce: string; private sequence = 0;
  private state: R16CompanionSnapshot;
  constructor(private readonly host: R16CompanionHost) {
    this.nonce = (host.randomId ?? (() => crypto.randomUUID()))();
    if (!ref(this.nonce) || this.nonce.length > 64) throw new Error("Invalid companion request namespace");
    this.state = freeze({ renderLease: this.newLease(), contentEpoch: 0, available: false, busy: false, uncertain: false,
      enrollments: freeze([]), selected: null, view: null, notice: "", pendingOperation: null, unknownIntent: null, download: null });
    this.syncSource();
  }
  private newLease(): R16CompanionRenderLease { return freeze(Object.create(null)) as R16CompanionRenderLease; }
  private now() { return (this.host.now ?? Date.now)(); }
  /** Stable snapshots for useSyncExternalStore. Private, in-memory consumers only. */
  snapshot = (): R16CompanionSnapshot => this.state;
  subscribe = (listener: () => void): (() => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
  private publish(patch: Partial<R16CompanionSnapshot>) {
    this.state = freeze({ ...this.state, ...patch, renderLease: this.newLease(), busy: !!this.pending || !!this.assembly, pendingOperation: this.pending?.request.operation ?? null });
    for (const listener of [...this.listeners]) listener();
  }
  acceptsSource(source: R16CompanionSource): boolean { return validSource(source) && sameR16CompanionSource(source, this.source) && sameR16CompanionSource(source, this.host.captureSource()); }
  syncSource(): void {
    const captured = this.host.captureSource(), next = validSource(captured) ? captured : null;
    if (next === null && this.source === null || sameR16CompanionSource(next, this.source)) return;
    this.retire(); this.source = next ? freeze({ ...next }) : null;
    if (this.source) this.publish({ available: true });
  }
  private clearExport() {
    this.assembly?.bytes.fill(0); this.assembly = null;
    if (this.state?.download) {
      try { (this.host.revokeDownload ?? (url => URL.revokeObjectURL(url)))(this.state.download.url); } catch { /* Continue retiring private memory even if a host cleanup fails. */ }
    }
    this.downloadDeadline = 0;
  }
  retire(): void {
    this.pending = null; this.clearExport(); this.source = null; this.versions.clear(); this.viewCursor = undefined;
    this.publish({ available: false, view: null, selected: null, enrollments: freeze([]), uncertain: false, unknownIntent: null, download: null,
      notice: "", contentEpoch: this.state.contentEpoch + 1 });
  }
  isCurrentLease(lease: R16CompanionRenderLease, requireView = false): boolean {
    this.syncSource();
    return lease === this.state.renderLease && this.state.available && !!this.source && this.acceptsSource(this.source)
      && (!requireView || !!this.state.view && !this.state.uncertain);
  }
  /** Host must call again immediately before its existing party sender. */
  invitation(lease: R16CompanionRenderLease): Readonly<{ characterName: string }> | null {
    return this.isCurrentLease(lease, true) && !this.state.busy && this.selected()?.online ? freeze({ characterName: this.state.view!.archive.characterName }) : null;
  }
  private selected() { return this.state.enrollments.find(e => e.residentId === this.state.selected); }
  private send(operation: R16CompanionOperation, payload: Record<string, unknown>, internal = false): boolean {
    if (!this.source || !this.acceptsSource(this.source) || this.pending || !internal && this.assembly) return false;
    const residentId = operation === "list" ? undefined : this.selected()?.residentId;
    if (operation !== "list" && !residentId || !Number.isSafeInteger(this.sequence + 1)) return false;
    const request = freeze({ type: "companionControl" as const, requestId: `wcp-${this.nonce}-${++this.sequence}`,
      ...(residentId ? { residentId } : {}), operation, payload: freeze({ ...payload }) });
    const source = this.source, pending: Pending = { request, source, deadline: this.now() + REQUEST_TTL,
      mutation: !["list", "view", "exportBegin", "exportChunk"].includes(operation), archiveId: this.selected()?.archiveId ?? null };
    this.pending = pending;
    this.publish({ notice: "等待服务器确认…" });
    // Publishing can synchronously change character/source. Host repeats this fence.
    if (this.pending !== pending || !this.acceptsSource(source)) { this.syncSource(); return false; }
    let outcome: "sent" | "not-sent" | "unknown";
    try { outcome = this.host.sendPrivate(source, request); } catch { outcome = "unknown"; }
    if (this.pending !== pending) return outcome === "sent";
    if (outcome === "sent") return true;
    this.pending = null; this.clearExport();
    this.publish({ uncertain: this.state.uncertain || outcome === "unknown" && pending.mutation,
      unknownIntent: outcome === "unknown" && pending.mutation ? this.intent(pending) : this.state.unknownIntent,
      download: null, notice: outcome === "not-sent" ? "连接不可用；请求未发送。" : "结果未确认。请刷新查看，操作不会自动重发。" });
    return false;
  }
  private intent(p: Pending) { return freeze({ requestId: p.request.requestId, operation: p.request.operation, ...(p.request.residentId ? { residentId: p.request.residentId } : {}) }); }
  refresh(lease: R16CompanionRenderLease, cursor?: string): boolean {
    if (!this.isCurrentLease(lease) || this.state.busy || cursor !== undefined && !cursorRef(cursor)) return false;
    return this.send(this.selected() ? "view" : "list", this.selected() ? { limit: 20, ...(cursor ? { cursor } : {}) } : {});
  }
  /** Read-only polling preserves the currently displayed server history page. */
  poll(lease: R16CompanionRenderLease): boolean { return this.refresh(lease, this.viewCursor); }
  list(lease: R16CompanionRenderLease): boolean { return this.isCurrentLease(lease) && !this.state.busy && this.send("list", {}); }
  choose(lease: R16CompanionRenderLease, residentId: string): boolean {
    if (!this.isCurrentLease(lease) || this.state.busy || !this.state.enrollments.some(e => e.residentId === residentId)) return false;
    const source = this.source;
    this.clearExport(); this.viewCursor = undefined;
    this.publish({ selected: residentId, view: null, download: null, contentEpoch: this.state.contentEpoch + 1 });
    if (!sameR16CompanionSource(source, this.source) || this.state.selected !== residentId) return false;
    return this.send("view", { limit: 20 });
  }
  act(lease: R16CompanionRenderLease, action: R16CompanionAction): boolean {
    if (!this.isCurrentLease(lease, true) || this.state.busy || !this.selected()?.online) return false;
    const v = this.state.view!, version = v.profile.policyVersion, source = this.source, enrollment = this.selected()!;
    let payload: Record<string, unknown> = { expectedPolicyVersion: version };
    switch (action.type) {
      case "teach": break;
      case "recall": case "resume": case "cancelSupply": case "cancelLife": payload = {}; break;
      case "instruct": case "proposeTeaching": {
        const value = r16CompanionInstruction(action.text);
        if (!r16CompanionModelReady(v) || !value) return false;
        if (action.type === "proposeTeaching" && v.runtime.teachingProposal.status === "planning" || action.type === "instruct" && lifeActive(v)) return false;
        payload.text = value; break;
      }
      case "confirmTeaching":
        if (r16CompanionTeachingId(v) !== action.proposalId) return false;
        payload.proposalId = action.proposalId; break;
      case "cancelTeaching":
        if (r16CompanionCancelTeachingId(v) !== action.proposalId) return false;
        payload = { proposalId: action.proposalId }; break;
      case "authorizeSupply":
        if (supplyActive(v) || lifeActive(v)) return false;
        payload = { ...payload, budgetGold: 50, quantity: 1, expiresInSeconds: 300 }; break;
      case "authorizeLife":
        if (![1200, 1800].includes(action.durationSeconds) || !r16CompanionCanAuthorizeLife(v)) return false;
        payload = { ...payload, budgetGold: 50, durationSeconds: action.durationSeconds }; break;
      case "resumeLife": {
        if (r16CompanionResumeLifeId(v, this.now()) !== action.lifePermitId) return false;
        payload.lifePermitId = action.lifePermitId; break;
      }
      case "revokeFeedback": case "deleteFeedback":
        if (!v.feedback.some(f => f.feedbackId === action.feedbackId && (action.type === "revokeFeedback" ? f.status === "active" : f.status !== "deleted"))) return false;
        payload.feedbackId = action.feedbackId; break;
      case "deleteEpisode":
        if (!v.episodes.some(e => e.episodeId === action.episodeId)) return false;
        payload.episodeId = action.episodeId; break;
      case "clearGrowth": payload.confirmArchiveId = v.archive.archiveId; break;
      default: return false;
    }
    this.clearExport();
    const forget = ["revokeFeedback", "deleteFeedback", "deleteEpisode", "clearGrowth"].includes(action.type);
    this.publish({ download: null, ...(forget ? { view: null, contentEpoch: this.state.contentEpoch + 1 } : {}) });
    if (!sameR16CompanionSource(source, this.source) || !this.source || !this.acceptsSource(this.source)
      || this.selected()?.residentId !== enrollment.residentId || this.selected()?.archiveId !== enrollment.archiveId || !forget && this.state.view !== v) return false;
    return this.send(action.type, payload);
  }
  exportArchive(lease: R16CompanionRenderLease): boolean {
    if (!this.isCurrentLease(lease, true) || this.state.busy || !this.selected()?.online) return false;
    const source = this.source, view = this.state.view;
    this.clearExport(); this.publish({ download: null });
    if (!sameR16CompanionSource(source, this.source) || this.state.view !== view) return false;
    return this.send("exportBegin", {});
  }
  /** Return true for EVERY private frame, even malformed/foreign/stale ones.
   * Host must early-return so none can enter generic decode/history/logging. */
  receive(source: R16CompanionSource, event: unknown): boolean {
    if (!record(event) || event.type !== "companionResult") return false;
    this.syncSource();
    const p = this.pending;
    if (!p || !this.acceptsSource(source) || !sameR16CompanionSource(source, p.source)) return true;
    if (!exact(event, ["type", "payload"]) || !record(event.payload)) { this.invalid(p); return true; }
    const r = event.payload;
    if (!exact(r, ["requestId", "residentId", "operation", "status", "result"]) || !ref(r.requestId)
      || !R16_COMPANION_OPERATIONS.includes(r.operation as R16CompanionOperation) || !statuses.includes(String(r.status))) { this.invalid(p); return true; }
    if (r.requestId !== p.request.requestId || r.operation !== p.request.operation || (r.residentId ?? undefined) !== p.request.residentId) return true;
    if (this.now() >= p.deadline) { this.tick(); return true; }
    try { if (new TextEncoder().encode(JSON.stringify(r.result)).length > 56 * 1024) { this.invalid(p); return true; } } catch { this.invalid(p); return true; }
    this.pending = null;
    if (r.status !== "ok") {
      this.clearExport();
      if (r.status === "denied") { this.retire(); this.publish({ notice: "本角色没有此伙伴的操作权限。" }); return true; }
      const fresh = r.status === "conflict" && this.selected() ? this.readView(r.result, this.selected()!, source) : null;
      this.publish({ view: fresh, download: null,
        ...(r.status === "offline" ? { enrollments: freeze(this.state.enrollments.map(e => e.residentId === p.request.residentId ? freeze({ ...e, online: false }) : e)) } : {}),
        uncertain: this.state.uncertain || p.mutation && ["unknown", "conflict", "invalid", "error"].includes(String(r.status)),
        unknownIntent: r.status === "unknown" && p.mutation ? this.intent(p) : this.state.unknownIntent,
        contentEpoch: this.state.contentEpoch + 1, notice: ({ unknown: "结果未确认。请刷新查看，操作不会自动重发。", offline: "伙伴当前离线，请稍后刷新。",
          conflict: "培养版本已变化，请刷新并核对后再操作。", invalid: "请求无效，请刷新伙伴资料。", error: "伙伴服务暂不可用，请刷新核对。" } as Record<string, string>)[String(r.status)] });
      return true;
    }
    if (r.operation === "list") {
      if (!record(r.result) || !exact(r.result, ["companions", "worldEpoch"]) || r.result.worldEpoch !== source.worldEpoch || !Array.isArray(r.result.companions) || r.result.companions.length > 64) return this.invalid(p);
      const rows: R16CompanionEnrollment[] = [];
      for (const row of r.result.companions) {
        if (!record(row) || !exact(row, ["residentId", "archiveId", "displayName", "online"]) || !ref(row.residentId) || !ref(row.archiveId) || typeof row.displayName !== "string" || !row.displayName.trim() || row.displayName.length > 128 || typeof row.online !== "boolean") return this.invalid(p);
        rows.push(freeze({ residentId: row.residentId, archiveId: row.archiveId, displayName: text(row.displayName, 64), online: row.online }));
      }
      if (new Set(rows.map(e => e.residentId)).size !== rows.length || new Set(rows.map(e => e.archiveId)).size !== rows.length) return this.invalid(p);
      this.clearExport();
      this.viewCursor = undefined;
      const allowed = new Set(rows.map(e => `${e.residentId}/${e.archiveId}`));
      for (const key of this.versions.keys()) if (!allowed.has(key)) this.versions.delete(key);
      const selected = rows.find(e => e.residentId === this.state.selected)?.residentId ?? rows[0]?.residentId ?? null;
      this.publish({ enrollments: freeze(rows), selected, view: null, download: null, contentEpoch: this.state.contentEpoch + 1, notice: rows.length ? "已读取授权伙伴列表。" : "本角色没有已授权的伙伴。" });
      if (selected) this.send("view", { limit: 20 });
    } else if (r.operation === "exportBegin") this.beginExport(r.result, p);
    else if (r.operation === "exportChunk") this.chunk(r.result, p);
    else {
      const selected = this.selected(), view = selected ? this.readView(r.result, selected, source) : null;
      if (!view || view.archive.archiveId !== p.archiveId) return this.invalid(p);
      const previous = this.state.view, nextCursor = r.operation === "view" ? p.request.payload.cursor as string | undefined : undefined;
      const sameContent = r.operation === "view" && previous?.archive.archiveId === view.archive.archiveId
        && previous.archive.residentId === view.archive.residentId && previous.archive.realmId === view.archive.realmId
        && previous.archive.worldEpoch === view.archive.worldEpoch && previous.profile.policyVersion === view.profile.policyVersion
        && previous.growthRevision === view.growthRevision && nextCursor === this.viewCursor;
      this.viewCursor = nextCursor;
      if (!sameContent) this.clearExport();
      this.publish({ view, uncertain: false, unknownIntent: view.runtime.supplyTask.status === "unknown" ? this.state.unknownIntent : null, download: sameContent ? this.state.download : null,
        enrollments: freeze(this.state.enrollments.map(e => e.residentId === view.archive.residentId ? freeze({ ...e, online: true }) : e)),
        contentEpoch: this.state.contentEpoch + (sameContent ? 0 : 1), notice: r.operation === "teach" || r.operation === "confirmTeaching" ? `教学已保存，培养版本 v${view.profile.policyVersion}。`
          : r.operation === "authorizeSupply" ? "已收到补给授权回执；购买与返程仍须核对实际记录。" : r.operation === "cancelSupply" ? "已收到撤销补给回执；已提交购买的结果仍需核对。" : `已读取服务器记录，培养版本 v${view.profile.policyVersion}。` });
    }
    return true;
  }
  private readView(raw: unknown, selected: R16CompanionEnrollment, source: R16CompanionSource) {
    const view = ownerView(raw, selected, source), key = `${selected.residentId}/${selected.archiveId}`, old = this.versions.get(key);
    if (!view || old && (view.profile.policyVersion < old.policy || view.growthRevision < old.growth)) return null;
    this.versions.set(key, { policy: view.profile.policyVersion, growth: view.growthRevision });
    return view;
  }
  private invalid(p: Pending): true {
    this.pending = null; this.clearExport();
    this.publish({ view: null, download: null, uncertain: this.state.uncertain || p.mutation, unknownIntent: p.mutation ? this.intent(p) : this.state.unknownIntent,
      contentEpoch: this.state.contentEpoch + 1, notice: "服务器资料未通过当前身份与格式校验，请刷新；操作不会自动重发。" });
    return true;
  }
  tick(): void {
    this.syncSource(); const now = this.now();
    const p = this.pending;
    if (p && now >= p.deadline) {
      this.pending = null; this.clearExport();
      this.publish({ download: null, uncertain: this.state.uncertain || p.mutation, unknownIntent: p.mutation ? this.intent(p) : this.state.unknownIntent,
        notice: "结果未确认。请刷新查看，操作不会自动重发。" });
    } else if (this.assembly && now >= this.assembly.deadline || this.downloadDeadline && now >= this.downloadDeadline) {
      this.clearExport(); this.publish({ download: null, notice: "私人导出已到期，请重新导出。" });
    }
  }
  private beginExport(raw: unknown, p: Pending) {
    if (!record(raw) || !exact(raw, ["exportId", "totalBytes", "sha256", "chunkCount"]) || !ref(raw.exportId) || !integer(raw.totalBytes, 1) || raw.totalBytes > MAX_EXPORT
      || raw.chunkCount !== Math.ceil(raw.totalBytes / CHUNK) || typeof raw.sha256 !== "string" || !/^[a-fA-F0-9]{64}$/.test(raw.sha256) || !p.archiveId) { this.invalid(p); return; }
    this.assembly = { exportId: raw.exportId, totalBytes: raw.totalBytes, sha256: raw.sha256.toLowerCase(), chunkCount: raw.chunkCount as number,
      next: 0, bytes: new Uint8Array(raw.totalBytes), offset: 0, deadline: p.deadline - REQUEST_TTL + EXPORT_TTL, source: p.source, archiveId: p.archiveId };
    this.send("exportChunk", { exportId: raw.exportId, index: 0 }, true);
  }
  private chunk(raw: unknown, p: Pending) {
    const a = this.assembly;
    if (!a || !record(raw) || !exact(raw, ["exportId", "index", "chunkCount", "dataBase64"]) || raw.exportId !== a.exportId || raw.index !== a.next || raw.chunkCount !== a.chunkCount
      || p.request.payload.index !== a.next || p.archiveId !== a.archiveId || !this.acceptsSource(a.source) || this.now() >= a.deadline
      || typeof raw.dataBase64 !== "string" || raw.dataBase64.length > 4 * Math.ceil(CHUNK / 3) || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(raw.dataBase64)) { this.invalid(p); return; }
    let decoded: string;
    try { decoded = atob(raw.dataBase64); if (btoa(decoded) !== raw.dataBase64) throw new Error(); } catch { this.invalid(p); return; }
    const expected = Math.min(CHUNK, a.totalBytes - a.offset);
    if (decoded.length !== expected) { this.invalid(p); return; }
    for (let i = 0; i < decoded.length; i++) a.bytes[a.offset + i] = decoded.charCodeAt(i);
    a.offset += expected; a.next++;
    if (a.next < a.chunkCount) this.send("exportChunk", { exportId: a.exportId, index: a.next }, true);
    else { this.publish({ notice: "正在校验完整导出…" }); void this.finishExport(a); }
  }
  private async finishExport(a: Assembly) {
    try {
      const hash = await (this.host.sha256 ?? (async bytes => {
        const digest = await crypto.subtle.digest("SHA-256", new Uint8Array(bytes).buffer);
        return Array.from(new Uint8Array(digest), n => n.toString(16).padStart(2, "0")).join("");
      }))(a.bytes);
      if (this.assembly !== a || !this.acceptsSource(a.source) || this.now() >= a.deadline) { if (this.assembly === a) this.tick(); return; }
      if (hash.toLowerCase() !== a.sha256 || a.offset !== a.totalBytes) throw new Error();
      const url = (this.host.makeDownload ?? (bytes => URL.createObjectURL(new Blob([new Uint8Array(bytes).buffer], { type: "application/json" }))))(a.bytes);
      if (this.assembly !== a || !this.acceptsSource(a.source) || this.now() >= a.deadline) {
        (this.host.revokeDownload ?? (value => URL.revokeObjectURL(value)))(url); return;
      }
      a.bytes.fill(0); this.assembly = null; this.downloadDeadline = a.deadline;
      this.publish({ download: freeze({ url, fileName: `companion-${a.exportId}.json` }), notice: "完整导出已校验，可下载。查看与导出无需购买。" });
    } catch {
      if (this.assembly === a) { this.clearExport(); this.publish({ download: null, notice: "导出校验失败，请重新读取；不会下载不完整资料。" }); }
    } finally { a.bytes.fill(0); }
  }
}
