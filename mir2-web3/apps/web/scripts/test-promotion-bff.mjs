import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { after, beforeEach, test } from "node:test";
import vm from "node:vm";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import { Ed25519Keypair } from "@mysten/sui/keypairs/ed25519";

const require = createRequire(import.meta.url);
const origin = "https://promotion.example.test";
const relay = "https://relay.example.test";
const wallet = "https://wallet.example.test";
const gateway = "http://127.0.0.1:7210";
const variables = ["NODE_ENV", "MIR2_GATEWAY_HTTP_URL", "MIR2_PROMOTION_GATEWAY_ALLOWED_ORIGINS", "MIR2_PROMOTION_SESSION_SECRET",
  "MIR2_PROMOTION_CAMPAIGN_ID", "MIR2_PROMOTION_PUBLIC_ORIGIN",
  "MIR2_PASSKEY_AUTH_SECRET", "MIR2_PASSKEY_ALLOWED_ORIGINS", "MIR2_ALLOWED_WEB_ORIGINS", "MIR2_DUBHE_CONNECT_RELAY_URL",
  "MIR2_DUBHE_WALLET_ORIGIN", "MIR2_DUBHE_CONNECT_NETWORK"];
const original = Object.fromEntries(variables.map((name) => [name, process.env[name]]));
after(() => { for (const name of variables) { if (original[name] === undefined) delete process.env[name]; else process.env[name] = original[name]; } });
const keypair = Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(17));
let now, pending, relayResult, requests, consumed, revoked, handler, publicData, mine, passwordSession, walletLinked;
const modules = new Map();
function load(path) {
  if (modules.has(path)) return modules.get(path);
  const compiled = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, {
    module, exports: module.exports, Buffer, TextEncoder, Response, Request, Headers, URL, AbortSignal, process,
    Date: class extends Date { static now() { return now; } },
    fetch: async (url, init) => { requests.push({ url, init }); return handler(url, init); },
    require: (name) => {
      if (name === "server-only") return {};
      if (name.startsWith("./")) return load(`../lib/server/${name.slice(2)}.ts`);
      return require(name);
    },
  }, { filename: path });
  modules.set(path, module.exports);
  return module.exports;
}
const auth = load("../lib/server/promotion-auth.ts");
const api = load("../lib/server/promotion-api.ts");
const dubhe = load("../lib/server/dubhe-connect-auth.ts");
function req(path, { method = "POST", cookie, csrf, body, headers = {} } = {}) {
  return new Request(`${origin}/api/promotions/${path}`, {
    method, headers: { origin, ...(cookie ? { cookie } : {}), ...(csrf ? { "x-promotion-csrf": csrf } : {}),
      ...(body === undefined ? {} : { "content-type": "application/json" }), ...headers },
    ...(body === undefined ? {} : { body: typeof body === "string" ? body : JSON.stringify(body) }),
  });
}
function cookies(response) { return response.headers.getSetCookie().filter((entry) => !entry.includes("Max-Age=0")).map((entry) => entry.split(";")[0]).join("; "); }
async function challenge(existingCookie) {
  const response = await auth.promotionChallenge(req("auth/challenge", { cookie: existingCookie }));
  assert.equal(response.status, 200, JSON.stringify(await response.clone().json()));
  return { response, cookie: cookies(response), payload: await response.json() };
}
async function approve() {
  const message = dubhe.buildDubheConnectLoginMessage(pending);
  const signed = await keypair.signPersonalMessage(new TextEncoder().encode(message));
  relayResult = { ...pending, status: "approved", result: {
    account: { address: keypair.toSuiAddress(), publicKey: keypair.getPublicKey().toBase64(), chains: ["sui:testnet"] },
    message, bytes: signed.bytes, signature: signed.signature, approvedAt: new Date(now).toISOString(),
  } };
}
async function login() {
  const created = await challenge(); await approve();
  const response = await auth.promotionPoll(req("auth/poll", { cookie: created.cookie }));
  assert.equal(response.status, 200, JSON.stringify(await response.clone().json()));
  assert.deepEqual(await response.json(), { status: "approved" });
  return { cookie: cookies(response), challengeCookie: created.cookie, response };
}
beforeEach(() => {
  delete process.env.MIR2_PROMOTION_PUBLIC_ORIGIN;
  now = Date.now(); requests = []; consumed = false; revoked = false; passwordSession = false; walletLinked = true;
  Object.assign(process.env, { NODE_ENV: "production", MIR2_GATEWAY_HTTP_URL: gateway,
    MIR2_PROMOTION_CAMPAIGN_ID: "preview-20261029",
    MIR2_PROMOTION_GATEWAY_ALLOWED_ORIGINS: gateway, MIR2_PROMOTION_SESSION_SECRET: "promotion-separate-test-key-with-64-characters-aaaaaaaaaaaaaaaaaaaa",
    MIR2_PASSKEY_AUTH_SECRET: "wallet-signing-test-key-with-more-than-32-characters", MIR2_PASSKEY_ALLOWED_ORIGINS: origin,
    MIR2_DUBHE_CONNECT_RELAY_URL: relay, MIR2_DUBHE_WALLET_ORIGIN: wallet, MIR2_DUBHE_CONNECT_NETWORK: "testnet" });
  pending = { id: "connect-session-17", relayOrigin: relay, approvalUrl: `${wallet}/connect/approve?session=connect-session-17&relay=${encodeURIComponent(relay)}`,
    pollUrl: `${relay}/sessions/connect-session-17`, status: "pending", request: { appName: "Mir2 Web3", origin,
      network: "testnet", walletOrigin: wallet, statement: "Sign in to Mir2 Web3 with Dubhe Wallet.", nonce: "relay-authoritative-nonce",
      requestedAt: new Date(now).toISOString(), expiresAt: new Date(now + 120000).toISOString() } };
  relayResult = pending;
  publicData = { campaign: null, sourceGaps: ["campaign_not_approved"], realPayoutsEnabled: false };
  mine = { characters: [{ index: 0, name: "MyWarrior", level: 30 }], leader: null, dashboard: { qualified: 0 },
    permissions: { isLeader: false, isAdmin: false }, sourceGaps: ["qualified_revenue_not_connected"], dataMode: "real" };
  handler = async (url, init) => {
    const parsed = new URL(url);
    const endpoint = parsed.origin + parsed.pathname;
    assert.equal(init.cache, "no-store"); assert.equal(init.redirect, "error"); assert.ok(init.signal);
    if (url === `${relay}/sessions` && init.method === "POST") return Response.json(pending);
    if (url === pending.pollUrl) return Response.json(relayResult);
    if (url === `${gateway}/v1/promotions/auth/password`) {
      assert.deepEqual(JSON.parse(init.body), { accountId: "old-account", password: "correct-password" });
      passwordSession = true; walletLinked = false;
      return Response.json({ token: "server-only-identity-bearer", session: { accountId: "canonical-player-17", sessionId: "identity-17",
        expiresAtMs: now + 3600000, authMethod: "password", issuedAtMs: now } });
    }
    if (url === `${gateway}/v1/promotions/auth/exchange`) {
      const value = JSON.parse(init.body);
      assert.deepEqual(Object.keys(value).sort(), ["credential", "provider"]);
      assert.equal(value.provider, "suiWallet");
      if (consumed) return Response.json({ error: "proof_already_consumed" }, { status: 401 });
      consumed = true;
      if (passwordSession) { assert.equal(new Headers(init.headers).get("authorization"), "Bearer server-only-identity-bearer"); walletLinked = true; }
      return Response.json({ token: "server-only-identity-bearer", session: { accountId: "canonical-player-17", sessionId: "identity-17", expiresAtMs: now + 3600000,
        authMethod: "sui_wallet", issuedAtMs: now } });
    }
    if (["public", "invite", "me", "members", "settlements", "admin"].some((route) => parsed.pathname === `/v1/promotions/${route}`) && init.method !== "POST") {
      assert.equal(parsed.searchParams.get("campaignId"), "preview-20261029");
      const allowed = parsed.pathname.endsWith("/members") ? ["campaignId", "offset", "limit"] : parsed.pathname.endsWith("/invite") ? ["campaignId", "code"] : ["campaignId"];
      assert.ok([...parsed.searchParams.keys()].every((key) => allowed.includes(key)));
    }
    if (endpoint === `${gateway}/v1/promotions/public`) return Response.json(publicData);
    if (endpoint === `${gateway}/v1/promotions/invite`) return Response.json({ campaignId: "preview-20261029", leaderDisplayName: "Leader", code: parsed.searchParams.get("code") });
    if (endpoint === `${gateway}/v1/promotions/click`) {
      assert.deepEqual(JSON.parse(init.body), { campaignId: "preview-20261029", code: "leader-code-A" });
      return Response.json({ clickId: "anonymous-click-17", expiresAtMs: now + 7 * 24 * 60 * 60 * 1000, leaderDisplayName: "Leader A",
        recruitmentEnabled: false, unrewardedPlayAvailable: true });
    }
    assert.equal(new Headers(init.headers).get("authorization"), "Bearer server-only-identity-bearer");
    if (url === `${gateway}/v1/identity/me`) return revoked ? Response.json({ error: "revoked" }, { status: 401 }) :
      Response.json({ accountId: "canonical-player-17", currentSessionId: "identity-17", sessions:[{sessionId:"identity-17",authMethod: consumed ? "sui_wallet" : "password",credentialId: consumed ? "wallet-credential-17" : "password-credential-17"}], credentials: walletLinked ?
        [{ credentialId: "wallet-credential-17", credentialKind: "sui_wallet", revokedAtMs: null, credentialSubject: "0x123…456" }] : [] });
    if (url === `${gateway}/v1/identity/sessions/revoke`) { revoked = true; return Response.json({ accepted: true }); }
    if (endpoint === `${gateway}/v1/promotions/me`) return Response.json(mine);
    return Response.json({ accepted: true });
  };
});

