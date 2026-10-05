import type { BevyInventoryModel } from "./bevy-bag-model";
import type { BevyQuestUiPresentation } from "./bevy-quest-ui";
import { crystalInventoryCapacities } from "./world-model/item-identity";

export type BagIdentity = {
  runGeneration: number;
  connectionGeneration: number;
  sessionGeneration: number;
  ownerRevision: number;
};
export type BagOwner = "react" | "transition" | "bevy";
export type BagOwnerToken = BagIdentity & { owner: BagOwner };
export type BagCommandToken = BagOwnerToken & { modelRevision?: number; presentationRevision?: number };
export type BagPage = "bag1" | "bag2" | "quest";
export type BagInputRegion = { left: number; top: number; width: number; height: number };
/** Only known fields are supplied; omitted requirement stats stay unknown in Rust. */
export type BevyBagPlayer = {
  hp?: number; maxHp?: number; mp?: number; maxMp?: number;
  gold?: number; credit?: number; level?: number;
  name?: string | null; className?: string | null; gender?: string | null;
  currentWeightKnown?: boolean;
};
export type BevyBagUiSnapshot = BagIdentity & {
  revision: number; modelRevision: number; presentationRevision: number;
  bagOpen: boolean; page: BagPage; inputEnabled: boolean;
  presentation: BevyQuestUiPresentation | null;
  model: BevyInventoryModel; player: BevyBagPlayer; blockedUniqueIds: number[];
};
export type BevyBagUiStatus = BagIdentity & {
  frame: number; ready: boolean; inputEnabled: boolean;
  appliedRevision: number; appliedModelRevision: number; appliedPresentationRevision: number;
  inputRegions: BagInputRegion[]; error: string | null;
};
export type BagSource = { container: 0; slot: number; uniqueId: number };
export type BevyBagUiIntent = BagIdentity & {
  intentSequence: number; modelRevision: number; presentationRevision: number;
} & (
  | { type: "useItem" | "equipItem"; source: BagSource }
  | { type: "moveItem"; source: BagSource; target: { container: 0; slot: number } }
  | { type: "close" }
  | { type: "selectPage"; page: BagPage }
  | { type: "handoff"; mode: "fullInventory" | "delete" }
);
export type BevyBagUiIntentResult = { accepted: boolean; error?: string };
export type BagPointerEdge = BagIdentity & {
  sequence: number; presentationRevision: number; pointerId: number;
  phase: "down" | "move" | "up" | "cancel" | "blur";
  origin: "bag" | "world"; x: number; y: number; button: 0 | 2;
};
export type BagPointerContext = BagIdentity & {
  presentationRevision: number;
  presentation: BevyQuestUiPresentation;
  inputRegions: BagInputRegion[];
};
export type BevyBagUiRuntime = {
  getMir2BagUiAbiVersion?: () => number;
  setMir2BagUiSnapshot?: (json: string) => boolean;
  getMir2BagUiStatus?: () => string;
  setMir2BagUiIntentSink?: (sink: (json: string) => string) => void;
  clearMir2BagUiIntentSink?: () => void;
  setMir2BagUiPointerEdge?: (json: string) => boolean;
};
export type BagHostInput = Omit<BevyBagUiSnapshot,
  "runGeneration" | "revision" | "modelRevision" | "presentationRevision" | "inputEnabled" | "model"> & {
  eligible: boolean;
  model: BevyInventoryModel | null;
};
export type BagHostState = { active: boolean; ownerRevision: number; error: string | null };
export type BagHostOptions = {
  runtime: BevyBagUiRuntime;
  read: () => BagHostInput;
  now: () => number;
  onOwner: (owner: BagOwner, runGeneration: number, revision: number) => void;
  onState: (state: BagHostState) => void;
  onIntent: (intent: BevyBagUiIntent) => BevyBagUiIntentResult;
};

const integer = (n: unknown): n is number => typeof n === "number" && Number.isSafeInteger(n) && n >= 0;
const record = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const page = (v: unknown): v is BagPage => v === "bag1" || v === "bag2" || v === "quest";
export const sameBagIdentity = (a: BagIdentity, b: BagIdentity): boolean =>
  a.runGeneration === b.runGeneration && a.connectionGeneration === b.connectionGeneration
  && a.sessionGeneration === b.sessionGeneration && a.ownerRevision === b.ownerRevision;
export const sameBagOwner = (a: BagOwnerToken, b: BagOwnerToken): boolean =>
  a.owner !== "transition" && a.owner === b.owner && sameBagIdentity(a, b);
