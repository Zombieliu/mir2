// Behavioral tests for the on-chain smart-mine client core (M4, WF-6):
// lib/onchain-mine-config.ts (pure config/helpers, re-exported by lib/onchain-mine.ts),
// lib/onchain-mine.ts (PTB builders) and lib/onchain-mine-state.ts (page-state machine).
//
// Two layers:
//   1. Pure logic (page-state machine, nonce tracker, ore reconcile, stones->stage,
//      ore-kind constructor targets, nonce persistence) — no external deps.
//   2. PTB builders (mine_batch / redeem) — asserted by inspecting the built
//      `Transaction.getData()` (commands + inputs), no network/wallet needed.
//
// The .ts source is transpiled in-memory via the `typescript` devDependency.
// `@mysten/sui/transactions` is injected into the module's require map from a real
// dynamic import, so the builders run against the actual Transaction implementation.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const suiTransactions = await import("@mysten/sui/transactions");

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

const mineConfig = loadTypeScriptModule(
  new URL("../lib/onchain-mine-config.ts", import.meta.url),
);
const mine = loadTypeScriptModule(new URL("../lib/onchain-mine.ts", import.meta.url), {
  "@mysten/sui/transactions": suiTransactions,
  "./onchain-mine-config": mineConfig,
});
const mineState = loadTypeScriptModule(new URL("../lib/onchain-mine-state.ts", import.meta.url));

const {
  TESTNET_MINE_DEPLOYMENT,
  ORE_KINDS,
  isOreKindName,
  oreKindConstructorTarget,
  buildMineBatchTransaction,
  buildRedeemTransaction,
  createNonceTracker,
  reconcileOptimisticOre,
  stonesLeftToVeinStage,
  VEIN_STAGE_FULL,
  VEIN_STAGE_CRACKED,
  VEIN_STAGE_DEPLETED,
} = mine;

// Fixed fixture — TESTNET_MINE_DEPLOYMENT ids are env-driven (empty in tests). Valid
// 32-byte hex ids (the PTB builder validates object/address ids).
const DEPLOYMENT = {
  packageId: `0x${"11".repeat(32)}`,
  frameworkPackageId: `0x${"22".repeat(32)}`,
  dappHubId: `0x${"33".repeat(32)}`,
  dappStorageId: `0x${"44".repeat(32)}`,
  randomId: "0x8",
  clockId: "0x6",
};
const USER_STORAGE = `0x${"55".repeat(32)}`;
void TESTNET_MINE_DEPLOYMENT;

let passed = 0;
function check(label, fn) {
  fn();
  passed += 1;
}

// Pull the MoveCall commands out of a built transaction's data snapshot.
function moveCalls(tx) {
  return tx
    .getData()
    .commands.filter((command) => command.MoveCall)
    .map((command) => command.MoveCall);
}

// ---------------------------------------------------------------------------
// Ore kinds
// ---------------------------------------------------------------------------

check("ORE_KINDS matches the on-chain enum (8 variants)", () => {
  assert.deepEqual(ORE_KINDS, [
    "Amethyst",
    "BlackIron",
    "Copper",
    "Gold",
    "Nephrite",
    "Platinum",
    "Ruby",
    "Silver",
  ]);
});

check("isOreKindName guards relayer/sim ore strings", () => {
  assert.equal(isOreKindName("BlackIron"), true);
  assert.equal(isOreKindName("Silver"), true);
  assert.equal(isOreKindName("Mithril"), false);
  assert.equal(isOreKindName("blackiron"), false, "case-sensitive (matches Move variant)");
});

check("oreKindConstructorTarget builds the new_* call target", () => {
  assert.equal(
    oreKindConstructorTarget(DEPLOYMENT, "BlackIron"),
    `${DEPLOYMENT.packageId}::ore_kind::new_blackiron`,
  );
  assert.equal(
    oreKindConstructorTarget(DEPLOYMENT, "Gold"),
    `${DEPLOYMENT.packageId}::ore_kind::new_gold`,
  );
});

// ---------------------------------------------------------------------------
// mine_batch PTB
// ---------------------------------------------------------------------------

