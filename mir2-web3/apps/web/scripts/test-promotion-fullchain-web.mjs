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
  vm.runInNewContext(compiled, { module, exports: module.exports, Request, Response, Headers, URL, AbortSignal, Intl, crypto,
    require: spec => ["../../lib/promotion-season-context", "./promotion-season-context"].includes(spec) ? load("../lib/promotion-season-context.ts") : require(spec), ...extras });
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
  const process = { env: { MIR2_PROMOTION_GATEWAY_HTTP_URL: GATEWAY } };
  const backend = load("../lib/server/promotion-backend.ts", { process, require: spec => spec === "server-only" ? {} : require(spec) });
  const extras = { process, require: spec =>
    spec === "./promotion-backend" ? backend :
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
test("all billing locales are complete and each profile offers its own character entry", () => {
  const copies = load("../app/promotions/promotion-copy.ts");
  const model = load("../app/promotions/promotion-model.ts");
  const windows = load("../app/promotions/promotion-windows-copy.ts");
  const billing = load("../app/promotions/promotion-billing.tsx", { require: spec => spec === "./promotion-model" ? model : spec === "./promotion-windows-copy" ? windows : spec === "./promotion-checkout-recovery" ? load("../app/promotions/promotion-checkout-recovery.ts") : spec.endsWith(".module.css") ?
    { default: new Proxy({}, { get: (_, name) => String(name) }) } : require(spec) });
  for (const [locale] of copies.PROMOTION_LANGUAGES) {
    assert.equal(billing.PROMOTION_BILLING_LABELS[locale].length, 9);
    assert.ok(billing.PROMOTION_BILLING_LABELS[locale].every(label => label.trim()));
    const html = renderToStaticMarkup(createElement(billing.PromotionBilling, { bootstrap: { characters: [], csrfToken: "synthetic" }, copy: copies.PROMOTION_COPY[locale], locale }));
    assert.ok(html.includes('/promotions/play')); assert.ok(!html.includes(TOKEN));
    const native = renderToStaticMarkup(createElement(billing.PromotionBilling, { bootstrap: { gameClient: "windows", characters: [], csrfToken: "synthetic" }, copy: copies.PROMOTION_COPY[locale], locale }));
    assert.ok(native.includes(windows.windowsDownloadPath(locale)), locale);
    assert.ok(native.includes(windows.PROMOTION_WINDOWS_COPY[locale].download), locale);
    assert.ok(native.includes(windows.PROMOTION_WINDOWS_COPY[locale].body), locale);
    assert.ok(!native.includes('/promotions/play'), locale);
    assert.ok(!native.includes(TOKEN), locale);
  }
});

// Exercise the actual TSX controller and its rendered event handlers. The hook
// scheduler only supplies React state/effect semantics; business behavior stays
// in promotion-app.tsx and every API operation is a synthetic local fixture.
async function invitationApp(initial = {}) {
  const copies = load("../app/promotions/promotion-copy.ts");
  const accounts = load("../app/promotions/promotion-account-copy.ts");
  const migrations = load("../app/promotions/promotion-migration-copy.ts");
  const actualModel = load("../app/promotions/promotion-model.ts");
  const f = { calls: [], candidateError: null, confirmError: null, candidateStatus: 409, confirmStatus: 409,
    authenticated: true, csrfToken: "synthetic-csrf", accountId: "synthetic-account", credentialId: "synthetic-credential",
    campaignId: "synthetic-season", termsVersion: "synthetic-v1", attribution: null, poll: null, bootstrapGate: null,
    confirmGate: null, bound: true, termsStatus: "approved", migrationCandidate: undefined, ...initial };
  const now = Date.now();
  f.now = now;
  const timers = new Set();
  const bootstrap = () => ({ authenticated: f.authenticated, csrfToken: f.authenticated ? f.csrfToken : null, characters: [], leader: null,
    binding: f.authenticated && f.bound ? { status: "linked", credentialId: f.credentialId } : { status: "pending" }, sourceGaps: [], permissions: [],
    campaign: { campaignId: f.campaignId, environment: "test", status: "active", startsAtMs: now - 60_000,
      endsAtMs: now + 86_400_000, termsVersion: f.termsVersion, termsStatus: f.termsStatus, recruitmentEnabled: true,
      commissionEnabled: true, payoutEnabled: false, rules: { minimumLevel: 1, activeUtcDays: 1, testWalletEnrollment: f.testWalletEnrollment === true } },
    me: f.authenticated ? { account: { id: f.accountId }, attribution: f.attribution, settlements: [],
      ...(f.migrationCandidate === undefined ? {} : { migrationCandidate: structuredClone(f.migrationCandidate) }) } : undefined });
  const model = { ...actualModel, async promotionRequest(path, options = {}) {
    f.calls.push({ path, body: options.body });
    if (path === "bootstrap") {
      const payload = bootstrap(), gate = f.bootstrapGate; f.bootstrapGate = null;
      if (gate) await gate;
      return payload;
    }
    if (path === "candidate") {
      if (f.candidateError) throw new actualModel.PromotionApiError(f.candidateStatus, f.candidateError);
      return { candidateId: "synthetic-candidate", leaderDisplayName: "Synthetic inviting leader", expiresAtMs: now + 60_000 };
    }
    if (path === "confirm") {
      const code = f.confirmError, status = f.confirmStatus, gate = f.confirmGate; f.confirmGate = null;
      if (gate) await gate;
      if (code) throw new actualModel.PromotionApiError(status, code);
      if (!f.confirmWithoutAttribution) {
        const proof = options.body?.candidateId === f.migrationCandidate?.candidateId ? f.migrationCandidate : null;
        f.attribution = { status: "attributed", ...(proof ? { leaderId: "synthetic-leader", confirmedAtMs: f.now, migration: {
          migrationId: proof.migrationId, sourceCampaignId: proof.sourceCampaignId, sourceConfirmedAtMs: proof.sourceConfirmedAtMs,
          eligibilityAnchorMs: proof.eligibilityAnchorMs, preparedAtMs: proof.preparedAtMs, confirmedAtMs: f.now,
          snapshotSha256: "c".repeat(64), leaderDisplayName: proof.leaderDisplayName,
        } } : {}) };
        if (proof) f.migrationCandidate = undefined;
      }
      return { status: "bound" };
    }
    assert.fail(`Unexpected fixture operation ${path}`);
  } };
  const slots = []; let cursor = 0, pending = [], dirty = false, tree;
  const same = (left, right) => left && right && left.length === right.length && left.every((value, i) => Object.is(value, right[i]));
  const hooks = {
    useState(initial) {
      const i = cursor++;
      if (!(i in slots)) slots[i] = typeof initial === "function" ? initial() : initial;
      return [slots[i], value => { slots[i] = typeof value === "function" ? value(slots[i]) : value; dirty = true; }];
    },
    useRef(initial) { const i = cursor++; return slots[i] ||= { current: initial }; },
    useCallback(callback, deps) {
      const i = cursor++; if (!slots[i] || !same(slots[i].deps, deps)) slots[i] = { callback, deps };
      return slots[i].callback;
    },
    useEffect(effect, deps) {
      const i = cursor++, previous = slots[i];
      if (!previous || !same(previous.deps, deps)) {
        const next = { deps, cleanup: previous?.cleanup }; slots[i] = next;
        pending.push(() => { next.cleanup?.(); next.cleanup = effect(); });
      }
    },
  };
  const location = { search: "", hash: "", origin: ORIGIN };
  const ui = load("../app/promotions/promotion-app.tsx", { AbortController, URLSearchParams, Date: class extends Date { static now() { return f.now; } },
    window: { location, addEventListener() {}, removeEventListener() {},
      setTimeout(callback, ms) { const timer = { callback, at: f.now + ms }; timers.add(timer); return timer; },
      clearTimeout(timer) { timers.delete(timer); } }, location, history: { replaceState() {} },
    navigator: { language: "zh-CN" }, localStorage: { getItem: () => "zh-Hans" },
    document: { documentElement: {}, title: "Fixture", visibilityState: "visible", addEventListener() {}, removeEventListener() {} },
    require: spec => spec === "react" ? hooks : spec === "./promotion-copy" ? copies :
      spec === "./promotion-account-copy" ? accounts : spec === "./promotion-model" ? model :
      spec === "./promotion-migration-copy" ? migrations :
      spec === "./promotion-earnings-copy" ? load("../app/promotions/promotion-earnings-copy.ts") :
      spec === "./promotion-refresh" ? { createPromotionRefresh(options) { f.poll = options; return { request() {}, stop() {} }; } } :
      spec === "./promotion-billing" ? { PromotionBilling() {}, PROMOTION_BILLING_LABELS: Object.fromEntries(copies.PROMOTION_LANGUAGES.map(([locale]) => [locale, ["Billing"]])) } :
      ["./promotion-wallet-login", "./promotion-admin"].includes(spec) ? {} :
      spec.endsWith(".module.css") ? { default: new Proxy({}, { get: (_, key) => String(key) }) } : require(spec),
  });
  const app = ui.PromotionApp;
  f.flush = async () => {
    for (let round = 0; round < 20; ++round) {
      dirty = false; cursor = 0; tree = app({});
      const effects = pending; pending = []; effects.forEach(effect => effect());
      await new Promise(resolve => setImmediate(resolve));
      if (!dirty && !pending.length) return;
    }
    assert.fail("Fixture controller did not settle");
  };
  const nodes = value => Array.isArray(value) ? value.flatMap(nodes) : value && typeof value === "object" && value.props
    ? [value, ...nodes(value.props.children)] : [];
  f.find = (type, predicate = () => true) => nodes(tree).find(node => node.type === type && predicate(node.props));
  f.text = value => Array.isArray(value) ? value.map(f.text).join("") : value && typeof value === "object" && value.props
    ? f.text(value.props.children) : value == null || typeof value === "boolean" ? "" : String(value);
  f.inlineError = () => f.find("div", props => props.id === "promotion-invitation-error");
  f.enter = async code => { f.find("input", props => props.autoComplete === "off").props.onChange({ target: { value: code } }); await f.flush(); };
  f.submit = async () => {
    const form = f.find("form", props => nodes(props.children).some(node => node.type === "input" && node.props.autoComplete === "off"));
    form.props.onSubmit({ preventDefault() {} }); await f.flush();
  };
  f.confirm = () => f.find("button", props => f.text(props.children) === copies.PROMOTION_COPY["zh-Hans"].confirmAttribution);
  f.accept = async () => { f.find("input", props => props.type === "checkbox").props.onChange({ target: { checked: true } }); await f.flush(); };
  f.statuses = () => nodes(tree).filter(node => node.props.role === "status").map(f.text);
  f.alerts = () => nodes(tree).filter(node => node.props.role === "alert").map(f.text);
  f.holdBootstrap = () => { let release; f.bootstrapGate = new Promise(resolve => { release = resolve; }); return release; };
  f.holdConfirm = () => { let release; f.confirmGate = new Promise(resolve => { release = resolve; }); return release; };
  f.advance = async ms => { f.now += ms; for (const timer of [...timers]) if (timer.at <= f.now) { timers.delete(timer); timer.callback(); } await f.flush(); };
  f.migrationPanel = () => f.find("section", props => props.id === "promotion-migration-pending");
  f.migrationConfirm = () => f.find("button", props => props.id === "promotion-migration-confirm");
  f.migrationTerms = () => f.find("input", props => props.id === "promotion-migration-terms");
  f.acceptMigration = async () => { f.migrationTerms().props.onChange({ target: { checked: true } }); await f.flush(); };
  f.migrationError = () => f.find("div", props => props.id === "promotion-migration-error");
  f.enrollment = () => f.find(ui.PromotionTestWalletEnrollment);
  f.navigate = async label => { f.find("button", props => f.text(props.children).endsWith(label)).props.onClick(); await f.flush(); };
  f.accounts = accounts;
  f.migrations = migrations;
  f.formatDate = value => actualModel.formatPromotionDate(value, "zh-Hans");
  f.copy = copies.PROMOTION_COPY["zh-Hans"];
  await f.flush(); return f;
}

test("invitation business errors remain inline after successful asynchronous background refresh", async () => {
  const f = await invitationApp();
  await f.enter("SYNTHETIC-UNAVAILABLE"); f.candidateError = "promotionInviteMissing"; await f.submit();
  assert.equal(f.text(f.inlineError()), f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteNotFound);
  const reads = f.calls.filter(call => call.path === "bootstrap").length;
  await f.poll.refresh(new AbortController().signal); await f.flush();
  assert.equal(f.calls.filter(call => call.path === "bootstrap").length, reads + 1);
  assert.equal(f.text(f.inlineError()), f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteNotFound);
  assert.ok(!f.statuses().some(text => text.includes("已确认")));
  await f.enter("SYNTHETIC-RETRY"); assert.equal(f.inlineError(), undefined, "Editing a code deliberately clears its old rejection");
});

test("finding a leader requires review, terms and a separate confirmation; confirmation failures persist", async () => {
  const f = await invitationApp(); await f.enter("SYNTHETIC-INVITE"); await f.submit();
  assert.deepEqual(f.calls.filter(call => call.body).map(call => call.path), ["candidate"]);
  assert.ok(f.statuses().includes(f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].candidatePending));
  assert.ok(!f.statuses().includes(f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].attributionConfirmed));
  assert.ok(!f.statuses().some(text => text === "记录请求: 已确认"));
  assert.equal(f.confirm().props.disabled, true, "Terms must be accepted before the confirmation button opens");
  await f.accept(); assert.equal(f.confirm().props.disabled, false);
  f.confirmError = "promotionAlreadyPaid"; await f.confirm().props.onClick(); await f.flush();
  assert.equal(f.text(f.inlineError()), f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteAlreadyPaid);
  await f.poll.refresh(new AbortController().signal); await f.flush();
  assert.equal(f.text(f.inlineError()), f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteAlreadyPaid);
  f.confirmError = null; await f.confirm().props.onClick(); await f.flush();
  assert.ok(f.statuses().includes(f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].attributionConfirmed));
  assert.equal(f.inlineError(), undefined);
  assert.equal(f.confirm(), undefined, "Confirmed attribution replaces the candidate form");
});

