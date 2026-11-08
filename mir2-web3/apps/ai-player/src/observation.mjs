const ENTITY_FIELDS = ["objectId", "kind", "name", "ownerName", "ai", "x", "y", "direction", "class", "gender", "level", "hp", "maxHp", "dead", "disposition", "ridingMount", "canMountAttack", "hasClassWeapon", "dazed", "fishing"];
const ITEM_FIELDS = ["key", "name", "icon", "uniqueId", "slot", "container", "quantity", "description", "durabilityCurrent", "durabilityMax", "sellValue", "equipSlot", "grade", "addedAttack", "addedDefence"];
const EQUIPMENT_FIELDS = ["slot", "key", "quantity", "name", "icon", "shape", "description", "durabilityCurrent", "durabilityMax", "grade", "attack", "defence", "addedAttack", "addedDefence", "addedLuck", "socketSlots"];
const QUEST_FIELDS = ["questId", "title", "summary", "objective", "progressLabel", "tracker", "stage", "current", "required", "rewardPreview"];
const SNAPSHOT_FIELDS = ["tick", "mapFileName", "mapTitle", "inSafeZone", "playerObjectId", "selfZoneObjectId", "playerHp", "playerMaxHp", "playerMp", "playerMaxMp", "playerExperience", "playerMaxExperience", "gold", "currentWeight", "maxWeight", "freeBagSlots", "maxBagSlots"];
const KINDS = new Set(["selfPlayer", "player", "monster", "npc"]);
const ITEM_MUTATIONS = new Set(["GainedItem", "RefreshItem", "DeleteItem", "UseItem", "DropItem", "EquipItem", "RemoveItem", "ItemMoved", "ItemMerged", "SplitItem1", "RepairItem", "DuraChanged"]);
const ENGAGEMENT_TTL_MS = 6000;
const MAX_ENGAGEMENTS = 64;
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const scalar = value => value === null || ["string", "number", "bigint", "boolean"].includes(typeof value);
const objectId = value => Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
const copy = value => structuredClone(value);
const pick = (value, fields) => Object.fromEntries(fields.filter(key => record(value) && Object.hasOwn(value, key) && scalar(value[key])).map(key => [key, value[key]]));
const list = (value, project) => Array.isArray(value) ? value.filter(record).map(project).filter(Boolean) : [];
const position = value => {
  const point = record(value?.location) ? value.location : value;
  return Number.isInteger(point?.x) && Number.isInteger(point?.y) ? { x: point.x, y: point.y } : {};
};
const groupName = value => typeof value === "string" && value.length > 0 && value.length <= 64 && !/[\p{Cc}\p{Cf}\p{Cs}]/u.test(value);
const groupNames = values => Array.isArray(values) ? [...new Set(values.filter(groupName))].slice(0, 64) : [];
const emptyParty = () => ({ allowGroup: false, members: [], leaderName: null, pendingInvite: null, memberInfo: [] });
const liveActor = value => value && ["selfPlayer", "player", "monster"].includes(value.kind) && value.dead !== true && (value.hp == null || value.hp > 0);
const validUid = value => typeof value === "bigint" ? value >= 0n && value <= 18446744073709551615n : Number.isSafeInteger(value) && value >= 0;

function clock(nowMs) {
  if (!Number.isSafeInteger(nowMs) || nowMs < 0) throw new TypeError("Observation time must be a nonnegative safe integer");
  return nowMs;
}

function entity(value, kind = value.kind) {
  if (!objectId(value.objectId) || !KINDS.has(kind)) return null;
  const projected = { ...pick(value, ENTITY_FIELDS), ...position(value), kind };
  if (Array.isArray(value.questIds)) projected.questIds = value.questIds.filter(Number.isSafeInteger);
  return projected;
}

function quest(value) {
  if (!Number.isSafeInteger(value.questId)) return null;
  return { ...pick(value, QUEST_FIELDS), objectives: list(value.objectives, objective => pick(objective, ["label", "current", "required", "done"])) };
}

