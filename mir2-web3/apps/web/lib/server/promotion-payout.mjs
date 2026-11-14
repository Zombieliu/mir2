// Server/operations only. This module never accepts or loads a private key.
import { createHash, timingSafeEqual } from "node:crypto";
import { Transaction, TransactionDataBuilder } from "@mysten/sui/transactions";
import { verifyTransactionSignature } from "@mysten/sui/verify";
import { normalizeStructTag } from "@mysten/sui/utils";
import { bcs } from "@mysten/sui/bcs";

export const PAYOUT_VERSION = "numeron-sui-payout-v1";
const U64_MAX = (1n << 64n) - 1n;
const ID = /^[A-Za-z0-9_.:@-]{1,128}$/;
const ADDRESS = /^0x[0-9a-f]{64}$/;
const HASH = /^[0-9a-f]{64}$/;
const BASE58 = /^[1-9A-HJ-NP-Za-km-z]{32,64}$/;
const SUI = normalizeStructTag("0x2::sui::SUI");
const Coin = bcs.struct("PromotionCoin", { id: bcs.Address, balance: bcs.u64() });
function fail(code) { throw new Error(code); }
function integer(value, { zero = false, max = U64_MAX } = {}) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]{0,38})$/.test(value)) fail("promotionPayoutIntegerRequired");
  const n = BigInt(value);
  if ((!zero && n === 0n) || n > max) fail("promotionPayoutAmountInvalid");
  return n;
}
function exactKeys(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).sort().join("|") !== [...keys].sort().join("|")) fail("promotionPayoutFieldsInvalid");
}
export function canonicalJson(value) {
  if (value === null || typeof value === "string" || typeof value === "boolean") return JSON.stringify(value);
  if (typeof value === "number" && Number.isSafeInteger(value)) return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") return `{${Object.keys(value).sort().map(k => `${JSON.stringify(k)}:${canonicalJson(value[k])}`).join(",")}}`;
  fail("promotionPayoutCanonicalValueInvalid");
}
export function sha256(value) { return createHash("sha256").update(value).digest("hex"); }
function ref(value) {
  exactKeys(value, ["objectId", "version", "digest"]);
  if (!ADDRESS.test(value.objectId) || !BASE58.test(value.digest)) fail("promotionPayoutObjectInvalid");
  integer(value.version); return value;
}

// The reviewed conversion quote is a rational number of atomic coin units per
// micro USD. No JS Number, ticker inference, live-price fetch, or implicit peg.
export function coinAtomicForMicroUsd(microUsd, numerator, denominator) {
  const amount = integer(microUsd) * integer(numerator) / integer(denominator);
  if (amount === 0n || amount > U64_MAX) fail("promotionPayoutAmountInvalid");
  return amount.toString();
}

