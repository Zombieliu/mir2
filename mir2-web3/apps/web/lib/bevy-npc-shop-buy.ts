import type { BevyInventoryModel } from "./bevy-bag-model";
import type { NpcGoldBuyAttemptSlotRuntime, NpcGoldBuyAttemptState, NpcGoldBuyTransportTicket } from "./client-core-runtime";

export type NpcGoldBuyGood = {
  unique_id: number; name: string; price: number; use_pearls: boolean;
  count: number; stock: number; panel_type: number; icon: number;
  icon_width: number; icon_height: number; description: string;
  tooltip_source: (Record<string, unknown> & { info: Record<string, unknown> }) | null;
  purchase_rate: number | null; requires_gold_buy_plan: boolean;
};
export type NpcGoldBuyShop = {
  goods: NpcGoldBuyGood[]; selected_id: number | null; hide_added_stats: boolean;
  selected_bag_slot_for_sell: null; selected_bag_slot_for_repair: null;
  service_mode: "buy"; supports_buy: boolean; supports_sell: boolean; repair_rate: null;
};
export type NpcGoldBuyOwner = {
  connectionGeneration: number; sessionGeneration: number; ownerRevision: number; playerObjectId: number;
};
/** JSON-only local input/layout context; no renderer measurement is required. */
export type NpcGoldBuyPresentation = Readonly<Record<string, unknown>> | null;
export type NpcGoldBuyCurrent = {
  owner: NpcGoldBuyOwner; serviceRevision: number; catalogRevision: number;
  presentation: NpcGoldBuyPresentation;
  shop: NpcGoldBuyShop; inventory: BevyInventoryModel; blocked: boolean;
};
export type NpcGoldBuyRuntime = { getMir2NpcGoldBuyPlan?: (json: string) => string };
export type NpcGoldBuyWire = Readonly<{
  type: "buyItem"; itemIndex: number; count: number; panelType: 0;
}>;
export type NpcGoldBuyQuote = Readonly<{
  maxQuantity: number; totalGold: number | null; canBuy: boolean;
  blockReason: string | null; command: NpcGoldBuyWire | null;
}>;
declare const npcGoldBuyProof: unique symbol;
/** A local token; only the issuing dispatcher's WeakMap can recognize it. */
export type NpcGoldBuyProof = Readonly<{ [npcGoldBuyProof]: true }>;
export type PreparedNpcGoldBuy = Readonly<{
  proof: NpcGoldBuyProof; quote: NpcGoldBuyQuote; wire: NpcGoldBuyWire;
}>;
export type NpcGoldBuyDispatcherOptions = {
  runtime: NpcGoldBuyRuntime; read: () => NpcGoldBuyCurrent | null;
  getAttemptSlot: () => NpcGoldBuyAttemptSlotRuntime | null;
  readSocket: () => { socket: object; isOpen: boolean } | null;
};
const record = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const safe = (v: unknown, max = Number.MAX_SAFE_INTEGER): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0 && v <= max;
const keys = (v: Record<string, unknown>, names: readonly string[]) =>
  Object.keys(v).sort().join("|") === [...names].sort().join("|");

// JSON transport only. Refuse values JSON would silently coerce/drop, callbacks,
// accessors and toJSON hooks. Optional fields should be absent, not undefined.
function jsonText(value: unknown): string {
  const stack = new Set<object>();
  function visit(v: unknown): void {
    if (v === null || typeof v === "string" || typeof v === "boolean") return;
    if (typeof v === "number" && Number.isFinite(v)) return;
    if (typeof v !== "object" || v === null) throw new Error("Non-JSON buy snapshot");
    const prototype = Object.getPrototypeOf(v);
    if (prototype !== Object.prototype && prototype !== null && prototype !== Array.prototype
      || stack.has(v) || "toJSON" in v || Object.getOwnPropertySymbols(v).length !== 0) {
      throw new Error("Non-plain buy snapshot");
    }
    stack.add(v);
    const descriptors = Object.getOwnPropertyDescriptors(v);
    for (const [name, descriptor] of Object.entries(descriptors)) {
      if (Array.isArray(v) && name === "length") continue;
      if (Array.isArray(v) && (!/^(0|[1-9][0-9]*)$/.test(name) || Number(name) >= v.length)) throw new Error("Non-index buy array");
      if (!descriptor.enumerable || !("value" in descriptor)) throw new Error("Non-data buy snapshot");
      visit(descriptor.value);
    }
    if (Array.isArray(v) && Object.keys(v).length !== v.length) throw new Error("Sparse buy snapshot");
    stack.delete(v);
  }
  visit(value);
  const json = JSON.stringify(value);
  if (typeof json !== "string") throw new Error("Missing JSON buy snapshot");
  return json;
}

