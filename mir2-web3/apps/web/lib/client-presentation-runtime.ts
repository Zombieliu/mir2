import { npcPearlJson } from "./npc-pearl-buy-source";
import { npcPearlBuyInput, parseNpcPearlBuyPlan, type NpcPearlBuyInput, type NpcPearlBuyPlan } from "./npc-pearl-buy";
export type { NpcPearlBuyInput, NpcPearlBuyPlan } from "./npc-pearl-buy";
import { readSharedItemTooltip, type CrystalTooltipItem, type CrystalTooltipDocument, type CrystalTooltipRuntime } from "./shared-item-tooltip";
import { readSharedFishingClickTargets as readFishingTargets, readSharedFishingClickDecision as readFishingDecision,
  type FishingClickWasmModule, type FishingClickTargetsInput, type FishingClickTargets, type FishingClickInput, type FishingClickDecision } from "./shared-fishing-click";
export type { FishingClickTargetsInput, FishingClickTargets, FishingClickInput, FishingClickDecision } from "./shared-fishing-click";

/** Thin, data-only adapters for optional shared Rust presentation policies. */
export type MapRouteInput = Readonly<{ width: number; height: number;
  origin: Readonly<{ x: number; y: number }>; goal: Readonly<{ x: number; y: number }>;
  edges: Uint8Array }>;
export type MapRouteDecision = Readonly<{ status: "ok"; steps: ReadonlyArray<Readonly<{ x: number; y: number }>> }>
  | Readonly<{ status: "outside" | "budget" | "unreachable" }>;

export type ChatUiDocument = Readonly<{ version: 1; epoch: number; open: boolean; size: 0 | 1 | 2;
  lineCount: 4 | 7 | 11; frameIndex: number; countBarIndex: number; top: number; height: number;
  controlTop: number; inputTop: number; track: number; knobTop: number; index: number;
  historyCount: number; appliedMask: number; draftMask: number | null }>;
export type ChatUiControls = Readonly<{ source: object; document: ChatUiDocument;
  observe: (count: number) => void;
  scroll: (action: "home" | "up" | "down" | "end", epoch: number) => void;
  drag: (localY: number, grabY: number, epoch: number) => void;
  resize: (epoch: number) => void; open: (epoch: number) => void;
  editFilter: (channel: number, visible: boolean, epoch: number) => void;
  editAll: (visible: boolean, epoch: number) => void;
  editTransparent: (transparent: boolean, epoch: number) => void;
  apply: (epoch: number) => void; cancel: (epoch: number) => void; defaults: (epoch: number) => void }>;
export type ChatUiRuntime = Readonly<{ source: object; document: () => ChatUiDocument;
  observe: (count: number) => boolean;
  scroll: (action: "home" | "up" | "down" | "end", epoch: number) => boolean;
  drag: (localY: number, grabY: number, epoch: number) => boolean;
  resize: (epoch: number) => boolean; open: (epoch: number) => boolean;
  editFilter: (channel: number, visible: boolean, epoch: number) => boolean;
  editAll: (visible: boolean, epoch: number) => boolean;
  editTransparent: (transparent: boolean, epoch: number) => boolean;
  apply: (epoch: number) => number; cancel: (epoch: number) => boolean; defaults: (epoch: number) => boolean;
  retire: () => boolean; restore: (mask: number) => boolean; dispose: () => void }>;

export type CashPreviewInput = Readonly<{ itemType: number; shape: number; requiredGender: number;
  armourShape: number; female: boolean; direction: number; elapsedMs: number }>;
export type CashPreviewLayerDocument = Readonly<{ version: 1; known: boolean; direction: number;
  layers: ReadonlyArray<Readonly<{ library: string; frame: number }>> }>;
export type NpcRepairQuoteInput = Readonly<{ uniqueId: number; tooltipSource: unknown; count: number;
  currentDura: number | undefined; maxDura: number | undefined; rate: number; special: boolean; gold: number }>;
