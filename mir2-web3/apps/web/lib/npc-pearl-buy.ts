import type { BevyInventoryModel } from './bevy-bag-model';
import type { NpcGoldBuyOwner, NpcGoldBuyPresentation, NpcGoldBuyQuote } from './bevy-npc-shop-buy';
import { npcPearlJson, type NpcPearlCatalog, type NpcPearlWallet } from './npc-pearl-buy-source';

export type NpcPearlBuyInput = Readonly<{ allowsBuy: boolean; selected: boolean; usePearls: boolean;
  uniqueId: number; unitPrice: number; stock: number; quantity: number;
  walletKnown: boolean; pearls: number; occupied: number; infoPrice: number; rate: number }>;
export type NpcPearlBuyPlan = Readonly<{ maxQuantity: 99; quote: number | null; admittedCount: number | null; denial: number }>;
export type NpcPearlBuyCurrent = Readonly<{ currency: 'pearls'; owner: NpcGoldBuyOwner;
  serviceRevision: number; catalogRevision: number; presentation: NpcGoldBuyPresentation;
  catalog: NpcPearlCatalog; wallet: NpcPearlWallet | null; selectedId: number | null;
  inventory: BevyInventoryModel; allowsBuy: boolean; blocked: boolean }>;
export type NpcPearlBuyQuote = NpcGoldBuyQuote & Readonly<{ currency: 'pearls'; totalPearls: number | null }>;
export type NpcPearlBuyCapture = { currency: 'pearls'; requestJson: string; contextJson: string;
  authorityJson: string; selectedId: number | null; quantity: number; blocked: boolean };
const integer = (v: unknown, lo: number, hi: number): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= lo && v <= hi;
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const exact = (v: unknown, names: readonly string[]): v is Record<string, unknown> => record(v)
  && Object.keys(v).sort().join('|') === [...names].sort().join('|');
