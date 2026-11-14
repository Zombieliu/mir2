import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import ts from "typescript";
import { buildR16GameplayBaseline, projectR16GameplayBaseline, validateR16GameplayBaseline,
  assertR16GameplayBaselineProjections } from "../../../packages/tooling/scripts/generate-r16-gameplay-baseline.mjs";
import { R16_GAMEPLAY_BASELINE } from "../../ai-player/src/generated-gameplay-baseline.mjs";
import { R16_GAMEPLAY_BASELINE as gatewayBaseline, isolatedEnvironment } from "../../ai-player/scripts/local-gateway.mjs";

const ROOT = resolve(import.meta.dirname, "../../.."), read = file => readFileSync(resolve(ROOT, file), "utf8");
const expected = { schema: 1, id: "r16-platinum-fiveclass-newcomer-v2", contentProfile: "platinum_fiveclass",
  questGuidance: "newcomer-v2", questCadence: "newcomer-v2", onboarding: "native" };
const sourcePath = "packages/game-data/data/r16-gameplay-baseline.json";
const nodePath = "apps/ai-player/src/generated-gameplay-baseline.mjs", webPath = "apps/web/lib/generated/r16-gameplay-baseline.json";

test("one canonical public baseline projects byte-exact deterministic Node/Web data with an inert import-free frozen Node leaf", () => {
  const source = JSON.parse(read(sourcePath)), actual = buildR16GameplayBaseline();
  assert.deepEqual(source, expected); assert.deepEqual(actual.baseline, expected); assert.deepEqual(R16_GAMEPLAY_BASELINE, expected);
  assert.deepEqual(JSON.parse(read(webPath)), expected); assert.equal(Object.isFrozen(R16_GAMEPLAY_BASELINE), true);
  assert.throws(() => { R16_GAMEPLAY_BASELINE.questCadence = "crystal"; }, TypeError);
  assert.equal(actual.nodeModule, read(nodePath)); assert.equal(actual.webJson, read(webPath));
  const reordered = Object.fromEntries(Object.entries(source).reverse());
  assert.deepEqual(projectR16GameplayBaseline(reordered), projectR16GameplayBaseline(source));
  assert.deepEqual(buildR16GameplayBaseline({ readSource: file => read(file).replaceAll("\r\n", "\n").replaceAll("\n", "\r\n") }), actual);
  const file = ts.createSourceFile(nodePath, read(nodePath), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  assert.equal(file.statements.length, 1); assert(ts.isVariableStatement(file.statements[0]));
  const declaration = file.statements[0].declarationList.declarations[0];
  assert.equal(declaration.name.getText(file), "R16_GAMEPLAY_BASELINE"); assert(ts.isCallExpression(declaration.initializer));
  assert.equal(declaration.initializer.expression.getText(file), "Object.freeze"); assert.equal(declaration.initializer.arguments.length, 1);
  const literal = declaration.initializer.arguments[0]; assert(ts.isObjectLiteralExpression(literal));
  assert.equal(literal.properties.length, 6);
  assert(literal.properties.every(property => ts.isPropertyAssignment(property)
    && (ts.isStringLiteral(property.initializer) || ts.isNumericLiteral(property.initializer))));
});

test("schema, identity, content, guidance, cadence, onboarding, extra fields and generated-byte drift are rejected explicitly", () => {
  for (const [key, value] of [["schema", 2], ["id", "other-r16"], ["contentProfile", "platinum_176"],
    ["questGuidance", "crystal"], ["questCadence", "newcomer-v1"], ["onboarding", "cards"]])
    assert.throws(() => validateR16GameplayBaseline({ ...expected, [key]: value }), /contract drifted/);
  for (const value of [null, [], {}, { ...expected, privateData: true }, { ...expected, questCadence: undefined }])
    assert.throws(() => validateR16GameplayBaseline(value), /exact public fields|contract drifted/);
  const projections = projectR16GameplayBaseline(expected);
  assert.deepEqual(assertR16GameplayBaselineProjections(expected, projections), projections);
  for (const altered of [{ ...projections, webJson: projections.webJson.replace("newcomer-v2", "crystal") },
    { ...projections, nodeModule: projections.nodeModule + "\n" }, { ...projections, nodeModule: "export const R16_GAMEPLAY_BASELINE = {};\n" }])
    assert.throws(() => assertR16GameplayBaselineProjections(expected, altered), /stale or modified/);
  for (const changed of ["config/quest-guidance/newcomer-v2.json", "config/quest-guidance/newcomer-journey-v2.json"])
    assert.throws(() => buildR16GameplayBaseline({ readSource(file) {
      const text = read(file); return file === changed ? JSON.stringify({ ...JSON.parse(text), profile: "crystal" }) : text;
    } }), /Native .* configuration drifted/);
});

test("the existing Windows resource export retains its signed generation pin while taking every gameplay field from the shared leaf", () => {
  for (const [key, value] of Object.entries(R16_GAMEPLAY_BASELINE)) assert.equal(gatewayBaseline[key], value, key);
  assert.equal(Object.isFrozen(gatewayBaseline), true); assert.equal(gatewayBaseline.profileVersion, 1);
  assert.equal(gatewayBaseline.generationManifestSha256, "56ddd5417a09a2515fece9a36be2607282084ec89058c412d2edd7a7dcb21bc8");
});

function gatewayFixture() {
  const file = ts.createSourceFile("local-gateway.mjs", read("apps/ai-player/scripts/local-gateway.mjs"), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const declarations = file.statements.filter(node => ts.isFunctionDeclaration(node) && node.name?.text === "startLocalGateway");
  assert.equal(declarations.length, 1, "run the real exported Windows launcher source, never a rewritten environment mirror");
  const calls = { spawn: [], files: [], health: [], closes: [] }, output = resolve(ROOT, "in-memory-gateway-only");
  const child = Object.assign(new EventEmitter(), { exitCode: null, signalCode: null, stdout: new EventEmitter(), stderr: new EventEmitter() });
  const ambient = { PATH: "fixture-system-path", HTTPS_PROXY: "http://proxy.invalid:3128", DATABASE_URL: "fixture-database",
    MIR2_CONTENT_PROFILE: "crystal_full", MIR2_QUEST_CADENCE: "newcomer-v1", NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: "crystal",
    MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: "1", OPENAI_API_KEY: "fixture-only" };
  const scope = {
    R16_GAMEPLAY_BASELINE: gatewayBaseline, repoRoot: ROOT, resolve,
    resolveGatewayExecutable: async executable => executable ?? "fixture-gateway.exe",
    randomUUID: () => "11111111-2222-3333-4444-555555555555", randomBytes: count => Buffer.alloc(count, 0x41),
    prepareLocalGatewayPaths: async () => ({ output, outputRoot: ROOT, accountStorePath: resolve(output, "accounts.json") }),
    writeFile: (...args) => { calls.files.push(args); assert.fail("this no-policy environment test must not write files"); },
    localSupplyServerEnvironment: () => assert.fail("supply configuration is outside this test"),
    freePort: async () => 54321,
    spawn: (...args) => { calls.spawn.push(args); return child; },
    isolatedEnvironment: overrides => isolatedEnvironment(overrides, ambient),
    fetch: async (...args) => { calls.health.push(args); return { ok: true }; },
    AbortSignal: { timeout: ms => { assert.equal(ms, 1000); return { fixture: "timeout" }; } },
    sleep: () => assert.fail("the fake healthy child requires no wait"),
    closeOwnedGatewayChild: async child => { calls.closes.push(child); },
  };
  const exports = {}, code = ts.transpileModule(declarations[0].getText(file), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
  new Function("exports", ...Object.keys(scope), code)(exports, ...Object.values(scope));
  return { start: exports.startLocalGateway, calls, child, ambient };
}

test("the real local gateway spawn uses the R16 content and V2 cadence without inheriting foreign gameplay/auth/model settings", async () => {
  const f = gatewayFixture(), result = await f.start();
  assert.equal(f.calls.spawn.length, 1); assert.equal(f.calls.files.length, 0);
  const [executable, args, options] = f.calls.spawn[0];
  assert.equal(executable, "fixture-gateway.exe"); assert.deepEqual(args, []); assert.equal(options.windowsHide, true);
  assert.equal(options.env.MIR2_CONTENT_PROFILE, R16_GAMEPLAY_BASELINE.contentProfile);
  assert.equal(options.env.MIR2_QUEST_CADENCE, R16_GAMEPLAY_BASELINE.questCadence);
  assert.equal(options.env.MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS, "0"); assert.equal(options.env.MIR2_AI_LIVE_ENABLED, "0");
  assert.equal(options.env.MIR2_ACCOUNT_STORE_BACKEND, "file"); assert.equal(options.env.MIR2_ZONE_ROUTING_MODE, "single");
  assert.equal(options.env.MIR2_GATEWAY_STDIN_SHUTDOWN, "1");
  for (const key of ["HTTPS_PROXY", "DATABASE_URL", "OPENAI_API_KEY", "NEXT_PUBLIC_MIR2_QUEST_GUIDANCE"]) assert.equal(options.env[key], undefined, key);
  assert.equal(options.env.PATH, f.ambient.PATH);
  assert.equal(result.contentProfile, expected.contentProfile); assert.equal(result.gameplayBaseline.matchesR16Profile, true);
  for (const [key, value] of Object.entries(expected)) assert.equal(result.gameplayBaseline[key], value, key);
  assert.equal(f.calls.health.length, 1); assert.equal(f.calls.health[0][0], "http://127.0.0.1:54321/health");
  await result.close(); assert.deepEqual(f.calls.closes, [f.child]);
});

test("existing explicit fixture profiles remain explicit and never claim to be the R16 baseline", async () => {
  for (const contentProfile of ["platinum_176", "crystal_full"]) {
    const f = gatewayFixture(), result = await f.start({ contentProfile });
    assert.equal(result.contentProfile, contentProfile); assert.equal(result.gameplayBaseline.matchesR16Profile, false);
    assert.equal(f.calls.spawn[0][2].env.MIR2_CONTENT_PROFILE, contentProfile);
    assert.equal(f.calls.spawn[0][2].env.MIR2_QUEST_CADENCE, expected.questCadence);
    await result.close();
  }
  const invalid = gatewayFixture(); await assert.rejects(invalid.start({ contentProfile: "unknown-profile" }), /explicit supported/);
  assert.equal(invalid.calls.spawn.length, 0); assert.equal(invalid.calls.health.length, 0); assert.equal(invalid.calls.files.length, 0);
});