export type NpcRepairQuote = Readonly<{ repairPrice: number; displayedTotal: number; totalPrice: number; affordable: boolean }>;
export type BagToBeltMoveInput = Readonly<{ inventoryCapacity: number; source: Readonly<{ container: number; slot: number; uniqueId: number }>; targetSlot: number }>;
export type BagToBeltMovePlan = Readonly<{ uniqueId: number; from: number; to: number }>;
type RawChatUi = { document(): string; observe(count: number): boolean;
  scroll(action: number, epoch: number): boolean; drag(y: number, grab: number, epoch: number): boolean;
  resize(epoch: number): boolean; open(epoch: number): boolean;
  edit_filter(channel: number, visible: boolean, epoch: number): boolean;
  edit_all(visible: boolean, epoch: number): boolean; edit_transparent(value: boolean, epoch: number): boolean;
  apply(epoch: number): number; cancel(epoch: number): boolean; defaults(epoch: number): boolean;
  retire(): boolean; restore(mask: number): boolean; free(): void };
export type MapRouteWasmModule = {
  getMir2MapRouteVersion?: () => number;
  getMir2MapRoutePlan?: (width: number, height: number, ox: number, oy: number, gx: number, gy: number,
    edges: Uint8Array) => Int32Array;
};
export type PresentationWasmModule = FishingClickWasmModule & {
  npc_pearl_buy_abi_version?: () => number;
  npc_pearl_buy_plan?: (allowsBuy: boolean, selected: boolean, usePearls: boolean, uniqueId: number, price: number,
    stock: number, quantity: number, walletKnown: boolean, pearls: number, occupied: number, infoPrice: number, rate: number) => string;
  entity_animation_abi_version?: () => number;
  EntityAnimationBridge?: new () => RawEntityAnimationBridge;
  bag_to_belt_move_abi_version?: () => number;
  bag_to_belt_move_plan?: (json: string) => string;
  item_tooltip_abi_version?: () => number;
  item_tooltip_document?: (json: string, nowDotnetTicks: string) => string;
  chat_ui_abi_version?: () => number; ChatUiBridge?: new (mask: number) => RawChatUi;
  cash_preview_abi_version?: () => number;
  cash_preview_layers?: (kind: number, shape: number, gender: number, armour: number,
    female: boolean, direction: number, elapsed: bigint) => string;
  cash_preview_turn?: (direction: number, right: boolean) => number;
  npc_repair_quote_abi_version?: () => number;
  npc_repair_quote?: (liveUid: bigint, sourceUid: bigint, infoIndex: number, userIndex: number,
    price: number, durability: number, count: number, current: number, max: number, added: Int32Array,
    rental: boolean, rate: number, special: boolean, gold: number) => string;
};

function boundedInteger(value: unknown, minimum: number, maximum: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}
function exactKeys(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
}

export function searchSharedMapRoute(module: MapRouteWasmModule, input: MapRouteInput): MapRouteDecision {
  if (module.getMir2MapRouteVersion?.() !== 1 || !module.getMir2MapRoutePlan) throw Error("Shared map routing is unavailable");
  const { width, height, origin, goal, edges } = input;
  if (!boundedInteger(width, 1, 16777216) || !boundedInteger(height, 1, 16777216)
    || width * height > 16777216 || !(edges instanceof Uint8Array) || edges.length !== width * height
    || !origin || !goal || ![origin, goal].every(p => boundedInteger(p.x, 0, width - 1) && boundedInteger(p.y, 0, height - 1))) {
    return { status: "outside" };
  }
  const raw = module.getMir2MapRoutePlan(width, height, origin.x, origin.y, goal.x, goal.y, edges);
  if (!(raw instanceof Int32Array) || raw.length < 1 || raw.length > 500001 || raw.length % 2 !== 1) {
    throw Error("Invalid shared route response");
  }
  if (raw[0] !== 0) {
    if (raw.length !== 1 || ![1, 2, 3].includes(raw[0])) throw Error("Invalid shared route status");
    return { status: raw[0] === 1 ? "outside" : raw[0] === 2 ? "budget" : "unreachable" };
  }
  const compass = [[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]] as const;
  const steps: Array<Readonly<{ x: number; y: number }>> = [];
  let previous = origin;
  for (let i = 1; i < raw.length; i += 2) {
    const point = { x: raw[i], y: raw[i + 1] };
    const direction = compass.findIndex(([dx, dy]) => point.x - previous.x === dx && point.y - previous.y === dy);
    if (!boundedInteger(point.x, 0, width - 1) || !boundedInteger(point.y, 0, height - 1) || direction < 0
      || (edges[previous.y * width + previous.x] & 1 << direction) === 0) throw Error("Invalid shared route step");
    steps.push(Object.freeze(point)); previous = point;
  }
  if (previous.x !== goal.x || previous.y !== goal.y) throw Error("Incomplete shared route response");
  return { status: "ok", steps: Object.freeze(steps) };
}

