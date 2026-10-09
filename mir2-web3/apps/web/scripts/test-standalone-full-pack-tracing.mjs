import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { createRequire } from "node:module";
import { existsSync, lstatSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

const require = createRequire(import.meta.url);
const web = fileURLToPath(new URL("..", import.meta.url));
const qa = process.env.MIR2_STANDALONE_TRACE_QA_DIR;
assert.ok(qa && isAbsolute(qa), "MIR2_STANDALONE_TRACE_QA_DIR must name the owned QA fixture directory");
assert.equal(basename(qa), "implementation01");
assert.equal(basename(dirname(qa)), "repo-qa-portable-asset-trace-01");
const fixtures = join(qa, "fixtures");
const helperPath = join(web, "lib/standalone-full-pack-tracing.ts");
const configPath = join(web, "next.config.ts");
const picomatch = require(join(web, "node_modules/next/dist/compiled/picomatch"));
mkdirSync(fixtures, { recursive: true });

function loadSource(file, dependencies, simulatedImportMetaUrl) {
  const compiled = ts.transpileModule(readFileSync(file, "utf8"), {
    fileName: file,
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true, esModuleInterop: true },
    reportDiagnostics: true,
  });
  assert.deepEqual((compiled.diagnostics ?? []).filter(item => item.category === ts.DiagnosticCategory.Error), []);
  const output = simulatedImportMetaUrl
    ? compiled.outputText.replaceAll("import.meta.url", JSON.stringify(simulatedImportMetaUrl))
    : compiled.outputText;
  const module = { exports: {} };
  new Function("exports", "module", "require", output)(module.exports, module, name => {
    if (Object.hasOwn(dependencies, name)) return dependencies[name];
    if (name.startsWith("node:")) return require(name);
    throw new Error(`Unexpected transpiled production dependency: ${name}`);
  });
  return module.exports;
}

const { rewriteStandaloneFullPackTrace } = loadSource(helperPath, {});
const targetRelative = "server/app/api/asset-manifest/route.js.nft.json";

function fixture(name, nft = { version: 1, files: ["ordinary.json"] }) {
  const projectDir = join(fixtures, `${name}-${randomUUID()}`);
  const distDir = join(projectDir, ".next-test");
  const target = join(distDir, targetRelative);
  mkdirSync(join(distDir, "server/app/api/asset-manifest"), { recursive: true });
  if (nft !== null) writeFileSync(target, typeof nft === "string" ? nft : JSON.stringify(nft));
  return { projectDir, distDir, target, metadata: { projectDir, distDir } };
}

function untouched(caseInfo, before) {
  assert.deepEqual(readFileSync(caseInfo.target), before);
  assert.deepEqual(
    require("node:fs").readdirSync(join(caseInfo.distDir, "server/app/api/asset-manifest")).sort(),
    ["route.js.nft.json"],
  );
}

test("actual helper removes only complete full-pack segments and retains opaque rows, order, duplicates and fields", async () => {
  const kept = ["ordinary.json", "generated/crystal-packs/full-other", "generated/crystal-packs/full-other/index.json",
    "generated/crystal-packs/full.json", "generated/crystal-packs/fullish", "generated/crystal-packs/notfull/full",
    "generated-other/crystal-packs/full", "generated/crystal-packs/full/../ordinary.json", "ordinary.json"];
  const removed = ["..\\public\\generated\\crystal-packs\\full", "../../public/generated/crystal-packs/full/index.json",
    "F:\\mirror\\generated\\crystal-packs\\full\\index.json", "/mirror/generated/crystal-packs/full/deep/page",
    "generated/crystal-packs/full/"];
  const source = { version: 1, files: [kept[0], ...removed, ...kept.slice(1)], custom: { label: "retained", rows: [1, 2] } };
  const c = fixture("positive", source);
  const before = readFileSync(c.target);
  const audit = await rewriteStandaloneFullPackTrace(c.metadata, c.projectDir);
  const after = readFileSync(c.target);
  const output = JSON.parse(after);
  assert.deepEqual(output, { ...source, files: kept });
  assert.notDeepEqual(after, before);
  assert.equal(audit.target, targetRelative);
  assert.equal(audit.rowsBefore, source.files.length);
  assert.equal(audit.rowsAfter, kept.length);
  assert.equal(audit.removedDirectoryRows, 2);
  assert.equal(audit.removedDescendantRows, 3);
  assert.equal(audit.beforeSha256, require("node:crypto").createHash("sha256").update(before).digest("hex"));
  assert.equal(audit.afterSha256, require("node:crypto").createHash("sha256").update(after).digest("hex"));
  assert.deepEqual(require("node:fs").readdirSync(join(c.distDir, "server/app/api/asset-manifest")), ["route.js.nft.json"]);
});

