import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  buildTemporalPackPlan,
  loadTemporalPackScenario,
  redactArgv,
  runTemporalPack,
  validateTemporalPackScenario,
} from "./capture-crystal-temporal-pack.mjs";
import {
  findMatchingSceneEffect,
  normalizeSceneEffectPhaseGate,
  waitForSceneEffectPhase,
} from "./capture-web-movement-jitter.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT_PATH = path.join(SCRIPT_DIR, "capture-crystal-temporal-pack.mjs");
const SCENARIO_PATH = path.join(SCRIPT_DIR, "scenarios", "bichon-332275-left4.json");

test("unit: builds a deterministic three-phase plan with managed handoffs", async () => {
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  const outputDir = path.join(os.tmpdir(), "mir2-temporal-pack-unit-plan");
  const first = buildTemporalPackPlan({
    scenario: loaded.scenario,
    scenarioPath: loaded.path,
    outputDir,
    dryRun: true,
  });
  const second = buildTemporalPackPlan({
    scenario: loaded.scenario,
    scenarioPath: loaded.path,
    outputDir,
    dryRun: true,
  });

  assert.deepEqual(first.manifest, second.manifest);
  assert.deepEqual(first.manifest.phaseOrder, ["native", "web", "report"]);
  assert.equal(first.manifest.phases.native.status, "planned");
  assert.equal(first.manifest.phases.web.status, "planned");
  assert.equal(first.manifest.phases.report.status, "planned");
  assert.match(first.phasePlans.native.argv[0], /capture-original-computer-use\.mjs$/);
  assert.match(first.phasePlans.web.argv[0], /capture-web-movement-jitter\.mjs$/);
  assert.match(first.phasePlans.report.argv[0], /report-movement-temporal-parity\.mjs$/);
  assert.equal(argValue(first.phasePlans.report.argv, "original"), first.phasePlans.native.artifacts.jsonPath);
  assert.equal(argValue(first.phasePlans.report.argv, "web"), first.phasePlans.web.artifacts.jsonPath);
});

test("unit: phase overrides are explicit and do not infer app launches", async () => {
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  const plan = buildTemporalPackPlan({
    scenario: loaded.scenario,
    scenarioPath: loaded.path,
    outputDir: path.join(os.tmpdir(), "mir2-temporal-pack-unit-web-only"),
    dryRun: true,
    phaseOverrides: { native: false, web: true, report: false },
  });
  assert.deepEqual(
    Object.fromEntries(Object.entries(plan.manifest.phases).map(([name, phase]) => [name, phase.status])),
    { native: "skipped", web: "planned", report: "skipped" },
  );
});

test("unit: report-only plans reuse deterministic pack artifacts and remain fail-closed at execution", async () => {
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  const plan = buildTemporalPackPlan({
    scenario: loaded.scenario,
    scenarioPath: loaded.path,
    outputDir: path.join(os.tmpdir(), "mir2-temporal-pack-unit-report-only"),
    dryRun: true,
    phaseOverrides: { native: false, web: false, report: true },
  });
  assert.equal(argValue(plan.phasePlans.report.argv, "original"), plan.phasePlans.native.artifacts.jsonPath);
  assert.equal(argValue(plan.phasePlans.report.argv, "web"), plan.phasePlans.web.artifacts.jsonPath);
});

test("unit: argv and manifest redact credentials and URL query secrets", async () => {
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  const scenario = structuredClone(loaded.scenario);
  scenario.phases.web.args.account = "private-account";
  scenario.phases.web.args.password = "private-password";
  scenario.phases.web.args.qaControlToken = "private-token";
  scenario.phases.web.args.baseUrl = "http://127.0.0.1:3002/?token=private-token";
  const plan = buildTemporalPackPlan({
    scenario,
    scenarioPath: loaded.path,
    outputDir: path.join(os.tmpdir(), "mir2-temporal-pack-unit-redaction"),
    dryRun: true,
  });
  const serialized = JSON.stringify(plan.manifest);
  for (const secret of ["private-account", "private-password", "private-token"]) {
    assert.equal(serialized.includes(secret), false);
  }
  assert.equal(plan.manifest.scenario.config.phases.web.args.account, "[redacted]");
  assert.equal(argValue(plan.manifest.phases.web.command.argv, "account"), "[redacted]");
  assert.equal(argValue(plan.manifest.phases.web.command.argv, "password"), "[redacted]");
  assert.equal(argValue(plan.manifest.phases.web.command.argv, "qaControlToken"), "[redacted]");
  assert.equal(
    redactArgv(["script.mjs", "--password", "secret-value"], new Set(["secret-value"]))[2],
    "[redacted]",
  );
});

