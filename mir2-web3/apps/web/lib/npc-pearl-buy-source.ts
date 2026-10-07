import { crystalItemSourceJson, crystalUserItemFields, parseCrystalItemTooltipSource, sameCrystalItemData } from "./crystal-item-source";

/** Raw NPCPearlGoods and signed creature-wallet custody. No purchase arithmetic. */
export type NpcPearlPhysicalOwner = Readonly<{ socket: object; connectionGeneration: number;
  sessionGeneration: number; playerObjectId: number; sceneRevision: number; mapFileName: string }>;
export type NpcPearlGood = Readonly<{ uniqueId: number; itemIndex: number; name: string; icon: number;
  unitPrice: number; infoPrice: number; count: number; stock: -1; tooltipSource: unknown; description: string }>;
export type NpcPearlCatalog = Readonly<{ revision: number; panelType: 0; rate: number;
  goods: readonly NpcPearlGood[]; rawPayload: Readonly<Record<string,unknown>> }>;
export type NpcPearlWallet = Readonly<{ revision: number; amount: number }>;

// The existing animation snapshot bridge admits 2 MiB / 2048 entities. A
// whole Trade catalogue uses that envelope; individual raw sources retain the
// stricter tooltip 262144-byte / 8192-node / 256-array limits below it.
export function npcPearlJson(value: unknown): string {
  // Gateway/Core f32 rates preserve -0.0 as valid zero; no other raw field does.
  return crystalItemSourceJson(value, 2_097_152, 262144, 2048, true);
}

const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const integer = (v: unknown, lo: number, hi: number): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= lo && v <= hi;
function ownerValid(owner: NpcPearlPhysicalOwner | null): owner is NpcPearlPhysicalOwner {
  return !!owner && !!owner.socket && typeof owner.socket === 'object'
    && integer(owner.connectionGeneration, 1, Number.MAX_SAFE_INTEGER) && integer(owner.sessionGeneration, 1, Number.MAX_SAFE_INTEGER)
    && integer(owner.playerObjectId, 1, 0xffff_ffff) && integer(owner.sceneRevision, 0, Number.MAX_SAFE_INTEGER)
    && typeof owner.mapFileName === 'string' && owner.mapFileName.length > 0;
}
function samePersonal(a: NpcPearlPhysicalOwner, b: NpcPearlPhysicalOwner): boolean {
  return a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.playerObjectId === b.playerObjectId;
}
function sameScene(a: NpcPearlPhysicalOwner, b: NpcPearlPhysicalOwner): boolean {
  return samePersonal(a, b) && a.sceneRevision === b.sceneRevision && a.mapFileName === b.mapFileName;
}
export type NpcPearlUnitPriceValidator = (infoPrice: number, rate: number, projectedPrice: number) => boolean;
const rowFields = new Set<string>([...crystalUserItemFields, "id", "uniqueId", "itemIndex", "purchaseItemIndex", "name", "icon", "price", "tooltipSource", "grade", "description"]);
const boundedText = (v: unknown, max: number): v is string => typeof v === "string"
  && new TextEncoder().encode(v).length <= max && !v.includes("\0");
