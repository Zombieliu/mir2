import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourceUrl = new URL("../lib/equipment-session-controller.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(sourceUrl, "utf8"), {
  fileName: fileURLToPath(sourceUrl),
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);
const { EquipmentSessionController } = module.exports;

const baseline = { capacity: 46, placements: [{ uniqueId: 0, container: 0, slot: 4 }] };
const operation = { kind: "equip", grid: "inventory", uniqueId: 0, to: 0 };
const identity = { connectionGeneration: 3, sessionGeneration: 5 };

function fakeRuntime() {
  const ledgers = [];
  return {
    ledgers,
    createEquipmentPendingLedger() {
      const pending = new Map();
      const ledger = {
        pending,
        replaceSnapshot(snapshot) {
          if (snapshot?.capacity !== 46 || !Array.isArray(snapshot.placements)) {
            return { ok: false, error: "invalidSnapshot", pending: pending.size };
          }
          return { ok: true, pending: pending.size, released: 0 };
        },
        reserve(op) {
          if (pending.has(op.uniqueId)) return { ok: false, error: "instanceBusy", pending: pending.size };
          pending.set(op.uniqueId, op);
          return { ok: true, pending: pending.size };
        },
        acknowledge(op) {
          const held = pending.get(op.uniqueId);
          const matched = Boolean(held && held.kind === op.kind && held.grid.toLowerCase() === op.grid.toLowerCase() && held.to === op.to);
          if (matched) pending.delete(op.uniqueId);
          return { ok: true, matched, pending: pending.size };
        },
        releaseUnsent(op) {
          const released = pending.delete(op.uniqueId);
          return { ok: true, released, pending: pending.size };
        },
        contains(op) { return { ok: true, reserved: pending.has(op.uniqueId), pending: pending.size }; },
        hasInstance(uniqueId) { return { ok: true, reserved: pending.has(uniqueId), pending: pending.size }; },
        status() { return { ok: true, ready: true, pending: pending.size, barriers: 0 }; },
      };
      ledgers.push(ledger);
      return ledger;
    },
  };
}

test("confirmed StartGame and complete valid baseline are required before equipment sends", () => {
  const runtime = fakeRuntime();
  const controller = new EquipmentSessionController(runtime);
  assert.equal(controller.reserve({ ...identity, ownerRevision: 0, operation }).error, "staleSession");
  assert.equal(controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: { capacity: 54 } }).ok, false);
  assert.equal(runtime.ledgers.length, 1);
  assert.equal(controller.status().ready, false);
  assert.equal(controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline }).ok, true);
  assert.equal(controller.reserve({ ...identity, ownerRevision: 0, operation }).ok, true);
});

test("owner changes guard new selections without erasing pending; unknown send cannot be cancelled", () => {
  const controller = new EquipmentSessionController(fakeRuntime());
  controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline });
  const ticket = controller.reserve({ ...identity, ownerRevision: 0, operation }).ticket;
  controller.setOwnerRevision(1);
  assert.equal(controller.reserve({ ...identity, ownerRevision: 0, operation }).error, "staleOwner");
  assert.equal(controller.markOutcomeUnknown(ticket).ok, true);
  assert.equal(controller.cancelDefinitelyUnsent(ticket).error, "notProvenUnsent");
  assert.equal(controller.hasPendingInstance(0).reserved, true);
  assert.equal(controller.applyAck({ ...identity, operation: { ...operation, to: 1 }, success: true }).matched, false);
  assert.equal(controller.applyAck({ ...identity, operation, success: false }).matched, true);
  assert.equal(controller.hasPendingInstance(0).reserved, false);
});