check("buildMineBatchTransaction splits the fee and calls mine_batch with 8 args", () => {
  const tx = buildMineBatchTransaction(DEPLOYMENT, {
    mineId: 1,
    swings: 5,
    nonce: 3,
    feeMist: 1000,
    userStorageId: USER_STORAGE,
  });
  const commands = tx.getData().commands;
  // First command splits the fee from gas; then the mine_batch move call.
  assert.ok(
    commands.some((command) => command.SplitCoins),
    "fee is split from the gas coin",
  );
  const calls = moveCalls(tx);
  assert.equal(calls.length, 1);
  const call = calls[0];
  assert.equal(call.package, DEPLOYMENT.packageId);
  assert.equal(call.module, "mine_system");
  assert.equal(call.function, "mine_batch");
  // dapp_storage, user_storage, mine_id, swings, nonce, fee, random, clock.
  assert.equal(call.arguments.length, 8, "mine_batch takes 8 explicit args");
});

check("buildMineBatchTransaction rejects empty / negative-fee batches", () => {
  assert.throws(
    () => buildMineBatchTransaction(DEPLOYMENT, { mineId: 1, swings: 0, nonce: 1, feeMist: 0, userStorageId: USER_STORAGE }),
    /swings >= 1/,
  );
  assert.throws(
    () => buildMineBatchTransaction(DEPLOYMENT, { mineId: 1, swings: 1, nonce: 1, feeMist: -1, userStorageId: USER_STORAGE }),
    /fee must be >= 0/,
  );
});

check("buildMineBatchTransaction accepts bigint params (no precision loss)", () => {
  const tx = buildMineBatchTransaction(DEPLOYMENT, {
    mineId: 1n,
    swings: 10n,
    nonce: 9_007_199_254_740_993n, // > Number.MAX_SAFE_INTEGER
    feeMist: 5_000n,
    userStorageId: USER_STORAGE,
  });
  assert.equal(moveCalls(tx)[0].function, "mine_batch");
});

// ---------------------------------------------------------------------------
// redeem PTB
// ---------------------------------------------------------------------------

check("buildRedeemTransaction constructs the OreKind then burns it", () => {
  const tx = buildRedeemTransaction(DEPLOYMENT, { oreKind: "BlackIron", amount: 4, userStorageId: USER_STORAGE });
  const calls = moveCalls(tx);
  assert.equal(calls.length, 2, "OreKind constructor + redeem");
  assert.equal(calls[0].function, "new_blackiron");
  assert.equal(calls[0].module, "ore_kind");
  assert.equal(calls[1].module, "redeem_system");
  assert.equal(calls[1].function, "redeem");
  // dapp_storage, user_storage, ore_kind (constructor result), amount.
  assert.equal(calls[1].arguments.length, 4);
});

check("buildRedeemTransaction rejects non-positive amounts", () => {
  assert.throws(() => buildRedeemTransaction(DEPLOYMENT, { oreKind: "Gold", amount: 0, userStorageId: USER_STORAGE }), /amount >= 1/);
});

// ---------------------------------------------------------------------------
// Nonce tracker
// ---------------------------------------------------------------------------

check("createNonceTracker yields strictly increasing nonces from 1", () => {
  const tracker = createNonceTracker();
  assert.equal(tracker.peek(), 1);
  assert.equal(tracker.next(), 1);
  assert.equal(tracker.next(), 2);
  assert.equal(tracker.peek(), 3, "peek does not consume");
});

check("createNonceTracker.syncFromChain advances but never rewinds", () => {
  const tracker = createNonceTracker();
  tracker.syncFromChain(7); // chain last-accepted nonce 7 -> next is 8
  assert.equal(tracker.peek(), 8);
  tracker.syncFromChain(2); // stale/lower confirmation must not rewind
  assert.equal(tracker.peek(), 8);
  assert.equal(tracker.next(), 8);
});

// ---------------------------------------------------------------------------
// Reconcile
// ---------------------------------------------------------------------------