function dialog(value) {
  if (!record(value)) return null;
  return {
    ...pick(value, ["npcObjectId", "npcName", "title", "footer"]),
    body: Array.isArray(value.body) ? value.body.filter(line => typeof line === "string") : [],
    links: list(value.links, link => typeof link.text === "string" && typeof link.target === "string" ? { text: link.text, target: link.target } : null),
    input: record(value.input) ? pick(value.input, ["target", "prompt"]) : null,
  };
}

function drop(value) {
  if (!objectId(value.objectId)) return null;
  // The private exactItem tree is never copied even if a malformed source includes it.
  return { ...pick(value, ["objectId", "name", "icon", "x", "y", "quantity", "sourceMonster", "ownerObjectId", "ownershipRemainingTicks"]), ...position(value) };
}

function initial(connectionEpoch = 0, revision = 0) {
  return {
    connectionEpoch, revision, ready: false, tick: null, mapIndex: null, mapFileName: null, mapTitle: null,
    inSafeZone: false, playerObjectId: null, selfZoneObjectId: null, playerHp: null, playerMaxHp: null, playerMp: null, playerMaxMp: null,
    playerExperience: null, playerMaxExperience: null, gold: null, currentWeight: null, maxWeight: null,
    freeBagSlots: null, maxBagSlots: null, player: null, entities: [], groundDrops: [], inventoryItems: [],
    beltItems: [], equipmentItems: [], questLog: [], activeNpcDialog: null, npcGoods: null, npcResponse: [],
    questOperationAck: null, lastChat: null, inventoryStale: true,
    party: emptyParty(), engagements: [], lastDamageAt: null, lastItemUse: null,
  };
}

/** Whitelist ordinary client observations; it never imports a personal ECS or admin projection. */
export class ObservationReducer {
  #state = initial();
  #started = false;
  #hasSnapshot = false;
  #pendingService = false;

