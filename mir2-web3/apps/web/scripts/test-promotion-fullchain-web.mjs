import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import vm from "node:vm";
import ts from "typescript";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
const require = createRequire(import.meta.url);
const ORIGIN = "https://mir2.obelisk.build";
const GATEWAY = "https://promotion-playtest.165.154.65.136.sslip.io";
const KEY = "synthetic-idempotency-key-20261029";
const TOKEN = "synthetic-commercial-token-not-for-browser";
function load(path, extras = {}) {
  const compiled = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX,
  } }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, { module, exports: module.exports, Request, Response, Headers, URL, AbortSignal, Intl, crypto, ...extras });
  return module.exports;
}
function fixture({ network = "testnet", anonymous = false, bound = true, payload = { accepted: true } } = {}) {
  const calls = [];
  const fail = (status, code) => { throw Object.assign(new Error(code), { status }); };
  const auth = {
    promotionBrowserRequest(request, write) {
      if (request.headers.has("authorization")) fail(400, "browser_bearer_not_allowed");
      if (write && request.headers.get("origin") !== ORIGIN) fail(403, "cross_origin_request");
    },
    promotionCsrf(request) { if (request.headers.get("x-promotion-csrf") !== "synthetic-csrf") fail(403, "csrf_required"); },
    promotionSession: async () => { if (anonymous) fail(401, "promotion_login_required"); return { accountId: "synthetic-account", token: TOKEN, walletBound: bound }; },
    promotionReadBody: request => request.json(), promotionJson: (body, status = 200) => Response.json(body, { status }),
    promotionFail: fail, promotionError: error => Response.json({ error: error.message }, { status: error.status || 503 }),
    promotionGateway: async (path, options) => { calls.push({ path, ...options }); return { ok: true, status: 200, payload }; },
  };
  const extras = { process: { env: { MIR2_PROMOTION_GATEWAY_HTTP_URL: GATEWAY } }, require: spec =>
    spec === "server-only" ? {} : spec === "./promotion-auth" ? auth : spec === "./promotion-wallet-context" ?
      { promotionWalletNetwork: () => network } : assert.fail(`unexpected import ${spec}`) };
  return { billing: load("../lib/server/promotion-billing.ts", extras).promotionBilling,
    game: load("../lib/server/promotion-game.ts", extras).promotionGameSession, calls };
}
function request(path, body, extraHeaders = {}) {
  return new Request(`${ORIGIN}/api/promotions/${path}`, { method: body === undefined ? "GET" : "POST",
    headers: { origin: ORIGIN, "x-promotion-csrf": "synthetic-csrf", "idempotency-key": KEY, "content-type": "application/json", ...extraHeaders },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
}
const checkoutBody = { characterIndex: 0, requestId: 10, offerId: "test_1000", orderRequestId: KEY };
test("billing status uses only the selected owned character and the server-side identity bearer", async () => {
  const f = fixture(); const response = await f.billing(request("billing/status?characterIndex=0&requestId=1"), "status");
  assert.equal(response.status, 200); assert.equal(f.calls[0].path, "/v1/billing/status");
  assert.equal(f.calls[0].token, TOKEN); assert.equal(f.calls[0].query, "?characterIndex=0&requestId=1");
  assert.ok(!JSON.stringify(await response.json()).includes(TOKEN));
});
test("billing denies anonymous, unbound and mainnet sessions before any billing I/O", async () => {
  for (const [config, expected] of [[{ anonymous: true }, 401], [{ bound: false }, 403], [{ network: "mainnet" }, 403]]) {
    const f = fixture(config); assert.equal((await f.billing(request("billing/checkout", checkoutBody), "checkout")).status, expected); assert.equal(f.calls.length, 0);
  }
});
test("billing retains browser bearer, Origin and CSRF boundaries", async () => {
  for (const [headers, expected] of [[{ origin: "https://untrusted.invalid" }, 403], [{ authorization: "Bearer synthetic" }, 400], [{ "x-promotion-csrf": "wrong" }, 403]]) {
    const f = fixture(); assert.equal((await f.billing(request("billing/checkout", checkoutBody, headers), "checkout")).status, expected); assert.equal(f.calls.length, 0);
  }
});
test("billing rejects identity overrides, mismatched intents, invalid integer IDs and unknown operations", async () => {
  for (const body of [{ ...checkoutBody, accountId: "other" }, { ...checkoutBody, orderRequestId: "different" }, { ...checkoutBody, characterIndex: -1 }, { ...checkoutBody, requestId: Number.MAX_SAFE_INTEGER + 1 }]) {
    const f = fixture(); assert.equal((await f.billing(request("billing/checkout", body), "checkout")).status, 400); assert.equal(f.calls.length, 0);
  }
  const f = fixture(); assert.equal((await f.billing(request("billing/source-events", {}), "source-events")).status, 404);
});
test("checkout capability is constrained to the configured HTTPS payment portal", async () => {
  const cap = `usdc_${"a".repeat(64)}.${"b".repeat(43)}`;
  for (const [url, expected] of [[`${GATEWAY}/v1/billing/sui-usdc/pay#${cap}`, 200], [`https://untrusted.invalid/v1/billing/sui-usdc/pay#${cap}`, 502], [`${GATEWAY}/other#${cap}`, 502], [`${GATEWAY}/v1/billing/sui-usdc/pay?token=secret#${cap}`, 502]]) {
    const f = fixture({ payload: { checkoutUrl: url } }); assert.equal((await f.billing(request("billing/checkout", checkoutBody), "checkout")).status, expected);
  }
});
test("monthly-card buy and exact-u64 activation use the fixed ordinary API", async () => {
  for (const [operation, body, path] of [["buy", { characterIndex: 1, requestId: 1, orderRequestId: KEY }, "/v1/billing/monthly-card/buy"],
    ["activate", { characterIndex: 1, requestId: 2, itemUniqueId: "18446744073709551615" }, "/v1/billing/monthly-card/activate"]]) {
    const f = fixture(); assert.equal((await f.billing(request(`billing/${operation}`, body), operation)).status, 200); assert.equal(f.calls[0].path, path);
  }
  const f = fixture(); assert.equal((await f.billing(request("billing/activate", { characterIndex: 1, requestId: 1, itemUniqueId: "18446744073709551616" }), "activate")).status, 400);
});
test("nested service identity tokens never appear in billing replies", async () => {
  const f = fixture({ payload: { credit: 100, token: TOKEN, items: [{ identityToken: TOKEN, amount: 1 }] } });
  const response = await f.billing(request("billing/status?characterIndex=0&requestId=1"), "status");
  assert.deepEqual(await response.json(), { credit: 100, items: [{ amount: 1 }] });
});
function gamePayload(changes = {}) { return { accountId: "synthetic-account", token: "synthetic-gameplay-only-token-xxxxxxxxxxxxxxxxxxxx", expiresAt: Date.now() + 60_000, realmId: "numeron-promotion-testnet", ...changes }; }
test("game broker returns only a short-lived gameplay grant for the authenticated account", async () => {
  const f = fixture({ payload: gamePayload() }); const response = await f.game(request("game-session", {}));
  assert.equal(response.status, 200); const body = await response.json(); assert.ok(body.token !== TOKEN);
  assert.equal(body.provider, "suiWallet"); assert.equal(f.calls[0].path, "/v1/promotions/game-session"); assert.equal(f.calls[0].token, TOKEN);
});
test("game broker rejects commercial token reuse, foreign accounts/realms and overlong expiry", async () => {
  for (const changes of [{ token: TOKEN }, { accountId: "other" }, { realmId: "mainnet" }, { expiresAt: Date.now() + 300_000 }]) {
    const f = fixture({ payload: gamePayload(changes) }); assert.equal((await f.game(request("game-session", {}))).status, 502);
  }
  const f = fixture(); assert.equal((await f.game(request("game-session", { accountId: "other" }))).status, 400); assert.equal(f.calls.length, 0);
});
test("test gameplay is selected only by the explicit route, not by a crafted URL parameter", () => {
  const realm = load("../lib/promotion-game-realm.ts");
  assert.equal(realm.isPromotionGamePath("/promotions/play"), true); assert.equal(realm.isPromotionGamePath("/promotions/play/"), true);
  for (const path of ["/", "/promotions", "/?gatewayWs=wss://untrusted.invalid", "/promotions/play-other"]) assert.equal(realm.isPromotionGamePath(path), false);
  assert.equal(realm.PROMOTION_GAME_WS, "wss://promotion-playtest.165.154.65.136.sslip.io/ws");
});
test("all nine billing locales are complete and no-role view offers the explicit test game", () => {
  const copies = load("../app/promotions/promotion-copy.ts");
  const model = load("../app/promotions/promotion-model.ts");
  const billing = load("../app/promotions/promotion-billing.tsx", { require: spec => spec === "./promotion-model" ? model : spec.endsWith(".module.css") ?
    { default: new Proxy({}, { get: (_, name) => String(name) }) } : require(spec) });
  for (const [locale] of copies.PROMOTION_LANGUAGES) {
    assert.equal(billing.PROMOTION_BILLING_LABELS[locale].length, 9);
    assert.ok(billing.PROMOTION_BILLING_LABELS[locale].every(label => label.trim()));
    const html = renderToStaticMarkup(createElement(billing.PromotionBilling, { bootstrap: { characters: [], csrfToken: "synthetic" }, copy: copies.PROMOTION_COPY[locale], locale }));
    assert.ok(html.includes('/promotions/play')); assert.ok(!html.includes(TOKEN));
  }
});