test("configured public origin binds CSRF through an internal Next URL without trusting proxy headers", () => {
  process.env.MIR2_PROMOTION_PUBLIC_ORIGIN = origin;
  assert.equal(auth.promotionBrowserRequest(new Request("http://localhost:7359/api/promotions/auth/challenge", {
    method: "POST", headers: { origin },
  }), true), origin);
  assert.throws(() => auth.promotionBrowserRequest(new Request("http://localhost:7359/api/promotions/auth/challenge", {
    method: "POST", headers: { origin: "https://attacker.invalid", "x-forwarded-host": "promotion.example.test" },
  }), true), /cross_origin_request/);
  process.env.MIR2_PROMOTION_PUBLIC_ORIGIN = `${origin}/unexpected-path`;
  assert.throws(() => auth.promotionBrowserRequest(req("auth/challenge"), true), /invalid_public_origin/);
});

test("actual Dubhe signed challenge and gateway exchange keep all bearers in encrypted host cookies", async () => {
  const created = await challenge();
  assert.equal(created.payload.intentToken, undefined); assert.equal(created.payload.token, undefined);
  assert.equal(created.payload.approvalUrl, pending.approvalUrl);
  assert.match(created.response.headers.get("set-cookie"), /HttpOnly; Secure; SameSite=Strict/);
  assert.ok(!created.cookie.includes("intentToken"));
  const pendingResponse = await auth.promotionPoll(req("auth/poll", { cookie: created.cookie }));
  assert.deepEqual(await pendingResponse.json(), { status: "pending" });
  await approve();
  const approved = await auth.promotionPoll(req("auth/poll", { cookie: created.cookie }));
  assert.deepEqual(await approved.json(), { status: "approved" });
  assert.ok(!cookies(approved).includes("server-only-identity-bearer"));
  const boot = await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie: cookies(approved) }));
  const data = await boot.json(); assert.equal(data.authenticated, true); assert.equal(data.characters[0].name, "MyWarrior");
  assert.equal(data.csrfToken.length, 43); assert.equal(data.me.dataMode, "real"); assert.deepEqual(data.permissions, []);
  assert.equal(data.binding.status, "linked"); assert.equal(data.binding.credentialId, "wallet-credential-17");
  assert.ok(!JSON.stringify(data).includes("server-only-identity-bearer")); assert.match(boot.headers.get("cache-control"), /no-store/);
});
test("different browser, cross-origin, injected auth and tampered challenge do not exchange credentials", async () => {
  const created = await challenge(); await approve();
  assert.equal((await auth.promotionPoll(req("auth/poll"))).status, 401);
  assert.equal((await auth.promotionPoll(req("auth/poll", { cookie: created.cookie, headers: { origin: "https://evil.test" } }))).status, 403);
  assert.equal((await auth.promotionPoll(req("auth/poll", { cookie: created.cookie, headers: { authorization: "Bearer injected" } }))).status, 400);
  assert.equal((await auth.promotionPoll(req("auth/poll", { cookie: created.cookie.slice(0, -8) + "AAAAAAAA" }))).status, 401);
  assert.equal(consumed, false);
});
test("expired, replayed and concurrently duplicated login challenge never create a second session", async () => {
  const created = await challenge(); await approve();
  const results = await Promise.all([auth.promotionPoll(req("auth/poll", { cookie: created.cookie })), auth.promotionPoll(req("auth/poll", { cookie: created.cookie }))]);
  assert.deepEqual(results.map((r) => r.status).sort(), [200, 401]);
  assert.equal((await auth.promotionPoll(req("auth/poll", { cookie: created.cookie }))).status, 401);
  now += 121000;
  const expired = await auth.promotionPoll(req("auth/poll", { cookie: created.cookie }));
  assert.deepEqual(await expired.json(), { status: "expired" }); assert.match(expired.headers.get("set-cookie"), /Max-Age=0/);
});
test("cookies are site-bound and a lost or reused signing secret fails closed", async () => {
  const created = await challenge();
  const crossSite = new Request("https://other.example.test/api/promotions/auth/poll", { method: "POST", headers: { origin: "https://other.example.test", cookie: created.cookie } });
  assert.equal((await auth.promotionPoll(crossSite)).status, 401);
  delete process.env.MIR2_PROMOTION_SESSION_SECRET;
  assert.equal((await auth.promotionChallenge(req("auth/challenge"))).status, 503);
  process.env.MIR2_PROMOTION_SESSION_SECRET = process.env.MIR2_PASSKEY_AUTH_SECRET;
  assert.equal((await auth.promotionChallenge(req("auth/challenge"))).status, 503);
});
test("revocation and expiry are checked against the authoritative identity service on every use", async () => {
  const { cookie } = await login();
  assert.equal((await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).status, 200);
  revoked = true;
  assert.equal((await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).status, 401);
  revoked = false; now += 3600001;
  assert.equal((await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).status, 401);
});
test("anonymous bootstrap has no fabricated characters or campaign, authenticated writes need CSRF and idempotency", async () => {
  const anon = await api.promotionBootstrap(req("bootstrap", { method: "GET" }));
  const anonymous = await anon.json(); assert.equal(anonymous.authenticated, false); assert.equal(anonymous.campaign, null); assert.deepEqual(anonymous.characters, []);
  const { cookie } = await login();
  const boot = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).json();
  const call = (extra = {}) => api.promotionProxy(req("apply", { cookie, body: { characterIndex: 0, termsVersion: "draft" }, ...extra }), ["apply"]);
  assert.equal((await call()).status, 403);
  assert.equal((await call({ csrf: boot.csrfToken })).status, 400);
  assert.equal((await call({ csrf: boot.csrfToken, headers: { "Idempotency-Key": "request-1234567890" } })).status, 200);
  assert.equal((await call({ csrf: boot.csrfToken, headers: { "Idempotency-Key": "request-1234567890", origin: "https://evil.test" } })).status, 403);
});
test("identity injection, unknown routes and query-based SSRF are rejected before forwarding", async () => {
  const { cookie } = await login();
  const { csrfToken } = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).json();
  for (const injected of [{ account_id: "victim" }, { nested: { playerId: "victim" } }, { token: "operator" }]) {
    const response = await api.promotionProxy(req("apply", { cookie, csrf: csrfToken, body: injected, headers: { "Idempotency-Key": "request-1234567890" } }), ["apply"]);
    assert.equal(response.status, 400);
  }
  assert.equal((await api.promotionProxy(req("admin/../me", { method: "GET", cookie }), ["admin", "..", "me"])).status, 404);
  assert.equal((await api.promotionProxy(req("public?url=http://169.254.169.254", { method: "GET" }), ["public"])).status, 400);
  assert.equal((await api.promotionProxy(req("members?accountId=victim", { method: "GET", cookie }), ["members"])).status, 400);
});
test("logout revokes the exact current identity and clears challenge and session cookies", async () => {
  const { cookie } = await login();
  const { csrfToken } = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).json();
  assert.equal((await auth.promotionLogout(req("auth/logout", { cookie }))).status, 403);
  const response = await auth.promotionLogout(req("auth/logout", { cookie, csrf: csrfToken }));
  assert.equal(response.status, 200); assert.equal(revoked, true); assert.equal(response.headers.getSetCookie().length, 3);
  assert.ok(response.headers.getSetCookie().every((entry) => entry.includes("Max-Age=0")));
  const revoke = requests.find((entry) => entry.url.endsWith("/sessions/revoke"));
  assert.deepEqual(JSON.parse(revoke.init.body), { sessionId: "identity-17", reason: "promotion_logout" });
});
test("gateway destinations and upstream body sizes are bounded and sensitive fields are not proxied", async () => {
  process.env.MIR2_GATEWAY_HTTP_URL = "http://169.254.169.254/latest/meta-data";
  assert.equal((await auth.promotionChallenge(req("auth/challenge"))).status, 503); assert.equal(requests.length, 0);
  process.env.MIR2_GATEWAY_HTTP_URL = gateway;
  const { cookie } = await login();
  mine.token = "must-never-leak"; mine.credentials = [{ privateKey: "also-never-leak" }];
  const response = await api.promotionProxy(req("me", { method: "GET", cookie }), ["me"]);
  assert.ok(!(await response.text()).includes("never-leak"));
  handler = async () => Response.json({ oversized: "x".repeat(1048577) });
  assert.equal((await api.promotionProxy(req("public", { method: "GET" }), ["public"])).status, 502);
});

