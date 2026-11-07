import { createHash } from "node:crypto";
import { LifeStore } from "./life-store.mjs";
import { projectLifeResources } from "./resources.mjs";
import { exact, same } from "../supply/purchase-wire.mjs";
import { validateSupplyGrant } from "../supply/supply-controller.mjs";

const BOUNDS = Object.freeze({ minX: 270, minY: 595, maxX: 310, maxY: 645 });
const HOME = Object.freeze({ x: 288, y: 616 });
const WAYPOINTS = Object.freeze([{ x: 286, y: 628 }, { x: 294, y: 633 }, { x: 301, y: 626 }]);
const MONSTERS = new Set(["Deer", "Hen", "Chicken", "Scarecrow"]);
const HUNT_ACQUIRE_RADIUS = 6;
const HUNT_RETAIN_RADIUS = 12;
const HUNT_DEFER_MS = 30_000;
const MAX_DEFERRED_HUNT_TARGETS = 32;
const ref = value => typeof value === "string" && /^[A-Za-z0-9._:-]{1,100}$/.test(value);
const uint = value => Number.isSafeInteger(value) && value >= 0;
const objectId = value => Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
const point = value => value && uint(value.x) && uint(value.y);
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const inside = (p, b) => point(p) && p.x >= b.minX && p.x <= b.maxX && p.y >= b.minY && p.y <= b.maxY;
const signature = state => JSON.stringify([state.connectionEpoch, state.mapFileName, state.mapIndex, state.playerObjectId, state.selfZoneObjectId]);
const liveMonster = value => value?.kind === "monster" && Number.isInteger(value.objectId) && point(value) && value.dead !== true && (value.hp == null || value.hp > 0) && value.disposition !== "friendly";
const terminal = status => ["completed", "cancelled", "failed"].includes(status);
const emptyNavigation = () => ({ waypointIndex: null, noPathCount: 0, failedWaypoints: [], returning: false, homeNoPathCount: 0 });
function navigationState(value) {
  if (value === undefined) return emptyNavigation();
  exact(value, ["waypointIndex", "noPathCount", "failedWaypoints", "returning", "homeNoPathCount"]);
  if (value.waypointIndex !== null && ![0, 1, 2].includes(value.waypointIndex) || !uint(value.noPathCount) || value.noPathCount > 2 ||
      !Array.isArray(value.failedWaypoints) || value.failedWaypoints.length > 3 || new Set(value.failedWaypoints).size !== value.failedWaypoints.length ||
      value.failedWaypoints.some(index => ![0, 1, 2].includes(index)) || typeof value.returning !== "boolean" || !uint(value.homeNoPathCount) || value.homeNoPathCount > 3) throw new Error("Life navigation journal is invalid.");
  return structuredClone(value);
}

/** Trusted server grants only. This runner never invents or extends authority. */
export function validateLifeGrant(value) {
  exact(value, ["lifePermitId", "worldEpoch", "actorCharacterIndex", "mapFileName", "startedAtMs", "expiresAtMs", "durationSeconds", "budgetGold", "maxPurchasedQuantity", "bounds", "home", "routeId", "waypoints"]);
  exact(value.bounds, ["minX", "minY", "maxX", "maxY"]); exact(value.home, ["x", "y"]);
  if (!ref(value.lifePermitId) || !ref(value.worldEpoch) || !uint(value.actorCharacterIndex) || value.mapFileName !== "0" || !uint(value.startedAtMs) || !uint(value.expiresAtMs) ||
      ![1200, 1800].includes(value.durationSeconds) || value.expiresAtMs - value.startedAtMs !== value.durationSeconds * 1000 || !Number.isInteger(value.budgetGold) || value.budgetGold < 1 || value.budgetGold > 50 || value.maxPurchasedQuantity !== 1 ||
      !same(value.bounds, BOUNDS) || !same(value.home, HOME) || value.routeId !== "bichon-south-v1" || !same(value.waypoints, WAYPOINTS)) throw new Error("Invalid bounded Bichon life grant.");
  return structuredClone(value);
}

/** A deterministic, bounded life loop. Models may select from the route, hunt
 * and rest vocabulary; they never receive an economic actuator or new limits. */
