import type { BevyInventoryModel } from "./bevy-bag-model";
import { validNpcGoldBuyBagModel, type BagInputRegion, type BagOwner } from "./bevy-bag-ui";
import type { BevyQuestUiPresentation } from "./bevy-quest-ui";
import type { Mir2Language } from "./localization";
import type { NpcGoldBuyShop, NpcGoldBuyWire } from "./bevy-npc-shop-buy";
import type { NpcGoldBuyAttemptPhase, NpcGoldBuyAttemptState } from "./client-core-runtime";

export type NpcShopIdentity = Readonly<{ runGeneration: number; connectionGeneration: number;
  sessionGeneration: number; ownerRevision: number; playerObjectId: number }>;
export type NpcShopProof = NpcShopIdentity & Readonly<{ revision: number; modelRevision: number;
  presentationRevision: number; serviceRevision: number; catalogRevision: number;
  coreAuthorityRevision: string; controlRevision: string }>;
export type NpcShopGesture = Readonly<{ pointerId: number; downSequence: number; sequence: number;
  origin: "shop" | "world"; button: 0 | 2 }>;
export type NpcShopAction = Readonly<{ type: "select"; uniqueId: number }> | Readonly<{
  type: "quantityInc" | "quantityDec" | "pageUp" | "pageDown" | "buy" | "close" | "handoffSell" }>;
export type NpcShopIntent = Readonly<{ proof: NpcShopProof; gesture: NpcShopGesture; intentSequence: number;
  action: NpcShopAction; selectedId: number | null; quantity: number; startIndex: number;
  preentryWithdraw: boolean; command: NpcGoldBuyWire | null }>;
export type NpcShopPointerContext = Readonly<{ proof: NpcShopProof; presentation: BevyQuestUiPresentation;
  inputRegions: readonly BagInputRegion[] }>;
/** Rust pointer RPC flattens proof and gesture; its emitted intent nests both. */
export type NpcShopPointerEdge = NpcShopProof & NpcShopGesture & Readonly<{
  phase: "down" | "move" | "up" | "cancel" | "blur"; x: number; y: number }>;
export type NpcShopFeedback = Readonly<{ phase: NpcGoldBuyAttemptPhase | null; pending: boolean;
  canReserve: boolean; previousUnknown: 0 }>;
/** All 23 PlayerStats fields are supplied by Page; missing data is never defaulted here. */
export type NpcShopPlayer = Readonly<{ hp: number; maxHp: number; mp: number; maxMp: number;
  gold: number; credit: number; level: number;
  crystalStats: readonly Readonly<{ stat: number; value: number }>[] | null;
  experience: number; maxExperience: number; currentWeight: number; maxWeight: number;
  currentWeightKnown: boolean; weights: Readonly<{ bag: number; wear: number; hand: number }> | null;
  name: string | null; className: string | null; gender: string | null; hair: number | null;
  wingEffect: number | null; guildName: string | null; guildRankName: string | null;
  mapName: string | null; inSafeZone: boolean }>;
export type NpcShopStatus = NpcShopIdentity & Readonly<{ frame: number; ready: boolean; inputEnabled: boolean;
  appliedRevision: number; appliedModelRevision: number; appliedPresentationRevision: number;
  appliedServiceRevision: number; appliedCatalogRevision: number; coreAuthorityRevision: string | null;
  controlRevision: string | null; selectedId: number | null; quantity: number; startIndex: number;
  feedback: NpcShopFeedback; inputRegions: readonly BagInputRegion[]; error: string | null }>;
export type NpcShopHostInput = Omit<NpcShopIdentity, "runGeneration"> & Readonly<{
  serviceRevision: number; catalogRevision: number; open: boolean; showBuy: boolean; eligible: boolean;
  presentation: BevyQuestUiPresentation | null; shop: NpcGoldBuyShop | null; inventory: BevyInventoryModel | null;
  player: NpcShopPlayer; language: Mir2Language; coreStatus: NpcGoldBuyAttemptState | null }>;
export type NpcShopRuntime = { getMir2NpcShopUiCapabilities?: () => string;
  setMir2NpcShopUiSnapshot?: (json: string) => boolean; getMir2NpcShopUiStatus?: () => string;
  setMir2NpcShopUiIntentSink?: (sink: (json: string) => boolean) => void;
  clearMir2NpcShopUiIntentSink?: () => void; setMir2NpcShopUiPointerEdge?: (json: string) => boolean;
  withdrawMir2NpcShopUiSnapshot?: (identityJson: string) => boolean };
export type NpcShopHostState = Readonly<{ active: boolean; transitioning: boolean; ownerRevision: number; error: string | null }>;
export type NpcShopHostOptions = { runtime: NpcShopRuntime; read: () => NpcShopHostInput; now: () => number;
  onOwner: (owner: BagOwner, run: number, ownerRevision: number) => void;
  onState: (state: NpcShopHostState) => void; onIntent: (intent: NpcShopIntent) => boolean };


const SAFE = Number.MAX_SAFE_INTEGER;
const MAX_JSON = 1_048_576;
const U64_MAX = "18446744073709551615";
const object = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const safe = (v: unknown, max = SAFE): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0 && v <= max;
const signed = (v: unknown, min: number, max: number): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
const exact = (v: Record<string, unknown>, keys: readonly string[]) => Object.keys(v).sort().join("|") === [...keys].sort().join("|");
const required = (v: Record<string, unknown>, keys: readonly string[]) => keys.every(k => Object.hasOwn(v, k));
const positive = (v: unknown): v is number => safe(v) && v > 0;
const decimal = (v: unknown): v is string => typeof v === "string" && /^[1-9][0-9]{0,19}$/.test(v)
  && (v.length < 20 || v <= U64_MAX);
