import { EventEmitter } from "node:events";
import { mkdtempSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { LifeController } from "../src/life/life-controller.mjs";

export const grant = (changes = {}) => ({ lifePermitId: "life:test", worldEpoch: "world:test", actorCharacterIndex: 2, mapFileName: "0",
  startedAtMs: 0, expiresAtMs: 1_200_000, durationSeconds: 1200, budgetGold: 50, maxPurchasedQuantity: 1,
  bounds: { minX: 270, minY: 595, maxX: 310, maxY: 645 }, home: { x: 288, y: 616 }, routeId: "bichon-south-v1",
  waypoints: [{ x: 286, y: 628 }, { x: 294, y: 633 }, { x: 301, y: 626 }], ...changes });
export const potion = (quantity = 2) => ({ uniqueId: 22n, name: "(HP)DrugSmall", quantity, container: "bag1", slot: 0 });
export const world = (changes = {}) => {
  const player = { kind: "selfPlayer", objectId: 12, name: "Alo", x: 288, y: 616, dead: false, hp: 18 };
  return { ready: true, connectionEpoch: 1, revision: 1, mapIndex: 0, mapFileName: "0", player, playerObjectId: 12, selfZoneObjectId: 50001,
    playerHp: 18, playerMaxHp: 18, gold: 85, freeBagSlots: 40, currentWeight: 5, maxWeight: 100, inventoryStale: false, inventoryItems: [potion()], beltItems: [],
    entities: [player], groundDrops: [], engagements: [], party: { members: [], pendingInvite: null }, inSafeZone: true, ...changes };
};
export const childGrant = (changes = {}) => ({ permitId: "supply:life", worldEpoch: "world:test", actorCharacterIndex: 2, mapFileName: "0", npcIndex: 9, npcName: "Merchant_Ruben",
  npcPosition: { x: 288, y: 608 }, itemIndex: 658, itemName: "(HP)DrugSmall", quantity: 1, maxGold: 50, expiresAtMs: 300_000,
  returnAnchor: { x: 288, y: 616 }, bounds: { minX: 270, minY: 595, maxX: 310, maxY: 635 }, ...changes });
export const database = () => join(mkdtempSync(join(tmpdir(), "numeron-life-")), "life.sqlite");
export class Client extends EventEmitter {
  worldEpoch = "world:test"; activeCharacterIndex = 2; requests = []; time = 0;
  constructor(changes = {}) { super(); this.state = world(changes); }
  snapshot() { return structuredClone(this.state); }
  async requestLifeSupply(request) { this.requests.push(structuredClone(request)); return { grant: childGrant({ expiresAtMs: this.time + 300_000 }) }; }
}
export function fixture(t, { client = new Client(), path = database(), requestPlan, onObserved } = {}) {
  const facts = [], memory = [], policy = { policyVersion: 1, solo: { considerLootWhenSafe: true }, party: { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true }, permissions: { mapAllowlist: ["0"] } };
  const store = { profile: () => structuredClone(policy), recordEpisode: event => memory.push(event) };
  const supply = { current: { status: "idle", permitId: null, acquiredQuantity: 0, chargedGold: null, receiptKnown: false }, authorized: [],
    view() { return structuredClone(this.current); }, authorize(value) { this.authorized.push(structuredClone(value)); this.current = { ...this.current, status: "authorized", permitId: value.permitId }; },
    cancel(reason) { this.current.status = this.current.status === "unknown" ? "unknown" : "cancelled"; this.cancelReason = reason; } };
  const controller = new LifeController({ client, dbPath: path, supplyController: supply, store, clock: () => client.time, requestPlan, onObserved: onObserved ?? (event => facts.push(event)) });
  t.after(() => controller.close());
  return { controller, client, store, policy, supply, path, facts, memory, decide: (baseAction = { kind: "wait", reason: "no-authorized-party-activity", policyVersion: policy.policyVersion }) => controller.decide({ state: client.snapshot(), policy: store.profile(), baseAction, nowMs: client.time }) };
}
