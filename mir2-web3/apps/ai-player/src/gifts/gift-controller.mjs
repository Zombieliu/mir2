import { DatabaseSync } from "node:sqlite";
import { createHash, randomUUID } from "node:crypto";
import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

const uint = value => Number.isSafeInteger(value) && value >= 0;
const gold = value => uint(value) && value <= 0xffff_ffff;
const name = value => typeof value === "string" && value.length > 0 && value.length <= 64 && !/[\p{Cc}\p{Cf}\p{Cs}]/u.test(value);
const terminal = value => ["completed", "cancelled"].includes(value);
const uid = value => typeof value === "bigint" && value >= 0n && value <= 18446744073709551615n || uint(value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const source = (state, client) => ({ worldEpoch: client.worldEpoch, characterIndex: client.activeCharacterIndex, connectionEpoch: state.connectionEpoch,
  characterEpoch: state.characterEpoch, playerObjectId: state.playerObjectId, playerName: state.player?.name, mapFileName: state.mapFileName });
const matches = (value, state) => value && value.connectionEpoch === state.connectionEpoch && value.characterEpoch === state.characterEpoch && value.playerObjectId === state.playerObjectId && value.playerName === state.player?.name && value.mapFileName === state.mapFileName;
const usable = state => state?.ready === true && state.player?.dead !== true && state.playerHp !== 0 && gold(state.gold) && state.inventoryStale === false &&
  state.fullSnapshot?.inventoryComplete === true && state.fullSnapshot?.tradeKnown === true && matches(state.fullSnapshot, state);
const zeroOutgoing = trade => trade?.ownOfferKnown === true && trade.ownGold === 0 && Array.isArray(trade.ownItems) && trade.ownItems.length === 0;
const inventory = state => {
  const items = [...(state.inventoryItems ?? []), ...(state.beltItems ?? [])], totals = {};
  if (items.length > 400) throw new Error("Gift inventory exceeds observation bounds.");
  for (const item of items) {
    if (!uid(item.uniqueId) || !uint(item.quantity) || item.quantity === 0) throw new Error("Gift inventory identity or quantity is unknown.");
    const key = String(item.uniqueId);
    if (Object.hasOwn(totals, key)) throw new Error("Gift inventory contains duplicate identities.");
    totals[key] = item.quantity;
  }
  return totals;
};

/** A receive-only ordinary trade adapter. Its execution journal is intentionally
 * separate from portable memories. Neither a closing window nor a lock ACK is
 * a gift receipt; unknown confirmations are never replayed. */
export class GiftController {
  #client; #owner; #db; #clock; #report; #onObserved; #entry = null; #busy = false; #closed = false; #closePromise; #closeResolve;

  constructor({ client, ownerName, dbPath, clock = Date.now, report = () => {}, onObserved = () => {} }) {
    if (!name(ownerName) || typeof dbPath !== "string" || !dbPath.length) throw new Error("Gift owner and a separate execution journal are required.");
    this.#client = client; this.#owner = ownerName; this.#clock = clock; this.#report = report; this.#onObserved = onObserved;
    const location = dbPath === ":memory:" ? dbPath : resolve(dbPath);
    if (location !== ":memory:") mkdirSync(dirname(location), { recursive: true });
    this.#db = new DatabaseSync(location);
    try {
      const version = this.#db.prepare("PRAGMA user_version").get().user_version;
      if (version !== 0 && version !== 1 || version === 0 && this.#db.prepare("SELECT COUNT(*) AS n FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").get().n !== 0) throw new Error("Unsupported gift execution journal.");
      this.#db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=3000; CREATE TABLE IF NOT EXISTS received_gifts(sequence INTEGER PRIMARY KEY AUTOINCREMENT,local_id TEXT NOT NULL UNIQUE,trade_key TEXT UNIQUE,entry_json TEXT NOT NULL); PRAGMA user_version=1;");
      const saved = this.#db.prepare("SELECT entry_json FROM received_gifts ORDER BY sequence DESC LIMIT 1").get();
      if (saved) {
        this.#entry = JSON.parse(saved.entry_json);
        if (this.#entry.ownerName !== ownerName || !["accepting", "receiving", "unknown", "paused", "completed", "cancelled"].includes(this.#entry.status) || typeof this.#entry.confirmAttempted !== "boolean") throw new Error("Gift journal does not match this owner or supported state.");
        if (!terminal(this.#entry.status)) {
          this.#entry.status = "unknown"; this.#entry.recovered = true;
          this.#entry.reason = "重启后保留原交易记录，结果未知；不会重放接受或成交确认。";
          this.#save();
        }
      }
    } catch (error) { this.#db.close(); throw error; }
  }

  view() {
    const entry = this.#entry;
    if (!entry) return { status: "idle", ownerName: this.#owner, tradeId: null, receivedGold: null, receivedItems: [], reason: "只接收主人通过原交易窗口赠予的补给。" };
    return { status: entry.status, ownerName: this.#owner, tradeId: entry.tradeId, confirmAttempted: entry.confirmAttempted, cancelAttempted: entry.cancelAttempted,
      receivedGold: entry.receipt?.receivedGold ?? null, receivedItems: structuredClone(entry.receipt?.receivedItems ?? []), reason: entry.reason };
  }

  hasWork(state = this.#client.snapshot()) {
    return Boolean(state.tradePendingInvite?.name === this.#owner || state.trade?.active || state.trade?.uncertain || this.#entry && !terminal(this.#entry.status));
  }

  observe(state = this.#client.snapshot()) {
    this.#open();
    if (this.#entry && !terminal(this.#entry.status)) { this.#receipt(state); this.#cancelled(state); }
    return this.view();
  }

  async tick(state = this.#client.snapshot()) {
    this.#open();
    if (this.#busy) return { performed: false, ...this.view() };
    this.#busy = true;
    try {
      if (this.#entry && !terminal(this.#entry.status)) {
        const entry = this.#entry;
        if (entry.recovered || !same(entry.source, source(state, this.#client))) return this.#pause("交易来源已变化或运行器已重启，先核对原交易；不会重新确认。");
        if (this.#receipt(state)) return { performed: false, ...this.view() };
        if (this.#cancelled(state)) return { performed: false, ...this.view() };
        if (entry.confirmAttempted || entry.status === "unknown" || entry.status === "paused") return { performed: false, ...this.view() };
        if (!state.trade?.active || state.trade.id !== entry.tradeId || state.trade.partnerName !== this.#owner || state.trade.uncertain || !zeroOutgoing(state.trade)) return this.#pause("当前交易身份、伙伴或本人空报价尚未核实。");
        if (!usable(state)) return { performed: false, ...this.view() };
        if (state.trade.locked || state.trade.escrowPrepared) return this.#pause("交易已锁定但缺少本运行器的确认记录，停止自动操作。");
        const offeredItems = state.trade.partnerItems;
        if (!gold(state.trade.partnerGold) || !Array.isArray(offeredItems) || state.trade.partnerGold === 0 && offeredItems.length === 0) return { performed: false, ...this.view() };
        const current = this.#client.snapshot();
        if (!same(source(state, this.#client), source(current, this.#client)) || current.revision !== state.revision || !usable(current) || !zeroOutgoing(current.trade)) return { performed: false, ...this.view() };
        entry.baseline = { gold: current.gold, items: inventory(current), revision: current.revision };
        entry.expectedGold = current.trade.partnerGold;
        entry.expectedItems = current.trade.partnerItems.map(item => ({ uniqueId: String(item.uniqueId), quantity: item.quantity, itemIndex: item.itemIndex }));
        // Persist before crossing the economic boundary. Every later tick and
        // restart only observes this attempt; it never sends another confirm.
        entry.confirmAttempted = true; entry.status = "unknown"; entry.reason = "已提交只收不付的交易确认，等待最终成交与完整状态。";
        this.#save();
        try { await this.#client.confirmReceiveOnlyGift({ tradeId: entry.tradeId, ownerName: this.#owner }); }
        catch { entry.reason = "成交结果未知；只等待原交易回执，不自动重试确认。"; this.#save(); }
        this.#receipt(this.#client.snapshot()); this.#cancelled(this.#client.snapshot());
        return { performed: true, ...this.view() };
      }
      const invite = state.tradePendingInvite;
      if (!invite || invite.name !== this.#owner || !state.ready || state.trade?.active || state.trade?.uncertain) return { performed: false, ...this.view() };
      if (!matches(invite, state) || !uint(this.#clock())) return { performed: false, ...this.view() };
      const current = this.#client.snapshot();
      if (current.tradePendingInvite?.id !== invite.id || current.revision !== state.revision) return { performed: false, ...this.view() };
      this.#entry = { localId: randomUUID(), ownerName: this.#owner, source: source(state, this.#client), inviteId: invite.id, tradeId: null,
        status: "accepting", reason: "正在接受主人的普通交易邀请；阿洛不放入物品或金币。", confirmAttempted: false, cancelAttempted: false, recovered: false,
        baseline: null, expectedGold: null, expectedItems: [], receipt: null, reported: false, startedAtMs: this.#clock() };
      this.#db.prepare("INSERT INTO received_gifts(local_id,entry_json) VALUES(?,?)").run(this.#entry.localId, JSON.stringify(this.#entry));
      try {
        const accepted = await this.#client.acceptOwnerGiftInvite(this.#owner, invite.id), actual = this.#client.snapshot();
        if (!same(this.#entry.source, source(actual, this.#client)) || !actual.trade?.active || actual.trade.partnerName !== this.#owner || actual.trade.id !== accepted.tradeId || !zeroOutgoing(actual.trade) || actual.trade.uncertain) throw new Error("Owner trade acceptance was not matched.");
        this.#entry.tradeId = accepted.tradeId;
        const tradeKey = createHash("sha256").update(JSON.stringify([this.#entry.source.worldEpoch, this.#entry.source.characterIndex, this.#owner, accepted.tradeId])).digest("hex");
        if (this.#db.prepare("SELECT local_id FROM received_gifts WHERE trade_key=?").get(tradeKey)) throw new Error("This gift trade was already recorded.");
        this.#db.prepare("UPDATE received_gifts SET trade_key=? WHERE local_id=?").run(tradeKey, this.#entry.localId);
        this.#entry.status = "receiving"; this.#entry.reason = "已接受主人交易；等待补给报价，本人付出始终为零。"; this.#save();
      } catch { this.#entry.status = "unknown"; this.#entry.reason = "接受交易未得到匹配的完整状态；不会自动重放邀请。"; this.#save(); }
      return { performed: true, ...this.view() };
    } catch {
      if (this.#entry) { this.#entry.status = "unknown"; this.#entry.reason = "交易观察或执行记录未核实，暂停自动收礼。"; try { this.#save(); } catch { /* no further network mutation */ } }
      this.#report({ type: "giftObservationUnavailable" });
      return { performed: false, ...this.view() };
    } finally { this.#busy = false; this.#closeResolve?.(); }
  }

  /** Ordinary cancellation for a danger/recall lane. A failed or unresolved
   * cancellation is not permission to start another economic confirmation. */
  async cancel(reason = "危险或召回，已请求取消普通交易。") {
    this.#open();
    if (this.#busy) return { performed: false, ...this.view() };
    if (this.#entry?.cancelAttempted) { this.observe(); return { performed: false, ...this.view() }; }
    this.#busy = true;
    try {
      if (this.#entry && !terminal(this.#entry.status)) { this.#entry.cancelAttempted = true; this.#entry.status = "unknown"; this.#entry.reason = String(reason).slice(0, 160); this.#save(); }
      try { await this.#client.cancelGiftTrade(); } catch { /* original outcome stays unknown */ }
      this.#cancelled(this.#client.snapshot());
      return { performed: true, ...this.view() };
    } finally { this.#busy = false; this.#closeResolve?.(); }
  }

  #receipt(state) {
    const entry = this.#entry, final = state.trade?.lastFinalConfirm, full = state.fullSnapshot;
    if (!entry?.confirmAttempted || entry.recovered || entry.receipt || !usable(state) || state.trade.active || state.trade.uncertain || !matches(final, state) || final.tradeId !== entry.tradeId || final.partnerName !== this.#owner ||
      final.revision <= entry.baseline.revision || full.revision <= final.revision || !same(entry.source, source(state, this.#client))) return false;
    const incomingGold = (state.trade.deliveryGold ?? []).filter(item => item.revision > entry.baseline.revision && item.revision < final.revision).reduce((sum, item) => sum + item.gold, 0);
    const delivered = (state.trade.deliveryItems ?? []).filter(item => item.revision > entry.baseline.revision && item.revision < final.revision);
    if (incomingGold !== entry.expectedGold || state.gold !== entry.baseline.gold + incomingGold || delivered.length !== entry.expectedItems.length) return false;
    const current = inventory(state), receivedItems = [];
    const expected = entry.expectedItems.map(item => `${item.itemIndex}:${item.quantity}`).sort();
    if (!same(expected, delivered.map(item => `${item.itemIndex}:${item.quantity}`).sort())) return false;
    for (const item of delivered) {
      const key = String(item.uniqueId);
      if (!uid(item.uniqueId) || !uint(item.quantity) || (current[key] ?? 0) !== (entry.baseline.items[key] ?? 0) + item.quantity || receivedItems.some(value => value.uniqueId === key)) return false;
      receivedItems.push({ uniqueId: key, quantity: item.quantity, itemIndex: item.itemIndex });
    }
    if (incomingGold === 0 && receivedItems.length === 0) return false;
    // Receiving gifts cannot silently remove or change existing inventory.
    for (const [key, quantity] of Object.entries(entry.baseline.items)) if ((current[key] ?? 0) !== quantity + (receivedItems.find(value => value.uniqueId === key)?.quantity ?? 0)) return false;
    entry.receipt = { receivedGold: incomingGold, receivedItems, sourceEventRef: `trade:${final.connectionEpoch}:${final.characterEpoch}:${final.revision}`, fullSnapshotRevision: full.revision };
    entry.status = "completed"; entry.reason = "已核对最终成交、真实金币和物品入包；阿洛没有付出资产。"; this.#save();
    // Mark before reporting: memory outages cannot cause duplicate delivery or
    // duplicate callbacks after a runtime restart.
    entry.reported = true; this.#save();
    try { this.#onObserved({ kind: "owner-gift-received", eventId: `gift:${entry.localId}`, ownerName: this.#owner, tradeId: entry.tradeId, ...structuredClone(entry.receipt) }); }
    catch { this.#report({ type: "giftMemoryWriteUnavailable" }); }
    this.#prune(); return true;
  }

  #cancelled(state) {
    const entry = this.#entry, cancelled = state.trade?.lastCancel;
    if (!entry || terminal(entry.status) || entry.recovered || !same(entry.source, source(state, this.#client)) || !matches(cancelled, state) || cancelled.tradeId !== entry.tradeId || cancelled.unlock !== false ||
      state.trade.active || state.trade.uncertain || !usable(state) || state.fullSnapshot.revision <= cancelled.revision) return false;
    entry.status = "cancelled"; entry.reason = "服务器已取消原交易；没有记录为收到补给。"; this.#save(); this.#prune(); return true;
  }

  #pause(reason) {
    this.#entry.status = "unknown"; this.#entry.reason = reason; this.#save(); return { performed: false, ...this.view() };
  }
  #open() { if (this.#closed) throw new Error("Gift controller is closed."); }
  #save() {
    if (this.#db.prepare("UPDATE received_gifts SET entry_json=? WHERE local_id=?").run(JSON.stringify(this.#entry), this.#entry.localId).changes !== 1) throw new Error("Gift execution record is unavailable.");
  }
  #prune() {
    this.#db.prepare("DELETE FROM received_gifts WHERE sequence NOT IN (SELECT sequence FROM received_gifts ORDER BY sequence DESC LIMIT 128)").run();
  }
  async close() {
    if (this.#closed) return;
    this.#closed = true;
    if (this.#busy) { this.#closePromise ??= new Promise(resolve => { this.#closeResolve = resolve; }); await this.#closePromise; }
    this.#db.close();
  }
}
