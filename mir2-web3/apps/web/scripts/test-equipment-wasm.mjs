import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const manifest = JSON.parse(readFileSync(new URL("../lib/generated/client_core_runtime.json", import.meta.url), "utf8"));
const packageRoot = new URL(`../public/client-core/${manifest.version}/`, import.meta.url);
const glue = readFileSync(new URL("mir2_platform_web.js", packageRoot), "utf8");
const binary = readFileSync(new URL("mir2_platform_web_bg.wasm", packageRoot));
const wasm = await import(`data:text/javascript;base64,${Buffer.from(glue).toString("base64")}`);
await wasm.default({ module_or_path: binary });

const controllerUrl = new URL("../lib/equipment-session-controller.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(controllerUrl, "utf8"), {
  fileName: fileURLToPath(controllerUrl),
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const controllerModule = { exports: {} };
new Function("exports", "module", compiled.outputText)(controllerModule.exports, controllerModule);
const { EquipmentSessionController } = controllerModule.exports;

const json = (value) => JSON.stringify(value);
const decode = (value) => JSON.parse(value);
function actualRuntime() {
  return {
    createEquipmentPendingLedger() {
      const bridge = new wasm.EquipmentPendingBridge();
      return {
        replaceSnapshot: (snapshot) => decode(bridge.replace_snapshot(json(snapshot))),
        reserve: (operation) => decode(bridge.reserve(json(operation))),
        acknowledge: (operation, success) => decode(bridge.acknowledge(json({ operation, success }))),
        releaseUnsent: (operation) => decode(bridge.release_unsent(json(operation))),
        contains: (operation) => decode(bridge.contains(json(operation))),
        hasInstance: (uniqueId) => decode(bridge.has_instance(json({ uniqueId }))),
        status: () => decode(bridge.status()),
      };
    },
  };
}

const session = { connectionGeneration: 3, sessionGeneration: 5 };
const bag = (uniqueId, slot) => ({ uniqueId, container: 0, slot });
const equip = (uniqueId, slot) => ({ uniqueId, container: 2, slot });
const stored = (uniqueId, slot) => ({ uniqueId, container: 4, slot });
const snapshot = (...placements) => ({ capacity: 46, placements });
const equipOperation = (uniqueId, to = 0, grid = "inventory") => ({ kind: "equip", grid, uniqueId, to });
const removeOperation = (uniqueId, to = 4) => ({ kind: "remove", grid: "inventory", uniqueId, to });

function started(initial, identity = session) {
  const controller = new EquipmentSessionController(actualRuntime());
  assert.equal(controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: initial }).ok, true);
  return controller;
}

test("published WASM exposes additive equipment ABI without changing Quest ABI", () => {
  assert.equal(wasm.client_core_abi_version(), manifest.abiVersion);
  assert.equal(wasm.equipment_pending_abi_version(), 1);
  assert.equal(binary.length, manifest.files["mir2_platform_web_bg.wasm"].bytes);
  assert.ok(binary.length < 256 * 1024);
});

test("real ledger holds UID 0 Remove through success ACK until authoritative bag placement", () => {
  const controller = started(snapshot(equip(0, 0)));
  const operation = removeOperation(0);
  const reserved = controller.reserve({ ...session, ownerRevision: 0, operation });
  assert.equal(reserved.ok, true);
  assert.equal(controller.markSent(reserved.ticket).ok, true);
  assert.equal(controller.applyAck({ ...session, operation, success: true }).matched, true);
  assert.equal(controller.status().barriers, 1);
  assert.equal(controller.hasPendingInstance(0).reserved, true);
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(equip(0, 0)) }).pending, 1);
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(bag(0, 4)) }).pending, 0);
});

test("real ledger records UID 4 Equip snapshot first and releases only on exact ACK", () => {
  const controller = started(snapshot(bag(4, 4)));
  const operation = equipOperation(4);
  const reserved = controller.reserve({ ...session, ownerRevision: 0, operation });
  assert.equal(reserved.ok, true);
  controller.markSent(reserved.ticket);
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(equip(4, 0)) }).pending, 1);
  assert.equal(controller.applyAck({ ...session, operation: { ...operation, to: 1 }, success: true }).matched, false);
  assert.equal(controller.status().pending, 1);
  assert.equal(controller.applyAck({ ...session, operation: { ...operation, grid: "Inventory" }, success: true }).matched, true);
  assert.equal(controller.status().pending, 0);
});

