// Offline tests plus the small, allowlisted checks used by gameplay-web-stage.yml.
// Operational modes never serialize pulled environment files or deployment JSON.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { verifyPinnedRuntime, ASSET_BASE_URL, ASSET_VERSION, OBJECT_PREFIX } from "./verify-bevy-runtime-r2-upload.mjs";
import { assertCompiledBevyRuntimeManifest } from "./lib/bevy-runtime-build-identity.mjs";
import { assertCompiledClientCoreManifest, readClientCoreRelease } from "./lib/client-core-release-files.mjs";
import { bevyRuntimeLocalRewrite } from "../lib/bevy-runtime-local-route.ts";

export const TEAM_ID = "team_pXY7oaLtibSTlKSmKXMXzrQR";
export const PROJECT_ID = "prj_Bl3ttB4PzcdCpJMYLRhzQjkdXyTz";
export const RELEASE_REF = "refs/heads/codex/gameplay-bugfix-release-20261025";
const WEB_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const readJson = file => {
  try { return JSON.parse(fs.readFileSync(file, "utf8")); }
  catch { throw new Error("Required stage JSON is absent or invalid"); }
};

export function validateInputs({ expectedSha, githubSha, actualSha, ref, runtimeVersion, manifestSha256 }) {
  assert.match(expectedSha ?? "", /^[0-9a-f]{40}$/, "invalid reviewed commit SHA");
  assert.equal(githubSha, expectedSha, "dispatch SHA differs from the reviewed commit");
  assert.equal(actualSha, expectedSha, "checkout differs from the reviewed commit");
  assert.equal(ref, RELEASE_REF, "staging is restricted to the reviewed release branch");
  assert.match(runtimeVersion ?? "", /^bevy-[0-9a-f]{16}$/, "invalid reviewed runtime version");
  assert.match(manifestSha256 ?? "", /^[0-9a-f]{64}$/, "invalid reviewed runtime manifest SHA256");
}

export function validateTarget(orgId, projectId) {
  assert.equal(orgId, TEAM_ID, "Vercel team differs from the reviewed target");
  assert.equal(projectId, PROJECT_ID, "Vercel project differs from the reviewed target");
}

export function deriveBuildConfig(original, expectedSha) {
  assert.match(expectedSha ?? "", /^[0-9a-f]{40}$/, "invalid reviewed commit for build command");
  assert.equal(original.buildCommand, "bash ./scripts/vercel-build.sh", "unexpected original Vercel build command");
  return { ...original, buildCommand: `env MIR2_USE_PREBUILT_BEVY_RUNTIME=1 MIR2_BEVY_QUEST_UI=1 MIR2_BEVY_RUNTIME_BASE_URL=${ASSET_BASE_URL} MIR2_DEPLOY_REVISION=${expectedSha} bash ./scripts/vercel-build.sh` };
}

export function verifyCompiledIdentity(required, runtime, core, expectedSha) {
  assert.match(expectedSha ?? "", /^[0-9a-f]{40}$/, "invalid reviewed build revision");
  assertCompiledBevyRuntimeManifest(required, runtime);
  assertCompiledClientCoreManifest(required, core);
  const env = required.config?.env;
  assert.equal(env?.MIR2_BUILD_REVISION, expectedSha, "compiled revision differs from the reviewed commit");
  assert.equal(env?.MIR2_PINNED_ASSET_VERSION, ASSET_VERSION, "build did not use the pinned production asset version");
  assert.equal(env?.MIR2_PINNED_ASSET_OBJECT_PREFIX, OBJECT_PREFIX, "build asset prefix mismatch");
  assert.equal(env?.MIR2_PINNED_ASSET_BASE_URL, ASSET_BASE_URL, "build asset base URL mismatch");
  assert.equal(required.config?.distDir, ".next", "unexpected Vercel Next output directory");
  assert.notEqual(required.config?.output, "standalone", "downloadable standalone output is not the Vercel stage path");
  assert.notEqual(required.config?.typescript?.ignoreBuildErrors, true, "TypeScript errors must remain fatal");
}

