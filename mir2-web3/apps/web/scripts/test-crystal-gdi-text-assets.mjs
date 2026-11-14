import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..");
const fixtureRoot = path.join(repoRoot, "tools", "crystal-gdi-text", "fixtures", "generated");
const publicRoot = path.join(scriptDir, "..", "public", "original-ui", "gdi-text");

const fixtureManifestBytes = await fs.readFile(path.join(fixtureRoot, "manifest.json"));
const publicManifestBytes = await fs.readFile(path.join(publicRoot, "manifest.json"));
assert.deepEqual(publicManifestBytes, fixtureManifestBytes, "public GDI manifest drifted from fixture baseline");

const manifest = JSON.parse(publicManifestBytes.toString("utf8"));
assert.equal(manifest.schemaVersion, 1);
assert.equal(manifest.generator?.renderer, "System.Windows.Forms.TextRenderer");
assert.equal(manifest.font?.resolvedFamily, "Arial");
assert.equal(manifest.font?.sizePoints, 8);
assert.equal(manifest.font?.dpi, 96);

const keys = new Set();
for (const asset of manifest.assets) {
  assert.match(asset.output, /^images\/[A-Za-z0-9._-]+\.png$/);
  assert.equal(keys.has(asset.key), false, `duplicate GDI key ${asset.key}`);
  keys.add(asset.key);
  const fixturePng = await fs.readFile(path.join(fixtureRoot, ...asset.output.split("/")));
  const publicPng = await fs.readFile(path.join(publicRoot, ...asset.output.split("/")));
  assert.deepEqual(publicPng, fixturePng, `${asset.key} public PNG drifted from fixture`);
  assert.equal(sha256(publicPng), asset.hash.png, `${asset.key} PNG hash mismatch`);
}

for (const required of [
  "entity-assistant-jane-auto",
  "entity-merchant-auto",
  "entity-ruben-auto",
  "entity-assistant-jane",
  "entity-merchant-ruben-multiline",
  "hud-hp",
  "minimap-coordinate",
  "chat-online-players",
  "chat-line-message-net-8",
  "pt-br-accents",
]) {
  assert.equal(keys.has(required), true, `missing required GDI fixture ${required}`);
}

const onlinePlayers = manifest.assets.find((asset) => asset.key === "chat-online-players");
for (const key of ["entity-assistant-jane-auto", "entity-merchant-auto", "entity-ruben-auto"]) {
  const name = manifest.assets.find((asset) => asset.key === key);
  assert.equal(name.size.mode, "auto", `${key} must follow Crystal MirLabel AutoSize`);
  assert.equal(name.size.output.width, name.size.measured.width + 2, `${key} must retain the full measured string plus outline`);
  assert.equal(name.size.output.height, name.size.measured.height + 2);
}
const jane = manifest.assets.find((asset) => asset.key === "entity-assistant-jane-auto");
const fixedJane = manifest.assets.find((asset) => asset.key === "entity-assistant-jane");
assert(jane.size.output.width > fixedJane.size.output.width, "a 50px fixed label must not cap an AutoSize world name");
assert.equal(fixedJane.size.mode, "fixed", "fixed-size UI requests keep their original fixture");
assert.equal(fixedJane.size.output.width, 50);
assert.equal(manifest.assets.find((asset) => asset.key === "entity-ruben-auto").foreground, "#FFFFFFFF",
  "Crystal NPC underscore subnames use white rather than the primary name colour");
assert.equal(onlinePlayers.background, "#FFFFFFFF");
const lineMessage = manifest.assets.find((asset) => asset.key === "chat-line-message-net-8");
assert.equal(lineMessage.foreground, "#FFFFFFFF");
assert.equal(lineMessage.background, "#FF0000FF");

const componentSource = await fs.readFile(
  path.join(scriptDir, "..", "app", "components", "crystal-gdi-text.tsx"),
  "utf8",
);
assert.match(componentSource, /data-crystal-gdi-text/);
assert.match(componentSource, /cssColourToArgb/);

console.log(`Crystal GDI text asset tests passed: ${keys.size} exact full-string assets.`);

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}
