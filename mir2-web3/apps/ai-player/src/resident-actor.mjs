import { createServer } from "node:net";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { DIRECTIONS, GatewayClient } from "./gateway-client.mjs";
import { CompanionController } from "./companion-controller.mjs";
import { acquireArchiveGuard } from "./archive-guard.mjs";
import { CompanionRuntimeService, validateRuntimeService, archiveKeyFromEnvironment } from "./companion-runtime-service.mjs";
import { SupplyController } from "./supply/supply-controller.mjs";
import { LifeController } from "./life/life-controller.mjs";
import { GiftController } from "./gifts/gift-controller.mjs";

const APP_ROOT = fileURLToPath(new URL("../", import.meta.url));

export async function acquireResidentGuard(port) {
  if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error("guardPort must be a fixed port from 1024 to 65535.");
  const guard = createServer((socket) => socket.destroy());
  await new Promise((resolve, reject) => {
    guard.once("error", reject);
    guard.listen({ host: "127.0.0.1", port, exclusive: true }, resolve);
  });
  return () => new Promise((resolve, reject) => guard.close((error) => error ? reject(error) : resolve()));
}

/** B1: one ordinary network player, with explicit actions and bounded nearby exploration. */
export class ResidentActor {
  #modelClient;
  #environment; #supplyFactory; #memoryPath; #runPromise = null; #runAbort = null; #stopPromise = null; #stopping = false;
  constructor(profile, { client = new GatewayClient(profile), report = () => {}, modelClient, environment = process.env, supplyFactory = options => new SupplyController(options) } = {}) {
    this.profile = profile;
    this.client = client;
    this.report = report;
    this.character = null;
    this.releaseGuard = null;
    this.companion = null;
    this.store = null;
    this.releaseArchiveGuard = null;
    this.runtimeService = null;
    this.supplyController = null;
    this.lifeController = null; this.giftController = null;
    this.#modelClient = modelClient;
    this.#environment = environment; this.#supplyFactory = supplyFactory;
  }