  snapshot(nowMs = Date.now()) {
    this.#pruneCombat(clock(nowMs));
    return copy(this.#state);
  }

  resetConnection() {
    this.#state = initial(this.#state.connectionEpoch + 1, this.#state.revision + 1);
    this.#started = false;
    this.#hasSnapshot = false;
    this.#pendingService = false;
    return this.snapshot();
  }

  apply(event, nowMs = Date.now()) {
    clock(nowMs);
    if (!record(event) || !record(event.payload)) return this.snapshot(nowMs);
    if (event.connectionEpoch !== undefined && event.connectionEpoch !== this.#state.connectionEpoch) return this.snapshot(nowMs);
    const payload = event.payload;
    let handled = false;
    if (event.type === "worldSnapshot") {
      this.#applySnapshot(payload, nowMs);
      handled = true;
    } else if (event.type === "packet" && typeof event.packet === "string") {
      handled = this.#applyPacket(event.packet, payload, nowMs);
    }
    if (handled) {
      this.#state.revision += 1;
      this.#state.ready = Boolean(this.#started && this.#hasSnapshot && Number.isInteger(this.#state.player?.x) && Number.isInteger(this.#state.player?.y) && typeof this.#state.mapFileName === "string" && this.#state.mapFileName.length);
    }
    return this.snapshot(nowMs);
  }

  #applySnapshot(payload, nowMs) {
    const previous = this.#state;
    this.#state = { ...initial(previous.connectionEpoch, previous.revision), ...pick(payload, SNAPSHOT_FIELDS), mapIndex: previous.mapIndex };
    const state = this.#state;
    state.selfZoneObjectId = objectId(payload.selfZoneObjectId) ? payload.selfZoneObjectId : null;
    state.entities = list(payload.entities, value => value.objectId === state.selfZoneObjectId && value.objectId !== state.playerObjectId ? null : entity(value));
    state.player = state.entities.find(value => value.kind === "selfPlayer" && value.objectId === state.playerObjectId) ?? null;
    state.groundDrops = list(payload.groundDrops, drop);
    state.inventoryItems = list(payload.inventoryItems, value => pick(value, ITEM_FIELDS));
    state.beltItems = list(payload.beltItems, value => pick(value, ITEM_FIELDS));
    state.equipmentItems = list(payload.equipmentItems, value => pick(value, EQUIPMENT_FIELDS));
    state.questLog = list(payload.questLog, quest);
    const sameMap = previous.mapFileName === state.mapFileName;
    state.party = sameMap ? copy(previous.party) : emptyParty();
    state.party.pendingInvite = copy(previous.party.pendingInvite);
    const group = record(payload.stage5Systems?.group) ? payload.stage5Systems.group : null;
    if (group) {
      if (typeof group.allowGroup === "boolean") state.party.allowGroup = group.allowGroup;
      if (Array.isArray(group.members)) {
        state.party.members = groupNames(group.members);
        state.party.leaderName = state.party.members[0] ?? null;
        state.party.memberInfo = state.party.memberInfo.filter(value => state.party.members.includes(value.name));
      }
    }
    this.#confirmGroupMembership();
    if (sameMap) {
      const selfBindingChanged = previous.playerObjectId !== state.playerObjectId || previous.selfZoneObjectId !== state.selfZoneObjectId;
      const previousSelfIds = new Set([previous.playerObjectId, previous.selfZoneObjectId].filter(objectId));
      state.engagements = copy(previous.engagements.filter(value => !selfBindingChanged || (!previousSelfIds.has(value.attackerId) && !previousSelfIds.has(value.targetId))));
      state.lastItemUse = copy(previous.lastItemUse);
      for (const value of state.entities) {
        const old = previous.entities.find(prior => prior.objectId === value.objectId);
        if (old?.lastAction) value.lastAction = copy(old.lastAction);
      }
      if (state.playerObjectId === previous.playerObjectId) {
        state.lastDamageAt = previous.lastDamageAt;
        this.#noteDamage(previous.playerHp, state.playerHp, nowMs);
      }
    }
    state.activeNpcDialog = dialog(payload.activeNpcDialog);
    state.questOperationAck = record(payload.questOperationAck) ? pick(payload.questOperationAck, ["operation", "requestId", "npcIndex", "questIndex", "selectedItemIndex", "success"]) : null;
    // Shop service packets are not part of WorldSnapshot; retain only on the same map and NPC service.
    const sameService = previous.mapFileName === state.mapFileName && state.activeNpcDialog !== null && (this.#pendingService || previous.activeNpcDialog?.npcObjectId === state.activeNpcDialog?.npcObjectId);
    state.npcGoods = sameService ? previous.npcGoods : null;
    state.npcResponse = sameService ? previous.npcResponse : [];
    state.inventoryStale = false;
    this.#hasSnapshot = state.player !== null;
    this.#pendingService = false;
  }

  #upsertEntity(value) {
    const index = this.#state.entities.findIndex(current => current.objectId === value.objectId);
    if (index < 0) this.#state.entities.push(value);
    else this.#state.entities[index] = { ...this.#state.entities[index], ...value };
    this.#state.player = this.#state.entities.find(current => current.kind === "selfPlayer" && current.objectId === this.#state.playerObjectId) ?? null;
  }

  #updateEntity(id, changes) {
    const current = this.#actorForId(id);
    if (current) this.#upsertEntity({ ...current, ...changes });
  }

  #isSelfId(id) {
    return objectId(id) && (id === this.#state.playerObjectId || id === this.#state.selfZoneObjectId);
  }

  #actorForId(id) {
    if (this.#isSelfId(id)) return this.#state.player;
    return this.#state.entities.find(value => value.objectId === id);
  }