const phase = (v: unknown): v is NpcGoldBuyAttemptPhase => ["queued", "bound", "entered", "flushed", "unknown", "definitelyUnsent"].includes(v as string);
const identityKeys = ["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId"] as const;
const proofKeys = [...identityKeys, "revision", "modelRevision", "presentationRevision", "serviceRevision", "catalogRevision", "coreAuthorityRevision", "controlRevision"] as const;
const gestureKeys = ["pointerId", "downSequence", "sequence", "origin", "button"] as const;
function identityValid(v: Record<string, unknown>): boolean {
  return [v.runGeneration,v.connectionGeneration,v.sessionGeneration].every(positive)
    && safe(v.ownerRevision) && positive(v.playerObjectId) && Number(v.playerObjectId) <= 0xffff_ffff;
}
export const sameNpcShopIdentity = (a: NpcShopIdentity, b: NpcShopIdentity): boolean => identityKeys.every(k => a[k] === b[k]);
function identity(v: NpcShopIdentity): NpcShopIdentity {
  return { runGeneration:v.runGeneration, connectionGeneration:v.connectionGeneration, sessionGeneration:v.sessionGeneration,
    ownerRevision:v.ownerRevision, playerObjectId:v.playerObjectId };
}
function proofValid(v: unknown): v is NpcShopProof {
  return object(v) && exact(v,proofKeys) && identityValid(v)
    && [v.revision,v.modelRevision,v.presentationRevision,v.serviceRevision,v.catalogRevision].every(positive)
    && decimal(v.coreAuthorityRevision) && decimal(v.controlRevision);
}
function sameProof(a: NpcShopProof, b: NpcShopProof): boolean { return proofKeys.every(k => a[k] === b[k]); }
function gestureValid(v: unknown): v is NpcShopGesture {
  return object(v) && exact(v,gestureKeys) && safe(v.pointerId) && positive(v.downSequence)
    && positive(v.sequence) && v.sequence >= v.downSequence && (v.origin === "shop" || v.origin === "world")
    && (v.button === 0 || v.button === 2);
}
function sameGesture(a: NpcShopGesture, b: NpcShopGesture): boolean {
  return a.pointerId === b.pointerId && a.downSequence === b.downSequence && a.origin === b.origin && a.button === b.button;
}
function edgeProof(e: NpcShopPointerEdge): NpcShopProof {
  return { ...identity(e), revision:e.revision, modelRevision:e.modelRevision, presentationRevision:e.presentationRevision,
    serviceRevision:e.serviceRevision, catalogRevision:e.catalogRevision, coreAuthorityRevision:e.coreAuthorityRevision,
    controlRevision:e.controlRevision };
}
function edgeGesture(e: NpcShopPointerEdge): NpcShopGesture {
  return { pointerId:e.pointerId,downSequence:e.downSequence,sequence:e.sequence,origin:e.origin,button:e.button };
}
function edgeValid(v: unknown): v is NpcShopPointerEdge {
  if (!object(v) || !exact(v,[...proofKeys,...gestureKeys,"phase","x","y"])) return false;
  const e = v as unknown as NpcShopPointerEdge;
  return proofValid(edgeProof(e)) && gestureValid(edgeGesture(e))
    && ["down","move","up","cancel","blur"].includes(e.phase)
    && (e.phase !== "down" || e.sequence === e.downSequence)
    && [e.x,e.y].every(n => typeof n === "number" && Number.isFinite(n) && Math.abs(n) <= 16384);
}
function actionValid(v: unknown): v is NpcShopAction {
  return object(v) && (v.type === "select" ? exact(v,["type","uniqueId"]) && safe(v.uniqueId)
    : exact(v,["type"]) && ["quantityInc","quantityDec","pageUp","pageDown","buy","close","handoffSell"].includes(v.type as string));
}
function wireValid(v: unknown): v is NpcGoldBuyWire {
  return object(v) && exact(v,["type","itemIndex","count","panelType"]) && v.type === "buyItem"
    && safe(v.itemIndex) && positive(v.count) && Number(v.count) <= 65535 && v.panelType === 0;
}
function feedbackValid(v: unknown): v is NpcShopFeedback {
  return object(v) && exact(v,["phase","pending","canReserve","previousUnknown"])
    && (v.phase === null || phase(v.phase)) && typeof v.pending === "boolean" && typeof v.canReserve === "boolean"
    && v.previousUnknown === 0 && !(v.pending && v.canReserve)
    && (!v.pending || v.phase !== null && v.phase !== "definitelyUnsent");
}
function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object") { for (const child of Object.values(value)) deepFreeze(child); Object.freeze(value); }
  return value;
}
/** Data transport only: accessors, cycles, coercion and raw defaults are refused. */
function jsonText(value: unknown): string {
  const stack = new Set<object>();
  function visit(v: unknown, depth: number): void {
    if (depth > 64) throw new Error("Deep NPC snapshot");
    if (v === null || typeof v === "string" || typeof v === "boolean" || typeof v === "number" && Number.isFinite(v)) return;
    if (typeof v !== "object" || v === null) throw new Error("Non-JSON NPC snapshot");
    const proto = Object.getPrototypeOf(v);
    if (proto !== Object.prototype && proto !== null && proto !== Array.prototype || stack.has(v)
      || "toJSON" in v || Object.getOwnPropertySymbols(v).length) throw new Error("Non-plain NPC snapshot");
    stack.add(v);
    for (const [key, descriptor] of Object.entries(Object.getOwnPropertyDescriptors(v))) {
      if (Array.isArray(v) && key === "length") continue;
      if (!descriptor.enumerable || !("value" in descriptor)) throw new Error("Non-data NPC snapshot");
      if (Array.isArray(v) && (!/^(0|[1-9][0-9]*)$/.test(key) || Number(key) >= v.length)) throw new Error("Non-index NPC array");
      visit(descriptor.value, depth + 1);
    }
    if (Array.isArray(v) && Object.keys(v).length !== v.length) throw new Error("Sparse NPC array");
    stack.delete(v);
  }
  visit(value, 0);
  const json = JSON.stringify(value);
  if (typeof json !== "string" || new TextEncoder().encode(json).length > MAX_JSON) throw new Error("Oversized NPC snapshot");
  return json;
}
export function parseNpcShopIntent(json: string): NpcShopIntent | null {
  try {
    const v: unknown = JSON.parse(json);
    if (!object(v) || !exact(v,["proof","gesture","intentSequence","action","selectedId","quantity","startIndex","preentryWithdraw","command"])
      || !proofValid(v.proof) || !gestureValid(v.gesture) || !positive(v.intentSequence) || !actionValid(v.action)
      || !(v.selectedId === null || safe(v.selectedId)) || !positive(v.quantity) || Number(v.quantity) > 65535
      || !safe(v.startIndex) || v.startIndex > 2048 || typeof v.preentryWithdraw !== "boolean") return null;
    if (v.gesture.origin !== "shop" || v.gesture.button !== 0 || v.preentryWithdraw !== (v.action.type !== "buy")) return null;
    if (v.action.type === "buy") {
      if (!wireValid(v.command) || v.selectedId !== v.command.itemIndex || v.quantity !== v.command.count) return null;
    } else if (v.command !== null) return null;
    jsonText(v); return deepFreeze(v as NpcShopIntent);
  } catch { return null; }
}
export function supportsNpcShopUi(runtime: NpcShopRuntime): boolean {
  try {
    const v: unknown = JSON.parse(runtime.getMir2NpcShopUiCapabilities?.() ?? "null");
    return object(v) && v.schemaVersion === 1 && v.npcShopUiAbiVersion === 1 && v.npcShopIntentAbiVersion === 1
      && v.compiled === true && v.startup === true && [runtime.setMir2NpcShopUiSnapshot,runtime.getMir2NpcShopUiStatus,
        runtime.setMir2NpcShopUiIntentSink,runtime.clearMir2NpcShopUiIntentSink,runtime.setMir2NpcShopUiPointerEdge,
        runtime.withdrawMir2NpcShopUiSnapshot].every(fn => typeof fn === "function");
  } catch { return false; }
}
export function fitsNpcShop(p: BevyQuestUiPresentation | null): boolean {
  if (!p || !positive(p.logicalWidth) || !positive(p.logicalHeight) || p.logicalWidth > 16384 || p.logicalHeight > 16384
    || typeof p.touch !== "boolean" || !Number.isFinite(p.stageCssScale) || p.stageCssScale < 0.05 || p.stageCssScale > 16) return false;
  const scale = p.touch ? 1.28 / p.stageCssScale : 1;
  return 584 * scale + 16 <= p.logicalWidth && 334 * scale + 16 <= p.logicalHeight;
}
export function readNpcShopStatus(runtime: NpcShopRuntime): NpcShopStatus | null {
  try {
    const v: unknown = JSON.parse(runtime.getMir2NpcShopUiStatus?.() ?? "null");
    if (!object(v) || !exact(v,[...identityKeys,"frame","ready","inputEnabled","appliedRevision","appliedModelRevision",
      "appliedPresentationRevision","appliedServiceRevision","appliedCatalogRevision","coreAuthorityRevision","controlRevision",
      "selectedId","quantity","startIndex","feedback","inputRegions","error"]) || !identityValid(v)
      || ![v.frame,v.appliedRevision,v.appliedModelRevision,v.appliedPresentationRevision,v.appliedServiceRevision,v.appliedCatalogRevision].every(value => safe(value))
      || typeof v.ready !== "boolean" || typeof v.inputEnabled !== "boolean"
      || !(v.coreAuthorityRevision === null || decimal(v.coreAuthorityRevision)) || !(v.controlRevision === null || decimal(v.controlRevision))
      || !(v.selectedId === null || safe(v.selectedId)) || !positive(v.quantity) || Number(v.quantity) > 65535
      || !safe(v.startIndex) || v.startIndex > 2048 || !feedbackValid(v.feedback)
      || !(v.error === null || typeof v.error === "string") || !Array.isArray(v.inputRegions) || v.inputRegions.length > 32
      || !v.inputRegions.every(r => object(r) && exact(r,["left","top","width","height"])
        && [r.left,r.top,r.width,r.height].every(n => typeof n === "number" && Number.isFinite(n))
        && Number(r.width) > 0 && Number(r.height) > 0)
      || v.inputEnabled && (!v.ready || v.coreAuthorityRevision === null || v.controlRevision === null)
      || v.ready && (!positive(v.appliedRevision) || !positive(v.appliedModelRevision) || !positive(v.appliedPresentationRevision)
        || !positive(v.appliedServiceRevision) || !positive(v.appliedCatalogRevision) || v.coreAuthorityRevision === null || v.controlRevision === null)) return null;
    jsonText(v); return deepFreeze(v as NpcShopStatus);
  } catch { return null; }
}

