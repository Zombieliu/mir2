import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

const font = "/original-ui/fonts/NotoSansCJKsc-Regular.otf";
const bytes = readFileSync(new URL(`../public${font}`, import.meta.url));
const sha = value => createHash("sha256").update(value).digest("hex");
assert.equal(bytes.length, 16_437_364);
assert.equal(sha(bytes), "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b");
const partBytes = 256 * 1024;
const parts = [];
for (let offset = 0; offset < bytes.length; offset += partBytes) {
  parts.push(sha(bytes.subarray(offset, Math.min(offset + partBytes, bytes.length))));
}
const pins = { [font]: { bytes: bytes.length, sha256: sha(bytes), partBytes, parts } };
const worker = new URL("../public/mir2-asset-worker.js", import.meta.url);
const source = readFileSync(worker, "utf8");
const start = "// BEGIN GENERATED FONT RANGE PINS";
const end = "// END GENERATED FONT RANGE PINS";
const generated = `${start}\nconst PINNED_FONT_RANGES = ${JSON.stringify(pins, null, 2)};\n${end}`;
const expression = /\/\/ BEGIN GENERATED FONT RANGE PINS[\s\S]*?\/\/ END GENERATED FONT RANGE PINS/;
const next = expression.test(source) ? source.replace(expression, generated)
  : source.replace('const DEFAULT_VERSION = "bootstrap";', `const DEFAULT_VERSION = "bootstrap";\n${generated}`);
assert(next.includes(generated));
if (process.argv.includes("--check")) assert.equal(source, next, "font pins need regeneration");
else writeFileSync(worker, next);
console.log(JSON.stringify({ font, bytes: bytes.length, sha256: sha(bytes), partBytes, parts: parts.length }));
