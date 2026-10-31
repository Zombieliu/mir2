#!/usr/bin/env node
// Human sign-only handoff. No RPC client, private key, worker key or execution API.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { randomBytes, timingSafeEqual } from "node:crypto";
import { readFile, realpath, stat, access, writeFile } from "node:fs/promises";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { preparePayoutTransaction, validateSignedPayout } from "../lib/server/promotion-payout.mjs";

export const HANDOFF_ORIGIN = "http://127.0.0.1:7373";
export const WALLET_ORIGIN = "https://wallet.dubhe.obelisk.build";
const MAX_BODY = 16_384;
const MAX_GAS = 10_000_000n;
const ADDRESS = /^0x[0-9a-f]{64}$/;
const HASH = /^[0-9a-f]{64}$/;
function fail(code) { throw new Error(code); }
function exact(value, keys) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).sort().join("|") !== [...keys].sort().join("|")) fail("handoffFieldsInvalid");
}

export async function validateHandoffScope(intent, unsigned, { expectedSender, expectedIntentSha256 } = {}) {
  if (!ADDRESS.test(expectedSender || "") || !HASH.test(expectedIntentSha256 || "")) fail("handoffExplicitScopeRequired");
  if (intent?.environment !== "test" || intent.network !== "sui:testnet" || intent.sender !== expectedSender ||
      intent.coinDecimals !== 6 || !intent.coinType?.endsWith("::usdc::USDC") ||
      intent.amountMicroUsd !== "10000" || intent.coinAmountAtomic !== "10000" ||
      intent.quoteNumerator !== "1" || intent.quoteDenominator !== "1") fail("handoffTestnetScopeInvalid");
  if (typeof intent.gasBudgetAtomic !== "string" || !/^[1-9][0-9]{0,19}$/.test(intent.gasBudgetAtomic) ||
      BigInt(intent.gasBudgetAtomic) > MAX_GAS) fail("handoffGasCapExceeded");
  exact(unsigned, ["intentSha256", "transactionBytes", "digest", "signatures"]);
  if (!Array.isArray(unsigned.signatures) || unsigned.signatures.length !== 0) fail("handoffUnsignedRequired");
  const frozen = await preparePayoutTransaction(intent);
  if (frozen.intentSha256 !== expectedIntentSha256 || unsigned.intentSha256 !== expectedIntentSha256 ||
      frozen.transactionBytes !== unsigned.transactionBytes || frozen.digest !== unsigned.digest) fail("handoffFrozenIntentMismatch");
  return { intent: structuredClone(intent), unsigned: structuredClone(unsigned) };
}

// This exact function is also embedded into the browser; keep it dependency-free.
export function parseBridgeResponse(event, { popup, id, transactionBytes, sender }) {
  if (event.origin !== "https://wallet.dubhe.obelisk.build" || !popup || event.source !== popup || event.data?.id !== id) return null;
  const data = event.data;
  const sameKeys = (value, keys) => value && typeof value === "object" && !Array.isArray(value) &&
    Object.keys(value).sort().join("|") === [...keys].sort().join("|");
  if (data.version !== 1 || data.source !== "dubhe-wallet" || data.method !== "signTransaction" || typeof data.ok !== "boolean") throw new Error("handoffBridgeResponseInvalid");
  if (!data.ok) {
    if (!sameKeys(data, ["id", "version", "source", "method", "ok", "error"])) throw new Error("handoffBridgeResponseInvalid");
    throw new Error("Wallet declined or could not sign this request. Nothing was submitted.");
  }
  if (!sameKeys(data, ["id", "version", "source", "method", "ok", "result"]) ||
      !sameKeys(data.result, ["account", "bytes", "signature"])) throw new Error("handoffBridgeResponseInvalid");
  const { account, bytes, signature } = data.result;
  if (!account || typeof account !== "object" || Array.isArray(account) || account.address !== sender ||
      Object.keys(account).some(key => !["address", "publicKey", "chains", "features", "label"].includes(key)) ||
      !Array.isArray(account.chains) || !account.chains.includes("sui:testnet")) throw new Error("handoffSignerMismatch");
  if (bytes !== transactionBytes) throw new Error("handoffFrozenBytesMismatch");
  if (typeof signature !== "string" || signature.length < 8 || signature.length > 12_000 ||
      !/^[A-Za-z0-9+/]+={0,2}$/.test(signature)) throw new Error("handoffSignatureFormatInvalid");
  return { bytes, signature };
}