test("account, credential and campaign changes reset invitation candidates, terms, errors and confirmation notices", async () => {
  for (const field of ["accountId", "credentialId", "campaignId"]) {
    const f = await invitationApp(); await f.enter("SYNTHETIC-INVITE"); await f.submit(); await f.accept();
    await f.confirm().props.onClick(); await f.flush();
    assert.ok(f.statuses().includes(f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].attributionConfirmed));
    const terms = f.termsVersion;
    f[field] += "-next"; f.attribution = null;
    await f.poll.refresh(new AbortController().signal); await f.flush();
    assert.equal(f.termsVersion, terms, "A context change must reset even when termsVersion is unchanged");
    assert.equal(f.confirm(), undefined, `${field}: old candidate must not survive`);
    assert.equal(f.inlineError(), undefined, field);
    assert.deepEqual(f.statuses(), [], `${field}: the new context must not inherit the previous confirmation`);
    await f.submit(); assert.equal(f.confirm().props.disabled, true, `${field}: previous terms acceptance must be cleared`);
    await f.accept(); f.confirmError = "promotionAlreadyPaid";
    await f.confirm().props.onClick(); await f.flush();
    assert.ok(f.inlineError(), `${field}: fixture must have a real rejected confirmation`);
    f[field] += "-again";
    await f.poll.refresh(new AbortController().signal); await f.flush();
    assert.equal(f.confirm(), undefined, field); assert.equal(f.inlineError(), undefined, field);
    await f.submit(); assert.equal(f.confirm().props.disabled, true, field);
  }
});

