import type { BevyInventoryModel } from "./bevy-bag-model";
import { validBagModel, type BagIdentity, type BagInputRegion, type BagOwner, type BevyBagPlayer } from "./bevy-bag-ui";
import type { BevyStorageModel } from "./bevy-storage-model";
import type { BevyQuestUiPresentation } from "./bevy-quest-ui";
import type { Mir2Language } from "./localization";

export type StorageIdentity = BagIdentity;
export type StorageSource = { container: 0 | 4; slot: number; uniqueId: number };
export type StorageTarget = { container: 0 | 4; slot: number };
export type StorageIntent = StorageIdentity & {
  intentSequence: number; modelRevision: number; presentationRevision: number; serviceRevision: number;
} & (
  | { type: "close" }
  | { type: "password" }
  | { type: "rent" }
  | { type: "storeItem"; source: StorageSource; target: StorageTarget }
  | { type: "takeBackItem"; source: StorageSource; target: StorageTarget }
  | { type: "moveItem"; source: StorageSource; target: StorageTarget }
  | { type: "mergeItem"; source: StorageSource; target: StorageSource }
);
export type StorageCommandProof = StorageIdentity & { owner: "bevy"; surface: "storage"; intent: StorageIntent };
export type StoragePointerContext = StorageIdentity & {
  modelRevision: number; presentationRevision: number; serviceRevision: number;
  presentation: BevyQuestUiPresentation; inputRegions: BagInputRegion[];
};
export type StoragePointerEdge = Omit<StoragePointerContext, "presentation" | "inputRegions"> & {
  sequence: number; downSequence: number; pointerId: number; phase: "down" | "move" | "up" | "cancel" | "blur";
  origin: "storage" | "world"; x: number; y: number; button: 0 | 2;
};
export type StorageStatus = StorageIdentity & {
  frame: number; ready: boolean; inputEnabled: boolean;
  appliedRevision: number; appliedModelRevision: number; appliedPresentationRevision: number; appliedServiceRevision: number;
  inputRegions: BagInputRegion[]; error: string | null;
};
export type StorageSnapshot = StorageIdentity & {
  revision: number; modelRevision: number; presentationRevision: number; serviceRevision: number;
  open: boolean; inputEnabled: boolean; presentation: BevyQuestUiPresentation | null;
  inventory: BevyInventoryModel; storage: BevyStorageModel; player: BevyBagPlayer;
  blockedUniqueIds: number[]; pendingCells: StorageTarget[]; language: Mir2Language;
};
export type StorageHostInput = Omit<StorageSnapshot, "runGeneration" | "revision" | "modelRevision" | "presentationRevision" | "inputEnabled" | "inventory" | "storage"> & {
  eligible: boolean; inventory: BevyInventoryModel | null; storage: BevyStorageModel | null;
};
export type StorageRuntime = {
  getMir2StorageUiCapabilities?: () => string;
  setMir2StorageUiSnapshot?: (json: string) => boolean;
  getMir2StorageUiStatus?: () => string;
  setMir2StorageUiIntentSink?: (sink: (json: string) => boolean) => void;
  clearMir2StorageUiIntentSink?: () => void;
  setMir2StorageUiPointerEdge?: (json: string) => boolean;
  withdrawMir2StorageUiSnapshot?: (identityJson: string) => boolean;
};
export type StorageHostState = { active: boolean; transitioning: boolean; ownerRevision: number; error: string | null };
export type StorageHostOptions = {
  runtime: StorageRuntime; read: () => StorageHostInput; now: () => number;
  onOwner: (owner: BagOwner, run: number, ownerRevision: number) => void;
  onState: (state: StorageHostState) => void;
  onIntent: (intent: StorageIntent) => boolean;
};
const safe = (n: unknown): n is number => typeof n === "number" && Number.isSafeInteger(n) && n >= 0;
const object = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const identityKeys = ["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision"] as const;
export const sameStorageIdentity = (a: StorageIdentity, b: StorageIdentity) => identityKeys.every(k => a[k] === b[k]);
const identityValid = (v: Record<string, unknown>) => identityKeys.every(k => safe(v[k]))
  && Number(v.runGeneration) > 0 && Number(v.connectionGeneration) > 0 && Number(v.sessionGeneration) > 0;
