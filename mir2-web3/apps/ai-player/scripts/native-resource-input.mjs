// Read-only R16 resource input for an isolated, unpacked development EXE.
// Never edits an installation, selector, feed, signed generation or source assets.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { constants } from "node:fs";
import { copyFile, lstat, mkdir, readFile, realpath, stat, symlink, writeFile } from "node:fs/promises";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import { isolatedEnvironment, R16_GAMEPLAY_BASELINE, repoRoot, sha256 } from "./local-gateway.mjs";

// Out-of-band R16 anchors from docs/evidence/unified-playtest-publish-20261101/README.md.
// The pinned receipt binds the exact detached CMS, VERSION and payload manifest.
export const R16_RESOURCE_ANCHORS = Object.freeze({
  candidate: "WN-CANDIDATE-20261101-unified-16",
  receiptSha256: "07b2d572fe8303a881a67bc69b9abe688503fbeea4fcdeabe4cf5e8e2cffa739",
  versionSha256: "85606530619c3df66cd338e7ae6fb43b59dbf90debcc2872a10687b147dc6084",
});

export function nativeFixtureArguments(args = []) {
  const names = new Map([["--gateway-exe", "gatewayExecutable"], ["--native-exe", "nativeExecutable"],
    ["--candidate-root", "candidateRoot"], ["--release-control-root", "releaseControlRoot"], ["--native-output-root", "nativeOutputRoot"]]);
  const result = { launchNative: false };
  for (let index = 0; index < args.length; index += 1) {
    const key = args[index];
    if (key === "--launch-native" && !result.launchNative) { result.launchNative = true; continue; }
    if (key === "--live-openai-env" && !result.liveOpenaiFromEnvironment) { result.liveOpenaiFromEnvironment = true; continue; }
    const name = names.get(key), value = args[++index];
    if (!name || Object.hasOwn(result, name) || typeof value !== "string" || !value.trim() || value.startsWith("-")) throw new Error("Invalid native fixture arguments.");
    result[name] = resolve(value);
  }
  return result;
}

async function boundedFile(path, limit) {
  const info = await stat(path);
  if (!info.isFile() || info.size > limit) throw new Error("R16 metadata is missing or too large.");
  const bytes = await readFile(path);
  if (bytes.length > limit) throw new Error("R16 metadata exceeded its limit.");
  return bytes;
}
const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const inside = (root, path) => { const value = relative(root, path); return value !== "" && !value.startsWith("..") && !isAbsolute(value); };

export function resourceManifestEntries(manifest) {
  if (manifest?.schema !== "mir2.windows.package-manifest.v4" || !Array.isArray(manifest.files)
    || manifest.fileCount !== manifest.files.length || manifest.files.length > 300_000) throw new Error("Invalid R16 payload manifest.");
  const seen = new Set(), resources = [];
  for (const entry of manifest.files) {
    const path = entry?.path;
    if (typeof path !== "string" || !path || path.includes("\\") || path.includes(":") || path.startsWith("/")
      || path.split("/").some(part => !part || part === "." || part === "..") || /[\x00-\x1f]/.test(path)
      || !Number.isSafeInteger(entry.size) || entry.size < 0 || !/^[a-f\d]{64}$/i.test(entry.sha256 ?? "")
      || seen.has(path.toLowerCase())) throw new Error("Unsafe or duplicate payload entry.");
    seen.add(path.toLowerCase());
    if (path.startsWith("mir2-assets/")) resources.push({ path, size: entry.size, sha256: entry.sha256.toLowerCase() });
  }
  const required = ["crystal-map-pack/0.map.gz", "bevy-entity-atlases/manifest.json", "generated/map-atlas/manifest.json",
    "generated/native-map-keyed/manifest.json", "original-effects/effects.generated.json", "original-ui/Items/meta.json",
    "original-ui/Items/0.png", "original-ui/Items/3792.png", "original-ui/StateItem/meta.json", "original-ui/StateItem/30.png",
    "original-ui/StateItem/5152.png", "original-ui/Prguse2/meta.json", ...[1202, 1203, 1204, 1205].map(n => `original-ui/Prguse2/${n}.png`),
    ...["Default", "Normal_Atk", "Compulsion_Atk", "Npc"].map(name => `original-ui/Cursors/Cursor_${name}.png`)];
  if (required.some(path => !seen.has(`mir2-assets/${path}`.toLowerCase()))) throw new Error("R16 native startup resources are incomplete.");
  return resources;
}