test("editing an invitation code resets review, terms and a confirmation notice while its refresh is still pending", async () => {
  const f = await invitationApp(); await f.enter("SYNTHETIC-FIRST"); await f.submit(); await f.accept();
  await f.enter("SYNTHETIC-SECOND"); assert.equal(f.confirm(), undefined);
  await f.submit(); assert.equal(f.confirm().props.disabled, true, "A new code requires fresh terms acceptance");
  await f.accept(); const release = f.holdBootstrap();
  f.confirm().props.onClick(); await f.flush();
  assert.ok(f.statuses().includes(f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].attributionConfirmed),
    "The successful confirm has returned, but the subsequent read is deliberately pending");
  await f.enter("SYNTHETIC-THIRD");
  assert.deepEqual(f.statuses(), [], "Changing the code must clear the previous confirmation feedback immediately");
  assert.equal(f.confirm(), undefined); assert.equal(f.inlineError(), undefined);
  release(); await f.flush();
  assert.deepEqual(f.statuses(), [], "A late successful refresh must not restore the old confirmation notice");
  assert.equal(f.calls.filter(call => call.path === "confirm").length, 1);
});

test("candidate and confirmation 401 failures stay visible after authenticated forms are removed", async () => {
  for (const operation of ["candidate", "confirm"]) {
    const f = await invitationApp(); await f.enter("SYNTHETIC-INVITE");
    if (operation === "confirm") { await f.submit(); await f.accept(); }
    f[`${operation}Error`] = "promotionAuthenticationRequired"; f[`${operation}Status`] = 401;
    if (operation === "candidate") await f.submit(); else { f.confirm().props.onClick(); await f.flush(); }
    assert.equal(f.find("input", props => props.autoComplete === "off"), undefined, `${operation}: expired identity must not retain the form`);
    assert.ok(f.alerts().includes(f.copy.sessionExpired), `${operation}: the global alert must explain the removed form`);
    assert.deepEqual(f.statuses(), [], operation);
  }
});