export function verifyOutputLeaves(webRoot, outputRoot, leaves, runtimeManifest) {
  for (const leaf of leaves) {
    assert.match(leaf.relativePath, /^(?:bevy-runtime\/v\/bevy-[0-9a-f]{16}\/pkg-(?:webgpu|webgl2|webgl2-shared)\/mir2_bevy_runtime(?:_bg\.wasm|\.js)|client-core\/[0-9a-f]{64}\/mir2_platform_web(?:_bg\.wasm|\.js))$/, "undeclared static runtime path");
    // The public URL is logical; the actual Next proxy rewrites it to this
    // immutable public directory. Vercel copies that physical directory.
    const destination = leaf.relativePath.startsWith("bevy-runtime/v/")
      ? bevyRuntimeLocalRewrite(runtimeManifest, `/${leaf.relativePath}`) : `/${leaf.relativePath}`;
    assert.ok(destination, "compiled runtime rewrite refused the reviewed leaf");
    const target = fs.realpathSync(path.join(outputRoot, "static", ...destination.slice(1).split("/")));
    const relative = path.relative(fs.realpathSync(webRoot), target);
    assert.ok(relative && !relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative), "static runtime target escapes the Web root");
    assert.ok(fs.statSync(target).isFile(), "static runtime leaf is not a file");
    const bytes = fs.readFileSync(target);
    assert.equal(hash(bytes), leaf.sha256, "Vercel output runtime/Core leaf SHA256 mismatch");
    assert.equal(bytes.length, leaf.bytes ?? fs.statSync(leaf.localPath).size, "Vercel output runtime/Core leaf size mismatch");
  }
}