const cellValid = (v: unknown): v is StorageTarget => object(v) && (v.container === 0 || v.container === 4)
  && safe(v.slot) && v.slot < (v.container === 0 ? 80 : 160);
const sourceValid = (v: unknown): v is StorageSource => cellValid(v) && safe((v as unknown as Record<string, unknown>).uniqueId);

export function parseStorageIntent(json: string): StorageIntent | null {
  try {
    const v: unknown = JSON.parse(json);
    if (!object(v) || !identityValid(v) || ![v.intentSequence, v.modelRevision, v.presentationRevision, v.serviceRevision].every(n => safe(n) && n > 0)) return null;
    if (v.type === "close" || v.type === "password" || v.type === "rent") return v as StorageIntent;
    if (!sourceValid(v.source) || !cellValid(v.target)) return null;
    if (v.type === "storeItem" && v.source.container === 0 && v.target.container === 4
      || v.type === "takeBackItem" && v.source.container === 4 && v.target.container === 0
      || v.type === "moveItem" && v.source.container === v.target.container
      || v.type === "mergeItem" && sourceValid(v.target)) return v as StorageIntent;
  } catch { /* A malformed renderer message never reaches the gateway. */ }
  return null;
}

export function supportsStorage(runtime: StorageRuntime): boolean {
  try {
    const v: unknown = JSON.parse(runtime.getMir2StorageUiCapabilities?.() ?? "null");
    return object(v) && v.schemaVersion === 1 && v.storageUiAbiVersion === 1 && v.storageIntentAbiVersion === 1
      && v.compiled === true && v.startup === true
      && [runtime.setMir2StorageUiSnapshot, runtime.getMir2StorageUiStatus, runtime.setMir2StorageUiIntentSink,
        runtime.clearMir2StorageUiIntentSink, runtime.setMir2StorageUiPointerEdge, runtime.withdrawMir2StorageUiSnapshot].every(fn => typeof fn === "function");
  } catch { return false; }
}

export function fitsStorage(p: BevyQuestUiPresentation | null): boolean {
  return Boolean(p && safe(p.logicalWidth) && safe(p.logicalHeight) && p.logicalWidth > 0 && p.logicalHeight > 0
    && p.logicalWidth <= 16384 && p.logicalHeight <= 16384 && Number.isFinite(p.stageCssScale) && p.stageCssScale > 0 && p.stageCssScale <= 16
    && (p.touch ? p.logicalWidth * p.stageCssScale >= 320 && p.logicalHeight * p.stageCssScale >= 240
      : p.logicalWidth >= 760 && p.logicalHeight >= 430));
}

export function validStorageModel(inventory: BevyInventoryModel | null, storage: BevyStorageModel | null): boolean {
  if (!validBagModel(inventory) || !storage || !safe(storage.size) || storage.size < 1 || storage.size > 160
    || ![storage.has_password, storage.unlocked, storage.has_expanded].every(v => typeof v === "boolean")
    || !Number.isFinite(storage.expiry) || !Number.isInteger(storage.expiry) || !Array.isArray(storage.items)) return false;
  const cells = new Set<number>(), ids = new Set(inventory.items.flatMap(i => i.uniqueId === null ? [] : [i.uniqueId]));
  for (const item of storage.items) {
    if (!item || item.container !== 4 || !safe(item.slot) || item.slot >= storage.size || cells.has(item.slot)
      || item.uniqueId !== null && (!safe(item.uniqueId) || ids.has(item.uniqueId))) return false;
    cells.add(item.slot);
    if (item.uniqueId !== null) ids.add(item.uniqueId);
  }
  return true;
}

export function readStorageStatus(runtime: StorageRuntime): StorageStatus | null {
  try {
    const v: unknown = JSON.parse(runtime.getMir2StorageUiStatus?.() ?? "null");
    if (!object(v) || !identityValid(v) || typeof v.ready !== "boolean" || typeof v.inputEnabled !== "boolean"
      || ![v.frame, v.appliedRevision, v.appliedModelRevision, v.appliedPresentationRevision, v.appliedServiceRevision].every(safe)
      || !(v.error === null || typeof v.error === "string") || !Array.isArray(v.inputRegions)
      || !v.inputRegions.every(r => object(r) && [r.left, r.top, r.width, r.height].every(n => typeof n === "number" && Number.isFinite(n))
        && Number(r.width) > 0 && Number(r.height) > 0)) return null;
    return v as StorageStatus;
  } catch { return null; }
}

