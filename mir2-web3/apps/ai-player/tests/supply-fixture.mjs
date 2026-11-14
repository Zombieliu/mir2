export const ACTOR = "11".repeat(32), SCOPE = "22".repeat(32), CATALOG = "33".repeat(32);
export const producer = (changes = {}) => ({ actor: ACTOR, producerScope: SCOPE, serverRevision: "1", ...changes });
export const intent = (changes = {}) => ({ request: { itemIndex: "77", count: 1, panelType: 0 }, currency: "gold", source: "trade", serviceCatalogProof: CATALOG, ...changes });
export const operation = (changes = {}) => ({ actor: ACTOR, requestScope: SCOPE, sequence: "1", intent: intent(), ...changes });
export const receipt = (op = operation(), changes = {}) => ({ producerScope: SCOPE, entry: { operation: structuredClone(op), serverRevision: "2", outcome: { committed: { request: structuredClone(op.intent.request), currency: "gold", source: "trade", charged: 40, admittedCount: 1, incomingUniqueId: "9007199254740993" } }, ...changes } });
export const goods = (changes = {}) => ({ id: 77, uniqueId: 77, purchaseItemIndex: "77", itemIndex: 658, count: 1, name: "(HP)DrugSmall", price: 40, ...changes });
export const shop = (changes = {}) => ({ npcObjectId: 8, scriptKey: "BichonProvince/BorderVillage/Grocery", service: "BUYSELL", packetType: "NPCGoods", rate: 1, panelType: 0, hideAddedStats: false, displayGoods: [goods()], ...changes });
export const world = (changes = {}) => ({
  tick: 1, mapFileName: "0", playerObjectId: 12, selfZoneObjectId: 50001, playerHp: 18, playerMaxHp: 18, gold: 200,
  entities: [{ objectId: 12, kind: "selfPlayer", name: "Alo", x: 288, y: 609, direction: "Down", hp: 18, maxHp: 18, dead: false }, { objectId: 8, kind: "npc", name: "Merchant_Ruben", x: 288, y: 608 }],
  inventoryItems: [], beltItems: [], equipmentItems: [], questLog: [], groundDrops: [],
  activeNpcDialog: { npcObjectId: 8, npcName: "Merchant_Ruben", links: [{ text: "View", target: "@BuySell" }], body: [] }, nativeNpcShop: shop(), ...changes,
});
export const grant = (changes = {}) => ({ permitId: "supply:test", worldEpoch: "epoch:test", actorCharacterIndex: 2, mapFileName: "0", npcIndex: 9, npcName: "Merchant_Ruben", npcPosition: { x: 288, y: 608 }, itemIndex: 658, itemName: "(HP)DrugSmall", quantity: 1, maxGold: 50, expiresAtMs: 300_000, returnAnchor: { x: 288, y: 616 }, bounds: { minX: 270, minY: 595, maxX: 310, maxY: 635 }, ...changes });
export const ownerFrame = (request, reply, snapshot = world(), authority = producer()) => ({ type: "npcPurchaseOwner", protocolVersion: 1, requestId: request.requestId, reply, snapshot, authority });
