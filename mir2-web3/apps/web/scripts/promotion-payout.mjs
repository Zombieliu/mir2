#!/usr/bin/env node
// Default is read-only probe. No private keys or automatic signing are supported.
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve, dirname } from "node:path";
import { createHmac } from "node:crypto";
import { SuiGrpcClient } from "@mysten/sui/grpc";
import { normalizeStructTag } from "@mysten/sui/utils";
import { preparePayoutTransaction, validateSignedPayout, executeReviewedPayout, reconcilePayout } from "../lib/server/promotion-payout.mjs";

const args = process.argv.slice(2);
const command = args[0] && !args[0].startsWith("--") ? args.shift() : "probe";
const options = {};
while (args.length) {
  const key = args.shift(); if (!key?.startsWith("--") || !args.length || args[0].startsWith("--")) throw new Error("Use --name value");
  if (options[key.slice(2)] !== undefined) throw new Error("Repeated option"); options[key.slice(2)] = args.shift();
}
const allowed = new Set(["network", "rpc", "intent", "signed", "out", "gateway", "confirm-digest"]);
if (Object.keys(options).some(k => !allowed.has(k))) throw new Error("Unknown option");
const network = options.network ?? "testnet";
if (!["testnet", "mainnet"].includes(network)) throw new Error("Only testnet/mainnet supported");
const allowMainnet = network === "mainnet" && process.env.MIR2_PROMOTION_PAYOUT_MAINNET_APPROVED === "1";
if (network === "mainnet" && command !== "probe" && !allowMainnet) throw new Error("promotionPayoutMainnetApprovalRequired");
const rpc = new URL(options.rpc ?? `https://fullnode.${network}.sui.io:443`);
if (rpc.protocol !== "https:" || rpc.username || rpc.password || rpc.search || rpc.hash) throw new Error("RPC must be a credential-free HTTPS URL");
const client = new SuiGrpcClient({ network, baseUrl: rpc.href.replace(/\/$/, "") });
const json = async path => JSON.parse(await readFile(resolve(path), "utf8"));
async function output(value) {
  const data = `${JSON.stringify(value, null, 2)}\n`;
  if (options.out) { const path = resolve(options.out); await mkdir(dirname(path), { recursive: true }); await writeFile(path, data, { flag: "wx" }); }
  console.log(data);
}
function gatewayJournal(intent) {
  if (!options.gateway) throw new Error("--gateway is required for durable PG journal; file journals are not accepted");
  const base = new URL(options.gateway);
  if (base.username || base.password || base.search || base.hash ||
      !["/", "/playtest", "/playtest/"].includes(base.pathname) ||
      (base.protocol !== "https:" && !(base.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(base.hostname)))) throw new Error("Invalid journal URL");
  const path = `${base.pathname.replace(/\/$/, "")}/v1/promotions/payout-worker`;
  const secret = process.env.MIR2_PROMOTION_PAYOUT_WORKER_SECRET;
  if (!secret || Buffer.byteLength(secret) < 32) throw new Error("MIR2_PROMOTION_PAYOUT_WORKER_SECRET must contain at least 32 bytes");
  const call = async value => {
    const body = JSON.stringify(value), timestamp = Date.now().toString();
    const signature = createHmac("sha256", secret).update(`mir2-promotion-source-v1\nPOST\n/v1/promotions/payout-worker\n${timestamp}\n${body}`).digest("hex");
    const response = await fetch(new URL(path, base), { method: "POST", redirect: "error", signal: AbortSignal.timeout(15000),
      headers: { "content-type": "application/json", "x-promotion-source-timestamp": timestamp, "x-promotion-source-signature": signature }, body });
    const text = await response.text(); if (text.length > 1048576) throw new Error("promotionPayoutJournalResponseTooLarge");
    if (!response.ok) { let code; try { code = JSON.parse(text).error; } catch {} throw new Error(typeof code === "string" && /^promotion[A-Za-z0-9]+$/.test(code) ? code : "promotionPayoutJournalUnavailable"); }
    return JSON.parse(text);
  };
  return {
    reserve: signed => call({ action: "reserve", intent, envelope: { intentSha256: signed.intentSha256, transactionBytes: signed.transactionBytes, digest: signed.digest, signatures: signed.signatures } }),
    markSubmitted: digest => call({ action: "markSubmitted", payoutId: intent.payoutId, digest }),
    recordOutcome: outcome => call({ action: "recordOutcome", payoutId: intent.payoutId, outcome }),
  };
}
if (command === "probe") {
  const [chain, gas, coin] = await Promise.all([
    client.core.getChainIdentifier({ signal: AbortSignal.timeout(15000) }),
    client.core.getReferenceGasPrice({ signal: AbortSignal.timeout(15000) }),
    client.core.getCoinMetadata({ coinType: normalizeStructTag("0x2::sui::SUI"), signal: AbortSignal.timeout(15000) }),
  ]);
  await output({ command, network: `sui:${network}`, rpc: rpc.origin, observedAt: new Date().toISOString(), sdkVersion: "2.16.2",
    chainIdentifier: chain.chainIdentifier, referenceGasPrice: gas.referenceGasPrice, suiDecimals: coin.coinMetadata?.decimals ?? null,
    transactionSubmitted: false, phoneSignatureAccepted: false, chainPaymentAccepted: false });
} else {
  if (!options.intent) throw new Error("--intent <frozen-intent.json> required");
  const intent = await json(options.intent);
  if (intent.network !== `sui:${network}`) throw new Error("promotionPayoutNetworkInvalid");
  if (command === "prepare") {
    const prepared = await preparePayoutTransaction(intent, { allowMainnet });
    console.error("Unsigned. Review the frozen settlement, recipient, coin, conversion, gas and epoch before wallet signing. No transaction submitted.");
    await output({ ...prepared, signatures: [] });
  } else if (command === "verify") {
    if (!options.signed) throw new Error("--signed <signed-envelope.json> required");
    const signed = await validateSignedPayout(intent, await json(options.signed), { allowMainnet, client });
    await output({ intentSha256: signed.intentSha256, digest: signed.digest, signatureValid: true, transactionSubmitted: false });
  } else if (command === "execute") {
    if (!options.signed) throw new Error("--signed <signed-envelope.json> required");
    const envelope = await json(options.signed);
    if (options["confirm-digest"] !== envelope.digest) throw new Error("--confirm-digest must equal the reviewed signed digest");
    await output(await executeReviewedPayout({ client, journal: gatewayJournal(intent), intent, envelope, allowMainnet }));
  } else if (command === "reconcile") {
    if (!options.signed) throw new Error("--signed <signed-envelope.json> required");
    const envelope = await json(options.signed);
    const signed = await validateSignedPayout(intent, envelope, { allowMainnet, client });
    await output(await reconcilePayout({ client, journal: gatewayJournal(intent), intent, digest: signed.digest }));
  } else throw new Error("Use probe, prepare, verify, execute or reconcile");
}
