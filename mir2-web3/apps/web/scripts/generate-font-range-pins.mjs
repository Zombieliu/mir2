import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

const sha = value => createHash("sha256").update(value).digest("hex");
const partBytes = 256 * 1024;
const originals = [
  ["/original-ui/fonts/NotoSansCJKsc-Regular.otf", 16_437_364,
    "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b", "font/otf"],
  ["/original-ui/fonts/NotoSansTC.ttf", 11_941_968,
    "864727d210d54f2537bbe23b3a839436c3992af72de9322af5270897246bd44f", "font/ttf"],
];
const pins = Object.fromEntries(originals.map(([font, size, digest, contentType]) => {
  const bytes = readFileSync(new URL(`../public${font}`, import.meta.url));
  assert.equal(bytes.length, size); assert.equal(sha(bytes), digest);
  const parts = [];
  for (let offset = 0; offset < bytes.length; offset += partBytes) {
    parts.push(sha(bytes.subarray(offset, Math.min(offset + partBytes, bytes.length))));
  }
  return [font, { bytes: bytes.length, sha256: digest, contentType, partBytes, parts }];
}));
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
console.log(JSON.stringify(Object.entries(pins).map(([font, pin]) => ({ font, ...pin, parts: pin.parts.length }))));
