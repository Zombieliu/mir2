import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..");
const sourceRoot = path.join(repoRoot, "tools", "crystal-gdi-text", "fixtures", "generated");
const outputRoot = path.join(scriptDir, "..", "public", "original-ui", "gdi-text");

const manifest = JSON.parse(await fs.readFile(path.join(sourceRoot, "manifest.json"), "utf8"));
if (manifest.schemaVersion !== 1 || !Array.isArray(manifest.assets) || manifest.assets.length === 0) {
  throw new Error("Crystal GDI text fixture manifest is missing or invalid.");
}

await fs.mkdir(path.join(outputRoot, "images"), { recursive: true });
await fs.copyFile(path.join(sourceRoot, "manifest.json"), path.join(outputRoot, "manifest.json"));
for (const asset of manifest.assets) {
  if (typeof asset.output !== "string" || !/^images\/[A-Za-z0-9._-]+\.png$/.test(asset.output)) {
    throw new Error(`Unsafe Crystal GDI text output path: ${String(asset.output)}`);
  }
  await fs.copyFile(
    path.join(sourceRoot, ...asset.output.split("/")),
    path.join(outputRoot, ...asset.output.split("/")),
  );
}

console.log(`Synced ${manifest.assets.length} Crystal GDI text assets.`);
