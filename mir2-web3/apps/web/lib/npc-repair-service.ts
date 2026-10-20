import { projectEquipmentGatewaySnapshot } from "./equipment-gateway-adapter";
import type { NpcRepairQuote, NpcRepairQuoteInput } from "./client-presentation-runtime";
import type { HuntingRepairView } from "./hunting-repair-client";

export type NpcRepairOwner = Readonly<{ socket: object; connectionGeneration: number; sessionGeneration: number;
  ownerRevision: number; playerObjectId: number; sceneRevision: number; mapFileName: string }>;
export type NpcRepairMode = "repair" | "special";
export type NpcRepairBagRow = Readonly<{ uniqueId: number | null; slot: number; name: string; icon: number;
  input: Omit<NpcRepairQuoteInput, "rate" | "special" | "gold"> | null }>;
export type NpcRepairRow = NpcRepairBagRow & Readonly<{ quote: NpcRepairQuote | null; disabled: boolean;
  reason: "unknown" | "gold" | "busy" | "locked" | null }>;
export type NpcRepairView = Readonly<{ stamp: object; source: object; mode: NpcRepairMode; name: string;
  paymentMode: "legacy" | "hunting-v1"; gold: number | null; hold: boolean; rows: readonly NpcRepairRow[];
  hunting: HuntingRepairView | null; wallet: NpcRepairWallet | null; recoveryOnly: boolean }>;
export type NpcRepairWallet = Readonly<{ boundGold: string; unboundGold: string }>;
export type NpcRepairSelection = Readonly<{ stamp: object; uniqueId: number }>;
export type NpcRepairDragGeometry = Readonly<{ pointerId: number; pointerType: "mouse" | "touch"; page: "bag1" | "bag2";
  stage: object; target: object; item: object; stageRect: readonly number[]; targetRect: readonly number[]; itemRect: readonly number[];
  virtualWidth: number; virtualHeight: number; scale: number; devicePixelRatio: number }>;
export type NpcRepairDrag = Readonly<{ stamp: object; uniqueId: number; slot: number }>;
export type NpcRepairDrop = Readonly<{ selection: NpcRepairSelection; submitted: boolean }>;
export type NpcRepairProof = Readonly<{ command: Readonly<{ type: "repairItem" | "specialRepairItem"; uniqueId: number }> }>;
type QuoteReader = (input: NpcRepairQuoteInput) => NpcRepairQuote | null;
const record = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const uint = (v: unknown, max = Number.MAX_SAFE_INTEGER): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0 && v <= max;
function ownerValid(o: NpcRepairOwner): boolean {
  return typeof o.socket === "object" && o.socket !== null && uint(o.connectionGeneration) && o.connectionGeneration > 0
    && uint(o.sessionGeneration) && o.sessionGeneration > 0 && uint(o.ownerRevision) && uint(o.sceneRevision)
    && uint(o.playerObjectId, 0xffffffff) && o.playerObjectId > 0 && typeof o.mapFileName === "string" && o.mapFileName.length > 0;
}
function sameOwner(a: NpcRepairOwner, b: NpcRepairOwner): boolean {
  return a.socket === b.socket && a.connectionGeneration === b.connectionGeneration && a.sessionGeneration === b.sessionGeneration
    && a.ownerRevision === b.ownerRevision && a.playerObjectId === b.playerObjectId && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
}
function concreteSource(item: Record<string, unknown>, id: number): boolean {
  const source = item.tooltipSource;
  if (!record(source) || !record(source.info) || !record(source.userItem)) return false;
  const info = source.info, user = source.userItem;
  return user.unique_id === id && user.item_index === info.item_index && typeof info.item_index === "number"
    && Number.isSafeInteger(info.item_index) && info.item_index >= -2147483648 && info.item_index <= 2147483647
    && uint(info.price, 0xffffffff) && uint(info.durability, 65535)
    && user.count === item.quantity && user.current_dura === item.durabilityCurrent && user.max_dura === item.durabilityMax
    && Array.isArray(user.added_stats) && user.added_stats.length <= 256
    && user.added_stats.every(s => record(s) && Object.keys(s).sort().join() === "stat,value" && uint(s.stat, 255)
      && typeof s.value === "number" && Number.isSafeInteger(s.value) && s.value >= -2147483648 && s.value <= 2147483647)
    && Object.hasOwn(user, "rental_information") && (user.rental_information === null || record(user.rental_information));
}
function quoteValid(q: NpcRepairQuote | null): q is NpcRepairQuote {
  return q !== null && record(q) && Object.keys(q).sort().join() === "affordable,displayedTotal,repairPrice,totalPrice"
    && uint(q.repairPrice, 0xffffffff) && uint(q.totalPrice, 0xffffffff) && typeof q.displayedTotal === "number"
    && Number.isFinite(q.displayedTotal) && q.displayedTotal >= 0 && q.displayedTotal <= Math.fround(0xffffffff)
    && typeof q.affordable === "boolean";
}

