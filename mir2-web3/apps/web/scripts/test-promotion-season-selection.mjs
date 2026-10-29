import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";
import ts from "typescript";

function moduleFrom(path, context = {}) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText;
  const module = { exports: {} };
  vm.runInNewContext(compiled, { module, exports: module.exports, URL, Request, Response, Headers, AbortSignal, Intl, ...context });
  return module.exports;
}
const model = moduleFrom("../app/promotions/promotion-model.ts");
const selected = "?campaignId=numeron-chain-acceptance-20261029";

test("no season selection preserves existing request shape and default behavior", () => {
  const body = { code: "existing" };
  const value = model.promotionSeasonRequest("candidate", body, "");
  assert.equal(value.path, "candidate"); assert.equal(value.body, body);
});
test("season selection follows bootstrap, invitation and paginated reads", () => {
  for (const path of ["bootstrap", "invite?code=abc", "members?offset=20&limit=20", "export?settlementId=settlement_1"]) {
    const value = model.promotionSeasonRequest(path, undefined, selected);
    const params = new URL(value.path, "https://promotion.invalid/").searchParams;
    assert.equal(params.get("campaignId"), "numeron-chain-acceptance-20261029");
    for (const [key, val] of new URL(path, "https://promotion.invalid/").searchParams) assert.equal(params.get(key), val);
  }
});
test("selected writes attach campaign without modifying caller state", () => {
  const body = { code: "abc", source: "invite_link" };
  const value = model.promotionSeasonRequest("candidate", body, selected);
  assert.equal(value.body.campaignId, "numeron-chain-acceptance-20261029");
  assert.equal(body.campaignId, undefined); assert.equal(value.body.code, "abc");
});
test("account-scoped authentication and receiving-wallet operations have no campaign injection", () => {
  for (const path of ["auth/challenge", "auth/poll", "auth/password", "payout-address"]) {
    const body = { marker: path };
    assert.equal(model.promotionSeasonRequest(path, body, selected).body, body);
    assert.equal(model.promotionSeasonRequest(path, undefined, selected).path, path);
  }
});
test("mixed-season requests, duplicate selectors and malformed IDs are rejected", () => {
  for (const [path, body, search] of [
    ["me?campaignId=another", undefined, selected], ["confirm", { campaignId: "another" }, selected],
    ["bootstrap", undefined, "?campaignId=a&campaignId=b"], ["bootstrap", undefined, "?campaignId=../../other"],
  ]) assert.throws(() => model.promotionSeasonRequest(path, body, search), error => error.status === 400);
});
test("sub-cent Testnet amounts retain exact integer precision", () => {
  assert.equal(model.formatPromotionUsd("23", "en"), "0.000023 USD");
  assert.equal(model.formatPromotionUsd("1000001", "en"), "1.000001 USD");
});

function bootstrapFixture() {
  const calls = [];
  const fail = (status, code) => { throw Object.assign(new Error(code), { status }); };
  const auth = {
    promotionBrowserRequest() {}, promotionSession: async () => null,
    promotionFail: fail, promotionJson: value => Response.json(value),
    promotionError: error => Response.json({ error: error.message }, { status: error.status || 503 }),
    promotionGateway: async (path, options) => {
      calls.push({ path, ...options });
      return { ok: true, status: 200, payload: {
        campaignId: new URLSearchParams(options.query).get("campaignId"), status: "active", environment: "test",
      } };
    },
  };
  const api = moduleFrom("../lib/server/promotion-api.ts", { process: { env: { MIR2_PROMOTION_CAMPAIGN_ID: "original-seven-day-season" } },
    require: name => name === "server-only" ? {} : name === "./promotion-auth" ? auth :
      name === "./promotion-wallet-context" ? { promotionWalletNetwork: () => "testnet" } : assert.fail(`unexpected module ${name}`),
  });
  return { api, calls };
}
test("BFF bootstrap selects the requested season while preserving the configured default", async () => {
  const { api, calls } = bootstrapFixture();
  for (const [search, expected] of [["", "original-seven-day-season"], [selected, "numeron-chain-acceptance-20261029"]]) {
    const result = await api.promotionBootstrap(new Request(`https://mir2.obelisk.build/api/promotions/bootstrap${search}`));
    assert.equal(result.status, 200); assert.equal((await result.json()).campaign.campaignId, expected);
    assert.equal(new URLSearchParams(calls.at(-1).query).get("campaignId"), expected);
  }
});
test("BFF rejects invalid season selectors before Gateway I/O", async () => {
  const { api, calls } = bootstrapFixture();
  for (const search of ["?campaignId=a&campaignId=b", "?campaignId=bad/path", "?gateway=https://untrusted.invalid"]) {
    const result = await api.promotionBootstrap(new Request(`https://mir2.obelisk.build/api/promotions/bootstrap${search}`));
    assert.equal(result.status, 400);
  }
  assert.equal(calls.length, 0);
});