test("existing account password proof plus an actual wallet signature binds through the exact server-held identity", async () => {
  const verified = await auth.promotionPassword(req("auth/password", { body: { accountId: "old-account", password: "correct-password" } }));
  assert.equal(verified.status, 200); const oldCookie = cookies(verified);
  assert.ok(!oldCookie.includes("password")); assert.ok(!(await verified.text()).includes("bearer"));
  const boot = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie: oldCookie }))).json();
  assert.equal(boot.binding.status, "pending");
  const created = await challenge(oldCookie); await approve();
  const result = await auth.promotionPoll(req("auth/poll", { cookie: `${created.cookie}; ${oldCookie}` }));
  assert.equal(result.status, 200); assert.equal(walletLinked, true);
  const updated = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie: cookies(result) }))).json();
  assert.equal(updated.binding.status, "linked");
});

test("old account binding cannot silently switch accounts when password proof expires or disappears", async () => {
  const verified = await auth.promotionPassword(req("auth/password", { body: { accountId: "old-account", password: "correct-password" } }));
  const oldCookie = cookies(verified);
  const created = await challenge(oldCookie); await approve();
  const result = await auth.promotionPoll(req("auth/poll", { cookie: created.cookie }));
  assert.equal(result.status, 401); assert.equal(consumed, false);
  now += 601000;
  pending.request.requestedAt = new Date(now).toISOString(); pending.request.expiresAt = new Date(now + 120000).toISOString(); relayResult = pending;
  const newer = await challenge(oldCookie); await approve();
  const stale = await auth.promotionPoll(req("auth/poll", { cookie: `${newer.cookie}; ${oldCookie}` }));
  assert.equal(stale.status, 401); assert.equal(consumed, false);
});

