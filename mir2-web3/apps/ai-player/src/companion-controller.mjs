import { decideCompanionAction, facingDirection } from "./companion-policy.mjs";
import { loadClientCollision, nextWalkDirection } from "./skills/navigation.mjs";

/** Executes one bounded ordinary action. Policy/feedback never changes game authority. */
export class CompanionController {
  #busy = false;
  #maps = new Map();
  #lastDecision = null;
  #potionPendingUntil = 0;
  #failedPotions = new Map();
  #uncertainPotions = new Map();
  #uncertainLoot = new Map();
  #uncertainInvite = null;

  constructor({ client, store, leaderName, homePosition, report = () => {}, clock = Date.now, mapLoader = loadClientCollision }) {
    if (typeof leaderName !== "string" || !leaderName.trim()) throw new Error("Configure the approved party leader name.");
    this.client = client;
    this.store = store;
    this.leaderName = leaderName;
    this.homePosition = homePosition;
    this.report = report;
    this.clock = clock;
    this.mapLoader = mapLoader;
    this.recallRequested = false;
  }

  teachSupportBeforeLoot(feedbackId) {
    const profile = this.store.teachSupportBeforeLoot({
      source: "local-owner", feedbackId,
      feedbackText: "组队时先支援，再拾取；离开队伍前先问我。",
      context: { mode: "party", leaderRef: `configured:${this.leaderName}` },
    });
    this.report({ type: "companionTaught", policyVersion: profile.policyVersion, party: profile.party });
    return profile;
  }

  #prepareState(state, nowMs) {
    state.recallRequested = this.recallRequested;
    for (const [uid, pending] of this.#uncertainPotions) {
      const item = [...state.inventoryItems, ...state.beltItems].find(value => String(value.uniqueId) === uid);
      const receipt = state.lastItemUse;
      const matchedReceipt = receipt && typeof receipt.grid === "string" && String(receipt.uniqueId) === uid && receipt.grid.toLowerCase() === pending.grid &&
        receipt.observedAt >= pending.startedAt && receipt.observedAt <= nowMs && typeof receipt.success === "boolean";
      const newInventory = !state.inventoryStale && (state.connectionEpoch !== pending.connectionEpoch || state.revision > pending.revision);
      if (matchedReceipt) {
        this.#uncertainPotions.delete(uid);
        if (receipt.success) this.#potionPendingUntil = Math.max(this.#potionPendingUntil, receipt.observedAt + 4_000);
        else this.#failedPotions.set(uid, nowMs + 10_000);
      } else if (newInventory && (item?.quantity ?? 0) < pending.quantity) {
        this.#uncertainPotions.delete(uid);
      }
    }
    for (const [id, pending] of this.#uncertainLoot) {
      if (state.connectionEpoch !== pending.connectionEpoch || state.mapFileName !== pending.mapFileName || !state.groundDrops.some(value => value.objectId === id)) this.#uncertainLoot.delete(id);
    }
    state.groundDrops = state.groundDrops.filter(value => !this.#uncertainLoot.has(value.objectId));
    // A normal potion heals over time. Retreat remains available throughout.
    if (nowMs < this.#potionPendingUntil) state.inventoryStale = true;
    else {
      const usable = item => !this.#uncertainPotions.has(String(item.uniqueId)) && (this.#failedPotions.get(String(item.uniqueId)) ?? 0) <= nowMs;
      state.inventoryItems = state.inventoryItems.filter(usable);
      state.beltItems = state.beltItems.filter(usable);
    }
    return state;
  }

  #sameWorld(before, after) {
    return after.ready && before.connectionEpoch === after.connectionEpoch && before.mapFileName === after.mapFileName && before.mapIndex === after.mapIndex && before.playerObjectId === after.playerObjectId && before.selfZoneObjectId === after.selfZoneObjectId;
  }