const identityValid = (v: Record<string, unknown>) =>
  [v.runGeneration, v.connectionGeneration, v.sessionGeneration].every((n) => integer(n) && n > 0)
  && integer(v.ownerRevision);
const sourceValid = (v: unknown): v is BagSource => record(v) && v.container === 0
  && integer(v.slot) && v.slot < 80 && integer(v.uniqueId);

export function parseBagIntent(json: string): BevyBagUiIntent | null {
  try {
    const v: unknown = JSON.parse(json);
    if (!record(v) || !identityValid(v) || !integer(v.intentSequence) || v.intentSequence === 0
      || !integer(v.modelRevision) || !integer(v.presentationRevision)) return null;
    if (v.type === "close" || (v.type === "selectPage" && page(v.page))
      || (v.type === "handoff" && (v.mode === "fullInventory" || v.mode === "delete"))) return v as BevyBagUiIntent;
    if (!sourceValid(v.source)) return null;
    if (v.type === "useItem" || v.type === "equipItem") return v as BevyBagUiIntent;
    if (v.type === "moveItem" && record(v.target) && v.target.container === 0
      && integer(v.target.slot) && v.target.slot < 80) return v as BevyBagUiIntent;
  } catch { /* Invalid input never becomes an item action. */ }
  return null;
}

export function readBagStatus(runtime: BevyBagUiRuntime): BevyBagUiStatus | null {
  try {
    const v: unknown = JSON.parse(runtime.getMir2BagUiStatus?.() ?? "null");
    if (!record(v) || !identityValid(v) || typeof v.ready !== "boolean" || typeof v.inputEnabled !== "boolean"
      || ![v.frame, v.appliedRevision, v.appliedModelRevision, v.appliedPresentationRevision].every(integer)
      || !(v.error === null || typeof v.error === "string") || !Array.isArray(v.inputRegions)
      || !v.inputRegions.every((r) => record(r) && [r.left, r.top, r.width, r.height].every((n) => typeof n === "number" && Number.isFinite(n))
        && Number(r.width) > 0 && Number(r.height) > 0)) return null;
    return v as BevyBagUiStatus;
  } catch { return null; }
}

export function hasBagUiAbi(runtime: BevyBagUiRuntime): boolean {
  try {
    return runtime.getMir2BagUiAbiVersion?.() === 1
      && [runtime.setMir2BagUiSnapshot, runtime.getMir2BagUiStatus, runtime.setMir2BagUiIntentSink,
        runtime.clearMir2BagUiIntentSink, runtime.setMir2BagUiPointerEdge].every((fn) => typeof fn === "function");
  } catch { return false; }
}

/** The projector validates metadata. This extra gate prevents malformed/repeated placement replacement. */
export function validNpcGoldBuyBagModel(model: BevyInventoryModel | null): model is BevyInventoryModel {
  return validBagModel(model, true);
}

export function validBagModel(model: BevyInventoryModel | null, npcGoldTrade = false): model is BevyInventoryModel {
  if (!model || !crystalInventoryCapacities(model.capacity)
    || !integer(model.gold) || model.gold > 0xffff_ffff || !Array.isArray(model.items)) return false;
  const evidence = model.npcGoldTradeCapacity;
  if (npcGoldTrade && evidence !== undefined && evidence !== null
    && (typeof evidence !== "object" || Array.isArray(evidence)
      || Object.keys(evidence).sort().join("|") !== "freshCompatibleUniqueIds|rosterValid"
      || typeof evidence.rosterValid !== "boolean" || !Array.isArray(evidence.freshCompatibleUniqueIds)
      || evidence.freshCompatibleUniqueIds.length > 86
      || !evidence.rosterValid && evidence.freshCompatibleUniqueIds.length !== 0
      || evidence.freshCompatibleUniqueIds.some(id => !integer(id))
      || new Set(evidence.freshCompatibleUniqueIds).size !== evidence.freshCompatibleUniqueIds.length)) return false;
  if (npcGoldTrade && evidence?.rosterValid === false) return false;
  const aliases = npcGoldTrade && evidence?.rosterValid === true;
  const compatible = new Set(npcGoldTrade ? evidence?.freshCompatibleUniqueIds ?? [] : []);
  const cells = new Set<string>();
  const ids = new Set<number>();
  const gridIds = new Set<string>();
  for (const item of model.items) {
    if (!item || !integer(item.slot) || !integer(item.container) || item.container > 3
      || item.slot >= [model.capacity - 6, 6, 14, 40][item.container]
      || (item.uniqueId !== null && !integer(item.uniqueId))) return false;
    const cell = `${item.container}:${item.slot}`;
    if (cells.has(cell) || item.uniqueId !== null && (gridIds.has(item.container + ":" + item.uniqueId)
      || ids.has(item.uniqueId) && (!aliases || compatible.has(item.uniqueId)))) return false;
    cells.add(cell);
    if (item.uniqueId !== null) { ids.add(item.uniqueId); gridIds.add(item.container + ":" + item.uniqueId); }
  }
  return [...compatible].every(id => {
    const rows = model.items.filter(item => item.uniqueId === id);
    return rows.length === 1 && rows[0].container < 2;
  });
}

