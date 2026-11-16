import assert from "node:assert/strict";
import { createCipheriv, createDecipheriv, createHash, createHmac, randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";

const require = createRequire(import.meta.url);
const WEB = new URL("../", import.meta.url);
const ORIGIN = "https://mir2.obelisk.build";
const PROFILES = {
  legacy: { id: "legacy-public-season", gateway: "https://promotion.example.test", checkout: "https://promotion.example.test",
    relay: "https://legacy-relay.example.test", wallet: "https://legacy-wallet.example.test", secret: "synthetic-legacy-wallet-secret-aaaaaaaaaaaaaaaa", session: "synthetic-legacy-cookie-secret-bbbbbbbbbbbbbbbb" },
  windows: { id: "windows-public-season", gateway: "https://api-playtest.example.test", checkout: "https://billing-playtest.example.test",
    relay: "https://windows-relay.example.test", wallet: "https://windows-wallet.example.test", secret: "synthetic-windows-wallet-secret-ccccccccccccccc", session: "synthetic-windows-cookie-secret-dddddddddddddd" },
};
const SESSION = "__Host-mir2-promotion-session";
const WINDOWS_SESSION = "__Host-mir2-promotion-windows-session";
const pair = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(64));

// Every local import is transpiled from the working source. Only network edges
// and the immutable environment are supplied by the fixture.
function sourceLoader({ env = {}, fetch, window, overrides = () => undefined } = {}) {
  const modules = new Map();
  const mockProcess = { env: new Proxy(env, { set() { throw Error("request mutated process.env"); }, deleteProperty() { throw Error("request deleted process.env"); } }) };
  function load(path) {
    const url = path instanceof URL ? path : new URL(path, WEB);
    if (modules.has(url.href)) return modules.get(url.href).exports;
    const compiled = ts.transpileModule(readFileSync(url, "utf8"), { compilerOptions: {
      target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX,
    } }).outputText;
    const module = { exports: {} }; modules.set(url.href, module);
    const sourceRequire = spec => {
      const overridden = overrides(spec, url);
      if (overridden !== undefined) return overridden;
      if (spec === "server-only") return {};
      if (!spec.startsWith(".")) return require(spec);
      const base = new URL(spec, url);
      const file = ["", ".ts", ".tsx"].map(ext => new URL(base.href + ext)).find(candidate => existsSync(candidate) && !candidate.pathname.endsWith(".css"));
      assert.ok(file, `missing fixture source import ${spec}`);
      return load(file);
    };
    vm.runInNewContext(compiled, { module, exports: module.exports, process: mockProcess, Buffer, TextEncoder, Blob,
      Request, Response, Headers, URL, AbortSignal, AbortController, Intl, crypto, fetch, window, require: sourceRequire }, { filename: fileURLToPath(url) });
    return module.exports;
  }
  return load;
}

