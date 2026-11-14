import { EventEmitter } from "node:events";
import { randomUUID } from "node:crypto";
import { parseGatewayJson, stringifyGatewayJson } from "./lossless-json.mjs";
import { ObservationReducer } from "./observation.mjs";
import { prepareNearbyChat } from "./skills/conversation.mjs";
import { PURCHASE_CAPABILITY, parsePurchaseFrame, purchaseAction, purchaseOperation, purchaseRequest, same, u64 } from "./supply/purchase-wire.mjs";

export const DIRECTIONS = Object.freeze([
  "Up", "UpRight", "Right", "DownRight", "Down", "DownLeft", "Left", "UpLeft",
]);

function validUrl(value) {
  const url = new URL(value);
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (!["ws:", "wss:"].includes(url.protocol) || (!local && url.protocol !== "wss:")) {
    throw new Error("Use wss for remote gateways, or ws on loopback.");
  }
  if (url.username || url.password || url.search || url.hash || !["/ws", "/playtest/ws"].includes(url.pathname)) {
    throw new Error("Use the /ws or R16 /playtest/ws gateway route without credentials or query parameters.");
  }
  return url.href;
}

function directionName(value) {
  const direction = DIRECTIONS.find((entry) => entry.toLowerCase() === String(value).toLowerCase());
  if (!direction) throw new Error("Unknown movement direction.");
  return direction;
}

/** Ordinary player transport. No raw-command API, model tools, or privileged route. */
export class GatewayClient extends EventEmitter {
  #socket = null;
  #waiters = new Set();
  #frames = [];
  #sequence = 0;
  #heartbeat = null;
  #failure = null;
  #authenticated = false;
  #requestInFlight = null;
  #generation = 0;
  #closingPromise = null;
  #WebSocket;
  #controlRequests = new Map();
  #purchaseRequests = new Map();
  #purchaseSequence = 0n;
  #purchaseProducer = null;
  #purchaseOptedIn = false;
  #activeCharacterIndex = null;
  #ownResidentId = null;
  #ownerTradeUnknown = new Set();

  constructor({ url, realmId, allowedMaps = ["0", "0141"], timeoutMs = 15_000, WebSocketImpl = globalThis.WebSocket }) {
    super();
    if (typeof realmId !== "string" || !realmId.trim()) throw new Error("An explicit realmId is required.");
    this.url = validUrl(url);
    this.realmId = realmId;
    this.allowedMaps = new Set(allowedMaps);
    this.timeoutMs = timeoutMs;
    this.features = { sharedParty: false, companionControl: false };
    this.worldEpoch = null;
    this.observation = new ObservationReducer();
    this.#WebSocket = WebSocketImpl;
  }