/** The touch painter needs a real wide stage; letterboxed 4:3 and portrait stay with React. */
export function eligibleBagPresentation(presentation: BevyQuestUiPresentation | null): boolean {
  if (!presentation) return false;
  const { logicalWidth, logicalHeight, stageCssScale, touch } = presentation;
  if (!Number.isSafeInteger(logicalWidth) || !Number.isSafeInteger(logicalHeight)
    || logicalWidth <= 0 || logicalHeight <= 0 || !Number.isFinite(stageCssScale) || stageCssScale <= 0) return false;
  if (!touch) return true;
  const cssWidth = logicalWidth * stageCssScale;
  const cssHeight = logicalHeight * stageCssScale;
  return Number.isFinite(cssWidth) && Number.isFinite(cssHeight)
    && cssWidth >= 600 && cssHeight >= 320 && cssWidth / cssHeight >= 1.5;
}

function nextRunGeneration(): number {
  const global = globalThis as typeof globalThis & { __mir2BagHostGeneration?: number };
  return global.__mir2BagHostGeneration = (global.__mir2BagHostGeneration ?? 0) + 1;
}

/** A renderer mailbox, not another operation ledger. Owner changes never clear equipment pending. */
export class BevyBagHost {
  private readonly runGeneration = nextRunGeneration();
  private phase: "prepare" | "arming" | "active" = "prepare";
  private ownerRevision: number;
  private revision = 0;
  private modelRevision = 0;
  private presentationRevision = 0;
  private modelKey = "";
  private layoutKey = "";
  private sentKey = "";
  private sent: BevyBagUiSnapshot | null = null;
  private lastFrame = -1;
  private lastFrameAt = 0;
  private armingStartedAt = 0;
  private intentSequence = 0;
  private stopped = false;
  private readonly capable: boolean;

  constructor(private readonly options: BagHostOptions) {
    this.ownerRevision = options.read().ownerRevision;
    this.capable = hasBagUiAbi(options.runtime);
    this.changeOwner("react");
    if (this.capable) options.runtime.setMir2BagUiIntentSink!((json) => JSON.stringify(this.intent(json)));
  }

  private changeOwner(owner: BagOwner) {
    this.ownerRevision = Math.max(this.ownerRevision, this.options.read().ownerRevision) + 1;
    this.intentSequence = 0;
    this.options.onOwner(owner, this.runGeneration, this.ownerRevision);
  }

  private fallback(error: string | null) {
    if (this.phase !== "prepare") this.changeOwner("react");
    this.phase = "prepare";
    this.options.onState({ active: false, ownerRevision: this.ownerRevision, error });
  }

  private eligible(input: BagHostInput): boolean {
    return input.eligible && input.bagOpen && input.connectionGeneration > 0 && input.sessionGeneration > 0
      && integer(input.connectionGeneration) && integer(input.sessionGeneration)
      && eligibleBagPresentation(input.presentation) && validBagModel(input.model);
  }

  private modelFingerprint(input: BagHostInput) {
    // Reservation flags may change during our own reserve -> send sequence.
    // The one ledger checks those flags; they are not a replacement item model.
    return JSON.stringify([input.model, input.player]);
  }

  private layoutFingerprint(input: BagHostInput) {
    return JSON.stringify([input.connectionGeneration, input.sessionGeneration, input.bagOpen, input.page, input.presentation]);
  }

  private publish(input: BagHostInput, enabled: boolean) {
    // Invalid input can hide the last model, but can never replace it or authorize input.
    const model = validBagModel(input.model) ? input.model : this.sent?.model;
    if (!model || input.connectionGeneration <= 0 || input.sessionGeneration <= 0) {
      if (this.sent) this.options.runtime.setMir2BagUiSnapshot!(JSON.stringify({ ...this.sent,
        revision: ++this.revision, ownerRevision: this.ownerRevision, bagOpen: false, inputEnabled: false }));
      return;
    }
    const { eligible: _eligible, ...data } = input;
    const snapshot = { ...data, model, runGeneration: this.runGeneration, ownerRevision: this.ownerRevision,
      bagOpen: this.eligible(input), inputEnabled: enabled,
      modelRevision: this.modelRevision, presentationRevision: this.presentationRevision };
    const key = JSON.stringify(snapshot);
    if (key === this.sentKey) return;
    const sent = { ...snapshot, revision: ++this.revision };
    if (!this.options.runtime.setMir2BagUiSnapshot!(JSON.stringify(sent))) throw new Error("Shared bag rejected the snapshot");
    this.sent = sent;
    this.sentKey = key;
  }

