#!/usr/bin/env node
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { FIXED_R16, FixedAssetError, downloadDraftBundle, extractFixedBundle, hashFile,
  publishValidated, requireFixed, validateDispatch, validateExtracted } from "./lib/fixed-r16-assets.mjs";

const execute = promisify(execFile), script = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(script), "../../../..");
const flags = new Map();
const receipt = { schemaVersion: 1, kind: "mir2-fixed-r16-publication-receipt", startedAt: new Date().toISOString(),
  passed: false, remoteWrites: 0, manifestPublished: false, materialSourceSha: FIXED_R16.materialSourceSha };
let output, outputCreated = false;

try {
  for (let index = 2; index < process.argv.length; index++) {
    const flag = process.argv[index];
    requireFixed(/^--[a-z][a-z0-9-]*$/.test(flag) && !flags.has(flag), "ARGUMENT_INVALID");
    if (["--dry-run", "--publish"].includes(flag)) flags.set(flag, true);
    else { requireFixed(index + 1 < process.argv.length && !process.argv[index + 1].startsWith("--"), "ARGUMENT_VALUE_MISSING"); flags.set(flag, process.argv[++index]); }
  }
  const allowed = ["--dry-run", "--publish", "--bundle", "--output-dir", "--source-sha", "--github-sha", "--repository",
    "--release-tag", "--bundle-sha256", "--payload-sha256", "--manifest-sha256"];
  requireFixed([...flags.keys()].every(key => allowed.includes(key)), "ARGUMENT_UNKNOWN");
  requireFixed(flags.has("--dry-run") !== flags.has("--publish"), "EXPLICIT_MODE_REQUIRED");
  const dryRun = flags.has("--dry-run");
  const input = { repository: flags.get("--repository") ?? process.env.GITHUB_REPOSITORY,
    sourceSha: flags.get("--source-sha"), githubSha: flags.get("--github-sha") ?? process.env.GITHUB_SHA,
    releaseTag: flags.get("--release-tag"), bundleSha256: flags.get("--bundle-sha256"),
    payloadSha256: flags.get("--payload-sha256"), manifestSha256: flags.get("--manifest-sha256") };
  validateDispatch(input);
  requireFixed(dryRun || (process.env.GITHUB_ACTIONS === "true" && input.githubSha === process.env.GITHUB_SHA), "PUBLISH_REQUIRES_PINNED_CI");
  requireFixed(!flags.has("--bundle") || dryRun, "LOCAL_BUNDLE_REVIEW_ONLY");
  requireFixed(typeof flags.get("--output-dir") === "string", "OUTPUT_DIRECTORY_REQUIRED");
  output = path.resolve(flags.get("--output-dir"));
  // Fresh owned output preserves every failed attempt without deleting old work.
  await fs.mkdir(output, { recursive: false, mode: 0o700 });
  outputCreated = true;
  const actualHead = (await execute("git", ["rev-parse", "HEAD"], { cwd: repoRoot })).stdout.trim();
  requireFixed(actualHead === input.sourceSha, "CHECKOUT_SOURCE_MISMATCH");
  const tracked = [path.relative(repoRoot, script).replaceAll(path.sep, "/"),
    "mir2-web3/apps/web/scripts/lib/fixed-r16-assets.mjs", ".github/workflows/web-assets-r2-release.yml"];
  const changed = (await execute("git", ["status", "--porcelain", "--", ...tracked], { cwd: repoRoot })).stdout;
  const sourceClean = changed.trim() === "";
  requireFixed(dryRun || sourceClean, "PUBLISH_SOURCE_DIRTY");
  Object.assign(receipt, { repository: input.repository, sourceSha: input.sourceSha, sourceClean, releaseTag: input.releaseTag,
    dryRun, bundleSha256: input.bundleSha256, payloadSha256: input.payloadSha256, manifestSha256: input.manifestSha256,
    publisherSha256: (await hashFile(script)).sha256,
    validatorSha256: (await hashFile(path.join(path.dirname(script), "lib", "fixed-r16-assets.mjs"))).sha256,
    node: process.version, zlib: process.versions.zlib });
  const onProgress = event => console.log(JSON.stringify(event));
  let bundle = flags.has("--bundle") ? path.resolve(flags.get("--bundle")) : path.join(output, FIXED_R16.bundleName);
  if (!flags.has("--bundle")) {
    receipt.draftSource = await downloadDraftBundle(input, bundle, process.env.GITHUB_TOKEN);
  }
  receipt.phase = "extract";
  const root = path.join(output, "extracted");
  const extracted = await extractFixedBundle(bundle, root, FIXED_R16, onProgress);
  receipt.phase = "validate";
  const validated = await validateExtracted(root, extracted, FIXED_R16, onProgress);
  receipt.phase = dryRun ? "dry-run" : "publish";
  await publishValidated(validated, { dryRun, receipt, onProgress,
    workerUrl: process.env.MIR2_R2_UPLOAD_WORKER_URL, secret: process.env.MIR2_R2_UPLOAD_SECRET });
  receipt.phase = "complete";
} catch (error) {
  receipt.passed = false;
  // Never print endpoint environment values, response bodies, signed GitHub URLs
  // or raw exception messages. Detailed safe verdicts are finite codes only.
  receipt.errorCode = error instanceof FixedAssetError ? error.code : `UNEXPECTED_${String(error?.name ?? "Error").replace(/[^A-Za-z0-9_]/g, "")}`;
  process.exitCode = 1;
} finally {
  receipt.finishedAt = new Date().toISOString();
  if (outputCreated) {
    try { await fs.writeFile(path.join(output, "receipt.json"), JSON.stringify(receipt, null, 2) + "\n", { flag: "wx", mode: 0o600 }); }
    catch { process.exitCode = 1; }
  }
  console.log(JSON.stringify(receipt));
}