const npcGoldBuyInventoryPackets = new Set([
  "GainedGold", "LoseGold", "UserInformation", "GainedItem", "DeleteItem", "DeleteItems",
  "ItemChanged", "ItemDurability", "MoveItem", "MergeItem", "SplitItem", "SplitItem1",
  "DropItem", "SellItem", "EquipItem", "RemoveItem", "StoreItem", "StoreItemV2",
  "TakeBackItem", "TakeBackItemV2", "ResizeInventory", "NewItem", "UserSlotsRefresh",
  "UseItem", "RemoveSlotItem", "EquipSlotItem", "DepositRefineItem", "RetrieveRefineItem",
  "DepositTradeItem", "RetrieveTradeItem", "TakeBackHeroItem", "TransferHeroItem",
  "CombineItem", "ItemUpgraded", "RefreshItem", "DuraChanged", "ItemRepaired",
  "ItemSlotSizeChanged", "ItemSealChanged", "DepositRentalItem", "RetrieveRentalItem",
  "UpdateRentalItem", "ConfirmItemRental", "MailLockedItem", "AwakeningLockedItem", "Awakening",
]);

export function npcGoldBuyInventoryMutationPacket(packet: string): boolean {
  return npcGoldBuyInventoryPackets.has(packet);
}

type InventoryOwner = Pick<NpcGoldBuyOwner, "connectionGeneration" | "sessionGeneration" | "playerObjectId">;
type InventoryStage = Readonly<{ epoch: number; owner: string | null }>;
/** Synchronous availability only; this epoch never enters the Core authority. */
export class NpcGoldBuyInventoryReadiness {
  private epoch = 0;
  private readyOwner: string | null = null;
  private readyInventory: string | null = null;
  private ownerKey(owner: InventoryOwner | null): string | null {
    return owner && safe(owner.connectionGeneration) && owner.connectionGeneration > 0
      && safe(owner.sessionGeneration) && owner.sessionGeneration > 0
      && safe(owner.playerObjectId, 0xffff_ffff) && owner.playerObjectId > 0
      ? owner.connectionGeneration + ":" + owner.sessionGeneration + ":" + owner.playerObjectId : null;
  }
  invalidate(): void {
    this.readyOwner = null;
    this.readyInventory = null;
    if (this.epoch < Number.MAX_SAFE_INTEGER) ++this.epoch;
  }
  begin(owner: InventoryOwner | null): InventoryStage {
    this.invalidate();
    return Object.freeze({ epoch: this.epoch, owner: this.ownerKey(owner) });
  }
  finish(stage: InventoryStage, currentOwner: InventoryOwner | null, complete: boolean, inventory: BevyInventoryModel | null): boolean {
    const key = this.ownerKey(currentOwner);
    if (!inventory) return false;
    if (!complete || this.epoch >= Number.MAX_SAFE_INTEGER || stage.epoch !== this.epoch
      || stage.owner === null || stage.owner !== key) return false;
    const inventoryJson = jsonText(inventory);
    this.readyOwner = key;
    this.readyInventory = inventoryJson;
    return true;
  }
  matches(owner: InventoryOwner | null, inventory: BevyInventoryModel): boolean {
    const key = this.ownerKey(owner);
    return key !== null && key === this.readyOwner && jsonText(inventory) === this.readyInventory;
  }
}

