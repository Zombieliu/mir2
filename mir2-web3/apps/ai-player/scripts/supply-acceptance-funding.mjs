// Test preparation only. The product supply runner receives no selling power.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { setTimeout as sleep } from "node:timers/promises";
import { ProtocolClient } from "../../web/scripts/quest-agent/protocol-client.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";
import { nextWalkDirection } from "../src/skills/navigation.mjs";

const allowed = new Set(["clientVersion", "login", "startGame", "keepAlive", "walk", "interact", "selectNpcDialog", "sellItem", "logOut"]);
const hash = value => createHash("sha256").update(String(value)).digest("hex");
const safe = value => JSON.parse(JSON.stringify(value, (_key, item) => typeof item === "bigint" ? item.toString() : item));
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const self = snapshot => snapshot?.entities?.find(e => String(e.objectId) === String(snapshot.playerObjectId));
const quantity = (snapshot, name) => [...snapshot?.inventoryItems ?? [], ...snapshot?.beltItems ?? []]
  .filter(item => item.name === name).reduce((n, item) => n + Number(item.quantity ?? 0), 0);

/** Reuses the ordinary quest client while preserving incoming/outgoing u64 UIDs.
 * The inherited trace is private and redacts login passwords. This narrow
 * wrapper cannot buy, spawn, give money, teleport, trade, or change saves. */
class FundingClient extends ProtocolClient {
  record(direction, value) { return super.record(direction, safe(value)); }
  send(command) {
    if (!allowed.has(command.type)) throw new Error("The funding helper refused an unsupported command.");
    const redacted = { ...command };
    if ("password" in redacted) redacted.password = "[redacted]";
    this.record("sent", redacted);
    this.ws.send(stringifyGatewayJson(command));
  }
  observeGatewayMessage(message) {
    super.observeGatewayMessage(message);
    if (message.type === "worldSnapshot") this.lastFullSequence = this.sequence;
  }
  async connect() {
    this.questDefinitions.clear();
    this.record("lifecycle", { type: "connection", url: this.url });
    this.ws = new WebSocket(this.url);
    this.ws.addEventListener("message", event => {
      try { this.observeGatewayMessage(parseGatewayJson(event.data)); }
      catch { this.failure = new Error("An exact ordinary funding observation could not be decoded."); }
    });
    this.ws.addEventListener("close", () => { this.closed = true; });
    this.ws.addEventListener("error", () => { this.failure = new Error("The ordinary funding connection failed."); });
    await this.wait(() => this.ws.readyState === WebSocket.OPEN, "funding connection");
    this.send({ type: "clientVersion" });
    this.timer = setInterval(() => { if (this.ws.readyState === WebSocket.OPEN) this.send({ type: "keepAlive", time: Date.now() }); }, 2000);
  }
}

function ensureHealthy(snapshot) {
  const player = self(snapshot);
  if (!player || player.dead === true || !Number.isSafeInteger(snapshot.playerHp)
      || !Number.isSafeInteger(snapshot.playerMaxHp) || snapshot.playerMaxHp <= 0
      || snapshot.playerHp < snapshot.playerMaxHp || snapshot.mapFileName !== "0") {
    throw new Error("Ordinary funding stopped because the character was not healthy in Bichon.");
  }
}

async function walk(client, collision, target, desiredDistance = 1) {
  for (let attempt = 0; attempt < 120; attempt++) {
    const snapshot = client.snapshot;
    ensureHealthy(snapshot);
    const player = self(snapshot);
    if (distance(player, target) <= desiredDistance) return;
    const occupied = snapshot.entities.filter(e => String(e.objectId) !== String(snapshot.playerObjectId) && e.dead !== true)
      .map(e => ({ x: e.x, y: e.y }));
    const direction = nextWalkDirection({ player, target, desiredDistance, collision, occupied });
    if (!direction) throw new Error("No ordinary collision-respecting path to the funding merchant.");
    const previous = { x: player.x, y: player.y };
    await client.request({ type: "walk", direction }, "UserLocation", 15_000);
    await client.wait(() => { const p = self(client.snapshot); return p && (p.x !== previous.x || p.y !== previous.y); }, "funding walk result", 15_000);
    await sleep(650);
  }
  throw new Error("The bounded ordinary funding walk exhausted its step allowance.");
}

