import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..", "..", "..");
const manifestPath = resolve(repoRoot, "packages", "game-data", "data", "generated", "crystal_game_shop_packet_manifest.json");
const outputPath = resolve(repoRoot, "apps", "web", "lib", "generated", "crystal-game-shop-data.ts");
const existing = readFileSync(outputPath, "utf8");
const infoMarker = "export const CRYSTAL_GAME_SHOP_ITEM_INFO_BY_INDEX = ";
const infoStart = existing.indexOf(infoMarker);
if (infoStart < 0) throw new Error("Missing item info map in generated game shop data");
const info = existing.slice(infoStart).trim();
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const header = "// Generated from packages/game-data/data/generated Crystal manifests.\n// Keep this app-local so Next dev can compile the client bundle without external directory imports.\n\n";
const rows = `export const CRYSTAL_GAME_SHOP_ITEMS = ${JSON.stringify(manifest.items, null, 2)} as const;\n\n`;
writeFileSync(outputPath, `${header}${rows}${info}\n`, "utf8");
console.log(`Generated ${manifest.items.length} Crystal game shop rows at ${outputPath}`);