test("password login rejects cross-origin, oversized credentials and role/account injection extras", async () => {
  assert.equal((await auth.promotionPassword(req("auth/password", { body: { accountId: "old-account", password: "correct-password", role: "admin" } }))).status, 400);
  assert.equal((await auth.promotionPassword(req("auth/password", { body: { accountId: "old-account", password: "x".repeat(257) } }))).status, 400);
  assert.equal((await auth.promotionPassword(req("auth/password", { body: { accountId: "old-account", password: "correct-password" }, headers: { origin: "https://evil.test" } }))).status, 403);
  assert.equal(requests.length, 0);
});

test("bootstrap distinguishes a configured draft from absent campaign data and pagination is bounded", async () => {
  publicData = { campaignId: "preview-20261029", environment: "development", status: "unconfigured", termsStatus: "draft", dataMode: "sandbox" };
  let data = await (await api.promotionBootstrap(req("bootstrap", { method: "GET" }))).json();
  assert.equal(data.campaign, null); assert.equal(data.campaignStatus, "unconfigured");
  assert.equal(data.public.dataMode, "sandbox");
  publicData = { ...publicData, status: "draft", recruitmentEnabled: false, commissionEnabled: false, payoutExecutionEnabled: false };
  data = await (await api.promotionBootstrap(req("bootstrap", { method: "GET" }))).json();
  assert.equal(data.campaign.campaignId, "preview-20261029"); assert.equal(data.campaign.recruitmentEnabled, false);
  const { cookie } = await login();
  assert.equal((await api.promotionProxy(req("members?offset=100001&limit=10", { method: "GET", cookie }), ["members"])).status, 400);
  assert.equal((await api.promotionProxy(req("members?offset=0&limit=101", { method: "GET", cookie }), ["members"])).status, 400);
  assert.equal((await api.promotionProxy(req("members?offset=0&limit=10", { method: "GET", cookie }), ["members"])).status, 200);
});

