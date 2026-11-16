import { sameBagOwner, type BagOwnerToken, type BagCommandToken, type BagInputRegion } from "./bevy-bag-ui";
import type { CharacterModel } from "./bevy-character-model";
import type { HudPlayer } from "./bevy-hud-ui";
import type { BevyQuestUiPresentation } from "./bevy-quest-ui";

export type CharacterIdentity = { runGeneration: number; connectionGeneration: number; sessionGeneration: number; ownerRevision: number;
  ledgerRunGeneration: number; hudGeneration: number };
export type CharacterSource = { container: 2; slot: number; uniqueId: number };
export type CharacterIntent = CharacterIdentity & { intentSequence: number; modelRevision: number; presentationRevision: number;
  type: "removeEquipment"; source: CharacterSource };
export type CharacterSnapshot = CharacterIdentity & { revision: number; modelRevision: number; presentationRevision: number;
  open: boolean; inputEnabled: boolean; presentation: BevyQuestUiPresentation | null; player: HudPlayer;
  model: CharacterModel; blockedUniqueIds: number[]; metadataVersion: number | null };
export type CharacterStatus = CharacterIdentity & { version: 1; frame: number; ready: boolean; inputEnabled: boolean;
  appliedRevision: number; appliedModelRevision: number; appliedPresentationRevision: number; inputRegions: BagInputRegion[]; error: string | null };
export type CharacterCommandProof = BagCommandToken & { surface: "character"; characterRunGeneration: number; hudGeneration: number;
  modelRevision: number; presentationRevision: number; source: CharacterSource };
export type EquipmentOwnerProof = BagCommandToken | CharacterCommandProof;
export type CharacterPointerContext = CharacterIdentity & { modelRevision: number; presentationRevision: number; inputRegions: BagInputRegion[];
  presentation: BevyQuestUiPresentation };
export type CharacterPointerEdge = CharacterIdentity & { sequence: number; modelRevision: number; presentationRevision: number;
  pointerId: number; phase: "down" | "move" | "up" | "cancel"; x: number; y: number; button: 0 | 2 };
export type CharacterRuntime = { getMir2CharacterUiCapabilities?: () => string; setMir2CharacterUiSnapshot?: (json: string) => boolean;
  getMir2CharacterUiStatus?: () => string; setMir2CharacterUiIntentSink?: (sink: (json: string) => string) => void;
  clearMir2CharacterUiIntentSink?: () => void; setMir2CharacterUiPointerEdge?: (json: string) => boolean };
export type CharacterInput = { owner: BagOwnerToken; hudGeneration: number; open: boolean; eligible: boolean;
  model: CharacterModel | null; player: HudPlayer | null; blockedUniqueIds: number[]; presentation: BevyQuestUiPresentation | null; metadataVersion: number | null };
const integer = (n: unknown): n is number => Number.isSafeInteger(n) && Number(n) >= 0;
const record = (v: unknown): v is Record<string, unknown> => Boolean(v) && typeof v === "object" && !Array.isArray(v);
const identityKeys = ["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision", "ledgerRunGeneration", "hudGeneration"] as const;
function sameIdentity(a: CharacterIdentity, b: CharacterIdentity) { return identityKeys.every(key => a[key] === b[key]); }
function validIdentity(v: Record<string, unknown>) { return identityKeys.every(key => integer(v[key]))
  && [v.runGeneration, v.connectionGeneration, v.sessionGeneration, v.hudGeneration].every(n => Number(n) > 0); }
