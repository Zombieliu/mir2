import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const temporary = mkdtempSync(path.join(os.tmpdir(), "mir2-promotion-earnings-"));
after(() => {
  assert.equal(path.dirname(path.resolve(temporary)), path.resolve(os.tmpdir()));
  assert.ok(path.basename(temporary).startsWith("mir2-promotion-earnings-"));
  rmSync(temporary, { recursive: true, force: true });
});
const program = ts.createProgram([
  fileURLToPath(new URL("../app/promotions/promotion-earnings-copy.ts", import.meta.url)),
  fileURLToPath(new URL("../app/promotions/promotion-model.ts", import.meta.url)),
], {
  target: ts.ScriptTarget.ES2022,
  module: ts.ModuleKind.CommonJS,
  strict: true,
  types: [],
  lib: ["lib.es2022.d.ts", "lib.dom.d.ts", "lib.dom.iterable.d.ts"],
  outDir: temporary,
  noEmitOnError: true,
});
const diagnostics = ts.getPreEmitDiagnostics(program);
assert.equal(diagnostics.length, 0, ts.formatDiagnosticsWithColorAndContext(diagnostics, {
  getCurrentDirectory: () => process.cwd(),
  getCanonicalFileName: file => file,
  getNewLine: () => "\n",
}));
assert.equal(program.emit().emitSkipped, false);
const require = createRequire(import.meta.url);
const { PROMOTION_EARNINGS_COPY } = require(path.join(temporary, "promotion-earnings-copy.js"));
const { PROMOTION_LANGUAGES } = require(path.join(temporary, "promotion-copy.js"));
const { formatPromotionRefreshTime } = require(path.join(temporary, "promotion-model.js"));

test("every supported locale distinguishes accrued commission from finalized records", () => {
  const locales = PROMOTION_LANGUAGES.map(([locale]) => locale);
  assert.equal(locales.length, 10);
  assert.deepEqual(Object.keys(PROMOTION_EARNINGS_COPY).sort(), [...locales].sort());
  const keys = ["accruedCommission", "finalizedCommission", "autoUpdate", "updatedAt", "seasonEnd", "settlementTiming", "finalizedNote"];
  for (const locale of locales) {
    const copy = PROMOTION_EARNINGS_COPY[locale];
    assert.deepEqual(Object.keys(copy).sort(), [...keys].sort(), locale);
    for (const key of keys) assert.ok(typeof copy[key] === "string" && copy[key].trim().length > 0, `${locale}.${key}`);
    assert.notEqual(copy.accruedCommission, copy.finalizedCommission, locale);
    assert.notEqual(copy.settlementTiming, copy.finalizedNote, locale);
  }
  assert.equal(PROMOTION_EARNINGS_COPY["zh-Hans"].accruedCommission, "本赛季累计佣金（待结算）");
  assert.equal(PROMOTION_EARNINGS_COPY["zh-Hans"].finalizedCommission, "本赛季已封账佣金");
  assert.equal(PROMOTION_EARNINGS_COPY.en.autoUpdate, "Updates every 10 seconds");
});

test("refresh time includes seconds in all supported locales", () => {
  const first = Date.UTC(2026, 9, 30, 4, 5, 6);
  for (const [locale] of PROMOTION_LANGUAGES) {
    const expected = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" }).format(new Date(first));
    assert.equal(formatPromotionRefreshTime(first, locale), expected, locale);
    assert.notEqual(formatPromotionRefreshTime(first, locale), formatPromotionRefreshTime(first + 1000, locale), `${locale} must display second-level changes`);
  }
});

test("missing, malformed and out-of-range refresh timestamps never imply a recent update", () => {
  for (const value of [undefined, null, "1793333106000", NaN, Infinity, -Infinity, -1, 0, 8.64e15 + 1]) {
    assert.equal(formatPromotionRefreshTime(value, "en"), "—", String(value));
  }
});