function freezeCatalogRaw<T>(value: T): T {
  if (value !== null && typeof value === "object") {
    for (const child of Object.values(value)) freezeCatalogRaw(child);
    Object.freeze(value);
  }
  return value;
}
function catalogGoods(payload: unknown, validateUnitPrice: NpcPearlUnitPriceValidator,
  current: () => boolean): { rawPayload: Readonly<Record<string,unknown>>; rate: number; goods: readonly NpcPearlGood[] } | null {
  try {
    const rawJson = npcPearlJson(payload), raw: unknown = JSON.parse(rawJson);
    if (!current() || !record(raw) || Object.keys(raw).length !== 3
      || Object.keys(raw).some(key => !["panelType", "rate", "list"].includes(key))
      || raw.panelType !== 0 || typeof raw.rate !== "number" || !Number.isFinite(raw.rate)
      || raw.rate < 0 || !Number.isFinite(Math.fround(raw.rate)) || !Array.isArray(raw.list)) return null;
    const ids = new Set<number>(), goods: NpcPearlGood[] = [];
    for (const row of raw.list) {
      if (!record(row) || Object.keys(row).some(key => !rowFields.has(key))
        || Object.hasOwn(row, "purchaseItemIndex") && (typeof row.purchaseItemIndex !== "string"
          || !/^(?:0|[1-9][0-9]{0,19})$/.test(row.purchaseItemIndex) || BigInt(row.purchaseItemIndex) > 18446744073709551615n)
        || !integer(row.unique_id, 0, Number.MAX_SAFE_INTEGER) || row.id !== row.unique_id
        || row.uniqueId !== row.unique_id || !integer(row.item_index, -2147483648, 2147483647)
        || row.itemIndex !== row.item_index || !integer(row.price, 0, 0xffff_ffff)
        || !integer(row.count, 1, 65535) || !integer(row.icon, 0, 0xffff_ffff)
        || !boundedText(row.name, 512) || row.name.length === 0 || ids.has(row.unique_id)
        || !integer(row.grade, 0, 255) || "description" in row && !boundedText(row.description, 8192)) return null;
      const source = parseCrystalItemTooltipSource(row.tooltipSource,
        { uniqueId: row.unique_id, itemIndex: row.item_index, count: row.count });
      if (!source || !record(source.info) || !record(source.userItem)
        || row.name !== source.info.name || row.grade !== source.info.grade
        || crystalUserItemFields.some(key => !Object.prototype.hasOwnProperty.call(row, key)
          || !sameCrystalItemData(row[key], (source.userItem as Record<string, unknown>)[key]))) return null;
      const infoPrice = source.info.price;
      if (!integer(infoPrice, 0, 0xffff_ffff) || validateUnitPrice(infoPrice, raw.rate, row.price) !== true || !current()) return null;
      ids.add(row.unique_id);
      // NPCPearlGoods is the typed Trade catalogue: unlimited stock is a
      // protocol fact on this packet route, never a missing-field default.
      goods.push(Object.freeze({ uniqueId: row.unique_id, itemIndex: row.item_index, name: row.name,
        icon: row.icon, unitPrice: row.price, infoPrice, count: row.count, stock: -1, tooltipSource: source,
        description: typeof row.description === "string" ? row.description : "" }));
    }
    return current() ? { rawPayload:freezeCatalogRaw(raw), rate: raw.rate, goods: Object.freeze(goods) } : null;
  } catch { return null; }
}

/** Wallet lifetime is personal; catalogue lifetime additionally follows the scene/service. */
export class NpcPearlShopSource {
  private catalogEpoch = 0;
  private walletEpoch = 0;
  private catalog: { owner: NpcPearlPhysicalOwner; value: NpcPearlCatalog } | null = null;
  private wallet: { owner: NpcPearlPhysicalOwner; value: NpcPearlWallet } | null = null;
  retireCatalog(): void { this.catalog = null; if (this.catalogEpoch < Number.MAX_SAFE_INTEGER) ++this.catalogEpoch; }
  retireWallet(): void { this.wallet = null; if (this.walletEpoch < Number.MAX_SAFE_INTEGER) ++this.walletEpoch; }
  retire(): void { this.retireCatalog(); this.retireWallet(); }
  observeCatalog(owner: NpcPearlPhysicalOwner | null, packet: string, payload: unknown,
    validateUnitPrice?: NpcPearlUnitPriceValidator): NpcPearlCatalog | null {
    this.retireCatalog();
    const epoch = this.catalogEpoch;
    if (!ownerValid(owner) || packet !== 'NPCPearlGoods' || epoch >= Number.MAX_SAFE_INTEGER
      || typeof validateUnitPrice !== "function") return null;
    const capturedOwner = Object.freeze({ ...owner });
    const current = () => this.catalogEpoch === epoch && this.catalog === null
      && ownerValid(owner) && sameScene(capturedOwner, owner);
    const parsed = catalogGoods(payload, validateUnitPrice, current);
    if (!parsed || !current()) return null;
    const value = Object.freeze({ revision: epoch, panelType: 0 as const, ...parsed });
    this.catalog = { owner: capturedOwner, value };
    return value;
  }
  observeWallet(owner: NpcPearlPhysicalOwner | null, amount: unknown): NpcPearlWallet | null {
    this.retireWallet();
    if (!ownerValid(owner) || !integer(amount, -2147483648, 2147483647) || this.walletEpoch >= Number.MAX_SAFE_INTEGER) return null;
    const value = Object.freeze({ revision: this.walletEpoch, amount });
    this.wallet = { owner: { ...owner }, value };
    return value;
  }
  currentCatalog(owner: NpcPearlPhysicalOwner | null): NpcPearlCatalog | null {
    return ownerValid(owner) && this.catalog && sameScene(this.catalog.owner, owner) ? this.catalog.value : null;
  }
  currentWallet(owner: NpcPearlPhysicalOwner | null): NpcPearlWallet | null {
    return ownerValid(owner) && this.wallet && samePersonal(this.wallet.owner, owner) ? this.wallet.value : null;
  }
}