type SocketBinding = Readonly<{ socket: object; transport: string }>;
type Capture = { requestJson: string; contextJson: string; authorityJson: string; selectedId: number | null; quantity: number; blocked: boolean };
type LiveCapture = Capture & SocketBinding;
function capture(current: NpcGoldBuyCurrent | null, quantity: number): Capture | null {
  try {
    if (!current || !safe(quantity, 65_535)) return null;
    jsonText(current);
    const owner = current.owner;
    if (!record(owner) || !keys(owner, ["connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId"])
      || !safe(owner.connectionGeneration) || owner.connectionGeneration === 0
      || !safe(owner.sessionGeneration) || owner.sessionGeneration === 0 || !safe(owner.ownerRevision)
      || !safe(owner.playerObjectId) || owner.playerObjectId === 0
      || !safe(current.serviceRevision) || current.serviceRevision === 0
      || !safe(current.catalogRevision) || current.catalogRevision === 0 || typeof current.blocked !== "boolean") return null;
    const requestJson = jsonText({ shop: current.shop, inventory: current.inventory, quantity });
    // This detached request is also the authority for validating the returned UID.
    const request = JSON.parse(requestJson) as { shop: NpcGoldBuyShop; inventory: BevyInventoryModel };
    // Availability evidence participates in every live proof, but cannot release
    // an Entered/Unknown flight when only that evidence is withdrawn or refreshed.
    const inventoryAuthority = { ...request.inventory };
    delete inventoryAuthority.npcGoldTradeCapacity;
    return { requestJson, contextJson: jsonText({ owner, serviceRevision: current.serviceRevision,
      catalogRevision: current.catalogRevision, presentation: current.presentation }),
    authorityJson: jsonText({ owner: { connectionGeneration: owner.connectionGeneration, sessionGeneration: owner.sessionGeneration, playerObjectId: owner.playerObjectId },
      serviceRevision: current.serviceRevision, catalogRevision: current.catalogRevision,
      shop: { ...request.shop, selected_id: null, selected_bag_slot_for_sell: null, selected_bag_slot_for_repair: null },
      inventory: inventoryAuthority }),
    selectedId: request.shop.selected_id, quantity, blocked: current.blocked };
  } catch { return null; }
}
function sameCapture(a: Capture | null, b: Capture): boolean {
  return a !== null && !a.blocked && !b.blocked && a.requestJson === b.requestJson && a.contextJson === b.contextJson;
}
function wireValue(value: unknown): NpcGoldBuyWire | null {
  try {
    jsonText(value);
    if (!record(value) || !keys(value, ["type", "itemIndex", "count", "panelType"])
      || value.type !== "buyItem" || !safe(value.itemIndex) || !safe(value.count, 65_535)
      || value.count === 0 || value.panelType !== 0) return null;
    return Object.freeze({ type: "buyItem", itemIndex: value.itemIndex, count: value.count, panelType: 0 });
  } catch { return null; }
}
function sameWire(value: unknown, expected: NpcGoldBuyWire): boolean {
  const wire = wireValue(value);
  return wire !== null && wire.itemIndex === expected.itemIndex && wire.count === expected.count;
}
function parseQuote(json: unknown, input: Capture): NpcGoldBuyQuote | null {
  try {
    if (typeof json !== "string") return null;
    const v: unknown = JSON.parse(json);
    if (!record(v) || !keys(v, ["maxQuantity", "totalGold", "canBuy", "blockReason", "command"])
      || !safe(v.maxQuantity, 65_535) || !(v.totalGold === null || safe(v.totalGold, 0xffff_ffff))
      || typeof v.canBuy !== "boolean" || !(v.blockReason === null || typeof v.blockReason === "string")) return null;
    const command = v.command === null ? null : wireValue(v.command);
    if (v.canBuy) {
      if (!command || v.blockReason !== null || v.totalGold === null
        || !safe(input.selectedId) || command.itemIndex !== input.selectedId
        || command.count !== input.quantity) return null;
    } else if (v.command !== null) return null;
    return Object.freeze({ maxQuantity: v.maxQuantity, totalGold: v.totalGold,
      canBuy: v.canBuy, blockReason: v.blockReason, command });
  } catch { return null; }
}
type Planner = NonNullable<NpcGoldBuyRuntime["getMir2NpcGoldBuyPlan"]>;
function invoke(runtime: NpcGoldBuyRuntime, planner: Planner, input: Capture): NpcGoldBuyQuote | null {
  try {
    const quote = parseQuote(planner.call(runtime, input.requestJson), input);
    return runtime.getMir2NpcGoldBuyPlan === planner ? quote : null;
  } catch { return null; }
}
/** Rust owns price/rate/stock/stack/capacity/quantity decisions; TS only checks its DTO. */
export function quoteNpcGoldBuy(
  runtime: NpcGoldBuyRuntime, current: NpcGoldBuyCurrent | null, quantity: number,
): NpcGoldBuyQuote | null {
  try {
    const input = capture(current, quantity), planner = runtime.getMir2NpcGoldBuyPlan;
    return input && !input.blocked && typeof planner === "function" ? invoke(runtime, planner, input) : null;
  } catch { return null; }
}
type Lease = {
 input: Capture; quoteJson: string; wire: NpcGoldBuyWire; planner: Planner;
 slot: NpcGoldBuyAttemptSlotRuntime; producer: object; socket: object;
 token: string; authorityRevision: string; ticket: NpcGoldBuyTransportTicket; consumed: boolean;
};

