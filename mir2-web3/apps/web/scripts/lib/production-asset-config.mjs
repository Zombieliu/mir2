import { lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

export const LEGACY_PRODUCTION_ASSET_CONFIG = "production-web-assets.json";
export const NUMERON_PRODUCTION_ASSET_CONFIG = "numeron-r16-web-assets.json";
const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../..");
const allowed = new Set([LEGACY_PRODUCTION_ASSET_CONFIG, NUMERON_PRODUCTION_ASSET_CONFIG]);

export function productionAssetConfigName(selection = process.env.MIR2_PRODUCTION_ASSET_CONFIG) {
  const name = selection === undefined || selection === "" ? LEGACY_PRODUCTION_ASSET_CONFIG : selection;
  if (typeof name !== "string" || !allowed.has(name)) {
    throw new Error("MIR2_PRODUCTION_ASSET_CONFIG must be production-web-assets.json or numeron-r16-web-assets.json (filename only)");
  }
  return name;
}

export function resolveProductionAssetConfigPath(root = projectRoot, selection = process.env.MIR2_PRODUCTION_ASSET_CONFIG) {
  const filename = path.resolve(root, "config", productionAssetConfigName(selection));
  const stat = lstatSync(filename);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error("Production asset profile must be a regular file");
  return filename;
}

export function assertNumeronFiniteProfile(config) {
  const prefix = "mir2/v/20261108-r16-08ec86d5105c", base = `https://assets.mir2.obelisk.build/${prefix}`;
  if (config?.schemaVersion !== 3 || config.profile !== "numeron-r16-finite"
    || config.version !== "20261108-r16-08ec86d5105c" || config.objectPrefix !== prefix || config.assetBaseUrl !== base
    || config.remoteReleaseUrl !== `${base}/remote-asset-release.json`
    || config.remoteReleaseSha256 !== "32255399aaae2dc9a133a5827252256ece782aaae5d237152ffebc2d8f272155"
    || config.bevyRuntime?.enabled !== true || config.bevyRuntime.version !== "bevy-0c553a754308e5b8"
    || config.bevyRuntime.assetBaseUrl !== base
    || config.fullCrystalPack?.enabled !== false || config.fullCrystalPack?.verified !== false
    || config.fullCrystalPack.contentHash !== null || config.fullCrystalPack.libraryCount !== 0 || config.fullCrystalPack.pageCount !== 0
    || config.mapAtlas?.enabled !== false || config.mapAtlas?.verified !== false
    || config.mapAtlas.contentHash !== null || config.mapAtlas.manifestPath !== null
    || config.mapAtlas.pageCount !== 0 || config.mapAtlas.maxPageBytes !== 0
    || config.finiteCoverage?.allMapsVerified !== false || config.finiteCoverage?.inheritedRemoteObjectsReverified !== false) {
    throw new Error("Numeron finite asset profile does not match its fixed release or disabled capabilities");
  }
}

export function loadProductionAssetConfig(root = projectRoot, selection = process.env.MIR2_PRODUCTION_ASSET_CONFIG) {
  const name = productionAssetConfigName(selection), filename = resolveProductionAssetConfigPath(root, name);
  const config = JSON.parse(readFileSync(filename, "utf8"));
  if (name === NUMERON_PRODUCTION_ASSET_CONFIG) assertNumeronFiniteProfile(config);
  return config;
}

export async function verifyProductionRemoteManifest(config, fetchImpl = globalThis.fetch) {
  if (config.remoteReleaseSha256 === undefined) return { verified: false, skipped: true, reason: "legacy profile has no added manifest hash gate" };
  if (!/^[a-f0-9]{64}$/.test(config.remoteReleaseSha256)) throw new Error("Invalid production remote manifest SHA256");
  const response = await fetchImpl(config.remoteReleaseUrl, { cache: "no-store", signal: AbortSignal.timeout(45000) });
  if (!response.ok || !response.body) throw new Error("Production remote manifest is unavailable");
  const hash = createHash("sha256"); let bytes = 0; const chunks = [];
  for await (const chunk of response.body) {
    bytes += chunk.length;
    if (bytes > 32 * 1024 * 1024) throw new Error("Production remote manifest exceeds the bounded read limit");
    hash.update(chunk); chunks.push(chunk);
  }
  const digest = hash.digest("hex");
  if (digest !== config.remoteReleaseSha256) throw new Error("Production remote manifest SHA256 mismatch");
  const manifest = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  if (manifest.version !== config.version || manifest.objectPrefix !== config.objectPrefix || manifest.assetBaseUrl !== config.assetBaseUrl) {
    throw new Error("Production remote manifest identity mismatch");
  }
  return { verified: true, bytes, sha256: digest };
}

async function main() {
  try {
    if (process.argv.length !== 3 || !["--path", "--verify-remote"].includes(process.argv[2])) throw new Error("Use --path or --verify-remote for the selected public asset profile");
    const config = loadProductionAssetConfig();
    if (process.argv[2] === "--path") console.log(resolveProductionAssetConfigPath());
    else console.log(JSON.stringify(await verifyProductionRemoteManifest(config)));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) void main();