test("zero-match route NFT remains byte-identical, including formatting", async () => {
  const raw = '{\n  "version": 1, "extra": {"x": 7},\n  "files": ["one.json", "one.json"]\n}\n';
  const c = fixture("no-match", raw);
  const before = readFileSync(c.target);
  const audit = await rewriteStandaloneFullPackTrace(c.metadata, c.projectDir);
  untouched(c, before);
  assert.equal(audit.beforeSha256, audit.afterSha256);
  assert.equal(audit.rowsBefore, audit.rowsAfter);
  assert.equal(audit.removedDirectoryRows + audit.removedDescendantRows, 0);
});

test("missing, malformed, schema, version and size budgets reject before writing", async () => {
  const cases = [
    ["missing", null], ["malformed", "{not-json"], ["array", "[]"],
    ["version", { version: 2, files: ["generated/crystal-packs/full"] }],
    ["empty-row", { version: 1, files: [""] }],
    ["nonstring", { version: 1, files: [7] }],
    ["too-many", { version: 1, files: Array(4097).fill("ordinary.json") }],
    ["too-large", `{"version":1,"files":["${"x".repeat(1024 * 1024)}"]}`],
  ];
  for (const [name, source] of cases) {
    const c = fixture(name, source);
    const before = source === null ? null : readFileSync(c.target);
    await assert.rejects(rewriteStandaloneFullPackTrace(c.metadata, c.projectDir), /Standalone full-pack trace:/);
    if (before) untouched(c, before);
    else assert.equal(existsSync(c.target), false);
  }
});

test("outside output, unexpected project and Turbopack marker fail closed", async () => {
  const c = fixture("boundary", { version: 1, files: ["generated/crystal-packs/full"] });
  const outside = fixture("outside", { version: 1, files: ["ordinary.json"] });
  const before = readFileSync(c.target);
  await assert.rejects(rewriteStandaloneFullPackTrace({ projectDir: c.projectDir, distDir: outside.distDir }, c.projectDir), /outside expected project/);
  await assert.rejects(rewriteStandaloneFullPackTrace(c.metadata, outside.projectDir), /outside expected project/);
  writeFileSync(join(c.distDir, "turbopack"), "owned marker");
  await assert.rejects(rewriteStandaloneFullPackTrace(c.metadata, c.projectDir), /Turbopack output is unsupported/);
  assert.deepEqual(readFileSync(c.target), before);
});

test("linked output parent and linked NFT are rejected without changing the target", async () => {
  const c = fixture("linked-parent", { version: 1, files: ["generated/crystal-packs/full"] });
  const parent = join(c.distDir, "server/app/api/asset-manifest");
  const other = fixture("linked-parent-target", { version: 1, files: ["ordinary.json"] });
  const fs = require("node:fs");
  fs.renameSync(parent, `${parent}-saved`);
  symlinkSync(join(other.distDir, "server/app/api/asset-manifest"), parent, "junction");
  await assert.rejects(rewriteStandaloneFullPackTrace(c.metadata, c.projectDir), /linked or non-directory output component/);
  assert.deepEqual(JSON.parse(readFileSync(other.target)), { version: 1, files: ["ordinary.json"] });

  const f = fixture("linked-file", null);
  const linkedTarget = fixture("linked-file-target", { version: 1, files: ["ordinary.json"] });
  symlinkSync(linkedTarget.target, f.target, "file");
  assert.equal(lstatSync(f.target).isSymbolicLink(), true);
  await assert.rejects(rewriteStandaloneFullPackTrace(f.metadata, f.projectDir), /linked or non-regular route NFT/);
  assert.deepEqual(JSON.parse(readFileSync(linkedTarget.target)), { version: 1, files: ["ordinary.json"] });
});

