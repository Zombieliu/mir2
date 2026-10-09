#!/usr/bin/env node
import path from "node:path";
import { fileURLToPath } from "node:url";
import { synchronizeOriginalSceneSpriteKeyIndex } from "./lib/original-scene-sprite-key-index.mjs";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
if (args.length > 1 || (args.length === 1 && args[0] !== "--check")) {
  throw new Error("Usage: generate-original-scene-sprite-key-index.mjs [--check]");
}
const result = await synchronizeOriginalSceneSpriteKeyIndex({
  manifestPath: path.join(webRoot, "public", "original-ui", "manifest.generated.json"),
  outputPath: path.join(webRoot, "lib", "generated", "original_scene_sprite_library_keys.json"),
  check: args[0] === "--check",
});
console.log(`UI sprite library key index ${args[0] === "--check" ? "checked" : result.changed ? "updated" : "unchanged"}: ${result.bytes} bytes`);