test("unit: validation fails closed on unknown fields, args, and malformed routes", async () => {
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  for (const mutate of [
    (scenario) => { scenario.unexpected = true; },
    (scenario) => { scenario.phases.web.args.launchAnything = "yes"; },
    (scenario) => { scenario.phases.native.args.route = "not-a-route"; },
    (scenario) => { scenario.phases.web.args.sampleMs = "50"; },
    (scenario) => { scenario.phases.report.args.preActionMs = -1; },
    (scenario) => { scenario.phases.web.args.frameImageFormat = "gif"; },
  ]) {
    const scenario = structuredClone(loaded.scenario);
    mutate(scenario);
    assert.throws(() => validateTemporalPackScenario(scenario));
  }
});

test("unit: scene-effect phase gate is opt-in and validates the complete request", () => {
  assert.deepEqual(normalizeSceneEffectPhaseGate(), {
    enabled: false,
    requested: { name: null, frame: null },
    timeoutMs: null,
  });
  assert.deepEqual(
    normalizeSceneEffectPhaseGate({ name: "map", frame: "328", timeoutMs: "2500" }),
    {
      enabled: true,
      requested: { name: "map", frame: 328 },
      timeoutMs: 2500,
    },
  );
  assert.throws(() => normalizeSceneEffectPhaseGate({ name: "map" }), /must be provided together/);
  assert.throws(() => normalizeSceneEffectPhaseGate({ frame: 328 }), /must be provided together/);
  assert.throws(
    () => normalizeSceneEffectPhaseGate({ timeoutMs: 1000 }),
    /requires sceneEffectName and sceneEffectFrame/,
  );
  assert.throws(
    () => normalizeSceneEffectPhaseGate({ name: "map", frame: 1.5 }),
    /non-negative integer/,
  );
});

test("unit: scene-effect phase gate waits for a visible matching source/name and image frame", async () => {
  const gate = normalizeSceneEffectPhaseGate({ name: "TownAura", frame: 328, timeoutMs: 100 });
  let nowMs = 1_000;
  const observations = [
    [{ effectSource: "map", effectName: "TownAura", src: "/original-effects/Effect/328.png", frame: 328, visible: false }],
    [{ effectSource: "map", effectName: "TownAura", src: "/original-effects/Effect/329.png", frame: 329, visible: true }],
    [{ effectSource: "map", effectName: "TownAura", src: "/original-effects/Effect/328.png", frame: 328, visible: true }],
  ];
  const client = { evaluate: async () => observations.shift() ?? [] };
  const evidence = await waitForSceneEffectPhase(client, gate, {
    now: () => nowMs,
    sleep: async (ms) => { nowMs += ms; },
    pollMs: 16,
  });

  assert.equal(evidence.success, true);
  assert.equal(evidence.waitedMs, 32);
  assert.deepEqual(evidence.requested, { name: "TownAura", frame: 328 });
  assert.equal(evidence.matched.effectSource, "map");
  assert.equal(evidence.matched.effectName, "TownAura");
  assert.equal(evidence.matched.src, "/original-effects/Effect/328.png");
  assert.equal(evidence.matched.frame, 328);
  assert.equal(
    findMatchingSceneEffect(
      [{ effectSource: "map", effectName: "Other", src: "/original-effects/Effect/328.png", visible: true }],
      normalizeSceneEffectPhaseGate({ name: "map", frame: 328 }),
    )?.effectSource,
    "map",
  );
});

test("unit: scene-effect phase gate fails strictly on timeout with failure evidence", async () => {
  const gate = normalizeSceneEffectPhaseGate({ name: "MissingAura", frame: 777, timeoutMs: 20 });
  let nowMs = 2_000;
  const client = { evaluate: async () => [] };

  await assert.rejects(
    waitForSceneEffectPhase(client, gate, {
      now: () => nowMs,
      sleep: async (ms) => { nowMs += ms; },
      pollMs: 8,
    }),
    (error) => {
      assert.match(error.message, /Timed out waiting for visible scene effect MissingAura frame 777/);
      assert.equal(error.sceneEffectPhaseGate.success, false);
      assert.equal(error.sceneEffectPhaseGate.waitedMs, 20);
      assert.deepEqual(error.sceneEffectPhaseGate.requested, { name: "MissingAura", frame: 777 });
      assert.equal(error.sceneEffectPhaseGate.matched, null);
      return true;
    },
  );
});