test("authentication wrappers bound body streams and ignore forwarded host spoofing", async () => {
  const oversized = req("auth/challenge", { body: { padding: "x".repeat(2000) } });
  assert.equal((await auth.promotionChallenge(oversized)).status, 413);
  const wrongOrigin = req("auth/challenge", { headers: { origin: "https://attacker.test", "x-forwarded-host": "attacker.test", "x-forwarded-proto": "https" } });
  assert.equal((await auth.promotionChallenge(wrongOrigin)).status, 403);
  assert.equal(requests.length, 0);
});

test("the strict gateway query contract gets only configured campaigns and route-specific fields", async () => {
  const { cookie } = await login();
  await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }));
  for (const route of ["public", "me", "members", "settlements", "admin"]) {
    assert.equal((await api.promotionProxy(req(route, { method: "GET", cookie }), [route])).status, 200);
  }
  for (const route of ["invite", "public"]) {
    const result = await api.promotionProxy(req(`${route}?code=leader-code`, { method: "GET" }), [route]);
    assert.equal(result.status, 200); assert.equal((await result.json()).leaderDisplayName, "Leader");
  }
  assert.equal((await api.promotionProxy(req("me?cursor=unexpected", { method: "GET", cookie }), ["me"])).status, 400);
  assert.equal((await api.promotionProxy(req("payout-address", { method: "GET", cookie }), ["payout-address"])).status, 200);
  const payout = requests.findLast((item) => new URL(item.url).pathname.endsWith("/payout-address"));
  assert.equal(new URL(payout.url).search, "");
});