check("reconcileOptimisticOre reports phantom / shortfall / match", () => {
  assert.deepEqual(reconcileOptimisticOre({ optimisticUnits: 3, confirmedUnits: 3 }), {
    deltaUnits: 0,
    phantom: false,
    shortfall: false,
  });
  // Showed 5 but chain granted 3 -> 2 phantom ore to remove.
  assert.deepEqual(reconcileOptimisticOre({ optimisticUnits: 5, confirmedUnits: 3 }), {
    deltaUnits: -2,
    phantom: true,
    shortfall: false,
  });
  // Showed 1 but chain granted 4 -> 3 short to add.
  assert.deepEqual(reconcileOptimisticOre({ optimisticUnits: 1, confirmedUnits: 4 }), {
    deltaUnits: 3,
    phantom: false,
    shortfall: true,
  });
});

// ---------------------------------------------------------------------------
// stones_left -> vein render stage
// ---------------------------------------------------------------------------

check("stonesLeftToVeinStage tiers match the sim's mine_stage exactly", () => {
  assert.equal(stonesLeftToVeinStage(0, 10), VEIN_STAGE_DEPLETED);
  // P0 semantics (mining.rs mine_stage): cracked only while stones*2 < max —
  // exactly half still renders FULL.
  assert.equal(stonesLeftToVeinStage(4, 10), VEIN_STAGE_CRACKED, "below half is cracked");
  assert.equal(stonesLeftToVeinStage(5, 10), VEIN_STAGE_FULL, "exactly half is full (P0 parity)");
  assert.equal(stonesLeftToVeinStage(6, 10), VEIN_STAGE_FULL);
  assert.equal(stonesLeftToVeinStage(10, 10), VEIN_STAGE_FULL);
  assert.equal(stonesLeftToVeinStage(1, 10), VEIN_STAGE_CRACKED);
});

// ---------------------------------------------------------------------------
// Page-state machine (lib/onchain-mine-state.ts)
// ---------------------------------------------------------------------------

const {
  abandonInFlightBatch,
  createOnchainMineState,
  hasBatchInFlight,
  canFlushBatch,
  recordSwing,
  beginBatch,
  batchSubmitted,
  batchFailed,
  confirmSettlement,
  resetPendingSwings,
  oreUnitsFromGainedItem,
  isOnchainVeinNode,
  loadPersistedNextNonce,
  persistNextNonce,
} = mineState;

check("swing accumulation flushes at batch size, single batch in flight", () => {
  let state = createOnchainMineState();
  for (let i = 0; i < 5; i += 1) state = recordSwing(state, 1);
  assert.equal(state.pendingSwings, 5);
  assert.equal(state.pendingOptimisticUnits, 5);
  assert.equal(canFlushBatch(state, 5), true);
  assert.equal(canFlushBatch(state, 6), false);

  const begun = beginBatch(state, 3);
  assert.ok(begun);
  assert.equal(begun.swings, 5);
  state = begun.state;
  assert.equal(state.pendingSwings, 0, "pending moved into the in-flight batch");
  assert.equal(state.inFlightSwings, 5);
  assert.equal(state.inFlightNonce, 3);
  assert.equal(hasBatchInFlight(state), true);
  assert.equal(canFlushBatch(recordSwing(state, 1), 1), false, "no second batch while in flight");
  assert.equal(beginBatch(state, 4), null, "cannot begin while one is in flight");

  state = batchSubmitted(state, "DIGEST1");
  assert.equal(state.inFlightDigest, "DIGEST1");
});

check("batchFailed restores the swings so the player's work is not lost", () => {
  let state = createOnchainMineState();
  state = recordSwing(recordSwing(state, 1), 1);
  state = beginBatch(state, 1).state;
  state = recordSwing(state, 1); // one more swing accumulated while in flight
  state = batchFailed(state, "wallet rejected");
  assert.equal(state.pendingSwings, 3, "2 in-flight + 1 new pending restored");
  assert.equal(state.inFlightSwings, 0);
  assert.equal(state.lastError, "wallet rejected");
  assert.equal(canFlushBatch(state, 3), true);
});