function parseChatDocument(raw: string): ChatUiDocument {
  if (typeof raw !== "string" || raw.length > 2048) throw Error("Invalid shared chat document");
  const d: unknown = JSON.parse(raw);
  const numeric = ["epoch","size","lineCount","frameIndex","countBarIndex","top","height","controlTop","inputTop","track","knobTop","index","historyCount","appliedMask"];
  if (!exactKeys(d, ["version","open",...numeric,"draftMask"]) || d.version !== 1 || typeof d.open !== "boolean"
    || !numeric.every(key => boundedInteger(d[key], 0, 4294967295)) || !boundedInteger(d.epoch, 1, 4294967295)
    || !boundedInteger(d.size, 0, 2) || ![4,7,11].includes(d.lineCount as number)
    || !boundedInteger(d.historyCount, 0, 1000000) || !boundedInteger(d.index, 0, Math.max(0, (d.historyCount as number) - 1))
    || !boundedInteger(d.appliedMask, 0, 1023) || (d.draftMask !== null && !boundedInteger(d.draftMask, 0, 1023))
    || d.open !== (d.draftMask !== null) || !boundedInteger(d.top, 0, 768) || !boundedInteger(d.height, 1, 768)
    || !boundedInteger(d.controlTop, 0, 768) || !boundedInteger(d.inputTop, 0, 768)
    || !boundedInteger(d.knobTop, 0, 768) || !boundedInteger(d.track, 0, 768)) throw Error("Invalid shared chat projection");
  return Object.freeze(d) as ChatUiDocument;
}

export function createSharedChatUi(module: PresentationWasmModule, mask: number): ChatUiRuntime {
  if (module.chat_ui_abi_version?.() !== 1 || !module.ChatUiBridge) throw Error("Shared chat controls are unavailable");
  if (!boundedInteger(mask, 0, 1023)) throw Error("Invalid chat preference mask");
  const bridge = new module.ChatUiBridge(mask), source = Object.freeze({});
  let disposed = false;
  const epochValid = (epoch: number) => !disposed && boundedInteger(epoch, 1, 4294967295);
  const scrollActions = { home: 0, up: 1, down: 2, end: 3 } as const;
  return {
    source,
    document() { if (disposed) throw Error("Chat controls were retired"); return parseChatDocument(bridge.document()); },
    observe(count) { return !disposed && boundedInteger(count, 0, 1000000) && bridge.observe(count); },
    scroll(action, epoch) { return epochValid(epoch) && Object.hasOwn(scrollActions, action) && bridge.scroll(scrollActions[action], epoch); },
    drag(y, grab, epoch) { return epochValid(epoch) && Number.isFinite(y) && Number.isFinite(grab)
      && Math.abs(y) <= 1000000 && Math.abs(grab) <= 1000000 && bridge.drag(y, grab, epoch); },
    resize(epoch) { return epochValid(epoch) && bridge.resize(epoch); },
    open(epoch) { return epochValid(epoch) && bridge.open(epoch); },
    editFilter(channel, visible, epoch) { return epochValid(epoch) && boundedInteger(channel, 0, 8)
      && typeof visible === "boolean" && bridge.edit_filter(channel, visible, epoch); },
    editAll(visible, epoch) { return epochValid(epoch) && typeof visible === "boolean" && bridge.edit_all(visible, epoch); },
    editTransparent(value, epoch) { return epochValid(epoch) && typeof value === "boolean" && bridge.edit_transparent(value, epoch); },
    apply(epoch) { if (!epochValid(epoch)) return -1; const result = bridge.apply(epoch);
      if (result !== -1 && !boundedInteger(result, 0, 1023)) throw Error("Invalid shared chat commit"); return result; },
    cancel(epoch) { return epochValid(epoch) && bridge.cancel(epoch); },
    defaults(epoch) { return epochValid(epoch) && bridge.defaults(epoch); },
    retire() { return !disposed && bridge.retire(); },
    restore(value) { return !disposed && boundedInteger(value, 0, 1023) && bridge.restore(value); },
    dispose() { if (!disposed) { disposed = true; bridge.free(); } },
  };
}