test("an absent campaign config reports the source gap and never calls a default campaign", async () => {
  delete process.env.MIR2_PROMOTION_CAMPAIGN_ID;
  const response = await api.promotionBootstrap(req("bootstrap", { method: "GET" }));
  assert.equal(response.status, 200); const data = await response.json();
  assert.equal(data.campaign, null); assert.equal(data.campaignStatus, "unconfigured");
  assert.equal(data.public.dataMode, "no_campaign");
  assert.deepEqual(data.sourceGaps, ["promotion_campaign_not_configured"]); assert.equal(requests.length, 0);
  assert.equal((await api.promotionProxy(req("public", { method: "GET" }), ["public"])).status, 503);
});

test("only the explicit admin exclude-member target can contain an account ID; the actor remains cookie-derived", async () => {
  const { cookie } = await login();
  const { csrfToken } = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).json();
  const headers = { "Idempotency-Key": "admin-operation-12345", "x-operator-role": "finance_admin" };
  const command = { action: "excludeMember", accountId: "member-under-review", reason: "review", evidenceRef: "case17" };
  const accepted = await api.promotionProxy(req("admin", { cookie, csrf: csrfToken, body: { command }, headers }), ["admin"]);
  assert.equal(accepted.status, 200);
  const forwarded = requests.findLast((item) => new URL(item.url).pathname.endsWith("/admin") && item.init.method === "POST");
  assert.equal(new Headers(forwarded.init.headers).get("x-operator-role"), null);
  assert.deepEqual(JSON.parse(forwarded.init.body), { campaignId: "preview-20261029", command });
  for (const body of [{ accountId: "actor", command }, { command: { ...command, actor: "admin" } },
    { command: { kind: "excludeMember", accountId: "victim" } }, { command: { action: "setCampaign", accountId: "victim" } }]) {
    assert.equal((await api.promotionProxy(req("admin", { cookie, csrf: csrfToken, body, headers }), ["admin"])).status, 400);
  }
});

