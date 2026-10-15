import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourceUrl = new URL("../lib/world-model/item-identity.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(sourceUrl, "utf8"), {
  fileName: fileURLToPath(sourceUrl),
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const identity = { exports: {} };
new Function("exports", "module", compiled.outputText)(identity.exports, identity);
const {
  authoritativeItemUniqueId,
  crystalInventoryCapacities,
  currentAuthoritativeEquipmentItem,
  currentAuthoritativeItem,
  equipmentSourceGrid,
  planBagMove,
  planEquipmentRemoval,
  projectInventoryItemIdentity,
  projectStoragePacketItemIdentity,
  sameAuthoritativeItemIdentities,
} = identity.exports;

const pageUrl = new URL("../app/page.tsx", import.meta.url);
const pageSource = readFileSync(pageUrl, "utf8");
const pageSyntax = ts.createSourceFile(fileURLToPath(pageUrl), pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
let moveItemDeclaration;
let ownerDeclaration;
function findMoveItem(node) {
  if (ts.isFunctionDeclaration(node) && node.name?.text === "moveItem") moveItemDeclaration = node.getText(pageSyntax);
  if (ts.isFunctionDeclaration(node) && node.name?.text === "currentEquipmentOwner") ownerDeclaration = node.getText(pageSyntax);
  ts.forEachChild(node, findMoveItem);
}
findMoveItem(pageSyntax);
assert.ok(moveItemDeclaration, "page moveItem declaration is required");
const bagModule = { exports: {} };
new Function("exports", "module", "require", ts.transpileModule(
  readFileSync(new URL("../lib/bevy-bag-ui.ts", import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText)(bagModule.exports, bagModule, () => identity.exports);
const moveItemJavaScript = ts.transpileModule(`${ownerDeclaration}\n${moveItemDeclaration}`, {
  fileName: "move-item.ts",
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;
function runPageMove({ current, selected, to, toContainer, maxBagSlots = 40, pendingIds = [],
  renderConnection = 3, renderSession = 5, connection = 3, playerSession = 5,
  suspendReason = null, cachedSnapshot = true, sendAccepted = true }) {
  const commands = [];
  const logs = [];
  const controller = pendingIds.length ? {
    hasPendingInstance: (uniqueId) => ({ ok: true, reserved: pendingIds.includes(uniqueId), pending: pendingIds.length }),
    status: () => ({ pending: pendingIds.length }),
  } : null;
  const moveItem = new Function("planBagMove", "authoritativeItemUniqueId", "worldRef",
    "equipmentControllerRef", "equipmentRenderConnectionGeneration", "equipmentConnectionGenerationRef",
    "equipmentRenderSessionGeneration", "equipmentSessionGenerationRef", "equipmentHostSuspendReasonRef",
    "equipmentStartGameRef", "equipmentSnapshotRef", "send", "appendLog", "t",
    "equipmentRenderOwnerToken", "equipmentBagOwnerRef", "sameBagOwner", "bevyBagSendGateRef",
    `${moveItemJavaScript}\nreturn moveItem;`)(
    planBagMove,
    authoritativeItemUniqueId,
    { current: { inventoryItems: current, maxBagSlots } },
    { current: controller },
    renderConnection, { current: connection }, renderSession, { current: playerSession },
    { current: suspendReason },
    { current: { connectionGeneration: connection, sessionGeneration: playerSession } },
    { current: cachedSnapshot ? { connectionGeneration: connection, sessionGeneration: playerSession } : null },
    (command) => {
      if (!sendAccepted) return false;
      commands.push(command);
      return true;
    },
    (message) => logs.push(message),
    (_key, _args, fallback) => fallback,
    { runGeneration: 0, ownerRevision: 0, owner: "react", connectionGeneration: renderConnection, sessionGeneration: renderSession },
    { current: { runGeneration: 0, ownerRevision: 0, owner: "react" } },
    bagModule.exports.sameBagOwner,
    { current: () => false },
  );
  const submitted = moveItem(selected, to, toContainer);
  return { commands, logs, submitted };
}

function inventoryMoveCalls() {
  const url = new URL("../app/components/original-client-inventory-window.tsx", import.meta.url);
  const syntax = ts.createSourceFile(fileURLToPath(url), readFileSync(url, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const calls = [];
  let mailItemLockedDeclaration;
  function visit(node) {
    if (ts.isFunctionDeclaration(node) && node.name?.text === "mailItemLocked") mailItemLockedDeclaration = node.getText(syntax);
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "onMoveItem") {
      calls.push(node);
    }
    ts.forEachChild(node, visit);
  }
  visit(syntax);
  assert.equal(calls.length, 3, "occupied bag, empty bag, and storage move callbacks");
  assert.ok(mailItemLockedDeclaration, "actual Mail item lock guard is required");
  return { syntax, calls, mailItemLockedDeclaration };
}

test("inventory move callbacks preserve the clicked destination page through the real page planner", () => {
  const { syntax, calls } = inventoryMoveCalls();
  const invoke = (index, values, onMoveItem) => new Function(
    "pendingMoveItem", "item", "slotIndex", "activeTab", "absoluteSlot", "onMoveItem", calls[index].getText(syntax),
  )(...values, onMoveItem);
  const source = { container: "bag1", slot: 0, uniqueId: 4, authoritativeUniqueId: 4 };
  const target = { container: "bag2", slot: 4, uniqueId: 0, authoritativeUniqueId: 0 };
  for (const callbackIndex of [0, 1]) {
    const current = callbackIndex === 0 ? [source, target] : [source];
    let result;
    invoke(callbackIndex, [source, target, 4, "bag2", 20], (selected, to, toContainer) => {
      result = runPageMove({ current, selected, to, toContainer, maxBagSlots: 48 });
    });
    assert.deepEqual(result.commands, [{ type: "moveItem", grid: "inventory", from: 0, to: 44 }]);
  }
  const storage = { container: "storage", slot: 19, uniqueId: 0, authoritativeUniqueId: 0 };
  let storageResult;
  invoke(2, [storage, target, 4, "bag2", 20], (selected, to, toContainer) => {
    assert.equal(toContainer, "storage");
    storageResult = runPageMove({ current: [], selected, to, toContainer });
  });
  assert.deepEqual(storageResult.commands, [{ type: "moveItem", grid: "storage", from: 19, to: 20 }]);
});

test("actual inventory handlers show a failed move only as failure and clear selection after either result", () => {
  const { syntax, calls, mailItemLockedDeclaration } = inventoryMoveCalls();
  for (const [index, call] of calls.entries()) {
    let handler = call.parent;
    while (handler && !ts.isFunctionDeclaration(handler) && !ts.isArrowFunction(handler)) handler = handler.parent;
    assert.ok(handler, "move call must belong to an inventory event handler");
    const javascript = ts.transpileModule(`${mailItemLockedDeclaration}\nconst handler = ${handler.getText(syntax)};`, {
      compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
    }).outputText;
    for (const submitted of [false, true]) {
      const feedback = [];
      const selections = [];
      let requests = 0;
      const scope = {
        window: {}, mailLocks: [], storageMode: null, deleteMode: false, sellMode: false,
        pendingMoveItem: { key: "sword", name: "WoodenSword", uniqueId: 4, authoritativeUniqueId: 4,
          container: index === 2 ? "storage" : "bag1", slot: 4 },
        slotIndex: 0, activeTab: "bag2", absoluteSlot: 20, slot: { key: "slot-0-0" },
        visibleStorageItems: [], storageLocked: false, storagePageLocked: false,
        onMoveItem: () => { requests++; return submitted; },
        onMergeItem: () => { throw new Error("move test must not enter Merge"); },
        setDeleteFeedback: (value) => feedback.push(value),
        setPendingMoveItem: (value) => selections.push(value),
        t: (key, _args, fallback) => fallback ?? key,
      };
      const keys = Object.keys(scope);
      const invoke = new Function(...keys, `${javascript}\nreturn handler;`)(...keys.map((key) => scope[key]));
      invoke({ key: "armour", name: "TargetItem", container: "bag2", slot: 0 });
      assert.equal(requests, 1);
      assert.deepEqual(selections, [null]);
      assert.deepEqual(feedback, [submitted
        ? index === 0 ? "ui.inventory: WoodenSword -> TargetItem"
          : index === 1 ? "ui.inventory: WoodenSword -> slot-0-0"
            : "Storage items: WoodenSword -> 21"
        : "Item action failed."]);
    }
  }
});

test("actual page move returns its submission result for rejected bag plans and every existing grid", () => {
  const item = { container: "bag1", slot: 4, authoritativeUniqueId: 4 };
  const input = { current: [item], selected: item, to: 0, toContainer: "bag1" };
  assert.equal(runPageMove(input).submitted, true);
  for (const rejected of [
    { toContainer: "bag2" }, { toContainer: undefined }, { pendingIds: [4] },
    { renderConnection: 2 }, { renderSession: 4 }, { suspendReason: "logoutPending" },
    { cachedSnapshot: false }, { sendAccepted: false },
    { current: [{ ...item, authoritativeUniqueId: 8 }] },
  ]) {
    const result = runPageMove({ ...input, ...rejected });
    assert.equal(result.submitted, false);
    assert.deepEqual(result.commands, []);
  }
  for (const container of ["belt", "storage", "quest"]) {
    const route = { current: [], selected: { ...item, uniqueId: 4, container }, to: 0, toContainer: container };
    assert.equal(runPageMove(route).submitted, true, container);
    assert.equal(runPageMove({ ...route, sendAccepted: false }).submitted, false, container);
  }
});

test("actual page move sends bag cells and blocks a pending equipment instance", () => {
  const item = { container: "bag1", slot: 0, uniqueId: 4, authoritativeUniqueId: 4 };
  assert.deepEqual(runPageMove({ current: [item], selected: item, to: 4, toContainer: "bag1" }).commands,
    [{ type: "moveItem", grid: "inventory", from: 0, to: 4 }]);
  assert.equal(runPageMove({ current: [item], selected: item, to: 4, toContainer: "bag1", pendingIds: [4] }).commands.length, 0);
  assert.equal(runPageMove({ current: [{ ...item, authoritativeUniqueId: 5 }], selected: item, to: 4, toContainer: "bag1" }).commands.length, 0);
  const bag2 = { container: "bag2", slot: 0, uniqueId: 0, authoritativeUniqueId: 0 };
  assert.deepEqual(runPageMove({ current: [bag2], selected: bag2, to: 7, toContainer: "bag2", maxBagSlots: 48 }).commands,
    [{ type: "moveItem", grid: "inventory", from: 40, to: 47 }]);
});

test("actual page move blocks pending destination, old render, logout, and missing baseline", () => {
  const source = { container: "bag1", slot: 0, authoritativeUniqueId: 4 };
  const target = { container: "bag1", slot: 4, authoritativeUniqueId: 0 };
  const input = { current: [source, target], selected: source, to: 4, toContainer: "bag1" };
  assert.deepEqual(runPageMove(input).commands,
    [{ type: "moveItem", grid: "inventory", from: 0, to: 4 }]);
  assert.equal(runPageMove({ ...input, pendingIds: [0] }).commands.length, 0);
  assert.equal(runPageMove({ ...input, pendingIds: [4] }).commands.length, 0);
  assert.equal(runPageMove({ ...input, renderConnection: 2 }).commands.length, 0);
  assert.equal(runPageMove({ ...input, renderSession: 4 }).commands.length, 0);
  assert.equal(runPageMove({ ...input, suspendReason: "logoutPending" }).commands.length, 0);
  assert.equal(runPageMove({ ...input, cachedSnapshot: false }).commands.length, 0);
});

test("actual page move leaves non-bag routes on their existing contract", () => {
  assert.deepEqual(runPageMove({ current: [], selected: { container: "belt", slot: 2 }, to: 3 }).commands,
    [{ type: "moveItem", grid: "belt", from: 2, to: 3 }]);
  assert.deepEqual(runPageMove({ current: [], selected: { container: "storage", slot: 19 }, to: 20 }).commands,
    [{ type: "moveItem", grid: "storage", from: 19, to: 20 }]);
  assert.deepEqual(runPageMove({ current: [], selected: { container: "storage", slot: 19 }, to: 20, toContainer: "storage" }).commands,
    [{ type: "moveItem", grid: "storage", from: 19, to: 20 }]);
  assert.deepEqual(runPageMove({ current: [], selected: { container: "quest", slot: 2, uniqueId: 9 }, to: 3, toContainer: "quest" }).commands,
    [{ type: "moveItem", grid: "questInventory", from: 9, to: 3 }]);
});

test("actual page moves between explicit bag pages and checks the real destination pending instance", () => {
  const source = { container: "bag1", slot: 0, authoritativeUniqueId: 4 };
  const target = { container: "bag2", slot: 4, authoritativeUniqueId: 0 };
  const otherPageTarget = { container: "bag1", slot: 4, authoritativeUniqueId: 8 };
  const current = [source, target, otherPageTarget];
  const input = { current, selected: source, to: 4, toContainer: "bag2", maxBagSlots: 48 };
  assert.deepEqual(runPageMove(input).commands,
    [{ type: "moveItem", grid: "inventory", from: 0, to: 44 }]);
  assert.equal(runPageMove({ ...input, pendingIds: [0] }).commands.length, 0);
  assert.equal(runPageMove({ ...input, pendingIds: [4] }).commands.length, 0);
  assert.equal(runPageMove({ ...input, pendingIds: [8] }).commands.length, 1,
    "an item at the same local index on the source page is not the target");
  assert.deepEqual(runPageMove({ ...input, selected: target, to: 0, toContainer: "bag1" }).commands,
    [{ type: "moveItem", grid: "inventory", from: 44, to: 0 }]);
  assert.equal(runPageMove({ ...input, selected: target, to: 0, toContainer: "bag1", pendingIds: [4] }).commands.length, 0);
});

test("actual page rejects missing or non-bag destinations instead of inferring a source page", () => {
  const item = { container: "bag1", slot: 0, authoritativeUniqueId: 0 };
  for (const toContainer of [undefined, null, "belt", "storage", "quest", "inventory"]) {
    const result = runPageMove({ current: [item], selected: item, to: 4, toContainer });
    assert.equal(result.commands.length, 0, String(toContainer));
    assert.equal(result.logs.length, 1);
  }
});

test("bag MoveItem uses current cells rather than the server instance ID", () => {
  const uid4AtZero = { container: "bag1", slot: 0, authoritativeUniqueId: 4 };
  assert.deepEqual(planBagMove([uid4AtZero], 40, uid4AtZero, 4, "bag1"),
    { command: { type: "moveItem", grid: "inventory", from: 0, to: 4 }, affectedUniqueIds: [4] });
  const uid0AtFour = { container: "bag1", slot: 4, authoritativeUniqueId: 0 };
  assert.deepEqual(planBagMove([uid0AtFour], 40, uid0AtFour, 0, "bag1"),
    { command: { type: "moveItem", grid: "inventory", from: 4, to: 0 }, affectedUniqueIds: [0] });
});

test("bag2 MoveItem offsets local cells and honors authoritative total capacity", () => {
  const secondBag = { container: "bag2", slot: 0, authoritativeUniqueId: 47 };
  assert.equal(planBagMove([secondBag], 40, secondBag, 1, "bag2"), null);
  assert.deepEqual(planBagMove([secondBag], 48, secondBag, 7, "bag2"),
    { command: { type: "moveItem", grid: "inventory", from: 40, to: 47 }, affectedUniqueIds: [47] });
  assert.equal(planBagMove([secondBag], 48, secondBag, 8, "bag2"), null);
  assert.equal(planBagMove([secondBag], 47, secondBag, 6, "bag2"), null);
});

test("cross-page bag moves use independent cells, including equal local indices, with capacity checks on both ends", () => {
  const first = { container: "bag1", slot: 0, authoritativeUniqueId: 4 };
  const second = { container: "bag2", slot: 0, authoritativeUniqueId: 0 };
  assert.deepEqual(planBagMove([first, second], 48, first, 0, "bag2"),
    { command: { type: "moveItem", grid: "inventory", from: 0, to: 40 }, affectedUniqueIds: [4, 0] });
  assert.deepEqual(planBagMove([first, second], 48, second, 0, "bag1"),
    { command: { type: "moveItem", grid: "inventory", from: 40, to: 0 }, affectedUniqueIds: [0, 4] });
  assert.deepEqual(runPageMove({ current: [first, second], selected: first, to: 0, toContainer: "bag2", maxBagSlots: 48 }).commands,
    [{ type: "moveItem", grid: "inventory", from: 0, to: 40 }]);
  assert.equal(planBagMove([first], 40, first, 0, "bag2"), null);
  assert.equal(planBagMove([second], 40, second, 1, "bag1"), null);
  assert.equal(planBagMove([first], 48, first, 8, "bag2"), null);
  const outsideCapacity = { ...second, slot: 8 };
  assert.equal(planBagMove([outsideCapacity], 48, outsideCapacity, 0, "bag1"), null);
  const lastUnlocked = { ...second, slot: 7 };
  assert.deepEqual(planBagMove([lastUnlocked], 48, lastUnlocked, 39, "bag1"),
    { command: { type: "moveItem", grid: "inventory", from: 47, to: 39 }, affectedUniqueIds: [0] });
});

test("cross-page bag target identity must be unique and authoritative on the target page", () => {
  const source = { container: "bag1", slot: 0, authoritativeUniqueId: 4 };
  const target = { container: "bag2", slot: 4, authoritativeUniqueId: 0 };
  assert.equal(planBagMove([source, target, { ...target, authoritativeUniqueId: 5 }], 48, source, 4, "bag2"), null);
  assert.equal(planBagMove([source, { ...target, authoritativeUniqueId: undefined }], 48, source, 4, "bag2"), null);
  assert.equal(planBagMove([source, target, { container: "bag1", slot: 4, authoritativeUniqueId: 0 }], 48, source, 4, "bag2"), null);
});

test("bag MoveItem rejects stale, ambiguous, untrusted, or invalid source and target", () => {
  const selected = { container: "bag1", slot: 0, authoritativeUniqueId: 4 };
  const replaced = { container: "bag1", slot: 0, authoritativeUniqueId: 9 };
  assert.equal(planBagMove([replaced], 40, selected, 4, "bag1"), null);
  assert.equal(planBagMove([selected, { container: "bag1", slot: 1, authoritativeUniqueId: 4 }], 40, selected, 4, "bag1"), null);
  assert.equal(planBagMove([selected, selected], 40, selected, 4, "bag1"), null);
  for (const badId of [undefined, -1, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.equal(planBagMove([{ ...selected, authoritativeUniqueId: badId }], 40,
      { ...selected, authoritativeUniqueId: badId }, 4, "bag1"), null);
  }
  for (const target of [-1, 0, 40, 1.5, NaN, Infinity]) {
    assert.equal(planBagMove([selected], 40, selected, target, "bag1"), null);
  }
  const occupied = { container: "bag1", slot: 4, authoritativeUniqueId: 0 };
  assert.deepEqual(planBagMove([selected, occupied], 40, selected, 4, "bag1"),
    { command: { type: "moveItem", grid: "inventory", from: 0, to: 4 }, affectedUniqueIds: [4, 0] });
  assert.equal(planBagMove([selected, occupied, { ...occupied, authoritativeUniqueId: 6 }], 40, selected, 4, "bag1"), null);
  assert.equal(planBagMove([selected, { ...occupied, authoritativeUniqueId: undefined }], 40, selected, 4, "bag1"), null);
  assert.equal(planBagMove([selected, occupied, { container: "bag1", slot: 5, authoritativeUniqueId: 0 }], 40, selected, 4, "bag1"), null);
});

test("Crystal ResizeInventory converts whole-array sizes to bag capacity", () => {
  assert.deepEqual(crystalInventoryCapacities(46), { inventoryCapacity: 46, maxBagSlots: 40 });
  assert.deepEqual(crystalInventoryCapacities(54), { inventoryCapacity: 54, maxBagSlots: 48 });
  assert.deepEqual(crystalInventoryCapacities(86), { inventoryCapacity: 86, maxBagSlots: 80 });
  for (const invalid of [0, 40, 47, 53, 55, 87, NaN, Infinity, 54.5, "54", null, undefined]) {
    assert.equal(crystalInventoryCapacities(invalid), null, String(invalid));
  }
});

test("missing server ID stays display-only, including second bag and belt slot aliases", () => {
  assert.deepEqual(projectInventoryItemIdentity(undefined, "bag1", 4), { uniqueId: 4 });
  assert.deepEqual(projectInventoryItemIdentity(null, "bag2", 4), { uniqueId: 44 });
  assert.deepEqual(projectInventoryItemIdentity(undefined, "belt", 2), { uniqueId: 2 });
  assert.deepEqual(projectInventoryItemIdentity(undefined, "storage", 2), { uniqueId: 2 });
  assert.equal(currentAuthoritativeItem([
    { container: "bag2", slot: 4, ...projectInventoryItemIdentity(null, "bag2", 4) },
  ], { container: "bag2", slot: 4, uniqueId: 44 }), null);
});

test("zero is a real server item instance ID", () => {
  const projected = projectInventoryItemIdentity(0, "bag1", 6);
  assert.deepEqual(projected, { uniqueId: 0, authoritativeUniqueId: 0 });
  const item = { container: "bag1", slot: 6, ...projected };
  assert.equal(currentAuthoritativeItem([item], { container: "bag1", slot: 6, uniqueId: 0 }), item);
});

test("invalid and imprecise IDs cannot authorize an item command", () => {
  for (const value of [-1, 1.5, Number.MAX_SAFE_INTEGER + 1, NaN, Infinity, "42", null, undefined]) {
    assert.equal(authoritativeItemUniqueId(value), undefined, String(value));
    const item = { container: "bag1", slot: 3, ...projectInventoryItemIdentity(value, "bag1", 3) };
    assert.equal(item.authoritativeUniqueId, undefined);
    assert.equal(currentAuthoritativeItem([item], { container: "bag1", slot: 3, uniqueId: item.uniqueId }), null);
  }
  assert.equal(authoritativeItemUniqueId(Number.MAX_SAFE_INTEGER), Number.MAX_SAFE_INTEGER);
});

test("same-slot replacement or movement invalidates a captured instance selection", () => {
  const original = { container: "bag1", slot: 7, ...projectInventoryItemIdentity(101, "bag1", 7) };
  const selection = { container: "bag1", slot: 7, uniqueId: original.authoritativeUniqueId };
  assert.equal(currentAuthoritativeItem([original], selection), original);
  const replacement = { container: "bag1", slot: 7, ...projectInventoryItemIdentity(102, "bag1", 7) };
  assert.equal(currentAuthoritativeItem([replacement], selection), null);
  const moved = { ...original, slot: 8 };
  assert.equal(currentAuthoritativeItem([moved], selection), null);
  assert.equal(currentAuthoritativeItem([original, replacement], selection), null);
});

test("equipment slot selection rejects a replaced or untrusted occupant before instance removal", () => {
  const selected = { slot: "weapon", authoritativeUniqueId: 0 };
  assert.equal(currentAuthoritativeEquipmentItem([selected], { slot: "weapon", uniqueId: 0 }), selected);
  assert.equal(currentAuthoritativeEquipmentItem([
    { slot: "weapon", authoritativeUniqueId: 91 },
  ], { slot: "weapon", uniqueId: 0 }), null);
  assert.equal(currentAuthoritativeEquipmentItem([
    { slot: "weapon" },
  ], { slot: "weapon", uniqueId: 0 }), null);
  assert.equal(currentAuthoritativeEquipmentItem([selected], {
    slot: "weapon", uniqueId: Number.MAX_SAFE_INTEGER + 1,
  }), null);
  assert.equal(currentAuthoritativeEquipmentItem([selected, selected], { slot: "weapon", uniqueId: 0 }), null);
});

test("remove request sends the equipped instance ID, independent of equipment and destination slots", () => {
  const selection = { slot: "weapon", uniqueId: 137 };
  const equipment = [{ slot: "weapon", authoritativeUniqueId: 137 }];
  const inventory = [{ container: "bag1", slot: 0 }, { container: "bag1", slot: 2 }];
  assert.deepEqual(planEquipmentRemoval(equipment, inventory, 40, selection), {
    type: "removeItem", uniqueId: 137, grid: "inventory", to: 1,
  });
  assert.deepEqual(planEquipmentRemoval([{ slot: "weapon", authoritativeUniqueId: 0 }], [], 40,
    { slot: "weapon", uniqueId: 0 }), {
    type: "removeItem", uniqueId: 0, grid: "inventory", to: 0,
  });
  assert.equal(planEquipmentRemoval([{ slot: "weapon", authoritativeUniqueId: 138 }], inventory, 40, selection), null);
  assert.equal(planEquipmentRemoval([{ slot: "weapon" }], inventory, 40, selection), null);
});

test("remove destination treats bag2 cells as unified indices and rejects a full or invalid bag", () => {
  const equipment = [{ slot: "weapon", authoritativeUniqueId: 137 }];
  const selection = { slot: "weapon", uniqueId: 137 };
  const bag1Full = Array.from({ length: 40 }, (_, slot) => ({ container: "bag1", slot }));
  const bag2First = [{ container: "bag2", slot: 0 }];
  const expanded = crystalInventoryCapacities(54);
  assert.deepEqual(planEquipmentRemoval(equipment, [...bag1Full, ...bag2First], expanded.maxBagSlots, selection), {
    type: "removeItem", uniqueId: 137, grid: "inventory", to: 41,
  });
  const bag2Full = Array.from({ length: 8 }, (_, slot) => ({ container: "bag2", slot }));
  assert.equal(planEquipmentRemoval(equipment, [...bag1Full, ...bag2Full], 48, selection), null);
  for (const invalidCapacity of [0, 41, 47, 81, 48.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.equal(planEquipmentRemoval(equipment, [], invalidCapacity, selection), null, String(invalidCapacity));
  }
});

test("movement snapshot authority changes survive an unchanged slot display ID", () => {
  const displayOnly = { uniqueId: 4, slot: 4 };
  const confirmed = { uniqueId: 4, authoritativeUniqueId: 4, slot: 4 };
  assert.equal(sameAuthoritativeItemIdentities([{ slot: 4 }], [displayOnly]), true);
  assert.equal(sameAuthoritativeItemIdentities([{ uniqueId: 4, slot: 4 }], [displayOnly]), false,
    "missing to confirmed UID 4 must force the full projection during a movement ACK");
  assert.equal(sameAuthoritativeItemIdentities([{ uniqueId: 4, slot: 4 }], [confirmed]), true);
  assert.equal(sameAuthoritativeItemIdentities([{ slot: 4 }], [confirmed]), false,
    "withdrawn authority must not retain a previously confirmed instance");
  assert.equal(sameAuthoritativeItemIdentities([{ uniqueId: Number.MAX_SAFE_INTEGER + 1, slot: 4 }], [confirmed]), false,
    "unsafe replacement cannot retain previous authority");
  assert.equal(sameAuthoritativeItemIdentities([{ uniqueId: 0 }], [{ authoritativeUniqueId: 0 }]), true);
});

test("equipment source grid preserves storage authority without changing destination slot", () => {
  assert.equal(equipmentSourceGrid("storage"), "storage");
  assert.equal(equipmentSourceGrid("bag1"), "inventory");
  assert.equal(equipmentSourceGrid("bag2"), "inventory");
  assert.equal(equipmentSourceGrid("belt"), "belt");
  assert.equal(equipmentSourceGrid("quest"), "questInventory");
});

test("storage refresh retains only packet-supplied authority", () => {
  assert.deepEqual(projectStoragePacketItemIdentity({ unique_id: 0 }, 5, 99), {
    uniqueId: 0,
    authoritativeUniqueId: 0,
  });
  assert.deepEqual(projectStoragePacketItemIdentity({}, 5, 99), { uniqueId: 99 });
  assert.deepEqual(projectStoragePacketItemIdentity({ uniqueId: Number.MAX_SAFE_INTEGER + 1 }, 5, 99), {
    uniqueId: Number.MAX_SAFE_INTEGER + 1,
  });
  assert.deepEqual(projectStoragePacketItemIdentity({ uniqueId: 12, unique_id: 13 }, 5), { uniqueId: 12 });
});
