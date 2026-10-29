import { sameSocialReplyOwner, type SocialReplyOwner } from "./social-incoming-replies";
import { exactSocialServiceI64, sameSocialServiceStream, validSocialServiceOwner } from "./creature-player-ui";

export type CashGameShopPayment = "gold" | "credit";
export type CashGameShopEntry = Readonly<{
  item_index: number; game_shop_index: number; item_name: string; gold_price: number; credit_price: number;
  count: number; item_shape: number; item_stack_size: number; class: string; category: string; stock: number;
  stock_level: number; individual_stock: boolean; deal: boolean; top_item: boolean; date_binary_datetime: string;
  can_buy_credit: boolean; can_buy_gold: boolean; info: Readonly<Record<string, unknown> & { image: number; item_type: number }>;
}>;
export type CashGameShopCatalog = Readonly<{
  entries: readonly CashGameShopEntry[]; pendingStock: readonly Readonly<{ gIndex: number; stockLevel: number }>[];
}>;
export type CashGameShopSource = Readonly<{
  owner: SocialReplyOwner; revision: number; entries: readonly CashGameShopEntry[];
  wallet: Readonly<{ gold: number; credit: number; className: string }>; receiptEnabled: boolean;
}>;
export type CashGameShopConfirmation = Readonly<{
  source: CashGameShopSource; gIndex: number; quantity: number; payment: CashGameShopPayment;
  itemName: string; count: number; total: number;
}>;
export type CashGameShopCommand = Readonly<{
  type: "gameShopBuy"; gIndex: number; quantity: number; priceType: 0 | 1; requestId: string;
}>;
export type CashGameShopProof = Readonly<{ confirmation: CashGameShopConfirmation; command: CashGameShopCommand }>;
export type CashGameShopReceipt = Readonly<{
  protocol: "nativeGameShopReceiptV1"; requestId: string; success: boolean; gIndex: number; quantity: number;
  priceType: 0 | 1; newStockLevel: number | null; mailId: number | null; code: string | null;
}>;
function object(v: unknown): v is Record<string, unknown> { return !!v && typeof v === "object" && !Array.isArray(v); }
function int(v: unknown, min: number, max: number): v is number {
  return typeof v === "number" && Number.isSafeInteger(v) && v >= min && v <= max;
}
function printable(v: unknown, max: number): v is string {
  return typeof v === "string" && v.length > 0 && v.length <= max && /^[\x20-\x7e]+$/.test(v);
}
function text(v: unknown, max: number): v is string { return typeof v === "string" && v.length <= max && !v.includes("\0"); }
function freezeJson(v: unknown, depth = 0): unknown {
  if (depth > 16) throw new Error("metadata depth");
  if (Array.isArray(v)) {
    if (v.length > 256) throw new Error("metadata array");
    return Object.freeze(v.map(x => freezeJson(x, depth + 1)));
  }
  if (object(v)) {
    if (Object.keys(v).length > 64) throw new Error("metadata fields");
    return Object.freeze(Object.fromEntries(Object.entries(v).map(([k,x]) => [k, freezeJson(x, depth + 1)])));
  }
  if (typeof v === "string" && v.length > 8192) throw new Error("metadata text");
  if (typeof v === "number" && !Number.isFinite(v)) throw new Error("metadata number");
  if (v !== null && !["string", "number", "boolean"].includes(typeof v)) throw new Error("metadata value");
  return v;
}
/** Exact GameShopInfo payload: gateway/web.rs:12123; nested GameShopItem is snake_case. */
export function readCashGameShopInfo(payload: unknown): CashGameShopEntry | null {
  if (!object(payload) || !object(payload.item) || !int(payload.stockLevel, 0, 2147483647)) return null;
  const item = payload.item, info = item.info;
  if (!object(info) || !int(item.g_index, 0, 2147483647) || !int(item.item_index, -2147483648, 2147483647)
    || info.item_index !== item.item_index || !text(info.name, 512) || !info.name || !int(info.image, 0, 65535)
    || !int(info.item_type, 0, 255) || !int(info.shape, -32768, 32767) || !int(info.stack_size, 0, 65535)
    || !int(item.gold_price, 0, 4294967295) || !int(item.credit_price, 0, 4294967295)
    || !int(item.count, 1, 65535) || !text(item.class, 256) || !text(item.category, 256) || !int(item.stock, 0, 2147483647)
    || [item.i_stock, item.deal, item.top_item, item.can_buy_credit, item.can_buy_gold].some(x => typeof x !== "boolean")) return null;
  const date = exactSocialServiceI64(item.date_binary_datetime);
  if (date === null) return null;
  try {
    return Object.freeze({ item_index: item.item_index, game_shop_index: item.g_index, item_name: info.name,
      gold_price: item.gold_price, credit_price: item.credit_price, count: item.count, item_shape: info.shape,
      item_stack_size: info.stack_size, class: item.class, category: item.category, stock: item.stock,
      stock_level: payload.stockLevel, individual_stock: item.i_stock as boolean, deal: item.deal as boolean,
      top_item: item.top_item as boolean, date_binary_datetime: date, can_buy_credit: item.can_buy_credit as boolean,
      can_buy_gold: item.can_buy_gold as boolean, info: freezeJson(info) as CashGameShopEntry["info"] });
  } catch { return null; }
}
export function emptyCashGameShopCatalog(): CashGameShopCatalog {
  return Object.freeze({ entries: Object.freeze([]), pendingStock: Object.freeze([]) });
}
export function upsertCashGameShopInfo(catalog: CashGameShopCatalog, payload: unknown): CashGameShopCatalog | null {
  let entry = readCashGameShopInfo(payload);
  if (!entry) return null;
  const patch = catalog.pendingStock.find(p => p.gIndex === entry!.game_shop_index);
  if (patch) entry = Object.freeze({ ...entry, stock_level: patch.stockLevel });
  const entries = catalog.entries.filter(e => e.game_shop_index !== entry!.game_shop_index);
  if (entries.length >= 512) return null;
  entries.push(entry);
  return Object.freeze({ entries: Object.freeze(entries.sort((a,b) => a.game_shop_index - b.game_shop_index)),
    pendingStock: Object.freeze(catalog.pendingStock.filter(p => p.gIndex !== entry!.game_shop_index)) });
}
/** GameShopStock's typed payload is {gIndex,stockLevel,typed:true}; no alias guesses. */
export function applyCashGameShopStock(catalog: CashGameShopCatalog, payload: unknown): CashGameShopCatalog | null {
  if (!object(payload) || !int(payload.gIndex, 0, 2147483647) || !int(payload.stockLevel, 0, 2147483647)
    || Object.keys(payload).some(k => !["gIndex", "stockLevel", "typed"].includes(k))
    || Object.hasOwn(payload, "typed") && payload.typed !== true) return null;
  const { gIndex, stockLevel } = payload;
  if (catalog.entries.some(e => e.game_shop_index === gIndex)) {
    return Object.freeze({ entries: Object.freeze(catalog.entries.map(e => e.game_shop_index === gIndex
      ? Object.freeze({ ...e, stock_level: stockLevel }) : e)), pendingStock: catalog.pendingStock });
  }
  const patches = catalog.pendingStock.filter(p => p.gIndex !== gIndex);
  if (patches.length >= 512) patches.shift();
  patches.push(Object.freeze({ gIndex, stockLevel }));
  return Object.freeze({ entries: catalog.entries, pendingStock: Object.freeze(patches) });
}
export function makeCashGameShopSource(catalog: CashGameShopCatalog, wallet: unknown, owner: SocialReplyOwner | null,
  revision: number, receiptEnabled: boolean): CashGameShopSource | null {
  if (!validSocialServiceOwner(owner) || !int(revision, 0, Number.MAX_SAFE_INTEGER) || !object(wallet)
    || !int(wallet.gold, 0, 4294967295) || !int(wallet.credit, 0, 4294967295)
    || typeof wallet.className !== "string" || !/^(warrior|wizard|taoist|assassin|archer)$/i.test(wallet.className)
    || typeof receiptEnabled !== "boolean" || catalog.entries.length > 512) return null;
  return Object.freeze({ owner: Object.freeze({ ...owner }), revision, entries: catalog.entries,
    wallet: Object.freeze({ gold: wallet.gold, credit: wallet.credit, className: wallet.className }), receiptEnabled });
}
export function cashGameShopQuantityLimit(item: CashGameShopEntry): number {
  const stacks = Math.min(99, Math.floor(5 * item.item_stack_size / item.count));
  return item.stock === 0 ? stacks : Math.min(stacks, item.stock_level);
}
export function cashGameShopSourceKey(source: CashGameShopSource): string {
  return JSON.stringify([source.revision, source.entries, source.wallet, source.receiptEnabled]);
}
/** Native game_shop_dialog.rs:507–536 / 810–866; confirmation has no send side effect. */
export function makeCashGameShopConfirmation(source: CashGameShopSource | null, gIndex: number, quantity: number,
  payment: CashGameShopPayment): CashGameShopConfirmation | null {
  if (!source || !source.receiptEnabled || !validSocialServiceOwner(source.owner) || !int(quantity, 1, 99)
    || payment !== "gold" && payment !== "credit") return null;
  const item = source.entries.find(e => e.game_shop_index === gIndex);
  if (!item || quantity > cashGameShopQuantityLimit(item)) return null;
  const allowedClass = item.class.trim().toLowerCase();
  if (allowedClass && allowedClass !== "all" && allowedClass !== "show all"
    && allowedClass !== source.wallet.className.toLowerCase()) return null;
  const price = payment === "gold" ? item.can_buy_gold && item.gold_price : item.can_buy_credit && item.credit_price;
  if (typeof price !== "number" || price <= 0) return null;
  const total = price * quantity;
  if (!int(total, 1, 4294967295) || total > (payment === "gold" ? source.wallet.gold : source.wallet.credit)) return null;
  return Object.freeze({ source, gIndex, quantity, payment, itemName: item.item_name, count: item.count, total });
}
export function cashGameShopConfirmationCurrent(confirmation: CashGameShopConfirmation, source: CashGameShopSource | null): boolean {
  if (!source || !sameSocialReplyOwner(confirmation.source.owner, source.owner)
    || cashGameShopSourceKey(confirmation.source) !== cashGameShopSourceKey(source)) return false;
  const next = makeCashGameShopConfirmation(source, confirmation.gIndex, confirmation.quantity, confirmation.payment);
  return !!next && next.itemName === confirmation.itemName && next.count === confirmation.count && next.total === confirmation.total;
}
/** ui-core/game_shop.rs:124–165. Mail/chat/wallet updates cannot match this receipt. */
export function readCashGameShopReceipt(raw: unknown): CashGameShopReceipt | null {
  if (!object(raw) || Object.keys(raw).some(k => !["type", "protocol", "requestId", "success", "gIndex", "quantity",
    "priceType", "newStockLevel", "mailId", "code"].includes(k))
    || Object.hasOwn(raw, "type") && raw.type !== "gameShopReceipt" || raw.protocol !== "nativeGameShopReceiptV1"
    || !printable(raw.requestId, 64) || typeof raw.success !== "boolean" || !int(raw.gIndex, 0, 2147483647)
    || !int(raw.quantity, 1, 99) || raw.priceType !== 0 && raw.priceType !== 1) return null;
  const stock = raw.newStockLevel ?? null, mail = raw.mailId ?? null, code = raw.code ?? null;
  if (stock !== null && !int(stock, 0, 2147483647)) return null;
  if (raw.success) { if (code !== null || !int(mail, 0, Number.MAX_SAFE_INTEGER)) return null; }
  else if (mail !== null || !printable(code, 64) || code !== "stockUnavailable" && stock !== null) return null;
  return Object.freeze({ protocol: raw.protocol, requestId: raw.requestId, success: raw.success, gIndex: raw.gIndex,
    quantity: raw.quantity, priceType: raw.priceType, newStockLevel: stock as number | null,
    mailId: mail as number | null, code: code as string | null });
}
export class CashGameShopPurchases {
  private row: { proof: CashGameShopProof; entered: boolean; unknown: boolean } | null = null;
  private usedIds = new Set<string>();
  pending(): CashGameShopProof | null { return this.row?.proof ?? null; }
  reserve(confirmation: CashGameShopConfirmation, source: CashGameShopSource | null, requestId: string): CashGameShopProof | null {
    if (this.row || !printable(requestId, 64) || this.usedIds.has(requestId)
      || !cashGameShopConfirmationCurrent(confirmation, source)) return null;
    const command = Object.freeze({ type: "gameShopBuy" as const, gIndex: confirmation.gIndex, quantity: confirmation.quantity,
      priceType: (confirmation.payment === "credit" ? 0 : 1) as 0 | 1, requestId });
    const proof = Object.freeze({ confirmation, command });
    this.usedIds.add(requestId); this.row = { proof, entered: false, unknown: false }; return proof;
  }
  allows(proof: CashGameShopProof, source: CashGameShopSource | null, command: unknown): boolean {
    return this.row?.proof === proof && !this.row.entered && cashGameShopConfirmationCurrent(proof.confirmation, source)
      && object(command) && Object.keys(command).length === 5
      && Object.entries(proof.command).every(([k,v]) => command[k] === v);
  }
  claim(proof: CashGameShopProof, source: CashGameShopSource | null, command: unknown): boolean {
    if (!this.allows(proof, source, command)) return false;
    this.row!.entered = true; return true;
  }
  cancelDefinitelyUnsent(proof: CashGameShopProof): boolean {
    if (this.row?.proof !== proof || this.row.entered) return false;
    this.row = null; return true;
  }
  markUnknown(proof: CashGameShopProof): void { if (this.row?.proof === proof && this.row.entered) this.row.unknown = true; }
  applyReceipt(raw: unknown, owner: SocialReplyOwner | null): Readonly<{ matched: boolean; receipt: CashGameShopReceipt | null }> {
    const receipt = readCashGameShopReceipt(raw), row = this.row;
    if (!receipt || !row?.entered || !sameSocialServiceStream(row.proof.confirmation.source.owner, owner)
      || ["requestId", "gIndex", "quantity", "priceType"].some(k => receipt[k as keyof CashGameShopReceipt]
        !== row.proof.command[k as keyof CashGameShopCommand])) return Object.freeze({ matched: false, receipt: null });
    this.row = null; return Object.freeze({ matched: true, receipt });
  }
  retireConnection(socket: object, connectionGeneration: number): boolean {
    if (!this.row) return true;
    const before = this.row.proof.confirmation.source.owner;
    if (!object(socket) || socket === before.socket || !int(connectionGeneration, before.connectionGeneration + 1, Number.MAX_SAFE_INTEGER)) return false;
    this.row = null; return true;
  }
}