function fixture() {
  const env = { VERCEL: "1", VERCEL_ENV: "production", NODE_ENV: "production",
    MIR2_PASSKEY_AUTH_SECRET: "synthetic-normal-game-secret-eeeeeeeeeeeeeeeee",
    MIR2_PROMOTION_WINDOWS_CAMPAIGN_ID: PROFILES.windows.id, MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS: `${PROFILES.legacy.id},legacy-archived-season` };
  for (const [name, profile] of Object.entries(PROFILES)) {
    const prefix = name === "windows" ? "MIR2_PROMOTION_WINDOWS_" : "MIR2_PROMOTION_";
    Object.assign(env, Object.fromEntries(Object.entries({ CAMPAIGN_ID: profile.id, GATEWAY_HTTP_URL: profile.gateway,
      GATEWAY_ALLOWED_ORIGINS: profile.gateway, CHECKOUT_ORIGIN: profile.checkout, WALLET_AUTH_SECRET: profile.secret,
      SESSION_SECRET: profile.session, ALLOWED_ORIGINS: ORIGIN, PUBLIC_ORIGIN: ORIGIN, DUBHE_CONNECT_NETWORK: "testnet",
      DUBHE_CONNECT_RELAY_URL: profile.relay, DUBHE_WALLET_ORIGIN: profile.wallet }).map(([key, value]) => [prefix + key, value])));
  }
  const calls = [], relays = new Map(), identities = new Map(); let sequence = 0, checkoutUrl;
  const fetch = async (url, init) => {
    const target = String(url); calls.push({ url: target, init });
    // Yield inside every mock edge so concurrent requests resume in interleaved scopes.
    await new Promise(resolve => setImmediate(resolve));
    const relay = Object.entries(PROFILES).find(([, value]) => target === `${value.relay}/sessions`);
    if (relay) {
      const body = JSON.parse(init.body), id = `synthetic-${++sequence}`, now = Date.now();
      const value = { id, relayOrigin: relay[1].relay, approvalUrl: `${body.walletOrigin}/connect/approve?session=${id}&relay=${encodeURIComponent(relay[1].relay)}`,
        pollUrl: `${relay[1].relay}/sessions/${id}`, status: "pending", request: { appName: body.appName, origin: body.origin,
          network: body.network, walletOrigin: body.walletOrigin, statement: body.statement, nonce: `nonce-${id}`,
          requestedAt: new Date(now).toISOString(), expiresAt: new Date(now + 120_000).toISOString() } };
      relays.set(value.pollUrl, value); return Response.json(value);
    }
    if (relays.has(target)) return Response.json(relays.get(target));
    const entry = Object.entries(PROFILES).find(([, value]) => new URL(target).origin === value.gateway);
    assert.ok(entry, "network edge must be a configured profile");
    const [name, profile] = entry, pathname = new URL(target).pathname;
    if (["/v1/promotions/auth/password", "/v1/promotions/auth/exchange"].includes(pathname)) {
      const wallet = pathname.endsWith("exchange");
      if (wallet) {
        const proof = JSON.parse(init.body).credential, [payload, signature] = proof.split(".");
        assert.equal(signature, createHmac("sha256", profile.secret).update(payload).digest("base64url"));
      }
      const session = { accountId: `${name}-account`, sessionId: `${name}-session-${++sequence}`, credentialId: `${name}-credential`,
        authMethod: wallet ? "sui_wallet" : "password", issuedAtMs: Date.now(), expiresAtMs: Date.now() + 3_600_000 };
      const token = `synthetic-${name}-identity-token-${sequence}`; identities.set(token, { name, session, revoked: false });
      return Response.json({ token, session });
    }
    if (pathname === "/v1/identity/me" || pathname === "/v1/identity/sessions/revoke") {
      const identity = identities.get(init.headers.get("authorization")?.replace(/^Bearer /, ""));
      if (!identity || identity.name !== name || identity.revoked) return Response.json({ error: "not_authorized" }, { status: 401 });
      if (pathname.endsWith("revoke")) { identity.revoked = true; return Response.json({ accepted: true }); }
      const { session } = identity;
      return Response.json({ accountId: session.accountId, currentSessionId: session.sessionId, sessions: [session],
        credentials: [{ credentialId: session.credentialId, credentialKind: "sui_wallet", revokedAtMs: null }] });
    }
    if (pathname === "/v1/billing/sui-usdc/checkout") return Response.json({ checkoutUrl });
    return Response.json({ accepted: true });
  };
  const load = sourceLoader({ env, fetch });
  const backend = load("lib/server/promotion-backend.ts"), auth = load("lib/server/promotion-auth.ts");
  function request(name, path = "bootstrap", { method = "GET", body, headers = {}, query = "" } = {}) {
    return new Request(`${ORIGIN}/api/promotions/${path}${query}`, { method,
      headers: { origin: ORIGIN, ...(name ? { "x-mir2-promotion-season": PROFILES[name].id } : {}), ...(body ? { "content-type": "application/json" } : {}), ...headers },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
  }
  function run(name, fn, options = {}) { const req = request(name, options.path, options); return backend.withPromotionBackend(req, () => fn(req)); }
  function password(name) { return run(name, req => auth.promotionPassword(req), { path: "auth/password", method: "POST", body: { accountId: "existing", password: "synthetic-existing-password" } }); }
  async function approveAll() {
    const dubhe = load("lib/server/dubhe-connect-auth.ts");
    for (const [url, value] of relays) {
      const message = dubhe.buildDubheConnectLoginMessage(value), signed = await pair.signPersonalMessage(new TextEncoder().encode(message));
      relays.set(url, { ...value, status: "approved", result: { account: { address: pair.toSuiAddress(), publicKey: pair.getPublicKey().toBase64(), chains: [`sui:${value.request.network}`] },
        message, bytes: signed.bytes, signature: signed.signature, approvedAt: new Date().toISOString() } });
    }
  }
  return { env, calls, backend, auth, request, run, password, approveAll, load, setCheckoutUrl(value) { checkoutUrl = value; } };
}
function cookies(response) { return response.headers.getSetCookie().map(value => value.split(";")[0]).join("; "); }
function valueOfCookie(response, name) { return response.headers.getSetCookie().find(value => value.startsWith(name + "=")).split(";")[0].slice(name.length + 1); }
function cryptCookie(value, secret, purpose, scope = "", encrypt = false) {
  const key = createHash("sha256").update("mir2-promotion-cookie-v1\0").update(secret).digest();
  const bytes = encrypt ? randomBytes(12) : Buffer.from(value, "base64url");
  const cipher = encrypt ? createCipheriv("aes-256-gcm", key, bytes) : createDecipheriv("aes-256-gcm", key, bytes.subarray(0, 12));
  cipher.setAAD(Buffer.from(`mir2-promotion-v1\0${purpose}\0${ORIGIN}${scope}`));
  if (encrypt) {
    const ciphertext = Buffer.concat([cipher.update(JSON.stringify(value)), cipher.final()]);
    return Buffer.concat([bytes, cipher.getAuthTag(), ciphertext]).toString("base64url");
  }
  cipher.setAuthTag(bytes.subarray(12, 28));
  return JSON.parse(Buffer.concat([cipher.update(bytes.subarray(28)), cipher.final()]).toString("utf8"));
}

test("default Windows and explicit legacy seasons use only server-configured profiles", async () => {
  const f = fixture();
  for (const [name, query, expected] of [[undefined, "", "windows"], ["windows", "", "windows"], ["legacy", "", "legacy"],
    [undefined, `?campaignId=${PROFILES.windows.id}`, "windows"], [undefined, `?campaignId=${PROFILES.legacy.id}`, "legacy"]]) {
    const response = await f.run(name, async () => Response.json({ id: f.backend.promotionBackendId(), gateway: f.backend.promotionSetting("MIR2_PROMOTION_GATEWAY_HTTP_URL") }), { query });
    assert.deepEqual(await response.json(), { id: expected, gateway: PROFILES[expected].gateway });
  }
  delete f.env.MIR2_PROMOTION_WINDOWS_CAMPAIGN_ID;
  assert.equal((await (await f.run(undefined, async () => Response.json({ id: f.backend.promotionBackendId() }), { query: "?campaignId=older-existing-campaign" })).json()).id, "legacy");
});

test("malformed, duplicate, unknown and conflicting selectors fail before the handler", async () => {
  const f = fixture(); let calls = 0;
  for (const [query, headers, status] of [["?campaignId=a&campaignId=b", {}, 400], ["?campaignId=bad/path", {}, 400],
    ["?campaignId=unknown", {}, 503], [`?campaignId=${PROFILES.legacy.id}`, { "x-mir2-promotion-season": PROFILES.windows.id }, 400],
    ["", { "x-mir2-promotion-season": "" }, 400], ["", { "x-mir2-promotion-season": "https://attacker.invalid" }, 400]]) {
    const response = await f.run(undefined, async () => { calls++; return Response.json({}); }, { query, headers });
    assert.equal(response.status, status); assert.match(response.headers.get("cache-control"), /no-store/);
    assert.equal(response.headers.get("x-content-type-options"), "nosniff");
  }
  assert.equal(calls, 0); assert.equal(f.calls.length, 0);
});

test("Windows catalog and session-key misconfiguration fail closed without breaking explicit legacy", async () => {
  for (const change of [env => { delete env.MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS; }, env => { env.MIR2_PROMOTION_LEGACY_CAMPAIGN_IDS = PROFILES.windows.id; },
    env => { env.MIR2_PROMOTION_WINDOWS_SESSION_SECRET = "short"; }, env => { env.MIR2_PROMOTION_WINDOWS_SESSION_SECRET = PROFILES.legacy.session; },
    env => { env.MIR2_PROMOTION_WINDOWS_SESSION_SECRET = PROFILES.windows.secret; }]) {
    const f = fixture(); change(f.env); let entered = false;
    const response = await f.run(undefined, async () => { entered = true; return Response.json({}); });
    assert.equal(response.status, 503); assert.equal(entered, false); assert.equal(f.calls.length, 0);
  }
  const f = fixture(); f.env.MIR2_PROMOTION_WINDOWS_SESSION_SECRET = "short";
  assert.equal((await f.run("legacy", req => f.auth.promotionPassword(req), { method: "POST", body: { accountId: "existing", password: "synthetic" } })).status, 200);
});

test("interleaved and nested asynchronous scopes never mix settings, cookie names or checkout origins", async () => {
  const f = fixture(), before = JSON.stringify(f.env);
  const responses = await Promise.all(Array.from({ length: 32 }, (_, index) => {
    const name = index % 2 ? "windows" : "legacy";
    return f.run(name, async () => {
      const first = f.backend.promotionBackendId(); await new Promise(resolve => setImmediate(resolve));
      assert.equal(f.backend.promotionBackendId(), first);
      assert.equal(f.backend.promotionCheckoutOrigin(), PROFILES[name].checkout);
      const nested = await f.run(name === "windows" ? "legacy" : "windows", async () => { await Promise.resolve(); return Response.json({ id: f.backend.promotionBackendId() }); });
      assert.notEqual((await nested.json()).id, name); assert.equal(f.backend.promotionBackendId(), name);
      return Response.json({ name, cookie: f.backend.promotionCookieName(SESSION) });
    });
  }));
  for (const response of responses) { const body = await response.json(); assert.equal(body.cookie, body.name === "windows" ? WINDOWS_SESSION : SESSION); }
  await assert.rejects(f.run("windows", async () => { await Promise.resolve(); throw Error("synthetic failure"); }), /synthetic failure/);
  assert.equal(f.backend.promotionBackendId(), "legacy"); assert.equal(JSON.stringify(f.env), before);
});

test("session cookies retain the legacy format and isolate Windows names, keys and authenticated AAD", async () => {
  const f = fixture(), [legacy, windows] = await Promise.all([f.password("legacy"), f.password("windows")]);
  assert.equal(legacy.status, 200); assert.equal(windows.status, 200);
  const legacyToken = valueOfCookie(legacy, SESSION), windowsToken = valueOfCookie(windows, WINDOWS_SESSION);
  const plain = cryptCookie(legacyToken, PROFILES.legacy.session, "session");
  assert.equal(plain.accountId, "legacy-account");
  assert.equal(cryptCookie(windowsToken, PROFILES.windows.session, "session", "\0windows-v1").accountId, "windows-account");
  assert.throws(() => cryptCookie(windowsToken, PROFILES.windows.session, "session"));
  assert.throws(() => cryptCookie(windowsToken, PROFILES.windows.session, "challenge", "\0windows-v1"));
  for (const response of [legacy, windows]) {
    assert.deepEqual(await response.json(), { status: "approved" });
    assert.ok(response.headers.getSetCookie().every(value => /Path=\/;.*HttpOnly; Secure; SameSite=Strict/.test(value)));
    assert.ok(!cookies(response).includes("identity-token"));
  }
  const before = f.calls.length;
  const renamed = `${WINDOWS_SESSION}=${legacyToken}`;
  const bad = await f.run("windows", async req => { try { await f.auth.promotionSession(req); return Response.json({}); } catch (error) { return f.auth.promotionError(error); } }, { headers: { cookie: renamed } });
  assert.equal(bad.status, 401); assert.equal(f.calls.length, before);
  // An existing legacy cookie with the old AAD must still authenticate after this deployment.
  const oldToken = cryptCookie(plain, PROFILES.legacy.session, "session", "", true);
  const old = await f.run("legacy", async req => Response.json({ account: (await f.auth.promotionSession(req)).accountId }), { headers: { cookie: `${SESSION}=${oldToken}` } });
  assert.equal((await old.json()).account, "legacy-account");
});

test("logging out Windows revokes only its identity and clears only its own three cookies", async () => {
  const f = fixture(), [legacy, windows] = await Promise.all([f.password("legacy"), f.password("windows")]);
  const plain = cryptCookie(valueOfCookie(windows, WINDOWS_SESSION), PROFILES.windows.session, "session", "\0windows-v1");
  const result = await f.run("windows", req => f.auth.promotionLogout(req), { method: "POST", headers: { cookie: cookies(windows), "x-promotion-csrf": plain.csrfToken } });
  assert.equal(result.status, 200);
  const cleared = result.headers.getSetCookie(); assert.equal(cleared.length, 3);
  assert.ok(cleared.every(value => value.startsWith("__Host-mir2-promotion-windows-") && value.includes("Max-Age=0")));
  const revoke = f.calls.filter(call => call.url.endsWith("/v1/identity/sessions/revoke"));
  assert.equal(revoke.length, 1); assert.ok(revoke[0].url.startsWith(PROFILES.windows.gateway));
  const remaining = await f.run("legacy", async req => Response.json({ account: (await f.auth.promotionSession(req)).accountId }), { headers: { cookie: cookies(legacy) } });
  assert.equal((await remaining.json()).account, "legacy-account");
});

test("concurrent real wallet challenges and signature exchange keep profile signing contexts separate", async () => {
  const f = fixture(), before = JSON.stringify(f.env);
  const challenges = await Promise.all(Object.keys(PROFILES).map(name => f.run(name, req => f.auth.promotionChallenge(req), { method: "POST" })));
  for (const [index, name] of Object.keys(PROFILES).entries()) {
    assert.equal(challenges[index].status, 200);
    const value = await challenges[index].clone().json(); assert.ok(value.approvalUrl.startsWith(PROFILES[name].wallet));
    assert.equal(value.intentToken, undefined); assert.equal(value.token, undefined);
  }
  await f.approveAll();
  const approvals = await Promise.all(Object.keys(PROFILES).map((name, index) => f.run(name, req => f.auth.promotionPoll(req), { method: "POST", headers: { cookie: cookies(challenges[index]) } })));
  for (const [index, name] of Object.keys(PROFILES).entries()) {
    assert.equal(approvals[index].status, 200); assert.deepEqual(await approvals[index].json(), { status: "approved" });
    const context = await f.run(name, async () => { const value = f.load("lib/server/promotion-wallet-context.ts").promotionWalletContext();
      assert.ok(Object.isFrozen(value)); assert.ok(Object.isFrozen(value.allowedOrigins));
      assert.equal(value.secret, PROFILES[name].secret); assert.equal(value.relayOrigin, PROFILES[name].relay); return Response.json({ network: value.network }); });
    assert.equal((await context.json()).network, "testnet");
  }
  assert.equal(f.calls.filter(call => call.url.endsWith("/v1/promotions/auth/exchange")).length, 2);
  assert.equal(JSON.stringify(f.env), before);
});

test("hosted Windows wallet settings cannot inherit the healthy legacy signing context", async () => {
  for (const suffix of ["WALLET_AUTH_SECRET", "ALLOWED_ORIGINS", "DUBHE_CONNECT_NETWORK", "PUBLIC_ORIGIN"]) {
    const f = fixture(); delete f.env[`MIR2_PROMOTION_WINDOWS_${suffix}`];
    const response = await f.run("windows", req => f.auth.promotionChallenge(req), { method: "POST" });
    assert.equal(response.status, 503, suffix); assert.equal(f.calls.length, 0);
    const legacy = await f.run("legacy", req => f.auth.promotionChallenge(req), { method: "POST" });
    assert.equal(legacy.status, 200, suffix);
    assert.ok(f.calls.every(call => call.url.startsWith(PROFILES.legacy.relay)), suffix);
  }
});

test("real bootstrap marks the default Windows profile and keeps selected legacy seasons on the web game", async () => {
  const f = fixture(), api = f.load("lib/server/promotion-api.ts");
  for (const name of [undefined, "legacy", "windows"]) {
    const expected = name ?? "windows", before = f.calls.length;
    const response = await f.run(name, req => api.promotionBootstrap(req));
    assert.equal(response.status, 200);
    const body = await response.json(); assert.equal(body.gameClient, expected === "windows" ? "windows" : "web");
    assert.equal(body.authenticated, false);
    assert.equal(f.calls.length, before + 1);
    const target = new URL(f.calls.at(-1).url); assert.equal(target.origin, PROFILES[expected].gateway);
    assert.equal(target.searchParams.get("campaignId"), PROFILES[expected].id);
  }
});

test("billing uses the separately pinned payment origin while identity traffic stays on each API host", async () => {
  const f = fixture();
  await Promise.all(Object.keys(PROFILES).map(name => f.run(name, async () => {
    await f.auth.promotionGateway("/v1/billing/status", { query: "?characterIndex=0&requestId=1" });
    return Response.json(await f.auth.promotionGateway("/v1/promotions/public"));
  })));
  for (const name of Object.keys(PROFILES)) {
    const calls = f.calls.filter(call => call.url.startsWith(PROFILES[name].gateway)); assert.equal(calls.length, 2);
    assert.equal(calls[0].init.headers.get("origin"), PROFILES[name].checkout); assert.equal(calls[1].init.headers.get("origin"), null);
    assert.ok(calls.every(call => call.init.redirect === "error" && call.init.cache === "no-store"));
  }
  for (const value of ["http://billing-playtest.example.test", `${PROFILES.windows.checkout}/`, `${PROFILES.windows.checkout}/pay`, `${PROFILES.windows.checkout}?token=x`, "https://user:secret@billing-playtest.example.test"]) {
    f.env.MIR2_PROMOTION_WINDOWS_CHECKOUT_ORIGIN = value; const before = f.calls.length;
    await assert.rejects(f.run("windows", async () => { await f.auth.promotionGateway("/v1/billing/status"); return Response.json({}); }), /invalid_promotion_checkout_origin/);
    assert.equal(f.calls.length, before);
  }
});

test("Windows checkout accepts only the pinned billing host and fixed scoped capability, not its API host", async () => {
  const f = fixture(), billing = f.load("lib/server/promotion-billing.ts");
  const challenge = await f.run("windows", req => f.auth.promotionChallenge(req), { method: "POST" });
  await f.approveAll();
  const login = await f.run("windows", req => f.auth.promotionPoll(req), { method: "POST", headers: { cookie: cookies(challenge) } });
  assert.equal(login.status, 200);
  const session = cryptCookie(valueOfCookie(login, WINDOWS_SESSION), PROFILES.windows.session, "session", "\0windows-v1");
  const capability = `usdc_${"a".repeat(64)}.${"b".repeat(43)}`;
  const valid = `${PROFILES.windows.checkout}/v1/billing/sui-usdc/pay#${capability}`;
  for (const [url, status] of [[valid, 200], [valid.replace(PROFILES.windows.checkout, PROFILES.windows.gateway), 502],
    [valid.replace(PROFILES.windows.checkout, PROFILES.legacy.checkout), 502], [valid.replace("/pay#", "/other#"), 502],
    [valid.replace("/pay#", "/pay?token=x#"), 502], [valid.replace("https://", "https://user:secret@"), 502], [valid + "x", 502]]) {
    f.setCheckoutUrl(url);
    const response = await f.run("windows", req => billing.promotionBilling(req, "checkout"), { method: "POST", path: "billing/checkout",
      headers: { cookie: cookies(login), "x-promotion-csrf": session.csrfToken, "idempotency-key": "synthetic-checkout-request" },
      body: { characterIndex: 0, requestId: 1, offerId: "test_1000", orderRequestId: "synthetic-checkout-request" } });
    assert.equal(response.status, status);
    assert.ok(!JSON.stringify(await response.json()).includes(session.token));
  }
});

test("all eight real route wrappers carry the selected scope across awaited handlers and params", async () => {
  const f = fixture(), called = [];
  const handler = name => async (...args) => { await Promise.resolve(); called.push({ name, args, backend: backend.promotionBackendId() }); return Response.json({ backend: backend.promotionBackendId() }); };
  let backend;
  const load = sourceLoader({ env: f.env, overrides: spec => spec.endsWith("/promotion-api") ? { promotionBootstrap: handler("bootstrap"), promotionProxy: handler("proxy") } :
    spec.endsWith("/promotion-auth") ? Object.fromEntries(["Challenge", "Poll", "Password", "Logout"].map(name => [`promotion${name}`, handler(name)])) :
    spec.endsWith("/promotion-billing") ? { promotionBilling: handler("billing") } : spec.endsWith("/promotion-game") ? { promotionGameSession: handler("game") } : undefined });
  backend = load("lib/server/promotion-backend.ts");
  const routes = [["bootstrap", ["GET"]], ["auth/challenge", ["POST"]], ["auth/poll", ["POST"]], ["auth/password", ["POST"]], ["auth/logout", ["POST"]],
    ["game-session", ["POST"]], ["billing/[operation]", ["GET", "POST"]], ["[...path]", ["GET", "POST"]]];
  for (const [path, methods] of routes) for (const method of methods) {
    const route = load(`app/api/promotions/${path}/route.ts`);
    for (const name of Object.keys(PROFILES)) {
      const response = await route[method](f.request(name, path, { method }), { params: Promise.resolve({ operation: "status", path: ["me"] }) });
      assert.equal((await response.json()).backend, name, `${method} ${path}`);
    }
    const before = called.length;
    const invalid = await route[method](f.request(undefined, path, { method, headers: { "x-mir2-promotion-season": "unknown" } }), { params: Promise.resolve({ operation: "status", path: ["me"] }) });
    assert.equal(invalid.status, 503); assert.equal(called.length, before);
  }
  assert.ok(called.some(call => call.name === "proxy" && call.args[1][0] === "me"));
  assert.ok(called.some(call => call.name === "billing" && call.args[1] === "status"));
});

test("client auth, billing, dashboard and export requests keep season selection outside identity bodies", async () => {
  const calls = [], browser = { location: { search: `?campaignId=${PROFILES.legacy.id}` } };
  const fetch = async (url, init) => { calls.push({ url, init });
    const canonical = '{"integer":"18446744073709551615"}';
    return Response.json(String(url).includes("/export?") ? { snapshotCanonicalJson: canonical, snapshotSha256: createHash("sha256").update(canonical).digest("hex") } : { accepted: true }); };
  const load = sourceLoader({ fetch, window: browser }), model = load("app/promotions/promotion-model.ts");
  for (const [path, body] of [["auth/challenge", {}], ["auth/password", { accountId: "existing", password: "synthetic" }], ["payout-address", { address: "synthetic" }],
    ["billing/checkout", { characterIndex: 0, requestId: 1 }], ["members?offset=20&limit=20", undefined]]) {
    await model.promotionRequest(path, { body });
    const call = calls.at(-1); assert.equal(call.init.headers["x-mir2-promotion-season"], PROFILES.legacy.id);
    assert.equal(call.init.credentials, "same-origin"); assert.equal(call.init.cache, "no-store");
    if (body) assert.deepEqual(JSON.parse(call.init.body), body);
    else assert.equal(new URL(call.url, ORIGIN).searchParams.get("campaignId"), PROFILES.legacy.id);
  }
  const blob = await model.readPromotionSettlementExport("synthetic-settlement");
  assert.equal(await blob.text(), '{"integer":"18446744073709551615"}');
  assert.equal(calls.at(-1).init.headers["x-mir2-promotion-season"], PROFILES.legacy.id);
  for (const search of ["?campaignId=a&campaignId=b", "?campaignId=bad/path", "?campaignId="]) {
    browser.location.search = search; const before = calls.length;
    await assert.rejects(model.promotionRequest("auth/password", { body: {} })); assert.equal(calls.length, before);
  }
});
