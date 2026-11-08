import { createServer } from "node:net";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { DIRECTIONS, GatewayClient } from "./gateway-client.mjs";
import { CompanionController } from "./companion-controller.mjs";
import { acquireArchiveGuard } from "./archive-guard.mjs";
import { CompanionRuntimeService, validateRuntimeService, archiveKeyFromEnvironment } from "./companion-runtime-service.mjs";

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
  constructor(profile, { client = new GatewayClient(profile), report = () => {} } = {}) {
    this.profile = profile;
    this.client = client;
    this.report = report;
    this.character = null;
    this.releaseGuard = null;
    this.companion = null;
    this.store = null;
    this.releaseArchiveGuard = null;
    this.runtimeService = null;
  }

  async start({ accountId, password, createAccount = false, createCharacter = false }) {
    if (!accountId || !password) throw new Error("Supply a dedicated account and password through environment variables.");
    const service = validateRuntimeService(this.profile);
    if (service) archiveKeyFromEnvironment(service).fill(0);
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
        if (!this.client.features.sharedParty) throw new Error("Enable MIR2_SHARED_PARTY_ENABLED on this gateway before starting a companion.");
        const options = this.profile.companion;
        if (!options.residentId || !options.leaderName) throw new Error("Configure companion residentId and approved leaderName.");
        const { CompanionStore } = await import("./memory/companion-store.mjs");
        this.store = new CompanionStore({
          path: resolve(APP_ROOT, options.memoryPath ?? `.data/${options.residentId}.sqlite`),
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
          this.runtimeService = new CompanionRuntimeService({ client: this.client, store: this.store, controller: this.companion,
            configuration: service, realmId: this.profile.realmId, report: this.report });
          await this.runtimeService.start();
        }
        this.report({ type: "companionReady", residentId: options.residentId, policyVersion: this.store.profile().policyVersion });
      }
      this.report({ type: "residentReady", state: publicStatus(this.client.snapshot(), Boolean(this.companion)) });
      return this.client.snapshot();
    } catch (error) {
      await this.stop({ save: this.client.snapshot().ready });
      throw error;
    }
  }

  async run({ durationMs = 60_000, explore = false, signal } = {}) {
    const started = Date.now();
    const initial = this.client.snapshot().player;
    let lastStatus = 0;
    let directionIndex = 0;
    while (!signal?.aborted && Date.now() - started < durationMs) {
      const state = this.client.snapshot();
      if (!state.ready) throw new Error("Resident lost its active world session.");
      if (Date.now() - lastStatus >= 5_000) {
        this.report({ type: "residentStatus", state: publicStatus(state, Boolean(this.companion)) });
        lastStatus = Date.now();
      }
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
    return publicStatus(this.client.snapshot(), Boolean(this.companion));
  }

  async stop({ save = true } = {}) {
    if (!this.releaseGuard && !this.releaseArchiveGuard && !this.store && !this.runtimeService) return false;
    let saved = false;
    try {
      if (this.runtimeService) { await this.runtimeService.close(); this.runtimeService = null; }
      if (save && this.client.snapshot().ready) saved = await this.client.logout();
    } finally {
      try {
        await this.client.close();
        this.store?.close(); this.store = null; this.companion = null;
      } finally {
        try { await this.releaseGuard?.(); }
        finally { this.releaseGuard = null; await this.releaseArchiveGuard?.(); this.releaseArchiveGuard = null; }
      }
    }
    this.report({ type: "residentStopped", saved });
    return saved;
  }
}

function directionToward(x, y) {
  return [
    ["UpLeft", "Up", "UpRight"],
    ["Left", "Down", "Right"],
    ["DownLeft", "Down", "DownRight"],
  ][Math.sign(y) + 1][Math.sign(x) + 1];
}

export function publicStatus(state, companion = false) {
  return {
    stage: companion ? "B2-companion" : "B1-body",
    modelEnabled: false,
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
