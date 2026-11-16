import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

// Use the project's installation by default; CI/operators can select their
// installed browser tooling without committing a machine-specific runtime path.
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.MIR2_PLAYWRIGHT_MODULE || "playwright");
const origin = process.env.MIR2_PROMOTION_UI_ORIGIN || "http://127.0.0.1:7368";
assert.match(origin, /^http:\/\/(?:127\.0\.0\.1|localhost):\d+$/,
  "Fixture UI verification is restricted to a local server");
const evidence = fileURLToPath(new URL("../../../docs/evidence/promotion-realtime-20261030/", import.meta.url));
mkdirSync(evidence, { recursive: true });
const results = [];
const failures = [];
const baseTime = Date.UTC(2026, 9, 30, 10, 0, 0);
const browser = await chromium.launch({ headless: true,
  ...(process.env.MIR2_CHROME_EXECUTABLE ? { executablePath: process.env.MIR2_CHROME_EXECUTABLE } : {}) });

function state() {
  return { authenticated: true, amount: "0", memberAmount: "0", epoch: 0, locked: false,
    accountId: "fixture-account", credentialId: "fixture-credential", pages: null,
    settlements: [], bootstrapCount: 0, membersCount: 0, calls: [], writes: [], hold: null };
}
function bootstrap(f) {
  const campaign = { campaignId: "fixture-realtime-season", environment: "test", status: f.locked ? "locked" : "active",
    startsAtMs: baseTime - 86_400_000, endsAtMs: baseTime + 90 * 86_400_000, termsVersion: "fixture-v1", termsStatus: "approved",
    recruitmentEnabled: !f.locked, commissionEnabled: true, payoutEnabled: false,
    fundingMode: "revenue_share", rules: { minimumLevel: 30, activeUtcDays: 3, reserveBps: 0 }, payoutPolicy: null };
  const dashboard = { clicks: 8, registrations: 3, confirmed: 2, qualified: 1, tiers: [{ rateBps: 1000, count: 1 }],
    eligibleRevenueMicroUsd: "25000000", estimatedCommissionMicroUsd: f.amount, commissionEnabled: true,
    asOfMs: baseTime + f.epoch * 1000 };
  const leader = { id: "fixture-leader", status: "approved", displayName: "测试团长", characterIndex: 0, inviteCode: "FIXTURE-ONLY" };
  const characters = [{ index: 0, name: "测试角色", level: 30, class: "Warrior", gender: "Male" }];
  return { authenticated: f.authenticated, csrfToken: f.authenticated ? "fixture-csrf-never-real" : null,
    campaign, characters: f.authenticated ? characters : [], leader: f.authenticated ? leader : null,
    dashboard: f.authenticated ? dashboard : null, permissions: [], sourceGaps: [],
    binding: f.authenticated ? { status: "linked", credentialId: f.credentialId, network: "sui:testnet" } : { status: "pending" },
    ...(f.authenticated ? { me: { account: { id: f.accountId, registeredAtMs: baseTime - 86_400_000 },
      characters, leader, dashboard, sourceGaps: [], attribution: null,
      settlements: structuredClone(f.settlements), appeals: [],
      payoutSettings: { status: "verified", payoutEnabled: false, network: "sui:testnet" },
      permissions: { isLeader: true, isAdmin: false } } } : {}),
    public: { campaignId: campaign.campaignId, environment: "test", payoutEnabled: false } };
}
function holdNext(f) {
  let release, started;
  const gate = new Promise(resolve => { release = resolve; });
  const seen = new Promise(resolve => { started = resolve; });
  f.hold = { gate, started };
  return { release, seen };
}
async function fixturePage(f, { clock = false, mobile = false } = {}) {
  // New contexts carry no user cookies or browser storage. Never inspect actual
  // browser sessions, and never allow a promotion API request through to backend.
  const context = await browser.newContext({ locale: "zh-CN", timezoneId: "Asia/Shanghai",
    viewport: mobile ? { width: 390, height: 844 } : { width: 1440, height: 1000 } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
  await page.addInitScript(() => {
    window.__promotionFixtureVisibility = "visible";
    Object.defineProperty(document, "visibilityState", { configurable: true,
      get: () => window.__promotionFixtureVisibility });
  });
  await page.route("**/api/promotions/**", async route => {
    const request = route.request(), url = new URL(request.url());
    const path = url.pathname.slice("/api/promotions/".length);
    f.calls.push({ method: request.method(), path, at: Date.now(),
      ...(path === "members" ? { offset: Number(url.searchParams.get("offset") || 0), accountId: f.accountId } : {}) });
    if (request.method() !== "GET") {
      f.writes.push(path);
      assert.equal(path, "auth/logout", "Only a locally fulfilled synthetic logout POST is permitted");
      f.authenticated = false;
      await route.fulfill({ json: { status: "signed_out" } });
      return;
    }
    let payload;
    if (path === "bootstrap") {
      ++f.bootstrapCount;
      payload = bootstrap(f);
      const held = f.hold;
      if (held) { f.hold = null; held.started(); await held.gate; }
    } else if (path === "members") {
      ++f.membersCount;
      const offset = Number(url.searchParams.get("offset") || 0);
      const selected = f.pages?.[offset];
      payload = { items: [{ memberRef: selected?.memberRef || "member-privacy-fixture", status: "qualified", reasonCode: "requirements_complete",
        estimatedCommissionMicroUsd: f.memberAmount, qualifiedAtMs: baseTime, rank: 1, rateBps: 1000 }], nextOffset: selected?.nextOffset ?? null };
    } else if (path === "settlements") payload = { items: structuredClone(f.settlements) };
    else if (path === "me") payload = bootstrap(f).me || { authenticated: false };
    else if (path === "public") payload = bootstrap(f).campaign;
    else throw new Error(`Unexpected fixture GET: ${path}`);
    // An intentionally delayed request may have been cancelled by the UI.
    try { await route.fulfill({ json: payload }); } catch (error) {
      if (!/closed|cancel|Invalid InterceptionId|No resource|not found/i.test(String(error))) throw error;
    }
  });
  if (clock) await page.clock.install({ time: new Date(baseTime) });
  return { context, page, errors };
}
async function ready(page, view = "earnings") {
  const response = await page.goto(`${origin}/promotions?lang=zh-CN#${view}`, { waitUntil: "domcontentloaded" });
  assert.equal(response.status(), 200, "Local promotion page must compile/render before fixture verification");
  await page.getByRole("button", { name: "退出登录", exact: true }).waitFor();
  await page.getByRole("button", { name: "刷新", exact: true }).waitFor({ state: "visible" });
  if (view !== "members") await page.waitForFunction(() => document.querySelector("time")?.getAttribute("datetime"));
  return response;
}
const refresh = page => page.getByRole("button", { name: "刷新", exact: true });
const nav = (page, name) => page.locator("nav").getByRole("button", { name, exact: true });
async function until(predicate, message, timeout = 5000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) { if (await predicate()) return; await new Promise(resolve => setTimeout(resolve, 30)); }
  throw new Error(message);
}
async function noOverflow(page) {
  const sizes = await page.evaluate(() => ({ viewport: innerWidth, doc: document.documentElement.scrollWidth,
    body: document.body.scrollWidth }));
  assert.ok(sizes.doc <= sizes.viewport + 1 && sizes.body <= sizes.viewport + 1, JSON.stringify(sizes));
  return sizes;
}
async function check(name, operation) {
  const began = Date.now();
  console.log(`UI_CHECK_START ${name}`);
  try { const details = await operation(); results.push({ name, passed: true, elapsedMs: Date.now() - began, ...details }); }
  catch (error) { failures.push({ name, error: error.message }); results.push({ name, passed: false, error: error.message }); }
  console.log(`UI_CHECK_DONE ${name}: ${results.at(-1).passed ? "passed" : "failed"}`);
}

try {
  const f = state(), desktop = await fixturePage(f), page = desktop.page;
  const initialDocument = await ready(page);
  await check("SSR neutral timezone hydrates to the browser zone without mismatched text", async () => {
    const html = await initialDocument.text();
    const footer = html.match(/<footer\b[^>]*>([\s\S]*?)<\/footer>/i)?.[1];
    assert.ok(footer, "Expected an SSR footer in the initial document");
    const footerText = footer.replace(/<[^>]*>/g, "");
    assert.match(footerText, / · —/);
    assert.doesNotMatch(footerText, /Asia\/Shanghai|\bUTC\b/,
      "The SSR document must not bake in the server or browser timezone");
    await page.waitForFunction(() => document.querySelector("footer")?.innerText.includes("Asia/Shanghai"));
    assert.deepEqual(desktop.errors, [], "Timezone hydration must not raise React #418");
    return { ssrTimezonePlaceholder: "—", browserTimezone: "Asia/Shanghai", hydrationErrors: 0 };
  });
  await check("Chinese earnings remain available while payout execution is off", async () => {
    assert.equal(bootstrap(f).campaign.payoutEnabled, false);
    await page.getByText("本赛季累计佣金（待结算）", { exact: true }).waitFor();
    await page.getByText("0.00 USD", { exact: true }).waitFor();
    assert.match(await page.locator("time").innerText(), /\d{1,2}:\d{2}:\d{2}/);
    assert.ok((await page.locator("main").innerText()).includes("每 10 秒更新"));
    return { screenshot: "EARNINGS-DESKTOP.png", overflow: await noOverflow(page) };
  });
  await check("one real ten-second poll updates accrued commission and second-level time without clicks", async () => {
    const before = await page.locator("time").getAttribute("datetime");
    const start = Date.now();
    f.amount = "2500000"; f.epoch = 10;
    await page.getByText("2.50 USD", { exact: true }).waitFor({ timeout: 15_000 });
    const elapsed = Date.now() - start;
    assert.ok(elapsed >= 8500, `Expected a real interval wait, saw ${elapsed} ms`);
    assert.notEqual(await page.locator("time").getAttribute("datetime"), before);
    await page.screenshot({ path: `${evidence}/EARNINGS-DESKTOP.png`, fullPage: true });
    return { realWaitMs: elapsed, bootstrapReads: f.bootstrapCount };
  });
  await check("same-page manual refresh takes fresh bootstrap me.settlements and finalized label", async () => {
    f.locked = true; f.epoch = 21;
    f.settlements = [{ id: "fixture-reviewed-final", status: "review", totalMicroUsd: "2500000", payableCents: 250,
      payableMicroUsd: "2500000", paidMicroUsd: "0", reserveMicroUsd: "0", reversalMicroUsd: "0",
      payoutId: null, payoutStatus: null, txDigest: null, checkpoint: null, createdAtMs: baseTime + 20_000,
      snapshotSha256: "a".repeat(64) }];
    await refresh(page).click();
    await page.getByText("fixture-reviewed-final", { exact: true }).waitFor();
    await page.getByText("本赛季已封账佣金", { exact: true }).waitFor();
    assert.equal(f.calls.filter(c => c.path === "settlements").length, 0, "Do not discard fresh me.settlements");
    await page.screenshot({ path: `${evidence}/SETTLEMENT-REFRESH-DESKTOP.png`, fullPage: true });
  });
  assert.deepEqual(desktop.errors, [], "Desktop console/page errors");
  await desktop.context.close();

  const membersFixture = state(), memberBrowser = await fixturePage(membersFixture, { clock: true });
  const membersPage = memberBrowser.page;
  await ready(membersPage, "members");
  const row = membersPage.locator("tr").filter({ hasText: "member-privacy-fixture" });
  await row.waitFor();
  await check("members refresh both manually and automatically on the same page", async () => {
    membersFixture.memberAmount = "1250000"; membersFixture.epoch = 30;
    await refresh(membersPage).click();
    await row.getByText("1.25 USD", { exact: true }).waitFor();
    membersFixture.memberAmount = "2500000"; membersFixture.epoch = 40;
    await membersPage.clock.runFor(10_100);
    await row.getByText("2.50 USD", { exact: true }).waitFor();
    return { memberReads: membersFixture.membersCount };
  });
  await check("hidden tab skips polling and visible transition refreshes immediately", async () => {
    await membersPage.evaluate(() => { window.__promotionFixtureVisibility = "hidden"; document.dispatchEvent(new Event("visibilitychange")); });
    const before = membersFixture.calls.length;
    await membersPage.clock.runFor(30_100);
    assert.equal(membersFixture.calls.length, before, "A hidden page must not request promotion data");
    membersFixture.memberAmount = "3750000"; membersFixture.epoch = 71;
    await membersPage.evaluate(() => { window.__promotionFixtureVisibility = "visible"; document.dispatchEvent(new Event("visibilitychange")); });
    await row.getByText("3.75 USD", { exact: true }).waitFor();
    return { simulatedVisibilityEvent: true, hiddenReadCount: 0 };
  });
  await check("navigation cancels a delayed old-view refresh before fetching obsolete members", async () => {
    const held = holdNext(membersFixture);
    await refresh(membersPage).click();
    await held.seen;
    await nav(membersPage, "活动规则").click();
    const count = membersFixture.membersCount;
    held.release();
    await new Promise(resolve => setTimeout(resolve, 300));
    assert.equal(membersFixture.membersCount, count, "Delayed old-view bootstrap must not fetch/commit obsolete members after navigation");
    assert.match(await membersPage.locator('nav button[aria-current="page"]').innerText(), /活动规则$/);
  });
  // Re-enter after any navigation test so logout always has an actual pending
  // authenticated read. Its POST is synthetic and fulfilled in the route above.
  await nav(membersPage, "邀请成员").click();
  await row.waitFor();
  await check("logout prevents a delayed authenticated response from restoring private data", async () => {
    const held = holdNext(membersFixture);
    await refresh(membersPage).click(); await held.seen;
    await membersPage.getByRole("button", { name: "退出登录", exact: true }).click();
    await until(async () => (await membersPage.getByRole("button", { name: "退出登录", exact: true }).count()) === 0, "Logout did not remove authenticated controls");
    held.release();
    await new Promise(resolve => setTimeout(resolve, 300));
    assert.equal(await membersPage.getByRole("button", { name: "退出登录", exact: true }).count(), 0);
    assert.equal(await membersPage.getByText("member-privacy-fixture", { exact: true }).count(), 0);
    assert.deepEqual(membersFixture.writes, ["auth/logout"]);
  });
  assert.deepEqual(memberBrowser.errors, [], "Member scenario console/page errors");
  await memberBrowser.context.close();

  const paginatedFixture = state();
  paginatedFixture.pages = {
    0: { memberRef: "old-identity-first-page", nextOffset: 20 },
    20: { memberRef: "old-identity-second-page", nextOffset: null },
  };
  const pagination = await fixturePage(paginatedFixture, { clock: true });
  await ready(pagination.page, "members");
  await check("new account and credential reset an existing second-page member view to offset zero", async () => {
    await pagination.page.getByText("old-identity-first-page", { exact: true }).waitFor();
    await pagination.page.getByRole("button", { name: "下一页", exact: true }).click();
    await pagination.page.getByText("old-identity-second-page", { exact: true }).waitFor();
    assert.ok(paginatedFixture.calls.some(call => call.path === "members" && call.offset === 20));
    const beforeSwitch = paginatedFixture.calls.length;
    paginatedFixture.accountId = "fixture-new-account";
    paginatedFixture.credentialId = "fixture-new-credential";
    paginatedFixture.pages = {
      0: { memberRef: "new-identity-first-page", nextOffset: 20 },
      20: { memberRef: "new-identity-second-page-must-not-load", nextOffset: null },
    };
    await refresh(pagination.page).click();
    await pagination.page.getByText("new-identity-first-page", { exact: true }).waitFor();
    const previousPage = pagination.page.getByRole("button", { name: "上一页", exact: true });
    await until(() => previousPage.isDisabled(), "Identity change did not reset the stored member offset");
    // A further manual refresh proves the reset was committed, rather than
    // changing only the first request after the account/credential transition.
    await refresh(pagination.page).click();
    await pagination.page.getByText("new-identity-first-page", { exact: true }).waitFor();
    const afterSwitch = paginatedFixture.calls.slice(beforeSwitch).filter(call => call.path === "members");
    assert.ok(afterSwitch.length >= 2, "Expected identity-switch and subsequent same-identity member reads");
    assert.ok(afterSwitch.every(call => call.offset === 0 && call.accountId === "fixture-new-account"),
      JSON.stringify(afterSwitch));
    assert.equal(await pagination.page.getByText("old-identity-second-page", { exact: true }).count(), 0);
    assert.equal(await pagination.page.getByText("new-identity-second-page-must-not-load", { exact: true }).count(), 0);
    assert.deepEqual(pagination.errors, []);
    await pagination.page.screenshot({ path: `${evidence}/IDENTITY-PAGINATION-DESKTOP.png`, fullPage: true });
    return { oldOffset: 20, newIdentityReadOffsets: afterSwitch.map(call => call.offset),
      screenshot: "IDENTITY-PAGINATION-DESKTOP.png" };
  });
  await pagination.context.close();

  const mobileFixture = state(); mobileFixture.amount = "2500000"; mobileFixture.epoch = 90;
  const mobile = await fixturePage(mobileFixture, { mobile: true });
  await ready(mobile.page);
  await check("mobile accrued earnings fit the viewport with no horizontal overflow", async () => {
    await mobile.page.getByText("本赛季累计佣金（待结算）", { exact: true }).waitFor();
    const overflow = await noOverflow(mobile.page);
    await mobile.page.screenshot({ path: `${evidence}/EARNINGS-MOBILE.png`, fullPage: true });
    assert.deepEqual(mobile.errors, []);
    return { screenshot: "EARNINGS-MOBILE.png", overflow };
  });
  await mobile.context.close();
} catch (error) {
  failures.push({ name: "suite", error: error.stack || error.message });
} finally {
  await browser.close();
  const report = { schema: "mir2.promotion-realtime-ui.v1", verifiedAtUtc: new Date().toISOString(), origin,
    allPromotionApisFulfilledByLocalFixture: true, realUserCookiesRead: false, realBusinessPosts: 0,
    results, failures, passed: failures.length === 0 };
  writeFileSync(`${evidence}/UI-VERIFICATION.json`, JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
  if (failures.length) process.exitCode = 1;
}
