import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.MIR2_PLAYWRIGHT_MODULE || "playwright");
const origin = process.env.MIR2_PROMOTION_UI_ORIGIN || "http://127.0.0.1:7381";
assert.match(origin, /^http:\/\/(?:127\.0\.0\.1|localhost):\d+$/,
  "Revenue-basis UI fixtures may run only against a local server");
const evidence = process.env.MIR2_PROMOTION_UI_EVIDENCE || mkdtempSync(path.join(os.tmpdir(), "mir2-revenue-basis-ui-"));
mkdirSync(evidence, { recursive: true });
const browser = await chromium.launch({ headless: true,
  ...(process.env.MIR2_CHROME_EXECUTABLE ? { executablePath: process.env.MIR2_CHROME_EXECUTABLE } : { channel: "chrome" }) });
const errors = [], reads = [], results = [];
const fixture = { policy: "monthly_purchase_full", environment: "test" };
const now = Date.UTC(2026, 9, 31, 0, 0, 0);
const dashboard = { clicks: 1, registrations: 1, confirmed: 1, qualified: 1, tiers: [{ rateBps: 1000, count: 1 }],
  eligibleRevenueMicroUsd: "10000000", estimatedCommissionMicroUsd: "1000000", commissionEnabled: true, asOfMs: now };
const leader = { id: "fixture-leader", status: "approved", displayName: "测试团长", characterIndex: 1, inviteCode: "FIXTURE-ONLY" };
function bootstrap() {
  const campaign = { campaignId: "fixture-season", environment: fixture.environment, status: "active", termsVersion: "fixture-v1",
    termsStatus: "approved", startsAtMs: now - 86400000, endsAtMs: now + 86400000, recruitmentEnabled: true,
    commissionEnabled: true, payoutEnabled: false, fundingMode: "revenue_share",
    rules: { minimumLevel: 3, activeUtcDays: 1, ...(fixture.policy === undefined ? {} : { revenueRecognition: fixture.policy }) } };
  return { authenticated: true, csrfToken: "fixture-only", gameClient: "windows", campaign, characters: [], leader, dashboard,
    sourceGaps: [], permissions: [], binding: { status: "linked", credentialId: "fixture-credential", network: "sui:testnet" },
    me: { account: { id: "fixture-account", registeredAtMs: now - 86400000 }, characters: [], leader, dashboard,
      sourceGaps: [], attribution: null, settlements: [], appeals: [], permissions: { isLeader: true, isAdmin: false },
      payoutSettings: { status: "verified", payoutEnabled: false } } };
}
const context = await browser.newContext({ locale: "zh-CN", timezoneId: "Asia/Shanghai", viewport: { width: 1440, height: 1100 } });
const page = await context.newPage();
page.on("pageerror", error => errors.push(error.message));
page.on("console", message => { if (message.type() === "error") errors.push(`${message.text()} (${message.location().url || "no source"})`); });
await page.route("**/api/**", async route => {
  const request = route.request(), pathname = new URL(request.url()).pathname;
  assert.equal(request.method(), "GET", "This read-only UI test must never create a real operation");
  reads.push(pathname);
  const payload = pathname === "/api/promotions/bootstrap" ? bootstrap() : pathname === "/api/promotions/me" ? bootstrap().me :
    pathname === "/api/promotions/settlements" ? { settlements: [] } : pathname === "/api/promotions/members" ? { members: [] } : null;
  assert.ok(payload, `Unexpected API request: ${pathname}`);
  await route.fulfill({ json: payload });
});
let documentRead = 0;
async function open(lang = "zh-CN", view = "earnings") {
  const response = await page.goto(`${origin}/promotions?lang=${lang}&fixtureRead=${++documentRead}#${view}`, { waitUntil: "networkidle" });
  assert.equal(response.status(), 200);
  await page.locator("[data-promotion-revenue-basis]").waitFor();
}
async function screenshot(name) {
  await page.evaluate(() => {
    const label = document.createElement("div");
    label.textContent = "LOCAL FIXTURE · NOT LIVE ACCOUNT DATA";
    label.style.cssText = "position:fixed;z-index:2147483647;bottom:8px;right:8px;max-width:calc(100vw - 16px);padding:8px;background:#fff1b8;color:#321900;border:2px solid #b57400;font:12px sans-serif;overflow-wrap:anywhere";
    document.body.appendChild(label);
  });
  await page.screenshot({ path: path.join(evidence, name), fullPage: true });
}
async function check(name, operation) {
  await operation();
  results.push({ name, passed: true });
}
try {
  await check("full purchase rule displays 1 USD and clear calculation, top-up and refund terms", async () => {
    await open();
    const panel = page.locator('[data-promotion-revenue-basis="monthly_purchase_full"]');
    await panel.getByText(/月卡消费金额 × 对应分段费率/).waitFor();
    assert.match(await panel.innerText(), /不按赛季剩余时长折算.*同一张月卡只计佣一次/s);
    assert.match(await panel.innerText(), /充值点券本身不计佣.*退款会扣回/s);
    await page.getByText("1.00 USD", { exact: true }).waitFor();
    await screenshot("monthly-purchase-full-desktop.png");
  });
  await check("activity rules show the same selected policy", async () => {
    await page.locator("nav").getByRole("button", { name: "活动规则", exact: true }).click();
    await page.locator('[data-promotion-revenue-basis="monthly_purchase_full"]').getByText(/月卡消费金额 × 对应分段费率/).waitFor();
  });
  await check("legacy campaigns retain service-overlap wording", async () => {
    fixture.policy = undefined;
    await open();
    const panel = page.locator('[data-promotion-revenue-basis="monthly_service_prorated"]');
    assert.match(await panel.innerText(), /服务有效期与本赛季重叠的时长折算计佣/);
    assert.doesNotMatch(await panel.innerText(), /实付金额全额计佣/);
    await screenshot("legacy-service-prorated-desktop.png");
  });
  await check("unknown server rule cannot advertise full purchase recognition", async () => {
    fixture.policy = "unknown";
    await open();
    assert.equal(await page.locator('[data-promotion-revenue-basis="monthly_purchase_full"]').count(), 0);
  });
  await check("production full policy follows the same rule and English copy", async () => {
    fixture.policy = "monthly_purchase_full";
    fixture.environment = "production";
    await open("en");
    assert.match(await page.locator('[data-promotion-revenue-basis="monthly_purchase_full"]').innerText(), /full verified paid amount.*Each card is counted once/s);
    assert.equal(await page.getByText("This is a test environment.", { exact: true }).count(), 0);
  });
  await check("Chinese and Arabic policy panels fit mobile width", async () => {
    fixture.environment = "test";
    await page.setViewportSize({ width: 390, height: 844 });
    for (const lang of ["zh-CN", "ar"]) {
      await open(lang);
      const dimensions = await page.evaluate(() => ({ viewport: innerWidth, doc: document.documentElement.scrollWidth, body: document.body.scrollWidth }));
      assert.ok(dimensions.doc <= dimensions.viewport + 1 && dimensions.body <= dimensions.viewport + 1, JSON.stringify(dimensions));
      await screenshot(`monthly-purchase-full-${lang}-mobile.png`);
    }
  });
  assert.deepEqual(errors, []);
  const report = { schema: "mir2.promotion-revenue-basis-ui.v1", observedAtUtc: new Date().toISOString(), origin,
    fixtureOnly: true, productionBackendRequests: 0, writeRequests: 0, reads, errors, results };
  writeFileSync(path.join(evidence, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify({ evidence, passed: results.length, errors }, null, 2));
} finally {
  await context.close();
  await browser.close();
}
