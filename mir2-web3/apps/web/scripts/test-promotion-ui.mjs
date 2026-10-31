import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { createRequire } from "node:module";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import vm from "node:vm";
import ts from "typescript";
const require = createRequire(import.meta.url);

function load(name, overrides = {}) {
  const source = readFileSync(new URL(`../app/promotions/${name}.ts`, import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, { module, exports: module.exports, Intl, URL, AbortSignal, crypto, TextEncoder, Blob, ...overrides });
  return module.exports;
}

function loadViews() {
  const source = readFileSync(new URL("../app/promotions/promotion-app.tsx", import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
  }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, { module, exports: module.exports, require: name => {
    if (name.endsWith(".module.css")) return { default: new Proxy({}, { get: (_, key) => String(key) }) };
    if (["./promotion-copy", "./promotion-account-copy", "./promotion-model"].includes(name)) return load(name.slice(2));
    if (["./promotion-wallet-login", "./promotion-admin"].includes(name)) return {};
    return require(name);
  } });
  return module.exports;
}

test("all nine locales disclose testnet value and current revenue-share terms", () => {
  const { PROMOTION_COPY: copy, PROMOTION_LANGUAGES: languages } = load("promotion-copy");
  const { PromotionDisclosure } = loadViews();
  for (const [locale] of languages) {
    const text = copy[locale];
    for (const key of ["testnet", "testnetBody", "rules5", "rules6", "verifyAddress"]) {
      assert.ok(text[key]?.trim(), `${locale}.${key}`);
      if (locale !== "en") assert.notEqual(text[key], copy.en[key], `${locale}.${key}`);
    }
    assert.match(text.rules2, /30/);
    assert.match(text.rules2, /3/);
    assert.match(text.rules2, /UTC/);
    const testMarkup = renderToStaticMarkup(createElement(PromotionDisclosure, { copy: text, gaps: [], environment: "test" }));
    assert.ok(testMarkup.includes(text.testnetBody), locale);
    const productionMarkup = renderToStaticMarkup(createElement(PromotionDisclosure, { copy: text, gaps: [], environment: "production" }));
    assert.ok(!productionMarkup.includes(text.testnetBody), locale);
  }
  assert.match(copy.en.rules5, /without a preset total budget or member limit/);
  assert.match(copy.en.rules6, /two independent reviews/);
  assert.doesNotMatch(copy.en.rules5, /three.month|3.month/i);
  assert.doesNotMatch(copy.en.rules6, /No automatic real payment is available/);
});

test("wallet-only enrollment requires explicit test flag, approved open terms and current configured dates", () => {
  const { hasTestWalletEnrollment: visible, canEnrollTestWallet: allowed, testWalletApplication: application } = load("promotion-model");
  const now = Date.now(), campaign = { environment: "test", rules: { testWalletEnrollment: true }, termsVersion: "test-v1", termsStatus: "approved", recruitmentEnabled: true, startsAtMs: now - 1000, endsAtMs: now + 7 * 86400000 };
  assert.equal(visible(campaign), true); assert.equal(allowed(campaign, now), true);
  assert.deepEqual(JSON.parse(JSON.stringify(application(campaign, { bound: true, acceptedTerms: true, now }))), { characterIndex: -1, termsVersion: "test-v1" });
  assert.equal(application(campaign, { bound: false, acceptedTerms: true, now }), null);
  assert.equal(application(campaign, { bound: true, acceptedTerms: false, now }), null);
  for (const delta of [{ environment: "production" }, { environment: "development" }, { rules: {} }, { rules: { testWalletEnrollment: "true" } },
    { termsStatus: "draft" }, { termsVersion: undefined }, { recruitmentEnabled: false }, { startsAtMs: now + 1 }, { endsAtMs: now }, { endsAtMs: undefined }]) {
    assert.equal(allowed({ ...campaign, ...delta }, now), false, JSON.stringify(delta));
    assert.equal(application({ ...campaign, ...delta }, { bound: true, acceptedTerms: true, now }), null, "No bypass request can be formed outside its test scope");
  }
  assert.equal(visible({ ...campaign, environment: "production" }), false, "A production campaign cannot expose the test bypass");
});

test("all nine locales render wallet test signup without fabricated characters or balances", () => {
  const { PROMOTION_COPY: copy, PROMOTION_LANGUAGES: languages } = load("promotion-copy"), { PromotionTestWalletEnrollment: Form } = loadViews();
  const now = Date.now(), campaign = { campaignId: "isolated-trial", environment: "test", rules: { testWalletEnrollment: true }, termsVersion: "test-v1", termsStatus: "approved", recruitmentEnabled: true, startsAtMs: now - 1000, endsAtMs: now + 7 * 86400000 };
  for (const [locale] of languages) {
    const props = { campaign, copy: copy[locale], locale, bound: true, acceptedTerms: true, busy: false, leader: null, onAccept() {}, onSubmit() {} };
    const html = renderToStaticMarkup(createElement(Form, props));
    assert.ok(html.includes(copy[locale].testEnrollmentAction)); assert.ok(html.includes(copy[locale].testEnrollmentBody));
    assert.doesNotMatch(html, /<select|No character|30\.00 USD/); assert.doesNotMatch(html, /<button[^>]+disabled/);
    assert.ok(html.includes("test-v1"));
    for (const delta of [{ bound: false }, { acceptedTerms: false }, { busy: true }, { leader: { status: "approved" } }]) {
      assert.match(renderToStaticMarkup(createElement(Form, { ...props, ...delta })), /<button[^>]+disabled/);
    }
    assert.equal(renderToStaticMarkup(createElement(Form, { ...props, campaign: { ...campaign, environment: "production" } })), "");
    for (const key of ["testEnrollmentTitle", "testEnrollmentBody", "testEnrollmentAction", "testStepsTitle", "testStepJoin", "testStepShare", "configuredEligibility"]) {
      assert.ok(copy[locale][key]?.trim()); if (locale !== "en") assert.notEqual(copy[locale][key], copy.en[key]);
    }
    assert.ok(copy[locale].configuredEligibility.includes("{level}") && copy[locale].configuredEligibility.includes("{days}"));
  }
});

test("receiving-wallet UI follows campaign payment flag and manifest asset for all nine locales", () => {
  const { PROMOTION_COPY: copy, PROMOTION_LANGUAGES: languages } = load("promotion-copy");
  const { PromotionReceivingWallet } = loadViews();
  for (const [locale] of languages) {
    const bootstrap = {
      campaign: { environment: "test", payoutEnabled: true, payoutPolicy: { status: "proposed", network: "sui:testnet", symbol: "USDC", coinType: "0xreviewed::usdc::USDC", coinDecimals: 6 } },
      binding: { status: "linked", credentialId: "current-sign-in-only", network: "sui:testnet" },
      me: { payoutSettings: { status: "verified", address: "0xverified-current-wallet", payoutEnabled: false } },
    };
    const render = () => renderToStaticMarkup(createElement(PromotionReceivingWallet, { bootstrap, copy: copy[locale], locale, busy: false, onVerify() {} }));
    const enabled = render();
    assert.ok(enabled.includes(`${copy[locale].payments} · ${copy[locale].on}`), locale);
    assert.ok(enabled.includes("0xreviewed::usdc::USDC"), locale);
    assert.ok(enabled.includes(copy[locale].pending), locale);
    assert.ok(enabled.includes(copy[locale].testnetBody), locale);
    assert.ok(enabled.includes(copy[locale].verifyAddress), locale);
    assert.doesNotMatch(enabled, /<select|<input/, "Only the current sign-in wallet may be reverified");
    bootstrap.campaign.payoutEnabled = false;
    bootstrap.campaign.payoutPolicy = null;
    const disabled = render();
    assert.ok(disabled.includes(`${copy[locale].payments} · ${copy[locale].off}`), locale);
    assert.ok(disabled.includes(copy[locale].notConfigured), locale);
    assert.ok(!disabled.includes("USDC"), "Unconfigured assets must not be invented");
    bootstrap.campaign.payoutEnabled = "true";
    assert.ok(render().includes(`${copy[locale].payments} · ${copy[locale].off}`), "Only authoritative boolean true enables the displayed flag");
    bootstrap.binding.credentialId = undefined;
    assert.match(render(), /<button[^>]+disabled=""/, "A current login credential is required");
    bootstrap.campaign.environment = "production";
    bootstrap.public = { environment: "test" };
    assert.ok(render().includes(copy[locale].testnetBody), "Public test mode also requires a testnet disclosure");
    bootstrap.campaign = null;
    assert.ok(render().includes(`${copy[locale].payments} · ${copy[locale].notConfigured}`), "Missing campaign must remain unconfigured");
  }
});

test("all nine website locales have complete, independent promotion and approval copy", () => {
  const { PROMOTION_COPY: dictionaries, PROMOTION_LANGUAGES: languages } = load("promotion-copy");
  const { PROMOTION_ACCOUNT_COPY: accounts } = load("promotion-account-copy");
  const { PROMOTION_ADMIN_COPY: administration } = load("promotion-admin-copy");
  assert.equal(languages.length, 9);
  const keys = Object.keys(dictionaries.en).sort();
  for (const [locale] of languages) {
    assert.deepEqual(Object.keys(dictionaries[locale]).sort(), keys);
    for (const [key, value] of Object.entries(dictionaries[locale])) {
      assert.equal(typeof value, "string", `${locale}.${key}`);
      assert.ok(value.trim(), `${locale}.${key}`);
    }
    assert.ok(accounts[locale].bindingBody);
    assert.ok(administration[locale].executeClosed);
    if (locale !== "en") {
      assert.notEqual(dictionaries[locale], dictionaries.en);
      assert.notEqual(dictionaries[locale].proposalBody, dictionaries.en.proposalBody);
      assert.notEqual(dictionaries[locale].unknownPayment, dictionaries.en.unknownPayment);
    }
  }
});

test("language preference follows the existing Numeron website convention", () => {
  const { normalizePromotionLocale: normalize } = load("promotion-copy");
  assert.equal(normalize("zh-CN"), "zh-Hant");
  assert.equal(normalize("zh-TW"), "zh-Hant");
  assert.equal(normalize("pt-PT"), "pt-BR");
  assert.equal(normalize("ar-SA"), "ar");
  assert.equal(normalize("ru-RU"), "ru");
  assert.equal(normalize("not-a-locale"), "en");
  assert.equal(normalize(null), "en");
});

test("microUSD rendering preserves sub-cent amounts, signs and integers above JS safe range", () => {
  const { formatPromotionUsd: usd } = load("promotion-model");
  assert.equal(usd("0", "en"), "0.00 USD");
  assert.equal(usd("52000000", "en"), "52.00 USD");
  assert.equal(usd("353000000", "en"), "353.00 USD");
  assert.equal(usd("1", "en"), "0.000001 USD");
  assert.equal(usd("-1", "en"), "−0.000001 USD");
  assert.equal(usd("1059000000", "en"), "1,059.00 USD");
  assert.equal(usd("9007199254740993", "en"), "9,007,199,254.740993 USD");
  assert.equal(usd("1000001", "pt-BR"), "1,000001 USD");
  for (const invalid of [undefined, null, "", "1.5", "1e6", "NaN", "Infinity", 52, "9".repeat(41)]) assert.equal(usd(invalid, "en"), "—");
});

test("invalid or missing authoritative dates remain unavailable", () => {
  const { formatPromotionDate: date } = load("promotion-model");
  for (const invalid of [undefined, null, 0, -1, Infinity, NaN, Number.MAX_VALUE]) assert.equal(date(invalid, "en"), "—");
  assert.notEqual(date(Date.UTC(2026, 9, 29), "en"), "—");
});

test("approved cents render exactly and reject unsafe or negative cached counters", () => {
  const { formatPromotionCents: cents } = load("promotion-model");
  assert.equal(cents(35300, "en"), "353.00 USD");
  assert.equal(cents(1, "en"), "0.01 USD");
  assert.equal(cents(0, "en"), "0.00 USD");
  for (const invalid of [null, undefined, -1, 0.5, "100", Number.MAX_SAFE_INTEGER + 1]) assert.equal(cents(invalid, "en"), "—");
});

test("approval links reject script/data schemes and remote cleartext", () => {
  const { validatePromotionApprovalUrl: validate } = load("promotion-model");
  assert.equal(validate("https://wallet.example/connect/approve?id=short-lived"), "https://wallet.example/connect/approve?id=short-lived");
  assert.equal(validate("http://127.0.0.1:7359/test"), "http://127.0.0.1:7359/test");
  for (const invalid of [null, {}, "javascript:alert(1)", "data:text/html,test", "http://remote.example/login", "/relative"]) assert.equal(validate(invalid), null);
});

test("mutations use same-origin cookies, CSRF and the original uncertain-operation idempotency key", async () => {
  const requests = [];
  const { promotionRequest } = load("promotion-model", {
    fetch: async (url, options) => { requests.push({ url, options }); return Response.json({ status: "bound" }); },
  });
  const options = { body: { candidateId: "candidate-reference", termsVersion: "approved-test-terms" }, csrfToken: "browser-csrf-value", idempotencyKey: "same-operation-retry-123" };
  await promotionRequest("confirm", options);
  await promotionRequest("confirm", options);
  for (const { url, options: request } of requests) {
    assert.equal(url, "/api/promotions/confirm");
    assert.equal(request.credentials, "same-origin");
    assert.equal(request.cache, "no-store");
    assert.equal(request.headers["x-promotion-csrf"], options.csrfToken);
    assert.equal(request.headers["Idempotency-Key"], options.idempotencyKey);
    assert.equal(request.headers.authorization, undefined);
    assert.deepEqual(JSON.parse(request.body), options.body);
  }
});

test("HTTP and malformed-response failures reject rather than creating a success state", async () => {
  const denied = load("promotion-model", { fetch: async () => Response.json({ error: "promotionAttributionFrozen" }, { status: 409 }) });
  await assert.rejects(denied.promotionRequest("confirm", { body: {}, csrfToken: "csrf" }), error => error.status === 409 && error.code === "promotionAttributionFrozen");
  const unavailable = load("promotion-model", { fetch: async () => new Response("upstream unavailable", { status: 503 }) });
  await assert.rejects(unavailable.promotionRequest("bootstrap"), error => error.code === "invalid_response");
  const malformed = load("promotion-model", { fetch: async () => Response.json(null) });
  await assert.rejects(malformed.promotionRequest("bootstrap"), error => error.code === "invalid_response");
  for (const body of [{}, []]) {
    const empty = load("promotion-model", { fetch: async () => Response.json(body) });
    await assert.rejects(empty.promotionRequest("bootstrap"), error => error.code === "invalid_response");
  }
});

test("canonical settlement export keeps original bytes and large integer text", async () => {
  const canonical = '{"amount":9007199254740993,"currency":"USD","members":[]}\n';
  const snapshotSha256 = createHash("sha256").update(canonical).digest("hex");
  const requests = [];
  const { readPromotionSettlementExport: download } = load("promotion-model", { fetch: async (url, options) => {
    requests.push({ url, options }); return Response.json({ snapshotCanonicalJson: canonical, snapshotSha256 });
  } });
  assert.equal(await (await download("settlement-safe-ref")).text(), canonical);
  assert.equal(requests[0].url, "/api/promotions/export?settlementId=settlement-safe-ref");
  assert.equal(requests[0].options.credentials, "same-origin");
  const denied = load("promotion-model", { fetch: async () => Response.json({ error: "denied" }, { status: 403 }) });
  await assert.rejects(denied.readPromotionSettlementExport("other"), error => error.status === 403);
  const corrupted = load("promotion-model", { fetch: async () => Response.json({ snapshotCanonicalJson: canonical + " ", snapshotSha256 }) });
  await assert.rejects(corrupted.readPromotionSettlementExport("other"), error => error.code === "settlement_export_hash_mismatch");
});
