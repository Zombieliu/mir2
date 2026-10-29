import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createHmac } from "node:crypto";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";

const require = createRequire(import.meta.url);
const GAME = { origin: "https://game.example.test", relay: "https://game-relay.example.test", wallet: "https://game-wallet.example.test", gateway: "https://game-gateway.example.test", secret: "synthetic-game-signing-secret-aaaaaaaaaaaaaaaaaaaa", network: "mainnet" };
const PROMO = { origin: "https://mir2.obelisk.build", relay: "https://promo-relay.example.test", wallet: "https://promo-wallet.example.test", gateway: "https://promo-gateway.example.test", secret: "synthetic-promotion-signing-secret-bbbbbbbbbbbbbbb", network: "testnet" };
const pair = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(51));
function fixture() {
  const env = { VERCEL: "1", VERCEL_ENV: "production", NODE_ENV: "production",
    MIR2_GATEWAY_HTTP_URL: GAME.gateway, MIR2_PASSKEY_AUTH_SECRET: GAME.secret, MIR2_PASSKEY_ALLOWED_ORIGINS: GAME.origin,
    MIR2_DUBHE_CONNECT_NETWORK: GAME.network, MIR2_DUBHE_CONNECT_RELAY_URL: GAME.relay, MIR2_DUBHE_WALLET_ORIGIN: GAME.wallet,
    MIR2_PROMOTION_GATEWAY_HTTP_URL: PROMO.gateway, MIR2_PROMOTION_GATEWAY_ALLOWED_ORIGINS: PROMO.gateway,
    MIR2_PROMOTION_WALLET_AUTH_SECRET: PROMO.secret, MIR2_PROMOTION_ALLOWED_ORIGINS: PROMO.origin,
    MIR2_PROMOTION_DUBHE_CONNECT_NETWORK: PROMO.network, MIR2_PROMOTION_DUBHE_CONNECT_RELAY_URL: PROMO.relay,
    MIR2_PROMOTION_DUBHE_WALLET_ORIGIN: PROMO.wallet, MIR2_PROMOTION_PUBLIC_ORIGIN: PROMO.origin,
    MIR2_PROMOTION_SESSION_SECRET: "synthetic-encrypted-cookie-secret-cccccccccccccccc" };
  const modules = new Map(), calls = [], sessions = new Map(); let sequence = 0, onFetch;
  const mockProcess = { env: new Proxy(env, { set() { throw Error("application must not mutate process.env"); }, deleteProperty() { throw Error("application must not delete process.env"); } }) };
  const defaultFetch = async (url, init) => {
    const target = String(url); calls.push({ url: target, init });
    if (onFetch) await onFetch(target, init);
    if ([GAME.relay + "/sessions", PROMO.relay + "/sessions"].includes(target)) {
      const request = JSON.parse(init.body), id = `synthetic-${++sequence}`, relay = new URL(target).origin, issuedAt = Date.now();
      const value = { id, relayOrigin: relay, approvalUrl: `${request.walletOrigin}/connect/approve?session=${id}&relay=${encodeURIComponent(relay)}`,
        pollUrl: `${relay}/sessions/${id}`, status: "pending", request: { appName: request.appName, origin: request.origin, network: request.network,
          walletOrigin: request.walletOrigin, statement: request.statement, nonce: `nonce-${id}`, requestedAt: new Date(issuedAt).toISOString(), expiresAt: new Date(issuedAt + 120000).toISOString() } };
      sessions.set(value.pollUrl, value); return Response.json(value);
    }
    if (sessions.has(target)) return Response.json(sessions.get(target));
    if (target === `${PROMO.gateway}/v1/promotions/auth/exchange`) {
      const body = JSON.parse(init.body); assert.ok(hmacValid(body.credential, PROMO.secret)); assert.ok(!hmacValid(body.credential, GAME.secret));
      return Response.json({ token: "synthetic-private-identity-bearer", session: { accountId: "synthetic-account", sessionId: "synthetic-identity", expiresAtMs: Date.now() + 3600000, authMethod: "sui_wallet", issuedAtMs: Date.now() } });
    }
    return Response.json({ accepted: true });
  };
  function load(name) {
    if (modules.has(name)) return modules.get(name);
    const file = new URL(`../lib/server/${name}.ts`, import.meta.url);
    const compiled = ts.transpileModule(readFileSync(file, "utf8"), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
    const module = { exports: {} };
    vm.runInNewContext(compiled, { module, exports: module.exports, process: mockProcess, Buffer, TextEncoder, Request, Response, Headers, URL, AbortSignal,
      fetch: defaultFetch, require: spec => spec === "server-only" ? {} : spec.startsWith("./") ? load(spec.slice(2)) : require(spec) }, { filename: fileURLToPath(file) });
    modules.set(name, module.exports); return module.exports;
  }
  const dubhe = load("dubhe-connect-auth"), auth = load("promotion-auth"), context = load("promotion-wallet-context"), passkey = load("passkey-auth");
  async function approveAll() {
    for (const [url, value] of sessions) {
      const message = dubhe.buildDubheConnectLoginMessage(value), signed = await pair.signPersonalMessage(new TextEncoder().encode(message));
      sessions.set(url, { ...value, status: "approved", result: { account: { address: pair.toSuiAddress(), publicKey: pair.getPublicKey().toBase64(), chains: [`sui:${value.request.network}`] },
        message, bytes: signed.bytes, signature: signed.signature, approvedAt: new Date().toISOString() } });
    }
  }
  return { env, calls, sessions, dubhe, auth, context, passkey, approveAll, setOnFetch(fn) { onFetch = fn; } };
}
function request(origin, token, extra = {}) { return new Request(`${origin}/api/wallet`, { method: "POST", headers: { origin, ...(token ? { authorization: `Bearer ${token}` } : {}), ...extra } }); }
function hmacValid(token, secret) { const [payload, signature] = token.split("."); return createHmac("sha256", secret).update(payload).digest("base64url") === signature; }
function intentHmacValid(token, secret) { const [payload, signature] = token.split("."); return createHmac("sha256", secret).update(`mir2-dubhe-connect-intent-v1\0${payload}`).digest("base64url") === signature; }

test("promotion configuration is immutable and separate from the existing game defaults", () => {
  const f = fixture(), context = f.context.promotionWalletContext();
  assert.ok(Object.isFrozen(context)); assert.ok(Object.isFrozen(context.allowedOrigins));
  assert.equal(context.network, "testnet"); assert.equal(context.relayOrigin, PROMO.relay); assert.equal(context.walletOrigin, PROMO.wallet);
  assert.ok(context.secret === PROMO.secret && context.secret !== GAME.secret);
  assert.equal(f.context.promotionWalletNetwork(), "testnet");
  assert.ok(f.passkey.passkeyGatewaySecret() === GAME.secret);
});

test("Vercel rejects each missing dedicated wallet setting without using game config", () => {
  for (const name of ["MIR2_PROMOTION_WALLET_AUTH_SECRET", "MIR2_PROMOTION_ALLOWED_ORIGINS", "MIR2_PROMOTION_DUBHE_CONNECT_NETWORK", "MIR2_PROMOTION_PUBLIC_ORIGIN"]) {
    const f = fixture(); delete f.env[name]; assert.throws(() => f.context.promotionWalletContext(), undefined, name); assert.equal(f.calls.length, 0);
  }
});

test("extra allowed origins and relay/wallet settings must be exact canonical secure origins", () => {
  for (const value of [`${PROMO.origin},https://extra.example.test/path`, `${PROMO.origin},https://extra.example.test/`, `${PROMO.origin},https://a@extra.example.test`, `${PROMO.origin},`, `${PROMO.origin},${PROMO.origin}`]) {
    const f = fixture(); f.env.MIR2_PROMOTION_ALLOWED_ORIGINS = value; assert.throws(() => f.context.promotionWalletContext());
  }
  const f = fixture(); f.env.MIR2_PROMOTION_ALLOWED_ORIGINS += ",https://extra.example.test"; assert.equal(f.context.promotionWalletContext().allowedOrigins.length, 2);
  for (const name of ["MIR2_PROMOTION_DUBHE_CONNECT_RELAY_URL", "MIR2_PROMOTION_DUBHE_WALLET_ORIGIN"]) {
    const invalid = fixture(); invalid.env[name] = "https://trusted.example.test/path"; assert.throws(() => invalid.context.promotionWalletContext());
  }
});

test("concurrent promotion and game Dubhe flows use different secret/network/origin end to end", async () => {
  const f = fixture(), context = f.context.promotionWalletContext(), envBefore = JSON.stringify(f.env);
  const [game, promo] = await Promise.all([f.dubhe.createDubheConnectLogin(request(GAME.origin)), f.dubhe.createDubheConnectLogin(request(PROMO.origin), context)]);
  assert.equal(game.status, 200); assert.equal(promo.status, 200);
  const g = await game.json(), p = await promo.json();
  assert.ok(intentHmacValid(g.intentToken, GAME.secret) && !intentHmacValid(g.intentToken, PROMO.secret));
  assert.ok(intentHmacValid(p.intentToken, PROMO.secret) && !intentHmacValid(p.intentToken, GAME.secret));
  assert.match(g.approvalUrl, /^https:\/\/game-wallet/); assert.match(p.approvalUrl, /^https:\/\/promo-wallet/);
  await f.approveAll();
  const [gr, pr] = await Promise.all([f.dubhe.readDubheConnectLogin(request(GAME.origin, g.intentToken)), f.dubhe.readDubheConnectLogin(request(PROMO.origin, p.intentToken), context)]);
  assert.equal(gr.status, 200); assert.equal(pr.status, 200);
  const gt = (await gr.json()).login.token, pt = (await pr.json()).login.token;
  assert.ok(hmacValid(gt, GAME.secret) && !hmacValid(gt, PROMO.secret)); assert.ok(hmacValid(pt, PROMO.secret) && !hmacValid(pt, GAME.secret));
  assert.equal(JSON.stringify(f.env), envBefore);
});

test("promotion intent cannot be replayed through the game verifier or another allowed origin", async () => {
  const f = fixture(); f.env.MIR2_PROMOTION_ALLOWED_ORIGINS += ",https://extra.example.test";
  const context = f.context.promotionWalletContext(), p = await (await f.dubhe.createDubheConnectLogin(request(PROMO.origin), context)).json(); await f.approveAll();
  assert.equal((await f.dubhe.readDubheConnectLogin(request(GAME.origin, p.intentToken))).status, 401);
  assert.equal((await f.dubhe.readDubheConnectLogin(request("https://extra.example.test", p.intentToken), context)).status, 401);
});

test("one promotion request keeps its configuration snapshot across asynchronous relay work", async () => {
  const f = fixture(), context = f.context.promotionWalletContext();
  f.setOnFetch(async () => { f.env.MIR2_PROMOTION_WALLET_AUTH_SECRET = "different-synthetic-secret-after-request-start"; f.env.MIR2_PROMOTION_DUBHE_CONNECT_NETWORK = "mainnet"; f.env.MIR2_PROMOTION_DUBHE_CONNECT_RELAY_URL = "https://changed.invalid"; });
  const created = await f.dubhe.createDubheConnectLogin(request(PROMO.origin), context); assert.equal(created.status, 200);
  const p = await created.json(); assert.ok(intentHmacValid(p.intentToken, PROMO.secret)); await f.approveAll();
  const result = await f.dubhe.readDubheConnectLogin(request(PROMO.origin, p.intentToken), context); assert.equal(result.status, 200); assert.ok(hmacValid((await result.json()).login.token, PROMO.secret));
  assert.ok(f.calls.every(v => !v.url.startsWith("https://changed.invalid")));
});

test("promotion BFF challenge/poll uses its context and private Gateway without leaking a bearer", async () => {
  const f = fixture(), before = JSON.stringify(f.env);
  const created = await f.auth.promotionChallenge(request(PROMO.origin)); assert.equal(created.status, 200);
  const payload = await created.json(); assert.equal(payload.intentToken, undefined); assert.equal(payload.token, undefined);
  const cookie = created.headers.getSetCookie().map(v => v.split(";")[0]).join("; "); await f.approveAll();
  const approved = await f.auth.promotionPoll(request(PROMO.origin, undefined, { cookie })); assert.equal(approved.status, 200);
  assert.deepEqual(await approved.json(), { status: "approved" });
  assert.ok(f.calls.some(v => v.url === `${PROMO.gateway}/v1/promotions/auth/exchange`));
  assert.ok(!f.calls.some(v => v.url.startsWith(GAME.gateway) || v.url.startsWith(GAME.relay)));
  assert.ok(!approved.headers.getSetCookie().join("").includes("synthetic-private-identity-bearer")); assert.equal(JSON.stringify(f.env), before);
});

test("Vercel Gateway fallback and unconfigured public origin fail closed before I/O", async () => {
  const f = fixture(); delete f.env.MIR2_PROMOTION_GATEWAY_HTTP_URL;
  await assert.rejects(f.auth.promotionGateway("/v1/promotions/public"), /promotion_gateway_not_configured/);
  assert.equal((await f.auth.promotionChallenge(request(PROMO.origin))).status, 503); assert.equal(f.calls.length, 0);
  f.env.MIR2_PROMOTION_GATEWAY_HTTP_URL = PROMO.gateway; delete f.env.MIR2_PROMOTION_PUBLIC_ORIGIN;
  assert.equal((await f.auth.promotionChallenge(request(PROMO.origin))).status, 503); assert.equal(f.calls.length, 0);
});

test("local legacy fallback remains available without changing normal game signing", async () => {
  const f = fixture(); delete f.env.VERCEL; delete f.env.VERCEL_ENV;
  for (const name of Object.keys(f.env)) if (name.startsWith("MIR2_PROMOTION_") && name !== "MIR2_PROMOTION_SESSION_SECRET") delete f.env[name];
  const context = f.context.promotionWalletContext(); assert.equal(context.network, "mainnet"); assert.ok(context.secret === GAME.secret);
  await f.auth.promotionGateway("/v1/promotions/public"); assert.equal(f.calls.at(-1).url, `${GAME.gateway}/v1/promotions/public`);
});

test("browser bodies/headers cannot choose signing context or override the configured Gateway", async () => {
  const f = fixture();
  const attempted = new Request(`${PROMO.origin}/api/promotions/auth/challenge`, { method: "POST", headers: { origin: PROMO.origin, "content-type": "application/json", "x-wallet-auth-secret": "attacker", "x-forwarded-host": "evil.example" }, body: JSON.stringify({ context: { secret: "attacker", network: "mainnet" } }) });
  assert.equal((await f.auth.promotionChallenge(attempted)).status, 400); assert.equal(f.calls.length, 0);
  const wrong = request("https://evil.example", undefined, { "x-forwarded-host": "mir2.obelisk.build" }); assert.equal((await f.auth.promotionChallenge(wrong)).status, 403);
  const invalid = { ...f.context.promotionWalletContext(), secret: "short" }; assert.equal((await f.dubhe.createDubheConnectLogin(request(PROMO.origin), invalid)).status, 503); assert.equal(f.calls.length, 0);
});

test("shared passkey functions use the optional context only when explicitly supplied", () => {
  const f = fixture(), context = f.context.promotionWalletContext(), payload = { auth: "mir2-login-challenge-v1", jti: "synthetic", address: pair.toSuiAddress(), origin: PROMO.origin, iatMs: Date.now(), expMs: Date.now() + 10000, authMethod: "wallet" };
  const game = f.passkey.signPasskeyChallenge(payload), promo = f.passkey.signPasskeyChallenge(payload, context);
  assert.ok(hmacValid(game, GAME.secret)); assert.ok(hmacValid(promo, PROMO.secret));
  assert.throws(() => f.passkey.verifyPasskeyChallenge(promo)); assert.equal(f.passkey.verifyPasskeyChallenge(promo, context).jti, "synthetic");
  assert.equal(f.passkey.passkeyRequestOriginAllowed(PROMO.origin, PROMO.origin), false); assert.equal(f.passkey.passkeyRequestOriginAllowed(PROMO.origin, PROMO.origin, context), true);
});

test("owned authentication modules pass a fresh strict TypeScript check", () => {
  const paths = ["promotion-auth", "promotion-wallet-context", "dubhe-connect-auth", "passkey-auth"].map(v => fileURLToPath(new URL(`../lib/server/${v}.ts`, import.meta.url)));
  const program = ts.createProgram(paths, { noEmit: true, strict: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, skipLibCheck: true, types: ["node"], lib: ["lib.es2022.d.ts", "lib.dom.d.ts", "lib.dom.iterable.d.ts"] });
  const diagnostics = ts.getPreEmitDiagnostics(program).filter(v => v.category === ts.DiagnosticCategory.Error);
  assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, { getCanonicalFileName: p => p, getCurrentDirectory: () => process.cwd(), getNewLine: () => "\n" }));
});