export class LifeController {
  #client; #journal; #supply; #store; #clock; #report; #onObserved; #requestPlan; #entry = null; #closed = false; #busy = false;
  #world = null; #hits = new Map(); #huntTarget = null; #listener; #pendingPlan = null; #planToken = 0; #resources = null; #closePromise; #closeResolve;
  #deferredHuntTargets = new Map();
  constructor({ client, dbPath, supplyController = null, store, clock = Date.now, report = () => {}, onObserved, requestPlan }) {
    this.#client = client; this.#supply = supplyController; this.#store = store; this.#clock = clock; this.#report = report; this.#requestPlan = requestPlan;
    this.#onObserved = onObserved ?? (event => this.#recordMemory(event));
    this.#journal = new LifeStore(dbPath);
    try {
      const saved = this.#journal.current();
      if (saved) {
        saved.grant = validateLifeGrant(saved.grant);
        if (!saved.task || !uint(saved.task.killsObserved) || !uint(saved.task.modelDecisionsUsed) || saved.task.modelDecisionsUsed > 4 || typeof saved.task.childSupplyClaimed !== "boolean" ||
            !uint(saved.task.spentGold) || saved.task.spentGold > saved.grant.budgetGold || ![0, 1].includes(saved.task.purchasedQuantity) || !Array.isArray(saved.task.pendingFacts) ||
            !Number.isInteger(saved.task.waypointIndex) || saved.task.waypointIndex < 0 || saved.task.waypointIndex >= saved.grant.waypoints.length || !Number.isSafeInteger(saved.task.policyVersion) || saved.task.policyVersion < 1 ||
            saved.task.childSupplyPermitId !== null && !ref(saved.task.childSupplyPermitId) || saved.task.childSupplyPermitId !== null && !saved.task.childSupplyClaimed ||
            saved.task.spentGold > 0 && saved.task.purchasedQuantity !== 1 || saved.task.purchasedQuantity === 1 && !saved.task.childSupplyClaimed) throw new Error("Life execution journal is invalid.");
        saved.task.navigation = navigationState(saved.task.navigation);
        this.#entry = saved;
        if (!terminal(saved.task.status)) { saved.task.status = "paused"; saved.task.step = "awaiting-resume"; saved.task.decisionMode = "rules"; saved.task.reason = "重启后保留期限、经历和采购名额；请明确恢复本次生活。"; this.#save(); }
      }
    } catch (error) { this.#journal.close(); throw error; }
    this.#listener = ({ event }) => {
      try { this.observe(event); }
      catch { if (this.#entry) Object.assign(this.#entry.task, { status: "paused", step: "awaiting-resume", reason: "实际经历记录未完成，生活已暂停。" }); this.#report({ type: "lifeObservationWriteUnavailable" }); }
    };
    client.on?.("frame", this.#listener);
  }

  view() {
    if (!this.#entry) return { status: "idle", step: "idle", phase: "idle", lifePermitId: null, spentGold: 0, chargedGold: 0, reservedGold: 0, remainingBudgetGold: 0,
      childSupplyClaimed: false, childSupplyPermitId: null, purchasedQuantity: 0, killsObserved: 0, modelDecisionsUsed: 0, resources: this.#resources };
    const { grant, task } = this.#entry;
    const child = this.#supply?.view(), reservedGold = child?.permitId === task.childSupplyPermitId && child?.status === "unknown" && child.acquiredQuantity !== 1 ? grant.budgetGold : 0;
    return { status: task.status, step: task.step, phase: task.activationPending ? "awaiting-server-activation" : task.step, activationPending: task.activationPending === true, lifePermitId: grant.lifePermitId, startedAtMs: grant.startedAtMs, expiresAtMs: grant.expiresAtMs,
      durationSeconds: grant.durationSeconds, budgetGold: grant.budgetGold, maxPurchasedQuantity: grant.maxPurchasedQuantity, spentGold: task.spentGold, chargedGold: task.spentGold,
      reservedGold, remainingBudgetGold: Math.max(0, grant.budgetGold - task.spentGold - reservedGold), purchasedQuantity: task.purchasedQuantity,
      childSupplyClaimed: task.childSupplyClaimed, childSupplyPermitId: task.childSupplyPermitId, killsObserved: task.killsObserved,
      waypointIndex: task.waypointIndex, modelDecisionsUsed: task.modelDecisionsUsed, decisionMode: task.decisionMode, reason: task.reason, reply: task.modelReply ?? null,
      navigation: navigationState(task.navigation), resources: this.#resources ? structuredClone(this.#resources) : null };
  }

  #currentWorld(state, grant = this.#entry?.grant) {
    return grant && state?.ready === true && state.mapFileName === grant.mapFileName && inside(state.player, grant.bounds) &&
      this.#client.worldEpoch === grant.worldEpoch && this.#client.activeCharacterIndex === grant.actorCharacterIndex;
  }
  #open() { if (this.#closed) throw new Error("Life controller is closed."); }
  #save() { this.#journal.save(this.#entry.grant.lifePermitId, this.#entry.task); }
  #set(update) { const task = this.#entry.task; if (Object.entries(update).some(([key, value]) => !same(task[key], value))) { Object.assign(task, update); this.#save(); } }
  #version() { const value = this.#store?.profile?.().policyVersion; if (!Number.isSafeInteger(value) || value < 1) throw new Error("Current companion policy is required."); return value; }
  #clearPursuit() { this.#huntTarget = null; this.#deferredHuntTargets.clear(); }
  #pruneDeferredTargets(state, nowMs) {
    for (const [id, deferred] of this.#deferredHuntTargets) {
      const target = (state.entities ?? []).find(e => e.objectId === id && liveMonster(e) && MONSTERS.has(e.name) && inside(e, this.#entry.grant.bounds));
      if (!target || target.name !== deferred.name || nowMs < deferred.observedAtMs || nowMs >= deferred.expiresAtMs) this.#deferredHuntTargets.delete(id);
    }
  }
  #validAuthorization(grant) {
    const now = this.#clock(), state = this.#client.snapshot();
    return uint(now) && grant.startedAtMs <= now && now < grant.expiresAtMs && this.#currentWorld(state, grant) && state.player?.dead !== true && state.playerHp !== 0;
  }

  authorize(raw, { awaitActivation = false } = {}) {
    this.#open(); const grant = validateLifeGrant(raw), old = this.#journal.find(grant.lifePermitId);
    if (old) { if (!same(old.grant, grant) || this.#entry?.grant.lifePermitId !== grant.lifePermitId) throw new Error("Life permit ID conflicts."); return this.view(); }
    const supply = this.#supply?.view();
    if (this.#busy || supply && !["idle", "completed", "cancelled", "failed"].includes(supply.status)) throw new Error("Finish or reconcile the current supply before authorizing another life.");
    if (!this.#validAuthorization(grant)) throw new Error("Life grant is not current for this actor and world.");
    this.#entry = this.#journal.create(grant, { status: "running", step: "patrolling", reason: "已明确授权在固定比奇路线独立生活。", policyVersion: this.#version(),
      waypointIndex: 0, childSupplyClaimed: false, childSupplyPermitId: null, childRequestId: null, spentGold: 0, purchasedQuantity: 0,
      killsObserved: 0, modelDecisionsUsed: 0, decisionMode: "rules", modelMode: "hunt", lastPlannedKey: null, modelReply: null, childCompletionPlanned: false,
      activationPending: awaitActivation, activationReason: "authorized", navigation: emptyNavigation(), cancelled: false, expired: false, needsReturn: false, departed: false, returnCycle: 0, homeRecordedCycle: -1, completedRecorded: false, pendingFacts: [] });
    this.#world = signature(this.#client.snapshot()); this.#hits.clear(); this.#clearPursuit(); this.#planToken++;
    this.#askPlan("authorized"); return this.view();
  }

  resume(raw, { awaitActivation = false } = {}) {
    this.#open(); const grant = validateLifeGrant(raw);
    if (!this.#entry || this.#entry.grant.lifePermitId !== grant.lifePermitId || !same(this.#entry.grant, grant) || terminal(this.#entry.task.status) || !this.#validAuthorization(grant)) throw new Error("Only the same unexpired life permit can be resumed.");
    if (this.#busy) throw new Error("Wait for the current life action before resuming.");
    this.#world = signature(this.#client.snapshot()); this.#hits.clear(); this.#clearPursuit(); this.#planToken++;
    const navigation = navigationState(this.#entry.task.navigation), current = this.#client.snapshot();
    const resumedNavigation = navigation.returning && distance(current.player, grant.home) <= 1 && current.inSafeZone === true ? emptyNavigation() : { ...navigation, homeNoPathCount: 0 };
    this.#set({ status: "running", step: "resting", reason: "已恢复原生活期限；保留已有采购名额和实际经历。", policyVersion: this.#version(), cancelled: false, activationPending: awaitActivation, activationReason: "resumed", navigation: resumedNavigation });
    this.#askPlan("resumed"); return this.view();
  }

  confirmActivation(lifePermitId) {
    this.#open();
    const entry = this.#entry;
    if (!entry || entry.grant.lifePermitId !== lifePermitId || entry.task.status !== "running" || entry.task.cancelled || entry.task.expired || !this.#validAuthorization(entry.grant) || signature(this.#client.snapshot()) !== this.#world || this.#version() !== entry.task.policyVersion) return this.view();
    if (!entry.task.activationPending) return this.view();
    this.#set({ activationPending: false });
    this.#askPlan(entry.task.activationReason ?? "authorized");
    return this.view();
  }

  cancel(reason = "主人已撤销本次生活。") {
    this.#open(); if (!this.#entry || terminal(this.#entry.task.status)) return this.view();
    this.#planToken++; this.#hits.clear(); this.#clearPursuit();
    const supply = this.#supply?.view();
    if (supply?.permitId === this.#entry.task.childSupplyPermitId && !["idle", "completed", "cancelled", "failed"].includes(supply.status)) this.#supply.cancel("life-owner-revoked");
    this.#set({ cancelled: true, activationPending: false, status: "returning", step: "returning-home", reason: String(reason).slice(0, 160) }); return this.view();
  }

  #resourcesFor(state, nowMs) { return state.lifeResources ?? projectLifeResources(state, { observedAtMs: nowMs, medicineUnresolved: state.medicineUnresolved === true }); }
  #result(kind, reason, policy, fields = {}) { return { kind, reason, policyVersion: policy.policyVersion, lifePermitId: this.#entry.grant.lifePermitId, ...fields }; }
  #home(state, policy, reason) {
    const { grant, task } = this.#entry;
    if (distance(state.player, grant.home) > 1) {
      if (task.step !== "returning-home") task.returnCycle++;
      this.#set({ status: "returning", step: "returning-home", reason });
      return this.#result("retreat", "life-return-home", policy, { target: { ...grant.home }, desiredDistance: 1 });
    }
    if (state.inSafeZone !== true) return this.#result("wait", "life-home-safety-unknown", policy);
    if (task.departed && task.homeRecordedCycle !== task.returnCycle) {
      task.homeRecordedCycle = task.returnCycle; task.departed = false;
      this.#queueFact("life-returned", `returned:${task.returnCycle}`, state, { actualPosition: { x: state.player.x, y: state.player.y }, home: { ...grant.home }, arrivalDistance: distance(state.player, grant.home) });
    }
    if (task.pendingFacts.length) return this.#result("wait", "life-memory-write-unavailable", policy);
    return null;
  }

  #childState() {
    const { task } = this.#entry, child = this.#supply?.view();
    if (!task.childSupplyPermitId || child?.permitId !== task.childSupplyPermitId) return null;
    if (child.receiptKnown === true && child.acquiredQuantity === 1 && Number.isInteger(child.chargedGold) && child.chargedGold > 0 && child.chargedGold <= this.#entry.grant.budgetGold) this.#set({ spentGold: child.chargedGold, purchasedQuantity: 1 });
    return child;
  }

  decide({ state, policy, baseAction, nowMs = this.#clock() }) {
    if (this.#closed || !this.#entry) return null;
    this.#resources = this.#resourcesFor(state, nowMs);
    const { grant, task } = this.#entry, wait = reason => this.#result("wait", reason, policy);
    // Pursuit is volatile observed intent. It cannot outlive authorization,
    // the current actor/world, a pause or a safe return decision.
    if (task.cancelled || task.expired || terminal(task.status) || task.status === "paused" || task.activationPending === true ||
        !uint(nowMs) || nowMs >= grant.expiresAtMs || !this.#currentWorld(state) || signature(state) !== this.#world ||
        policy.policyVersion !== task.policyVersion || state.player?.dead === true || state.playerHp === 0) this.#clearPursuit();
    this.#pruneDeferredTargets(state, nowMs);
    this.#flushFacts();
    if (task.pendingFacts.length) return wait("life-memory-write-unavailable");
    if (terminal(task.status)) return wait("life-session-finished");
    if (!this.#currentWorld(state) || !this.#world || signature(state) !== this.#world || policy.policyVersion !== task.policyVersion) {
      this.#planToken++; this.#set({ status: "paused", step: "awaiting-resume", reason: "角色、世界或培养版本已改变；生活已暂停。" }); return wait("life-world-or-policy-changed");
    }
    if (state.player?.dead === true || state.playerHp === 0) { this.#set({ status: "paused", step: "awaiting-resume", reason: "角色死亡；生活已暂停。" }); return wait("life-dead"); }
    if (task.status === "paused") return wait("life-awaiting-resume");
    if (task.activationPending === true && !task.cancelled) return wait("life-awaiting-server-activation");
    if (!uint(nowMs)) { this.#set({ status: "paused", step: "awaiting-resume", reason: "无法确认当前授权时间。" }); return wait("life-clock-unknown"); }
    if (distance(state.player, grant.home) > 1 && !task.departed) this.#set({ departed: true });
    if (nowMs >= grant.expiresAtMs && !task.expired) {
      this.#planToken++; task.expired = true;
      const child = this.#childState();
      if (child && !["idle", "completed", "cancelled", "failed"].includes(child.status)) this.#supply.cancel("life-deadline-reached");
      this.#set({ status: "returning", step: "returning-home", reason: "本次生活已到期，停止狩猎与新花费，安全返程。" });
    }
    if (state.recallRequested === true && !task.cancelled) this.cancel("主人已召回，本次独立生活停止并返程。");
    if (task.cancelled || task.expired) {
      const returning = this.#home(state, policy, task.reason); if (returning) return returning;
      const status = task.cancelled ? "cancelled" : "completed";
      this.#set({ status, step: status, reason: task.cancelled ? "本次生活已撤销，已观察到回家。" : "本次生活到期并已观察到回家。" });
      if (!task.completedRecorded) { task.completedRecorded = true; this.#queueFact("life-completed", "completed", state, { completion: status, actualPosition: { x: state.player.x, y: state.player.y } }); }
      return wait("life-session-finished");
    }
    if (baseAction.kind === "retreat" && ["unsafe-withdraw", "continue-unsafe-withdraw"].includes(baseAction.reason)) {
      if (distance(state.player, grant.home) <= 1 && state.inSafeZone === true) {
        const recording = this.#home(state, policy, "已观察到安全返程。"); if (recording) return recording;
        this.#set({ needsReturn: false, status: "running", step: "resting", reason: "已在安全点休息，恢复健康后再行动。" }); return wait("life-rest-until-healthy");
      }
      if (!task.needsReturn) task.returnCycle++;
      this.#set({ needsReturn: true, status: "returning", step: "returning-home", reason: "生存规则正在撤回安全点；恢复后才继续生活。" });
      return wait("life-survival-return-priority");
    }
    if (task.navigation?.returning === true) {
      const returning = this.#home(state, policy, "三个固定路点均无可行路径；停止巡猎，安全返家。"); if (returning) return returning;
      this.#planToken++;
      this.#set({ status: "paused", step: "awaiting-resume", needsReturn: false, reason: "固定路线阻塞，已安全回家；等待主人确认后再出发。" });
      return wait("life-navigation-await-owner");
    }
    if (task.needsReturn) {
      const returning = this.#home(state, policy, "完成已经开始的安全返程。"); if (returning) return returning;
      this.#set({ needsReturn: false, status: "running", step: "resting", reason: "已观察到安全返程，先检查恢复与补给。" });
    }
    if (baseAction.kind === "usePotion") { this.#set({ step: "recovering", reason: "生存规则正在处理自有药品；等待真实结果。" }); return wait("life-medicine-priority"); }
    const child = this.#childState();
    if (child && !["completed", "cancelled", "failed"].includes(child.status)) return wait("life-child-supply-active");
    if (child?.status === "completed" && !task.childCompletionPlanned) { task.childCompletionPlanned = true; this.#save(); this.#askPlan("supply-completed"); }
    if (state.medicineUnresolved === true || state.trade?.uncertain === true || state.trade?.active === true || !this.#resources.known) {
      this.#set({ step: "awaiting-observation", reason: "等待药品、交易或完整资源观察，暂不开始新行动。" }); return wait("life-resource-or-economic-result-unknown");
    }
    const resource = this.#resources, full = resource.freeBagSlots === 0 || resource.currentWeight >= resource.maxWeight;
    const healthy = resource.hp / resource.maxHp >= 0.8;
    const needsSupply = resource.usableHpPotionCount <= 1;
    if (task.modelMode === "rest" && uint(task.modelRestUntilMs) && nowMs >= task.modelRestUntilMs) this.#set({ modelMode: "hunt", modelRestUntilMs: null });
    if (healthy && !needsSupply && !full && task.step === "resting") this.#askPlan("rest-completed");
    if (!healthy || full || needsSupply || task.modelMode === "rest") {
      const returning = this.#home(state, policy, "资源或身体状态需要回家检查。"); if (returning) return returning;
      if (!healthy) { this.#set({ status: "running", step: "resting", reason: "已在安全点休息，恢复健康后再行动。" }); return wait("life-rest-until-healthy"); }
      if (full) { this.#set({ status: "running", step: "awaiting-owner", reason: "背包或负重已满，等主人整理；不擅自卖物。" }); return wait("life-bag-full-await-owner"); }
      if (needsSupply) {
        if (!task.childSupplyClaimed && resource.gold >= 40 && grant.budgetGold >= 40 && this.#supply) {
          this.#set({ status: "running", step: "awaiting-supply", reason: "在安全点准备申请本次唯一的一瓶药补给。" }); return this.#result("life", "life-claim-one-supply", policy);
        }
        this.#set({ status: "running", step: "awaiting-owner", reason: task.childSupplyClaimed ? "本次采购名额已用，等主人补给或结束生活。" : "金币或采购预算不足，等主人补给。" });
        return wait("life-await-owner-supply");
      }
      if (task.modelMode === "rest") { this.#set({ status: "running", step: "resting", reason: "按当前有限计划在安全点休息。" }); return wait("life-model-requested-rest"); }
    }
    // The explicit life grant permits the fixed solo route without leaving the
    // shared group. Existing taught assistance/after-combat rules still win.
    if (baseAction.kind === "attack" && baseAction.reason === "assist-confirmed-party-engagement" && inside(baseAction.target, grant.bounds)) return { ...baseAction, lifePermitId: grant.lifePermitId };
    // Acquiring and retaining use different radii: an actual collision detour
    // may initially increase straight-line distance. Retention still requires
    // a currently visible, living, allowlisted monster inside the same grant.
    const pendingTarget = (state.entities ?? []).find(e => e.objectId === this.#huntTarget && !this.#deferredHuntTargets.has(e.objectId) && liveMonster(e) && MONSTERS.has(e.name) &&
      inside(e, grant.bounds) && distance(state.player, e) <= HUNT_RETAIN_RADIUS);
    if (!pendingTarget) this.#huntTarget = null;
    const combatActive = Boolean(pendingTarget) || (state.engagements ?? []).some(e => Number.isFinite(e.observedAt) && nowMs >= e.observedAt && nowMs - e.observedAt <= 6000 && (state.entities ?? []).some(entity => liveMonster(entity) && [e.attackerId, e.targetId].includes(entity.objectId)));
    if (baseAction.kind === "loot" && !combatActive && inside(baseAction.target, grant.bounds)) return { ...baseAction, lifePermitId: grant.lifePermitId };
    if (!combatActive && policy.solo?.considerLootWhenSafe !== false) {
      const allies = new Set([state.playerObjectId, state.selfZoneObjectId].filter(objectId)), members = new Set(state.party?.members ?? []);
      for (const entity of state.entities ?? []) if (entity.kind === "player" && members.has(entity.name) && objectId(entity.objectId)) allies.add(entity.objectId);
      const drop = (state.groundDrops ?? []).filter(value => objectId(value?.objectId) && inside(value, grant.bounds) && distance(state.player, value) <= 3 &&
        (value.ownerObjectId == null || allies.has(value.ownerObjectId) || Number.isFinite(value.ownershipRemainingTicks) && value.ownershipRemainingTicks <= 0))
        .sort((a, b) => distance(state.player, a) - distance(state.player, b) || a.objectId - b.objectId)[0];
      if (drop) return this.#result("loot", "life-after-combat-loot", policy, { targetId: drop.objectId, target: { x: drop.x, y: drop.y }, desiredDistance: 0 });
    }
    const targets = (state.entities ?? []).filter(e => !this.#deferredHuntTargets.has(e.objectId) && liveMonster(e) && MONSTERS.has(e.name) && inside(e, grant.bounds) &&
      distance(state.player, e) <= (e.objectId === this.#huntTarget ? HUNT_RETAIN_RADIUS : HUNT_ACQUIRE_RADIUS))
      .sort((a, b) => Number(b.objectId === this.#huntTarget) - Number(a.objectId === this.#huntTarget) || distance(state.player, a) - distance(state.player, b) || a.objectId - b.objectId);
    if (task.modelMode !== "route" && targets[0]) {
      const target = targets[0]; this.#huntTarget = target.objectId; this.#set({ status: "running", step: "hunting", reason: "在授权范围内处理当前真实可见的小型猎物。" });
      return this.#result("attack", "life-hunt-observed-monster", policy, { targetId: target.objectId, target: { x: target.x, y: target.y }, desiredDistance: 1 });
    }
    let waypoint = grant.waypoints[task.waypointIndex];
    if (distance(state.player, waypoint) <= 1) {
      task.waypointIndex = (task.waypointIndex + 1) % grant.waypoints.length; waypoint = grant.waypoints[task.waypointIndex];
      if (task.modelMode === "route") task.modelMode = "hunt";
      this.#save();
    }
    this.#set({ status: "running", step: "patrolling", reason: "沿已授权的比奇南侧路线观察和生活。" });
    return this.#result("follow", "life-route-waypoint", policy, { targetId: `life-waypoint:${task.waypointIndex}`, target: { ...waypoint }, desiredDistance: 1 });
  }

  async tick(snapshot = this.#client.snapshot(), { stillSafe = () => true } = {}) {
    this.#open(); if (this.#busy || !this.#entry) return { performed: false, ...this.view() };
    const { grant, task } = this.#entry;
    if (task.step !== "awaiting-supply" || task.childSupplyClaimed || task.cancelled || task.expired) return { performed: false, ...this.view() };
    this.#busy = true;
    try {
      const valid = () => {
        const current = this.#client.snapshot(), resources = this.#resourcesFor(current, this.#clock());
        return !this.#closed && this.#entry.grant.lifePermitId === grant.lifePermitId && this.#currentWorld(current, grant) && signature(current) === this.#world && signature(snapshot) === this.#world &&
          uint(this.#clock()) && this.#clock() < grant.expiresAtMs && this.#version() === task.policyVersion && !task.cancelled && !task.expired && current.player?.dead !== true &&
          current.inSafeZone === true && distance(current.player, grant.home) <= 1 && resources.known && resources.hp / resources.maxHp >= 0.8 && resources.gold >= 40 && resources.freeBagSlots > 0 && resources.currentWeight < resources.maxWeight &&
          current.trade?.uncertain !== true && current.trade?.active !== true && stillSafe();
      };
      if (!valid()) return { performed: false, blocked: true, ...this.view() };
      const requestId = `life:${createHash("sha256").update(grant.lifePermitId).digest("hex")}:supply`;
      this.#set({ childSupplyClaimed: true, childRequestId: requestId, step: "claiming-supply", reason: "唯一补给申请已持久登记；结果未知时不会再次申请。" });
      try {
        const result = await this.#client.requestLifeSupply({ lifePermitId: grant.lifePermitId, requestId });
        const child = validateSupplyGrant(result?.grant);
        if (child.worldEpoch !== grant.worldEpoch || child.actorCharacterIndex !== grant.actorCharacterIndex || child.maxGold > grant.budgetGold || child.quantity !== 1 || child.mapFileName !== grant.mapFileName || !same(child.returnAnchor, grant.home) || child.expiresAtMs > grant.expiresAtMs) throw new Error("Life child supply exceeds its parent.");
        this.#set({ childSupplyPermitId: child.permitId });
        if (!valid()) { this.#set({ step: "awaiting-owner", reason: "补给申请期间状态已改变；保留名额但不开始花费。" }); return { performed: true, blocked: true, ...this.view() }; }
        this.#supply.authorize(child); this.#set({ step: "supplying", reason: "只执行本次已授权的一瓶药补给。" });
      } catch { this.#set({ step: "awaiting-owner", reason: "本次补给申请未核实；名额保留，不自动重试或申请替代订单。" }); }
      return { performed: true, ...this.view() };
    } finally { this.#busy = false; this.#closeResolve?.(); }
  }

  /** Called only by the ordinary executor after its second observation,
   * policy/world/permit checks and actual collision planning. No new authority,
   * model request or economy operation is introduced by a blocked route. */
  recordNavigation({ state, action, outcome, nowMs = this.#clock() }) {
    if (this.#closed || !this.#entry || !["no-path", "rejected", "moved"].includes(outcome)) return false;
    const { grant, task } = this.#entry;
    if (terminal(task.status) || task.status === "paused" || task.activationPending === true || !uint(nowMs) ||
        !this.#currentWorld(state) || signature(state) !== this.#world || state.player?.dead === true || state.playerHp === 0 ||
        action?.lifePermitId !== grant.lifePermitId || action.policyVersion !== task.policyVersion || this.#version() !== task.policyVersion) return false;
    if (action.kind === "attack" && action.reason === "life-hunt-observed-monster" && task.status === "running" && !task.cancelled && !task.expired && nowMs < grant.expiresAtMs) {
      const target = (state.entities ?? []).find(e => e.objectId === action.targetId && liveMonster(e) && MONSTERS.has(e.name) && inside(e, grant.bounds) &&
        same(action.target, { x: e.x, y: e.y }) && distance(state.player, e) <= (e.objectId === this.#huntTarget ? HUNT_RETAIN_RADIUS : HUNT_ACQUIRE_RADIUS));
      if (!target || outcome === "moved") return false;
      this.#deferredHuntTargets.delete(target.objectId);
      this.#deferredHuntTargets.set(target.objectId, { name: target.name, observedAtMs: nowMs, expiresAtMs: nowMs + HUNT_DEFER_MS });
      while (this.#deferredHuntTargets.size > MAX_DEFERRED_HUNT_TARGETS) this.#deferredHuntTargets.delete(this.#deferredHuntTargets.keys().next().value);
      this.#huntTarget = null;
      this.#set({ step: "patrolling", reason: "当前猎物路线过长或受阻；暂时寻找其他目标。" });
      this.#report({ type: "lifeTargetDeferred", targetId: target.objectId, retryAfterMs: nowMs + HUNT_DEFER_MS });
      return true;
    }
    const route = action.kind === "follow" && action.reason === "life-route-waypoint" && task.status === "running" && task.step === "patrolling" &&
      !task.cancelled && !task.expired && nowMs < grant.expiresAtMs && action.targetId === `life-waypoint:${task.waypointIndex}` &&
      action.desiredDistance === 1 && same(action.target, grant.waypoints[task.waypointIndex]);
    const home = action.kind === "retreat" && action.reason === "life-return-home" && task.step === "returning-home" &&
      action.desiredDistance === 1 && same(action.target, grant.home);
    if (!route && !home) return false;
    const navigation = navigationState(task.navigation);
    if (outcome === "moved") {
      this.#set({ navigation: route ? emptyNavigation() : { ...navigation, noPathCount: 0, homeNoPathCount: 0 } });
      return true;
    }
    if (home) {
      const homeNoPathCount = Math.min(3, navigation.homeNoPathCount + 1);
      this.#set({ navigation: { ...navigation, homeNoPathCount }, ...(homeNoPathCount === 3 ? {
        status: "paused", step: "awaiting-resume", reason: "返家路线连续三次被拒绝或不可达；已暂停生活并等待主人处理，尚未到家。" } : {}) });
      return true;
    }
    const count = navigation.waypointIndex === task.waypointIndex ? navigation.noPathCount + 1 : 1;
    if (count < 3) {
      this.#set({ navigation: { ...navigation, waypointIndex: task.waypointIndex, noPathCount: count }, reason: `当前固定路点无可行路径（${count}/3），保持观察。` });
      return true;
    }
    this.#planToken++; // A late finite plan must not restore the obstructed point.
    const failedWaypoints = [...new Set([...navigation.failedWaypoints, task.waypointIndex])];
    if (failedWaypoints.length === grant.waypoints.length) {
      this.#set({ navigation: { ...navigation, noPathCount: 0, failedWaypoints, returning: true, homeNoPathCount: 0 },
        status: "returning", step: "returning-home", returnCycle: task.returnCycle + 1, reason: "三个固定路点均无可行路径；停止巡猎，安全返家。" });
    } else {
      const next = [1, 2].map(offset => (task.waypointIndex + offset) % grant.waypoints.length).find(index => !failedWaypoints.includes(index));
      this.#set({ waypointIndex: next, navigation: { ...navigation, waypointIndex: next, noPathCount: 0, failedWaypoints },
        reason: "当前固定路点连续三次不可达；改查下一个已授权路点。" });
    }
    return true;
  }

  movementConstraint(state, action) {
    if (!this.#entry || !action?.lifePermitId || action.lifePermitId !== this.#entry.grant.lifePermitId || !this.#currentWorld(state) || signature(state) !== this.#world || this.#version() !== this.#entry.task.policyVersion) return null;
    const bounds = this.#entry.grant.bounds;
    return { bounds: { ...bounds }, permits: next => inside(next, bounds) };
  }

  observe(event) {
    if (this.#closed || !this.#entry || this.#entry.task.activationPending === true || this.#entry.task.status === "paused" || terminal(this.#entry.task.status)) return;
    const state = this.#client.snapshot(), now = this.#clock(), { grant, task } = this.#entry, value = event?.payload;
    if (!this.#currentWorld(state) || signature(state) !== this.#world || !uint(now) || now >= grant.expiresAtMs || task.cancelled || task.expired || this.#version() !== task.policyVersion) return;
    if (event?.packet === "ObjectStruck" && objectId(value?.attackerId) && objectId(value?.objectId) && value.attackerId !== value.objectId && [state.playerObjectId, state.selfZoneObjectId].filter(objectId).includes(value.attackerId) && (state.entities ?? []).some(e => e.objectId === value.objectId && liveMonster(e) && inside(e, grant.bounds))) {
      this.#hits.set(value.objectId, now);
    } else if (event?.packet === "ObjectDied" && this.#hits.has(value?.objectId)) {
      const hit = this.#hits.get(value.objectId); this.#hits.delete(value.objectId);
      if (now < hit || now - hit > 30_000) return;
      task.killsObserved++;
      this.#queueFact("life-kill", `kill:${task.killsObserved}`, state, { targetObjectId: value.objectId });
    }
  }

  #queueFact(kind, suffix, state, facts) {
    const { grant, task } = this.#entry, digest = createHash("sha256").update(grant.lifePermitId).digest("hex");
    task.pendingFacts.push({ kind, eventId: `life:${digest}:${suffix}`, lifePermitId: grant.lifePermitId, mapFileName: grant.mapFileName, observedAtMs: this.#clock(),
      sourceEventRef: `observation:${state.connectionEpoch}:${state.revision}`, killsObserved: task.killsObserved, ...facts });
    this.#save(); this.#flushFacts();
  }
  #flushFacts() {
    const task = this.#entry.task;
    while (task.pendingFacts.length) {
      try { this.#onObserved(structuredClone(task.pendingFacts[0])); task.pendingFacts.shift(); this.#save(); }
      catch { this.#report({ type: "lifeMemoryWriteUnavailable" }); return; }
    }
  }
  #recordMemory(event) {
    const summary = event.kind === "life-kill" ? `服务器观察：阿洛实际命中目标后收到其死亡事件；本次独立生活参战击杀累计${event.killsObserved}。`
      : event.kind === "life-returned" ? `服务器观察：阿洛实际回到比奇授权安全点的${event.arrivalDistance}格范围内。` : `服务器观察：本次独立生活已${event.completion === "cancelled" ? "撤销" : "到期"}，并实际回到授权安全点旁。`;
    this.#store?.recordEpisode({ episodeId: event.eventId, kind: "life-progress", action: event.kind === "life-kill" ? "hunt" : "return-home", outcome: "observed", summary,
      locationRef: "map:0", ...(event.targetObjectId === undefined ? {} : { objectRef: `monster:${event.targetObjectId}` }), context: { mode: "solo", sourceEventRef: event.sourceEventRef } });
  }

  #askPlan(reason) {
    if (!this.#requestPlan || !this.#entry || this.#entry.task.activationPending === true || this.#entry.task.modelDecisionsUsed >= 4 || this.#closed) return;
    const { grant, task } = this.#entry;
    const plannedKey = `${reason}:${task.returnCycle}`;
    if (task.lastPlannedKey === plannedKey) return;
    const token = ++this.#planToken;
    this.#set({ modelDecisionsUsed: task.modelDecisionsUsed + 1, decisionMode: "model-planning", lastPlannedKey: plannedKey });
    const state = this.#client.snapshot();
    this.#pendingPlan = Promise.resolve().then(() => {
      if (this.#closed || token !== this.#planToken || task.cancelled || task.expired || this.#clock() >= grant.expiresAtMs || this.#version() !== task.policyVersion || !this.#currentWorld(this.#client.snapshot()) || signature(this.#client.snapshot()) !== signature(state)) return null;
      return this.#requestPlan({ lifePermitId: grant.lifePermitId, reason, state, resources: this.#resourcesFor(state, this.#clock()), routeId: grant.routeId, waypoints: structuredClone(grant.waypoints) });
    })
      .then(plan => {
        if (this.#closed || token !== this.#planToken || !this.#entry || this.#entry.grant.lifePermitId !== grant.lifePermitId || task.cancelled || task.expired || this.#clock() >= grant.expiresAtMs || this.#version() !== task.policyVersion || !this.#currentWorld(this.#client.snapshot()) || signature(this.#client.snapshot()) !== signature(state)) return;
        if (plan == null || plan.status === "pending") { this.#set({ decisionMode: "rules" }); return; }
        if (!["route", "hunt", "rest"].includes(plan.mode) || Object.keys(plan).some(key => !["mode", "waypointIndex", "reply", "rememberedEpisodeIds"].includes(key)) ||
            plan.reply !== undefined && (typeof plan.reply !== "string" || [...plan.reply].length > 400 || /[\p{Cc}\p{Cf}\p{Cs}]/u.test(plan.reply) || /\b(?:sk|ghp|github_pat)[-_][a-z0-9_-]{8,}|\bbearer\s+\S+/iu.test(plan.reply)) ||
            plan.rememberedEpisodeIds !== undefined && (!Array.isArray(plan.rememberedEpisodeIds) || plan.rememberedEpisodeIds.length > 5 || plan.rememberedEpisodeIds.some(value => typeof value !== "string" || value.length > 100)) ||
            plan.waypointIndex !== undefined && (!Number.isInteger(plan.waypointIndex) || plan.waypointIndex < 0 || plan.waypointIndex >= grant.waypoints.length)) throw new Error("Unsupported finite life plan.");
        this.#set({ modelMode: plan.mode, decisionMode: "model", modelRestUntilMs: plan.mode === "rest" ? this.#clock() + 30_000 : null, modelReply: plan.reply ?? null, ...(plan.waypointIndex === undefined ? {} : { waypointIndex: plan.waypointIndex }) });
      }).catch(() => { if (!this.#closed && token === this.#planToken) this.#set({ decisionMode: "rules" }); });
  }

  async close() {
    if (this.#closePromise) return this.#closePromise;
    this.#closed = true; this.#planToken++; this.#client.off?.("frame", this.#listener);
    this.#closePromise = (async () => { if (this.#busy) await new Promise(resolve => { this.#closeResolve = resolve; }); this.#journal.close(); })();
    return this.#closePromise;
  }
}