function paintedRegions(v: NpcShopStatus, p: BevyQuestUiPresentation | null): boolean {
  if (!p || !fitsNpcShop(p) || v.inputRegions.length !== 2) return false;
  const scale = p.touch ? 1.28 / p.stageCssScale : 1;
  const left = (p.logicalWidth - 584 * scale) * 0.5, top = (p.logicalHeight - 334 * scale) * 0.5;
  const expected = [[left,top,244*scale,334*scale],[left+254*scale,top+34*scale,330*scale,80*scale]];
  return v.inputRegions.every((r,i) => r.left >= -0.1 && r.top >= -0.1 && r.left+r.width <= p.logicalWidth+0.1
    && r.top+r.height <= p.logicalHeight+0.1
    && [r.left,r.top,r.width,r.height].every((n,j) => Math.abs(n-expected[i][j]) <= 0.1));
}

const playerKeys = ["hp","maxHp","mp","maxMp","gold","credit","level","crystalStats","experience","maxExperience",
  "currentWeight","maxWeight","currentWeightKnown","weights","name","className","gender","hair","wingEffect",
  "guildName","guildRankName","mapName","inSafeZone"] as const;
function playerValid(p: unknown): p is NpcShopPlayer {
  return object(p) && exact(p,playerKeys)
    && [p.hp,p.maxHp,p.mp,p.maxMp].every(v => signed(v,-2147483648,2147483647))
    && [p.gold,p.credit,p.level].every(v => safe(v,0xffff_ffff))
    && [p.experience,p.maxExperience].every(v => signed(v,-SAFE,SAFE))
    && [p.currentWeight,p.maxWeight].every(v => safe(v,65535))
    && typeof p.currentWeightKnown === "boolean" && typeof p.inSafeZone === "boolean"
    && [p.name,p.className,p.gender,p.guildName,p.guildRankName,p.mapName].every(v => v === null || typeof v === "string")
    && [p.hair,p.wingEffect].every(v => v === null || safe(v,255))
    && (p.crystalStats === null || Array.isArray(p.crystalStats) && p.crystalStats.length <= 2048
      && p.crystalStats.every(v => object(v) && exact(v,["stat","value"]) && safe(v.stat,255) && signed(v.value,-2147483648,2147483647)))
    && (p.weights === null || object(p.weights) && exact(p.weights,["bag","wear","hand"])
      && [p.weights.bag,p.weights.wear,p.weights.hand].every(v => safe(v,0xffff_ffff)));
}
function wholeDirectory(shop: NpcGoldBuyShop | null): boolean {
  if (!shop || shop.service_mode !== "buy" || !shop.supports_buy || !Array.isArray(shop.goods)
    || !shop.goods.length || shop.goods.length > 2048) return false;
  const ids = new Set<number>();
  return shop.goods.every(g => {
    if (!object(g) || !required(g,["unique_id","name","price","purchase_rate","requires_gold_buy_plan","use_pearls",
      "count","stock","panel_type","icon","icon_width","icon_height","description","tooltip_source"])
      || !safe(g.unique_id) || ids.has(g.unique_id) || typeof g.name !== "string" || typeof g.description !== "string"
      || !safe(g.price,0xffff_ffff) || !safe(g.count,65535) || !signed(g.stock,-2147483648,-1)
      || g.use_pearls !== false || g.panel_type !== 0 || typeof g.requires_gold_buy_plan !== "boolean"
      || ![g.icon,g.icon_width,g.icon_height].every(v => safe(v,65535))
      || !(g.purchase_rate === null || typeof g.purchase_rate === "number" && Number.isFinite(g.purchase_rate))
      || !(g.tooltip_source === null || object(g.tooltip_source))) return false;
    ids.add(g.unique_id);
    const user = g.tooltip_source && object(g.tooltip_source.userItem) ? g.tooltip_source.userItem : null;
    return g.requires_gold_buy_plan || g.purchase_rate !== null || user?.is_shop_item === true;
  });
}
type InputCapture = { input: NpcShopHostInput; modelKey: string; presentationKey: string; feedback: NpcShopFeedback; coreRevision: string | null };
function captureInput(read: () => NpcShopHostInput): InputCapture | null {
  try {
    const input = JSON.parse(jsonText(read())) as NpcShopHostInput;
    if (!object(input) || ![input.connectionGeneration,input.sessionGeneration,input.serviceRevision,input.catalogRevision].every(positive)
      || !safe(input.ownerRevision) || !positive(input.playerObjectId) || input.playerObjectId > 0xffff_ffff
      || ![input.open,input.showBuy,input.eligible].every(v => typeof v === "boolean") || !playerValid(input.player)
      || !["en","zh-CN","ja"].includes(input.language)) return null;
    const core = input.coreStatus;
    if (core !== null && (!object(core) || !(core.authorityRevision === "0" || decimal(core.authorityRevision))
      || typeof core.canReserve !== "boolean" || !(core.lastPhase === null || phase(core.lastPhase))
      || !(core.flight === null || object(core.flight) && decimal(core.flight.token) && decimal(core.flight.authorityRevision)
        && phase(core.flight.phase)))) return null;
    const coreRevision = core && decimal(core.authorityRevision) ? core.authorityRevision : null;
    const feedback: NpcShopFeedback = { phase:core?.flight?.phase ?? core?.lastPhase ?? null,
      pending:core?.flight != null,canReserve:core?.canReserve ?? false,previousUnknown:0 };
    if (!feedbackValid(feedback)) return null;
    if (input.shop !== null && (!object(input.shop)
      || !required(input.shop,["goods","selected_id","hide_added_stats","selected_bag_slot_for_sell",
        "selected_bag_slot_for_repair","service_mode","supports_buy","supports_sell","repair_rate"])
      || !(input.shop.selected_id === null || safe(input.shop.selected_id))
      || input.shop.selected_bag_slot_for_sell !== null || input.shop.selected_bag_slot_for_repair !== null
      || typeof input.shop.hide_added_stats !== "boolean" || typeof input.shop.supports_buy !== "boolean"
      || typeof input.shop.supports_sell !== "boolean" || input.shop.repair_rate !== null)) return null;
    // Normalize only present, valid local fields. Never repair a partial raw model.
    const shop = input.shop ? { ...input.shop, selected_id:null,selected_bag_slot_for_sell:null,selected_bag_slot_for_repair:null } : null;
    const normalizedInput: NpcShopHostInput = { ...input,shop };
    return { input:normalizedInput, coreRevision, feedback,
      modelKey:jsonText([shop,input.inventory,input.player,input.serviceRevision,input.catalogRevision]),
      presentationKey:jsonText([input.connectionGeneration,input.sessionGeneration,input.ownerRevision,input.playerObjectId,
        input.serviceRevision,input.catalogRevision,input.open,input.showBuy,input.eligible,input.presentation,input.language,coreRevision]) };
  } catch { return null; }
}
function globals() {
  return globalThis as typeof globalThis & { __mir2NpcShopHosts?: WeakMap<NpcShopRuntime,NpcShopHost>; __mir2NpcShopRun?: number };
}
function registry(): WeakMap<NpcShopRuntime,NpcShopHost> { return globals().__mir2NpcShopHosts ??= new WeakMap(); }
function nextRun(runtime: NpcShopRuntime): number | null {
  const global = globals(), prior = global.__mir2NpcShopRun ?? 0;
  let observed = 0;
  try {
    if (runtime.getMir2NpcShopUiStatus) {
      const raw: unknown = JSON.parse(runtime.getMir2NpcShopUiStatus());
      if (!object(raw) || !safe(raw.runGeneration)) return null;
      observed = raw.runGeneration;
    }
  } catch { return null; }
  if (!safe(prior) || Math.max(prior,observed) >= SAFE) return null;
  return global.__mir2NpcShopRun = Math.max(prior,observed) + 1;
}
type Snapshot = NpcShopIdentity & { revision:number;modelRevision:number;presentationRevision:number;serviceRevision:number;
  catalogRevision:number;coreAuthorityRevision:string|null;open:boolean;inputEnabled:boolean;showBuy:boolean;
  presentation:BevyQuestUiPresentation|null;shop:NpcGoldBuyShop;inventory:BevyInventoryModel;player:NpcShopPlayer;
  language:Mir2Language;feedback:NpcShopFeedback };