export async function validateHandoffSignature(scope, envelope) {
  // No client is provided: verification is offline. Signatures needing an online
  // verifier fail rather than introducing a hidden RPC or transaction submission.
  return validateSignedPayout(scope.intent, envelope);
}

export function authorizeHandoffPost(request, origin, csrf) {
  const token = request.headers["x-payout-csrf"];
  return request.method === "POST" && request.url === "/signed" && request.headers.origin === origin &&
    request.headers.host === new URL(origin).host && request.headers["content-type"] === "application/json" &&
    typeof token === "string" && Buffer.byteLength(token) === Buffer.byteLength(csrf) && timingSafeEqual(Buffer.from(token), Buffer.from(csrf));
}

async function readJsonFile(path) {
  if (!isAbsolute(path)) fail("handoffAbsoluteInputRequired");
  const info = await stat(path);
  if (!info.isFile() || info.size > 65_536 || info.size === 0) fail("handoffInputFileInvalid");
  return JSON.parse(await readFile(path, "utf8"));
}

export async function validateOutputPath(path) {
  if (!isAbsolute(path) || !path.endsWith(".json")) fail("handoffAbsoluteOutputRequired");
  const parent = await realpath(dirname(path));
  if (basename(parent).toLowerCase() !== "private" || !(await stat(parent)).isDirectory()) fail("handoffPrivateDirectoryRequired");
  const checkout = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
  const within = relative(checkout, parent);
  if (!within || (!within.startsWith("..") && !isAbsolute(within))) fail("handoffOutputOutsideCheckoutRequired");
  try { await access(path); fail("handoffOutputAlreadyExists"); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
  return resolve(parent, basename(path));
}

export async function saveHandoffSignature(scope, envelope, out) {
  const signed = await validateHandoffSignature(scope, envelope);
  const path = await validateOutputPath(out);
  // Exclusive creation; inherits the already-private parent directory's ACL.
  await writeFile(path, `${JSON.stringify(signed, null, 2)}\n`, { flag: "wx", mode: 0o600 });
  return { status: "signature_verified_and_saved", intentSha256: signed.intentSha256, digest: signed.digest,
    transactionSubmitted: false, chainPaymentAccepted: false };
}

function jsonScript(value) { return JSON.stringify(value).replace(/[<>&\u2028\u2029]/g, c => `\\u${c.charCodeAt(0).toString(16).padStart(4, "0")}`); }
function escapeHtml(value) { return String(value).replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]); }
export function renderHandoffPage(scope, csrf, nonce) {
  const { intent, unsigned } = scope;
  const rows = [
    ["Environment", "Sui Testnet — test coins have no real monetary value"], ["Amount", "0.01 test USDC (10,000 atomic units; 0.01 USD test accounting)"],
    ["Sender — human signing wallet", intent.sender], ["Receiver — verified recipient", intent.destination], ["Coin type", intent.coinType],
    ["Gas source", intent.gasSource], ["Gas budget / hard cap", `${intent.gasBudgetAtomic} MIST / 10,000,000 MIST (0.01 SUI)`],
    ["Gas price", intent.gasPriceAtomic], ["Gas validity", JSON.stringify(intent.gasValidDuring)], ["Expiry epoch", intent.expiresAtEpoch],
    ["Intent SHA-256", unsigned.intentSha256], ["Transaction digest", unsigned.digest], ["Settlement SHA-256", intent.settlementSha256],
  ];
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Numeron · Testnet sign-only handoff</title><style nonce="${nonce}">body{margin:0;background:#11160f;color:#e0e8d7;font:15px/1.6 system-ui}main{max-width:860px;margin:auto;padding:28px}h1{font-size:26px}.notice{border:1px solid #b8a06a;padding:16px;border-radius:5px;color:#ead29e}dt{color:#aab59e;margin-top:16px;font-size:13px}dd{margin:4px 0;font:12px/1.7 monospace;overflow-wrap:anywhere}button{padding:13px 18px;margin:16px 8px 0 0;border:1px solid #b8a06a;background:#d8bf8b;color:#11160f;border-radius:4px;font:inherit;cursor:pointer}button:disabled{opacity:.5;cursor:wait}label{display:flex;gap:10px;margin-top:24px}#status{overflow-wrap:anywhere;white-space:pre-wrap}.muted{color:#aab59e;font-size:13px}</style></head><body><main><h1>Human approval · sign only</h1><p class="notice"><strong>Technical Testnet fixture, not season commission.</strong><br>No real monetary value. This page requests a transaction signature only. It cannot broadcast a payment. After signature verification, a separate reviewed worker must persist the signed attempt before any submission.</p><dl>${rows.map(([label, value]) => `<dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd>`).join("")}</dl><p class="muted">Open this page in the same browser where the sender wallet can be unlocked. The recipient's login signature cannot authorize the sender. Select Sui Testnet and check the full sender, receiver, amount, gas cap and intent hash above.</p><label><input id="reviewed" type="checkbox">I reviewed these frozen details. Request only a signature for this exact 0.01 test USDC transaction.</label><button id="sign" disabled>Open Dubhe Wallet · sign only</button><button id="cancel" disabled>Cancel request</button><p id="status" role="status" aria-live="polite">No wallet request has been created. No transaction has been submitted.</p><script nonce="${nonce}">
const frozen=${jsonScript(unsigned)}, sender=${jsonScript(intent.sender)}, csrf=${jsonScript(csrf)}, localOrigin=${jsonScript(HANDOFF_ORIGIN)}, walletOrigin=${jsonScript(WALLET_ORIGIN)};
const parseBridgeResponse=${parseBridgeResponse.toString()};
const sign=document.getElementById('sign'),reviewed=document.getElementById('reviewed'),cancel=document.getElementById('cancel'),status=document.getElementById('status');
let active=null,finished=false;
function reset(message){if(active){clearTimeout(active.timer);clearInterval(active.closed);active.popup?.close();active=null;}status.textContent=message;cancel.disabled=true;sign.disabled=finished||!reviewed.checked;}
reviewed.addEventListener('change',()=>{sign.disabled=finished||!!active||!reviewed.checked;});
cancel.addEventListener('click',()=>reset('Request cancelled. Nothing was submitted.'));
sign.addEventListener('click',()=>{
 if(active||finished||!reviewed.checked||location.origin!==localOrigin)return;
 const id='numeron-sign-'+crypto.randomUUID();
 const request={id,version:1,source:'dubhe-dapp',method:'signTransaction',origin:localOrigin,requestedAt:new Date().toISOString(),params:{transaction:frozen.transactionBytes,chain:'sui:testnet',accountAddress:sender}};
 const bytes=new TextEncoder().encode(JSON.stringify(request));let binary='';for(const byte of bytes)binary+=String.fromCharCode(byte);
 const popup=window.open(walletOrigin+'/bridge/popup?request='+encodeURIComponent(btoa(binary)),id,'popup=yes,width=500,height=820');
 if(!popup){reset('Wallet popup was blocked. Allow popups and click again.');return;}
 active={id,popup,started:Date.now(),processing:false,timer:setTimeout(()=>reset('Wallet request expired. Nothing was submitted.'),120000),closed:setInterval(()=>{if(active?.popup.closed&&!active.processing)reset('Wallet was closed. Nothing was submitted.');},500)};
 sign.disabled=true;cancel.disabled=false;status.textContent='Waiting for human approval in Dubhe Wallet. Sign only; do not use sign and execute.';
});
window.addEventListener('message',async event=>{
 if(!active||active.processing||Date.now()-active.started>120000)return;
 let result;try{result=parseBridgeResponse(event,{popup:active.popup,id:active.id,transactionBytes:frozen.transactionBytes,sender});}catch(error){reset(error.message);return;}
 if(!result)return;active.processing=true;cancel.disabled=true;clearInterval(active.closed);clearTimeout(active.timer);
 status.textContent='Verifying the exact transaction and sender signature locally. No payment is being submitted.';
 try{const response=await fetch('/signed',{method:'POST',credentials:'omit',redirect:'error',cache:'no-store',signal:AbortSignal.timeout(10000),headers:{'content-type':'application/json','x-payout-csrf':csrf},body:JSON.stringify({...frozen,signatures:[result.signature]})});const data=await response.json();if(!response.ok)throw new Error(data.error||'Signature was not saved.');finished=true;reviewed.disabled=true;reset('Signature verified and saved. No transaction was broadcast. A separate reviewed worker must persist this signed attempt before submission.');}catch(error){reset(error.message||'Signature verification failed. No transaction was submitted.');}
});
</script></main></body></html>`;
}

async function main() {
  const { values } = parseArgs({ options: { intent: { type: "string" }, unsigned: { type: "string" }, out: { type: "string" },
    "expected-sender": { type: "string" }, "expected-intent-sha256": { type: "string" }, help: { type: "boolean" } }, strict: true });
  if (values.help) { console.log("Sign-only human handoff: --intent <absolute.json> --unsigned <absolute.json> --out <existing-private-directory/new.json> --expected-sender <full-address> --expected-intent-sha256 <sha256>. Listens only on 127.0.0.1:7373. No signing or broadcast without a human wallet action."); return; }
  for (const key of ["intent", "unsigned", "out", "expected-sender", "expected-intent-sha256"]) assert.ok(values[key], `Missing --${key}`);
  const scope = await validateHandoffScope(await readJsonFile(values.intent), await readJsonFile(values.unsigned), {
    expectedSender: values["expected-sender"], expectedIntentSha256: values["expected-intent-sha256"],
  });
  const out = await validateOutputPath(values.out);
  const csrf = randomBytes(32).toString("base64url"), nonce = randomBytes(24).toString("base64url");
  const html = renderHandoffPage(scope, csrf, nonce);
  let busy = false, completed = false;
  const server = createServer(async (request, response) => {
    const send = (status, value, type = "application/json") => { response.writeHead(status, {
      "content-type": `${type}; charset=utf-8`, "cache-control": "no-store", "referrer-policy": "no-referrer", "x-content-type-options": "nosniff",
      "content-security-policy": `default-src 'none'; script-src 'nonce-${nonce}'; style-src 'nonce-${nonce}'; connect-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'self'`,
    }); response.end(type === "text/html" ? value : JSON.stringify(value)); };
    if (request.headers.host !== new URL(HANDOFF_ORIGIN).host) { send(403, { error: "handoffHostRejected" }); return; }
    if (request.method === "GET" && request.url === "/") { send(200, html, "text/html"); return; }
    if (request.method !== "POST" || request.url !== "/signed") { send(404, { error: "handoffRouteUnavailable" }); return; }
    if (!authorizeHandoffPost(request, HANDOFF_ORIGIN, csrf)) { send(403, { error: "handoffOriginOrCsrfRejected" }); return; }
    if (busy || completed) { send(409, { error: "handoffAlreadyProcessingOrSaved" }); return; }
    busy = true;
    const timer = setTimeout(() => request.destroy(), 10_000);
    try {
      const chunks = []; let size = 0;
      for await (const chunk of request) { size += chunk.length; if (size > MAX_BODY) { send(413, { error: "handoffBodyTooLarge" }); return; } chunks.push(chunk); }
      const envelope = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      const result = await saveHandoffSignature(scope, envelope, out);
      completed = true; send(200, result);
      console.log(JSON.stringify({ ...result, output: out }));
      setTimeout(() => server.close(), 250);
    } catch (error) {
      const code = typeof error.message === "string" && /^(?:handoff|promotion)[A-Za-z]+$/.test(error.message) ? error.message : "handoffSignatureRejected";
      if (!response.headersSent) send(400, { error: code, transactionSubmitted: false });
    } finally { clearTimeout(timer); busy = false; }
  });
  server.requestTimeout = 10_000; server.headersTimeout = 8_000; server.maxRequestsPerSocket = 20;
  server.on("error", () => { console.error("Sign-only handoff could not listen; check whether port 7373 is already in use."); process.exitCode = 1; });
  const expiry = setTimeout(() => server.close(), 15 * 60_000); expiry.unref();
  server.on("close", () => clearTimeout(expiry));
  server.listen(7373, "127.0.0.1", () => console.log(JSON.stringify({ url: HANDOFF_ORIGIN, network: "sui:testnet", testUsdc: "0.01",
    gasCapSui: "0.01", intentSha256: scope.unsigned.intentSha256, digest: scope.unsigned.digest, transactionSubmitted: false, expiresInMinutes: 15 })));
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main().catch(error => { console.error(error.message); process.exitCode = 1; });