function registry(): WeakMap<StorageRuntime, BevyStorageHost> {
  const global = globalThis as typeof globalThis & { __mir2StorageHosts?: WeakMap<StorageRuntime, BevyStorageHost> };
  return global.__mir2StorageHosts ??= new WeakMap();
}
function nextRun() {
  const global = globalThis as typeof globalThis & { __mir2StorageRun?: number };
  return global.__mir2StorageRun = (global.__mir2StorageRun ?? 0) + 1;
}
function identity(v: StorageIdentity): StorageIdentity {
  return { runGeneration: v.runGeneration, connectionGeneration: v.connectionGeneration,
    sessionGeneration: v.sessionGeneration, ownerRevision: v.ownerRevision };
}

/** Mailbox and input ownership only; the existing Page request Map owns transport. */
export class BevyStorageHost {
  private readonly runGeneration = nextRun();
  private phase: "prepare" | "arming" | "active" = "prepare";
  private ownerRevision: number;
  private revision = 0; private modelRevision = 0; private presentationRevision = 0;
  private modelKey = ""; private presentationKey = ""; private sentKey = "";
  private sent: StorageSnapshot | null = null;
  private lastFrame = 0; private lastFrameAt: number; private armingAt = 0;
  private sequence = 0; private proof: StorageIntent | null = null; private stopped = false;
  private attached = false;
  private pointerLease: StoragePointerEdge | null = null;
  constructor(private readonly options: StorageHostOptions) {
    this.ownerRevision = options.read().ownerRevision;
    this.lastFrameAt = options.now();
    registry().set(options.runtime, this);
    this.attach();
  }
  private attach() {
    if (!this.attached && this.owns() && supportsStorage(this.options.runtime)) {
      this.options.runtime.setMir2StorageUiIntentSink!(json => this.intent(json)); this.attached = true;
    }
  }
  private owns() { return !this.stopped && registry().get(this.options.runtime) === this; }
  private modelFingerprint(i: StorageHostInput) { return JSON.stringify([i.inventory, i.storage, i.player]); }
  private presentationFingerprint(i: StorageHostInput) { return JSON.stringify([i.connectionGeneration, i.sessionGeneration, i.serviceRevision, i.open, i.presentation, i.language]); }
  private eligible(i: StorageHostInput) {
    return i.eligible && i.open && safe(i.connectionGeneration) && i.connectionGeneration > 0
      && safe(i.sessionGeneration) && i.sessionGeneration > 0 && safe(i.serviceRevision) && i.serviceRevision > 0
      && fitsStorage(i.presentation) && validStorageModel(i.inventory, i.storage);
  }
  private state(error: string | null = null) {
    this.options.onState({ active: this.phase === "active", transitioning: this.phase === "arming", ownerRevision: this.ownerRevision, error });
  }
  private changeOwner(owner: BagOwner) {
    this.ownerRevision = Math.max(this.ownerRevision, this.options.read().ownerRevision) + 1;
    this.proof = null;
    this.options.onOwner(owner, this.runGeneration, this.ownerRevision);
  }
  private fallback(error: string | null) {
    if (!this.owns()) return;
    if (this.phase !== "prepare") this.changeOwner("react");
    this.phase = "prepare"; this.proof = null; this.state(error);
  }
  private publish(i: StorageHostInput, enabled: boolean) {
    if (!this.owns()) return;
    if (!i.inventory || !i.storage || !validStorageModel(i.inventory, i.storage)) { this.withdraw(); return; }
    const { eligible: _eligible, ...input } = i;
    const snapshot = { ...input, inventory: i.inventory, storage: i.storage,
      ...identity({ ...i, runGeneration: this.runGeneration, ownerRevision: this.ownerRevision }),
      open: this.eligible(i), inputEnabled: enabled && this.eligible(i), modelRevision: this.modelRevision, presentationRevision: this.presentationRevision };
    const key = JSON.stringify(snapshot);
    if (key === this.sentKey) return;
    const sent = { ...snapshot, revision: ++this.revision };
    if (!this.options.runtime.setMir2StorageUiSnapshot?.(JSON.stringify(sent))) throw new Error("Shared storage rejected the snapshot");
    this.sent = sent; this.sentKey = key;
  }
  private status(): StorageStatus | null {
    const v = readStorageStatus(this.options.runtime), s = this.sent;
    if (!this.owns() || !v || !s || !sameStorageIdentity(v, s)) return null;
    if (v.frame > this.lastFrame) { this.lastFrame = v.frame; this.lastFrameAt = this.options.now(); }
    return v.ready && !v.error && v.frame > 0 && v.frame >= this.lastFrame && this.options.now() - this.lastFrameAt <= 2000 ? v : null;
  }
  private exact(v: StorageStatus) {
    const s = this.sent;
    return Boolean(s && v.appliedRevision === s.revision && v.appliedModelRevision === s.modelRevision
      && v.appliedPresentationRevision === s.presentationRevision && v.appliedServiceRevision === s.serviceRevision);
  }
  tick() {
    if (!this.owns()) return;
    if (!supportsStorage(this.options.runtime)) { this.withdraw(); this.state("Shared storage ABI 1 is unavailable"); return; }
    try {
      this.attach();
      const i = this.options.read(), mk = this.modelFingerprint(i), pk = this.presentationFingerprint(i);
      if (pk !== this.presentationKey) { this.fallback(null); this.presentationKey = pk; ++this.presentationRevision; }
      if (mk !== this.modelKey) { this.proof = null; this.modelKey = mk; ++this.modelRevision; }
      if (!this.eligible(i)) { this.fallback(null); this.publish(i, false); return; }
      this.publish(i, this.phase !== "prepare");
      const v = this.status();
      if (!v) {
        if (this.phase === "arming" && this.options.now() - this.armingAt <= 2000 && !readStorageStatus(this.options.runtime)?.error) return;
        this.fallback("Shared storage is waiting for its painted controls"); this.publish(i, false); return;
      }
      if (this.phase === "prepare" && this.exact(v) && !v.inputEnabled && v.inputRegions.length > 0) {
        this.changeOwner("transition"); this.phase = "arming"; this.armingAt = this.options.now(); this.state(); this.publish(i, true);
      } else if (this.phase === "arming" && this.exact(v) && v.inputEnabled && v.inputRegions.length > 0) {
        this.phase = "active"; this.options.onOwner("bevy", this.runGeneration, this.ownerRevision); this.state();
      } else if (this.phase === "arming" && this.options.now() - this.armingAt > 2000
        || this.phase === "active" && (!v.inputEnabled || v.appliedPresentationRevision !== this.presentationRevision || v.appliedServiceRevision !== i.serviceRevision)) {
        this.fallback("Shared storage input handoff expired"); this.publish(i, false);
      }
    } catch (e) {
      this.withdraw(); this.state(e instanceof Error ? e.message : "Shared storage unavailable");
    }
  }
  pointerContext(): StoragePointerContext | null {
    if (!this.owns() || this.phase !== "active" || !supportsStorage(this.options.runtime)) return null;
    const i = this.options.read(), v = this.status(), s = this.sent;
    if (!this.eligible(i) || !v?.inputEnabled || !s?.presentation || !this.exact(v)
      || i.ownerRevision !== this.ownerRevision || this.modelFingerprint(i) !== this.modelKey
      || this.presentationFingerprint(i) !== this.presentationKey) return null;
    return { ...identity(s), modelRevision: s.modelRevision, presentationRevision: s.presentationRevision,
      serviceRevision: s.serviceRevision, presentation: s.presentation, inputRegions: v.inputRegions };
  }
  pointer(edge: StoragePointerEdge): boolean {
    if (!this.owns()) return false;
    if (!safe(edge.sequence) || edge.sequence === 0 || !safe(edge.downSequence) || edge.downSequence === 0
      || edge.phase === "down" && edge.downSequence !== edge.sequence) return false;
    const c = this.pointerContext();
    const current = c && sameStorageIdentity(c, edge) && c.modelRevision === edge.modelRevision
      && c.presentationRevision === edge.presentationRevision && c.serviceRevision === edge.serviceRevision;
    if (!current && edge.phase !== "up" && edge.phase !== "cancel" && edge.phase !== "blur") return false;
    const lease = this.pointerLease;
    const matching = lease && sameStorageIdentity(lease, edge) && lease.modelRevision === edge.modelRevision
      && lease.presentationRevision === edge.presentationRevision && lease.serviceRevision === edge.serviceRevision
      && lease.downSequence === edge.downSequence && lease.pointerId === edge.pointerId && lease.origin === edge.origin && lease.button === edge.button;
    if (current && edge.phase === "down") { this.proof = null; this.pointerLease = edge; }
    else if (matching && edge.phase !== "move") {
      this.pointerLease = null;
      if (edge.phase === "cancel" || edge.phase === "blur") this.proof = null;
    }
    const terminal = !current && edge.phase === "up" ? { ...edge, phase: "cancel" as const } : edge;
    try {
      const accepted = this.options.runtime.setMir2StorageUiPointerEdge?.(JSON.stringify(terminal)) === true;
      if (!accepted && this.pointerLease === edge) this.pointerLease = null;
      return accepted;
    }
    catch { this.withdraw(); return false; }
  }
  allows(i: StorageIntent): boolean {
    const c = this.pointerContext();
    return Boolean(this.proof && JSON.stringify(i) === JSON.stringify(this.proof) && c && sameStorageIdentity(c, i)
      && c.modelRevision === i.modelRevision && c.presentationRevision === i.presentationRevision && c.serviceRevision === i.serviceRevision);
  }
  claim(i: StorageIntent): boolean { if (!this.allows(i)) return false; this.proof = null; return true; }
  private intent(json: string): boolean {
    const i = parseStorageIntent(json), c = this.pointerContext();
    if (!i || !c || !sameStorageIdentity(c, i) || i.intentSequence <= this.sequence
      || i.modelRevision !== c.modelRevision || i.presentationRevision !== c.presentationRevision || i.serviceRevision !== c.serviceRevision) return false;
    if ("source" in i) { Object.freeze(i.source); Object.freeze(i.target); }
    Object.freeze(i);
    this.sequence = i.intentSequence; this.proof = i;
    try { return this.options.onIntent(i) === true; } catch { return false; }
  }
  withdraw() {
    if (!this.owns()) return;
    const s = this.sent; this.fallback(null); this.sent = null; this.sentKey = ""; this.pointerLease = null;
    if (s) try { this.options.runtime.withdrawMir2StorageUiSnapshot?.(JSON.stringify(identity(s))); } catch { /* Retired renderer. */ }
  }
  stop() {
    if (this.stopped) return;
    if (this.owns()) { this.withdraw(); registry().delete(this.options.runtime); this.options.runtime.clearMir2StorageUiIntentSink?.(); }
    this.stopped = true;
  }
}