check("confirmSettlement reconciles optimistic vs chain-confirmed (多退少补)", () => {
  let state = createOnchainMineState();
  for (let i = 0; i < 4; i += 1) state = recordSwing(state, 1); // optimistic 4
  state = beginBatch(state, 1).state;
  state = batchSubmitted(state, "D");
  // Chain granted only 3 -> 1 phantom ore was shown.
  state = confirmSettlement(state, 3);
  assert.equal(hasBatchInFlight(state), false);
  assert.equal(state.confirmedUnits, 3);
  assert.equal(state.settledBatches, 1);
  assert.deepEqual(state.lastReconcile, { deltaUnits: -1, phantom: true, shortfall: false });
  // Settlement with unreadable units: settles, no reconcile delta claimed.
  state = recordSwing(state, 1);
  state = beginBatch(state, 2).state;
  const before = state.lastReconcile;
  state = confirmSettlement(state, null);
  assert.equal(state.settledBatches, 2);
  assert.equal(state.lastReconcile, before, "unknown units never claim a delta");
  // Settling with nothing in flight is a no-op.
  assert.equal(confirmSettlement(state, 5), state);
});

check("resetPendingSwings drops accumulation only", () => {
  let state = recordSwing(createOnchainMineState(), 1);
  state = resetPendingSwings(state);
  assert.equal(state.pendingSwings, 0);
  assert.equal(state.pendingOptimisticUnits, 0);
});

check("abandonInFlightBatch drops the batch WITHOUT restoring swings (nonce is spent)", () => {
  let state = recordSwing(recordSwing(createOnchainMineState(), 1), 1);
  state = beginBatch(state, 1).state;
  state = batchSubmitted(state, "DIGEST");
  state = abandonInFlightBatch(state, "settlement timeout");
  assert.equal(hasBatchInFlight(state), false);
  assert.equal(state.pendingSwings, 0, "swings are NOT restored — resubmit would double-mine");
  assert.equal(state.lastError, "settlement timeout");
  // No-op when nothing is in flight.
  assert.equal(abandonInFlightBatch(state, "x"), state);
});

check("oreUnitsFromGainedItem reads fresh chain-grant items only", () => {
  // Fresh chain grant: count 1, dura == maxDura == units*1000.
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 5000, maxDura: 5000 }), 5);
  // Incremented stack / partial dura / non-1000 multiples are unreadable.
  assert.equal(oreUnitsFromGainedItem({ count: 2, currentDura: 5000, maxDura: 5000 }), null);
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 4000, maxDura: 5000 }), null);
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 4500, maxDura: 4500 }), null);
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 0, maxDura: 0 }), null);
  // The plausibility bound rejects unrelated fresh items with round durabilities
  // (a 20000-dura shop weapon is NOT a 20-ore grant when the batch was 5 swings).
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 20000, maxDura: 20000 }, 5), null);
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 5000, maxDura: 5000 }, 5), 5);
  assert.equal(oreUnitsFromGainedItem({ count: 1, currentDura: 5000, maxDura: 5000 }, 4), null);
});

check("isOnchainVeinNode matches the configured cell only", () => {
  assert.equal(isOnchainVeinNode({ x: 335, y: 270 }, { x: 335, y: 270 }), true);
  assert.equal(isOnchainVeinNode({ x: 334, y: 270 }, { x: 335, y: 270 }), false);
});

check("nonce persistence round-trips per account+package and defaults to 1", () => {
  const store = new Map();
  const storage = {
    getItem: (key) => (store.has(key) ? store.get(key) : null),
    setItem: (key, value) => store.set(key, value),
  };
  assert.equal(loadPersistedNextNonce("sui:0xa", "0xpkg", storage), 1);
  persistNextNonce("sui:0xa", "0xpkg", 7, storage);
  assert.equal(loadPersistedNextNonce("sui:0xa", "0xpkg", storage), 7);
  assert.equal(
    loadPersistedNextNonce("sui:0xb", "0xpkg", storage),
    1,
    "scoped per account",
  );
  persistNextNonce("sui:0xa", "0xpkg", 0, storage);
  assert.equal(loadPersistedNextNonce("sui:0xa", "0xpkg", storage), 1, "floors to 1");
  assert.equal(loadPersistedNextNonce("sui:0xa", "0xpkg", null), 1, "no storage -> 1");
});

console.log(`onchain mine client tests passed (${passed} groups)`);