test("a missing write token keeps the session-expired reason inline while the authenticated form is still visible", async () => {
  const f = await invitationApp(); f.csrfToken = null;
  await f.poll.refresh(new AbortController().signal); await f.flush();
  await f.enter("SYNTHETIC-INVITE"); await f.submit();
  assert.ok(f.find("input", props => props.autoComplete === "off"));
  assert.equal(f.text(f.inlineError()), f.copy.sessionExpired);
  assert.equal(f.calls.filter(call => call.path === "candidate").length, 0, "No unauthenticated mutation may be sent");
});

function migrationProof(overrides = {}) {
  const now = Date.now();
  return { migrationId: "synthetic-migration", candidateId: "synthetic-migration-candidate", leaderDisplayName: "Synthetic original leader",
    sourceCampaignId: "synthetic-legacy-season", sourceConfirmedAtMs: now - 3 * 86_400_000, eligibilityAnchorMs: now - 3 * 86_400_000,
    preparedAtMs: now - 60_000, expiresAtMs: now + 3_600_000, targetTermsVersion: "synthetic-v1", ...overrides };
}

test("a migration proof is read-only until its own terms are accepted; only the ordinary confirm contract is sent", async () => {
  const proof = migrationProof(), f = await invitationApp({ migrationCandidate: proof });
  assert.ok(f.migrationPanel()); assert.equal(f.migrationTerms().props.checked, false);
  assert.equal(f.migrationConfirm().props.disabled, true);
  assert.equal(f.find("input", props => props.autoComplete === "off"), undefined, "A staged migration is not a manual-code candidate");
  await f.poll.refresh(new AbortController().signal); await f.flush();
  f.migrationConfirm().props.onClick(); await f.flush();
  assert.equal(f.calls.filter(call => call.body).length, 0, "Rendering, reading and a disabled action never confirm a migration");
  await f.acceptMigration(); assert.equal(f.migrationConfirm().props.disabled, false);
  f.migrationConfirm().props.onClick(); await f.flush();
  const writes = f.calls.filter(call => call.body);
  assert.deepEqual(writes.map(call => call.path), ["confirm"]);
  assert.equal(JSON.stringify(writes[0].body), JSON.stringify({ candidateId: proof.candidateId, termsVersion: proof.targetTermsVersion }));
  assert.equal(f.migrationPanel(), undefined);
  const completed = f.find("div", props => props.id === "promotion-migration-completed"), text = f.text(completed);
  const copy = f.migrations.PROMOTION_MIGRATION_COPY["zh-Hans"];
  assert.ok(completed); assert.ok(text.includes(copy.completedTitle));
  assert.ok(text.includes(copy.sourceConfirmedAt) && text.includes(f.formatDate(proof.sourceConfirmedAtMs)));
  assert.ok(text.includes(copy.confirmedAt) && text.includes(f.formatDate(f.now)));
  assert.notEqual(f.formatDate(proof.sourceConfirmedAtMs), f.formatDate(f.now));
  assert.ok(text.includes(proof.migrationId) && text.includes("c".repeat(64)) && text.includes(copy.financeNote));
  assert.ok(!f.statuses().includes(f.accounts.PROMOTION_ACCOUNT_COPY["zh-Hans"].attributionConfirmed), "Migration does not use the ordinary optimistic notice");
});

