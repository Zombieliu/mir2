import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { after, test } from "node:test";
import vm from "node:vm";

import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";
import ts from "typescript";

const require = createRequire(import.meta.url);
const variables = [
  "MIR2_PASSKEY_AUTH_SECRET", "MIR2_PASSKEY_ALLOWED_ORIGINS", "MIR2_ALLOWED_WEB_ORIGINS",
  "MIR2_DUBHE_CONNECT_RELAY_URL", "MIR2_DUBHE_WALLET_ORIGIN", "MIR2_DUBHE_CONNECT_NETWORK",
];
const original = Object.fromEntries(variables.map((name) => [name, process.env[name]]));
after(() => {
  for (const name of variables) {
    if (original[name] === undefined) delete process.env[name];
    else process.env[name] = original[name];
  }
});

const origin = "https://play.example.test";
const relayOrigin = "https://relay.example.test";
const walletOrigin = "https://wallet.example.test";
const keypair = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(7));
const otherKeypair = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(9));
let now = Date.now();
let fetchImplementation;
const requests = [];
const modules = new Map();

function loadModule(path) {
  if (modules.has(path)) return modules.get(path);
  const compiled = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  const module = { exports: {} };
  const context = vm.createContext({
    module, exports: module.exports, Buffer, TextEncoder, Response, Request, URL, AbortSignal,
    process,
    Date: class extends Date { static now() { return now; } },
    fetch: async (...args) => {
      requests.push(args);
      return fetchImplementation(...args);
    },
    require: (name) => {
      if (name === "server-only") return {};
      if (name === "./passkey-auth") return loadModule("../lib/server/passkey-auth.ts");
      return require(name);
    },
  });
  vm.runInContext(compiled, context, { filename: path });
  modules.set(path, module.exports);
  return module.exports;
}

const { createDubheConnectLogin, readDubheConnectLogin, buildDubheConnectLoginMessage } =
  loadModule("../lib/server/dubhe-connect-auth.ts");

function reset() {
  now = Date.now();
  requests.length = 0;
  process.env.MIR2_PASSKEY_AUTH_SECRET = "dubhe-test-secret-with-at-least-32-characters";
  process.env.MIR2_PASSKEY_ALLOWED_ORIGINS = origin;
  delete process.env.MIR2_ALLOWED_WEB_ORIGINS;
  process.env.MIR2_DUBHE_CONNECT_RELAY_URL = relayOrigin;
  process.env.MIR2_DUBHE_WALLET_ORIGIN = walletOrigin;
  process.env.MIR2_DUBHE_CONNECT_NETWORK = "testnet";
}

function session() {
  const id = "session-123";
  return {
    id, relayOrigin,
    approvalUrl: `${walletOrigin}/connect/approve?session=${id}&relay=${encodeURIComponent(relayOrigin)}`,
    pollUrl: `${relayOrigin}/sessions/${id}`,
    status: "pending",
    request: {
      appName: "Mir2 Web3", origin, network: "testnet", walletOrigin,
      statement: "Sign in to Mir2 Web3 with Dubhe Wallet.", nonce: "unique-server-nonce",
      requestedAt: new Date(now).toISOString(), expiresAt: new Date(now + 120_000).toISOString(),
    },
  };
}

function request(path, options = {}) {
  return new Request(`${origin}/api/wallet/dubhe/session${path}`, {
    method: "POST", headers: { origin, ...options.headers },
    ...(options.body === undefined ? {} : { body: options.body }),
  });
}

async function create(pending = session()) {
  fetchImplementation = async () => Response.json(pending);
  const result = await createDubheConnectLogin(request(""));
  assert.equal(result.status, 200);
  assert.equal(result.headers.get("cache-control"), "no-store");
  return result.json();
}

async function approve(pending) {
  const message = buildDubheConnectLoginMessage(pending);
  const signed = await keypair.signPersonalMessage(new TextEncoder().encode(message));
  return {
    ...pending, status: "approved", result: {
      account: {
        address: keypair.toSuiAddress(), publicKey: keypair.getPublicKey().toBase64(),
        chains: ["sui:testnet"], features: ["sui:signPersonalMessage"],
      },
      message, bytes: signed.bytes, signature: signed.signature,
      approvedAt: new Date(now).toISOString(),
    },
  };
}