test("same destination slot and owner change allow one command, never a duplicate", () => {
  const controller = started(snapshot(bag(4, 4), bag(5, 5)));
  const first = equipOperation(4);
  assert.equal(controller.reserve({ ...session, ownerRevision: 0, operation: first }).ok, true);
  assert.equal(controller.setOwnerRevision(1).ok, true);
  assert.equal(controller.reserve({ ...session, ownerRevision: 0, operation: equipOperation(5) }).error, "staleOwner");
  assert.equal(controller.reserve({ ...session, ownerRevision: 1, operation: equipOperation(5) }).error, "equipmentSlotBusy");
  assert.equal(controller.reserve({ ...session, ownerRevision: 1, operation: first }).error, "instanceBusy");
  assert.equal(controller.status().pending, 1);
});

test("duplicate UID or occupied cell snapshot is atomically rejected without unlocking", () => {
  const controller = started(snapshot(bag(4, 4)));
  const operation = equipOperation(4);
  controller.reserve({ ...session, ownerRevision: 0, operation });
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(bag(4, 4), equip(4, 0)) }).error, "invalidPlacement");
  assert.equal(controller.status().pending, 1);
  assert.equal(controller.status().ready, false);
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(bag(4, 4), bag(9, 4)) }).error, "invalidPlacement");
  assert.equal(controller.hasPendingInstance(4).reserved, true);
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(equip(4, 0)) }).ok, true);
  assert.equal(controller.applyAck({ ...session, operation, success: true }).matched, true);
  assert.equal(controller.status().pending, 0);
});

test("storage Equip requires explicit source proof and keeps legacy ACK-only completion", () => {
  const initial = { ...snapshot(stored(8, 3)), storageCapacity: 64 };
  const controller = started(initial);
  assert.equal(controller.reserve({ ...session, ownerRevision: 0, operation: equipOperation(8, 1) }).error, "placementMismatch");
  const operation = equipOperation(8, 1, "storage");
  assert.equal(controller.reserve({ ...session, ownerRevision: 0, operation }).ok, true);
  assert.equal(controller.applyAck({ ...session, operation, success: true }).matched, true);
  assert.equal(controller.status().pending, 0);
});

test("unknown send outcome stays reserved; only a definitely unsent ticket can release", () => {
  const controller = started(snapshot(bag(4, 4)));
  const operation = equipOperation(4);
  const reserved = controller.reserve({ ...session, ownerRevision: 0, operation });
  assert.equal(controller.markOutcomeUnknown(reserved.ticket).ok, true);
  assert.equal(controller.cancelDefinitelyUnsent(reserved.ticket).error, "notProvenUnsent");
  assert.equal(controller.hasPendingInstance(4).reserved, true);
  assert.equal(controller.applyAck({ ...session, operation, success: false }).matched, true);
  assert.equal(controller.status().pending, 0);
});

test("old socket and session cannot alter a new confirmed session, including same-socket relogin", () => {
  const controller = started(snapshot(bag(4, 4)));
  const operation = equipOperation(4);
  controller.reserve({ ...session, ownerRevision: 0, operation });
  controller.suspendConnection({ ...session, reason: "socketClosed" });
  assert.equal(controller.observeSnapshot({ ...session, complete: true, snapshot: snapshot(equip(4, 0)) }).error, "connectionSuspended");
  const next = { connectionGeneration: 4, sessionGeneration: 6 };
  assert.equal(controller.establishBaseline({ ...next, startGameConfirmed: true, complete: true, snapshot: snapshot(bag(4, 4)) }).ok, true);
  assert.equal(controller.applyAck({ ...session, operation, success: true }).error, "staleSession");
  assert.equal(controller.status().pending, 0);
  assert.equal(controller.terminateSession(next).ok, true);
  assert.equal(controller.observeSnapshot({ ...next, complete: true, snapshot: snapshot(bag(4, 4)) }).error, "staleSession");
  assert.equal(controller.establishBaseline({ connectionGeneration: 4, sessionGeneration: 7,
    startGameConfirmed: true, complete: true, snapshot: snapshot(bag(4, 4)) }).ok, true);
});