  get sequence() { return this.#sequence; }
  get activeCharacterIndex() { return this.#activeCharacterIndex; }
  snapshot() { return this.observation.snapshot(); }

  async connect() {
    if (this.#socket || this.#closingPromise) throw new Error("Client already has a connection or is closing.");
    this.#failure = null;
    this.#authenticated = false;
    this.#frames = [];
    this.#purchaseProducer = null;
    this.#purchaseOptedIn = false;
    this.#activeCharacterIndex = null;
    this.#ownResidentId = null;
    this.#ownerTradeUnknown.clear();
    this.observation.resetConnection();
    const after = this.#sequence;
    const socket = new this.#WebSocket(this.url);
    this.#socket = socket;
    const generation = ++this.#generation;
    const current = () => this.#socket === socket && this.#generation === generation;
    socket.addEventListener("message", (event) => { if (current()) this.#receive(event.data); });
    socket.addEventListener("error", () => { if (current()) this.#fail(new Error("Gateway WebSocket connection failed.")); });
    socket.addEventListener("close", () => {
      if (!current()) return;
      this.#socket = null;
      clearInterval(this.#heartbeat);
      this.#authenticated = false;
      this.observation.resetConnection();
      this.#fail(new Error("Gateway connection closed."));
      this.emit("disconnected");
    });
    try {
      const realm = await this.waitFor((event) => event.type === "realmInfo", { after });
      if (realm.payload.realmId !== this.realmId) throw new Error("Gateway realmId does not match the configured world.");
      this.features = realm.payload.features;
      this.worldEpoch = realm.payload.worldEpoch ?? null;
      await this.waitFor((event) => event.packet === "Connected", { after });
      await this.#request({ type: "clientVersion" }, ["ClientVersion"], (event) => event.payload?.result === 1);
      this.#heartbeat = setInterval(() => {
        try { this.#send({ type: "keepAlive", time: Date.now() }); } catch { /* close rejects current waiters */ }
      }, 10_000);
      this.#heartbeat.unref?.();
      return realm.payload;
    } catch (error) {
      await this.close();
      throw error;
    }
  }

  async createAccount(accountId, password) {
    return this.#request({ type: "newAccount", accountId, password }, ["NewAccount"], (event) => event.payload?.result === 8);
  }

  /** Private owner/runtime channel, isolated from ordinary observation/logs. */
  async companionControl(operation, { residentId, payload = {}, requestId = randomUUID() } = {}) {
    this.#requireCompanionControl();
    if (!["list", "view", "instruct", "teach", "proposeTeaching", "confirmTeaching", "cancelTeaching", "authorizeSupply", "cancelSupply", "authorizeLife", "cancelLife", "resumeLife", "requestLifeSupply", "revokeFeedback", "deleteFeedback", "deleteEpisode", "clearGrowth", "recall", "resume", "exportBegin", "exportChunk", "trialEvent"].includes(operation)) throw new Error("Unsupported companion operation.");
    if (operation !== "list" && (typeof residentId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(residentId)) || operation === "list" && residentId !== undefined) throw new Error("Invalid companion resident reference.");
    if (operation === "requestLifeSupply" && (!this.#ownResidentId || residentId !== this.#ownResidentId || !payload || typeof payload !== "object" || Array.isArray(payload) || Object.keys(payload).length !== 1 || typeof payload.lifePermitId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(payload.lifePermitId))) throw new Error("Life child supply requires this registered actor and only its life permit.");
    return this.#controlRequest({ type: "companionControl", requestId, ...(residentId ? { residentId } : {}), operation, payload }, operation);
  }

  async registerCompanionRuntime({ residentId, archiveId, requestId = randomUUID() }) {
    this.#requireCompanionControl();
    if (![residentId, archiveId].every(value => typeof value === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(value))) throw new Error("Invalid companion enrollment reference.");
    const generation = this.#generation, character = this.#activeCharacterIndex, entry = this.snapshot().characterEpoch;
    const registration = await this.#controlRequest({ type: "companionRuntimeRegister", requestId, residentId, archiveId }, "register");
    if (generation === this.#generation && character === this.#activeCharacterIndex && entry === this.snapshot().characterEpoch && this.snapshot().ready && registration.status === "ok" && registration.result?.archiveId === archiveId && registration.result?.worldEpoch === this.worldEpoch && Number.isSafeInteger(registration.result?.generation) && registration.result.generation > 0) this.#ownResidentId = residentId;
    return registration;
  }

  /** Only the actor's registered runtime can request its server-fixed child grant. */
  async requestLifeSupply({ lifePermitId, requestId = randomUUID(), residentId = this.#ownResidentId } = {}) {
    if (typeof lifePermitId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(lifePermitId) || !this.#ownResidentId || residentId !== this.#ownResidentId) throw new Error("A current registered actor and life permit are required.");
    const reply = await this.companionControl("requestLifeSupply", { residentId, requestId, payload: { lifePermitId } });
    if (reply.status !== "ok" || !reply.result || typeof reply.result !== "object" || Array.isArray(reply.result)) {
      const error = new Error("Life supply was not confirmed by the server; do not request a replacement automatically.");
      error.code = reply.status === "unknown" ? "COMPANION_RESULT_UNKNOWN" : "LIFE_SUPPLY_NOT_CONFIRMED";
      throw error;
    }
    return structuredClone(reply.result);
  }

  replyCompanionRuntime({ requestId, generation, status, result }) {
    this.#requireCompanionControl();
    if (typeof requestId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(requestId)) throw new Error("Invalid companion reply reference.");
    if (!Number.isSafeInteger(generation) || generation < 1 || !["ok", "conflict", "error", "invalid", "unknown"].includes(status)) throw new Error("Invalid companion runtime reply.");
    this.#send({ type: "companionRuntimeReply", requestId, generation, status, result });
  }

  #controlRequest(command, operation) {
    if (typeof command.requestId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(command.requestId) || this.#controlRequests.has(command.requestId) || this.#controlRequests.size >= 8) throw new Error("Invalid or busy companion request.");
    if (Buffer.byteLength(stringifyGatewayJson(command)) > 60 * 1024) throw new Error("Companion request is too large.");
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#controlRequests.delete(command.requestId);
        const error = new Error("Companion operation result is unknown; do not replay a mutation automatically.");
        error.code = "COMPANION_RESULT_UNKNOWN";
        reject(error);
      }, Math.max(20_000, this.timeoutMs));
      this.#controlRequests.set(command.requestId, { operation, resolve, reject, timer });
      try { this.#send(command); }
      catch (error) { clearTimeout(timer); this.#controlRequests.delete(command.requestId); reject(error); }
    });
  }

  async login(accountId, password) {
    const event = await this.#request({ type: "login", accountId, password }, ["LoginSuccess", "Login", "LoginBanned"],
      (message) => message.packet === "LoginSuccess" && Array.isArray(message.payload?.characters));
    this.#authenticated = true;
    return event.payload.characters;
  }

  async createCharacter(name, { gender = "Male", characterClass = "Warrior" } = {}) {
    this.#requireAuthentication();
    const event = await this.#request({ type: "newCharacter", name, gender, class: characterClass },
      ["NewCharacterSuccess", "NewCharacter"], (message) => message.packet === "NewCharacterSuccess");
    return event.payload.character;
  }

  async startGame(characterIndex) {
    this.#requireAuthentication();
    this.#purchaseProducer = null;
    this.#ownResidentId = null;
    this.#ownerTradeUnknown.clear();
    if (!Number.isSafeInteger(characterIndex) || characterIndex < 0) throw new Error("Invalid character index.");
    const after = this.#sequence;
    await this.#request({ type: "startGame", characterIndex }, ["StartGame", "StartGameBanned", "StartGameDelay"],
      (event) => event.packet === "StartGame" && event.payload?.result === 4 && event.payload?.resolution > 0);
    await this.waitFor((event) => event.type === "worldSnapshot" && this.snapshot().ready, { after });
    this.#activeCharacterIndex = characterIndex;
    return this.#requireInWorld();
  }

  async walk(direction) {
    const before = this.#requireInWorld();
    await this.#request({ type: "walk", direction: directionName(direction) }, ["UserLocation"]);
    const after = this.#requireInWorld();
    return {
      moved: before.player?.x !== after.player?.x || before.player?.y !== after.player?.y,
      before: before.player,
      after: after.player,
    };
  }

  async turn(direction) {
    this.#requireInWorld();
    await this.#request({ type: "turn", direction: directionName(direction) }, ["UserLocation"]);
    return this.snapshot();
  }

  async setAllowGroup(allowGroup) {
    this.#requireSharedParty();
    const after = this.#sequence;
    await this.#request({ type: "switchGroup", allowGroup: Boolean(allowGroup) }, ["SwitchGroup"],
      (event) => event.payload?.allowGroup === Boolean(allowGroup));
    await this.waitFor((event) => event.type === "worldSnapshot" && this.snapshot().party.allowGroup === Boolean(allowGroup), { after });
    return this.snapshot().party;
  }

  async inviteMember(name) {
    this.#requireSharedParty();
    if (typeof name !== "string" || !name.trim() || name.length > 64) throw new Error("Invalid party member name.");
    this.#send({ type: "addMember", name });
    return { sent: true, confirmed: false };
  }

  async acceptGroupInvite() {
    this.#requireSharedParty();
    const inviter = this.snapshot().party.pendingInvite?.name;
    if (!inviter) throw new Error("No current party invitation.");
    await this.#perform({ type: "groupInvite", acceptInvite: true },
      (event) => event.packet === "GroupMemberInfo" && event.payload?.members?.some((member) => member.name === inviter) &&
        event.payload.members.some((member) => member.name === this.snapshot().player?.name));
    return this.snapshot().party;
  }

  async leaveGroup() {
    this.#requireSharedParty();
    const before = this.snapshot().party;
    if (before.members.length === 0) return before;
    // R16 reserves DelMember for the leader. Every member can leave through
    // the ordinary SwitchGroup preference; restore the prior invite setting
    // only after an authoritative empty roster has actually been observed.
    await this.setAllowGroup(false);
    if (this.snapshot().party.members.length !== 0) {
      await this.waitFor(event => event.type === "worldSnapshot" && this.snapshot().party.members.length === 0);
    }
    if (before.allowGroup) await this.setAllowGroup(true);
    return this.snapshot().party;
  }

  async attackDirection(direction) {
    this.#requireInWorld();
    if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
    this.#send({ type: "attackDirection", direction: directionName(direction), spell: 0 });
    return { sent: true, confirmed: false };
  }

  async setPeaceAttackMode() {
    this.#requireInWorld();
    await this.#request({ type: "changeAMode", mode: 0 }, ["ChangeAMode"], event => event.payload?.mode === 0);
    return true;
  }

  async useOwnedItem(uniqueId, grid = "inventory") {
    const before = this.#requireInWorld();
    if (!["inventory", "belt"].includes(grid)) throw new Error("Unsupported companion item grid.");
    const collection = grid === "belt" ? before.beltItems : before.inventoryItems;
    const item = collection.find((entry) => String(entry.uniqueId) === String(uniqueId));
    if (!item || item.quantity <= 0 || before.inventoryStale) throw new Error("The exact owned item must be observed before use.");
    const after = this.#sequence;
    const receipt = await this.#request({ type: "useItem", uniqueId: item.uniqueId, grid }, ["UseItem"], () => true,
      (event) => String(event.payload?.uniqueId) === String(item.uniqueId) && String(event.payload?.grid).toLowerCase() === grid);
    if (receipt.payload.success) {
      await this.waitFor((event) => event.type === "worldSnapshot" &&
        (this.snapshot()[grid === "belt" ? "beltItems" : "inventoryItems"].find((entry) => String(entry.uniqueId) === String(item.uniqueId))?.quantity ?? 0) < item.quantity,
      { after });
    }
    return { success: receipt.payload.success === true, uniqueId: item.uniqueId };
  }

  async pickUpTile() {
    const before = this.#requireInWorld();
    const fingerprint = (state) => stringifyGatewayJson({ gold: state.gold, inventory: state.inventoryItems, belt: state.beltItems });
    try {
      // Party projections can precede the action's own snapshot. An unchanged
      // pre-action snapshot is never a successful pickup receipt.
      await this.#perform({ type: "pickUpTile" }, event => event.type === "worldSnapshot" && fingerprint(before) !== fingerprint(this.snapshot()),
        () => true, { timeoutMs: Math.min(this.timeoutMs, 2_000) });
      return { changed: true };
    } catch (error) {
      if (error.code !== "GATEWAY_RESULT_TIMEOUT") throw error;
      return { changed: false, confirmed: false };
    }
  }

  /** Only the currently observed NPC and its real buy link are addressable. */
  async interactNpc(objectId) {
    const before = this.#requireInWorld();
    const npc = before.entities.find(value => value.kind === "npc" && value.objectId === objectId);
    if (!npc || !Number.isInteger(npc.x) || !Number.isInteger(npc.y) || Math.max(Math.abs(npc.x - before.player.x), Math.abs(npc.y - before.player.y)) > 1) throw new Error("Observe the adjacent NPC before interacting.");
    await this.#perform({ type: "interact", objectId }, event => event.type === "worldSnapshot" && this.snapshot().activeNpcDialog?.npcObjectId === objectId);
    return this.snapshot().activeNpcDialog;
  }

  async selectNpcBuyLink() {
    const before = this.#requireInWorld(), dialog = before.activeNpcDialog;
    const link = dialog?.links?.find(value => ["@buy", "@buysell"].includes(value.target.toLowerCase()));
    if (!dialog || !link || !before.entities.some(value => value.kind === "npc" && value.objectId === dialog.npcObjectId)) throw new Error("The current NPC has no observed buy link.");
    await this.#request({ type: "selectNpcDialog", target: link.target }, ["NPCGoods"], event => event.payload?.panelType === 0);
    return this.snapshot().npcGoods;
  }

  beginNpcPurchase() { return this.#npcPurchase({ kind: "begin" }); }
  quoteNpcPurchase(request) { return this.#npcPurchase({ kind: "quote", request: purchaseRequest(request) }); }
  purchaseNpcItem(operation) { return this.#npcPurchase({ kind: "purchase", operation: purchaseOperation(operation) }); }
  queryNpcPurchase(operation) { return this.#npcPurchase({ kind: "query", operation: purchaseOperation(operation) }); }

  async #npcPurchase(action) {
    const before = this.#requireInWorld();
    purchaseAction(action);
    if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
    if (action.kind !== "begin" && !this.#purchaseProducer) throw new Error("Begin the current authenticated NPC purchase producer first.");
    if (action.operation && (action.operation.actor !== this.#purchaseProducer.actor || action.kind === "purchase" && action.operation.requestScope !== this.#purchaseProducer.producerScope)) throw new Error("NPC purchase operation belongs to another actor or retired producer.");
    if (action.kind === "begin") this.#purchaseProducer = null;
    const request = { type: "npcPurchaseOwner", protocolVersion: 1, requestId: (++this.#purchaseSequence).toString(), action };
    u64(request.requestId, true);
    const token = Symbol("npc-purchase"); this.#requestInFlight = token;
    try {
      if (!this.#purchaseOptedIn) { this.#send({ type: "clientCapabilities", capabilities: [PURCHASE_CAPABILITY] }); this.#purchaseOptedIn = true; }
      return await new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          this.#purchaseRequests.delete(request.requestId);
          const error = new Error("NPC purchase result unknown; query the original operation without replaying a purchase.");
          error.code = "NPC_PURCHASE_UNKNOWN"; reject(error);
        }, this.timeoutMs);
        this.#purchaseRequests.set(request.requestId, { request, before, generation: this.#generation, resolve, reject, timer });
        try { this.#send(request); }
        catch (error) { clearTimeout(timer); this.#purchaseRequests.delete(request.requestId); reject(error); }
      });
    } finally { if (this.#requestInFlight === token) this.#requestInFlight = null; }
  }

  async dropOwnedItem(uniqueId, count = 1) {
    const state = this.#requireInWorld();
    const item = state.inventoryItems.find((entry) => String(entry.uniqueId) === String(uniqueId));
    if (!item || state.inventoryStale || !Number.isSafeInteger(count) || count < 1 || count > item.quantity) throw new Error("Invalid owned item drop.");
    const after = this.#sequence;
    await this.#request({ type: "dropItem", key: item.key, uniqueId: item.uniqueId, count, heroInventory: false }, ["DropItem"],
      event => event.payload?.success === true,
      event => String(event.payload?.uniqueId) === String(item.uniqueId) && event.payload?.count === count && event.payload?.heroInventory === false);
    await this.waitFor(event => event.type === "worldSnapshot" &&
      (this.snapshot().inventoryItems.find(entry => String(entry.uniqueId) === String(item.uniqueId))?.quantity ?? 0) < item.quantity, { after });
    return this.snapshot();
  }

  async equipOwnedItem(uniqueId, to) {
    const state = this.#requireInWorld();
    const item = state.inventoryItems.find((entry) => String(entry.uniqueId) === String(uniqueId));
    if (!item || !Number.isSafeInteger(to) || to < 0 || to > 13) throw new Error("Invalid owned equipment selection.");
    const after = this.#sequence;
    await this.#request({ type: "equipItem", uniqueId: item.uniqueId, grid: "inventory", to }, ["EquipItem"],
      event => event.payload?.success === true,
      event => String(event.payload?.uniqueId) === String(item.uniqueId) && event.payload?.to === to && String(event.payload?.grid).toLowerCase() === "inventory");
    await this.waitFor(event => event.type === "worldSnapshot" && !this.snapshot().inventoryItems.some(entry => String(entry.uniqueId) === String(item.uniqueId)), { after });
    return this.snapshot();
  }

  async removeOwnedEquipment(slot) {
    const state = this.#requireInWorld();
    const reference = { weapon: 0, armour: 1 }[slot];
    const item = state.equipmentItems.find(item => item.slot === slot);
    const occupied = new Set(state.inventoryItems.filter(item => item.container === "bag1").map(item => item.slot));
    const to = Array.from({ length: 46 }, (_, index) => index).find(index => !occupied.has(index));
    if (reference === undefined || !item || state.inventoryStale || to === undefined) throw new Error("Owned equipment and an empty inventory slot must be observed before removal.");
    const after = this.#sequence;
    // Existing Crystal-compatible RemoveItem addresses a worn slot using its
    // public equipment reference (0/1), not an inventory item's unique UID.
    await this.#request({ type: "removeItem", uniqueId: reference, grid: "inventory", to }, ["RemoveItem"],
      event => event.payload?.success === true,
      event => event.payload?.uniqueId === reference && event.payload?.to === to && String(event.payload?.grid).toLowerCase() === "inventory");
    await this.waitFor(event => event.type === "worldSnapshot" && !this.snapshot().equipmentItems.some(entry => entry.slot === slot) &&
      this.snapshot().inventoryItems.some(entry => entry.key === item.key && entry.name === item.name), { after });
    return this.snapshot();
  }

  async sayNearby(text) {
    this.#requireInWorld();
    const message = prepareNearbyChat(text);
    if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
    this.#send({ type: "chat", message });
    // Normal Chat has no request ID; same-text echoes cannot prove the speaker.
    // A separate observer can bind ObjectPlayer -> ObjectChat for acceptance.
    return { sent: true, confirmed: false };
  }

  /** The normal inviter packet addresses the mutually facing player, not a name. */
  async requestPlayerTrade() {
    const state = this.#requireInWorld();
    if (!this.#currentTradeSnapshot(state) || state.trade.active || state.trade.uncertain || state.tradePendingInvite) throw new Error("A complete idle player trade observation is required.");
    const offsets = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]], direction = DIRECTIONS.indexOf(state.player.direction);
    if (direction < 0) throw new Error("Observe the player's facing before requesting a trade.");
    const [dx, dy] = offsets[direction];
    const peers = state.entities.filter(value => value.kind === "player" && value.x === state.player.x + dx && value.y === state.player.y + dy && value.dead !== true && value.hp !== 0);
    if (peers.length !== 1 || DIRECTIONS.indexOf(peers[0].direction) !== (direction + 4) % 8) throw new Error("Observe one adjacent mutually facing player before requesting a trade.");
    if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
    this.#send({ type: "tradeRequest" });
    return { sent: true, confirmed: false };
  }

  /** Crystal adds this positive delta to held gold and debits the wallet now.
   * It is not a replacement total and an unknown result must not be replayed. */
  async offerTradeGold(amount) {
    const before = this.#editablePlayerTrade(), tradeId = before.trade.id;
    if (!Number.isSafeInteger(amount) || amount < 1 || amount > 0xffff_ffff || amount > before.gold || before.trade.ownGold + amount > 0xffff_ffff) throw new Error("The positive trade gold delta must fit the current available wallet.");
    try {
      await this.#perform({ type: "tradeGold", amount }, event => {
        const current = this.snapshot();
        return event.type === "worldSnapshot" && this.#sameTradeActor(before, current) && this.#currentTradeSnapshot(current) && current.fullSnapshot.revision > before.revision &&
          (!current.trade.active || current.trade.id !== tradeId || current.gold !== before.gold || current.trade.ownGold !== before.trade.ownGold);
      });
      const actual = this.snapshot();
      if (actual.trade.active !== true || actual.trade.uncertain || actual.trade.id !== tradeId || actual.trade.ownOfferKnown !== true || actual.trade.ownGold !== before.trade.ownGold + amount || actual.gold !== before.gold - amount) throw new Error("Trade gold custody did not match the exact delta.");
      return { sent: true, offerObserved: true, tradeId, offeredGold: actual.trade.ownGold, walletGold: actual.gold };
    } catch (cause) {
      this.#markOwnerTradeUnknown(tradeId);
      const error = new Error("Player trade gold result is unknown; do not replay the additive offer."); error.code = "PLAYER_TRADE_UNKNOWN";
      if (cause?.code === "GATEWAY_RESULT_TIMEOUT") error.timedOut = true;
      throw error;
    }
  }

  async confirmPlayerTrade() {
    const before = this.#editablePlayerTrade(), tradeId = before.trade.id;
    try {
      await this.#perform({ type: "tradeConfirm", locked: true }, event => event.type === "packet" && ["TradeConfirm", "TradeCancel"].includes(event.packet) || event.type === "worldSnapshot" && this.#sameTradeActor(before, this.snapshot()) && this.#currentTradeSnapshot(this.snapshot()) && this.snapshot().fullSnapshot.revision > before.revision);
      const actual = this.snapshot();
      return { sent: true, confirmed: false, tradeId,
        prepared: this.#sameTradeActor(before, actual) && this.#currentTradeSnapshot(actual) && actual.trade.active === true && actual.trade.id === tradeId && actual.trade.ownOfferKnown === true && actual.trade.locked === true && actual.trade.escrowPrepared === true };
    } catch {
      this.#markOwnerTradeUnknown(tradeId);
      const error = new Error("Player trade confirmation is unknown; wait for its final receipt without replaying confirmation."); error.code = "PLAYER_TRADE_UNKNOWN"; throw error;
    }
  }

  #currentTradeSnapshot(state) {
    const full = state.fullSnapshot;
    return state.ready === true && state.inventoryStale === false && Number.isSafeInteger(state.gold) && state.gold >= 0 && state.gold <= 0xffff_ffff &&
      full?.inventoryComplete === true && full.tradeKnown === true && full.connectionEpoch === state.connectionEpoch && full.characterEpoch === state.characterEpoch && full.playerObjectId === state.playerObjectId && full.playerName === state.player?.name && full.mapFileName === state.mapFileName;
  }
  #sameTradeActor(before, current) {
    return before.connectionEpoch === current.connectionEpoch && before.characterEpoch === current.characterEpoch && before.playerObjectId === current.playerObjectId && before.player?.name === current.player?.name && before.mapFileName === current.mapFileName;
  }
  #editablePlayerTrade() {
    const state = this.#requireInWorld(), trade = state.trade;
    if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
    if (!this.#currentTradeSnapshot(state) || !trade?.active || trade.uncertain || !trade.id || trade.ownOfferKnown !== true || !Number.isSafeInteger(trade.ownGold) || trade.ownGold < 0 || trade.locked || trade.escrowPrepared || this.#ownerTradeUnknown.has(trade.id)) throw new Error("A complete current unlocked player trade with a known outcome is required.");
    return state;
  }
  #markOwnerTradeUnknown(tradeId) {
    this.#ownerTradeUnknown.add(tradeId);
    if (this.#ownerTradeUnknown.size > 128) this.#ownerTradeUnknown.delete(this.#ownerTradeUnknown.values().next().value);
  }

  /** Receive-only gifts use the existing player trade packets, never an asset grant. */
  async acceptOwnerGiftInvite(ownerName, inviteId) {
    const before = this.#requireInWorld(), invite = before.tradePendingInvite;
    if (!invite || invite.name !== ownerName || invite.id !== inviteId || before.trade?.active || before.trade?.uncertain || invite.connectionEpoch !== before.connectionEpoch || invite.characterEpoch !== before.characterEpoch) throw new Error("The current owner trade invitation is required.");
    await this.#perform({ type: "tradeReply", acceptInvite: true }, event => {
      const actual = this.snapshot();
      return event.type === "worldSnapshot" && actual.ready && actual.connectionEpoch === before.connectionEpoch && actual.characterEpoch === before.characterEpoch && actual.playerObjectId === before.playerObjectId && actual.trade?.active === true && actual.trade.partnerName === ownerName && actual.trade.ownOfferKnown === true;
    });
    return { accepted: true, tradeId: this.snapshot().trade.id };
  }

  async confirmReceiveOnlyGift({ tradeId, ownerName } = {}) {
    const before = this.#requireInWorld(), trade = before.trade;
    if (!trade?.active || trade.uncertain || trade.id !== tradeId || trade.partnerName !== ownerName || trade.ownOfferKnown !== true || trade.ownGold !== 0 || trade.ownItems.length !== 0 || trade.locked || trade.escrowPrepared || before.inventoryStale || !before.fullSnapshot?.inventoryComplete || !before.fullSnapshot?.tradeKnown) throw new Error("A complete current zero-outgoing owner trade is required.");
    await this.#perform({ type: "tradeConfirm", locked: true }, event => event.type === "packet" && ["TradeConfirm", "TradeCancel"].includes(event.packet) || event.type === "worldSnapshot" && this.snapshot().fullSnapshot?.revision > before.revision);
    return { sent: true, confirmed: false };
  }

  async cancelGiftTrade() {
    const before = this.#requireInWorld();
    if (!before.trade?.active && !before.trade?.uncertain && !before.tradePendingInvite) return { sent: false, confirmed: false };
    if (!before.trade?.active && !before.trade?.uncertain && before.tradePendingInvite) {
      if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
      this.#send({ type: "tradeReply", acceptInvite: false });
      // Crystal sends no reply to a declined invitation. Forget only this local
      // invitation intent; this does not claim cancellation or asset delivery.
      this.observation.noteTradeInviteDeclined(before.tradePendingInvite.id);
      return { sent: true, confirmed: false };
    }
    await this.#perform({ type: "tradeCancel" }, event => event.type === "packet" && event.packet === "TradeCancel" || event.type === "worldSnapshot" && this.snapshot().fullSnapshot?.revision > before.revision);
    return { sent: true, confirmed: false };
  }

  async logout() {
    this.#requireAuthentication();
    this.#ownResidentId = null;
    this.#ownerTradeUnknown.clear();
    const after = this.#sequence;
    await this.#request({ type: "logOut" }, ["LogOutSuccess", "LogOutFailed"], (event) => event.packet === "LogOutSuccess");
    await this.waitFor((event) => event.type === "worldSnapshot" && !this.snapshot().ready, { after });
    return true;
  }

  /** Only sanitized inbound game events reach observers; identity/resume tokens never do. */
  waitFor(predicate, { after = this.#sequence, timeoutMs = this.timeoutMs } = {}) {
    if (this.#failure) return Promise.reject(this.#failure);
    try {
      const found = this.#frames.find((frame) => frame.sequence > after && predicate(frame.event));
      if (found) return Promise.resolve(found.event);
    } catch (error) { return Promise.reject(error); }
    return new Promise((resolve, reject) => {
      const waiter = { predicate, after, resolve, reject, timer: null };
      waiter.timer = setTimeout(() => {
        this.#waiters.delete(waiter);
        const error = new Error("Timed out waiting for the expected gateway result.");
        error.code = "GATEWAY_RESULT_TIMEOUT";
        reject(error);
      }, timeoutMs);
      this.#waiters.add(waiter);
    });
  }

  async close() {
    if (this.#closingPromise) return this.#closingPromise;
    this.#closingPromise = this.#closeConnection();
    try { await this.#closingPromise; } finally { this.#closingPromise = null; }
  }

  async #closeConnection() {
    clearInterval(this.#heartbeat);
    const socket = this.#socket;
    this.#generation += 1;
    this.#socket = null;
    this.#authenticated = false;
    this.#fail(new Error("Gateway client closed."));
    this.observation.resetConnection();
    if (!socket || socket.readyState === 3) return;
    const closed = new Promise((resolve) => {
      const timer = setTimeout(resolve, 2_000);
      socket.addEventListener("close", () => { clearTimeout(timer); resolve(); }, { once: true });
    });
    if (socket.readyState === 0 || socket.readyState === 1) {
      try { socket.close(); } catch { /* a failed handshake can already be closed */ }
    }
    await closed;
  }

  #receive(raw) {
    let event;
    try {
      if (typeof raw !== "string" || Buffer.byteLength(raw) > 2 * 1024 * 1024) throw new Error("Invalid gateway frame.");
      event = parseGatewayJson(raw);
      if (!event || typeof event !== "object") throw new Error("Invalid gateway event.");
      if (event.type === "npcPurchaseOwner") {
        const pending = this.#purchaseRequests.get(event.requestId);
        if (!pending) return; // A late/unsolicited owner frame never updates state.
        const frame = parsePurchaseFrame(raw, pending.request);
        const current = this.#requireInWorld();
        if (pending.generation !== this.#generation || pending.before.connectionEpoch !== current.connectionEpoch || pending.before.playerObjectId !== current.playerObjectId || pending.before.mapFileName !== current.mapFileName) throw new Error("Stale NPC purchase actor.");
        if (frame.authority) {
          if (pending.request.action.kind !== "begin" && (!this.#purchaseProducer || frame.authority.actor !== this.#purchaseProducer.actor || frame.authority.producerScope !== this.#purchaseProducer.producerScope || BigInt(frame.authority.serverRevision) < BigInt(this.#purchaseProducer.serverRevision))) throw new Error("Stale NPC purchase authority.");
          this.observation.applyNpcPurchaseSnapshot(frame.snapshot, frame.authority);
          if (frame.reply.kind === "producer" && !same(frame.reply.producer, frame.authority)) throw new Error("NPC purchase Begin has no matching authority.");
          this.#purchaseProducer = structuredClone(frame.authority);
        } else if (["producer", "quote"].includes(frame.reply.kind)) throw new Error("NPC purchase preparation requires a complete authority snapshot.");
        clearTimeout(pending.timer); this.#purchaseRequests.delete(frame.requestId);
        pending.resolve(structuredClone(frame));
        return;
      }
      if (["companionRequest", "companionResult"].includes(event.type)) {
        if (Buffer.byteLength(raw) > 64 * 1024 || !event.payload || typeof event.payload !== "object" || Array.isArray(event.payload)) throw new Error("Invalid private companion frame.");
        const value = event.payload;
        if (typeof value.requestId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(value.requestId)) throw new Error("Invalid companion correlation.");
        if (event.type === "companionRequest") {
          if (this.features.companionControl !== true || !this.worldEpoch || !this.#authenticated || !this.snapshot().ready) return;
          this.emit("companionRequest", structuredClone(value));
        } else {
          const pending = this.#controlRequests.get(value.requestId);
          if (!pending || pending.operation !== value.operation) return;
          clearTimeout(pending.timer);
          this.#controlRequests.delete(value.requestId);
          pending.resolve(structuredClone(value));
        }
        return;
      }
      if (event.type === "error") {
        this.observation.resetConnection();
        this.#fail(new Error("Gateway reported an action or routing error; its result is unconfirmed."));
        this.#socket?.close();
        return;
      }
      if (!["realmInfo", "packet", "worldSnapshot"].includes(event.type)) return;
      if (event.type === "realmInfo") {
        const { realmId, profileId, profileVersion, bundleHash, worldEpoch } = event.payload ?? {};
        event = { type: "realmInfo", payload: { realmId, profileId, profileVersion, bundleHash,
          ...(typeof worldEpoch === "string" ? { worldEpoch } : {}),
          features: { sharedParty: event.payload?.features?.sharedParty === true, companionControl: event.payload?.features?.companionControl === true } } };
      }
      const before = this.snapshot();
      this.observation.apply(event);
      const state = this.snapshot();
      if (event.type === "packet" && ["StartGame", "LogOutSuccess", "ReturnToLogin"].includes(event.packet) || before.ready && (before.playerObjectId !== state.playerObjectId || before.player?.name !== state.player?.name)) this.#ownResidentId = null;
      if (state.ready && !this.allowedMaps.has(state.mapFileName)) throw new Error("Player left the configured map allowlist.");
    } catch {
      this.#fail(new Error("Gateway observation failed validation."));
      this.#socket?.close();
      return;
    }
    const frame = { sequence: ++this.#sequence, event };
    this.#frames.push(frame);
    if (this.#frames.length > 128) this.#frames.shift();
    for (const waiter of [...this.#waiters]) {
      if (frame.sequence <= waiter.after) continue;
      try {
        if (!waiter.predicate(event)) continue;
        clearTimeout(waiter.timer);
        this.#waiters.delete(waiter);
        waiter.resolve(event);
      } catch (error) {
        clearTimeout(waiter.timer);
        this.#waiters.delete(waiter);
        waiter.reject(error);
      }
    }
    this.emit("frame", frame);
  }

  #fail(error) {
    if (!this.#failure) {
      this.#authenticated = false;
      this.observation.resetConnection();
    }
    this.#failure ??= error;
    for (const waiter of this.#waiters) {
      clearTimeout(waiter.timer);
      waiter.reject(this.#failure);
    }
    this.#waiters.clear();
    this.#requestInFlight = null;
    this.#purchaseProducer = null;
    this.#purchaseOptedIn = false;
    this.#activeCharacterIndex = null;
    this.#ownResidentId = null;
    this.#ownerTradeUnknown.clear();
    for (const request of this.#purchaseRequests.values()) { clearTimeout(request.timer); request.reject(this.#failure); }
    this.#purchaseRequests.clear();
    for (const request of this.#controlRequests.values()) { clearTimeout(request.timer); request.reject(error); }
    this.#controlRequests.clear();
  }

  #send(command) {
    if (this.#failure) throw this.#failure;
    if (!this.#socket || this.#socket.readyState !== 1) throw new Error("Gateway is not connected.");
    this.#socket.send(stringifyGatewayJson(command));
    this.emit("command", { type: command.type });
  }

  async #request(command, packetNames, accept = () => true, match = () => true) {
    return this.#perform(command, (message) => message.type === "packet" && packetNames.includes(message.packet) && match(message), accept);
  }

  async #perform(command, match, accept = () => true, waitOptions = {}) {
    if (this.#requestInFlight) throw new Error("Wait for the current player action before sending another.");
    const request = Symbol("player-action");
    this.#requestInFlight = request;
    try {
      const after = this.#sequence;
      this.#send(command);
      const event = await this.waitFor(match, { ...waitOptions, after });
      if (!accept(event)) {
        const result = Number.isSafeInteger(event.payload?.result) ? event.payload.result : "unavailable";
        throw new Error(`Gateway rejected ${command.type} (${event.packet}, result ${result}).`);
      }
      return event;
    } finally {
      if (this.#requestInFlight === request) this.#requestInFlight = null;
    }
  }

  #requireAuthentication() {
    if (!this.#authenticated) throw new Error("An authenticated account is required.");
  }

  #requireSharedParty() {
    this.#requireInWorld();
    if (!this.features.sharedParty) throw new Error("This gateway has not enabled authoritative shared parties.");
  }

  #requireCompanionControl() {
    this.#requireInWorld();
    if (!this.features.companionControl || !this.worldEpoch) throw new Error("This world has not enabled trusted companion controls.");
  }

  #requireInWorld() {
    this.#requireAuthentication();
    const state = this.snapshot();
    if (!state.ready || !state.player) throw new Error("Player has not entered the world.");
    if (!this.allowedMaps.has(state.mapFileName)) throw new Error("Player is outside the map allowlist.");
    return state;
  }
}