async function status(intentToken, resolved, options = {}) {
  fetchImplementation = async () => Response.json(resolved);
  return readDubheConnectLogin(request("/status", {
    headers: { authorization: `Bearer ${intentToken}`, ...options.headers }, ...options,
  }));
}

function proofPayload(token) {
  return JSON.parse(Buffer.from(token.split(".")[0], "base64url").toString("utf8"));
}

test("creates a bounded trusted session and keeps browser intent out of the QR", async () => {
  reset();
  const pending = session();
  const created = await create(pending);
  assert.equal(created.sessionId, pending.id);
  assert.equal(created.expiresAt, Date.parse(pending.request.expiresAt));
  assert.equal(created.approvalUrl, pending.approvalUrl);
  assert.ok(!created.approvalUrl.includes(created.intentToken));
  assert.equal(requests[0][0], `${relayOrigin}/sessions`);
  assert.equal(requests[0][1].cache, "no-store");
  assert.equal(requests[0][1].redirect, "error");
  assert.equal(JSON.parse(requests[0][1].body).expiresInMs, 120_000);
  const result = await status(created.intentToken, pending);
  assert.deepEqual(await result.json(), { status: "pending" });
  assert.equal(requests[1][0], `${relayOrigin}/sessions/${pending.id}`);
});

test("accepts empty POST streams from NextRequest and an explicit empty object", async () => {
  reset();
  const pending = session();
  fetchImplementation = async () => Response.json(pending);
  const streamedRequest = (path, authorization) => new Request(
    `${origin}/api/wallet/dubhe/session${path}`,
    {
      method: "POST", headers: { origin, ...(authorization ? { authorization } : {}) },
      body: new ReadableStream({ start(controller) { controller.close(); } }), duplex: "half",
    },
  );
  const result = await createDubheConnectLogin(streamedRequest(""));
  assert.equal(result.status, 200);
  const created = await result.json();
  const polled = await readDubheConnectLogin(streamedRequest("/status", `Bearer ${created.intentToken}`));
  assert.equal(polled.status, 200);
  assert.deepEqual(await polled.json(), { status: "pending" });
  assert.equal((await createDubheConnectLogin(request("", { body: "{}" }))).status, 200);
});

test("verifies a real Sui signature and reissues only the same fixed-expiry one-use proof", async () => {
  reset();
  const pending = session();
  const created = await create(pending);
  const approved = await approve(pending);
  const first = await (await status(created.intentToken, approved)).json();
  assert.equal(first.status, "approved");
  assert.equal(first.login.accountId, `sui:${keypair.toSuiAddress()}`);
  assert.equal(first.login.provider, "suiWallet");
  assert.deepEqual(proofPayload(first.login.token), {
    auth: "sui-passkey-v2", accountId: first.login.accountId,
    jti: `dubhe-connect:${pending.id}`, expMs: created.expiresAt, authMethod: "wallet",
  });
  now += 61_000;
  const second = await (await status(created.intentToken, approved)).json();
  assert.equal(second.login.token, first.login.token);
  assert.equal(second.login.expiresAt, first.login.expiresAt);
});

test("rejects missing/disallowed Origin, body-selected sessions, and large bodies before relay access", async () => {
  reset();
  for (const options of [
    { headers: { origin: "https://attacker.example" } },
    { headers: { origin: "" } },
    { body: JSON.stringify({ relayOrigin: "http://169.254.169.254" }) },
    { body: "x".repeat(1_025) },
  ]) {
    const result = await createDubheConnectLogin(request("", options));
    assert.ok(result.status >= 400);
    assert.equal(result.headers.get("cache-control"), "no-store");
  }
  assert.equal(requests.length, 0);
});

