import { loadClientCollision, nextWalkDirection } from "../skills/navigation.mjs";
import { exact, opaque, u64, purchaseOperation, purchaseReceipt, same } from "./purchase-wire.mjs";
import { SupplyStore } from "./supply-store.mjs";

const NPC = Object.freeze({ index: 9, name: "Merchant_Ruben", x: 288, y: 608, script: "BichonProvince/BorderVillage/Grocery" });
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const point = v => v && Number.isSafeInteger(v.x) && Number.isSafeInteger(v.y);
const ref = v => typeof v === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(v);
const inside = (p, b) => point(p) && p.x >= b.minX && p.x <= b.maxX && p.y >= b.minY && p.y <= b.maxY;
const terminal = s => ["completed", "cancelled", "failed"].includes(s);
const idle = () => ({ status: "idle", step: "idle", budgetGold: 50, chargedGold: null, acquiredQuantity: 0, receiptKnown: false, returnedToAnchor: false, reason: "", lastError: null, permitId: null, expiresAtMs: null, receiptRevision: null });
const validGold = n => Number.isSafeInteger(n) && n >= 0 && n <= 0xffff_ffff;

/** A grant is trusted only because root obtained it from the authenticated
 * server permit route. These checks constrain this runner; the server enforces
 * spending independently. No grant may be reconstructed from portable memory. */
export function validateSupplyGrant(value) {
  exact(value, ["permitId", "worldEpoch", "actorCharacterIndex", "mapFileName", "npcIndex", "npcName", "npcPosition", "itemIndex", "itemName", "quantity", "maxGold", "expiresAtMs", "returnAnchor", "bounds"]);
  exact(value.npcPosition, ["x", "y"]); exact(value.returnAnchor, ["x", "y"]); exact(value.bounds, ["minX", "minY", "maxX", "maxY"]);
  const b = value.bounds;
  if (!ref(value.permitId) || !ref(value.worldEpoch) || !Number.isSafeInteger(value.actorCharacterIndex) || value.actorCharacterIndex < 0 || value.mapFileName !== "0" || value.npcIndex !== NPC.index || value.npcName !== NPC.name || value.npcPosition.x !== NPC.x || value.npcPosition.y !== NPC.y || value.itemIndex !== 658 || value.itemName !== "(HP)DrugSmall" || value.quantity !== 1 || !Number.isInteger(value.maxGold) || value.maxGold < 1 || value.maxGold > 50 || !Number.isSafeInteger(value.expiresAtMs) || value.expiresAtMs < 0 ||
    !Object.values(b).every(Number.isSafeInteger) || b.minX < 270 || b.maxX > 310 || b.minY < 595 || b.maxY > 635 || b.minX > b.maxX || b.minY > b.maxY || !inside(value.returnAnchor, b) || !inside(value.npcPosition, b)) throw new Error("Invalid bounded Bichon supply grant.");
  return structuredClone(value);
}

export class SupplyController {
  #client; #store; #clock; #report; #observed; #stepTo; #mapLoader; #collision; #entry = null; #busy = false; #closed = false; #begun = false; #nextQueryAt = 0; #closePromise; #closeResolve;
  constructor({ client, dbPath, clock = Date.now, report = () => {}, onObserved = () => {}, stepTo, mapLoader = loadClientCollision }) {
    this.#client = client; this.#clock = clock; this.#report = report; this.#observed = onObserved; this.#stepTo = stepTo; this.#mapLoader = mapLoader;
    this.#store = new SupplyStore(dbPath);
    const saved = this.#store.current();
    if (saved) {
      saved.grant = validateSupplyGrant(saved.grant);
      if (saved.task.operation) purchaseOperation(saved.task.operation);
      this.#entry = saved;
      if (!terminal(saved.task.status)) {
        saved.task.recovered = true;
        saved.task.status = saved.task.attempted && !saved.task.receiptApplied ? "unknown" : "paused";
        saved.task.step = saved.task.attempted && !saved.task.receiptApplied ? "reconcile" : saved.task.navigationBlocked ? "navigation-blocked" : "awaiting-new-authorization";
        saved.task.reason = saved.task.navigationBlocked && !(saved.task.attempted && !saved.task.receiptApplied) ? saved.task.navigationBlockReason : "重启后保留原委托与购买记录；不恢复新的花费。";
        this.#save();
      }
    }
  }