test("a checkout alias is accepted only at the project boundary", async () => {
  const c = fixture("alias", { version: 1, files: ["generated/crystal-packs/full"] });
  const alias = join(fixtures, `alias-link-${randomUUID()}`);
  symlinkSync(c.projectDir, alias, "junction");
  const audit = await rewriteStandaloneFullPackTrace({ projectDir: alias, distDir: join(alias, ".next-test") }, c.projectDir);
  assert.equal(audit.removedDirectoryRows, 1);
  assert.deepEqual(JSON.parse(readFileSync(c.target)).files, []);
});

function configFor(projectDir, nodeEnv, standalone) {
  const previous = { nodeEnv: process.env.NODE_ENV, standalone: process.env.MIR2_NEXT_STANDALONE, vercel: process.env.VERCEL };
  try {
    process.env.NODE_ENV = nodeEnv;
    process.env.MIR2_NEXT_STANDALONE = standalone;
    delete process.env.VERCEL;
    mkdirSync(join(projectDir, "lib/generated"), { recursive: true });
    writeFileSync(join(projectDir, "lib/generated/bevy_runtime_version.json"), "fixture-runtime");
    const dependencies = { "./lib/standalone-full-pack-tracing": { rewriteStandaloneFullPackTrace } };
    return loadSource(configPath, dependencies, pathToFileURL(join(projectDir, "next.config.ts")).href).default;
  } finally {
    for (const [key, value] of Object.entries({ NODE_ENV: previous.nodeEnv, MIR2_NEXT_STANDALONE: previous.standalone, VERCEL: previous.vercel })) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
  }
}

test("actual next.config gates hook, serial trace setting and one safe shared string glob", async () => {
  const c = fixture("config", { version: 1, files: ["generated/crystal-packs/full", "ordinary.json"] });
  const ordinary = configFor(c.projectDir, "production", "0");
  const development = configFor(c.projectDir, "development", "1");
  const production = configFor(c.projectDir, "production", "1");
  assert.equal(ordinary.compiler?.runAfterProductionCompile, undefined);
  assert.equal(ordinary.experimental?.parallelServerBuildTraces, undefined);
  assert.equal(ordinary.outputFileTracingExcludes["next-server"], undefined);
  assert.equal(development.compiler?.runAfterProductionCompile, undefined);
  assert.equal(development.experimental?.parallelServerBuildTraces, undefined);
  assert.equal(development.outputFileTracingExcludes["next-server"], undefined);
  assert.equal(production.output, "standalone");
  assert.equal(production.experimental.parallelServerBuildTraces, false);
  assert.deepEqual(production.outputFileTracingExcludes["next-server"], ["**/generated/crystal-packs/full/**"]);
  const log = console.log;
  const messages = [];
  console.log = message => messages.push(message);
  try {
    await production.compiler.runAfterProductionCompile(c.metadata);
  } finally {
    console.log = log;
  }
  assert.deepEqual(JSON.parse(readFileSync(c.target)).files, ["ordinary.json"]);
  assert.equal(messages.length, 1);
  assert.match(messages[0], /^\[mir2-full-pack-trace\] /);
  assert.equal(messages[0].includes(c.projectDir), false);
  assert.equal(messages[0].includes("index.json"), false);
});

test("installed Picomatch shared glob matches node and descendants without neighboring or traversal rows", () => {
  const glob = "**/generated/crystal-packs/full/**";
  const match = picomatch(glob, { contains: true, dot: true });
  for (const candidate of ["generated/crystal-packs/full", "generated/crystal-packs/full/index.json",
    "F:\\root\\generated\\crystal-packs\\full", "F:/root/generated/crystal-packs/full/deep/page"])
    assert.equal(match(candidate), true, candidate);
  for (const candidate of ["generated/crystal-packs/full-other", "generated/crystal-packs/full-other/index.json",
    "generated/crystal-packs/full.json", "generated/crystal-packs/fullish", "generated/crystal-packs/notfull/full",
    "generated-other/crystal-packs/full", "generated/crystal-packs/full/../ordinary.json"])
    assert.equal(match(candidate), false, candidate);
});