test("rejects intent tampering and use from another otherwise allowed origin", async () => {
  reset();
  const pending = session();
  const created = await create(pending);
  const [payload, mac] = created.intentToken.split(".");
  const changed = proofPayload(created.intentToken);
  changed.session.id = "another-session";
  const forged = `${Buffer.from(JSON.stringify(changed)).toString("base64url")}.${mac}`;
  assert.equal((await status(forged, pending)).status, 401);
  assert.equal((await status(`${payload}.${mac.slice(0, -1)}`, pending)).status, 401);
  process.env.MIR2_PASSKEY_ALLOWED_ORIGINS = `${origin},https://another.example`;
  assert.equal((await status(created.intentToken, pending, {
    headers: { origin: "https://another.example", authorization: `Bearer ${created.intentToken}` },
  })).status, 401);
  assert.equal(requests.length, 1);
});

test("rejects create responses that change trusted origins, protocol URLs, identity fields, or TTL", async () => {
  reset();
  const mutations = [
    (value) => { value.id = "../other"; },
    (value) => { value.relayOrigin = "https://evil.example"; },
    (value) => { value.pollUrl = "https://evil.example/session"; },
    (value) => { value.approvalUrl += "&relay=https://evil.example"; },
    (value) => { value.request.origin = "https://evil.example"; },
    (value) => { value.request.walletOrigin = "https://evil.example"; },
    (value) => { value.request.appName = "Another App"; },
    (value) => { value.request.statement = "Another statement"; },
    (value) => { value.request.network = "mainnet"; },
    (value) => { value.request.callbackUrl = `${origin}/callback`; },
    (value) => { value.request.requestedAt = now; },
    (value) => { value.request.expiresAt = new Date(now + 300_001).toISOString(); },
    (value) => { value.status = "approved"; },
  ];
  for (const mutate of mutations) {
    const value = session();
    mutate(value);
    fetchImplementation = async () => Response.json(value);
    assert.ok((await createDubheConnectLogin(request(""))).status >= 400);
  }
});

test("rejects changed signed envelopes and wrong account, key, chain, message, bytes, signature, or time", async () => {
  reset();
  const pending = session();
  const created = await create(pending);
  const approved = await approve(pending);
  const mutations = [
    (value) => { value.request.nonce = "another-nonce"; },
    (value) => { value.id = "another-session"; },
    (value) => { value.result.account.address = otherKeypair.toSuiAddress(); },
    (value) => { value.result.account.publicKey = otherKeypair.getPublicKey().toBase64(); },
    (value) => { value.result.account.chains = ["sui:mainnet"]; },
    (value) => { value.result.message = value.result.message.replace("Version: 2\n", ""); },
    (value) => { value.result.bytes = Buffer.from("tampered").toString("base64"); },
    (value) => { value.result.signature = value.result.signature.slice(0, -8); },
    (value) => { value.result.approvedAt = new Date(now + 120_001).toISOString(); },
    (value) => { value.result.approvedAt = new Date(now - 1).toISOString(); },
  ];
  for (const mutate of mutations) {
    const value = structuredClone(approved);
    mutate(value);
    assert.ok((await status(created.intentToken, value)).status >= 400);
  }
});

test("expires intents before relay access and reports wallet rejection without issuing a proof", async () => {
  reset();
  const pending = session();
  const created = await create(pending);
  assert.deepEqual(await (await status(created.intentToken, { ...pending, status: "rejected" })).json(),
    { status: "rejected" });
  const count = requests.length;
  now = created.expiresAt;
  assert.deepEqual(await (await status(created.intentToken, await approve(pending))).json(),
    { status: "expired" });
  assert.equal(requests.length, count);
});

test("bounds relay responses and fails closed on malformed JSON, upstream failures, or unsafe config", async () => {
  reset();
  for (const upstream of [
    () => new Response("x".repeat(32_769)),
    () => new Response("invalid JSON"),
    () => new Response("failure", { status: 500 }),
  ]) {
    fetchImplementation = async () => upstream();
    const result = await createDubheConnectLogin(request(""));
    assert.ok(result.status >= 400);
    assert.deepEqual(await result.json(), { error: "Dubhe Wallet login is unavailable or could not be verified" });
  }
  const count = requests.length;
  process.env.MIR2_DUBHE_CONNECT_RELAY_URL = "http://insecure.example";
  assert.ok((await createDubheConnectLogin(request(""))).status >= 400);
  assert.equal(requests.length, count);
});