export function readSharedCashPreview(module: PresentationWasmModule, input: CashPreviewInput): CashPreviewLayerDocument | null {
  if (module.cash_preview_abi_version?.() !== 1 || !module.cash_preview_layers) return null;
  if (!boundedInteger(input.itemType, 0, 255) || !boundedInteger(input.shape, 0, 32767)
    || !boundedInteger(input.requiredGender, 0, 255) || !boundedInteger(input.armourShape, -32768, 32767)
    || typeof input.female !== "boolean" || !boundedInteger(input.direction, 1, 8)
    || !boundedInteger(input.elapsedMs, 0, Number.MAX_SAFE_INTEGER)) return null;
  const raw = module.cash_preview_layers(input.itemType, input.shape, input.requiredGender, input.armourShape,
    input.female, input.direction, BigInt(input.elapsedMs));
  if (typeof raw !== "string" || raw.length > 2048) return null;
  let d: unknown;
  try { d = JSON.parse(raw); } catch { return null; }
  if (!exactKeys(d, ["version","known","direction","layers"]) || d.version !== 1 || typeof d.known !== "boolean"
    || d.direction !== input.direction || !Array.isArray(d.layers) || d.layers.length > 3
    || (!d.known && d.layers.length !== 0) || (d.known && d.layers.length === 0)) return null;
  const seen = new Set<string>(), layers: Array<Readonly<{ library: string; frame: number }>> = [];
  for (const layer of d.layers) {
    if (!exactKeys(layer, ["library","frame"]) || typeof layer.library !== "string"
      || !/^(?:CArmour|CWeapon|AWeaponR|AWeaponL|ARWeapon|Mount|Transform)\/[0-9]{2,5}$/.test(layer.library)
      || !boundedInteger(layer.frame, 0, 65535)) return null;
    const suffix = layer.library.split("/")[1];
    if (Number(suffix) > 32767 || String(Number(suffix)).padStart(2,"0") !== suffix) return null;
    const key = `${layer.library}:${layer.frame}`; if (seen.has(key)) return null; seen.add(key);
    layers.push(Object.freeze({ library: layer.library, frame: layer.frame }));
  }
  return Object.freeze({ version: 1, known: d.known, direction: input.direction, layers: Object.freeze(layers) });
}

export function turnSharedCashPreview(module: PresentationWasmModule, direction: number, right: boolean): number | null {
  if (module.cash_preview_abi_version?.() !== 1 || !module.cash_preview_turn
    || !boundedInteger(direction, 1, 8) || typeof right !== "boolean") return null;
  const result = module.cash_preview_turn(direction, right);
  return boundedInteger(result, 1, 8) ? result : null;
}