  view() {
    if (!this.#entry) return idle();
    const { grant, task } = this.#entry;
    return { status: task.status, step: task.step, budgetGold: grant.maxGold, chargedGold: task.chargedGold, acquiredQuantity: task.acquiredQuantity,
      receiptKnown: Boolean(task.receipt), returnedToAnchor: task.returnedToAnchor, reason: task.reason, lastError: task.lastError,
      permitId: grant.permitId, expiresAtMs: grant.expiresAtMs, receiptRevision: task.receipt?.entry?.serverRevision ?? null };
  }

  authorize(raw) {
    this.#open(); const grant = validateSupplyGrant(raw), now = this.#clock(), state = this.#client.snapshot();
    const old = this.#store.find(grant.permitId);
    if (old) { if (!same(old.grant, grant)) throw new Error("Supply permit ID conflicts."); return this.view(); }
    if (this.#busy || this.#entry && !terminal(this.#entry.task.status) && this.#entry.task.attempted && !this.#entry.task.receiptApplied) throw new Error("Reconcile the original supply operation before another authorization.");
    if (!Number.isSafeInteger(now) || now < 0 || this.#client.worldEpoch !== grant.worldEpoch || this.#client.activeCharacterIndex !== grant.actorCharacterIndex || !this.#world(state, grant) || grant.expiresAtMs <= now || grant.expiresAtMs > now + 300_000) throw new Error("Supply grant does not match the current actor, world or validity window.");
    this.#entry = this.#store.create(grant, { status: "authorized", step: "travel", chargedGold: null, acquiredQuantity: 0, returnedToAnchor: false,
      reason: "已确认本次有限补给授权。", lastError: null, operation: null, attempted: false, receipt: null, receiptApplied: false,
      cancelled: false, recovered: false, quotedGold: null, acquisitionReported: false, returnReported: false });
    this.#begun = false; this.#nextQueryAt = 0; return this.view();
  }

  cancel(reason = "主人已撤销本次补给。") {
    this.#open(); if (!this.#entry) return this.view();
    const task = this.#entry.task; task.cancelled = true;
    task.status = task.attempted && !task.receiptApplied ? "unknown" : "cancelled";
    task.step = task.status === "unknown" ? "reconcile" : "cancelled";
    task.reason = String(reason).slice(0, 160); this.#save(); return this.view();
  }

  pauseNavigation(reason = "补给路线连续三次被拒绝或不可达；已暂停，尚未确认抵达授权点。") {
    this.#open(); if (!this.#entry || terminal(this.#entry.task.status)) return this.view();
    const task = this.#entry.task;
    task.navigationBlocked = true; task.navigationBlockReason = String(reason).slice(0, 160);
    task.status = task.attempted && !task.receiptApplied ? "unknown" : "paused";
    task.step = task.status === "unknown" ? "reconcile" : "navigation-blocked";
    task.reason = task.navigationBlockReason; task.lastError = task.navigationBlockReason;
    this.#save(); return this.view();
  }

  async tick(snapshot = this.#client.snapshot()) {
    this.#open();
    if (this.#busy || !this.#entry) return { performed: false, ...this.view() };
    const { grant, task } = this.#entry;
    if (terminal(task.status)) return { performed: false, ...this.view() };
    this.#busy = true;
    try {
      if (!Number.isSafeInteger(this.#clock()) || this.#clock() < 0 || !this.#world(snapshot, grant) || snapshot.player?.dead === true || snapshot.playerHp === 0) { this.#pause("角色、地图或健康状态发生变化，补给已暂停。"); return { performed: false, ...this.view() }; }
      if (task.attempted && !task.receiptApplied) return await this.#reconcile();
      if (task.status === "paused") return { performed: false, ...this.view() };
      if (task.receiptApplied) return await this.#return(snapshot);
      if (task.cancelled || this.#clock() >= grant.expiresAtMs) { this.#pause(task.cancelled ? "本次补给已撤销。" : "本次补给授权已到期。"); return { performed: false, ...this.view() }; }
      task.status = "running";
      if (task.step === "travel") {
        if (distance(snapshot.player, grant.npcPosition) > 1) return await this.#walk(snapshot, grant.npcPosition, 1);
        task.step = "open-shop"; this.#save(); return { performed: false, ...this.view() };
      }
      const npc = snapshot.entities?.find(e => e.kind === "npc" && e.name === NPC.name && e.x === NPC.x && e.y === NPC.y);
      if (!npc || distance(snapshot.player, npc) > 1) throw new Error("Expected adjacent Ruben is not currently observed.");
      if (task.step === "open-shop") { await this.#client.interactNpc(npc.objectId); task.step = "select-shop"; }
      else if (task.step === "select-shop") { await this.#client.selectNpcBuyLink(); task.step = "begin"; }
      else if (task.step === "begin") {
        const frame = await this.#client.beginNpcPurchase();
        if (frame.reply?.kind !== "producer" || !this.#paired(frame)) throw new Error("A complete current purchase producer is required.");
        task.producer = structuredClone(frame.reply.producer); this.#begun = true; task.step = "quote";
      } else if (task.step === "quote") {
        const before = this.#client.snapshot(), good = this.#good(before);
        const request = { itemIndex: good.purchaseItemIndex, count: 1, panelType: 0 };
        const frame = await this.#client.quoteNpcPurchase(request);
        const observed = this.#client.snapshot(), current = this.#good(observed);
        if (frame.reply?.kind !== "quote" || !this.#paired(frame) || !same(frame.reply.intent.request, request) || frame.reply.intent.currency !== "gold" || frame.reply.intent.source !== "trade" || current.purchaseItemIndex !== good.purchaseItemIndex || current.price !== good.price || !validGold(observed.gold) || observed.gold < current.price || current.price > grant.maxGold) throw new Error("The actual quote is outside the permitted goods, price or balance.");
        task.operation = purchaseOperation({ actor: task.producer.actor, requestScope: task.producer.producerScope, sequence: this.#entry.sequence, intent: frame.reply.intent });
        task.quotedGold = current.price; task.step = "ready-to-purchase";
      } else if (task.step === "ready-to-purchase") {
        // Write the exact operation before crossing the economic boundary.
        // Every later tick and every restart can only query this operation.
        const current = this.#client.snapshot();
        if (!this.#world(current, grant) || current.playerHp === 0 || current.player?.dead || this.#clock() >= grant.expiresAtMs || task.cancelled) { this.#pause("发送前授权或角色状态已变化。"); return { performed: false, ...this.view() }; }
        this.#good(current);
        task.attempted = true; task.status = "unknown"; task.step = "purchase-pending"; this.#save();
        try { this.#acceptFrame(await this.#client.purchaseNpcItem(task.operation)); }
        catch { task.status = "unknown"; task.step = "reconcile"; task.lastError = "购买结果未知；只查询原操作，不重新购买。"; }
      } else throw new Error("Unsupported supply phase.");
      this.#save(); return { performed: true, ...this.view() };
    } catch {
      if (task.attempted && !task.receiptApplied) { task.status = "unknown"; task.step = "reconcile"; task.lastError = "原购买结果尚未核实；不会再次发送购买。"; this.#save(); }
      else { task.status = "failed"; task.step = "failed"; task.lastError = "补给未通过当前观察、权限或服务检查。"; this.#save(); }
      return { performed: false, ...this.view() };
    } finally { this.#busy = false; this.#closeResolve?.(); }
  }

  #world(state, grant) { return state?.ready === true && state.mapFileName === grant.mapFileName && inside(state.player, grant.bounds) && this.#client.worldEpoch === grant.worldEpoch && this.#client.activeCharacterIndex === grant.actorCharacterIndex; }
  #open() { if (this.#closed) throw new Error("Supply controller is closed."); }
  #save() { this.#store.save(this.#entry.grant.permitId, this.#entry.task); }
  #pause(reason) { const task = this.#entry.task; task.status = "paused"; task.step = "paused"; task.reason = reason; this.#save(); }
  #paired(frame) {
    const state = this.#client.snapshot();
    try { exact(frame.authority, ["actor", "producerScope", "serverRevision"]); opaque(frame.authority.actor); opaque(frame.authority.producerScope); u64(frame.authority.serverRevision, false, true); }
    catch { return false; }
    return frame.snapshot !== null && frame.snapshot?.mapFileName === "0" && frame.snapshot?.playerObjectId === state.playerObjectId && Array.isArray(frame.snapshot?.inventoryItems) && Array.isArray(frame.snapshot?.beltItems) && Array.isArray(frame.snapshot?.equipmentItems) && Array.isArray(frame.snapshot?.entities) &&
      frame.snapshot.entities.some(e => e.kind === "selfPlayer" && e.objectId === state.playerObjectId && e.name === state.player?.name) && same(state.npcPurchaseAuthority, frame.authority) && state.ready === true && state.inventoryStale === false;
  }
  #good(state) {
    const shop = state.nativeNpcShop, grant = this.#entry.grant;
    const merchants = Array.isArray(state.entities) ? state.entities.filter(e => e.kind === "npc" && e.objectId === shop?.npcObjectId) : [];
    const merchant = merchants.length === 1 ? merchants[0] : null;
    // Entering the real Buy page can clear its dialogue. The current server
    // shop and its observed adjacent merchant bind the service instead; a
    // remaining dialogue for a different NPC still invalidates that binding.
    if (!this.#world(state, grant) || !shop || !Number.isInteger(shop.npcObjectId) || shop.npcObjectId < 0 || shop.npcObjectId > 0xffff_ffff || shop.scriptKey !== NPC.script || typeof shop.service !== "string" || shop.service.toUpperCase() !== "BUYSELL" || shop.packetType !== "NPCGoods" || shop.panelType !== 0 || !Array.isArray(shop.list) || !merchant || merchant.name !== NPC.name || merchant.x !== NPC.x || merchant.y !== NPC.y || distance(state.player, merchant) > 1 || state.activeNpcDialog != null && state.activeNpcDialog.npcObjectId !== shop.npcObjectId) throw new Error("The permitted current NPC buy service is unavailable.");
    const goods = shop.list.filter(item => item.itemIndex === 658 && item.name === "(HP)DrugSmall" && item.count === 1 && typeof item.purchaseItemIndex === "string" && /^(?:0|[1-9][0-9]{0,19})$/.test(item.purchaseItemIndex));
    if (goods.length !== 1 || !validGold(goods[0].price) || goods[0].price === 0 || goods[0].price > grant.maxGold) throw new Error("The observed single potion price exceeds the grant.");
    return goods[0];
  }
  #acceptFrame(frame) {
    const { task, grant } = this.#entry;
    if (!["purchase", "recovery", "failure"].includes(frame.reply?.kind)) throw new Error("No terminal economic reply.");
    const raw = frame.reply?.receipt;
    if (!raw) { task.status = "unknown"; task.step = "reconcile"; return; }
    const receipt = purchaseReceipt(raw, task.operation), paid = receipt.entry.outcome.committed;
    if (task.receipt && !same(task.receipt.entry, receipt.entry)) throw new Error("Purchase receipt conflicts with prior observation.");
    task.receipt = receipt;
    if (!paid) { task.status = "failed"; task.step = "rejected"; task.reason = "服务器拒绝了本次购买；不会重试。"; task.receiptApplied = true; return; }
    if (paid.currency !== "gold" || paid.source !== "trade" || paid.admittedCount !== 1 || paid.charged > grant.maxGold || paid.charged !== task.quotedGold) throw new Error("Receipt exceeds authorized purchase.");
    task.chargedGold = paid.charged;
    if (!this.#paired(frame) || frame.authority.actor !== task.operation.actor || BigInt(frame.authority.serverRevision) < BigInt(receipt.entry.serverRevision)) { task.status = "unknown"; task.step = "reconcile"; return; }
    task.receiptApplied = true; task.acquiredQuantity = paid.admittedCount;
    task.status = task.cancelled ? "cancelled" : task.recovered || task.navigationBlocked ? "paused" : "returning";
    task.step = task.cancelled ? "cancelled" : task.navigationBlocked ? "navigation-blocked" : task.recovered ? "awaiting-new-authorization" : "return";
    task.lastError = task.navigationBlocked ? task.navigationBlockReason : null; task.reason = task.navigationBlocked ? task.navigationBlockReason : "服务器已确认真实购药与扣款。";
    this.#save(); this.#reportAcquisition();
  }
  #reportAcquisition() {
    const { grant, task } = this.#entry;
    if (task.acquisitionReported || !task.receiptApplied || task.acquiredQuantity !== 1) return;
    try {
      this.#observed({ kind: "supply-acquired", permitId: grant.permitId, chargedGold: task.chargedGold, acquiredQuantity: 1, receiptRevision: task.receipt.entry.serverRevision, incomingUniqueId: task.receipt.entry.outcome.committed.incomingUniqueId });
      task.acquisitionReported = true; this.#save();
    } catch { this.#report({ type: "supplyMemoryWriteUnavailable" }); }
  }
  async #reconcile() {
    const task = this.#entry.task;
    if (this.#clock() < this.#nextQueryAt) return { performed: false, ...this.view() };
    this.#nextQueryAt = this.#clock() + 5_000;
    try {
      if (!this.#begun) {
        const frame = await this.#client.beginNpcPurchase();
        if (frame.reply?.kind !== "producer" || frame.reply.producer.actor !== task.operation.actor || !this.#paired(frame)) throw new Error("Original actor recovery is unavailable.");
        this.#begun = true; return { performed: true, ...this.view() };
      }
      this.#acceptFrame(await this.#client.queryNpcPurchase(task.operation)); this.#save();
    } catch { task.status = "unknown"; task.step = "reconcile"; task.lastError = "原购买结果尚未核实；不会再次购买。"; this.#save(); }
    return { performed: true, ...this.view() };
  }
  async #return(snapshot) {
    const { grant, task } = this.#entry;
    this.#reportAcquisition();
    const current = this.#client.snapshot();
    if (!this.#world(current, grant) || current.connectionEpoch !== snapshot.connectionEpoch || current.playerObjectId !== snapshot.playerObjectId || current.player?.dead === true || current.playerHp === 0) {
      this.#pause("返程观察或角色状态发生变化，尚未确认抵达授权点旁。");
      return { performed: false, ...this.view() };
    }
    const arrivalDistance = distance(current.player, grant.returnAnchor);
    // The authorized owner can still occupy the return tile. Arrival means
    // reaching that point or an adjacent legal tile, never entering its owner.
    if (arrivalDistance > 1) return await this.#walk(current, grant.returnAnchor, 1);
    task.returnActualPosition = { x: current.player.x, y: current.player.y };
    task.returnArrivalDistance = arrivalDistance;
    task.returnedToAnchor = true; task.status = "completed"; task.step = "completed"; task.reason = "已真实购药并回到本次授权点旁。"; this.#save();
    if (!task.returnReported) {
      try { this.#observed({ kind: "supply-returned", permitId: grant.permitId, chargedGold: task.chargedGold, acquiredQuantity: task.acquiredQuantity, receiptRevision: task.receipt.entry.serverRevision,
        returnAnchor: structuredClone(grant.returnAnchor), actualPosition: structuredClone(task.returnActualPosition), arrivalDistance,
        scope: { mapFileName: grant.mapFileName, bounds: structuredClone(grant.bounds), maximumDistanceFromAnchor: 1 } }); task.returnReported = true; this.#save(); }
      catch { this.#report({ type: "supplyMemoryWriteUnavailable" }); }
    }
    return { performed: false, ...this.view() };
  }
  async #walk(snapshot, target, desiredDistance) {
    const { grant, task } = this.#entry;
    if (this.#stepTo) { const result = await this.#stepTo({ snapshot, target: structuredClone(target), desiredDistance, bounds: structuredClone(grant.bounds) }); return { ...result, ...this.view() }; }
    this.#collision ??= await this.#mapLoader("0");
    const current = this.#client.snapshot();
    if (this.#closed || !this.#world(current, grant) || current.connectionEpoch !== snapshot.connectionEpoch || current.playerObjectId !== snapshot.playerObjectId || current.player?.dead === true || current.playerHp === 0 || task.cancelled || !task.receiptApplied && this.#clock() >= grant.expiresAtMs) return { performed: false, ...this.view() };
    const occupied = current.entities.filter(e => e.objectId !== current.playerObjectId && e.objectId !== current.selfZoneObjectId && e.dead !== true).map(e => ({ x: e.x, y: e.y }));
    const direction = nextWalkDirection({ player: current.player, target, desiredDistance, collision: this.#collision, occupied, movementBounds: grant.bounds });
    if (!direction) { task.lastError = "授权路线当前受阻，等待重新观察。"; this.#save(); return { performed: false, blocked: true, ...this.view() }; }
    const result = await this.#client.walk(direction);
    return { performed: true, moved: result.moved === true, ...this.view() };
  }
  async close() {
    if (this.#closePromise) return this.#closePromise;
    this.#closed = true;
    this.#closePromise = (async () => {
      if (this.#busy) await new Promise(resolve => { this.#closeResolve = resolve; });
      this.#store.close();
    })();
    return this.#closePromise;
  }
}