/** Gesture proofs bind a shared Core flight to the actual current socket. */
export class NpcGoldBuyDispatcher {
 private readonly proofs = new WeakMap<NpcGoldBuyProof, Lease>();
 private busy = false;
 private active = true;
 private producer: object = Object.freeze({});
 private slot: NpcGoldBuyAttemptSlotRuntime | null = null;
 constructor(private readonly options: NpcGoldBuyDispatcherOptions) {}

 /** A StrictMode effect restart uses a fresh host identity, never a fresh Core. */
 activate(): void { if (!this.active) { this.producer = Object.freeze({}); this.active = true; } this.observe(); }
 dispose(): void { if (!this.active) return; this.active = false; this.slot?.withdrawProducer(this.producer); }
 withdraw(): void {
  try { this.slot?.transact(this.producer, { op: 'availability', available: false }); } catch { /* Fail closed. */ }
 }
  /** Read the existing Core holder without attaching or observing a producer. */
  status(): NpcGoldBuyAttemptState | null {
   if (this.busy || !this.active || !this.slot) return null;
   const slot = this.slot, producer = this.producer; this.busy = true;
   try {
    const result = slot.transact(producer, { op: 'status' });
    return result.ok && result.matched && this.active && this.slot === slot && this.producer === producer ? result.state : null;
   } catch { return null; } finally { this.busy = false; }
  }
  /** Retire only current pre-entry work before the common UI changes locally. */
  withdrawAt(authorityRevision: string): boolean {
   if (this.busy || !this.active || !this.slot || typeof authorityRevision !== "string" || !/^[1-9][0-9]{0,19}$/.test(authorityRevision)
    || BigInt(authorityRevision) > 18446744073709551615n) return false;
   const slot = this.slot, producer = this.producer; this.busy = true;
   try {
    const before = slot.transact(producer, { op: 'status' });
    if (!before.ok || !before.matched || before.state.authorityRevision !== authorityRevision
     || !this.active || this.slot !== slot || this.producer !== producer) return false;
    const retired = slot.transact(producer, { op: 'availability', available: false });
    if (!retired.ok || !retired.matched || retired.state.authorityRevision !== authorityRevision || retired.state.canReserve
     || !this.active || this.slot !== slot || this.producer !== producer) return false;
    const after = slot.transact(producer, { op: 'status' });
    return after.ok && after.matched && after.state.authorityRevision === authorityRevision && !after.state.canReserve
     && this.active && this.slot === slot && this.producer === producer;
   } catch { return false; } finally { this.busy = false; }
  }
 observe(): void {
  if (this.busy || !this.active) return;
  this.busy = true;
  try { this.live(1); } catch { this.withdraw(); } finally { this.busy = false; }
 }
 private currentSlot(): NpcGoldBuyAttemptSlotRuntime | null {
  if (!this.active) return null;
  const candidate = this.options.getAttemptSlot();
  if (candidate !== this.slot) {
   if (this.slot) { this.slot.withdrawProducer(this.producer); this.producer = Object.freeze({}); }
   this.slot = candidate;
  }
  return candidate?.attachProducer(this.producer) ? candidate : null;
 }
 private live(quantity: number): LiveCapture | null {
  const slot = this.currentSlot(), producer = this.producer;
  if (!slot) return null;
  // Only the actual current OPEN socket establishes the trusted connection,
  // before authority/availability can admit a reservation on that connection.
  const binding = this.socketBinding(slot);
  if (!binding) return null;
  let input: Capture | null = null;
  try { input = capture(this.options.read(), quantity); } catch { /* Withdraw pre-entry work. */ }
  if (!this.bindingCurrent(slot, producer, binding)) return null;
  if (!input) { this.withdraw(); return null; }
  const observed = slot.transact(producer, { op: 'observe', authority: input.authorityJson });
  if (!this.captureCurrent(slot, producer, binding, input)) return null;
  if (!observed.ok || !observed.matched) { this.withdraw(); return null; }
  const available = slot.transact(producer, { op: 'availability', available: !input.blocked });
  return available.ok && available.matched && this.captureCurrent(slot, producer, binding, input)
   ? { ...input, ...binding } : null;
 }
 private socketBinding(slot: NpcGoldBuyAttemptSlotRuntime): SocketBinding | null {
  const producer = this.producer, source = this.options.readSocket();
  if (!source || !source.isOpen || !this.active || this.slot !== slot || this.producer !== producer) return null;
  const transport = slot.transportFor(producer, source.socket, source.socket, source.isOpen);
  const binding = transport ? { socket: source.socket, transport } : null;
  return binding && this.bindingCurrent(slot, producer, binding) ? binding : null;
 }
 /** No connection observation or Core call: old proofs cannot advance a socket. */
 private bindingCurrent(slot: NpcGoldBuyAttemptSlotRuntime, producer: object, binding: Readonly<{ socket: object }>): boolean {
  const current = this.options.readSocket();
  return !!current && current.isOpen && current.socket === binding.socket
   && this.active && this.slot === slot && this.producer === producer;
 }
 private captureCurrent(slot: NpcGoldBuyAttemptSlotRuntime, producer: object, binding: SocketBinding, input: Capture): boolean {
  const current = capture(this.options.read(), input.quantity);
  return current !== null && current.requestJson === input.requestJson
   && current.contextJson === input.contextJson && current.blocked === input.blocked
   && this.bindingCurrent(slot, producer, binding);
 }
 private sameLive(value: LiveCapture | null, input: Capture, binding: SocketBinding): boolean {
  return sameCapture(value, input) && value !== null
   && value.socket === binding.socket && value.transport === binding.transport;
 }