export function npcPearlBuyInput(json: string): NpcPearlBuyInput | null {
  try {
    const v: unknown = JSON.parse(json);
    if (!exact(v, ['allowsBuy','selected','usePearls','uniqueId','unitPrice','stock','quantity','walletKnown','pearls','occupied','infoPrice','rate'])
      || typeof v.allowsBuy !== 'boolean' || typeof v.selected !== 'boolean' || typeof v.usePearls !== 'boolean'
      || typeof v.walletKnown !== 'boolean' || !integer(v.uniqueId,0,Number.MAX_SAFE_INTEGER)
      || !integer(v.unitPrice,0,0xffff_ffff) || !integer(v.stock,-2147483648,2147483647)
      || !integer(v.quantity,0,65535) || !integer(v.pearls,-2147483648,2147483647)
      || !integer(v.occupied,0,0xffff_ffff) || !integer(v.infoPrice,0,0xffff_ffff)
       || typeof v.rate !== 'number' || !Number.isFinite(v.rate) || v.rate < 0 || !Number.isFinite(Math.fround(v.rate))) return null;
    return Object.freeze(v) as NpcPearlBuyInput;
  } catch { return null; }
}
export function parseNpcPearlBuyPlan(value: unknown): NpcPearlBuyPlan | null {
  if (!exact(value,['version','ok','maxQuantity','quote','admittedCount','denial'])
    || value.version !== 1 || value.ok !== true || value.maxQuantity !== 99
    || !(value.quote === null || integer(value.quote,0,0xffff_ffff))
    || !(value.admittedCount === null || integer(value.admittedCount,1,99))
    || !integer(value.denial,0,7) || (value.denial === 0) !== (value.admittedCount !== null)
    || value.denial === 0 && value.quote === null) return null;
  return Object.freeze({ maxQuantity: 99, quote: value.quote, admittedCount: value.admittedCount, denial: value.denial });
}
export function captureNpcPearlBuy(current: NpcPearlBuyCurrent | null, quantity: number): NpcPearlBuyCapture | null {
  try {
    if (!current || !integer(quantity,0,65535)) return null;
    npcPearlJson(current);
    const owner = current.owner;
    if (current.currency !== 'pearls' || !exact(owner,['connectionGeneration','sessionGeneration','ownerRevision','playerObjectId'])
      || !integer(owner.connectionGeneration,1,Number.MAX_SAFE_INTEGER) || !integer(owner.sessionGeneration,1,Number.MAX_SAFE_INTEGER)
      || !integer(owner.ownerRevision,0,Number.MAX_SAFE_INTEGER) || !integer(owner.playerObjectId,1,0xffff_ffff)
      || !integer(current.serviceRevision,1,Number.MAX_SAFE_INTEGER) || !integer(current.catalogRevision,1,Number.MAX_SAFE_INTEGER)
      || current.catalog.revision !== current.catalogRevision || current.catalog.panelType !== 0
      || typeof current.allowsBuy !== 'boolean' || typeof current.blocked !== 'boolean'
      || !Array.isArray(current.inventory.items) || !Array.isArray(current.catalog.goods)
      || current.wallet !== null && (!integer(current.wallet.revision,1,Number.MAX_SAFE_INTEGER)
        || !integer(current.wallet.amount,-2147483648,2147483647))) return null;
    const good = current.catalog.goods.find(row => row.uniqueId === current.selectedId);
    if (current.selectedId !== null && !good) return null;
    const occupied = current.inventory.items.filter(row => row.container === 0).length;
    // Zero scalar placeholders are ignored under their explicit unknown flags.
    // They never grant a wallet, selected item, or source capability.
    const requestJson = npcPearlJson({ allowsBuy:current.allowsBuy, selected:!!good, usePearls:true,
      uniqueId:good?.uniqueId ?? 0, unitPrice:good?.unitPrice ?? 0, stock:good?.stock ?? -1,
      quantity, walletKnown:current.wallet !== null, pearls:current.wallet?.amount ?? 0, occupied,
       infoPrice:good?.infoPrice ?? 0,rate:current.catalog.rate });
    if (!npcPearlBuyInput(requestJson)) return null;
    return { currency:'pearls',requestJson, contextJson:npcPearlJson({ owner,serviceRevision:current.serviceRevision,
      catalogRevision:current.catalogRevision,presentation:current.presentation,wallet:current.wallet }),
      authorityJson:npcPearlJson({ currency:'pearls',owner:{connectionGeneration:owner.connectionGeneration,
        sessionGeneration:owner.sessionGeneration,playerObjectId:owner.playerObjectId},serviceRevision:current.serviceRevision,
        catalogRevision:current.catalogRevision,catalog:current.catalog,wallet:current.wallet,inventory:current.inventory }),
      selectedId:good?.uniqueId ?? null,quantity,blocked:current.blocked };
  } catch { return null; }
}
const denials = ['', 'serviceUnavailable','missingSelection','notPearl','unknownWallet','stock','pearls','bagFull'] as const;
export function invokeNpcPearlBuy(runtime: object, planner:(json:string)=>string, input:NpcPearlBuyCapture): NpcPearlBuyQuote | null {
  try {
    const raw = planner.call(runtime,input.requestJson);
    if (typeof raw !== 'string' || raw.length > 1024) return null;
    const plan = parseNpcPearlBuyPlan(JSON.parse(raw));
    if (!plan) return null;
    const canBuy = plan.denial === 0;
    if (canBuy && !integer(input.selectedId,0,Number.MAX_SAFE_INTEGER)) return null;
    return Object.freeze({ currency:'pearls',maxQuantity:plan.maxQuantity,totalGold:null,totalPearls:plan.quote,
      canBuy,blockReason:canBuy ? null : denials[plan.denial],command:canBuy ? Object.freeze({
        type:'buyItem' as const,itemIndex:input.selectedId!,count:plan.admittedCount!,panelType:0 as const }) : null });
  } catch { return null; }
}