test("ordinary invitation consent does not authorize a newly staged migration", async () => {
  const f = await invitationApp(); await f.enter("SYNTHETIC-INVITE"); await f.submit(); await f.accept();
  assert.equal(f.confirm().props.disabled, false);
  f.migrationCandidate = migrationProof(); await f.poll.refresh(new AbortController().signal); await f.flush();
  assert.equal(f.migrationTerms().props.checked, false); assert.equal(f.migrationConfirm().props.disabled, true);
  f.migrationConfirm().props.onClick(); await f.flush();
  assert.equal(f.calls.filter(call => call.path === "confirm").length, 0);
});

test("wallet-enrollment terms do not authorize the independent member migration confirmation", async () => {
  const f = await invitationApp({ testWalletEnrollment: true, migrationCandidate: migrationProof() });
  await f.navigate(f.copy.testEnrollmentTitle);
  f.enrollment().props.onAccept(true); await f.flush();
  assert.equal(f.enrollment().props.acceptedTerms, true, "Fixture accepted only the leader enrollment terms");
  await f.navigate(f.copy.overview);
  assert.equal(f.migrationTerms().props.checked, false); assert.equal(f.migrationConfirm().props.disabled, true);
  f.migrationConfirm().props.onClick(); await f.flush();
  assert.equal(f.calls.filter(call => call.body).length, 0, "Neither enrollment nor migration is submitted by accepting the other form's terms");
});

