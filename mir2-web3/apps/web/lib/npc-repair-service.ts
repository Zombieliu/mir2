import { projectEquipmentGatewaySnapshot } from "./equipment-gateway-adapter";
import type { NpcRepairQuote, NpcRepairQuoteInput } from "./client-presentation-runtime";

export type NpcRepairOwner = Readonly<{ socket: object; connectionGeneration: number; sessionGeneration: number;
  ownerRevision: number; playerObjectId: number; sceneRevision: number; mapFileName: string }>;
export type NpcRepairMode = "repair" | "special";
export type NpcRepairBagRow = Readonly<{ uniqueId: number | null; slot: number; name: string; icon: number;
  input: Omit<NpcRepairQuoteInput, "rate" | "special" | "gold"> | null }>;
export type NpcRepairRow = NpcRepairBagRow & Readonly<{ quote: NpcRepairQuote | null; disabled: boolean;
  reason: "unknown" | "gold" | "busy" | "locked" | null }>;
export type NpcRepairView = Readonly<{ stamp: object; source: object; mode: NpcRepairMode; name: string;
  gold: number; hold: boolean; rows: readonly NpcRepairRow[] }>;
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
  const layout = projectEquipmentGatewaySnapshot(raw);
  if (!layout || !record(raw) || !uint(raw.gold, 0xffffffff)) return null;
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
type Active = Readonly<{ owner: NpcRepairOwner; mode: NpcRepairMode; rate: number; source: object; name: string; epoch: number }>;
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
function snapshotSource(owner: NpcRepairOwner | null, raw: unknown): RawSource | null {
  if (!owner || !ownerValid(owner) || !record(raw) || raw.playerObjectId !== owner.playerObjectId || raw.mapFileName !== owner.mapFileName) return null;
  try {
    const snapshot = JSON.parse(JSON.stringify({ playerObjectId: raw.playerObjectId, mapFileName: raw.mapFileName,
      gold: raw.gold, inventoryCapacity: raw.inventoryCapacity, inventoryItems: raw.inventoryItems,
      beltItems: raw.beltItems, equipmentItems: raw.equipmentItems, storageItems: raw.storageItems, storageSize: raw.storageSize })) as Record<string, unknown>;
    const layout = projectEquipmentGatewaySnapshot(snapshot), bag = projectNpcRepairBag(snapshot);
    if (!layout || !bag) return null;
    const bagSources = (snapshot.inventoryItems as Record<string, unknown>[]).flatMap((item, index) => layout.placements[index].container !== 0 ? [] : [{
      uniqueId: layout.placements[index].uniqueId, slot: layout.placements[index].slot,
      name: item.name, icon: item.icon, quantity: item.quantity, currentDura: item.durabilityCurrent, maxDura: item.durabilityMax,
      tooltipSource: item.tooltipSource,
    }]);
    const key = JSON.stringify({ gold: snapshot.gold, layout, bagSources });
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
  open(owner: NpcRepairOwner, mode: NpcRepairMode, rate: unknown, source: object, name = ""): boolean {
    this.close();
    if (!ownerValid(owner) || (mode !== "repair" && mode !== "special") || typeof rate !== "number" || !Number.isFinite(rate)
      || rate < 0 || !Number.isFinite(Math.fround(rate)) || !source || !barrier(owner.socket) || this.epoch >= Number.MAX_SAFE_INTEGER) return false;
    this.active = Object.freeze({ owner: Object.freeze({ ...owner }), mode, rate, source, name, epoch: this.epoch }); return true;
  }
  close(): void { this.active = null; this.currentCapture = null; this.holdMode = null; if (this.epoch < Number.MAX_SAFE_INTEGER) ++this.epoch; }
  invalidateInventory(): void {
    this.raw = null; this.currentCapture = null;
    if (this.inventoryEpoch < Number.MAX_SAFE_INTEGER) ++this.inventoryEpoch; else this.close();
  }
  /** Changed/unknown input retires intents before any receive callbacks run. */
  prepareSnapshot(owner: NpcRepairOwner | null, raw: unknown): void {
    const source = snapshotSource(owner, raw);
    if (!source || !owner || this.active && !sameOwner(this.active.owner, owner)) this.holdMode = null;
    if (!source || !owner || !this.raw || !sameOwner(this.raw.owner, owner) || this.raw.key !== source.key) this.invalidateInventory();
  }
  observeSnapshot(owner: NpcRepairOwner | null, raw: unknown): boolean {
    const source = snapshotSource(owner, raw);
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
      hold: this.holdMode === active.mode, rows: Object.freeze(rows) });
    const capture = { active, key, view }; this.views.set(view, capture); this.currentCapture = capture; return view;
  }
  private current(capture: Capture, view: NpcRepairView | null): boolean {
    const live = view && this.views.get(view);
    return !!live && live === this.currentCapture && this.raw !== null && this.active === capture.active
      && live.active === capture.active && live.key === capture.key && live.view.stamp === capture.view.stamp;
  }
  select(view: NpcRepairView, uniqueId: number, current: NpcRepairView | null): NpcRepairSelection | null {
    const capture = this.views.get(view);
    if (!capture || !this.current(capture, current) || !view.rows.some(row => row.uniqueId === uniqueId && row.input !== null
      && row.quote !== null && (row.reason === null || row.reason === "gold"))) return null;
    const selection = Object.freeze({ stamp: view.stamp, uniqueId }); this.selections.set(selection, capture); return selection;
  }
  toggleHold(view: NpcRepairView, current: NpcRepairView | null): boolean | null {
    const capture = this.views.get(view);
    if (!capture || !this.current(capture, current)) return null;
    this.holdMode = this.holdMode === capture.active.mode ? null : capture.active.mode;
    return this.holdMode !== null;
  }
  beginDrag(view: NpcRepairView, uniqueId: number, slot: number, geometry: NpcRepairDragGeometry,
    current: NpcRepairView | null): NpcRepairDrag | null {
    const capture = this.views.get(view);
    if (!capture || !this.current(capture, current) || !geometryValid(geometry) || !uint(uniqueId) || !uint(slot, 79)
      || Math.floor(slot / 40) !== (geometry.page === "bag2" ? 1 : 0)
      || !view.rows.some(row => row.uniqueId === uniqueId && row.slot === slot && row.input !== null && row.quote !== null
        && (row.reason === null || row.reason === "gold"))) return null;
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
      || !current!.rows.some(row => row.uniqueId === drag.uniqueId && row.slot === drag.slot && row.input !== null && row.quote !== null
        && (row.reason === null || row.reason === "gold"))) return null;
    const selection = Object.freeze({ stamp: current!.stamp, uniqueId: drag.uniqueId });
    this.selections.set(selection, lease.capture); return selection;
  }
  reserve(selection: NpcRepairSelection, current: NpcRepairView | null): NpcRepairProof | null {
    const capture = this.selections.get(selection); this.selections.delete(selection);
    if (!capture || !this.current(capture, current) || !current!.rows.some(row => row.uniqueId === selection.uniqueId && !row.disabled)) return null;
    const command = Object.freeze({ type: capture.active.mode === "special" ? "specialRepairItem" as const : "repairItem" as const, uniqueId: selection.uniqueId });
    const proof = Object.freeze({ command }); this.proofs.set(proof, { capture, used: false }); return proof;
  }
  enter(proof: NpcRepairProof, current: NpcRepairView | null, wire: Record<string, unknown>): boolean {
    const lease = this.proofs.get(proof);
    if (!lease || lease.used) return false;
    lease.used = true;
    if (!this.current(lease.capture, current) || Object.keys(wire).sort().join() !== "type,uniqueId"
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