test("campaign is injected only into campaign operations, never into payout settings or auth", async () => {
  const { cookie } = await login();
  const { csrfToken } = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).json();
  for (const [route, body] of [["apply", { characterIndex: 0, termsVersion: "draft" }],
    ["payout-address", { credentialId: "wallet-credential-17", network: "sui:testnet" }]]) {
    const result = await api.promotionProxy(req(route, { cookie, csrf: csrfToken, body, headers: { "Idempotency-Key": "request-1234567890" } }), [route]);
    assert.equal(result.status, 200);
    const forwarded = requests.findLast((item) => new URL(item.url).pathname.endsWith(`/${route}`) && item.init.method === "POST");
    assert.equal(new URL(forwarded.url).search, "");
    assert.deepEqual(JSON.parse(forwarded.init.body), route === "apply" ? { ...body, campaignId: "preview-20261029" } : body);
  }
});

test("anonymous clicks stay in browser-bound encrypted cookies and only the matching code can become a candidate", async () => {
  const click = await api.promotionProxy(req("click", { body: { code: "leader-code-A" } }), ["click"]);
  assert.equal(click.status, 200); const clickCookie = cookies(click);
  assert.match(click.headers.get("set-cookie"), /HttpOnly; Secure; SameSite=Strict/);
  const publicClick = await click.json(); assert.equal(publicClick.clickId, undefined); assert.equal(publicClick.leaderDisplayName, "Leader A");
  assert.ok(!clickCookie.includes("anonymous-click-17"));
  const { cookie } = await login();
  const { csrfToken } = await (await api.promotionBootstrap(req("bootstrap", { method: "GET", cookie }))).json();
  const candidate = (extra = {}) => api.promotionProxy(req("candidate", { cookie: `${cookie}; ${clickCookie}`, csrf: csrfToken,
    body: { code: "leader-code-A", source: "invite_link" }, headers: { "Idempotency-Key": "candidate-request-1234" }, ...extra }), ["candidate"]);
  assert.equal((await candidate({ cookie })).status, 409);
  assert.equal((await candidate({ body: { code: "leader-code-B", source: "invite_link" } })).status, 409);
  assert.equal((await candidate({ body: { code: "leader-code-A", source: "invite_link", clickId: "attacker-selected" } })).status, 400);
  assert.equal((await candidate()).status, 200);
  const forwarded = requests.findLast((entry) => new URL(entry.url).pathname.endsWith("/candidate"));
  assert.equal(JSON.parse(forwarded.init.body).clickId, "anonymous-click-17");
  assert.equal((await candidate({ body: { code: "leader-code-B", source: "manual_code" } })).status, 200);
  const manual = requests.findLast((entry) => new URL(entry.url).pathname.endsWith("/candidate"));
  assert.equal(JSON.parse(manual.init.body).clickId, undefined);
  const confirmed = await api.promotionProxy(req("confirm", { cookie: `${cookie}; ${clickCookie}`, csrf: csrfToken,
    body: { candidateId: "server-candidate-17", termsVersion: "draft" }, headers: { "Idempotency-Key": "confirm-request-12345" } }), ["confirm"]);
  assert.equal(confirmed.status, 200); assert.match(confirmed.headers.get("set-cookie"), /__Host-mir2-promotion-click=;.*Max-Age=0/);
});

