import assert from "node:assert/strict";
import { test } from "node:test";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { normalizeStructTag } from "@mysten/sui/utils";
import { Transaction } from "@mysten/sui/transactions";
import { bcs } from "@mysten/sui/bcs";
import { PAYOUT_VERSION, canonicalJson, coinAtomicForMicroUsd, validatePayoutIntent, preparePayoutTransaction,
  validateSignedPayout, inspectPayoutResult, executeReviewedPayout, reconcilePayout } from "../lib/server/promotion-payout.mjs";

const key = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(31));
const address = digit => `0x${digit.repeat(64)}`;
const chain = "69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD";
const sui = normalizeStructTag("0x2::sui::SUI");
function intent(overrides = {}) { return {
  version: PAYOUT_VERSION, environment: "test", campaignId: "campaign", payoutId: "payout", settlementId: "settlement",
  settlementSha256: "a".repeat(64), network: "sui:testnet", chainIdentifier: chain, coinType: sui, coinDecimals: 9,
  amountMicroUsd: "10000", coinAmountAtomic: "10000000", quoteNumerator: "1000", quoteDenominator: "1",
  fxSnapshotRef: "quote-review-v1", destination: address("2"), addressVersion: "1", sender: key.toSuiAddress(),
  gasBudgetAtomic: "10000000", gasPriceAtomic: "1000", gasSource: "coinObjects", gasValidDuring: null, gasPayment: [{ objectId: address("3"), version: "1", digest: "11111111111111111111111111111111" }],
  payoutCoin: null, expiresAtEpoch: "999999", ...overrides,
}; }
async function envelope(value) { const prepared = await preparePayoutTransaction(value); return { ...prepared, signatures: [(await key.signTransaction(Buffer.from(prepared.transactionBytes, "base64"))).signature] }; }
function receipt(value, digest, { success = true, amount = value.coinAmountAtomic } = {}) {
  const tx = { digest, status: { success }, balanceChanges: [{ address: value.destination, coinType: value.coinType, amount }],
    effects: { transactionDigest: digest, status: { success }, gasUsed: { computationCost: "100", storageCost: "200", storageRebate: "50" } } };
  return success ? { $kind: "Transaction", Transaction: tx } : { $kind: "FailedTransaction", FailedTransaction: tx };
}
function harness(value, signed, { timeout = false, indexed = true, reserveFails = false } = {}) {
  let status, digest, sha, submitCalls = 0, queryCalls = 0, persisted = false;
  const observations = [], events = [];
  const journal = {
    async reserve(v) {
      events.push("reserve"); if (reserveFails) throw new Error("db unavailable");
      if (persisted) return { status, digest, intentSha256: sha, maySubmit: false };
      persisted = true; status = "reserved"; digest = v.digest; sha = v.intentSha256;
      return { status, digest, intentSha256: sha, maySubmit: true };
    },
    async markSubmitted(d) { events.push("markSubmitted"); assert.equal(persisted, true); assert.equal(d, signed.digest); status = "submitted"; },
    async recordOutcome(v) { events.push(`outcome:${v.status}`); if (!["confirmed", "failed"].includes(status)) status = v.status; observations.push(v); },
  };
  const client = { core: {
    async getChainIdentifier() { return { chainIdentifier: chain }; },
    async getCoinMetadata() { return { coinMetadata: { decimals: value.coinDecimals } }; },
    async getBalance() { return { balance: { addressBalance: "1000000000", coinBalance: "0", balance: "1000000000" } }; },
    async getObjects() { return { objects: [...value.gasPayment, ...(value.payoutCoin ? [value.payoutCoin] : [])].map((v, index) => ({ ...v, type: `0x2::coin::Coin<${index < value.gasPayment.length ? sui : value.coinType}>`, owner: { $kind: "AddressOwner", AddressOwner: value.sender }, content: bcs.struct("Coin", { id: bcs.Address, balance: bcs.u64() }).serialize({ id: v.objectId, balance: "1000000000" }).toBytes() })) }; },
    async executeTransaction(v) { events.push("execute"); submitCalls++; assert.equal(persisted, true); assert.equal(status, "submitted"); assert.equal(Buffer.from(v.transaction).toString("base64"), signed.transactionBytes); if (timeout) throw new Error("response lost"); return receipt(value, signed.digest); },
    async getTransaction(v) { events.push("query"); queryCalls++; assert.equal(v.digest, signed.digest); if (!indexed) throw new Error("not indexed"); return receipt(value, signed.digest); },
  } };
  return { client, journal, observations, events, counts: () => ({ submitCalls, queryCalls }), setIndexed: v => { indexed = v; } };
}
test("integer quote preserves precision above 2^53 and floors exactly once", () => {
  assert.equal(coinAtomicForMicroUsd("9007199254740993", "1000", "1000"), "9007199254740993");
  assert.equal(coinAtomicForMicroUsd("10001", "2", "3"), "6667");
  for (const v of [10000, "1e4", "1.0", "01", "-1", "0"]) assert.throws(() => coinAtomicForMicroUsd(v, "1", "1"));
  assert.throws(() => coinAtomicForMicroUsd("18446744073709551615", "2", "1"), /AmountInvalid/);
});
test("canonical review material has stable ordering and string integers", () => {
  assert.equal(canonicalJson({ z: "9007199254740993", a: { z: 1, a: 2 } }), '{"a":{"a":2,"z":1},"z":"9007199254740993"}');
  assert.throws(() => canonicalJson({ amount: 9007199254740992 }));
});
test("unknown, mainnet, quote, duplicate objects and authority fields fail closed", () => {
  for (const v of [intent({ actor: "admin" }), intent({ network: "sui:mainnet" }), intent({ coinAmountAtomic: "10000001" }),
    intent({ gasPayment: [] }), intent({ gasPayment: [...intent().gasPayment, ...intent().gasPayment] }), intent({ destination: key.toSuiAddress() }),
    intent({ coinDecimals: 19 }), intent({ coinType: "0x2::sui::SUI" })]) assert.throws(() => validatePayoutIntent(v));
  assert.throws(() => validatePayoutIntent(intent({ network: "sui:mainnet" }), { allowMainnet: true }));
});
test("fully frozen transaction builds offline, deterministically, with digest", async () => {
  const a = await preparePayoutTransaction(intent()), b = await preparePayoutTransaction(intent());
  assert.deepEqual(a, b); const data = Transaction.from(Buffer.from(a.transactionBytes, "base64")).getData();
  assert.equal(data.commands.length, 2); assert.equal(data.sender, key.toSuiAddress());
});
test("real Ed25519 transaction signature verifies without submitting", async () => {
  const i = intent(), e = await envelope(i); assert.equal((await validateSignedPayout(i, e)).digest, e.digest);
});
test("changed recipient/settlement/coin amount/gas cannot reuse signed envelope", async () => {
  const i = intent(), e = await envelope(i);
  for (const delta of [{ destination: address("4") }, { settlementSha256: "b".repeat(64) }, { gasBudgetAtomic: "10000001" },
    { coinAmountAtomic: "10001000", amountMicroUsd: "10001" }, { addressVersion: "2" }, { expiresAtEpoch: "1000000" }]) {
    await assert.rejects(validateSignedPayout({ ...i, ...delta }, e), /Mismatch/);
  }
});
test("extra onchain transfer is rejected even with a valid sender signature", async () => {
  const i = intent(), e = await envelope(i); const tx = Transaction.from(Buffer.from(e.transactionBytes, "base64"));
  tx.transferObjects([tx.gas], tx.pure.address(address("5"))); const bytes = await tx.build();
  const signature = (await key.signTransaction(bytes)).signature;
  await assert.rejects(validateSignedPayout(i, { ...e, transactionBytes: Buffer.from(bytes).toString("base64"), signatures: [signature] }), /Mismatch/);
});
test("signature by a different wallet fails", async () => {
  const i = intent(), e = await envelope(i); const other = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(41));
  e.signatures = [(await other.signTransaction(Buffer.from(e.transactionBytes, "base64"))).signature];
  await assert.rejects(validateSignedPayout(i, e), /SignatureInvalid/);
});
test("wallet-login personal-message signatures never authorize a payout transaction", async () => {
  const i=intent(),e=await envelope(i);
  e.signatures=[(await key.signPersonalMessage(Buffer.from(e.transactionBytes,"base64"))).signature];
  await assert.rejects(validateSignedPayout(i,e),/SignatureInvalid/);
});
test("persist and claim precede submission; final receipt must be read back", async () => {
  const i = intent(), e = await envelope(i), h = harness(i, e);
  const result = await executeReviewedPayout({ ...h, intent: i, envelope: e });
  assert.equal(result.status, "confirmed"); assert.equal(result.gasUsedAtomic, "250");
  assert.deepEqual(h.events, ["reserve", "markSubmitted", "execute", "query", "outcome:confirmed"]);
});
test("concurrent duplicate execution uses one signed digest and sends once", async () => {
  const i = intent(), e = await envelope(i), h = harness(i, e);
  await Promise.all(Array.from({ length: 6 }, () => executeReviewedPayout({ ...h, intent: i, envelope: e })));
  assert.equal(h.counts().submitCalls, 1);
});
test("database reservation failure performs no transfer", async () => {
  const i = intent(), e = await envelope(i), h = harness(i, e, { reserveFails: true });
  await assert.rejects(executeReviewedPayout({ ...h, intent: i, envelope: e }), /db unavailable/);
  assert.equal(h.counts().submitCalls, 0);
});
test("RPC timeout becomes unknown; rerun only queries same digest and never builds new transfer", async () => {
  const i = intent(), e = await envelope(i), h = harness(i, e, { timeout: true, indexed: false });
  assert.equal((await executeReviewedPayout({ ...h, intent: i, envelope: e })).status, "unknown");
  assert.equal((await executeReviewedPayout({ ...h, intent: i, envelope: e })).status, "unknown");
  h.setIndexed(true); assert.equal((await reconcilePayout({ ...h, intent: i, digest: e.digest })).status, "confirmed");
  assert.equal(h.counts().submitCalls, 1);
});
test("wrong chain and coin decimals block before persistence or transfer", async () => {
  for (const mismatch of ["chain", "decimals"]) {
    const i = intent(), e = await envelope(i), h = harness(i, e);
    if (mismatch === "chain") h.client.core.getChainIdentifier = async () => ({ chainIdentifier: "1".repeat(32) });
    else h.client.core.getCoinMetadata = async () => ({ coinMetadata: { decimals: 6 } });
    await assert.rejects(executeReviewedPayout({ ...h, intent: i, envelope: e }), /Mismatch/); assert.equal(h.events.length, 0);
  }
});
test("digest, effects, amount, third-party and gas inconsistencies never mark paid", async () => {
  const i = intent(), e = await envelope(i);
  const cases = [receipt(i, "wrong"), receipt(i, e.digest, { amount: "9999999" }), receipt(i, e.digest)];
  cases[2].Transaction.balanceChanges.push({ address: address("8"), coinType: sui, amount: "1" });
  const missing = receipt(i, e.digest); delete missing.Transaction.effects.gasUsed; cases.push(missing);
  for (const r of cases) assert.equal(inspectPayoutResult(i, e.digest, r).status, "unknown");
});
test("final chain failure records gas and never becomes payment success", async () => {
  const i = intent(), e = await envelope(i); const r = inspectPayoutResult(i, e.digest, receipt(i, e.digest, { success: false }));
  assert.equal(r.status, "failed"); assert.equal(r.gasUsedAtomic, "250");
});
test("wrong actual coin object type/owner/version/balance blocks send despite valid signed bytes", async () => {
  for (const field of ["type", "owner", "version", "balance"]) {
    const i = intent(), e = await envelope(i), h = harness(i, e), original = h.client.core.getObjects;
    h.client.core.getObjects = async () => {
      const r = await original(); const o = r.objects[0];
      if (field === "type") o.type = "0x9::fake::FAKE";
      if (field === "owner") o.owner.AddressOwner = address("9");
      if (field === "version") o.version = "2";
      if (field === "balance") o.content = bcs.struct("Coin", { id: bcs.Address, balance: bcs.u64() }).serialize({ id: o.objectId, balance: "1" }).toBytes();
      return r;
    };
    const r = await executeReviewedPayout({ ...h, intent: i, envelope: e });
    assert.equal(r.status, "unknown"); assert.equal(h.counts().submitCalls, 0);
  }
});
test("an unavailable later RPC cannot regress a previously persisted final state", async () => {
  const i=intent(),e=await envelope(i),h=harness(i,e,{indexed:false});
  h.journal.recordOutcome=async ()=>({status:"confirmed",digest:e.digest,intentSha256:e.intentSha256});
  assert.equal((await reconcilePayout({...h,intent:i,digest:e.digest})).status,"confirmed");
  assert.equal(h.counts().submitCalls,0);
});
test("Sui Testnet USDC six-decimal transfer uses a separately frozen coin object and exact USD units", async () => {
  const coinType="0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC";
  const i=intent({coinType,coinDecimals:6,amountMicroUsd:"50000000",coinAmountAtomic:"50000000",quoteNumerator:"1",quoteDenominator:"1",
    payoutCoin:{objectId:address("7"),version:"1",digest:"11111111111111111111111111111111"}});
  const e=await envelope(i),h=harness(i,e);const r=await executeReviewedPayout({...h,intent:i,envelope:e});
  assert.equal(r.status,"confirmed");assert.equal(r.coinAmountAtomic,"50000000");assert.equal(r.coinType,coinType);
  assert.equal(h.counts().submitCalls,1);
});
test("Testnet non-SUI coin payout can freeze address-balance gas without generating another transaction", async () => {
  const coinType="0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC";
  const i=intent({coinType,coinDecimals:6,amountMicroUsd:"10000",coinAmountAtomic:"10000",quoteNumerator:"1",quoteDenominator:"1",gasSource:"addressBalance",gasValidDuring:{minEpoch:"998",nonce:2345678901},expiresAtEpoch:"999",gasPayment:[],
    payoutCoin:{objectId:address("7"),version:"1",digest:"11111111111111111111111111111111"}});
  const e=await envelope(i),h=harness(i,e),data=Transaction.from(Buffer.from(e.transactionBytes,"base64")).getData();
  assert.deepEqual(data.gasData.payment,[]);assert.equal(data.commands.length,2);
  assert.equal(data.expiration.ValidDuring.minEpoch,"998");assert.equal(data.expiration.ValidDuring.maxEpoch,"999");
  assert.equal(data.expiration.ValidDuring.nonce,2345678901);assert.equal(data.expiration.ValidDuring.chain,i.chainIdentifier);
  for(const change of [{gasValidDuring:null},{gasValidDuring:{minEpoch:"996",nonce:1}},{gasValidDuring:{minEpoch:"998",nonce:-1}}]) {
    assert.throws(()=>validatePayoutIntent({...i,...change}));
  }
  const changedExpiry={...i,gasValidDuring:{...i.gasValidDuring,nonce:2345678902}};
  await assert.rejects(()=>validateSignedPayout(changedExpiry,e),/promotionPayoutSignedIntentMismatch/);
  assert.equal((await executeReviewedPayout({...h,intent:i,envelope:e})).status,"confirmed");
  const low=harness(i,e);low.client.core.getBalance=async()=>({balance:{addressBalance:"1",coinBalance:"1000000000",balance:"1000000001"}});
  assert.equal((await executeReviewedPayout({...low,intent:i,envelope:e})).code,"promotionPayoutFundingInsufficient");
  assert.equal(low.counts().submitCalls,0);
});