/** DOM/input ownership only. Selection, quotes and transport attempts stay in Rust. */
export class NpcShopHost {
  private readonly runGeneration: number | null;
  private phase: "prepare" | "arming" | "active" = "prepare";
  private ownerRevision = 0;
  private revision = 0; private modelRevision = 0; private presentationRevision = 0;
  private serviceHigh = 0; private catalogHigh = 0;
  private modelKey = ""; private presentationKey = ""; private sentKey = "";
  private sent: Snapshot | null = null;
  private stopped = false; private attached = false; private busy = false;
  private attachedMethods: unknown[] | null = null;
  private lastFrame = 0; private lastFrameAt = 0; private lastNow = 0; private armingAt = 0;
  private intentSequence = 0;
  private proof: NpcShopIntent | null = null;
  private claimedIntent: NpcShopIntent | null = null;
  private pointerLease: NpcShopPointerEdge | null = null;
  private terminal: NpcShopPointerEdge | null = null;
  private callbackIntent: NpcShopIntent | null = null;
  private deferredWithdraw = false; private deferredStop = false;
  private retirement: NpcShopIntent | null = null;
  private awaiting: { frame: number; at: number } | null = null;
  private fatal: string | null = null;
  private cleanupScheduled = false;
  constructor(private readonly options: NpcShopHostOptions) {
    registry().set(options.runtime,this);
    this.runGeneration = nextRun(options.runtime);
    if (this.runGeneration === null) this.fatal = "Shared NPC host run exhausted";
    const c = captureInput(options.read);
    this.ownerRevision = c?.input.ownerRevision ?? 0;
  }
  private owns(): boolean { return !this.stopped && registry().get(this.options.runtime) === this; }
  private methods(): unknown[] {
    const r = this.options.runtime;
    return [r.getMir2NpcShopUiCapabilities,r.setMir2NpcShopUiSnapshot,r.getMir2NpcShopUiStatus,
      r.setMir2NpcShopUiIntentSink,r.clearMir2NpcShopUiIntentSink,r.setMir2NpcShopUiPointerEdge,r.withdrawMir2NpcShopUiSnapshot];
  }
  private methodsCurrent(): boolean { return Boolean(this.attachedMethods && this.methods().every((m,i) => m === this.attachedMethods![i])); }
  private attach(): boolean {
    if (!this.owns() || !supportsNpcShopUi(this.options.runtime)) return false;
    if (this.attached) return this.methodsCurrent();
    const methods = this.methods();
    this.options.runtime.setMir2NpcShopUiIntentSink!(json => this.intent(json));
    if (!this.owns() || !this.methods().every((m,i) => m === methods[i])) return false;
    this.attached = true; this.attachedMethods = methods; return true;
  }
  private time(): number | null {
    const now = this.options.now();
    if (!Number.isFinite(now) || now < 0 || now < this.lastNow) return null;
    this.lastNow = now; return now;
  }
  private notify(owner: BagOwner): void {
    if (this.owns() && this.runGeneration !== null) this.options.onOwner(owner,this.runGeneration,this.ownerRevision);
  }
  private state(error: string | null = null): void {
    if (this.owns()) this.options.onState({ active:this.phase === "active",transitioning:this.phase === "arming",
      ownerRevision:this.ownerRevision,error:this.fatal ?? error });
  }
  private fallback(error: string | null): void {
    if (!this.owns()) return;
    const changed = this.phase !== "prepare";
    this.phase = "prepare"; this.proof = null; this.pointerLease = null; this.terminal = null; this.awaiting = null;
    if (changed) this.notify("react"); this.state(error);
  }
  private advance(kind: "revision" | "modelRevision" | "presentationRevision"): boolean {
    if (this[kind] >= SAFE) { this.fatal = "Shared NPC host clock exhausted"; return false; }
    ++this[kind]; return true;
  }
  private eligible(c: InputCapture): boolean {
    const i = c.input;
    return i.eligible && i.open && i.showBuy && c.coreRevision !== null && fitsNpcShop(i.presentation)
      && wholeDirectory(i.shop) && validNpcGoldBuyBagModel(i.inventory);
  }
  private sameSource(c: InputCapture): boolean {
    const s = this.sent;
    return Boolean(s && c.modelKey === this.modelKey && c.presentationKey === this.presentationKey
      && c.input.ownerRevision === s.ownerRevision && c.input.connectionGeneration === s.connectionGeneration
      && c.input.sessionGeneration === s.sessionGeneration && c.input.playerObjectId === s.playerObjectId
      && c.input.serviceRevision === s.serviceRevision && c.input.catalogRevision === s.catalogRevision
      && c.coreRevision === s.coreAuthorityRevision);
  }
  private publish(c: InputCapture, enabled: boolean): boolean {
    if (!this.owns() || this.callbackIntent || !this.methodsCurrent() || this.runGeneration === null || !this.eligible(c)
      || !c.input.shop || !c.input.inventory || !this.modelRevision || !this.presentationRevision) return false;
    const i = c.input;
    const next = { runGeneration:this.runGeneration,connectionGeneration:i.connectionGeneration,sessionGeneration:i.sessionGeneration,
      ownerRevision:i.ownerRevision,playerObjectId:i.playerObjectId,modelRevision:this.modelRevision,presentationRevision:this.presentationRevision,
      serviceRevision:i.serviceRevision,catalogRevision:i.catalogRevision,coreAuthorityRevision:c.coreRevision,
      open:true,inputEnabled:enabled,showBuy:true,presentation:i.presentation,shop:i.shop,inventory:i.inventory,player:i.player,
      language:i.language,feedback:c.feedback };
    const key = jsonText(next);
    if (key === this.sentKey) return true;
    if (i.serviceRevision < this.serviceHigh || i.catalogRevision < this.catalogHigh) {
      this.fatal = "Shared NPC service clock regressed"; return false;
    }
    if (!this.advance("revision")) return false;
    const snapshot: Snapshot = deepFreeze({ ...next,revision:this.revision } as Snapshot);
    this.proof = null; this.pointerLease = null; this.terminal = null;
    this.sent = snapshot; this.sentKey = key;
    this.serviceHigh = i.serviceRevision; this.catalogHigh = i.catalogRevision;
    const accepted = this.options.runtime.setMir2NpcShopUiSnapshot!(jsonText(snapshot)) === true;
    if (!accepted || !this.owns() || !this.methodsCurrent() || this.sent !== snapshot) return false;
    const current = captureInput(this.options.read);
    return Boolean(current && this.sameSource(current));
  }
  /** Exact applied snapshot only. No input read, Core observe, attachment or mutation. */
  readStatus(): NpcShopStatus | null {
    if (!this.owns() || !this.methodsCurrent() || !this.sent) return null;
    const s = this.sent, v = readNpcShopStatus(this.options.runtime);
    if (!this.owns() || !this.methodsCurrent() || this.sent !== s || !v || !sameNpcShopIdentity(s,v)
      || v.appliedRevision !== s.revision || v.appliedModelRevision !== s.modelRevision
      || v.appliedPresentationRevision !== s.presentationRevision || v.appliedServiceRevision !== s.serviceRevision
      || v.appliedCatalogRevision !== s.catalogRevision || v.coreAuthorityRevision !== s.coreAuthorityRevision) return null;
    return v;
  }
  private freshStatus(now: number): NpcShopStatus | null {
    const v = this.readStatus();
    if (!v || !v.frame || v.frame < this.lastFrame) return null;
    if (v.frame > this.lastFrame) { this.lastFrame = v.frame; this.lastFrameAt = now; }
    return v.ready && !v.error && paintedRegions(v,this.sent?.presentation ?? null) && now - this.lastFrameAt <= 2000 ? v : null;
  }
  blocksInput(): boolean { return this.owns() && (this.phase !== "prepare" || this.retirement !== null || this.deferredWithdraw || this.deferredStop); }
  tick(): void {
    if (!this.owns() || this.busy || this.callbackIntent) return;
    this.busy = true;
    try {
      if (this.deferredStop) { this.finishStop(); return; }
      if (this.retirement || this.deferredWithdraw) {
        this.retirement = null; this.deferredWithdraw = false; this.withdrawNow();
        return;
      }
      const now = this.time();
      if (now === null) { this.withdrawNow(); this.state("Shared NPC host clock unavailable"); return; }
      if (this.fatal || !this.attach()) { this.withdrawNow(); this.state(this.fatal ?? "Shared NPC ABI 1 is unavailable"); return; }
      const c = captureInput(this.options.read);
      if (!c || !this.eligible(c)) { this.withdrawNow(); return; }
      this.ownerRevision = c.input.ownerRevision;
      if (c.presentationKey !== this.presentationKey) {
        this.fallback(null); this.presentationKey = c.presentationKey;
        if (!this.advance("presentationRevision")) { this.withdrawNow(); return; }
      }
      if (c.modelKey !== this.modelKey) {
        this.fallback(null); this.modelKey = c.modelKey;
        if (!this.advance("modelRevision")) { this.withdrawNow(); return; }
      }
      if (!this.publish(c,this.phase !== "prepare")) { this.withdrawNow(); this.state(this.fatal ?? "Shared NPC rejected the snapshot"); return; }
      const v = this.freshStatus(now);
      if (!v) {
        const raw = this.readStatus();
        if (this.phase === "arming" && now - this.armingAt <= 2000 && !raw?.error) return;
        if (this.phase === "active" && this.awaiting && now - this.awaiting.at <= 2000 && !raw?.error) return;
        this.fallback(raw?.error ?? "Shared NPC is waiting for painted controls");
        this.publish(c,false); return;
      }
      if (this.awaiting && v.frame <= this.awaiting.frame) return;
      this.awaiting = null;
      if (this.phase === "prepare" && !v.inputEnabled && v.inputRegions.length) {
        this.phase = "arming"; this.armingAt = now; this.notify("transition"); this.state();
        if (!this.publish(c,true)) { this.withdrawNow(); this.state("Shared NPC input arming failed"); }
      } else if (this.phase === "arming" && v.inputEnabled && v.inputRegions.length) {
        this.phase = "active"; this.notify("bevy"); this.state();
      } else if (this.phase === "active" && (!v.inputEnabled || !v.inputRegions.length)
        || this.phase === "arming" && now - this.armingAt > 2000) {
        this.fallback("Shared NPC input handoff expired"); this.publish(c,false);
      }
    } catch (error) {
      this.withdrawNow(); this.state(error instanceof Error ? error.message : "Shared NPC unavailable");
    } finally { this.busy = false; }
  }
  pointerContext(): NpcShopPointerContext | null {
    if (!this.owns() || this.fatal || this.phase !== "active" || this.awaiting || this.retirement || this.deferredWithdraw || this.deferredStop
      || !supportsNpcShopUi(this.options.runtime)) return null;
    const c = captureInput(this.options.read), v = this.readStatus(), now = this.time(), s = this.sent;
    if (!c || !this.eligible(c) || !this.sameSource(c) || !v?.ready || !v.inputEnabled || v.error || !s?.presentation
      || !decimal(v.coreAuthorityRevision) || !decimal(v.controlRevision) || !paintedRegions(v,s.presentation) || now === null
      || this.callbackIntent === null && jsonText(c.feedback) !== jsonText(s.feedback)
      || v.frame < this.lastFrame || now - this.lastFrameAt > 2000 || !this.owns() || !this.methodsCurrent()) return null;
    const proof: NpcShopProof = { ...identity(v),revision:v.appliedRevision,modelRevision:v.appliedModelRevision,
      presentationRevision:v.appliedPresentationRevision,serviceRevision:v.appliedServiceRevision,catalogRevision:v.appliedCatalogRevision,
      coreAuthorityRevision:v.coreAuthorityRevision,controlRevision:v.controlRevision };
    return deepFreeze({ proof,presentation:s.presentation,inputRegions:v.inputRegions });
  }
  pointer(edge: NpcShopPointerEdge): boolean {
    if (!this.owns() || !this.methodsCurrent() || !edgeValid(edge)) return false;
    const c = this.pointerContext(), current = Boolean(c && sameProof(c.proof,edgeProof(edge))), lease = this.pointerLease;
    const matching = Boolean(lease && sameProof(edgeProof(lease),edgeProof(edge)) && sameGesture(lease,edge));
    if (edge.phase === "down" && (!current || this.callbackIntent || lease)) return false;
    if (edge.phase !== "down" && !matching && (current || edge.phase === "move")) return false;
    const previousProof = this.proof, previousTerminal = this.terminal, previousLease = this.pointerLease;
    if (edge.phase === "down") { this.proof = null; this.terminal = null; this.pointerLease = deepFreeze({ ...edge }); }
    else if (matching && edge.phase !== "move") {
      this.pointerLease = null;
      if (edge.phase === "up" && current) this.terminal = deepFreeze({ ...edge });
      else { this.terminal = null; this.proof = null; }
    }
    const rpc = !current && edge.phase === "up" ? { ...edge,phase:"cancel" as const } : edge;
    const ownedLease = this.pointerLease, ownedTerminal = this.terminal, ownedProof = this.proof;
    try {
      const accepted = this.options.runtime.setMir2NpcShopUiPointerEdge!(jsonText(rpc)) === true;
      if (!accepted && this.owns()) {
        if (this.pointerLease === ownedLease && this.terminal === ownedTerminal && this.proof === ownedProof) {
          this.pointerLease = previousLease; this.terminal = previousTerminal; this.proof = previousProof;
        }
      }
      return accepted && this.owns() && this.methodsCurrent();
    } catch { this.withdraw(); return false; }
  }
  allows(intent: NpcShopIntent): boolean {
    if (!this.owns() || this.proof !== intent || this.retirement || this.deferredWithdraw || this.deferredStop) return false;
    const c = this.pointerContext(), v = this.readStatus();
    return Boolean(c && v && this.proof === intent && sameProof(c.proof,intent.proof)
      && v.selectedId === intent.selectedId && v.quantity === intent.quantity && v.startIndex === intent.startIndex);
  }
  claim(intent: NpcShopIntent): boolean {
    if (!this.allows(intent)) return false;
    this.proof = null; this.claimedIntent = intent.action.type === "buy" ? intent : null; return true;
  }
  /** Final synchronous UI checkpoint after Page's source/socket checks; no ABI or Core reads. */
  claimCurrent(intent: NpcShopIntent): boolean {
    return this.claimedIntent === intent && this.callbackIntent === intent && intent.action.type === "buy"
      && this.owns() && this.attached && this.methodsCurrent() && this.runGeneration !== null
      && intent.proof.runGeneration === this.runGeneration && this.phase === "active" && !this.fatal
      && !this.awaiting && !this.retirement && !this.deferredWithdraw && !this.deferredStop;
  }
  /** Called only within the current close/Sell sink, before changing Page's service/tab. */
  deferRetirement(intent: NpcShopIntent): boolean {
    if (this.callbackIntent !== intent || !["close","handoffSell"].includes(intent.action.type) || !this.allows(intent)) return false;
    this.retirement = intent; return true;
  }
  private intent(json: string): boolean {
    if (this.callbackIntent || !this.owns() || !this.methodsCurrent() || this.phase !== "active") return false;
    const intent = parseNpcShopIntent(json), c = this.pointerContext(), terminal = this.terminal, status = this.readStatus();
    if (!intent || !c || !status || !terminal || intent.intentSequence <= this.intentSequence
      || !sameProof(intent.proof,c.proof) || !sameProof(intent.proof,edgeProof(terminal))
      || !sameGesture(intent.gesture,terminal) || intent.gesture.sequence !== terminal.sequence
      || intent.selectedId !== status.selectedId || intent.quantity !== status.quantity || intent.startIndex !== status.startIndex) return false;
    this.intentSequence = intent.intentSequence; this.proof = intent; this.callbackIntent = intent;
    let accepted = false;
    try {
      accepted = this.options.onIntent(intent) === true;
      if (!this.owns() || !this.methodsCurrent() || this.deferredWithdraw || this.deferredStop) accepted = false;
      if (accepted && this.retirement !== intent) {
        const current = captureInput(this.options.read);
        if (!current || !this.sameSource(current)) accepted = false;
      }
      if (accepted && this.retirement !== intent) this.awaiting = { frame:status.frame,at:this.lastNow };
      if (!accepted && this.retirement === intent) this.retirement = null;
      return accepted;
    } catch { if (this.retirement === intent) this.retirement = null; return false; }
    finally { this.callbackIntent = null; this.claimedIntent = null; }
  }
  private scheduleCleanup(): void {
    if (this.cleanupScheduled) return;
    this.cleanupScheduled = true;
    // A microtask runs after the entire wasm Update/callback stack, including
    // Last's postcallback checkpoint. Unmount may already have removed our timer.
    void Promise.resolve().then(() => {
      this.cleanupScheduled = false;
      if (!this.owns() || this.callbackIntent) return;
      try {
        if (this.deferredStop) this.finishStop();
        else if (this.deferredWithdraw) { this.deferredWithdraw = false; this.withdrawNow(); }
      } catch {
        this.fatal = "Shared NPC cleanup failed"; this.stopped = true;
        if (registry().get(this.options.runtime) === this) registry().delete(this.options.runtime);
      }
    });
  }
  private withdrawNow(): void {
    if (!this.owns()) return;
    if (this.callbackIntent) { this.deferredWithdraw = true; this.scheduleCleanup(); return; }
    const sent = this.sent;
    this.sent = null; this.sentKey = ""; this.proof = null; this.pointerLease = null; this.terminal = null; this.awaiting = null;
    this.fallback(null);
    if (sent && this.owns() && this.methodsCurrent()) {
      try { this.options.runtime.withdrawMir2NpcShopUiSnapshot!(jsonText(identity(sent))); } catch { /* Exact old surface only. */ }
    }
  }
  withdraw(): void {
    if (!this.owns()) return;
    if (this.callbackIntent) { this.deferredWithdraw = true; this.scheduleCleanup(); return; }
    this.retirement = null; this.deferredWithdraw = false; this.withdrawNow();
  }
  private finishStop(): void {
    if (this.owns()) {
      this.deferredStop = false; this.retirement = null; this.deferredWithdraw = false; this.withdrawNow();
      if (this.owns()) { registry().delete(this.options.runtime);
        if (this.methodsCurrent()) try { this.options.runtime.clearMir2NpcShopUiIntentSink?.(); } catch { /* Retired sink. */ } }
    }
    this.stopped = true;
  }
  stop(): void {
    if (this.stopped) return;
    if (this.callbackIntent && this.owns()) { this.deferredStop = true; this.scheduleCleanup(); return; }
    this.finishStop();
  }
}

