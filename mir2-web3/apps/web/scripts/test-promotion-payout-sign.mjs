import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, mkdir, readFile, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import vm from "node:vm";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import { normalizeStructTag } from "@mysten/sui/utils";
import { preparePayoutTransaction } from "../lib/server/promotion-payout.mjs";
import { HANDOFF_ORIGIN, WALLET_ORIGIN, validateHandoffScope, parseBridgeResponse, validateHandoffSignature,
  authorizeHandoffPost, validateOutputPath, saveHandoffSignature, renderHandoffPage } from "./promotion-payout-sign.mjs";

// Synthetic keys are local unit fixtures; no mobile wallet or chain acceptance.
const key = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(73));
const address = digit => `0x${digit.repeat(64)}`;
function intent(overrides = {}) { return {
  version: "numeron-sui-payout-v1", environment: "test", campaignId: "technical-test", payoutId: "technical-payout", settlementId: "technical-settlement",
  settlementSha256: "a".repeat(64), network: "sui:testnet", chainIdentifier: "69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD",
  coinType: normalizeStructTag("0xa::usdc::USDC"), coinDecimals: 6, amountMicroUsd: "10000", coinAmountAtomic: "10000",
  quoteNumerator: "1", quoteDenominator: "1", fxSnapshotRef: "synthetic-offline-test-only", destination: address("2"), addressVersion: "1",
  sender: key.toSuiAddress(), gasBudgetAtomic: "10000000", gasPriceAtomic: "1000", gasSource: "addressBalance",
  gasValidDuring: { minEpoch: "1266", nonce: 123 }, gasPayment: [], payoutCoin: { objectId: address("3"), version: "1", digest: "11111111111111111111111111111111" },
  expiresAtEpoch: "1267", ...overrides,
}; }
async function fixture() {
  const value = intent(), prepared = await preparePayoutTransaction(value), unsigned = { ...prepared, signatures: [] };
  const pin = { expectedSender: value.sender, expectedIntentSha256: prepared.intentSha256 };
  return { value, unsigned, pin, scope: await validateHandoffScope(value, unsigned, pin) };
}
async function signed(scope, signer = key) {
  return { ...scope.unsigned, signatures: [(await signer.signTransaction(Buffer.from(scope.unsigned.transactionBytes, "base64"))).signature] };
}

test("handoff scope requires explicit exact sender/hash and frozen unsigned bytes", async () => {
  const { value, unsigned, pin } = await fixture();
  assert.equal((await validateHandoffScope(value, unsigned, pin)).unsigned.transactionBytes, unsigned.transactionBytes);
  for (const missing of [undefined, {}, { expectedSender: value.sender }, { expectedIntentSha256: pin.expectedIntentSha256 }]) await assert.rejects(validateHandoffScope(value, unsigned, missing), /ExplicitScopeRequired/);
  await assert.rejects(validateHandoffScope(value, unsigned, { ...pin, expectedSender: address("4") }), /ScopeInvalid/);
  await assert.rejects(validateHandoffScope(value, unsigned, { ...pin, expectedIntentSha256: "b".repeat(64) }), /FrozenIntentMismatch/);
  for (const delta of [{ transactionBytes: unsigned.transactionBytes + "AA==" }, { digest: "1".repeat(32) }, { intentSha256: "c".repeat(64) }, { signatures: ["already-signed"] }]) {
    await assert.rejects(validateHandoffScope(value, { ...unsigned, ...delta }, pin));
  }
});

test("mainnet, different asset/amount, and gas above 0.01 SUI are refused", async () => {
  const { value, unsigned, pin } = await fixture();
  for (const delta of [{ environment: "production" }, { network: "sui:mainnet" }, { coinDecimals: 9 }, { coinType: normalizeStructTag("0x2::sui::SUI") },
    { amountMicroUsd: "10001" }, { coinAmountAtomic: "10001" }, { quoteNumerator: "2" }, { gasBudgetAtomic: "10000001" }]) {
    await assert.rejects(validateHandoffScope({ ...value, ...delta }, unsigned, pin), /ScopeInvalid|GasCapExceeded/);
  }
});

test("bridge requires exact trusted origin, original popup, request id and sign-only schema", async () => {
  const { scope } = await fixture(), popup = {}, context = { popup, id: "unit-request", sender: scope.intent.sender, transactionBytes: scope.unsigned.transactionBytes };
  const envelope = await signed(scope);
  const event = { origin: WALLET_ORIGIN, source: popup, data: { id: context.id, version: 1, source: "dubhe-wallet", method: "signTransaction", ok: true,
    result: { account: { address: scope.intent.sender, chains: ["sui:testnet"], publicKey: key.getPublicKey().toBase64() }, bytes: scope.unsigned.transactionBytes, signature: envelope.signatures[0] } } };
  assert.equal(parseBridgeResponse(event, context).signature, envelope.signatures[0]);
  for (const delta of [{ origin: "https://wallet.dubhe.obelisk.build.attacker.invalid" }, { origin: HANDOFF_ORIGIN }, { source: {} }, { data: { ...event.data, id: "another-request" } }]) assert.equal(parseBridgeResponse({ ...event, ...delta }, context), null);
  for (const delta of [{ method: "signAndExecuteTransaction" }, { version: 2 }, { source: "dubhe-dapp" }, { actor: "admin" }]) assert.throws(() => parseBridgeResponse({ ...event, data: { ...event.data, ...delta } }, context), /ResponseInvalid/);
  for (const result of [{ ...event.data.result, bytes: "changed" }, { ...event.data.result, account: { address: address("4"), chains: ["sui:testnet"] } },
    { ...event.data.result, account: { address: scope.intent.sender, chains: ["sui:mainnet"] } }, { ...event.data.result, role: "finance" }]) {
    assert.throws(() => parseBridgeResponse({ ...event, data: { ...event.data, result } }, context), /Mismatch|ResponseInvalid/);
  }
});

