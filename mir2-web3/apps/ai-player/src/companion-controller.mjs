import { decideCompanionAction, facingDirection } from "./companion-policy.mjs";
import { loadClientCollision, nextWalkDirection } from "./skills/navigation.mjs";
import { projectLifeResources } from "./life/resources.mjs";

const movementOccupants = state => state.entities.filter(entity => entity.objectId !== state.playerObjectId && entity.objectId !== state.selfZoneObjectId && entity.dead !== true &&
  (entity.kind === "npc" || entity.hp == null || entity.hp > 0)).map(entity => ({ x: entity.x, y: entity.y }));
const WALK_OFFSETS = Object.freeze({ Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] });
const MOVEMENT_REJECTION_TTL_MS = 3_000;
const MAX_MOVEMENT_REJECTIONS = 64;

/** Executes one bounded ordinary action. Policy/feedback never changes game authority. */
export class CompanionController {
  #busy = false;
  #closed = false;
  #idleWaiters = new Set();
  #maps = new Map();
  #lastDecision = null;
  #potionPendingUntil = 0;
  #failedPotions = new Map();
  #uncertainPotions = new Map();
  #uncertainLoot = new Map();
  #uncertainInvite = null;
  #activeWithdrawal = null;
  #movementWorld = null;
  #movementRejections = new Map();
  #supplyNavigation = null;

  constructor({ client, store, leaderName, homePosition, report = () => {}, clock = Date.now, mapLoader = loadClientCollision, supplyController = null, lifeController = null, giftController = null }) {
    if (typeof leaderName !== "string" || !leaderName.trim()) throw new Error("Configure the approved party leader name.");
    this.client = client;
    this.store = store;
    this.leaderName = leaderName;
    this.homePosition = homePosition;
    this.report = report;
    this.clock = clock;
    this.mapLoader = mapLoader;
    this.recallRequested = false;
    this.supplyController = supplyController;
    this.lifeController = lifeController;
    this.giftController = giftController;
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
    this.#synchronizeMovement(state, nowMs);
    const ownedInventory = state.inventoryItems, ownedBelt = state.beltItems;
    state.recallRequested = this.recallRequested;
    if (this.#activeWithdrawal) {
      const { world, target } = this.#activeWithdrawal;
      // Survival is a complete trip, not a fresh HP threshold at each step.
      // Never carry a destination into a different character/map/connection.
      if (!this.#sameWorld(world, state) || state.player?.dead === true || state.playerHp === 0 ||
        state.inSafeZone === true || state.player?.x === target.x && state.player?.y === target.y) {
        this.#activeWithdrawal = null;
      }
    }
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
    state.medicineUnresolved = this.#medicineUnresolved(state, nowMs);
    state.lifeResources = projectLifeResources({ ...state, inventoryItems: ownedInventory, beltItems: ownedBelt }, { observedAtMs: nowMs,
      medicineUnresolved: state.medicineUnresolved, usableItems: [...state.inventoryItems, ...state.beltItems] });
    return state;
  }

  resourceView() { return this.#prepareState(this.client.snapshot(), this.clock()).lifeResources; }

  #baseAction(state, nowMs) {
    const action = decideCompanionAction({ state, policy: this.store.profile(), leaderName: this.leaderName, nowMs,
      homePosition: this.homePosition, withdrawalTarget: this.#activeWithdrawal?.target });
    const gift = this.giftController?.view(), unresolved = gift && !["idle", "completed", "cancelled"].includes(gift.status);
    // A trade blocks new work and spending. Survival may proceed after the
    // single cancellation lane, even if the final trade result is still unknown.
    if ((state.trade?.uncertain === true || state.trade?.active === true || unresolved) && !this.#survivalAction(action)) return { kind: "wait", reason: state.trade?.active === true ? "trade-in-progress" : "trade-outcome-unknown", policyVersion: this.store.profile().policyVersion };
    return action;
  }

  #survivalAction(action) { return action.kind === "usePotion" || action.kind === "retreat" || action.kind === "attack" && action.reason === "defend-confirmed-engagement"; }

  #protectedAction(action) {
    return action.kind === "usePotion" || action.kind === "retreat" || action.kind === "attack" && action.reason === "defend-confirmed-engagement" ||
      action.kind === "wait" && !["near-party-leader", "no-authorized-party-activity", "party-combat-still-active", "leader-not-visible"].includes(action.reason);
  }

  #activeSupply() {
    if (!this.supplyController) return null;
    const view = this.supplyController.view();
    return view && !["idle", "completed", "cancelled", "failed"].includes(view.status) ? view : null;
  }

  #medicineUnresolved(state, nowMs) {
    return this.#uncertainPotions.size > 0 || nowMs < this.#potionPendingUntil || state.inventoryStale === true;
  }