/** A captured down origin never becomes an NPC gesture after crossing the panel. */
export class NpcShopPointerRouter {
  private sequence = 0; private lease: NpcShopPointerEdge | null = null;
  private quarantine = new Map<number,NpcShopPointerEdge>();
  private currentTerminal: NpcShopPointerEdge | null = null;
  get held(): NpcShopPointerEdge | null { return this.lease; }
  private next(): number | null { return this.sequence < SAFE ? ++this.sequence : null; }
  down(c: NpcShopPointerContext,pointerId:number,button:0|2,x:number,y:number): NpcShopPointerEdge | null {
    if (this.lease || !proofValid(c.proof) || !safe(pointerId) || (button !== 0 && button !== 2)
      || ![x,y].every(v => Number.isFinite(v) && Math.abs(v) <= 16384)) return null;
    const sequence = this.next(); if (sequence === null) return null;
    this.currentTerminal = null;
    // Keep a canceled down until its late terminal is quarantined, even if
    // the browser reuses that pointer id for a new down first.
    const origin = c.inputRegions.some(r => x >= r.left && y >= r.top && x < r.left+r.width && y < r.top+r.height) ? "shop" : "world";
    return this.lease = deepFreeze({ ...c.proof,pointerId,button,x,y,origin,sequence,downSequence:sequence,phase:"down" });
  }
  edge(phase:"move"|"up"|"cancel",pointerId:number,x:number,y:number): NpcShopPointerEdge | null {
    if (![x,y].every(v => Number.isFinite(v) && Math.abs(v) <= 16384) || !safe(pointerId)) return null;
    if (phase !== "move") this.currentTerminal = null;
    const retired = this.quarantine.get(pointerId);
    if (phase !== "move" && retired) {
      const sequence = this.next(); if (sequence === null) return null;
      this.quarantine.delete(pointerId); return deepFreeze({ ...retired,phase,sequence,x,y });
    }
    if (!this.lease || this.lease.pointerId !== pointerId) return null;
    const sequence = this.next(); if (sequence === null) return null;
    const edge = deepFreeze({ ...this.lease,phase,sequence,x,y });
    this.lease = phase === "move" ? edge : null;
    if (phase !== "move") this.currentTerminal = edge;
    return edge;
  }
  cancel(phase:"cancel"|"blur"="cancel"): NpcShopPointerEdge | null {
    if (!this.lease) return null;
    const sequence = this.next(); if (sequence === null) return null;
    const lease = this.lease; this.quarantine.set(lease.pointerId,lease); this.lease = null;
    const edge = deepFreeze({ ...lease,phase,sequence });
    this.currentTerminal = edge; return edge;
  }
  /** Recover only the exact consumed current terminal; never cancel a newer lease. */
  cancelTerminal(edge: NpcShopPointerEdge): NpcShopPointerEdge | null {
    if (this.currentTerminal !== edge || this.lease) return null;
    this.currentTerminal = null;
    const sequence = this.next(); if (sequence === null) return null;
    return deepFreeze({ ...edge,phase:"cancel" as const,sequence });
  }
  matches(c:NpcShopPointerContext|null): boolean { return Boolean(this.lease && c && sameProof(edgeProof(this.lease),c.proof)); }
}
