import { readFile } from "node:fs/promises";
import process from "node:process";

const root = new URL("../", import.meta.url);
const [packageJson, tauriConfig, cargoToml] = await Promise.all([
  readFile(new URL("package.json", root), "utf8").then(JSON.parse),
  readFile(new URL("src-tauri/tauri.conf.json", root), "utf8").then(JSON.parse),
  readFile(new URL("src-tauri/Cargo.toml", root), "utf8"),
]);

const cargoVersion = cargoToml.match(/^version = "([^"]+)"$/m)?.[1];
const versions = new Set([packageJson.version, tauriConfig.version, cargoVersion]);
if (versions.size !== 1 || versions.has(undefined)) {
  throw new Error(
    `Desktop versions must match: package=${packageJson.version}, tauri=${tauriConfig.version}, cargo=${cargoVersion}`,
  );
}

const required = [
  "TAURI_SIGNING_PRIVATE_KEY",
  "DUBHE_NODE_UPDATER_PUBLIC_KEY",
  "DUBHE_NODE_UPDATE_STABLE_URL",
  "DUBHE_NODE_UPDATE_BETA_URL",
  "DUBHE_NODE_UPDATE_ROLLBACK_URL",
];
const missing = required.filter((name) => !process.env[name]?.trim());
if (missing.length > 0) {
  throw new Error(`Missing release configuration: ${missing.join(", ")}`);
}

for (const name of [
  "DUBHE_NODE_UPDATE_STABLE_URL",
  "DUBHE_NODE_UPDATE_BETA_URL",
  "DUBHE_NODE_UPDATE_ROLLBACK_URL",
]) {
  const endpoint = new URL(process.env[name]);
  if (endpoint.protocol !== "https:") {
    throw new Error(`${name} must use HTTPS`);
  }
}

if (!tauriConfig.bundle?.externalBin?.length || tauriConfig.bundle.externalBin.length !== 4) {
  throw new Error("Release must contain all four native Sidecars");
}

console.log(`DUBHE_NODE_RELEASE_CHECK_PASS version=${packageJson.version}`);