  #inviteKey(state) {
    const invite = state.party?.pendingInvite;
    return `${state.connectionEpoch}:${invite?.name}:${invite?.observedAt}`;
  }

  async tick() {
    if (this.#busy) return { kind: "wait", reason: "previous-action-in-flight" };
    this.#busy = true;
    try {
      const nowMs = this.clock();
      const state = this.#prepareState(this.client.snapshot(), nowMs);
      const policy = this.store.profile();
      const action = decideCompanionAction({ state, policy, leaderName: this.leaderName, nowMs, homePosition: this.homePosition });
      const key = `${action.policyVersion}:${action.kind}:${action.reason}:${action.targetId ?? ""}`;
      if (key !== this.#lastDecision) {
        this.store.recordEpisode({ kind: "decision", action: action.kind.toLowerCase(), outcome: "selected", summary: action.reason,
          context: { mode: state.party?.members?.length ? "party" : "solo", sourceEventRef: `observation:${state.connectionEpoch}:${state.revision}` } });
        this.report({ type: "companionDecision", ...action });
        this.#lastDecision = key;
      }
      let outcome = { performed: false };
      if (action.kind === "acceptInvite") {
        const current = this.#prepareState(this.client.snapshot(), this.clock());
        const invitation = this.#inviteKey(current);
        const fresh = decideCompanionAction({ state: current, policy: this.store.profile(), leaderName: this.leaderName, nowMs: this.clock(), homePosition: this.homePosition });
        if (!this.#sameWorld(state, current) || fresh.kind !== "acceptInvite" || invitation !== this.#inviteKey(state) || invitation === this.#uncertainInvite) return { ...action, performed: false, blocked: true };
        try {
          await this.client.acceptGroupInvite();
          this.#uncertainInvite = null;
          outcome = { performed: true, confirmed: true };
        } catch (error) {
          this.#uncertainInvite = invitation;
          this.store.recordEpisode({ kind: "party", action: "accept-invite", outcome: "uncertain", summary: "Invitation acceptance result unknown; the same invitation is not replayed." });
          throw error;
        }
      } else if (action.kind === "usePotion") {
        // Set before sending so an unknown result cannot be blindly replayed.
        this.#potionPendingUntil = nowMs + 4_000;
        const item = [...state.inventoryItems, ...state.beltItems].find(value => String(value.uniqueId) === String(action.uniqueId));
        this.#uncertainPotions.set(String(action.uniqueId), { quantity: item.quantity, grid: action.grid, connectionEpoch: state.connectionEpoch, revision: state.revision, startedAt: nowMs });
        let receipt;
        try {
          receipt = await this.client.useOwnedItem(action.uniqueId, action.grid);
          if (typeof receipt?.success !== "boolean" || receipt.uniqueId !== undefined && String(receipt.uniqueId) !== String(action.uniqueId)) throw new Error("Item use returned no matching authoritative receipt.");
        } catch (error) {
          this.#potionPendingUntil = Math.max(this.#potionPendingUntil, this.clock() + 4_000);
          this.store.recordEpisode({ kind: "item-use", action: "use-potion", outcome: "uncertain", objectRef: `item:${action.uniqueId}`, summary: "Item use result unknown; the exact item is quarantined until observed reconciliation." });
          throw error;
        }
        this.#uncertainPotions.delete(String(action.uniqueId));
        if (receipt.success) this.#potionPendingUntil = this.clock() + 4_000;
        else { this.#potionPendingUntil = 0; this.#failedPotions.set(String(action.uniqueId), this.clock() + 10_000); }
        this.store.recordEpisode({ kind: "item-use", action: "use-potion", outcome: receipt.success ? "accepted" : "rejected",
          objectRef: `item:${action.uniqueId}`, summary: receipt.success ? "Potion consumed; timed recovery remains server-owned." : "Item use rejected." });
        outcome = { performed: true, confirmed: receipt.success };
      } else if (["follow", "retreat", "attack", "loot"].includes(action.kind)) {
        outcome = await this.#executeSpatial(action, state);
      }
      return { ...action, ...outcome };
    } finally {
      this.#busy = false;
    }
  }

  async #executeSpatial(action, state) {
    const target = action.target;
    if (!target) return { performed: false, blocked: true };
    const distance = Math.max(Math.abs(state.player.x - target.x), Math.abs(state.player.y - target.y));
    if (distance > (action.desiredDistance ?? 1)) {
      let collision = this.#maps.get(state.mapFileName);
      if (!collision) {
        collision = await this.mapLoader(state.mapFileName);
        this.#maps.set(state.mapFileName, collision);
      }
      const occupied = state.entities.filter((entity) => entity.objectId !== state.playerObjectId && entity.dead !== true)
        .map((entity) => ({ x: entity.x, y: entity.y }));
      const direction = nextWalkDirection({ player: state.player, target, desiredDistance: action.desiredDistance ?? 1, collision, occupied });
      if (!direction) return { performed: false, blocked: true };
      this.report({ type: "companionStep", direction, from: { x: state.player.x, y: state.player.y }, target, action: action.kind });
      const result = await this.client.walk(direction);
      return { performed: true, moved: result.moved };
    }
    if (action.kind === "attack") {
      // Revalidate after any awaited work. No cached target survives disappearance.
      const current = this.#prepareState(this.client.snapshot(), this.clock());
      const live = current.entities.find((entity) => entity.objectId === action.targetId && entity.kind === "monster" && entity.disposition !== "friendly" && entity.dead !== true && (entity.hp == null || entity.hp > 0));
      if (!this.#sameWorld(state, current) || !live || !Number.isInteger(current.player?.x) || !Number.isInteger(current.player?.y) || Math.max(Math.abs(current.player.x - live.x), Math.abs(current.player.y - live.y)) !== 1) return { performed: false, blocked: true };
      const fresh = decideCompanionAction({ state: current, policy: this.store.profile(), leaderName: this.leaderName, nowMs: this.clock(), homePosition: this.homePosition });
      const occupiedByOther = current.entities.some((entity) => entity.objectId !== live.objectId && entity.dead !== true && (entity.hp == null || entity.hp > 0) && entity.x === live.x && entity.y === live.y);
      if (fresh.kind !== "attack" || fresh.targetId !== live.objectId || occupiedByOther) return { performed: false, blocked: true };
      await this.client.attackDirection(facingDirection(current.player, live));
      return { performed: true, confirmed: false };
    }
    if (action.kind === "loot") {
      let result;
      try {
        result = await this.client.pickUpTile();
      } catch (error) {
        this.#uncertainLoot.set(action.targetId, { connectionEpoch: state.connectionEpoch, mapFileName: state.mapFileName });
        this.store.recordEpisode({ kind: "inventory-change", action: "pickup", outcome: "uncertain", objectRef: `drop:${action.targetId}`, summary: "Tile pickup result unknown; this observed drop is not blindly retried." });
        throw error;
      }
      if (result.changed !== true && result.confirmed === false) {
        this.#uncertainLoot.set(action.targetId, { connectionEpoch: state.connectionEpoch, mapFileName: state.mapFileName });
        this.store.recordEpisode({ kind: "inventory-change", action: "pickup", outcome: "uncertain", objectRef: `drop:${action.targetId}`, summary: "Tile pickup result unknown; this observed drop is not blindly retried." });
        return { performed: true, changed: false, confirmed: false, uncertain: true };
      }
      if (result.changed) this.store.recordEpisode({ kind: "inventory-change", action: "pickup", outcome: "observed", summary: "Owned inventory or gold changed after ordinary tile pickup." });
      return { performed: true, changed: result.changed };
    }
    return { performed: false, arrived: true };
  }
}