export function validatePayoutIntent(intent, { allowMainnet = false } = {}) {
  exactKeys(intent, ["version", "environment", "campaignId", "payoutId", "settlementId", "settlementSha256",
    "network", "chainIdentifier", "coinType", "coinDecimals", "amountMicroUsd", "coinAmountAtomic", "quoteNumerator",
    "quoteDenominator", "fxSnapshotRef", "destination", "addressVersion", "sender", "gasBudgetAtomic", "gasPriceAtomic",
    "gasSource", "gasValidDuring", "gasPayment", "payoutCoin", "expiresAtEpoch"]);
  if (intent.version !== PAYOUT_VERSION || !["development", "test", "production"].includes(intent.environment)) fail("promotionPayoutVersionInvalid");
  for (const key of ["campaignId", "payoutId", "settlementId", "fxSnapshotRef"]) if (!ID.test(intent[key])) fail("promotionPayoutIdentifierInvalid");
  if (!HASH.test(intent.settlementSha256)) fail("promotionSnapshotChanged");
  if (!["sui:testnet", "sui:mainnet"].includes(intent.network) || !BASE58.test(intent.chainIdentifier)) fail("promotionPayoutNetworkInvalid");
  if (intent.network === "sui:mainnet" && (!allowMainnet || intent.environment !== "production")) fail("promotionPayoutMainnetApprovalRequired");
  if (intent.environment === "production" && intent.network !== "sui:mainnet") fail("promotionPayoutNetworkInvalid");
  if (!ADDRESS.test(intent.destination) || !ADDRESS.test(intent.sender) || intent.destination === intent.sender) fail("promotionPayoutAddressInvalid");
  if (!Number.isInteger(intent.coinDecimals) || intent.coinDecimals < 0 || intent.coinDecimals > 18) fail("promotionPayoutDecimalsInvalid");
  let coinType; try { coinType = normalizeStructTag(intent.coinType); } catch { fail("promotionPayoutCoinInvalid"); }
  if (coinType !== intent.coinType) fail("promotionPayoutCoinNotCanonical");
  integer(intent.addressVersion); integer(intent.gasBudgetAtomic); integer(intent.gasPriceAtomic); integer(intent.expiresAtEpoch);
  if (coinAtomicForMicroUsd(intent.amountMicroUsd, intent.quoteNumerator, intent.quoteDenominator) !== intent.coinAmountAtomic) fail("promotionPayoutQuoteMismatch");
  if (!Array.isArray(intent.gasPayment) || intent.gasPayment.length > 8 ||
      !["coinObjects", "addressBalance"].includes(intent.gasSource) ||
      (intent.gasSource === "coinObjects" && intent.gasPayment.length === 0) ||
      (intent.gasSource === "addressBalance" && (intent.gasPayment.length !== 0 || intent.network !== "sui:testnet" || coinType === SUI))) fail("promotionPayoutGasRequired");
  if (intent.gasSource === "addressBalance") {
    exactKeys(intent.gasValidDuring, ["minEpoch", "nonce"]);
    const minEpoch = integer(intent.gasValidDuring.minEpoch, { zero: true });
    const maxEpoch = integer(intent.expiresAtEpoch);
    if (maxEpoch < minEpoch || maxEpoch > minEpoch + 1n || !Number.isInteger(intent.gasValidDuring.nonce) ||
        intent.gasValidDuring.nonce < 0 || intent.gasValidDuring.nonce > 0xffff_ffff) fail("promotionPayoutExpirationInvalid");
  } else if (intent.gasValidDuring !== null) fail("promotionPayoutExpirationInvalid");
  intent.gasPayment.forEach(ref);
  const ids = new Set(intent.gasPayment.map(v => v.objectId));
  if (ids.size !== intent.gasPayment.length) fail("promotionPayoutObjectConflict");
  if (coinType === SUI) { if (intent.payoutCoin !== null) fail("promotionPayoutObjectInvalid"); }
  else { ref(intent.payoutCoin); if (ids.has(intent.payoutCoin.objectId)) fail("promotionPayoutObjectConflict"); }
  return { intentSha256: sha256(canonicalJson(intent)), coinType };
}

export async function preparePayoutTransaction(intent, options = {}) {
  const { intentSha256, coinType } = validatePayoutIntent(intent, options);
  const tx = new Transaction();
  tx.setSender(intent.sender); tx.setGasOwner(intent.sender);
  tx.setGasBudget(intent.gasBudgetAtomic); tx.setGasPrice(intent.gasPriceAtomic); tx.setGasPayment(intent.gasPayment);
  tx.setExpiration(intent.gasSource === "addressBalance" ? { ValidDuring: {
    minEpoch: intent.gasValidDuring.minEpoch, maxEpoch: intent.expiresAtEpoch,
    minTimestamp: null, maxTimestamp: null, chain: intent.chainIdentifier, nonce: intent.gasValidDuring.nonce,
  } } : { Epoch: intent.expiresAtEpoch });
  const source = coinType === SUI ? tx.gas : tx.objectRef(intent.payoutCoin);
  const [coin] = tx.splitCoins(source, [tx.pure.u64(intent.coinAmountAtomic)]);
  tx.transferObjects([coin], tx.pure.address(intent.destination));
  // All object versions/gas/expiration are already frozen; no RPC build plugin.
  const bytes = await tx.build();
  return { intentSha256, transactionBytes: Buffer.from(bytes).toString("base64"),
    digest: TransactionDataBuilder.getDigestFromBytes(bytes) };
}