/** Full raw placement authority only; incomplete concrete pricing stays unknown. */
export function projectNpcRepairBag(raw: unknown): readonly NpcRepairBagRow[] | null {
  return projectRepairBag(raw, false);
}
function projectRepairBag(raw: unknown, hunting: boolean): readonly NpcRepairBagRow[] | null {
  const layout = projectEquipmentGatewaySnapshot(raw);
  if (!layout || !record(raw) || !hunting && !uint(raw.gold, 0xffffffff)) return null;
  const ids = new Set<number>(), cells = new Set<string>();
  for (const p of layout.placements) {
    const key = `${p.container}:${p.slot}`;
    const capacity = p.container === 0 ? layout.capacity - 6 : p.container === 1 ? 6 : p.container === 2 ? 14 : p.container === 4 ? layout.storageCapacity : 40;
    if (capacity === undefined || p.slot >= capacity || cells.has(key) || p.uniqueId !== null && ids.has(p.uniqueId)) return null;
    cells.add(key); if (p.uniqueId !== null) ids.add(p.uniqueId);
  }
  const items = raw.inventoryItems as Record<string, unknown>[];
  return Object.freeze(items.flatMap((item, index) => {
    const placement = layout.placements[index];
    if (placement.container !== 0) return [];
    const input = placement.uniqueId !== null && uint(item.quantity, 65535) && item.quantity > 0
      && uint(item.durabilityCurrent, 65535) && uint(item.durabilityMax, 65535) && item.durabilityCurrent <= item.durabilityMax
      && concreteSource(item, placement.uniqueId)
      ? Object.freeze({ uniqueId: placement.uniqueId, tooltipSource: item.tooltipSource, count: item.quantity,
          currentDura: item.durabilityCurrent, maxDura: item.durabilityMax }) : null;
    return [Object.freeze({ uniqueId: placement.uniqueId, slot: placement.slot,
      name: typeof item.name === "string" ? item.name : "?", icon: uint(item.icon, 0xffffffff) ? item.icon : 0, input })];
  }));
}

// The physical transport owns entered requests. Service close, remount and HMR
// cannot erase them. An incompatible existing holder is never reset to empty.
type Barrier = Readonly<{ contract: "npcRepairEntered.v1"; ids: Set<number> }>;
function barrier(socket: object, create = true): Barrier | null {
  const key = Symbol.for("mir2.npcRepair.entered.v1"), descriptor = Object.getOwnPropertyDescriptor(socket, key);
  if (descriptor) {
    const value: unknown = "value" in descriptor ? descriptor.value : null;
    return descriptor.configurable === false && descriptor.writable === false && record(value) && Object.isFrozen(value)
      && Object.keys(value).sort().join() === "contract,ids" && value.contract === "npcRepairEntered.v1"
      && value.ids instanceof Set && [...value.ids].every(id => uint(id)) ? value as Barrier : null;
  }
  if (!create) return null;
  try {
    const value: Barrier = Object.freeze({ contract: "npcRepairEntered.v1", ids: new Set<number>() });
    Object.defineProperty(socket, key, { value, configurable: false, writable: false }); return value;
  } catch { return null; }
}
type Active = Readonly<{ owner: NpcRepairOwner; mode: NpcRepairMode; paymentMode: "legacy" | "hunting-v1";
  rate: number; source: object; name: string; epoch: number }>;
