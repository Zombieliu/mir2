// Derives the Traditional Chinese (Taiwan) localization table from the committed
// Simplified Chinese (`zh-CN`) texts using OpenCC's `cn -> twp` profile, which
// applies both character conversion and Taiwanese vocabulary (e.g. 服务器 ->
// 伺服器, 在线 -> 線上, 设置 -> 設定). OpenCC only rewrites Han characters, so
// placeholders ({0}, {0:#,##0}), escape sequences (\n) and ASCII are preserved.
//
// Output is consumed by import-crystal-localization.mjs. Re-run this whenever the
// Simplified Chinese source changes:
//   npm i opencc-js   # (one-off; not a committed dependency)
//   node packages/tooling/scripts/generate-traditional-chinese.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import * as OpenCC from "opencc-js";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const bundlePath = resolve(
  repoRoot,
  "apps",
  "web",
  "lib",
  "generated",
  "localization_bundle.json",
);
const outputPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "localization",
  "chinese-traditional.json",
);

const bundle = JSON.parse(readFileSync(bundlePath, "utf8"));
const simplified = bundle.languages["zh-CN"].texts;
const convert = OpenCC.Converter({ from: "cn", to: "twp" });

const traditional = {};
for (const key of Object.keys(simplified).sort((left, right) =>
  left.localeCompare(right),
)) {
  traditional[key] = convert(simplified[key]);
}

writeFileSync(outputPath, `${JSON.stringify(traditional, null, 2)}\n`, "utf8");
console.log(`wrote ${outputPath} (${Object.keys(traditional).length} keys)`);
