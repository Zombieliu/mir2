// Behavioral tests for lib/onchain-mine.ts — the client core of the on-chain
// smart-mine loop (M4, WF-6).
//
// Two layers:
//   1. Pure logic (swing batcher, nonce tracker, ore reconcile, stones->stage,
//      ore-kind constructor targets) — no external deps.
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

const mine = loadTypeScriptModule(new URL("../lib/onchain-mine.ts", import.meta.url), {
  "@mysten/sui/transactions": suiTransactions,
});

const {
  TESTNET_MINE_DEPLOYMENT,
  ORE_KINDS,
  isOreKindName,
  oreKindConstructorTarget,
  buildMineBatchTransaction,
  buildRedeemTransaction,
  createSwingBatcher,
  createNonceTracker,
  reconcileOptimisticOre,
  stonesLeftToVeinStage,
  VEIN_STAGE_FULL,
  VEIN_STAGE_CRACKED,
  VEIN_STAGE_DEPLETED,
} = mine;

const DEPLOYMENT = TESTNET_MINE_DEPLOYMENT;

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
    `${DEPLOYMENT.packageId}::mir2_mine_ore_kind::new_black_iron`,
  );
  assert.equal(
    oreKindConstructorTarget(DEPLOYMENT, "Gold"),
    `${DEPLOYMENT.packageId}::mir2_mine_ore_kind::new_gold`,
  );
});

// ---------------------------------------------------------------------------
// mine_batch PTB
// ---------------------------------------------------------------------------

check("buildMineBatchTransaction splits the fee and calls mine_batch with 7 args", () => {
  const tx = buildMineBatchTransaction(DEPLOYMENT, {
    mineId: 1,
    swings: 5,
    nonce: 3,
    feeMist: 1000,
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
  // schema, mine_id, swings, nonce, fee, random, clock (ctx is injected on-chain).
  assert.equal(call.arguments.length, 7, "mine_batch takes 7 explicit args");
});

check("buildMineBatchTransaction rejects empty / negative-fee batches", () => {
  assert.throws(
    () => buildMineBatchTransaction(DEPLOYMENT, { mineId: 1, swings: 0, nonce: 1, feeMist: 0 }),
    /swings >= 1/,
  );
  assert.throws(
    () => buildMineBatchTransaction(DEPLOYMENT, { mineId: 1, swings: 1, nonce: 1, feeMist: -1 }),
    /fee must be >= 0/,
  );
});

check("buildMineBatchTransaction accepts bigint params (no precision loss)", () => {
  const tx = buildMineBatchTransaction(DEPLOYMENT, {
    mineId: 1n,
    swings: 10n,
    nonce: 9_007_199_254_740_993n, // > Number.MAX_SAFE_INTEGER
    feeMist: 5_000n,
  });
  assert.equal(moveCalls(tx)[0].function, "mine_batch");
});

// ---------------------------------------------------------------------------
// redeem PTB
// ---------------------------------------------------------------------------

check("buildRedeemTransaction constructs the OreKind then burns it", () => {
  const tx = buildRedeemTransaction(DEPLOYMENT, { oreKind: "BlackIron", amount: 4 });
  const calls = moveCalls(tx);
  assert.equal(calls.length, 2, "OreKind constructor + redeem");
  assert.equal(calls[0].function, "new_black_iron");
  assert.equal(calls[0].module, "mir2_mine_ore_kind");
  assert.equal(calls[1].module, "redeem_system");
  assert.equal(calls[1].function, "redeem");
  // schema, ore_kind (the constructor result), amount.
  assert.equal(calls[1].arguments.length, 3);
});

check("buildRedeemTransaction rejects non-positive amounts", () => {
  assert.throws(() => buildRedeemTransaction(DEPLOYMENT, { oreKind: "Gold", amount: 0 }), /amount >= 1/);
});

// ---------------------------------------------------------------------------
// Swing batcher
// ---------------------------------------------------------------------------

check("createSwingBatcher flushes at batchSize and resets on takeBatch", () => {
  const batcher = createSwingBatcher({ batchSize: 3 });
  assert.equal(batcher.recordSwing().shouldFlush, false); // 1
  assert.equal(batcher.recordSwing().shouldFlush, false); // 2
  const third = batcher.recordSwing(); // 3
  assert.equal(third.pending, 3);
  assert.equal(third.shouldFlush, true);
  assert.equal(batcher.takeBatch(), 3, "takeBatch returns the accumulated count");
  assert.equal(batcher.pending(), 0, "pending resets after takeBatch");
  assert.equal(batcher.recordSwing().shouldFlush, false, "the cycle restarts");
});

check("createSwingBatcher coerces a bad batchSize to >= 1 and reset drops pending", () => {
  const batcher = createSwingBatcher({ batchSize: 0 });
  assert.equal(batcher.recordSwing().shouldFlush, true, "batchSize floored to 1");
  batcher.recordSwing();
  batcher.reset();
  assert.equal(batcher.pending(), 0);
  assert.equal(batcher.takeBatch(), 0, "takeBatch on empty is a no-op count");
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

check("stonesLeftToVeinStage tiers full / cracked / depleted", () => {
  assert.equal(stonesLeftToVeinStage(0, 10), VEIN_STAGE_DEPLETED);
  assert.equal(stonesLeftToVeinStage(5, 10), VEIN_STAGE_CRACKED, "<= half is cracked");
  assert.equal(stonesLeftToVeinStage(6, 10), VEIN_STAGE_FULL, "> half is full");
  assert.equal(stonesLeftToVeinStage(10, 10), VEIN_STAGE_FULL);
  assert.equal(stonesLeftToVeinStage(1, 10), VEIN_STAGE_CRACKED);
});

console.log(`onchain mine client tests passed (${passed} groups)`);