type Capture = Readonly<{ active: Active; key: string; view: NpcRepairView }>;
function geometryValid(g: NpcRepairDragGeometry): boolean {
  return record(g) && uint(g.pointerId) && (g.pointerType === "mouse" || g.pointerType === "touch") && (g.page === "bag1" || g.page === "bag2")
    && typeof g.stage === "object" && g.stage !== null && typeof g.target === "object" && g.target !== null
    && typeof g.item === "object" && g.item !== null && [g.stageRect, g.targetRect, g.itemRect].every(r => Array.isArray(r)
      && r.length === 4 && r.every(Number.isFinite) && r[2] > 0 && r[3] > 0)
    && [g.virtualWidth, g.virtualHeight, g.scale, g.devicePixelRatio].every(n => Number.isFinite(n) && n > 0);
}
function sameGeometry(a: NpcRepairDragGeometry, b: NpcRepairDragGeometry): boolean {
  return geometryValid(b) && a.pointerId === b.pointerId && a.pointerType === b.pointerType && a.page === b.page
    && a.stage === b.stage && a.target === b.target && a.item === b.item && a.virtualWidth === b.virtualWidth
    && a.virtualHeight === b.virtualHeight && a.scale === b.scale && a.devicePixelRatio === b.devicePixelRatio
    && ["stageRect", "targetRect", "itemRect"].every(k => a[k as "stageRect"].every((n, i) => n === b[k as "stageRect"][i]));
}
function freezeData(value: unknown): void {
  if (value && typeof value === "object") { for (const child of Object.values(value)) freezeData(child); Object.freeze(value); }
}
type RawSource = Readonly<{ snapshot: Record<string, unknown>; key: string }>;
/** Only the carried source graph participates; movement/chat/world clocks do not. */
function snapshotSource(owner: NpcRepairOwner | null, raw: unknown, hunting = false): RawSource | null {
  if (!owner || !ownerValid(owner) || !record(raw) || raw.playerObjectId !== owner.playerObjectId || raw.mapFileName !== owner.mapFileName) return null;
  try {
    const snapshot = JSON.parse(JSON.stringify({ playerObjectId: raw.playerObjectId, mapFileName: raw.mapFileName,
      gold: raw.gold, inventoryCapacity: raw.inventoryCapacity, inventoryItems: raw.inventoryItems,
      beltItems: raw.beltItems, equipmentItems: raw.equipmentItems, storageItems: raw.storageItems, storageSize: raw.storageSize })) as Record<string, unknown>;
    const layout = projectEquipmentGatewaySnapshot(snapshot), bag = projectRepairBag(snapshot, hunting);
    if (!layout || !bag) return null;
    const bagSources = (snapshot.inventoryItems as Record<string, unknown>[]).flatMap((item, index) => layout.placements[index].container !== 0 ? [] : [{
      uniqueId: layout.placements[index].uniqueId, slot: layout.placements[index].slot,
      name: item.name, icon: item.icon, quantity: item.quantity, currentDura: item.durabilityCurrent, maxDura: item.durabilityMax,
      tooltipSource: item.tooltipSource,
    }]);
    const key = JSON.stringify({ ...(hunting ? {} : { gold: snapshot.gold }), layout, bagSources });
    freezeData(snapshot); return { snapshot, key };
  } catch { return null; }
}
/** Source/proof lifetime adapter; Rust is the sole pricing policy. */
export class NpcRepairService {
  private epoch = 0;
  private inventoryEpoch = 0;
  private active: Active | null = null;
  private raw: (RawSource & { owner: NpcRepairOwner }) | null = null;
  private currentCapture: Capture | null = null;
  private views = new WeakMap<NpcRepairView, Capture>();
  private selections = new WeakMap<NpcRepairSelection, Capture>();
  private proofs = new WeakMap<NpcRepairProof, { capture: Capture; used: boolean }>();
  private drags = new WeakMap<NpcRepairDrag, { capture: Capture; geometry: NpcRepairDragGeometry; used: boolean }>();
  private holdMode: NpcRepairMode | null = null;
  private recoverySource = Object.freeze({});
  open(owner: NpcRepairOwner, mode: NpcRepairMode, rate: unknown, source: object, name = ""): boolean {
    this.close();
    if (!ownerValid(owner) || (mode !== "repair" && mode !== "special") || typeof rate !== "number" || !Number.isFinite(rate)
      || rate < 0 || !Number.isFinite(Math.fround(rate)) || !source || !barrier(owner.socket) || this.epoch >= Number.MAX_SAFE_INTEGER) return false;
    this.active = Object.freeze({ owner: Object.freeze({ ...owner }), mode, paymentMode: "legacy", rate, source, name, epoch: this.epoch }); return true;
  }
  /** Bound fees come exclusively from the owner protocol; no legacy rate is accepted. */
  openHunting(owner: NpcRepairOwner, mode: NpcRepairMode, source: object, name = ""): boolean {
    this.close();
    if (!ownerValid(owner) || (mode !== "repair" && mode !== "special") || !source || this.epoch >= Number.MAX_SAFE_INTEGER) return false;
    this.active = Object.freeze({ owner: Object.freeze({ ...owner }), mode, paymentMode: "hunting-v1", rate: 0, source, name, epoch: this.epoch });
    return true;
  }
  close(): void { this.active = null; this.currentCapture = null; this.holdMode = null; if (this.epoch < Number.MAX_SAFE_INTEGER) ++this.epoch; }
  invalidateInventory(): void {
    this.raw = null; this.currentCapture = null;
    if (this.inventoryEpoch < Number.MAX_SAFE_INTEGER) ++this.inventoryEpoch; else this.close();
  }
  /** Changed/unknown input retires intents before any receive callbacks run. */
  prepareSnapshot(owner: NpcRepairOwner | null, raw: unknown): void {
    const source = snapshotSource(owner, raw, this.active?.paymentMode === "hunting-v1");
    if (!source || !owner || this.active && !sameOwner(this.active.owner, owner)) this.holdMode = null;
    if (!source || !owner || !this.raw || !sameOwner(this.raw.owner, owner) || this.raw.key !== source.key) this.invalidateInventory();
  }
  observeSnapshot(owner: NpcRepairOwner | null, raw: unknown): boolean {
    const source = snapshotSource(owner, raw, this.active?.paymentMode === "hunting-v1");
    if (owner && this.active && !sameOwner(this.active.owner, owner)) this.holdMode = null;
    if (!source || !owner || this.inventoryEpoch >= Number.MAX_SAFE_INTEGER) { this.holdMode = null; this.invalidateInventory(); return false; }
    if (this.raw && sameOwner(this.raw.owner, owner) && this.raw.key === source.key) return true;
    // Equality can preserve a continuously known source only. An explicit
    // mutation/unknown interval cleared raw and already spent the old epoch.
    this.invalidateInventory();
    this.raw = { owner: Object.freeze({ ...owner }), ...source }; return true;
  }
  view(owner: NpcRepairOwner | null, quoteReader: QuoteReader, canMutate: (uniqueId: number) => boolean): NpcRepairView | null {
    const active = this.active, raw = this.raw;
    if (!active || !owner || !sameOwner(active.owner, owner)) { this.close(); return null; }
    if (active.paymentMode !== "legacy") return null;
    const pending = barrier(owner.socket);
    if (!pending || !raw || !sameOwner(raw.owner, owner)) return null;
    const bag = projectNpcRepairBag(raw.snapshot); if (!bag) return null;
    const gold = raw.snapshot.gold as number;
    const rows = bag.map(row => {
      let quote: NpcRepairQuote | null = null, allowed = false;
      try {
        if (row.input) quote = quoteReader({ ...row.input, rate: active.rate, special: active.mode === "special", gold });
        quote = quoteValid(quote) ? Object.freeze({ ...quote }) : null;
        if (row.uniqueId !== null) allowed = canMutate(row.uniqueId);
      } catch { quote = null; }
      const reason: NpcRepairRow["reason"] = !row.input || !quote ? "unknown" : pending.ids.has(row.uniqueId!) ? "busy"
        : !allowed ? "locked" : !quote.affordable ? "gold" : null;
      return Object.freeze({ ...row, quote, reason, disabled: reason !== null });
    });
    if (this.active !== active || this.raw !== raw) return null;
    const key = JSON.stringify({ epoch: active.epoch, inventoryEpoch: this.inventoryEpoch, rate: active.rate, gold, bag, rows });
    const stamp = this.currentCapture?.active === active && this.currentCapture.key === key ? this.currentCapture.view.stamp : Object.freeze({});
    const view: NpcRepairView = Object.freeze({ stamp, source: active.source, mode: active.mode, name: active.name, gold,
      paymentMode: "legacy", hold: this.holdMode === active.mode, rows: Object.freeze(rows), hunting: null, wallet: null, recoveryOnly: false });
    const capture = { active, key, view }; this.views.set(view, capture); this.currentCapture = capture; return view;
  }
  /** Transport/receipt/wallet updates never change the carried-item proof stamp. */
  viewHunting(owner: NpcRepairOwner | null, state: HuntingRepairView, canMutate: (uniqueId: number) => boolean,
    wallet: NpcRepairWallet | null = null): NpcRepairView | null {
    if (!owner || !ownerValid(owner)) return null;
    const money = (v: string) => typeof v === "string" && /^(0|[1-9][0-9]*)$/.test(v) && v.length <= 20 && BigInt(v) <= 18446744073709551615n;
    const observedWallet = wallet && money(wallet.boundGold) && money(wallet.unboundGold)
      ? Object.freeze({ boundGold: wallet.boundGold, unboundGold: wallet.unboundGold }) : null;
    const hunting = Object.freeze({ ...state, quote: state.quote && Object.freeze({ ...state.quote }), receipt: state.receipt && Object.freeze({ ...state.receipt }) });
    const active = this.active, raw = this.raw;
    // The original receipt may outlive the NPC/item source after Query applied
    // its full snapshot. Keep that visible result readonly until the owner
    // adapter resets it on ordinary Close; never recreate a source capture.
    const recovery = () => state.quoteId !== null && (state.phase === "pending"
      || state.phase === "committed" && state.visible && !state.reloadRequired && state.receipt?.quoteId === state.quoteId) ? Object.freeze({
      stamp: this.recoverySource, source: this.recoverySource, mode: state.quote?.special ? "special" as const : "repair" as const,
      name: "", paymentMode: "hunting-v1" as const, gold: null, hold: false, rows: Object.freeze([]), hunting,
      wallet: observedWallet, recoveryOnly: true }) : null;
    if (active?.paymentMode === "hunting-v1" && !sameOwner(active.owner, owner)) { this.close(); return recovery(); }
    if (!active || active.paymentMode !== "hunting-v1" || !raw || !sameOwner(raw.owner, owner)) return recovery();
    const bag = projectRepairBag(raw.snapshot, true); if (!bag) return recovery();
    const rows = bag.map(row => {
      let allowed = false; try { if (row.uniqueId !== null) allowed = canMutate(row.uniqueId); } catch { /* closed */ }
      const reason: NpcRepairRow["reason"] = !row.input ? "unknown" : !allowed ? "locked" : null;
      return Object.freeze({ ...row, quote: null, reason, disabled: reason !== null });
    });
    if (this.active !== active || this.raw !== raw) return null;
    const key = JSON.stringify({ epoch: active.epoch, inventoryEpoch: this.inventoryEpoch, bag });
    const stamp = this.currentCapture?.active === active && this.currentCapture.key === key ? this.currentCapture.view.stamp : Object.freeze({});
    const view: NpcRepairView = Object.freeze({ stamp, source: active.source, mode: active.mode, name: active.name,
      paymentMode: "hunting-v1", gold: null, hold: false, rows: Object.freeze(rows), hunting, wallet: observedWallet, recoveryOnly: false });
    const capture = { active, key, view }; this.views.set(view, capture); this.currentCapture = capture; return view;
  }
  /** Non-consuming full-source proof used by the owner adapter on Quote/Confirm. */
  huntingIntent(selection: NpcRepairSelection, current: NpcRepairView | null): Readonly<{ uniqueId: string; special: boolean }> | null {
    const capture = this.selections.get(selection);
    if (!capture || capture.active.paymentMode !== "hunting-v1" || !this.current(capture, current)
      || !uint(selection.uniqueId)
      || !current!.rows.some(row => row.uniqueId === selection.uniqueId && row.input !== null && !row.disabled)) return null;
    return Object.freeze({ uniqueId: String(selection.uniqueId), special: capture.active.mode === "special" });
  }
  private selectable(view: NpcRepairView, row: NpcRepairRow): boolean {
    return row.input !== null && (view.paymentMode === "hunting-v1"
      ? view.hunting?.phase !== "pending" && !view.recoveryOnly && row.reason === null
      : row.quote !== null && (row.reason === null || row.reason === "gold"));
  }
  private current(capture: Capture, view: NpcRepairView | null): boolean {
    const live = view && this.views.get(view);
    return !!live && live === this.currentCapture && this.raw !== null && this.active === capture.active
      && live.active === capture.active && live.key === capture.key && live.view.stamp === capture.view.stamp;
  }
  select(view: NpcRepairView, uniqueId: number, current: NpcRepairView | null): NpcRepairSelection | null {
    const capture = this.views.get(view);
    if (!capture || !this.current(capture, current) || !current!.rows.some(row => row.uniqueId === uniqueId && this.selectable(current!, row))) return null;
    const selection = Object.freeze({ stamp: view.stamp, uniqueId }); this.selections.set(selection, capture); return selection;
  }
  toggleHold(view: NpcRepairView, current: NpcRepairView | null): boolean | null {
    const capture = this.views.get(view);
    if (!capture || capture.active.paymentMode !== "legacy" || !this.current(capture, current)) return null;
    this.holdMode = this.holdMode === capture.active.mode ? null : capture.active.mode;
    return this.holdMode !== null;
  }
  beginDrag(view: NpcRepairView, uniqueId: number, slot: number, geometry: NpcRepairDragGeometry,
    current: NpcRepairView | null): NpcRepairDrag | null {
    const capture = this.views.get(view);
    if (!capture || !this.current(capture, current) || !geometryValid(geometry) || !uint(uniqueId) || !uint(slot, 79)
      || Math.floor(slot / 40) !== (geometry.page === "bag2" ? 1 : 0)
      || !current!.rows.some(row => row.uniqueId === uniqueId && row.slot === slot && this.selectable(current!, row))) return null;
    const drag = Object.freeze({ stamp: view.stamp, uniqueId, slot });
    const detached = Object.freeze({ ...geometry, stageRect: Object.freeze([...geometry.stageRect]),
      targetRect: Object.freeze([...geometry.targetRect]), itemRect: Object.freeze([...geometry.itemRect]) });
    this.drags.set(drag, { capture, geometry: detached, used: false }); return drag;
  }
  cancelDrag(drag: NpcRepairDrag): void { const lease = this.drags.get(drag); if (lease) lease.used = true; }
  drop(drag: NpcRepairDrag, geometry: NpcRepairDragGeometry, x: number, y: number,
    current: NpcRepairView | null): NpcRepairSelection | null {
    const lease = this.drags.get(drag);
    if (!lease || lease.used) return null;
    lease.used = true; // Spend the terminal before validation or caller callbacks.
    const r = lease.geometry.targetRect;
    if (!this.current(lease.capture, current) || !sameGeometry(lease.geometry, geometry) || !Number.isFinite(x) || !Number.isFinite(y)
      || x < r[0] || y < r[1] || x >= r[0] + r[2] || y >= r[1] + r[3]
      || !current!.rows.some(row => row.uniqueId === drag.uniqueId && row.slot === drag.slot && this.selectable(current!, row))) return null;
    const selection = Object.freeze({ stamp: current!.stamp, uniqueId: drag.uniqueId });
    this.selections.set(selection, lease.capture); return selection;
  }
  reserve(selection: NpcRepairSelection, current: NpcRepairView | null): NpcRepairProof | null {
    const capture = this.selections.get(selection);
    if (capture?.active.paymentMode === "hunting-v1") return null;
    this.selections.delete(selection);
    if (!capture || capture.active.paymentMode !== "legacy" || !this.current(capture, current) || !current!.rows.some(row => row.uniqueId === selection.uniqueId && !row.disabled)) return null;
    const command = Object.freeze({ type: capture.active.mode === "special" ? "specialRepairItem" as const : "repairItem" as const, uniqueId: selection.uniqueId });
    const proof = Object.freeze({ command }); this.proofs.set(proof, { capture, used: false }); return proof;
  }
  enter(proof: NpcRepairProof, current: NpcRepairView | null, wire: Record<string, unknown>): boolean {
    const lease = this.proofs.get(proof);
    if (!lease || lease.used) return false;
    lease.used = true;
    if (lease.capture.active.paymentMode !== "legacy" || !this.current(lease.capture, current) || Object.keys(wire).sort().join() !== "type,uniqueId"
      || wire.type !== proof.command.type || wire.uniqueId !== proof.command.uniqueId
      || !current!.rows.some(row => row.uniqueId === proof.command.uniqueId && !row.disabled)) return false;
    const pending = barrier(lease.capture.active.owner.socket);
    if (!pending || pending.ids.has(proof.command.uniqueId)) return false;
    pending.ids.add(proof.command.uniqueId); return true;
  }
  acknowledge(socket: object, packet: string, payload: unknown): boolean {
    if (packet !== "ItemRepaired" || !record(payload) || Object.keys(payload).sort().join() !== "currentDura,maxDura,uniqueId"
      || !uint(payload.uniqueId) || !uint(payload.currentDura, 65535) || !uint(payload.maxDura, 65535)
      || payload.currentDura > payload.maxDura) return false;
    return barrier(socket, false)?.ids.delete(payload.uniqueId) === true;
  }
}
