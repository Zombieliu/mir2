import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const expected = ["zh-CN", "en", "ja", "ko", "zh-TW", "es", "pt-BR", "de", "fr", "ru", "pl", "tr", "th", "vi", "id", "ar"];
const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");
const siteCopy = read("../lib/site-copy.ts");
const marketCopy = read("../lib/market-copy.ts");
const downloadCopy = read("../lib/download-copy.ts");
const sitemap = read("../app/sitemap.ts");

for (const locale of expected) {
  if (![siteCopy, marketCopy].some((source) => source.includes(`"${locale}"`) || source.includes(`, ${locale},`))) throw new Error(`Website locale missing: ${locale}`);
  if (!downloadCopy.includes(`${JSON.stringify(locale)}:`) && !downloadCopy.includes(`  ${locale}:`)) throw new Error(`Download locale missing: ${locale}`);
  if (!["zh-CN", "en"].includes(locale) && !marketCopy.includes(`${JSON.stringify(locale)}:`) && !marketCopy.includes(`  ${locale}:`)) throw new Error(`Website market pack missing: ${locale}`);
}
if (!siteCopy.includes("createMarketSiteCopies(en)")) throw new Error("Generated site dictionaries are not connected");
if (!marketCopy.includes("ui: { official:")) throw new Error("Generated product UI dictionary is incomplete");
if (!sitemap.includes("locales.flatMap")) throw new Error("Sitemap is not locale-expanded");
if (!sitemap.includes('"/download"')) throw new Error("Download route missing from sitemap");

function files(path) {
  return readdirSync(path).flatMap((name) => {
    const child = join(path, name);
    return statSync(child).isDirectory() ? files(child) : [child];
  });
}
const appRoot = fileURLToPath(new URL("../app", import.meta.url));
for (const file of files(appRoot).filter((path) => /\.(ts|tsx)$/.test(path))) {
  const source = readFileSync(file, "utf8");
  const normalizedFile = file.replace(/\\/g, "/");
  if (normalizedFile.includes("/[locale]/") && !normalizedFile.endsWith("/layout.tsx") && /locale\s*[!=]==?\s*["']zh-CN["']\s*\?/.test(source)) throw new Error(`Binary zh-CN fallback remains in ${file}`);
  if (source.includes("Unable to submit. Please try again later.")) throw new Error(`Hard-coded English error remains in ${file}`);
}
console.log(`Website i18n check passed: ${expected.length} locales, localized sitemap, no binary zh-CN fallback.`);
