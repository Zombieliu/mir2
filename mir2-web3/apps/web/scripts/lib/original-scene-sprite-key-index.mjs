import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";

export function createOriginalSceneSpriteKeyIndex(manifestBytes) {
  if (!Buffer.isBuffer(manifestBytes)) throw new Error("UI manifest must be read as raw bytes");
  let manifest;
  try { manifest = JSON.parse(manifestBytes.toString("utf8")); }
  catch { throw new Error("UI manifest is invalid JSON"); }
  const libraries = manifest?.libraries;
  if (!libraries || typeof libraries !== "object" || Array.isArray(libraries)) {
    throw new Error("UI manifest has no libraries object");
  }
  const rawKeys = Object.keys(libraries);
  if (rawKeys.length === 0 || rawKeys.some((key) => !key.trim())) {
    throw new Error("UI manifest has no valid library keys");
  }
  const keys = [...new Set(rawKeys.map((key) => key.replaceAll("\\", "/")))].sort();
  return {
    schemaVersion: 1,
    sourceSha256: createHash("sha256").update(manifestBytes).digest("hex"),
    keys,
  };
}

export function serializeOriginalSceneSpriteKeyIndex(index) {
  if (index?.schemaVersion !== 1 || !/^[a-f0-9]{64}$/.test(index.sourceSha256) ||
      !Array.isArray(index.keys) || !index.keys.length ||
      index.keys.some((key) => typeof key !== "string" || !key.trim()) ||
      new Set(index.keys).size !== index.keys.length ||
      index.keys.some((key, position) => position > 0 && index.keys[position - 1] >= key)) {
    throw new Error("Malformed UI sprite key index");
  }
  return `${JSON.stringify(index)}\n`;
}

export async function synchronizeOriginalSceneSpriteKeyIndex({ manifestPath, outputPath, check = false }) {
  const expected = serializeOriginalSceneSpriteKeyIndex(
    createOriginalSceneSpriteKeyIndex(await fs.readFile(manifestPath)),
  );
  let current = null;
  try { current = await fs.readFile(outputPath, "utf8"); }
  catch (error) { if (error?.code !== "ENOENT") throw error; }
  if (current === expected) return { changed: false, bytes: Buffer.byteLength(expected) };
  if (check) throw new Error(`UI sprite key index is missing, malformed or stale: ${outputPath}`);
  await fs.writeFile(outputPath, expected, "utf8");
  return { changed: true, bytes: Buffer.byteLength(expected) };
}