  #movementWorldKey(state) {
    return state?.ready === true ? JSON.stringify([this.client.worldEpoch ?? null, this.client.activeCharacterIndex ?? null,
      state.connectionEpoch, state.mapFileName, state.mapIndex, state.playerObjectId, state.selfZoneObjectId]) : null;
  }

  #synchronizeMovement(state, nowMs) {
    const world = this.#movementWorldKey(state);
    if (world !== this.#movementWorld || world === null || !Number.isFinite(nowMs)) {
      this.#movementWorld = world; this.#movementRejections.clear(); this.#supplyNavigation = null;
    }
    for (const [key, value] of this.#movementRejections) {
      if (nowMs < value.observedAtMs || nowMs >= value.expiresAtMs) this.#movementRejections.delete(key);
    }
  }

  #navigationOccupants(state, nowMs) {
    this.#synchronizeMovement(state, nowMs);
    return [...movementOccupants(state), ...[...this.#movementRejections.values()].map(({ x, y }) => ({ x, y }))];
  }

  #recordSupplyNavigation(state, permitId, outcome) {
    const supply = this.#activeSupply(), world = this.#movementWorldKey(state);
    if (!world || supply?.permitId !== permitId) return;
    if (outcome === "moved") { this.#supplyNavigation = null; return; }
    if (!["no-path", "rejected"].includes(outcome)) return;
    const previous = this.#supplyNavigation;
    const failures = previous?.world === world && previous.permitId === permitId ? previous.failures + 1 : 1;
    this.#supplyNavigation = { world, permitId, failures: Math.min(failures, 3) };
    if (failures === 3) {
      this.supplyController.pauseNavigation?.("补给路线连续三次被拒绝或不可达；已暂停，尚未确认抵达授权点。");
      this.report({ type: "companionNavigationBlocked", reason: "supply-route-paused", action: "supply", from: { x: state.player.x, y: state.player.y } });
    }
  }

  async #walkObserved(direction, state, { action, target }) {
    const world = this.#movementWorldKey(state), offset = WALK_OFFSETS[direction];
    const destination = { x: state.player.x + offset[0], y: state.player.y + offset[1] };
    // A timeout has no authoritative result, so it never invents an obstacle.
    const result = await this.client.walk(direction);
    const nowMs = this.clock(), observed = this.#prepareState(this.client.snapshot(), nowMs);
    if (!this.#sameWorld(state, observed) || this.#movementWorldKey(observed) !== world) return { moved: false, feedback: null, observed };
    const changed = observed.player?.x !== state.player.x || observed.player?.y !== state.player.y;
    if (result?.moved === true && changed) {
      this.#movementRejections.delete(`${observed.player.x},${observed.player.y}`);
      return { moved: true, feedback: "moved", observed };
    }
    if (result?.moved === false && !changed && Number.isFinite(nowMs)) {
      const key = `${destination.x},${destination.y}`;
      this.#movementRejections.delete(key);
      this.#movementRejections.set(key, { ...destination, observedAtMs: nowMs, expiresAtMs: nowMs + MOVEMENT_REJECTION_TTL_MS });
      while (this.#movementRejections.size > MAX_MOVEMENT_REJECTIONS) this.#movementRejections.delete(this.#movementRejections.keys().next().value);
      this.report({ type: "companionMovementRejected", action, direction, from: { x: state.player.x, y: state.player.y }, destination, target: { ...target }, temporaryUntilMs: nowMs + MOVEMENT_REJECTION_TTL_MS });
      return { moved: false, feedback: "rejected", observed };
    }
    return { moved: false, feedback: null, observed };
  }

  #decide(state, nowMs) {
    const baseAction = this.#baseAction(state, nowMs);
    const gift = this.giftController?.view();
    if (this.giftController?.hasWork(state) && !this.#survivalAction(baseAction) && (state.tradePendingInvite?.name === this.leaderName || ["accepting", "receiving"].includes(gift?.status))) {
      return { kind: "gift", reason: "receive-owner-supply", policyVersion: baseAction.policyVersion };
    }
    const lifeAction = this.lifeController?.decide({ state, policy: this.store.profile(), baseAction, nowMs });
    // Returning after expiry, cancellation or an obstructed route must not be
    // starved by repeatedly selecting a nearby monster. Healing, withdrawal
    // and protected unknown/dead states retain their earlier safety priority.
    if (lifeAction?.kind === "retreat" && lifeAction.reason === "life-return-home" &&
        baseAction.kind === "attack" && baseAction.reason === "defend-confirmed-engagement") return lifeAction;
    if (lifeAction?.kind === "wait" && ["life-session-finished", "life-awaiting-resume", "life-world-or-policy-changed", "life-awaiting-server-activation", "life-navigation-await-owner", "life-home-safety-unknown"].includes(lifeAction.reason) &&
        baseAction.kind === "attack" && baseAction.reason === "defend-confirmed-engagement") return lifeAction;
    if (this.#protectedAction(baseAction)) {
      // The model still observes death/world invalidation, but cannot replace
      // the deterministic survival action selected above.
      this.modelDriver?.decide({ state, baseAction, nowMs });
      const bounded = lifeAction?.lifePermitId ? { ...baseAction, lifePermitId: lifeAction.lifePermitId } : null;
      return bounded && this.lifeController.movementConstraint(state, bounded) ? bounded : baseAction;
    }
    const supply = this.#activeSupply();
    if (supply) {
      const routePaused = supply.status !== "unknown" && this.#supplyNavigation?.world === this.#movementWorldKey(state) && this.#supplyNavigation.permitId === supply.permitId && this.#supplyNavigation.failures >= 3;
      return { kind: this.#medicineUnresolved(state, nowMs) || routePaused ? "wait" : "supply",
        reason: this.#medicineUnresolved(state, nowMs) ? "supply-waits-for-potion-reconciliation" : routePaused ? "supply-route-blocked-await-owner" : "authorized-supply", policyVersion: baseAction.policyVersion };
    }
    return lifeAction ?? this.modelDriver?.decide({ state, baseAction, nowMs }) ?? baseAction;
  }

  #sameWorld(before, after) {
    return !this.#closed && after.ready && before.connectionEpoch === after.connectionEpoch && before.mapFileName === after.mapFileName && before.mapIndex === after.mapIndex && before.playerObjectId === after.playerObjectId && before.selfZoneObjectId === after.selfZoneObjectId;
  }

  #inviteKey(state) {
    const invite = state.party?.pendingInvite;
    return `${state.connectionEpoch}:${invite?.name}:${invite?.observedAt}`;
  }

  async tick() {
    if (this.#closed) return { kind: "wait", reason: "controller-closed", performed: false };
    if (this.#busy) return { kind: "wait", reason: "previous-action-in-flight" };
    this.#busy = true;
    try {
      const nowMs = this.clock();
      const state = this.#prepareState(this.client.snapshot(), nowMs);
      this.giftController?.observe(state);
      const gift = this.giftController?.view(), survival = this.#baseAction(state, nowMs);
      if (this.giftController?.hasWork(state) && this.#survivalAction(survival) && gift?.cancelAttempted !== true &&
          (state.trade?.active || state.tradePendingInvite?.name === this.leaderName || ["accepting", "receiving"].includes(gift?.status))) {
        const result = await this.giftController.cancel("危险或召回，先取消原交易；下一次行动处理生存。");
        return { kind: "gift", reason: "cancel-trade-before-survival", policyVersion: survival.policyVersion, ...result };
      }
      const action = this.#decide(state, nowMs);
      if (action.kind === "wait" && ["dead", "world-not-ready", "map-not-authorized"].includes(action.reason) && this.#activeSupply()) {
        this.supplyController.cancel(`companion-${action.reason}`);
      }
      if (action.kind === "retreat" && action.reason === "unsafe-withdraw" && state.inSafeZone !== true &&
        (state.player.x !== action.target.x || state.player.y !== action.target.y)) {
        this.#activeWithdrawal = { world: { ready: state.ready, connectionEpoch: state.connectionEpoch, mapFileName: state.mapFileName,
          mapIndex: state.mapIndex, playerObjectId: state.playerObjectId, selfZoneObjectId: state.selfZoneObjectId }, target: { ...action.target } };
      }
      const key = `${action.policyVersion}:${action.kind}:${action.reason}:${action.targetId ?? ""}`;
      if (key !== this.#lastDecision) {
        if (!["supply", "life", "gift"].includes(action.kind)) this.store.recordEpisode({ kind: "decision", action: action.kind.toLowerCase(), outcome: "selected", summary: action.reason,
          context: { mode: state.party?.members?.length ? "party" : "solo", sourceEventRef: `observation:${state.connectionEpoch}:${state.revision}` } });
        this.report({ type: "companionDecision", ...action });
        this.#lastDecision = key;
      }
      let outcome = { performed: false };
      if (action.kind === "gift") {
        const result = await this.giftController.tick(state); return { ...action, ...result, reason: action.reason };
      } else if (action.kind === "supply") {
        const result = await this.supplyController.tick(state);
        return { ...action, ...result, reason: action.reason };
      } else if (action.kind === "life") {
        const life = this.lifeController;
        const result = await life.tick(state, { stillSafe: () => {
          const now = this.clock(), current = this.#prepareState(this.client.snapshot(), now);
          return !this.#closed && this.lifeController === life && life.view().lifePermitId === action.lifePermitId && this.#sameWorld(state, current) &&
            this.store.profile().policyVersion === action.policyVersion && !this.#protectedAction(this.#baseAction(current, now)) && !this.#medicineUnresolved(current, now);
        } });
        return { ...action, ...result, reason: action.reason };
      } else if (action.kind === "acceptInvite") {
        const current = this.#prepareState(this.client.snapshot(), this.clock());
        const invitation = this.#inviteKey(current);
        const fresh = this.#decide(current, this.clock());
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
      for (const resolveIdle of this.#idleWaiters) resolveIdle();
      this.#idleWaiters.clear();
    }
  }

  /** Supply owns this tick, but its awaited collision read cannot bypass a
   * newly observed medicine, recall, threat, revoked permit or world boundary. */
  async stepForSupply({ snapshot, target, desiredDistance, bounds }) {
    const permit = this.#activeSupply(), version = this.store.profile().policyVersion;
    const inside = p => p && [p.x, p.y].every(Number.isInteger) && p.x >= bounds.minX && p.x <= bounds.maxX && p.y >= bounds.minY && p.y <= bounds.maxY;
    const currentState = () => {
      const now = this.clock(), state = this.#prepareState(this.client.snapshot(), now), currentPermit = this.#activeSupply();
      if (!permit || !currentPermit || currentPermit.permitId !== permit.permitId || !["authorized", "running", "returning"].includes(currentPermit.status) ||
          this.#closed || !this.#sameWorld(snapshot, state) || !inside(state.player) || !inside(target) || this.store.profile().policyVersion !== version ||
          this.#protectedAction(this.#baseAction(state, now)) || this.#medicineUnresolved(state, now) ||
          (!Number.isFinite(now) || now >= currentPermit.expiresAtMs && currentPermit.acquiredQuantity !== 1)) return null;
      return state;
    };
    if (!bounds || ![bounds.minX, bounds.minY, bounds.maxX, bounds.maxY].every(Number.isInteger) || !Number.isInteger(desiredDistance) || desiredDistance < 0 || desiredDistance > 1 || !currentState()) return { performed: false, blocked: true };
    let collision = this.#maps.get(snapshot.mapFileName);
    if (!collision) { collision = await this.mapLoader(snapshot.mapFileName); this.#maps.set(snapshot.mapFileName, collision); }
    const state = currentState(); if (!state) return { performed: false, blocked: true };
    const occupied = this.#navigationOccupants(state, this.clock());
    const direction = nextWalkDirection({ player: state.player, target, desiredDistance, collision, occupied, movementBounds: bounds });
    if (!direction) { this.#recordSupplyNavigation(state, permit.permitId, "no-path"); return { performed: false, blocked: true }; }
    this.report({ type: "companionStep", direction, from: { x: state.player.x, y: state.player.y }, target, action: "supply" });
    const result = await this.#walkObserved(direction, state, { action: "supply", target });
    this.#recordSupplyNavigation(result.observed, permit.permitId, result.feedback);
    return { performed: true, moved: result.moved };
  }

  async close() {
    this.#closed = true;
    if (this.#busy) await new Promise(resolveIdle => this.#idleWaiters.add(resolveIdle));
    this.#movementRejections.clear(); this.#supplyNavigation = null;
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
      const current = this.#prepareState(this.client.snapshot(), this.clock());
      if (!this.#sameWorld(state, current)) return { performed: false, blocked: true };
      const fresh = this.#decide(current, this.clock());
      if (fresh.kind !== action.kind || fresh.policyVersion !== action.policyVersion || fresh.targetId !== action.targetId || fresh.lifePermitId !== action.lifePermitId || !fresh.target) return { performed: false, blocked: true };
      if (Math.max(Math.abs(current.player.x - fresh.target.x), Math.abs(current.player.y - fresh.target.y)) <= (fresh.desiredDistance ?? 1)) return { performed: false, arrived: true };
      const occupied = this.#navigationOccupants(current, this.clock());
      const constraint = fresh.lifePermitId ? this.lifeController?.movementConstraint(current, fresh) : this.modelDriver?.movementConstraint(current, fresh);
      if (fresh.lifePermitId && !constraint) return { performed: false, blocked: true };
      const direction = nextWalkDirection({ player: current.player, target: fresh.target, desiredDistance: fresh.desiredDistance ?? 1, collision, occupied, movementBounds: constraint?.bounds ?? null,
        maxPathSteps: fresh.lifePermitId && fresh.kind === "attack" && fresh.reason === "life-hunt-observed-monster" ? 12 : null });
      if (!direction) {
        if (fresh.lifePermitId) this.report({ type: "companionNavigationBlocked", from: { x: current.player.x, y: current.player.y }, target: fresh.target,
          reason: "no-observed-path", occupied: occupied.slice(0, 128), occupiedTruncated: occupied.length > 128 });
        if (fresh.lifePermitId) this.lifeController?.recordNavigation?.({ state: current, action: fresh, outcome: "no-path", nowMs: this.clock() });
        return { performed: false, blocked: true };
      }
      if (constraint && !constraint.permits({ x: current.player.x + WALK_OFFSETS[direction][0], y: current.player.y + WALK_OFFSETS[direction][1] })) return { performed: false, blocked: true };
      this.report({ type: "companionStep", direction, from: { x: current.player.x, y: current.player.y }, target: fresh.target, action: fresh.kind });
      const result = await this.#walkObserved(direction, current, { action: fresh.kind, target: fresh.target });
      if (fresh.lifePermitId && result.feedback) this.lifeController?.recordNavigation?.({ state: result.observed, action: fresh, outcome: result.feedback, nowMs: this.clock() });
      return { performed: true, moved: result.moved };
    }
    if (action.kind === "attack") {
      // Revalidate after any awaited work. No cached target survives disappearance.
      const current = this.#prepareState(this.client.snapshot(), this.clock());
      const live = current.entities.find((entity) => entity.objectId === action.targetId && entity.kind === "monster" && entity.disposition !== "friendly" && entity.dead !== true && (entity.hp == null || entity.hp > 0));
      if (!this.#sameWorld(state, current) || !live || !Number.isInteger(current.player?.x) || !Number.isInteger(current.player?.y) || Math.max(Math.abs(current.player.x - live.x), Math.abs(current.player.y - live.y)) !== 1) return { performed: false, blocked: true };
      const fresh = this.#decide(current, this.clock());
      const occupiedByOther = current.entities.some((entity) => entity.objectId !== live.objectId && entity.dead !== true && (entity.hp == null || entity.hp > 0) && entity.x === live.x && entity.y === live.y);
      if (fresh.kind !== "attack" || fresh.targetId !== live.objectId || fresh.lifePermitId !== action.lifePermitId || occupiedByOther) return { performed: false, blocked: true };
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