export async function readR16ResourceMetadata({ candidateRoot, releaseControlRoot } = {}) {
  const control = resolve(releaseControlRoot ?? resolve(repoRoot, "../../mir2-release/unified-playtest-publish-20261101"));
  const receiptBytes = await boundedFile(resolve(control, "windows-r16/LOCAL-ARTIFACTS.json"), 1024 * 1024);
  if (digest(receiptBytes) !== R16_RESOURCE_ANCHORS.receiptSha256) throw new Error("The R16 receipt is not the pinned release input.");
  const receipt = JSON.parse(receiptBytes.toString("utf8").replace(/^\uFEFF/, ""));
  if (receipt.passed !== true || receipt.strictFullV4Passed !== true || receipt.candidate !== R16_RESOURCE_ANCHORS.candidate) throw new Error("R16 receipt did not pass its original gate.");
  const generationBytes = await boundedFile(resolve(control, "packed-r16/generations", R16_GAMEPLAY_BASELINE.generationManifestSha256, "GENERATION.json"), 64 * 1024);
  if (digest(generationBytes) !== R16_GAMEPLAY_BASELINE.generationManifestSha256) throw new Error("R16 generation differs from the canonical signed release.");
  const generation = JSON.parse(generationBytes.toString("utf8"));
  if (generation.contentProfile !== R16_GAMEPLAY_BASELINE.contentProfile) throw new Error("R16 generation gameplay profile differs.");
  const root = await realpath(resolve(candidateRoot ?? receipt.packageRoot));
  const metadata = {};
  for (const name of ["VERSION.json", "PACKAGE-MANIFEST.json", "RELEASE-STATEMENT.json", "RELEASE-STATEMENT.p7s"]) {
    const pin = receipt.metadata?.[name], bytes = await boundedFile(resolve(root, name), name === "PACKAGE-MANIFEST.json" ? 80 * 1024 * 1024 : 1024 * 1024);
    if (!pin || bytes.length !== pin.sizeBytes || digest(bytes) !== pin.sha256?.toLowerCase()) throw new Error("R16 signed metadata no longer matches its trusted receipt.");
    metadata[name] = bytes;
  }
  if (digest(metadata["VERSION.json"]) !== R16_RESOURCE_ANCHORS.versionSha256) throw new Error("R16 VERSION differs from the canonical input.");
  const version = JSON.parse(metadata["VERSION.json"].toString("utf8")), statement = JSON.parse(metadata["RELEASE-STATEMENT.json"].toString("utf8"));
  if (version.candidate !== R16_RESOURCE_ANCHORS.candidate || statement.candidate !== version.candidate
    || statement.versionSha256?.toLowerCase() !== R16_RESOURCE_ANCHORS.versionSha256
    || version.packageManifestSha256?.toLowerCase() !== digest(metadata["PACKAGE-MANIFEST.json"])
    || statement.packageManifestSha256 !== version.packageManifestSha256) throw new Error("R16 signed metadata binding differs.");
  const entries = resourceManifestEntries(JSON.parse(metadata["PACKAGE-MANIFEST.json"].toString("utf8")));
  return { root, assetRoot: await realpath(resolve(root, "mir2-assets")), candidate: version.candidate, entries,
    contentProfile: generation.contentProfile, generationManifestSha256: digest(generationBytes),
    versionSha256: R16_RESOURCE_ANCHORS.versionSha256, manifestSha256: digest(metadata["PACKAGE-MANIFEST.json"]),
    signatureSha256: digest(metadata["RELEASE-STATEMENT.p7s"]), receiptSha256: digest(receiptBytes) };
}

