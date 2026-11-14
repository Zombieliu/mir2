import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.MIR2_PLAYWRIGHT_MODULE || "playwright");
const origin = process.env.MIR2_PROMOTION_UI_ORIGIN || "http://127.0.0.1:7368";
assert.match(origin, /^http:\/\/(?:127\.0\.0\.1|localhost):\d+$/);
const evidence = fileURLToPath(new URL("../../../docs/evidence/promotion-windows-bridge-20261030/", import.meta.url));
mkdirSync(evidence, { recursive: true });
const browser = await chromium.launch({ headless: true,
  ...(process.env.MIR2_CHROME_EXECUTABLE ? { executablePath: process.env.MIR2_CHROME_EXECUTABLE } : {}) });
const campaignId = "numeron-windows-playtest-20261030";
const locales = [["zh-CN", "zh-CN"], ["zh-TW", "zh-TW"], ["en", "en"], ["pt-BR", "pt-BR"],
  ["ru", "ru"], ["hi", "en"], ["id", "id"], ["vi", "vi"], ["th", "th"], ["ar", "ar"]];
const results = [];
try {
  for (const [lang, download] of locales) {
    const context = await browser.newContext({ locale: lang, timezoneId: "Asia/Shanghai", viewport: { width: lang === "zh-CN" ? 390 : 1365, height: 900 } });
    const page = await context.newPage();
    const errors = [], requests = [], sockets = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.routeWebSocket(/\/ws(?:\?|$)/, socket => { sockets.push(socket.url()); socket.close(); });
    await page.route("**/api/**", async route => {
      const req = route.request(), path = new URL(req.url()).pathname;
      requests.push({ path, method: req.method(), season: req.headers()["x-mir2-promotion-season"] });
      assert.equal(req.method(), "GET", "UI fixtures may never submit an actual operation");
      assert.equal(path, "/api/promotions/bootstrap");
      await route.fulfill({ json: { authenticated: false, csrfToken: null, gameClient: "windows",
        campaign: { campaignId }, characters: [], leader: null, dashboard: null,
        binding: { status: "pending" }, sourceGaps: [], permissions: [] } });
    });
    await page.goto(`${origin}/promotions/play?lang=${lang}&campaignId=${campaignId}`, { waitUntil: "networkidle" });
    try { await page.locator(`a[href="/${download}/download"]`).waitFor(); }
    catch (error) {
      writeFileSync(`${evidence}/windows-ui-failure.json`, JSON.stringify({ lang, requests, errors, body: await page.locator("body").innerText() }, null, 2));
      await page.screenshot({ path: `${evidence}/windows-ui-failure.png`, fullPage: true });
      throw error;
    }
    assert.equal(await page.locator("h1").count(), 1);
    if (lang === "zh-CN") assert.equal(await page.locator("h1").textContent(), "在 Windows 游戏中创建角色");
    assert.equal(await page.locator("canvas").count(), 0, "Windows guidance cannot mount the legacy game");
    assert.equal(await page.locator(`a[href="/promotions?campaignId=${campaignId}#billing"]`).count(), 1);
    assert.equal(await page.locator("main").getAttribute("dir"), lang === "ar" ? "rtl" : "ltr");
    assert.ok(requests.length && requests.every(req => req.season === campaignId));
    assert.deepEqual(errors, []); assert.deepEqual(sockets, []);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
    if (["zh-CN", "ar"].includes(lang)) await page.screenshot({ path: `${evidence}/windows-guide-${lang}.png`, fullPage: true });
    results.push({ locale: lang, passed: true, requests, sockets: sockets.length });
    await context.close();
  }
} finally {
  await browser.close();
  writeFileSync(`${evidence}/windows-ui.json`, JSON.stringify({ fixtureOnly: true, origin, results }, null, 2));
}
console.log(JSON.stringify({ passed: results.length, fixtureOnly: true, realPayments: 0 }));