 preview(current: NpcGoldBuyCurrent | null, quantity: number): NpcGoldBuyQuote | null {
  if (this.busy || !this.active) return null;
  this.busy = true;
  try {
   const input = capture(current, quantity), live = this.live(quantity), slot = this.slot;
   const planner = this.options.runtime.getMir2NpcGoldBuyPlan;
   if (!slot || !input || !sameCapture(live, input) || typeof planner !== 'function') return null;
   const quote = invoke(this.options.runtime, planner, input);
   if (!quote || !live || !this.sameLive(this.live(quantity), input, live) || this.slot !== slot) return null;
   const producer = this.producer, status = slot.transact(producer, { op: 'status' });
   if (!status.ok || !this.bindingCurrent(slot, producer, live)) return null;
   return status.state.canReserve ? quote : Object.freeze({ ...quote, canBuy: false, command: null, blockReason: 'localPending' });
  } catch { this.withdraw(); return null; }
  finally { this.busy = false; }
 }

 prepare(current: NpcGoldBuyCurrent | null, quantity: number): PreparedNpcGoldBuy | null {
  if (this.busy || !this.active) return null;
  this.busy = true;
  let reserved: { slot: NpcGoldBuyAttemptSlotRuntime; producer: object; token: string; ticket: NpcGoldBuyTransportTicket | null } | null = null;
  let published = false;
  try {
   const input = capture(current, quantity), live = this.live(quantity), slot = this.slot;
   const planner = this.options.runtime.getMir2NpcGoldBuyPlan;
   if (!slot || !input || !sameCapture(live, input) || typeof planner !== 'function') return null;
   const quote = invoke(this.options.runtime, planner, input);
   if (!quote?.canBuy || !quote.command || !live
    || !this.sameLive(this.live(quantity), input, live) || this.slot !== slot) return null;
   const binding: SocketBinding = live, producer = this.producer;
   if (!this.bindingCurrent(slot, producer, binding)) return null;
   const ticket = Object.freeze({ transport: binding.transport, body: jsonText(quote.command) });
   const result = slot.transact(producer, { op: 'reserve' });
   if (!result.ok || !result.matched || !result.state.flight || result.state.flight.phase !== 'queued') return null;
   const { token, authorityRevision } = result.state.flight;
   reserved = { slot, producer, token, ticket: null };
   if (!this.bindingCurrent(slot, producer, binding)) return null;
   const bound = slot.transact(producer, { op: 'bind', token, ticket });
   if (!bound.ok || !bound.matched || bound.state.flight?.token !== token || bound.state.flight.phase !== 'bound') return null;
   reserved.ticket = ticket;
   if (!this.bindingCurrent(slot, producer, binding)) return null;
   const proof = Object.freeze({}) as NpcGoldBuyProof;
   this.proofs.set(proof, { input, quoteJson: JSON.stringify(quote), wire: quote.command, planner,
    slot, producer, socket: binding.socket, token, authorityRevision, ticket, consumed: false });
   published = true;
   return Object.freeze({ proof, quote, wire: quote.command });
  } catch { return null; }
  finally {
   if (reserved && !published) {
    try { reserved.slot.transact(reserved.producer, reserved.ticket
     ? { op: 'receipt', token: reserved.token, ticket: reserved.ticket, outcome: 'definitelyUnsent' }
     : { op: 'rejectUnpublished', token: reserved.token }); } catch { this.withdraw(); }
   }
   this.busy = false;
  }
 }