/** Source decoding only. Price, stat weight, rounding and wallet admission all belong to Rust. */
export function readSharedNpcRepairQuote(module: PresentationWasmModule, input: NpcRepairQuoteInput): NpcRepairQuote | null {
  if (module.npc_repair_quote_abi_version?.() !== 1 || !module.npc_repair_quote
    || !boundedInteger(input.uniqueId, 0, Number.MAX_SAFE_INTEGER) || !boundedInteger(input.count, 1, 65535)
    || !boundedInteger(input.currentDura, 0, 65535) || !boundedInteger(input.maxDura, 0, 65535)
    || !boundedInteger(input.gold, 0, 4294967295) || !Number.isFinite(input.rate) || input.rate < 0
    || !Number.isFinite(Math.fround(input.rate)) || typeof input.special !== "boolean") return null;
  const source = input.tooltipSource;
  if (typeof source !== "object" || !source || Array.isArray(source)) return null;
  const { info, userItem } = source as Record<string, unknown>;
  if (typeof info !== "object" || !info || Array.isArray(info) || typeof userItem !== "object" || !userItem || Array.isArray(userItem)) return null;
  const template = info as Record<string, unknown>, user = userItem as Record<string, unknown>;
  if (!boundedInteger(template.item_index, -2147483648, 2147483647) || user.item_index !== template.item_index
    || user.unique_id !== input.uniqueId || !boundedInteger(template.price, 0, 4294967295)
    || !boundedInteger(template.durability, 0, 65535) || !Array.isArray(user.added_stats) || user.added_stats.length > 256
    || !Object.hasOwn(user, "rental_information") || user.rental_information !== null
      && (typeof user.rental_information !== "object" || Array.isArray(user.rental_information))) return null;
  const values: number[] = [];
  for (const stat of user.added_stats) {
    if (!exactKeys(stat, ["stat", "value"]) || !boundedInteger(stat.stat, 0, 255)
      || !boundedInteger(stat.value, -2147483648, 2147483647)) return null;
    values.push(stat.value);
  }
  try {
    const text = module.npc_repair_quote(BigInt(input.uniqueId), BigInt(input.uniqueId), template.item_index,
      template.item_index, template.price, template.durability, input.count, input.currentDura, input.maxDura,
      Int32Array.from(values), user.rental_information !== null, input.rate, input.special, input.gold);
    if (typeof text !== "string" || text.length > 1024) return null;
    const result: unknown = JSON.parse(text);
    if (!exactKeys(result, ["version", "known", "quote"]) || result.version !== 1 || result.known !== true
      || !exactKeys(result.quote, ["repairPrice", "displayedTotal", "totalPrice", "affordable"])) return null;
    const quote = result.quote;
    if (!boundedInteger(quote.repairPrice, 0, 4294967295) || !boundedInteger(quote.totalPrice, 0, 4294967295)
      || typeof quote.displayedTotal !== "number" || !Number.isFinite(quote.displayedTotal) || quote.displayedTotal < 0
      || quote.displayedTotal > Math.fround(4294967295) || typeof quote.affordable !== "boolean") return null;
    return Object.freeze(quote) as NpcRepairQuote;
  } catch { return null; }
}

const itemTooltipAdapters = new WeakMap<PresentationWasmModule, {
  abi: NonNullable<PresentationWasmModule["item_tooltip_abi_version"]>;
  getter: NonNullable<PresentationWasmModule["item_tooltip_document"]>;
  runtime: CrystalTooltipRuntime;
}>();