/** Down origin stays fixed until its matching release, including world gestures. */
export class StoragePointerRouter {
  private sequence = 0; private lease: StoragePointerEdge | null = null;
  private quarantine = new Map<number, StoragePointerEdge>();
  get held() { return this.lease; }
  down(c: StoragePointerContext, pointerId: number, button: 0 | 2, x: number, y: number): StoragePointerEdge | null {
    if (this.lease) return null;
    this.quarantine.delete(pointerId);
    const origin = c.inputRegions.some(r => x >= r.left && y >= r.top && x < r.left + r.width && y < r.top + r.height) ? "storage" : "world";
    const { presentation: _p, inputRegions: _r, ...clock } = c;
    const sequence = ++this.sequence;
    return this.lease = { ...clock, sequence, downSequence: sequence, pointerId, button, x, y, origin, phase: "down" };
  }
  edge(phase: "move" | "up" | "cancel", pointerId: number, x: number, y: number): StoragePointerEdge | null {
    const retired = this.quarantine.get(pointerId);
    if (phase !== "move") { this.quarantine.delete(pointerId); if (retired) return { ...retired, sequence: ++this.sequence, phase, x, y }; }
    if (!this.lease || this.lease.pointerId !== pointerId) return null;
    const e = { ...this.lease, sequence: ++this.sequence, phase, x, y };
    this.lease = phase === "move" ? e : null;
    return e;
  }
  cancel(phase: "cancel" | "blur" = "cancel") {
    if (!this.lease) return null;
    this.quarantine.set(this.lease.pointerId, this.lease);
    const e = { ...this.lease, phase, sequence: ++this.sequence }; this.lease = null; return e;
  }
  matches(c: StoragePointerContext | null) {
    return Boolean(this.lease && c && sameStorageIdentity(this.lease, c) && this.lease.modelRevision === c.modelRevision
      && this.lease.presentationRevision === c.presentationRevision && this.lease.serviceRevision === c.serviceRevision);
  }
}