export async function validateSignedPayout(intent, envelope, options = {}) {
  exactKeys(envelope, ["intentSha256", "transactionBytes", "digest", "signatures"]);
  const prepared = await preparePayoutTransaction(intent, options);
  if (envelope.intentSha256 !== prepared.intentSha256 || envelope.digest !== prepared.digest ||
      typeof envelope.transactionBytes !== "string" || envelope.transactionBytes.length > 65536) fail("promotionPayoutSignedIntentMismatch");
  const expected = Buffer.from(prepared.transactionBytes, "base64");
  const supplied = Buffer.from(envelope.transactionBytes, "base64");
  if (supplied.toString("base64") !== envelope.transactionBytes || supplied.length !== expected.length ||
      !timingSafeEqual(supplied, expected)) fail("promotionPayoutSignedIntentMismatch");
  if (!Array.isArray(envelope.signatures) || envelope.signatures.length !== 1 || typeof envelope.signatures[0] !== "string" ||
      envelope.signatures[0].length > 16384) fail("promotionPayoutSignatureInvalid");
  try { await verifyTransactionSignature(supplied, envelope.signatures[0], { address: intent.sender, client: options.client }); }
  catch { fail("promotionPayoutSignatureInvalid"); }
  return { ...prepared, signatures: [...envelope.signatures] };
}

export async function verifyPayoutChain(client, intent) {
  const { chainIdentifier } = await client.core.getChainIdentifier({ signal: AbortSignal.timeout(15000) });
  if (chainIdentifier !== intent.chainIdentifier) fail("promotionPayoutChainMismatch");
  const { coinMetadata } = await client.core.getCoinMetadata({ coinType: intent.coinType, signal: AbortSignal.timeout(15000) });
  if (!coinMetadata || coinMetadata.decimals !== intent.coinDecimals) fail("promotionPayoutDecimalsMismatch");
}

export async function verifyPayoutFundingObjects(client, intent) {
  const refs = [...intent.gasPayment, ...(intent.payoutCoin ? [intent.payoutCoin] : [])];
  const response = await client.core.getObjects({ objectIds: refs.map(v => v.objectId), include: { content: true }, signal: AbortSignal.timeout(15000) });
  let gasBalance = 0n, transferBalance = 0n;
  for (const [index, frozen] of refs.entries()) {
    const object = response.objects[index];
    const coinType = index < intent.gasPayment.length ? SUI : intent.coinType;
    if (!object || object instanceof Error || object.objectId !== frozen.objectId || object.version !== frozen.version ||
        object.digest !== frozen.digest || object.owner?.$kind !== "AddressOwner" || object.owner.AddressOwner !== intent.sender ||
        normalizeStructTag(object.type) !== normalizeStructTag(`0x2::coin::Coin<${coinType}>`) || !object.content) fail("promotionPayoutFundingObjectChanged");
    let coin; try { coin = Coin.parse(object.content); } catch { fail("promotionPayoutFundingObjectInvalid"); }
    if (coin.id !== frozen.objectId) fail("promotionPayoutFundingObjectInvalid");
    if (index < intent.gasPayment.length) gasBalance += BigInt(coin.balance); else transferBalance += BigInt(coin.balance);
  }
  const gasRequired = BigInt(intent.gasBudgetAtomic) + (intent.coinType === SUI ? BigInt(intent.coinAmountAtomic) : 0n);
  if (intent.gasSource === "addressBalance") {
    const { balance } = await client.core.getBalance({ owner: intent.sender, coinType: SUI, signal: AbortSignal.timeout(15000) });
    gasBalance = integer(balance.addressBalance, { zero: true });
  }
  if (gasBalance < gasRequired || (intent.payoutCoin && transferBalance < BigInt(intent.coinAmountAtomic))) fail("promotionPayoutFundingInsufficient");
}