test("only a definitely unsent ticket can release, and a sent ticket survives renderer cleanup", () => {
  const controller = new EquipmentSessionController(fakeRuntime());
  controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline });
  const first = controller.reserve({ ...identity, ownerRevision: 0, operation }).ticket;
  assert.equal(controller.cancelDefinitelyUnsent(first).released, true);
  const second = controller.reserve({ ...identity, ownerRevision: 0, operation }).ticket;
  controller.markSent(second);
  controller.setOwnerRevision(2);
  assert.equal(controller.cancelDefinitelyUnsent(second).error, "notProvenUnsent");
  assert.equal(controller.status().pending, 1);
});

test("invalid snapshot blocks sends but preserves locks; valid same-session snapshot restores readiness", () => {
  const controller = new EquipmentSessionController(fakeRuntime());
  controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline });
  controller.reserve({ ...identity, ownerRevision: 0, operation });
  assert.equal(controller.invalidateSnapshot(identity).error, "invalidSnapshot");
  assert.equal(controller.status().pending, 1);
  assert.equal(controller.reserve({ ...identity, ownerRevision: 0, operation }).error, "notReady");
  assert.equal(controller.observeSnapshot({ ...identity, complete: true, snapshot: baseline }).ok, true);
  assert.equal(controller.status().ready, true);
  assert.equal(controller.status().pending, 1);
});

test("socket close cannot be resumed by a snapshot or stale ACK; new confirmed session may use same or newer socket", () => {
  const controller = new EquipmentSessionController(fakeRuntime());
  controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline });
  controller.reserve({ ...identity, ownerRevision: 0, operation });
  controller.suspendConnection({ ...identity, reason: "socketClosed" });
  assert.equal(controller.status().pending, 1);
  assert.equal(controller.suspendConnection({ ...identity, reason: "logoutPending" }).error, "connectionSuspended");
  assert.equal(controller.resumeSameSession({ ...identity, logoutFailed: true }).error, "cannotResume");
  assert.equal(controller.observeSnapshot({ ...identity, complete: true, snapshot: baseline }).error, "connectionSuspended");
  assert.equal(controller.applyAck({ ...identity, operation, success: true }).error, "staleSession");
  const next = { connectionGeneration: 4, sessionGeneration: 6 };
  assert.equal(controller.establishBaseline({ ...next, startGameConfirmed: true, complete: true, snapshot: baseline }).ok, true);
  assert.equal(controller.applyAck({ ...identity, operation, success: true }).error, "staleSession");
  assert.equal(controller.status().pending, 0);
  controller.terminateSession(next);
  assert.equal(controller.observeSnapshot({ ...next, complete: true, snapshot: baseline }).error, "staleSession");
  assert.equal(controller.establishBaseline({ ...next, startGameConfirmed: true, complete: true, snapshot: baseline }).error, "invalidBaseline");
  assert.equal(controller.establishBaseline({ connectionGeneration: 4, sessionGeneration: 7, startGameConfirmed: true, complete: true, snapshot: baseline }).ok, true);
});

test("failed logout on the same live socket resumes its pending ledger without a new snapshot", () => {
  const controller = new EquipmentSessionController(fakeRuntime());
  controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline });
  controller.reserve({ ...identity, ownerRevision: 0, operation });
  controller.suspendConnection({ ...identity, reason: "logoutPending" });
  assert.equal(controller.reserve({ ...identity, ownerRevision: 0, operation }).error, "notReady");
  assert.equal(controller.resumeSameSession({ ...identity, logoutFailed: true }).ok, true);
  assert.equal(controller.status().ready, true);
  assert.equal(controller.status().pending, 1);
  assert.equal(controller.applyAck({ ...identity, operation, success: false }).matched, true);
});

test("missing additive WASM equipment capability fails closed without changing Quest runtime", () => {
  const controller = new EquipmentSessionController({ createEquipmentPendingLedger() { throw new Error("old bundle"); } });
  assert.equal(controller.establishBaseline({ ...identity, startGameConfirmed: true, complete: true, snapshot: baseline }).error, "ledgerUnavailable");
  assert.equal(controller.status().ready, false);
  assert.equal(controller.reserve({ ...identity, ownerRevision: 0, operation }).ok, false);
});