export function parseCharacterIntent(json: string): CharacterIntent | null {
  try { const v: unknown = JSON.parse(json);
    if (!record(v) || !validIdentity(v) || v.type !== "removeEquipment" || !integer(v.intentSequence) || v.intentSequence === 0
      || !integer(v.modelRevision) || !integer(v.presentationRevision) || !record(v.source)
      || Object.keys(v.source).sort().join() !== "container,slot,uniqueId" || v.source.container !== 2
      || !integer(v.source.slot) || v.source.slot >= 14 || !integer(v.source.uniqueId)) return null;
    if (Object.keys(v).sort().join() !== [...identityKeys, "intentSequence", "modelRevision", "presentationRevision", "type", "source"].sort().join()) return null;
    return v as CharacterIntent;
  } catch { return null; }
}
export function supportsCharacter(runtime: CharacterRuntime | null): boolean {
  try { const v = JSON.parse(runtime?.getMir2CharacterUiCapabilities?.() ?? "null");
    return Boolean(v && Object.keys(v).sort().join() === "characterEquipmentIntentAbiVersion,characterPageAbiVersion,compiled,schemaVersion,startup"
      && v.schemaVersion === 1 && v.characterPageAbiVersion === 1 && v.characterEquipmentIntentAbiVersion === 1
      && v.compiled === true && v.startup === true
      && [runtime?.setMir2CharacterUiSnapshot, runtime?.getMir2CharacterUiStatus, runtime?.setMir2CharacterUiIntentSink,
        runtime?.clearMir2CharacterUiIntentSink, runtime?.setMir2CharacterUiPointerEdge].every(fn => typeof fn === "function"));
  } catch { return false; }
}
export function readCharacterStatus(runtime: CharacterRuntime): CharacterStatus | null {
  try { const v: unknown = JSON.parse(runtime.getMir2CharacterUiStatus?.() ?? "null");
    if (!record(v) || !validIdentity(v) || v.version !== 1 || typeof v.ready !== "boolean" || typeof v.inputEnabled !== "boolean"
      || ![v.frame, v.appliedRevision, v.appliedModelRevision, v.appliedPresentationRevision].every(integer)
      || !(v.error === null || typeof v.error === "string") || !Array.isArray(v.inputRegions)
      || !v.inputRegions.every(r => record(r) && [r.left, r.top, r.width, r.height].every(n => typeof n === "number" && Number.isFinite(n))
        && Number(r.width) > 0 && Number(r.height) > 0)) return null;
    return v as CharacterStatus;
  } catch { return null; }
}
/** Fixed authored cells require actual CSS fit and 44px touch targets. Smaller stages stay compatible. */
export function fitsCharacter(p: BevyQuestUiPresentation | null): boolean {
  if (!p || ![p.logicalWidth,p.logicalHeight,p.stageCssScale].every(Number.isFinite) || p.stageCssScale <= 0 || p.stageCssScale > 16
    || p.logicalWidth < 1024 || p.logicalWidth > 16384 || p.logicalHeight < 768 || p.logicalHeight > 16384) return false;
  return !p.touch || p.stageCssScale * 32 >= 44 && p.stageCssScale * 10 >= 14
    && 760 + 264 <= p.logicalWidth && 380 <= p.logicalHeight;
}
function nextRun() { const s = globalThis as typeof globalThis & { __mir2CharacterRun?: number };
  const n = (s.__mir2CharacterRun ?? 0) + 1; if (!integer(n)) throw Error("Character generation exhausted"); return s.__mir2CharacterRun = n; }
function runtimeOwners() {
  const s = globalThis as typeof globalThis & { __mir2CharacterRuntimeOwners?: WeakMap<object, number> };
  return s.__mir2CharacterRuntimeOwners ??= new WeakMap<object, number>();
}
type Options = { runtime: CharacterRuntime; isCurrent?: () => boolean; read: () => CharacterInput; now: () => number; onState: (ready: boolean) => void;
  onIntent: (intent: CharacterIntent, proof: CharacterCommandProof) => boolean };