async function sellObservedStarter(client, collision, specification) {
  await walk(client, collision, specification.merchant, 1);
  const npc = client.snapshot.entities.find(e => e.kind === "npc" && e.name === specification.merchant.name
    && e.x === specification.merchant.x && e.y === specification.merchant.y);
  assert.ok(npc, "The real funding merchant must be currently observed.");
  const beforeInteract = client.sequence;
  client.send({ type: "interact", objectId: npc.objectId });
  await client.wait(() => client.sequence > beforeInteract
    && String(client.snapshot.activeNpcDialog?.npcObjectId) === String(npc.objectId), "funding merchant dialog");
  const link = client.snapshot.activeNpcDialog.links.find(link => ["@sell", "@buysell"].includes(String(link.target).toLowerCase())
    && link.disabled !== true && link.enabled !== false);
  assert.ok(link, "The merchant must expose a real enabled sell service.");
  await client.request({ type: "selectNpcDialog", target: link.target }, "NPCSell", 15_000);
  const item = client.snapshot.inventoryItems.find(item => item.name === specification.itemName);
  assert.ok(item && item.quantity === 1, "Sell only the exact observed single owned starter item.");
  assert.ok(Number.isSafeInteger(item.sellValue) && item.sellValue > 0, "Use only an observed positive sale price.");
  const beforeGold = client.snapshot.gold;
  assert.ok(Number.isSafeInteger(beforeGold) && beforeGold >= 0);
  const before = client.sequence;
  const acknowledgement = await client.request({ type: "sellItem", uniqueId: item.uniqueId, count: 1 }, "SellItem", 15_000);
  assert.equal(acknowledgement.payload?.success, true, "The ordinary server must accept the starter sale.");
  await client.wait(() => client.lastFullSequence > before && client.snapshot.gold === beforeGold + item.sellValue
    && !client.snapshot.inventoryItems.some(value => String(value.uniqueId) === String(item.uniqueId)), "fresh exact starter sale result", 15_000);
  ensureHealthy(client.snapshot);
  return { merchantName: specification.merchant.name, itemName: item.name, itemUniqueIdHash: hash(item.uniqueId), quantity: 1,
    observedUnitValue: item.sellValue, beforeGold, afterGold: client.snapshot.gold, exactInventoryRemoval: true };
}

export async function fundSyntheticSupplyActor({ url, identity, tracePath, collision, checkpoint = () => {} }) {
  const client = new FundingClient(url, tracePath);
  try {
    await client.connect();
    await client.request({ type: "login", accountId: identity.accountId, password: identity.password }, "LoginSuccess");
    const acknowledgement = await client.request({ type: "startGame", characterIndex: identity.characterIndex }, "StartGame");
    assert.equal(acknowledgement.payload?.result, 4);
    await client.wait(() => client.snapshot?.mapFileName === "0" && self(client.snapshot), "ordinary funding character bootstrap");
    ensureHealthy(client.snapshot);
    const before = { gold: client.snapshot.gold, hpPotions: quantity(client.snapshot, "(HP)DrugSmall"), candles: quantity(client.snapshot, "Candle") };
    assert.ok(before.hpPotions >= 1 && before.candles >= 1, "Preserve the original starter HP potion and candle.");
    const sales = [];
    for (const specification of [
      { itemName: "WoodenSword", merchant: { name: "Blacksmith_Smith", x: 296, y: 613 } },
      { itemName: "BaseDress(M)", merchant: { name: "Merchant_Whitney", x: 305, y: 608 } },
    ]) {
      sales.push(await sellObservedStarter(client, collision, specification));
      checkpoint(`ordinary-starter-sale-${sales.length}`);
    }
    assert.ok(client.snapshot.gold >= 80, "The real starter sales must fund two distinct 40-gold purchases.");
    assert.equal(quantity(client.snapshot, "(HP)DrugSmall"), before.hpPotions);
    assert.equal(quantity(client.snapshot, "Candle"), before.candles);
    await walk(client, collision, { x: 288, y: 616 }, 0);
    return { source: "ordinary-owned-starter-resale", initialGold: before.gold, finalGold: client.snapshot.gold,
      sales, preservedHpPotions: before.hpPotions, preservedCandles: before.candles,
      actualPreparationPosition: { x: self(client.snapshot).x, y: self(client.snapshot).y },
      syntheticOnly: true, productSellingCapabilityAdded: false, questOrCombatRevenueClaimed: false };
  } finally { await client.close(); }
}