/** Optional data-only formatter. Catalogue previews stay on the renderer ABI. */
export function readSharedPresentationItemTooltip(module: PresentationWasmModule, item: CrystalTooltipItem,
  player: Readonly<Record<string, unknown>>, nowMs = Date.now()): CrystalTooltipDocument | null {
  try {
    const abi = module.item_tooltip_abi_version, getter = module.item_tooltip_document;
    if (typeof abi !== "function" || typeof getter !== "function" || abi.call(module) !== 1
      || item.sourceKind !== undefined && item.sourceKind !== "instance") return null;
    let adapter = itemTooltipAdapters.get(module);
    if (!adapter || adapter.abi !== abi || adapter.getter !== getter) {
      adapter = {abi, getter, runtime: {getMir2ItemTooltipDocument(json, clock) {
        if (module.item_tooltip_abi_version !== abi || module.item_tooltip_document !== getter || abi.call(module) !== 1) {
          throw Error("Item tooltip formatter changed");
        }
        return getter.call(module, json, clock);
      }}};
      itemTooltipAdapters.set(module, adapter);
    }
    const document = readSharedItemTooltip(adapter.runtime, item, player, nowMs);
    return module.item_tooltip_abi_version === abi && module.item_tooltip_document === getter && abi.call(module) === 1 ? document : null;
  } catch { return null; }
}

/** Optional shared planner; wire source offset and capacity policy remain in Rust. */
export function readSharedBagToBeltMovePlan(module: PresentationWasmModule, input: BagToBeltMoveInput): BagToBeltMovePlan | null {
  try {
    const abi = module.bag_to_belt_move_abi_version, planner = module.bag_to_belt_move_plan;
    if (typeof abi !== "function" || typeof planner !== "function" || abi.call(module) !== 1
      || !boundedInteger(input.inventoryCapacity, 0, Number.MAX_SAFE_INTEGER)
      || !input.source || !boundedInteger(input.source.container, 0, 255)
      || !boundedInteger(input.source.slot, 0, Number.MAX_SAFE_INTEGER)
      || !boundedInteger(input.source.uniqueId, 0, Number.MAX_SAFE_INTEGER)
      || !boundedInteger(input.targetSlot, 0, 255)) return null;
    const request = JSON.stringify({ version: 1, inventoryCapacity: input.inventoryCapacity,
      source: { container: input.source.container, slot: input.source.slot, uniqueId: input.source.uniqueId }, targetSlot: input.targetSlot });
    if (module.bag_to_belt_move_abi_version !== abi || module.bag_to_belt_move_plan !== planner || abi.call(module) !== 1) return null;
    const text = planner.call(module, request);
    if (module.bag_to_belt_move_abi_version !== abi || module.bag_to_belt_move_plan !== planner || abi.call(module) !== 1
      || typeof text !== "string" || text.length > 1024) return null;
    const result: unknown = JSON.parse(text);
    if (!exactKeys(result, ["version", "ok", "plan"]) || result.version !== 1 || result.ok !== true
      || !exactKeys(result.plan, ["uniqueId", "from", "to"])) return null;
    const plan = result.plan;
    if (plan.uniqueId !== input.source.uniqueId || !boundedInteger(plan.uniqueId, 0, Number.MAX_SAFE_INTEGER)
      || !boundedInteger(plan.from, 0, 255) || !boundedInteger(plan.to, 0, 5) || plan.to !== input.targetSlot) return null;
    return Object.freeze({ uniqueId: plan.uniqueId, from: plan.from, to: plan.to });
  } catch { return null; }
}

export function readSharedFishingClickTargets(module: PresentationWasmModule, input: FishingClickTargetsInput): FishingClickTargets | null {
  return readFishingTargets(module, input);
}

export function readSharedFishingClickDecision(module: PresentationWasmModule, input: FishingClickInput): FishingClickDecision | null {
  return readFishingDecision(module, input);
}

export type EntityAnimationRuntime = Readonly<{
  source: object;
  resolveMir2EntityAnimationPoses(snapshotJson: string): string;
  getMir2EntityActionPose(queryJson: string): string;
  resetMir2EntityAnimations(): void;
}>;

type RawEntityAnimationBridge = {
  resolveMir2EntityAnimationPoses(snapshotJson: string): string;
  getMir2EntityActionPose(queryJson: string): string;
  resetMir2EntityAnimations(): void;
};