  async start({ accountId, password, createAccount = false, createCharacter = false }) {
    if (!accountId || !password) throw new Error("Supply a dedicated account and password through environment variables.");
    const service = validateRuntimeService(this.profile);
    if (service) archiveKeyFromEnvironment(service, this.#environment).fill(0);
    this.#stopping = false;
    if (service) this.releaseArchiveGuard = await acquireArchiveGuard(this.profile.companion.archiveId);
    try {
      this.releaseGuard = await acquireResidentGuard(this.profile.guardPort);
      await this.client.connect();
      if (service && (!this.client.features.companionControl || this.client.worldEpoch !== this.profile.companion.worldEpoch)) throw new Error("This gateway has not enabled the configured companion world enrollment.");
      if (createAccount) await this.client.createAccount(accountId, password);
      const characters = await this.client.login(accountId, password);
      this.character = characters.find((entry) => entry.name === this.profile.characterName);
      if (!this.character && createCharacter) this.character = await this.client.createCharacter(this.profile.characterName);
      if (!this.character) throw new Error("Configured character was not found; character creation must be explicitly enabled.");
      if (String(this.character.class).toLowerCase() !== "warrior") throw new Error("The Bichon B1 resident requires a Warrior.");
      if (service && this.character.index !== this.profile.companion.characterIndex) throw new Error("The login character does not match the enrolled archive.");
      if (this.profile.companion) {
        if (!this.client.features.sharedParty) throw new Error("This gateway does not advertise authoritative shared groups for the companion.");
        const options = this.profile.companion;
        if (!options.residentId || !options.leaderName) throw new Error("Configure companion residentId and approved leaderName.");
        const { CompanionStore } = await import("./memory/companion-store.mjs");
        this.#memoryPath = resolve(APP_ROOT, options.memoryPath ?? `.data/${options.residentId}.sqlite`);
        this.store = new CompanionStore({
          path: this.#memoryPath,
          binding: { realmId: this.profile.realmId, residentId: options.residentId,
            characterName: this.character.name, characterIndex: this.character.index,
            ...(options.operatorRef ? { operatorRef: options.operatorRef } : {}) },
          ...(service ? { archiveId: options.archiveId, ownerRef: options.ownerRef, worldEpoch: options.worldEpoch,
            displayName: options.displayName ?? "阿洛", appearance: { class: "Warrior", gender: this.character.gender ?? "Male" },
            baseSetting: options.baseSetting ?? "比奇的合成人冒险者", adoptLegacy: options.adoptLegacy === true } : {}),
        });
        if (!this.releaseArchiveGuard) this.releaseArchiveGuard = await acquireArchiveGuard(this.store.archive().archiveId);
      }
      await this.client.startGame(this.character.index);
      if (this.profile.companion) {
        const options = this.profile.companion;
        await this.client.setPeaceAttackMode();
        for (const [name, slot] of [["WoodenSword", 0], ["BaseDress(M)", 1]]) {
          const item = this.client.snapshot().inventoryItems.find((entry) => entry.name === name);
          if (item) await this.client.equipOwnedItem(item.uniqueId, slot);
        }
        this.companion = new CompanionController({
          client: this.client, store: this.store, leaderName: options.leaderName,
          // The exported Crystal Bichon start safe zone, not the place where a
          // restarted process happens to find its character in the wilderness.
          homePosition: options.homePosition ?? { x: 288, y: 616, mapFileName: "0" },
          report: this.report,
        });
        await this.client.setAllowGroup(true);
        if (service) {
          this.supplyController = this.#supplyFactory({ client: this.client, dbPath: `${this.#memoryPath}.supply.sqlite`, report: this.report,
            stepTo: request => this.companion.stepForSupply(request), onObserved: event => recordSupplyObservation(this.store, event) });
          this.companion.supplyController = this.supplyController;
          this.lifeController = new LifeController({ client: this.client, dbPath: `${this.#memoryPath}.life.sqlite`, supplyController: this.supplyController, store: this.store, report: this.report,
            requestPlan: service.provider === "openai" ? input => this.runtimeService?.planLife(input) ?? null : undefined });
          this.giftController = new GiftController({ client: this.client, ownerName: options.leaderName, dbPath: `${this.#memoryPath}.gifts.sqlite`, report: this.report,
            onObserved: event => recordGiftObservation(this.store, event) });
          this.companion.lifeController = this.lifeController; this.companion.giftController = this.giftController;
          this.runtimeService = new CompanionRuntimeService({ client: this.client, store: this.store, controller: this.companion,
            configuration: service, realmId: this.profile.realmId, report: this.report, modelClient: this.#modelClient,
            environment: this.#environment, supplyController: this.supplyController, lifeController: this.lifeController, giftController: this.giftController });
          await this.runtimeService.start();
        }
        this.report({ type: "companionReady", residentId: options.residentId, policyVersion: this.store.profile().policyVersion });
      }
    this.report({ type: "residentReady", state: publicStatus(this.client.snapshot(), Boolean(this.companion), this.runtimeService?.modelEnabled === true) });
      return this.client.snapshot();
    } catch (error) {
      await this.stop({ save: this.client.snapshot().ready });
      throw error;
    }
  }

  async run(options = {}) {
    if (this.#runPromise || this.#stopping) throw new Error("Resident is already running or stopping.");
    this.#runAbort = new AbortController();
    const signal = options.signal ? AbortSignal.any([options.signal, this.#runAbort.signal]) : this.#runAbort.signal;
    const running = this.#runLoop({ ...options, signal }); this.#runPromise = running;
    try { return await running; }
    finally { if (this.#runPromise === running) { this.#runPromise = null; this.#runAbort = null; } }
  }

  async #runLoop({ durationMs = 60_000, explore = false, signal } = {}) {
    const started = Date.now();
    const initial = this.client.snapshot().player;
    let lastStatus = 0;
    let directionIndex = 0;
    while (!this.#stopping && !signal?.aborted && Date.now() - started < durationMs) {
      const state = this.client.snapshot();
      if (!state.ready) throw new Error("Resident lost its active world session.");
      if (Date.now() - lastStatus >= 5_000) {
        this.report({ type: "residentStatus", state: publicStatus(state, Boolean(this.companion), this.runtimeService?.modelEnabled === true) });
        lastStatus = Date.now();
      }
      if (this.#stopping || signal?.aborted) break;
      // Exploration is an explicit B1 diagnostic, not a survival or life policy.
      const healthKnown = Number.isFinite(state.playerHp) && Number.isFinite(state.playerMaxHp) && state.playerMaxHp > 0;
      if (this.companion) {
        await this.companion.tick();
      } else if (explore && healthKnown && !state.player?.dead && state.playerHp >= state.playerMaxHp * 0.8) {
        const offsetX = state.player.x - initial.x;
        const offsetY = state.player.y - initial.y;
        const direction = Math.max(Math.abs(offsetX), Math.abs(offsetY)) >= 2
          ? DIRECTIONS.find((value) => value === directionToward(-offsetX, -offsetY))
          : ["Right", "Down", "Left", "Up"][directionIndex++ % 4];
        const result = await this.client.walk(direction);
        this.report({ type: "movement", moved: result.moved, position: result.after && { x: result.after.x, y: result.after.y } });
      }
      await sleep(this.companion ? 650 : 1_000, undefined, { signal }).catch((error) => {
        if (error.name !== "AbortError") throw error;
      });
    }
    return publicStatus(this.client.snapshot(), Boolean(this.companion), this.runtimeService?.modelEnabled === true);
  }

  async stop({ save = true } = {}) {
    if (this.#stopPromise) return this.#stopPromise;
    if (!this.releaseGuard && !this.releaseArchiveGuard && !this.store && !this.runtimeService && !this.supplyController && !this.#runPromise) return false;
    const stopping = this.#stop({ save }); this.#stopPromise = stopping;
    try { return await stopping; }
    finally { if (this.#stopPromise === stopping) this.#stopPromise = null; }
  }

  async #stop({ save }) {
    let saved = false;
    this.#stopping = true; this.#runAbort?.abort();
    try {
      if (this.companion) await this.companion.close();
      if (this.#runPromise) await this.#runPromise.catch(() => {});
      try { if (this.runtimeService) { await this.runtimeService.close(); this.runtimeService = null; } }
      finally {
        try { if (this.giftController) await this.giftController.close(); }
        finally { try { if (this.lifeController) await this.lifeController.close(); } finally { if (this.supplyController) await this.supplyController.close(); } }
      }
      if (save && this.client.snapshot().ready) saved = await this.client.logout();
    } finally {
      try {
        try { await this.client.close(); }
        finally { this.store?.close(); this.store = null; this.companion = null; this.supplyController = null; this.lifeController = null; this.giftController = null; this.#memoryPath = null; }
      } finally {
        try { await this.releaseGuard?.(); }
        finally { this.releaseGuard = null; await this.releaseArchiveGuard?.(); this.releaseArchiveGuard = null; }
      }
    }
    this.report({ type: "residentStopped", saved });
    return saved;
  }
}

/** Called only by the receipt-verifying supply controller. Plans, permits and
 * pending results are not observed adventures and never enter portable growth. */
export function recordSupplyObservation(store, event) {
  const u64 = (value, terminal = false) => typeof value === "string" && /^(?:0|[1-9][0-9]{0,19})$/.test(value) && BigInt(value) > 0n && BigInt(value) <= 18446744073709551615n && (!terminal || BigInt(value) < 18446744073709551615n);
  const point = value => value && [value.x, value.y].every(n => Number.isInteger(n) && n >= 0 && n <= 65535);
  if (!event || !["supply-acquired", "supply-returned"].includes(event.kind) || typeof event.permitId !== "string" || !/^[A-Za-z0-9._:-]{1,100}$/.test(event.permitId) ||
      !Number.isInteger(event.chargedGold) || event.chargedGold < 1 || event.chargedGold > 50 || event.acquiredQuantity !== 1 || !u64(event.receiptRevision, true)) throw new Error("Verified supply observation is required.");
  const digest = createHash("sha256").update(event.permitId).digest("hex");
  const common = { outcome: "observed", locationRef: "map:0", context: { mode: "solo", sourceEventRef: `receipt:${digest}:${event.receiptRevision}` } };
  if (event.kind === "supply-acquired") {
    if (!u64(event.incomingUniqueId)) throw new Error("Verified incoming item identity is required.");
    return store.recordEpisode({ ...common, episodeId: `supply:${digest}:acquired`, kind: "supply-acquired", action: "purchase", targetRef: "npc:9", objectRef: `item:${event.incomingUniqueId}`,
      summary: `服务器回执：在比奇杂货商处实际买入1瓶小型生命药水，扣除${event.chargedGold}游戏金币；回执版本${event.receiptRevision}。` });
  }
  const scope = event.scope, distance = point(event.returnAnchor) && point(event.actualPosition) ? Math.max(Math.abs(event.returnAnchor.x - event.actualPosition.x), Math.abs(event.returnAnchor.y - event.actualPosition.y)) : null;
  const inside = p => scope?.bounds && [scope.bounds.minX, scope.bounds.minY, scope.bounds.maxX, scope.bounds.maxY].every(Number.isInteger) &&
    scope.bounds.minX >= 270 && scope.bounds.maxX <= 310 && scope.bounds.minY >= 595 && scope.bounds.maxY <= 635 && scope.bounds.minX <= scope.bounds.maxX && scope.bounds.minY <= scope.bounds.maxY &&
    p.x >= scope.bounds.minX && p.x <= scope.bounds.maxX && p.y >= scope.bounds.minY && p.y <= scope.bounds.maxY;
  if (distance === null || distance > 1 || distance !== event.arrivalDistance || scope?.mapFileName !== "0" || scope.maximumDistanceFromAnchor !== 1 || !inside(event.returnAnchor) || !inside(event.actualPosition)) throw new Error("Observed arrival inside the permitted scope is required.");
  return store.recordEpisode({ ...common, episodeId: `supply:${digest}:returned`, kind: "supply-returned", action: "return-home",
    summary: `服务器回执已确认购药1瓶、扣除${event.chargedGold}游戏金币；随后实际回到本次授权出发点(${event.returnAnchor.x},${event.returnAnchor.y})的1格范围内。` });
}

export function recordGiftObservation(store, event) {
  if (event?.kind !== "owner-gift-received" || !/^gift:[a-f0-9-]{36}$/.test(event.eventId) || !Number.isSafeInteger(event.receivedGold) || event.receivedGold < 0 || !Array.isArray(event.receivedItems) || !/^trade:\d+:\d+:\d+$/.test(event.sourceEventRef)) throw new Error("Verified owner gift observation is required.");
  return store.recordEpisode({ episodeId: event.eventId, kind: "owner-gift-received", action: "receive-gift", outcome: "observed", locationRef: "map:0",
    summary: `已核对主人赠予的${event.receivedGold}游戏金币和${event.receivedItems.length}件补给入包；阿洛没有付出资产。`, context: { mode: "solo", sourceEventRef: event.sourceEventRef } });
}

function directionToward(x, y) {
  return [
    ["UpLeft", "Up", "UpRight"],
    ["Left", "Down", "Right"],
    ["DownLeft", "Down", "DownRight"],
  ][Math.sign(y) + 1][Math.sign(x) + 1];
}

export function publicStatus(state, companion = false, modelConfigured = false) {
  return {
    stage: companion ? "B2-companion" : "B1-body",
    modelEnabled: modelConfigured,
    longTermMemoryEnabled: companion,
    memoryScope: companion ? "local-owner-feedback-and-action-episodes" : "none",
    ready: state.ready,
    mapFileName: state.mapFileName,
    player: state.player && { x: state.player.x, y: state.player.y, direction: state.player.direction },
    hp: state.playerHp,
    maxHp: state.playerMaxHp,
    gold: state.gold,
    nearbyPlayers: state.entities.filter((entity) => entity.kind === "player").length,
    inventoryStacks: state.inventoryItems.length,
    quests: state.questLog.length,
  };
}