  private healthyStatus(): BevyBagUiStatus | null {
    const status = readBagStatus(this.options.runtime);
    if (!status || !this.sent || !sameBagIdentity(status, this.sent)) return null;
    const now = this.options.now();
    if (status.frame > this.lastFrame) { this.lastFrame = status.frame; this.lastFrameAt = now; }
    if (!status.ready || status.error || status.frame === 0 || status.frame < this.lastFrame || now - this.lastFrameAt > 2000) return null;
    return status;
  }

  tick() {
    if (this.stopped) return;
    if (!this.capable) { this.fallback("Shared bag ABI 1 is unavailable"); return; }
    try {
      const input = this.options.read();
      const model = this.modelFingerprint(input);
      const layout = this.layoutFingerprint(input);
      if (layout !== this.layoutKey) {
        this.fallback(null);
        this.layoutKey = layout;
        this.presentationRevision += 1;
      }
      if (model !== this.modelKey) { this.modelKey = model; this.modelRevision += 1; }
      if (!this.eligible(input)) {
        this.fallback(null);
        this.publish(input, false);
        return;
      }
      this.publish(input, this.phase !== "prepare");
      const status = this.healthyStatus();
      if (!status) {
        // The first enabled frame belongs to the new epoch. The preceding
        // disabled frame is expected while the renderer applies this request.
        if (this.phase === "arming" && this.options.now() - this.armingStartedAt <= 2000
          && !readBagStatus(this.options.runtime)?.error) return;
        this.fallback("Shared bag is waiting for a live renderer");
        this.publish(input, false);
        return;
      }
      const exact = status.appliedRevision === this.sent?.revision
        && status.appliedModelRevision === this.modelRevision
        && status.appliedPresentationRevision === this.presentationRevision;
      if (this.phase === "arming" && (!exact || !status.inputEnabled)
        && this.options.now() - this.armingStartedAt > 2000) {
        this.fallback("Shared bag did not apply the input handoff");
        this.publish(input, false);
        return;
      }
      if (this.phase === "prepare" && exact && !status.inputEnabled && status.inputRegions.length > 0) {
        this.changeOwner("transition");
        this.phase = "arming";
        this.armingStartedAt = this.options.now();
        this.publish(input, true);
      } else if (this.phase === "arming" && exact && status.inputEnabled && status.inputRegions.length > 0) {
        this.phase = "active";
        this.options.onOwner("bevy", this.runGeneration, this.ownerRevision);
        this.options.onState({ active: true, ownerRevision: this.ownerRevision, error: null });
      } else if (this.phase === "active" && (!status.inputEnabled || status.appliedPresentationRevision !== this.presentationRevision)) {
        this.fallback("Shared bag layout changed");
        this.publish(input, false);
      }
    } catch (error) {
      this.fallback(error instanceof Error ? error.message : "Shared bag unavailable");
      // Hide the last accepted frame too, even when the current model is malformed.
      if (this.sent) {
        try { this.options.runtime.setMir2BagUiSnapshot!(JSON.stringify({ ...this.sent, revision: ++this.revision,
          ownerRevision: this.ownerRevision, inputEnabled: false, bagOpen: false })); } catch { /* React owns input. */ }
      }
      this.sentKey = "";
    }
  }

  /** Also used synchronously by the send gate, before the next polling tick. */
  pointerContext(): BagPointerContext | null {
    if (this.stopped || this.phase !== "active") return null;
    const input = this.options.read();
    const status = this.healthyStatus();
    if (!this.eligible(input) || !status?.inputEnabled || !this.sent?.presentation
      || this.layoutFingerprint(input) !== this.layoutKey
      || input.ownerRevision !== this.ownerRevision
      || status.appliedPresentationRevision !== this.presentationRevision) {
      this.fallback("Shared bag input expired");
      return null;
    }
    return { runGeneration: this.runGeneration, connectionGeneration: input.connectionGeneration,
      sessionGeneration: input.sessionGeneration, ownerRevision: this.ownerRevision,
      presentationRevision: this.presentationRevision, presentation: this.sent.presentation,
      inputRegions: status.inputRegions };
  }