test("anonymous click rejects cross-origin, tampering, missing secret, and client-selected fields before writing", async () => {
  assert.equal((await api.promotionProxy(req("click", { body: { code: "leader-code-A" }, headers: { origin: "https://evil.test" } }), ["click"])).status, 403);
  assert.equal((await api.promotionProxy(req("click", { body: { code: "leader-code-A", clickId: "injected" } }), ["click"])).status, 400);
  delete process.env.MIR2_PROMOTION_SESSION_SECRET;
  assert.equal((await api.promotionProxy(req("click", { body: { code: "leader-code-A" } }), ["click"])).status, 503);
  assert.equal(requests.length, 0);
});

test("finance export forwards only the session and strict settlement selection while preserving canonical input bytes", async () => {
  const { cookie } = await login();
  const previous = handler;
  const canonicalJson = '{"account_id":"fixture-α","message":"line\\nvalue","z":1000000}';
  handler = async (url, init) => {
    const parsed = new URL(url);
    if (parsed.pathname === "/v1/promotions/export") {
      assert.equal(parsed.searchParams.get("campaignId"), "preview-20261029"); assert.equal(parsed.searchParams.get("settlementId"), "fixture-settlement");
      assert.deepEqual([...parsed.searchParams.keys()].sort(), ["campaignId", "settlementId"]);
      assert.equal(new Headers(init.headers).get("authorization"), "Bearer server-only-identity-bearer");
      return Response.json({ settlementId: "fixture-settlement", snapshotCanonicalJson: canonicalJson, snapshotSha256: "fixed-input-hash", dataMode: "real" });
    }
    return previous(url, init);
  };
  const result = await api.promotionProxy(req("export?settlementId=fixture-settlement", { method: "GET", cookie }), ["export"]);
  assert.equal(result.status, 200); assert.equal((await result.json()).snapshotCanonicalJson, canonicalJson);
  assert.equal((await api.promotionProxy(req("export", { method: "GET", cookie }), ["export"])).status, 400);
  assert.equal((await api.promotionProxy(req("export?settlementId=fixture-settlement&accountId=other", { method: "GET", cookie }), ["export"])).status, 400);
});

test("each Next route resolves to the bounded server handler and opts out of static caching", () => {
  const routes = ["bootstrap", "auth/challenge", "auth/poll", "auth/password", "auth/logout", "[...path]"];
  const sources = routes.map((route) => fileURLToPath(new URL(`../app/api/promotions/${route}/route.ts`, import.meta.url)));
  const program = ts.createProgram(sources, {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler,
    noEmit: true, strict: true, skipLibCheck: true, lib: ["lib.es2022.d.ts", "lib.dom.d.ts", "lib.dom.iterable.d.ts"], types: ["node"],
    typeRoots: [fileURLToPath(new URL("../node_modules/@types", import.meta.url))],
  });
  const errors = ts.getPreEmitDiagnostics(program);
  assert.equal(errors.length, 0, errors.map((item) => ts.flattenDiagnosticMessageText(item.messageText, "\n")).join("\n"));
  for (const route of routes) {
    const source = readFileSync(new URL(`../app/api/promotions/${route}/route.ts`, import.meta.url), "utf8");
    assert.match(source, /runtime = "nodejs"/); assert.match(source, /dynamic = "force-dynamic"/);
  }
});