test("unit: checked-in Bichon scenario uses verified Computer Use clicks without credentials", async () => {
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  assert.equal(
    loaded.scenario.phases.native.args.route,
    "400,370,left,700,step1;400,370,left,1600,step2;400,370,left,2500,step3;400,370,left,3400,step4",
  );
  assert.equal(loaded.scenario.phases.native.args.frameCaptureMode, "computerUse");
  assert.equal(loaded.scenario.phases.web.args.initialRendererReadyTimeoutMs, 90_000);
  assert.equal(loaded.scenario.phases.web.args.finalRendererReadyTimeoutMs, 60_000);
  assert.equal("account" in loaded.scenario.phases.web.args, false);
  assert.equal("password" in loaded.scenario.phases.web.args, false);
});

test("e2e dry-run: CLI writes one redacted manifest and launches no capture apps", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-temporal-pack-e2e-"));
  const outputDir = path.join(tempRoot, "pack");
  try {
    const result = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--scenario", SCENARIO_PATH, "--dryRun", "true", "--output", outputDir],
      { encoding: "utf8", env: { ...process.env, MIR2_QA_PASSWORD: "e2e-private-password" } },
    );
    assert.equal(result.status, 0, result.stderr || result.stdout);
    const summary = JSON.parse(result.stdout);
    assert.equal(summary.ok, true);
    assert.equal(summary.dryRun, true);
    assert.deepEqual(summary.phases, { native: "planned", web: "planned", report: "planned" });

    const entries = (await fs.readdir(outputDir)).sort();
    assert.deepEqual(entries, ["manifest.json"]);
    const manifestText = await fs.readFile(path.join(outputDir, "manifest.json"), "utf8");
    const manifest = JSON.parse(manifestText);
    assert.equal(manifest.status, "dry-run");
    assert.equal(manifest.generatedAt, null);
    assert.equal(manifestText.includes("e2e-private-password"), false);
    assert.match(manifest.phases.native.command.argv[0], /capture-original-computer-use\.mjs$/);
    assert.match(manifest.phases.web.command.argv[0], /capture-web-movement-jitter\.mjs$/);
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});

test("e2e dry-run: invalid CLI input creates no output directory", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-temporal-pack-invalid-"));
  const outputDir = path.join(tempRoot, "must-not-exist");
  try {
    const result = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--scenario", SCENARIO_PATH, "--dryRun", "true", "--output", outputDir, "--unknown", "1"],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Unknown argument --unknown/);
    await assert.rejects(fs.access(outputDir));
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});

test("e2e dry-run: report-only mode rejects missing prerequisite artifacts before writing", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-temporal-pack-missing-inputs-"));
  const outputDir = path.join(tempRoot, "must-not-exist");
  try {
    const result = spawnSync(
      process.execPath,
      [
        SCRIPT_PATH,
        "--scenario",
        SCENARIO_PATH,
        "--dryRun",
        "true",
        "--output",
        outputDir,
        "--phases",
        "report",
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /report phase --original input does not exist/);
    await assert.rejects(fs.access(outputDir));
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});

test("e2e: trusted desktop runner can execute the native phase in-process", async () => {
  const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-temporal-pack-native-runner-"));
  const outputDir = path.join(tempRoot, "pack");
  const loaded = await loadTemporalPackScenario(SCENARIO_PATH);
  let receivedOptions = null;
  try {
    const summary = await runTemporalPack({
      scenario: loaded.scenario,
      scenarioPath: loaded.path,
      outputDir,
      phaseOverrides: { native: true, web: false, report: false },
      nativeCapture: async (options) => {
        receivedOptions = options;
        const jsonPath = path.join(options.outputDir, `${options.prefix}.json`);
        await fs.writeFile(jsonPath, `${JSON.stringify({ ok: true })}\n`, "utf8");
        return { ok: true, jsonPath };
      },
    });
    assert.equal(summary.ok, true);
    assert.equal(summary.phases.native, "passed");
    assert.equal(receivedOptions.prefix, "bichon-332275-left4-native");
    assert.equal(receivedOptions.outputDir, outputDir);
    assert.match(receivedOptions.route, /step4$/);
  } finally {
    await fs.rm(tempRoot, { recursive: true, force: true });
  }
});

function argValue(argv, key) {
  const index = argv.indexOf(`--${key}`);
  assert.notEqual(index, -1, `missing --${key}`);
  return argv[index + 1];
}