  pointer(edge: BagPointerEdge): boolean {
    // A retired gesture may still release after its owner disappeared. Rust
    // treats stale terminal edges as cleanup only, never as a click.
    if (this.capable && (edge.phase === "up" || edge.phase === "cancel" || edge.phase === "blur")) {
      const context = this.pointerContext();
      const current = context && sameBagIdentity(context, edge) && context.presentationRevision === edge.presentationRevision;
      // Even a currently matching Rust epoch must not click after host focus,
      // layout or health loss, before the hide snapshot reaches its next frame.
      const terminal = !current && edge.phase === "up" ? { ...edge, phase: "cancel" as const } : edge;
      try { return this.options.runtime.setMir2BagUiPointerEdge!(JSON.stringify(terminal)); } catch { return false; }
    }
    const context = this.pointerContext();
    if (!context || !sameBagIdentity(context, edge) || edge.presentationRevision !== context.presentationRevision) return false;
    try { return this.options.runtime.setMir2BagUiPointerEdge!(JSON.stringify(edge)); } catch { this.fallback("Shared bag input failed"); return false; }
  }

  allows(token: BagCommandToken): boolean {
    const context = this.pointerContext();
    return !!context && sameBagIdentity(token, context)
      && (token.presentationRevision === undefined || token.presentationRevision === this.presentationRevision)
      && (token.modelRevision === undefined || (token.modelRevision === this.modelRevision
        && this.modelFingerprint(this.options.read()) === this.modelKey));
  }

  private intent(json: string): BevyBagUiIntentResult {
    const intent = parseBagIntent(json);
    const context = this.pointerContext();
    const input = this.options.read();
    if (!intent || !context || !sameBagIdentity(intent, context) || intent.intentSequence <= this.intentSequence
      || intent.presentationRevision !== this.presentationRevision || intent.modelRevision !== this.modelRevision
      || this.modelFingerprint(input) !== this.modelKey) return { accepted: false, error: "The bag selection changed." };
    this.intentSequence = intent.intentSequence;
    try { return this.options.onIntent(intent); } catch { return { accepted: false, error: "The item action could not be sent." }; }
  }

  /** Local modal/close handoff invalidates queues before React exposes the replacement. */
  yieldToReact() {
    this.fallback(null);
    if (this.capable && !this.stopped) {
      try { this.publish({ ...this.options.read(), eligible: false }, false); } catch { /* React remains owner. */ }
    }
  }

  stop() {
    if (this.stopped) return;
    this.yieldToReact();
    this.stopped = true;
    if (this.capable) this.options.runtime.clearMir2BagUiIntentSink!();
  }
}

/** Ordered down-origin routing. A canceled held pointer cannot acquire a new owner until release. */
export class BagPointerRouter {
  private sequence = 0;
  private lease: BagPointerEdge | null = null;
  private quarantine = new Map<number, BagPointerEdge>();
  get held() { return this.lease; }
  down(context: BagPointerContext, pointerId: number, button: 0 | 2, x: number, y: number): BagPointerEdge | null {
    if (this.lease) return null;
    // A new browser down proves the prior physical press with this ID ended,
    // even when blur/owner loss prevented its terminal edge from arriving.
    this.quarantine.delete(pointerId);
    const origin = context.inputRegions.some((r) => x >= r.left && y >= r.top && x < r.left + r.width && y < r.top + r.height) ? "bag" : "world";
    const { inputRegions: _regions, presentation: _presentation, ...identity } = context;
    return this.lease = { ...identity, pointerId, button, origin, x, y, phase: "down", sequence: ++this.sequence };
  }
  edge(phase: "move" | "up" | "cancel", pointerId: number, x: number, y: number): BagPointerEdge | null {
    const retired = this.quarantine.get(pointerId);
    if (phase !== "move") {
      this.quarantine.delete(pointerId);
      if (retired) return { ...retired, phase, x, y, sequence: ++this.sequence };
    }
    if (!this.lease || this.lease.pointerId !== pointerId) return null;
    const edge = { ...this.lease, phase, x, y, sequence: ++this.sequence };
    this.lease = phase === "move" ? edge : null;
    return edge;
  }
  cancel(phase: "cancel" | "blur" = "cancel"): BagPointerEdge | null {
    if (!this.lease) return null;
    this.quarantine.set(this.lease.pointerId, this.lease);
    const edge = { ...this.lease, phase, sequence: ++this.sequence };
    this.lease = null;
    return edge;
  }
  matches(context: BagPointerContext | null): boolean {
    return !!this.lease && !!context && sameBagIdentity(this.lease, context)
      && this.lease.presentationRevision === context.presentationRevision;
  }
}