test("migration failures remain visible through successful reads and use each safe public rejection category", async () => {
  const f = await invitationApp({ migrationCandidate: migrationProof() }); await f.acceptMigration();
  const codes = ["ProofInvalid", "Expired", "IdentityConflict", "LeaderConsentRequired", "TermsRequired", "PaymentUnverified",
    "PaymentPrecedesSource", "Conflict", "PolicyDenied", "SourceChanged"];
  for (const suffix of codes) {
    const code = `promotionMigration${suffix}`; f.confirmError = code;
    f.migrationConfirm().props.onClick(); await f.flush();
    const expected = f.migrations.promotionMigrationError(code, "zh-Hans");
    assert.ok(expected); assert.equal(f.text(f.migrationError()), expected);
    await f.poll.refresh(new AbortController().signal); await f.flush();
    assert.equal(f.text(f.migrationError()), expected, `${code}: successful refresh must preserve the rejected write`);
    assert.equal(f.find("div", props => props.id === "promotion-migration-completed"), undefined);
  }
  assert.ok(f.calls.filter(call => call.body).every(call => call.path === "confirm"));
});

test("migration consent and feedback are isolated by identity, activity, proof and terms; stale handlers cannot confirm", async () => {
  for (const field of ["accountId", "credentialId", "campaignId", "migrationId", "candidateId", "termsVersion"]) {
    const f = await invitationApp({ migrationCandidate: migrationProof() }); await f.acceptMigration();
    const oldButton = f.migrationConfirm(); f.confirmError = "promotionMigrationProofInvalid";
    oldButton.props.onClick(); await f.flush(); assert.ok(f.migrationError());
    if (["migrationId", "candidateId"].includes(field)) f.migrationCandidate = { ...f.migrationCandidate, [field]: `${f.migrationCandidate[field]}-next` };
    else f[field] += "-next";
    if (field === "termsVersion") f.migrationCandidate = { ...f.migrationCandidate, targetTermsVersion: f.termsVersion };
    await f.poll.refresh(new AbortController().signal); await f.flush();
    assert.equal(f.migrationError(), undefined, field); assert.equal(f.migrationTerms().props.checked, false, field);
    assert.equal(f.migrationConfirm().props.disabled, true, field);
    const writes = f.calls.filter(call => call.body).length;
    oldButton.props.onClick(); await f.flush();
    assert.equal(f.calls.filter(call => call.body).length, writes, `${field}: a previous context's event handler is retired`);
  }
});

test("a late migration rejection belongs only to the old proof and is not shown on a replacement proof", async () => {
  const f = await invitationApp({ migrationCandidate: migrationProof() }); await f.acceptMigration();
  const release = f.holdConfirm(); f.confirmError = "promotionMigrationIdentityConflict";
  f.migrationConfirm().props.onClick(); await f.flush();
  f.migrationCandidate = migrationProof({ migrationId: "synthetic-replacement", candidateId: "synthetic-replacement-candidate" });
  await f.poll.refresh(new AbortController().signal); await f.flush();
  release(); await f.flush();
  assert.equal(f.migrationError(), undefined); assert.equal(f.migrationTerms().props.checked, false);
  assert.equal(f.migrationConfirm().props.disabled, true);
  assert.equal(f.find("div", props => props.id === "promotion-migration-completed"), undefined);
});