  #clearWorld() {
    this.#state.entities = [];
    this.#state.groundDrops = [];
    this.#state.player = null;
    this.#state.selfZoneObjectId = null;
    this.#state.activeNpcDialog = null;
    this.#state.npcGoods = null;
    this.#state.npcResponse = [];
    this.#state.lastChat = null;
    this.#state.engagements = [];
    this.#state.lastDamageAt = null;
    this.#state.lastItemUse = null;
    this.#hasSnapshot = false;
    this.#pendingService = false;
  }

  #confirmGroupMembership() {
    const { party, player } = this.#state;
    if (player && party.members.length > 1 && party.members.includes(player.name)) party.pendingInvite = null;
  }

  #noteDamage(previousHp, currentHp, nowMs) {
    if (Number.isFinite(previousHp) && Number.isFinite(currentHp) && currentHp < previousHp) this.#state.lastDamageAt = nowMs;
  }

  #removeEngagements(id) {
    const removed = this.#isSelfId(id) ? new Set([this.#state.playerObjectId, this.#state.selfZoneObjectId].filter(objectId)) : new Set([id]);
    this.#state.engagements = this.#state.engagements.filter(value => !removed.has(value.attackerId) && !removed.has(value.targetId));
  }

  #pruneCombat(nowMs) {
    const state = this.#state;
    const visible = new Set(state.entities.filter(liveActor).map(value => value.objectId));
    if (liveActor(state.player) && objectId(state.selfZoneObjectId)) visible.add(state.selfZoneObjectId);
    const retained = state.engagements.filter(value => visible.has(value.attackerId) && visible.has(value.targetId) && nowMs >= value.observedAt && nowMs - value.observedAt < ENGAGEMENT_TTL_MS).slice(-MAX_ENGAGEMENTS);
    let changed = retained.length !== state.engagements.length;
    state.engagements = retained;
    for (const value of state.entities) {
      if (value.lastAction && (!liveActor(value) || nowMs < value.lastAction.observedAt || nowMs - value.lastAction.observedAt >= ENGAGEMENT_TTL_MS)) {
        delete value.lastAction;
        changed = true;
      }
    }
    if (changed) state.revision += 1;
  }

  #applyPacket(name, payload, nowMs) {
    const state = this.#state;
    if (name === "StartGame") {
      this.#clearWorld();
      this.#started = payload.result === 4;
    } else if (name === "LogOutSuccess" || name === "ReturnToLogin") {
      this.#state = initial(state.connectionEpoch, state.revision);
      this.#started = false;
      this.#hasSnapshot = false;
      this.#pendingService = false;
    } else if (name === "MapInformation" || name === "MapChanged") {
      if (name === "MapChanged" || payload.fileName !== state.mapFileName || payload.mapIndex !== state.mapIndex) this.#clearWorld();
      Object.assign(state, pick(payload, ["mapIndex"]), { mapFileName: payload.fileName ?? null, mapTitle: payload.title ?? null });
    } else if (name === "UserInformation" && objectId(payload.objectId)) {
      state.playerObjectId = payload.objectId;
      Object.assign(state, { playerHp: payload.hp, playerMaxHp: payload.maxHp, playerMp: payload.mp, playerMaxMp: payload.maxMp, gold: payload.gold, playerExperience: payload.experience, playerMaxExperience: payload.maxExperience });
      this.#upsertEntity(entity(payload, "selfPlayer"));
    } else if (name === "UserLocation" || name === "UserAttackMove") {
      this.#updateEntity(state.playerObjectId, { ...position(payload), ...pick(payload, ["direction"]) });
    } else if (["ObjectPlayer", "ObjectMonster", "ObjectNpc", "NewNpcInfo"].includes(name) && objectId(payload.objectId)) {
      const kind = name === "ObjectPlayer" ? "player" : name === "ObjectMonster" ? "monster" : "npc";
      if (!this.#isSelfId(payload.objectId)) this.#upsertEntity(entity(payload, kind));
    } else if (["ObjectWalk", "ObjectRun", "ObjectTurn", "ObjectBackStep"].includes(name)) {
      // Observer movement/poses may arrive after the owner's UserLocation ACK.
      if (!this.#isSelfId(payload.objectId)) this.#updateEntity(payload.objectId, { ...position(payload), ...pick(payload, ["direction"]) });
    } else if (["ObjectRemove", "ObjectTeleportOut", "ObjectHide"].includes(name)) {
      const removedSelf = this.#isSelfId(payload.objectId);
      this.#removeEngagements(payload.objectId);
      state.entities = state.entities.filter(value => value.objectId !== payload.objectId && (!removedSelf || value.objectId !== state.playerObjectId));
      state.groundDrops = state.groundDrops.filter(value => value.objectId !== payload.objectId);
      if (removedSelf) { state.player = null; state.selfZoneObjectId = null; this.#hasSnapshot = false; }
      if (payload.objectId === state.activeNpcDialog?.npcObjectId) { state.activeNpcDialog = null; state.npcGoods = null; }
    } else if (name === "ObjectItem" || name === "ObjectGold") {
      const observed = drop({ ...payload, ...position(payload), icon: payload.image, quantity: payload.gold, name: name === "ObjectGold" ? `${payload.gold} Gold` : payload.name });
      if (observed) state.groundDrops = [...state.groundDrops.filter(value => value.objectId !== observed.objectId), observed];
    } else if (name === "HealthChanged") {
      this.#noteDamage(state.playerHp, payload.hp, nowMs);
      Object.assign(state, { playerHp: payload.hp, playerMp: payload.mp });
      this.#updateEntity(state.playerObjectId, pick(payload, ["hp"]));
    } else if (name === "Death") {
      this.#removeEngagements(state.playerObjectId);
      state.playerHp = 0;
      this.#updateEntity(state.playerObjectId, { ...position(payload), ...pick(payload, ["direction"]), hp: 0, dead: true });
    } else if (name === "Revived") {
      state.playerHp = null;
      this.#updateEntity(state.playerObjectId, { hp: null, dead: false });
    } else if (name === "ObjectDied" || name === "ObjectRevived") {
      this.#removeEngagements(payload.objectId);
      this.#updateEntity(payload.objectId, { ...position(payload), ...pick(payload, ["direction"]), dead: name === "ObjectDied" });
      if (name === "ObjectDied" && this.#isSelfId(payload.objectId)) {
        state.playerHp = 0;
        this.#updateEntity(payload.objectId, { hp: 0 });
      }
    } else if (name === "ObjectHealth") {
      // Percentage is not absolute HP and does not identify an attacker or helper.
      if (Number.isInteger(payload.percent) && payload.percent >= 0 && payload.percent <= 100) this.#updateEntity(payload.objectId, { healthPercent: payload.percent });
    } else if (name === "ObjectAttack") {
      const actor = this.#actorForId(payload.objectId);
      if (liveActor(actor)) this.#updateEntity(payload.objectId, {
        ...(!this.#isSelfId(payload.objectId) ? { ...position(payload), ...pick(payload, ["direction"]) } : {}),
        lastAction: { kind: "attack", observedAt: nowMs, ...pick(payload, ["direction", "spell", "level", "attackType"]) },
      });
    } else if (name === "ObjectStruck") {
      const attacker = this.#actorForId(payload.attackerId);
      const target = this.#actorForId(payload.objectId);
      if (payload.attackerId !== payload.objectId && attacker !== target && liveActor(attacker) && liveActor(target)) {
        if (!this.#isSelfId(payload.objectId)) this.#updateEntity(payload.objectId, { ...position(payload), ...pick(payload, ["direction"]) });
        state.engagements = [...state.engagements.filter(value => value.attackerId !== payload.attackerId || value.targetId !== payload.objectId), { attackerId: payload.attackerId, targetId: payload.objectId, observedAt: nowMs }].slice(-MAX_ENGAGEMENTS);
      }
    } else if (name === "GainedGold" || name === "LoseGold") {
      if (Number.isSafeInteger(state.gold) && Number.isSafeInteger(payload.gold) && payload.gold >= 0) {
        const next = state.gold + (name === "GainedGold" ? payload.gold : -payload.gold);
        state.gold = next >= 0 && next <= 0xffff_ffff ? next : null;
      }
    } else if (name === "LevelChanged") {
      state.playerExperience = payload.experience;
      state.playerMaxExperience = payload.maxExperience ?? payload.max_experience;
      this.#updateEntity(state.playerObjectId, pick(payload, ["level"]));
    } else if (name === "GainExperience") {
      if (Number.isSafeInteger(payload.amount) && payload.amount >= 0 && (typeof state.playerExperience === "bigint" || Number.isSafeInteger(state.playerExperience))) {
        const next = BigInt(state.playerExperience) + BigInt(payload.amount);
        state.playerExperience = next > BigInt(Number.MAX_SAFE_INTEGER) ? next : Number(next);
      }
    } else if (name === "ChangeQuest" && Number.isSafeInteger(payload.questId)) {
      const index = state.questLog.findIndex(value => value.questId === payload.questId);
      if ((payload.questState ?? payload.state) === 2) state.questLog = state.questLog.filter(value => value.questId !== payload.questId);
      else {
        const value = { ...(index < 0 ? { questId: payload.questId } : state.questLog[index]), ...pick(payload, ["current", "required"]), stage: payload.taken ? (payload.completed ? "readyToTurnIn" : "inProgress") : "available", objectives: list(payload.objectives, value => pick(value, ["label", "current", "required", "done"])) };
        if (index < 0) state.questLog.push(value); else state.questLog[index] = value;
      }
    } else if (name === "CompleteQuest" && Array.isArray(payload.completedQuests)) {
      for (const id of payload.completedQuests.filter(Number.isSafeInteger)) {
        const index = state.questLog.findIndex(value => value.questId === id);
        if (index < 0) state.questLog.push({ questId: id, stage: "completed" }); else state.questLog[index].stage = "completed";
      }
    } else if (name === "NPCGoods") {
      state.npcGoods = { ...pick(payload, ["rate", "panelType", "hideAddedStats"]), list: list(payload.list, item => pick(item, ["id", "uniqueId", "itemIndex", "count", "name", "icon", "price", "grade", "description"])) };
    } else if (name === "NPCResponse" && Array.isArray(payload.page)) {
      state.npcResponse = payload.page.filter(line => typeof line === "string");
      state.activeNpcDialog = null;
      state.npcGoods = null;
      this.#pendingService = true;
    } else if (name === "SwitchGroup" && typeof payload.allowGroup === "boolean") {
      state.party.allowGroup = payload.allowGroup;
    } else if (name === "GroupInvite" && groupName(payload.name)) {
      state.party.pendingInvite = { name: payload.name, observedAt: nowMs };
    } else if (name === "AddMember" && groupName(payload.name)) {
      state.party.members = groupNames([...state.party.members, payload.name]);
      state.party.leaderName ??= state.party.members[0] ?? null;
      this.#confirmGroupMembership();
    } else if (name === "DeleteMember" && groupName(payload.name)) {
      state.party.members = state.party.members.filter(value => value !== payload.name);
      state.party.memberInfo = state.party.memberInfo.filter(value => value.name !== payload.name);
      if (payload.name === state.player?.name) { state.party.members = []; state.party.memberInfo = []; }
      if (!state.party.members.includes(state.party.leaderName)) state.party.leaderName = state.party.members[0] ?? null;
    } else if (name === "DeleteGroup") {
      state.party = { ...emptyParty(), allowGroup: state.party.allowGroup };
    } else if (name === "GroupMemberInfo" && Array.isArray(payload.members)) {
      state.party.memberInfo = list(payload.members, value => groupName(value.name) ? pick(value, ["name", "level", "class", "hp", "maxHp", "online"]) : null).slice(0, 64);
      state.party.members = groupNames(state.party.memberInfo.map(value => value.name));
      state.party.leaderName = state.party.members.includes(payload.leaderName) ? payload.leaderName : state.party.members[0] ?? null;
      this.#confirmGroupMembership();
    } else if (name === "UseItem") {
      if (validUid(payload.uniqueId) && typeof payload.success === "boolean" && typeof payload.grid === "string") {
        state.lastItemUse = { uniqueId: payload.uniqueId, success: payload.success, grid: payload.grid, observedAt: nowMs };
      }
      if (payload.success !== false) state.inventoryStale = true;
    } else if (name === "ObjectChat" && objectId(payload.objectId) && typeof payload.text === "string") {
      const speaker = state.entities.find(value => value.objectId === payload.objectId);
      if (speaker) state.lastChat = { objectId: payload.objectId, name: speaker.name, text: payload.text, ...pick(payload, ["chatType"]), connectionEpoch: state.connectionEpoch };
    } else if (ITEM_MUTATIONS.has(name)) {
      // Full UserItem packets lack container/slot; only a complete snapshot can locate them.
      if (payload.success !== false) state.inventoryStale = true;
    } else return false;
    return true;
  }
}