/** Lazily owns one shared animation world per Core facade. */
export function createSharedEntityAnimationAccessor(module: PresentationWasmModule): () => EntityAnimationRuntime | null {
  let cached: { runtime: EntityAnimationRuntime; current: () => boolean } | null = null;
  let attempted = false, busy = false;
  return () => {
    if (busy) return null;
    busy = true;
    try {
      if (cached) return cached.current() ? cached.runtime : null;
      if (attempted) return null;
      attempted = true;
      const abi = module.entity_animation_abi_version, Bridge = module.EntityAnimationBridge;
      if (typeof abi !== "function" || typeof Bridge !== "function" || abi.call(module) !== 1
        || module.entity_animation_abi_version !== abi || module.EntityAnimationBridge !== Bridge) return null;
      const source = new Bridge();
      if (!source || typeof source !== "object") return null;
      const resolve = source.resolveMir2EntityAnimationPoses, peek = source.getMir2EntityActionPose;
      const reset = source.resetMir2EntityAnimations;
      if (typeof resolve !== "function" || typeof peek !== "function" || typeof reset !== "function") return null;
      const identities = () => module.entity_animation_abi_version === abi && module.EntityAnimationBridge === Bridge
        && source.resolveMir2EntityAnimationPoses === resolve && source.getMir2EntityActionPose === peek
        && source.resetMir2EntityAnimations === reset;
      const current = () => identities() && abi.call(module) === 1 && identities();
      const unavailable = () => new Error("Shared entity animation bridge is unavailable");
      const bounded = (text: unknown, maximum: number): text is string => typeof text === "string"
        && text.length <= maximum && new TextEncoder().encode(text).byteLength <= maximum;
      const read = (input: string, maximum: number, getter: (json: string) => string): string => {
        if (busy) throw unavailable();
        busy = true;
        try {
          if (!bounded(input, maximum) || !current()) throw unavailable();
          const result = getter.call(source, input);
          if (!current() || !bounded(result, maximum)) throw unavailable();
          // Page/Shell retain their existing semantic JSON and authority guards.
          return result;
        } finally { busy = false; }
      };
      const runtime: EntityAnimationRuntime = Object.freeze({ source,
        resolveMir2EntityAnimationPoses(snapshotJson: string) { return read(snapshotJson, 2_097_152, resolve); },
        getMir2EntityActionPose(queryJson: string) { return read(queryJson, 2048, peek); },
        resetMir2EntityAnimations() {
          if (busy) throw unavailable();
          busy = true;
          try {
            if (!current()) throw unavailable();
            reset.call(source);
            if (!current()) throw unavailable();
          } finally { busy = false; }
        },
      });
      if (!current()) return null;
      cached = { runtime, current };
      return runtime;
    } catch { return null; }
    finally { busy = false; }
  };
}

/** Optional scalar policy; quantity, stock, price, wallet and 46-entry admission remain Rust rules. */
export function readSharedNpcPearlBuyPlan(module: PresentationWasmModule, input: NpcPearlBuyInput): NpcPearlBuyPlan | null {
  try {
    const abi = module.npc_pearl_buy_abi_version, planner = module.npc_pearl_buy_plan;
    if (typeof abi !== 'function' || typeof planner !== 'function') return null;
    const checked = npcPearlBuyInput(npcPearlJson(input));
    if (!checked) return null;
    const version = abi.call(module);
    if (version !== 2 || module.npc_pearl_buy_abi_version !== abi || module.npc_pearl_buy_plan !== planner) return null;
    const text = planner.call(module,checked.allowsBuy,checked.selected,checked.usePearls,checked.uniqueId,checked.unitPrice,
      checked.stock,checked.quantity,checked.walletKnown,checked.pearls,checked.occupied,checked.infoPrice,checked.rate);
    const finalVersion = abi.call(module);
    if (finalVersion !== 2 || module.npc_pearl_buy_abi_version !== abi || module.npc_pearl_buy_plan !== planner
      || typeof text !== 'string' || text.length > 1024) return null;
    return parseNpcPearlBuyPlan(JSON.parse(text));
  } catch { return null; }
}