export function inspectPayoutResult(intent, digest, result) {
  const tx = result?.Transaction ?? result?.FailedTransaction;
  if (!tx || tx.digest !== digest || tx.effects?.transactionDigest !== digest || !tx.status || !tx.effects.status) return { status: "unknown", code: "promotionPayoutEffectsMissing" };
  const gas = tx.effects.gasUsed;
  let gasUsedAtomic;
  try { gasUsedAtomic = (integer(gas.computationCost, { zero: true }) + integer(gas.storageCost, { zero: true }) - integer(gas.storageRebate, { zero: true })).toString(); }
  catch { return { status: "unknown", code: "promotionPayoutGasUnknown" }; }
  if (tx.status.success === false && tx.effects.status.success === false) return { status: "failed", digest, gasUsedAtomic, code: "promotionPayoutChainFailure" };
  if (result.$kind !== "Transaction" || tx.status.success !== true || tx.effects.status.success !== true || !Array.isArray(tx.balanceChanges)) return { status: "unknown", code: "promotionPayoutEffectsMissing" };
  let received = 0n;
  try {
    for (const change of tx.balanceChanges) {
      const coin = normalizeStructTag(change.coinType);
      if (change.address === intent.destination && coin === intent.coinType) received += BigInt(change.amount);
      if (change.address !== intent.sender && change.address !== intent.destination && BigInt(change.amount) !== 0n) fail("unexpected");
      if (change.address === intent.destination && coin !== intent.coinType && BigInt(change.amount) !== 0n) fail("unexpected");
    }
  } catch { return { status: "unknown", code: "promotionPayoutUnexpectedTransfer" }; }
  if (received !== BigInt(intent.coinAmountAtomic)) return { status: "unknown", code: "promotionPayoutTransferMismatch" };
  return { status: "confirmed", digest, gasUsedAtomic, destination: intent.destination, coinType: intent.coinType,
    coinAmountAtomic: intent.coinAmountAtomic, intentSha256: sha256(canonicalJson(intent)) };
}

// Journal reserve must COMMIT immutable intent, exact signed bytes, signatures
// and digest before resolving. Its CAS claim permits exactly one first send.
// Existing submitted/unknown attempts are query-only, even after a timeout.
export async function executeReviewedPayout({ client, journal, intent, envelope, allowMainnet = false }) {
  const signed = await validateSignedPayout(intent, envelope, { allowMainnet, client });
  await verifyPayoutChain(client, intent);
  const reservation = await journal.reserve({ intent, ...signed });
  if (reservation.digest !== signed.digest || reservation.intentSha256 !== signed.intentSha256) fail("promotionPayoutAttemptConflict");
  if (reservation.status === "confirmed" || reservation.status === "failed") return reservation;
  if (reservation.maySubmit === true) {
    try { await verifyPayoutFundingObjects(client, intent); }
    catch (error) {
      const code = /^promotionPayout[A-Za-z0-9]+$/.test(error.message) ? error.message : "promotionPayoutFundingUnknown";
      const outcome = { status: "unknown", digest: signed.digest, code };
      await journal.recordOutcome(outcome); return outcome;
    }
    // Claim is durable before network I/O. A crash here is unknown, never a new TX.
    await journal.markSubmitted(signed.digest);
    try {
      await client.core.executeTransaction({ transaction: Buffer.from(signed.transactionBytes, "base64"), signatures: signed.signatures,
        include: { effects: true, balanceChanges: true }, signal: AbortSignal.timeout(20000) });
    } catch { await journal.recordOutcome({ status: "unknown", digest: signed.digest, code: "promotionPayoutSubmissionUnknown" }); }
  }
  return reconcilePayout({ client, journal, intent, digest: signed.digest });
}

export async function reconcilePayout({ client, journal, intent, digest }) {
  await verifyPayoutChain(client, intent);
  let result;
  try { result = await client.core.getTransaction({ digest, include: { effects: true, balanceChanges: true }, signal: AbortSignal.timeout(15000) }); }
  catch { const outcome = { status: "unknown", digest, code: "promotionPayoutNotIndexed" }; const recorded = await journal.recordOutcome(outcome); return ["confirmed", "failed"].includes(recorded?.status) ? recorded : outcome; }
  const outcome = { digest, ...inspectPayoutResult(intent, digest, result) };
  // This update must itself compare the persisted digest and frozen intent SHA.
  const recorded = await journal.recordOutcome(outcome);
  return ["confirmed", "failed"].includes(recorded?.status) ? recorded : outcome;
}