test("a late migration 401 cannot clear a newly refreshed account, credential or replacement proof", async () => {
  for (const change of ["accountId", "credentialId", "proof"]) {
    const f = await invitationApp({ migrationCandidate: migrationProof() }); await f.acceptMigration();
    f.confirmError = "promotionAuthenticationRequired"; f.confirmStatus = 401;
    const release = f.holdConfirm(); f.migrationConfirm().props.onClick(); await f.flush();
    if (change === "proof") f.migrationCandidate = migrationProof({ migrationId: "synthetic-new-proof", candidateId: "synthetic-new-candidate" });
    else f[change] += "-new-session";
    await f.poll.refresh(new AbortController().signal); await f.flush();
    assert.ok(f.migrationPanel(), `${change}: the new authenticated bootstrap was committed`);
    assert.equal(f.migrationTerms().props.checked, false);
    release(); await f.flush();
    assert.ok(f.migrationPanel(), `${change}: a retired request must not clear the new bootstrap`);
    assert.ok(f.find("button", props => f.text(props.children) === f.copy.logout), `${change}: new identity remains signed in`);
    assert.ok(!f.alerts().includes(f.copy.sessionExpired), `${change}: old session failure must not label the new context expired`);
    assert.equal(f.migrationError(), undefined, change);
    f.confirmError = null; await f.acceptMigration();
    assert.equal(f.migrationConfirm().props.disabled, false, `${change}: the new proof remains usable after fresh consent`);
  }
});

test("successful confirm does not fabricate completed migration while the read model still returns a pending proof", async () => {
  const f = await invitationApp({ migrationCandidate: migrationProof(), confirmWithoutAttribution: true });
  await f.acceptMigration(); f.migrationConfirm().props.onClick(); await f.flush();
  assert.ok(f.migrationPanel()); assert.equal(f.find("div", props => props.id === "promotion-migration-completed"), undefined);
  assert.ok(f.statuses().includes(f.migrations.PROMOTION_MIGRATION_COPY["zh-Hans"].submitted));
  assert.equal(f.migrationConfirm().props.disabled, true, "Wait for an authoritative read instead of repeating a successful write");
  await f.poll.refresh(new AbortController().signal); await f.flush();
  assert.ok(f.statuses().includes(f.migrations.PROMOTION_MIGRATION_COPY["zh-Hans"].submitted));
  assert.equal(f.calls.filter(call => call.path === "confirm").length, 1);
});

test("migration confirmation fails closed for wrong terms, unbound wallets, malformed proof and expiry", async () => {
  for (const config of [{ termsStatus: "draft" }, { termsVersion: "different-terms" }, { bound: false },
    { migrationCandidate: migrationProof({ expiresAtMs: 0 }) }]) {
    const f = await invitationApp({ migrationCandidate: migrationProof(), ...config });
    assert.equal(f.migrationConfirm().props.disabled, true);
    f.migrationConfirm().props.onClick(); await f.flush(); assert.equal(f.calls.filter(call => call.body).length, 0);
  }
  const f = await invitationApp({ migrationCandidate: migrationProof({ expiresAtMs: Date.now() + 30_000 }) });
  await f.acceptMigration(); const button = f.migrationConfirm(); assert.equal(button.props.disabled, false);
  await f.advance(30_100); assert.equal(f.migrationConfirm().props.disabled, true);
  assert.ok(f.alerts().includes(f.migrations.PROMOTION_MIGRATION_COPY["zh-Hans"].expired));
  button.props.onClick(); await f.flush(); assert.equal(f.calls.filter(call => call.body).length, 0);
});