async function verifyPinnedDetachedCms(root) {
  if (process.platform !== "win32") throw new Error("The Windows R16 CMS check requires Windows.");
  const command = `$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Security; $root=$env:MIR2_FIXTURE_R16_ROOT; $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new([IO.File]::ReadAllBytes([IO.Path]::Combine($root,'RELEASE-STATEMENT.json'))),$true); $cms.Decode([IO.File]::ReadAllBytes([IO.Path]::Combine($root,'RELEASE-STATEMENT.p7s'))); if($cms.SignerInfos.Count -ne 1){throw 'Invalid R16 signer count'}; $cms.CheckSignature($true); $cert=$cms.SignerInfos[0].Certificate; $eku=$cert.Extensions | Where-Object {$_.Oid.Value -eq '2.5.29.37'} | Select-Object -First 1; if($null -eq $cert -or $cert.NotBefore.ToUniversalTime() -gt [DateTime]::UtcNow -or $cert.NotAfter.ToUniversalTime() -lt [DateTime]::UtcNow -or $null -eq $eku -or @($eku.EnhancedKeyUsages | ForEach-Object {$_.Value}) -notcontains '1.3.6.1.5.5.7.3.3'){throw 'Invalid R16 code-signing certificate'}; Write-Output 'r16-cms-valid';`;
  await new Promise((resolveDone, reject) => {
    const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(command, "utf16le").toString("base64")],
      { windowsHide: true, stdio: ["ignore", "pipe", "ignore"], env: isolatedEnvironment({ MIR2_FIXTURE_R16_ROOT: root }) });
    let output = "";
    const timer = setTimeout(() => { child.kill(); reject(new Error("R16 CMS verification timed out.")); }, 30_000);
    child.stdout.on("data", data => { output = (output + data).slice(-1024); });
    child.once("error", () => { clearTimeout(timer); reject(new Error("R16 CMS verifier could not start.")); });
    child.once("exit", code => { clearTimeout(timer); code === 0 && output.trim() === "r16-cms-valid" ? resolveDone() : reject(new Error("R16 detached CMS verification failed.")); });
  });
}

export async function verifyResourceEntry(root, entry) {
  const path = resolve(root, entry.path);
  if (!inside(root, path)) throw new Error("Resource path escaped its input package.");
  const info = await lstat(path);
  if (!info.isFile() || info.isSymbolicLink() || info.size !== entry.size || await sha256(path) !== entry.sha256) throw new Error("R16 resource bytes differ from the signed manifest.");
}

export async function verifyR16ResourceInput(options = {}) {
  const input = await readR16ResourceMetadata(options);
  await verifyPinnedDetachedCms(input.root);
  let index = 0, completed = 0;
  await Promise.all(Array.from({ length: 8 }, async () => {
    while (index < input.entries.length) {
      const entry = input.entries[index++]; await verifyResourceEntry(input.root, entry); completed += 1;
      if (completed % 8192 === 0) options.onProgress?.({ verifiedResources: completed, totalResources: input.entries.length });
    }
  }));
  return { ...input, cmsVerified: true, verifiedResourceCount: completed };
}

export async function assembleNativeFixture({ directory, nativeExecutable, resources }) {
  if (!resources?.cmsVerified || resources.verifiedResourceCount !== resources.entries.length || resources.entries.length === 0) throw new Error("Verify complete R16 resource bytes before assembly.");
  const sourceExe = await realpath(resolve(nativeExecutable ?? resolve(repoRoot, "apps/game-client/platform-windows/target/release/mir2-platform-windows.exe")));
  const requested = resolve(directory);
  // Resolve the existing parent before the fresh mkdir, including junctions.
  const destination = resolve(await realpath(dirname(requested)), basename(requested));
  if (destination === resources.root || inside(resources.root, destination) || destination === resources.assetRoot || inside(destination, resources.root)) throw new Error("The fixture must not overlap the R16 input.");
  if (!(await stat(sourceExe)).isFile()) throw new Error("The native EXE input is missing.");
  await mkdir(destination); // Fresh directory only. Never merge or overwrite a bundle.
  const exe = resolve(destination, "mir2-platform-windows.exe"), assetRoot = resolve(destination, "mir2-assets");
  await copyFile(sourceExe, exe, constants.COPYFILE_EXCL);
  await symlink(resources.assetRoot, assetRoot, process.platform === "win32" ? "junction" : "dir");
  const provenance = { schema: "mir2-private-native-fixture-resources/1", mode: "unpacked-development-exe-with-read-only-input-links",
    signedGenerationModified: false, publicInstallationModified: false, sourceExe, nativeSha256: await sha256(exe),
    resourceInput: { candidate: resources.candidate, root: resources.root, assetRoot: resources.assetRoot,
      contentProfile: resources.contentProfile, generationManifestSha256: resources.generationManifestSha256,
      receiptSha256: resources.receiptSha256, versionSha256: resources.versionSha256, manifestSha256: resources.manifestSha256,
      signatureSha256: resources.signatureSha256, cmsVerified: true, verifiedResourceCount: resources.verifiedResourceCount },
    limitation: "A private development fixture; not a new signed generation or an immutable independent resource copy." };
  await writeFile(resolve(destination, "resource-input.json"), JSON.stringify(provenance, null, 2) + "\n", { flag: "wx" });
  return { exe, assetRoot, provenance };
}