 private valid(lease: Lease, wire: unknown, body?: string, socket?: object): boolean {
  if (!this.active || this.slot !== lease.slot || this.producer !== lease.producer || !sameWire(wire, lease.wire)
   || jsonText(wire) !== lease.ticket.body || body !== undefined && body !== lease.ticket.body
   || socket !== undefined && socket !== lease.socket || this.options.runtime.getMir2NpcGoldBuyPlan !== lease.planner
   || !this.bindingCurrent(lease.slot, lease.producer, lease)
   || !this.sameLive(this.live(lease.input.quantity), lease.input, { socket: lease.socket, transport: lease.ticket.transport })) return false;
  const quote = invoke(this.options.runtime, lease.planner, lease.input);
  if (!quote?.canBuy || JSON.stringify(quote) !== lease.quoteJson
   || !this.sameLive(this.live(lease.input.quantity), lease.input, { socket: lease.socket, transport: lease.ticket.transport })
   || this.slot !== lease.slot || this.producer !== lease.producer) return false;
  if (!this.bindingCurrent(lease.slot, lease.producer, lease)
   || !sameWire(wire, lease.wire) || jsonText(wire) !== lease.ticket.body) return false;
  const allowed = lease.slot.transact(lease.producer, { op: 'allows', token: lease.token, ticket: lease.ticket });
  return allowed.ok && allowed.matched && allowed.state.authorityRevision === lease.authorityRevision
   && this.bindingCurrent(lease.slot, lease.producer, lease);
 }
 allows(proof: NpcGoldBuyProof, wire: unknown): boolean {
  const lease = this.proofs.get(proof);
  if (!lease || lease.consumed || this.busy || !this.active) return false;
  this.busy = true;
  try { return this.valid(lease, wire); } catch { this.withdraw(); return false; } finally { this.busy = false; }
 }
 /** All host reads precede entry. When supplied, beforeEntry is the trusted final
  * complete model/UI/socket checkpoint: true certifies the current source after
  * those reads. No external host callback follows it before Core enter; a caller
  * that changes its source and still returns true violates this contract. */
 claim(proof: NpcGoldBuyProof, wire: unknown, body: string, socket: object, beforeEntry?: () => boolean): boolean {
  const lease = this.proofs.get(proof);
  if (!lease || lease.consumed || this.busy || !this.active) return false;
  lease.consumed = true; this.busy = true;
  try {
   if (!this.valid(lease, wire, body, socket)) return false;
   if (!this.bindingCurrent(lease.slot, lease.producer, lease)) return false;
   if (beforeEntry) {
    let allowed = false;
    try { allowed = beforeEntry() === true; } catch { return false; }
    if (!allowed || !this.active || this.slot !== lease.slot || this.producer !== lease.producer) return false;
   }
   const entered = lease.slot.transact(lease.producer, { op: 'enter', token: lease.token, ticket: lease.ticket });
   return entered.ok && entered.matched && entered.state.flight?.token === lease.token && entered.state.flight.phase === 'entered';
  } catch { this.withdraw(); return false; } finally { this.busy = false; }
 }
 /** Exact Core receipts cannot downgrade an entered/unknown/flushed attempt. */
 rejectIfUnentered(proof: NpcGoldBuyProof): void {
  const lease = this.proofs.get(proof); if (!lease) return; lease.consumed = true;
  try { lease.slot.transact(lease.producer, { op: 'receipt', token: lease.token, ticket: lease.ticket, outcome: 'definitelyUnsent' }); } catch { /* Core barrier remains. */ }
 }
 transportResult(proof: NpcGoldBuyProof, outcome: 'flushed'|'unknown'): void {
  const lease = this.proofs.get(proof); if (!lease) return;
  try { lease.slot.transact(lease.producer, { op: 'receipt', token: lease.token, ticket: lease.ticket, outcome }); } catch { /* No reset or retry on receipt failure. */ }
 }
}