test("migration 401 remains visible globally and old-backend responses retain the normal invitation flow", async () => {
  const f = await invitationApp({ migrationCandidate: migrationProof() }); await f.acceptMigration();
  f.confirmError = "promotionAuthenticationRequired"; f.confirmStatus = 401;
  f.migrationConfirm().props.onClick(); await f.flush();
  assert.equal(f.migrationPanel(), undefined); assert.ok(f.alerts().includes(f.copy.sessionExpired));
  const legacy = await invitationApp();
  assert.equal(legacy.migrationPanel(), undefined); assert.equal(legacy.find("div", props => props.id === "promotion-migration-completed"), undefined);
  await legacy.enter("SYNTHETIC-LEGACY"); await legacy.submit(); await legacy.accept();
  assert.equal(legacy.confirm().props.disabled, false);
  const ordinary = await invitationApp({ attribution: { status: "bound", leaderId: "synthetic-leader", confirmedAtMs: Date.now() } });
  assert.equal(ordinary.migrationPanel(), undefined); assert.equal(ordinary.find("div", props => props.id === "promotion-migration-completed"), undefined);
});

test("all ten migration locales provide complete pending, audit, completed and safe-error copy", () => {
  const { PROMOTION_LANGUAGES } = load("../app/promotions/promotion-copy.ts");
  const { PROMOTION_MIGRATION_COPY, promotionMigrationError } = load("../app/promotions/promotion-migration-copy.ts");
  const codes = ["ProofInvalid", "Expired", "IdentityConflict", "LeaderConsentRequired", "TermsRequired", "PaymentUnverified",
    "PaymentPrecedesSource", "Conflict", "PolicyDenied", "SourceChanged"];
  assert.equal(Object.keys(PROMOTION_MIGRATION_COPY).length, PROMOTION_LANGUAGES.length);
  const englishKeys = Object.keys(PROMOTION_MIGRATION_COPY.en).sort();
  for (const [locale] of PROMOTION_LANGUAGES) {
    const copy = PROMOTION_MIGRATION_COPY[locale];
    assert.deepEqual(Object.keys(copy).sort(), englishKeys);
    assert.ok(Object.values(copy).every(value => typeof value === "string" && value.trim()));
    assert.notEqual(copy.pendingTitle, copy.completedTitle); assert.notEqual(copy.sourceConfirmedAt, copy.confirmedAt);
    for (const suffix of codes) assert.ok(promotionMigrationError(`promotionMigration${suffix}`, locale));
    assert.equal(promotionMigrationError("synthetic-private-proof-payload", locale), null);
  }
  assert.match(PROMOTION_MIGRATION_COPY["zh-Hans"].financeNote, /只迁移邀请关系/);
  assert.match(PROMOTION_MIGRATION_COPY.en.paymentUnverified, /Do not make a replacement payment/);
});

test("all invitation locales explain public failure categories without exposing account details", () => {
  const { PROMOTION_LANGUAGES } = load("../app/promotions/promotion-copy.ts");
  const { PROMOTION_ACCOUNT_COPY, promotionInvitationError } = load("../app/promotions/promotion-account-copy.ts");
  const cases = { promotionInviteMissing: "inviteNotFound", promotionInviteUnavailable: "inviteNotFound",
    promotionSelfReferral: "inviteSelf", promotionExistingAccountExcluded: "inviteExisting",
    promotionConfirmationExpired: "inviteWindowClosed", promotionCandidateExpired: "inviteCandidateExpired",
    promotionAlreadyPaid: "inviteAlreadyPaid", promotionAttributionFrozen: "inviteFrozen",
    promotionCampaignMissing: "inviteCampaignMismatch", promotion_campaign_mismatch: "inviteCampaignMismatch",
    promotionTermsChanged: "inviteTermsChanged", network_unavailable: "inviteUnconfirmed" };
  for (const [locale] of PROMOTION_LANGUAGES) {
    const copy = PROMOTION_ACCOUNT_COPY[locale];
    for (const key of ["invitationSteps", "findLeader", "candidatePending", "attributionConfirmed", ...Object.values(cases)]) {
      assert.ok(typeof copy[key] === "string" && copy[key].trim(), `${locale}.${key}`);
    }
    assert.notEqual(copy.candidatePending, copy.attributionConfirmed, locale);
    for (const [code, key] of Object.entries(cases)) assert.equal(promotionInvitationError(code, locale), copy[key]);
    assert.equal(promotionInvitationError("synthetic-private-account-details", locale), null);
  }
  assert.match(PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteSelf, /自己的邀请码/);
  assert.match(PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteWindowClosed, /确认期限/);
  assert.match(PROMOTION_ACCOUNT_COPY["zh-Hans"].inviteAlreadyPaid, /首次充值前/);
});