/** Renderer mailbox; the page's equipment controller remains the only pending ledger. */
export class CharacterHost {
  private run = nextRun(); private revision = 0; private modelRevision = 0; private presentationRevision = 0;
  private modelKey = ""; private presentationKey = ""; private sent: CharacterSnapshot | null = null;
  private owner: BagOwnerToken | null = null; private active = false; private sequence = 0;
  private frame = -1; private frameAt = 0; private stopped = false;
  constructor(private options: Options) {
    try { if (supportsCharacter(options.runtime)) {
      runtimeOwners().set(options.runtime, this.run);
      options.runtime.setMir2CharacterUiIntentSink?.(json => {
      const intent = parseCharacterIntent(json);
      if (!intent || !this.qualifies() || !this.sent || !sameIdentity(intent, this.sent) || intent.modelRevision !== this.sent.modelRevision
        || intent.presentationRevision !== this.sent.presentationRevision || intent.intentSequence <= this.sequence || !this.owner) return JSON.stringify({ accepted: false });
      this.sequence = intent.intentSequence;
      const proof: CharacterCommandProof = { ...this.owner, surface: "character", characterRunGeneration: this.run, hudGeneration: intent.hudGeneration,
        modelRevision: intent.modelRevision, presentationRevision: intent.presentationRevision, source: intent.source };
      return JSON.stringify({ accepted: this.allows(proof) && options.onIntent(intent, proof) });
    }); } } catch { this.stop(); }
  }
  private ownsRuntime() { return runtimeOwners().get(this.options.runtime) === this.run; }
  private qualifyStatus(status: CharacterStatus | null) {
    const s = this.sent;
    return Boolean(s && status && status.ready && status.inputEnabled && sameIdentity(s, status) && status.appliedRevision === s.revision
      && status.appliedModelRevision === s.modelRevision && status.appliedPresentationRevision === s.presentationRevision && status.error === null);
  }
  private qualifies(ownReservation?: number) {
    if (!this.ownsRuntime() || this.options.isCurrent?.() === false) { this.withdraw(); return false; }
    if (this.stopped || !this.sent || !this.owner || !this.active) return false;
    const live = this.options.read();
    const expected = JSON.parse(this.modelKey) as [unknown, unknown, number[], unknown];
    expected[2] = expected[2].filter(n => n !== ownReservation);
    if (!live.eligible || !live.open || !live.model || !live.player || !fitsCharacter(live.presentation) || !sameBagOwner(this.owner, live.owner)
      || live.hudGeneration !== this.sent.hudGeneration || JSON.stringify([live.model, live.player, live.blockedUniqueIds.filter(n => n !== ownReservation), live.metadataVersion]) !== JSON.stringify(expected)
      || JSON.stringify([live.owner, live.hudGeneration, live.presentation, live.open]) !== this.presentationKey) return false;
    return this.options.now() - this.frameAt <= 500 && this.qualifyStatus(readCharacterStatus(this.options.runtime));
  }
  tick() {
    if (this.stopped) return;
    try {
      if (!this.ownsRuntime() || this.options.isCurrent?.() === false) { this.stop(); return; }
      const input = this.options.read();
      if (!supportsCharacter(this.options.runtime) || !input.eligible || !input.open || input.owner.owner === "transition"
        || !input.model || !input.player || !fitsCharacter(input.presentation)) { this.withdraw(); return; }
      const modelKey = JSON.stringify([input.model, input.player, input.blockedUniqueIds, input.metadataVersion]);
      const presentationKey = JSON.stringify([input.owner, input.hudGeneration, input.presentation, input.open]);
      if (modelKey !== this.modelKey || presentationKey !== this.presentationKey || !this.sent) {
        this.active = false; this.options.onState(false);
        if (modelKey !== this.modelKey) ++this.modelRevision;
        if (presentationKey !== this.presentationKey) ++this.presentationRevision;
        this.modelKey = modelKey; this.presentationKey = presentationKey; this.owner = { ...input.owner };
        const s: CharacterSnapshot = { runGeneration: this.run, connectionGeneration: input.owner.connectionGeneration,
          sessionGeneration: input.owner.sessionGeneration, ownerRevision: input.owner.ownerRevision, ledgerRunGeneration: input.owner.runGeneration,
          hudGeneration: input.hudGeneration, revision: ++this.revision, modelRevision: this.modelRevision, presentationRevision: this.presentationRevision,
          open: true, inputEnabled: true, presentation: input.presentation, player: input.player, model: input.model,
          blockedUniqueIds: input.blockedUniqueIds, metadataVersion: input.metadataVersion };
        this.sent = s;
        if (!this.options.runtime.setMir2CharacterUiSnapshot?.(JSON.stringify(s))) { this.withdraw(); return; }
      }
      const status = readCharacterStatus(this.options.runtime);
      if (status && status.frame !== this.frame) { this.frame = status.frame; this.frameAt = this.options.now(); }
      this.active = this.qualifyStatus(status) && this.options.now() - this.frameAt <= 500;
      this.options.onState(this.active);
    } catch { this.withdraw(); }
  }
  allows(proof: CharacterCommandProof, ownReservation?: number) {
    const s = this.sent, item = s?.model.items.find(i => i.container === 2 && i.slot === proof.source.slot && i.uniqueId === proof.source.uniqueId);
    return Boolean(this.qualifies(ownReservation === proof.source.uniqueId ? ownReservation : undefined) && s && this.owner && sameBagOwner(proof, this.owner) && proof.characterRunGeneration === this.run
      && proof.hudGeneration === s.hudGeneration && proof.modelRevision === s.modelRevision && proof.presentationRevision === s.presentationRevision
      && item && !s.blockedUniqueIds.includes(proof.source.uniqueId));
  }
  pointerContext(): CharacterPointerContext | null {
    if (!this.qualifies() || !this.sent?.presentation) return null;
    const status = readCharacterStatus(this.options.runtime)!;
    const s = this.sent;
    return { runGeneration:s.runGeneration, connectionGeneration:s.connectionGeneration, sessionGeneration:s.sessionGeneration,
      ownerRevision:s.ownerRevision, ledgerRunGeneration:s.ledgerRunGeneration, hudGeneration:s.hudGeneration,
      modelRevision:s.modelRevision, presentationRevision:s.presentationRevision,
      presentation:s.presentation!, inputRegions:status.inputRegions };
  }
  pointer(edge: CharacterPointerEdge) {
    try { if (edge.phase !== "cancel" && !this.qualifies()) return false;
      return this.options.runtime.setMir2CharacterUiPointerEdge?.(JSON.stringify(edge)) === true;
    } catch { this.withdraw(); return false; }
  }
  withdraw() {
    this.active = false; this.options.onState(false);
    const s = this.sent; this.sent = null; this.owner = null;
    if (s && this.ownsRuntime()) try { this.options.runtime.setMir2CharacterUiSnapshot?.(JSON.stringify({ ...s, revision: ++this.revision, open: false, inputEnabled: false })); } catch { /* Retire the captured runtime even when it rejects withdrawal. */ }
  }
  stop() { this.withdraw(); this.stopped = true; if (this.ownsRuntime()) {
    runtimeOwners().delete(this.options.runtime);
    try { this.options.runtime.clearMir2CharacterUiIntentSink?.(); } catch { /* Already retired. */ }
  } }
}
export class CharacterPointerRouter {
  held: { context: CharacterPointerContext; pointerId: number; button: 0 | 2; invalid: boolean } | null = null;
  private sequence = 0;
  matches(c: CharacterPointerContext | null) { const h = this.held; return Boolean(h && c && sameIdentity(h.context, c)
    && h.context.modelRevision === c.modelRevision && h.context.presentationRevision === c.presentationRevision); }
  contains(c: CharacterPointerContext, x: number, y: number) { return c.inputRegions.some(r => x >= r.left && y >= r.top && x < r.left + r.width && y < r.top + r.height); }
  down(c: CharacterPointerContext, pointerId: number, button: 0 | 2, x: number, y: number) {
    if (this.held || !this.contains(c, x, y)) return null;
    this.held = { context: c, pointerId, button, invalid: false }; return this.edge("down", pointerId, x, y);
  }
  edge(phase: CharacterPointerEdge["phase"], pointerId: number, x: number, y: number): CharacterPointerEdge | null {
    const h = this.held; if (!h || h.pointerId !== pointerId) return null;
    if (!this.contains(h.context, x, y)) h.invalid = true;
    const c=h.context;
    const wire: CharacterPointerEdge = { runGeneration:c.runGeneration,connectionGeneration:c.connectionGeneration,sessionGeneration:c.sessionGeneration,
      ownerRevision:c.ownerRevision,ledgerRunGeneration:c.ledgerRunGeneration,hudGeneration:c.hudGeneration,
      modelRevision:c.modelRevision,presentationRevision:c.presentationRevision,sequence:++this.sequence,pointerId,
      phase:h.invalid ? "cancel" : phase,x,y,button:h.button };
    if (phase === "up" || phase === "cancel" || h.invalid) this.held = null;
    return wire;
  }
  cancel() { const h = this.held; return h ? this.edge("cancel", h.pointerId, 0, 0) : null; }
}