/** The private log is data only. Return fixed categories, never matching text. */
export function classifyBuildFailure(log, exitCode) {
  assert.equal(typeof log, "string", "invalid diagnostic log input");
  assert.ok(Number.isInteger(exitCode) && exitCode > 0 && exitCode <= 255, "invalid build exit code");
  const rules = [
    ["MISSING_AUTH", /No existing credentials|MISSING_AUTH|not authenticated|Please run [`'"]?vercel login/i],
    ["INVALID_TOKEN", /Invalid token|specified token is not valid|invalid authentication token|invalid-token/i],
    ["PATH_MISSING", /Root Directory[^\r\n]*(?:does not exist|not found)|rootDirectory[^\r\n]*(?:does not exist|not found)|ENOENT|no such file or directory/i],
    ["CONFIG_INVALID", /Invalid (?:vercel\.json|configuration|config)|Could not (?:read|parse)[^\r\n]*config|Failed to parse[^\r\n]*(?:config|vercel\.json)|config(?:uration)?[^\r\n]*schema/i],
    ["FRAMEWORK_MISSING", /No Next\.js version detected|(?:Could not|Unable to) detect[^\r\n]*framework|No framework detected|Framework[^\r\n]*not found/i],
    ["CLI_ARGUMENT_INVALID", /unknown or unexpected option|unknown option|Unknown argument|not a valid subcommand/i],
    ["BUILD_COMMAND_FAILED", /Command[^\r\n]*exited with (?:code|status)|Build command[^\r\n]*failed|BUILD_FAILED|Build failed|Failed to compile/i],
  ];
  const categories = rules.filter(([, pattern]) => pattern.test(log)).map(([category]) => category);
  return { scope: "vercel-build-failure", exitCode, categories: categories.length ? categories : ["UNCLASSIFIED"] };
}

function expectedInputs() {
  const actualSha = execFileSync("git", ["rev-parse", "HEAD"], { cwd: WEB_ROOT, encoding: "utf8" }).trim();
  validateInputs({ expectedSha: process.env.EXPECTED_COMMIT_SHA, githubSha: process.env.GITHUB_SHA,
    actualSha, ref: process.env.GITHUB_REF, runtimeVersion: process.env.EXPECTED_RUNTIME_VERSION,
    manifestSha256: process.env.EXPECTED_RUNTIME_MANIFEST_SHA256 });
  return actualSha;
}

function identities() {
  const pinned = verifyPinnedRuntime({ webRoot: WEB_ROOT, runtimeVersion: process.env.EXPECTED_RUNTIME_VERSION,
    manifestSha256: process.env.EXPECTED_RUNTIME_MANIFEST_SHA256 });
  const core = readClientCoreRelease({ webRoot: WEB_ROOT });
  assert.ok(core.manifest.presentation && core.manifest.npcPurchase, "all three Core packages are required");
  return { pinned, core, runtimeManifestSha256: hash(fs.readFileSync(path.join(WEB_ROOT, "lib/generated/bevy_runtime_version.json"))),
    coreManifestSha256: hash(fs.readFileSync(path.join(WEB_ROOT, "lib/generated/client_core_runtime.json"))) };
}

async function main(mode) {
  if (mode === "--classify-build-failure") {
    // Only the fixed private build-log filename is opened. No env-file reads.
    const exitCode = Number(process.env.BUILD_EXIT_CODE);
    try {
      const log = fs.readFileSync(path.join(process.env.RUNNER_TEMP, "gameplay-web-vercel-build.log"), "utf8");
      console.error(JSON.stringify(classifyBuildFailure(log, exitCode)));
    } catch {
      console.error(JSON.stringify({ scope: "vercel-build-failure", exitCode: Number.isInteger(exitCode) ? exitCode : null,
        categories: ["DIAGNOSTIC_UNAVAILABLE"] }));
    }
    return;
  }
  if (mode === "--target") {
    validateTarget(process.env.VERCEL_ORG_ID, process.env.VERCEL_PROJECT_ID);
    return;
  }
  if (mode === "--project") {
    const project = readJson(path.join(WEB_ROOT, ".vercel/project.json"));
    validateTarget(project.orgId, project.projectId);
    assert.equal(project.settings?.nodeVersion, "24.x", "pulled project Node version changed from reviewed 24.x");
    return;
  }
  const commit = expectedInputs();
  if (mode === "--prepare") {
    assert.equal(process.versions.node.split(".")[0], "24", "use the reviewed project Node major version");
    const { pinned, core, runtimeManifestSha256, coreManifestSha256 } = identities();
    fs.writeFileSync(process.env.STAGE_CONFIG_PATH,
      `${JSON.stringify(deriveBuildConfig(readJson(path.join(WEB_ROOT, "vercel.json")), commit), null, 2)}\n`, { flag: "wx" });
    fs.writeFileSync(process.env.STAGE_IDENTITY_PATH, JSON.stringify({ commit, runtimeManifestSha256,
      coreManifestSha256, runtimeVersion: pinned.normalized.version, coreVersion: core.manifest.version }), { flag: "wx" });
    return;
  }
  if (mode === "--verify-build") {
    const before = readJson(process.env.STAGE_IDENTITY_PATH);
    const { pinned, core, runtimeManifestSha256, coreManifestSha256 } = identities();
    assert.equal(before.commit, commit);
    assert.equal(runtimeManifestSha256, before.runtimeManifestSha256, "build changed the reviewed runtime identity");
    assert.equal(coreManifestSha256, before.coreManifestSha256, "build changed the reviewed Core identity");
    verifyCompiledIdentity(readJson(path.join(WEB_ROOT, ".next/required-server-files.json")), pinned.manifest, core.manifest, commit);
    const output = path.join(WEB_ROOT, ".vercel/output");
    assert.equal(readJson(path.join(output, "config.json")).version, 3, "missing Vercel Build Output API v3 output");
    verifyOutputLeaves(WEB_ROOT, output, [...pinned.files, ...core.files], pinned.manifest);
    const result = { ok: true, scope: "production-build-only", staged: false, commit, runtimeVersion: pinned.normalized.version,
      runtimeManifestSha256, coreManifestSha256, runtimeLeaves: pinned.files.length, coreLeaves: core.files.length };
    console.log(JSON.stringify(result));
    if (process.env.GITHUB_STEP_SUMMARY)
      fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `Production build only; no deployment created.\n\n\u0060\u0060\u0060json\n${JSON.stringify(result, null, 2)}\n\u0060\u0060\u0060\n`);
    return;
  }
  throw new Error("Unknown stage check mode");
}

if (process.argv.length > 2) {
  await main(process.argv[2]);
} else {
  const goodInputs = { expectedSha: "a".repeat(40), githubSha: "a".repeat(40), actualSha: "a".repeat(40),
    ref: RELEASE_REF, runtimeVersion: "bevy-0123456789abcdef", manifestSha256: "b".repeat(64) };
  test("dispatch is bound to exact commit, release branch and reviewed manifest input syntax", () => {
    validateInputs(goodInputs);
    for (const patch of [{ expectedSha: "main" }, { githubSha: "c".repeat(40) }, { actualSha: "c".repeat(40) },
      { ref: "refs/heads/main" }, { runtimeVersion: "bevy-abc;echo bad" }, { manifestSha256: "B".repeat(64) }])
      assert.throws(() => validateInputs({ ...goodInputs, ...patch }));
  });
  test("only the reviewed team/project are accepted", () => {
    validateTarget(TEAM_ID, PROJECT_ID);
    assert.throws(() => validateTarget("team_other", PROJECT_ID));
    assert.throws(() => validateTarget(TEAM_ID, "prj_other"));
  });
  test("build config preserves every existing property and only fixes buildCommand environments", () => {
    const original = readJson(path.join(WEB_ROOT, "vercel.json"));
    const result = deriveBuildConfig(original, goodInputs.expectedSha);
    assert.deepEqual({ ...result, buildCommand: original.buildCommand }, original);
    assert.equal(original.buildCommand, "bash ./scripts/vercel-build.sh");
    assert.equal(result.buildCommand, `env MIR2_USE_PREBUILT_BEVY_RUNTIME=1 MIR2_BEVY_QUEST_UI=1 MIR2_BEVY_RUNTIME_BASE_URL=${ASSET_BASE_URL} MIR2_DEPLOY_REVISION=${goodInputs.expectedSha} bash ./scripts/vercel-build.sh`);
    assert.throws(() => deriveBuildConfig({ ...original, buildCommand: "next build" }, goodInputs.expectedSha));
    assert.throws(() => deriveBuildConfig(original, "main;echo bad"));
  });
  test("compiled runtime/Core snapshots and production source pins must match", () => {
    const runtime = readJson(path.join(WEB_ROOT, "lib/generated/bevy_runtime_version.json"));
    const core = readJson(path.join(WEB_ROOT, "lib/generated/client_core_runtime.json"));
    const required = { config: { distDir: ".next", env: {
      MIR2_BEVY_RUNTIME_BUILD_MANIFEST: JSON.stringify(runtime), MIR2_CLIENT_CORE_BUILD_MANIFEST: JSON.stringify(core),
      MIR2_BUILD_REVISION: goodInputs.expectedSha,
      MIR2_PINNED_ASSET_VERSION: ASSET_VERSION, MIR2_PINNED_ASSET_OBJECT_PREFIX: OBJECT_PREFIX, MIR2_PINNED_ASSET_BASE_URL: ASSET_BASE_URL,
    } } };
    verifyCompiledIdentity(required, runtime, core, goodInputs.expectedSha);
    for (const mutate of [v => delete v.config.env.MIR2_BEVY_RUNTIME_BUILD_MANIFEST,
      v => delete v.config.env.MIR2_CLIENT_CORE_BUILD_MANIFEST,
      v => v.config.env.MIR2_BUILD_REVISION = "c".repeat(40),
      v => v.config.env.MIR2_PINNED_ASSET_OBJECT_PREFIX = "old-prefix",
      v => v.config.typescript = { ignoreBuildErrors: true }, v => v.config.output = "standalone"] ) {
      const bad = structuredClone(required); mutate(bad);
      assert.throws(() => verifyCompiledIdentity(bad, runtime, core, goodInputs.expectedSha));
    }
  });
  test("actual proxy maps the logical runtime URL to physical Vercel static bytes", t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-stage-test-"));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const output = path.join(root, ".vercel/output");
    const runtime = readJson(path.join(WEB_ROOT, "lib/generated/bevy_runtime_version.json"));
    const relativePath = `bevy-runtime/v/${runtime.version}/pkg-webgpu/mir2_bevy_runtime.js`;
    const physicalPath = `bevy-runtime-releases/${runtime.version}/pkg-webgpu/mir2_bevy_runtime.js`;
    assert.equal(bevyRuntimeLocalRewrite(runtime, `/${relativePath}`), `/${physicalPath}`);
    const localPath = path.join(output, "static", physicalPath);
    fs.mkdirSync(path.dirname(localPath), { recursive: true });
    fs.writeFileSync(localPath, "reviewed-bytes");
    const leaf = { relativePath, localPath, bytes: 15, sha256: hash(Buffer.from("reviewed-bytes")) };
    // The declared byte count is enforced independently of SHA256.
    assert.equal(fs.existsSync(path.join(output, "static", relativePath)), false, "logical URL need not exist in static output");
    assert.throws(() => verifyOutputLeaves(root, output, [leaf], runtime));
    leaf.bytes = Buffer.byteLength("reviewed-bytes");
    verifyOutputLeaves(root, output, [leaf], runtime);
    fs.writeFileSync(localPath, "changed-bytes");
    assert.throws(() => verifyOutputLeaves(root, output, [leaf], runtime));
    assert.throws(() => verifyOutputLeaves(root, output, [{ ...leaf, relativePath: "../../outside.js" }], runtime));
  });
  test("workflow isolates the credential to production pull and cannot create a deployment", () => {
    const workflow = fs.readFileSync(path.resolve(WEB_ROOT, "../../../.github/workflows/gameplay-web-stage.yml"), "utf8");
    assert.match(workflow, /branches: \[codex\/gameplay-bugfix-release-20261025\]/);
    assert.match(workflow, /paths: \[\.github\/workflows\/gameplay-web-stage\.yml\]/);
    assert.match(workflow, /if: \$\{\{ github\.event_name == 'push' \}\}/);
    assert.match(workflow, /if: \$\{\{ github\.event_name == 'workflow_dispatch' && github\.ref == 'refs\/heads\/codex\/gameplay-bugfix-release-20261025' \}\}/);
    assert.equal((workflow.match(/persist-credentials: false/g) ?? []).length, 2);
    assert.equal((workflow.match(/node-version: 24/g) ?? []).length, 2);
    assert.equal((workflow.match(/secrets\.VERCEL_TOKEN/g) ?? []).length, 1);
    const credentialStep = workflow.slice(workflow.indexOf("      - name: Pull existing production environment"), workflow.indexOf("      - name: Build the original Vercel production output"));
    assert.match(credentialStep, /node scripts\/test-gameplay-web-stage\.mjs --target[\s\S]*npx --yes vercel@56\.4\.1 pull --yes --environment=production[\s\S]*node scripts\/test-gameplay-web-stage\.mjs --project/);
    assert.ok(workflow.indexOf("--verifyRemote") < workflow.indexOf("secrets.VERCEL_TOKEN"));
    assert.match(workflow, /npx --yes vercel@56\.4\.1 build --prod --yes --local-config "\$STAGE_CONFIG_PATH"/);
    assert.match(workflow, /build_exit=\$\?[\s\S]*BUILD_EXIT_CODE="\$build_exit" node scripts\/test-gameplay-web-stage\.mjs --classify-build-failure[\s\S]*exit "\$build_exit"/);
    assert.match(workflow, /npm run vercel:prune-output -- --report/);
    assert.match(workflow, /node scripts\/test-gameplay-web-stage\.mjs --verify-build/);
    assert.doesNotMatch(workflow, /--token|upload-artifact|vercel@[^\s]+ (?:deploy|promote|alias|env|link)|wrangler|MIR2_R2_UPLOAD_SECRET|--force/);
  });
  test("private build diagnostics return only fixed categories and the exit code", () => {
    const secret = "PRIVATE_VALUE_MUST_NEVER_APPEAR";
    const cases = [
      ["No existing credentials found. Please run vercel login", "MISSING_AUTH"],
      ["Invalid token", "INVALID_TOKEN"],
      ["Root Directory /private/value does not exist", "PATH_MISSING"],
      ["Invalid vercel.json schema", "CONFIG_INVALID"],
      ["No Next.js version detected", "FRAMEWORK_MISSING"],
      ["unknown or unexpected option", "CLI_ARGUMENT_INVALID"],
      ["Command npm run build exited with code 7", "BUILD_COMMAND_FAILED"],
      ["unexpected unrelated message", "UNCLASSIFIED"],
    ];
    for (const [message, category] of cases) {
      const result = classifyBuildFailure(`${message}: ${secret}\nTOKEN=${secret}`, 7);
      assert.deepEqual(result, { scope: "vercel-build-failure", exitCode: 7, categories: [category] });
      assert.ok(!JSON.stringify(result).includes(secret));
      assert.ok(!JSON.stringify(result).includes("/private/value"));
    }
    assert.throws(() => classifyBuildFailure(secret, 0));
    assert.throws(() => classifyBuildFailure(secret, 256));
  });
  test("diagnostic CLI never echoes a private log value or path", t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-stage-diagnostic-"));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const secret = "PRIVATE_CLI_TEST_VALUE_MUST_NOT_APPEAR";
    fs.writeFileSync(path.join(root, "gameplay-web-vercel-build.log"), `No existing credentials found: ${secret}\n/private/${secret}\n`);
    const result = spawnSync(process.execPath, [fileURLToPath(import.meta.url), "--classify-build-failure"],
      { env: { ...process.env, RUNNER_TEMP: root, BUILD_EXIT_CODE: "7" }, encoding: "utf8" });
    assert.equal(result.status, 0);
    assert.equal(result.stdout, "");
    assert.ok(!result.stderr.includes(secret));
    assert.ok(!result.stderr.includes(root));
    const records = result.stderr.split(/\r?\n/).filter(line => line.startsWith('{"scope":'));
    assert.equal(records.length, 1);
    assert.deepEqual(JSON.parse(records[0]), { scope: "vercel-build-failure", exitCode: 7, categories: ["MISSING_AUTH"] });
  });
}