test("local verification accepts the frozen transaction signer and rejects login signatures and changed bytes", async () => {
  const { scope } = await fixture(), envelope = await signed(scope);
  assert.equal((await validateHandoffSignature(scope, envelope)).digest, envelope.digest);
  const other = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(74));
  await assert.rejects(validateHandoffSignature(scope, await signed(scope, other)), /SignatureInvalid/);
  const personal = await key.signPersonalMessage(Buffer.from(envelope.transactionBytes, "base64"));
  await assert.rejects(validateHandoffSignature(scope, { ...envelope, signatures: [personal.signature] }), /SignatureInvalid/);
  await assert.rejects(validateHandoffSignature(scope, { ...envelope, transactionBytes: envelope.transactionBytes + "AA==" }), /Mismatch/);
  await assert.rejects(validateHandoffSignature(scope, { ...envelope, actor: "admin" }), /FieldsInvalid/);
});

test("POST authorization rejects foreign origin, Host, CSRF, extra route and malformed Unicode", () => {
  const csrf = "a".repeat(43), request = { method: "POST", url: "/signed", headers: { host: "127.0.0.1:7373", origin: HANDOFF_ORIGIN, "content-type": "application/json", "x-payout-csrf": csrf } };
  assert.equal(authorizeHandoffPost(request, HANDOFF_ORIGIN, csrf), true);
  for (const headers of [{ origin: "https://attacker.invalid" }, { host: "attacker.invalid:7373" }, { "x-payout-csrf": "wrong" }, { "x-payout-csrf": "é".repeat(43) }, { "content-type": "text/plain" }]) assert.equal(authorizeHandoffPost({ ...request, headers: { ...request.headers, ...headers } }, HANDOFF_ORIGIN, csrf), false);
  for (const delta of [{ method: "GET" }, { url: "/signed?out=attacker" }, { url: "/execute" }]) assert.equal(authorizeHandoffPost({ ...request, ...delta }, HANDOFF_ORIGIN, csrf), false);
});

test("signed output is a new private-directory file and cannot overwrite existing evidence", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "promotion-sign-unit-"));
  try {
    const privateDirectory = join(temporary, "private"); await mkdir(privateDirectory);
    const out = join(privateDirectory, "signed.json"), { scope } = await fixture(), envelope = await signed(scope);
    await assert.rejects(validateOutputPath("relative.json"), /AbsoluteOutputRequired/);
    await assert.rejects(validateOutputPath(join(temporary, "wrong.json")), /PrivateDirectoryRequired/);
    const result = await saveHandoffSignature(scope, envelope, out);
    assert.equal(result.transactionSubmitted, false); assert.equal(result.chainPaymentAccepted, false);
    assert.deepEqual(JSON.parse(await readFile(out, "utf8")), envelope);
    await assert.rejects(saveHandoffSignature(scope, envelope, out), /OutputAlreadyExists/);
    assert.deepEqual(JSON.parse(await readFile(out, "utf8")), envelope);
  } finally {
    const target = await realpath(temporary), allowedRoot = await realpath(tmpdir());
    assert.equal(dirname(target), allowedRoot); assert.ok(basename(target).startsWith("promotion-sign-unit-"));
    await rm(target, { recursive: true, force: true });
  }
});

test("human page displays all frozen review details and contains only a signTransaction request", async () => {
  const { scope } = await fixture(), html = renderHandoffPage(scope, "unit-csrf", "unit-nonce");
  for (const value of [scope.intent.sender, scope.intent.destination, scope.unsigned.intentSha256, scope.unsigned.digest, scope.intent.gasSource, "0.01 test USDC", "Technical Testnet fixture", "0.01 SUI"]) assert.ok(html.includes(value), value);
  assert.match(html, /method:'signTransaction'/); assert.doesNotMatch(html, /method:'signAndExecuteTransaction'|executeTransaction\(|PAYOUT_WORKER|SECRET_KEY/);
  const script = html.match(/<script nonce="unit-nonce">([\s\S]*)<\/script>/)?.[1];
  assert.ok(script); new vm.Script(script); // Syntax only. No wallet popup/request.
});
