import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/asset-orchestrator.ts", import.meta.url));
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, []);
const loadedModule = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  loadedModule.exports,
  loadedModule,
  () => ({}),
);

const { AssetPrewarmOrchestrator } = loadedModule.exports;

const hasWork = (stage, lane) =>
  (stage === "login" && lane === "critical") ||
  (stage !== "login" && lane === "background");

test("afterPlayable holds background work and keeps only the latest screen", async () => {
  const runs = [];
  const orchestrator = new AssetPrewarmOrchestrator({
    backgroundMode: "afterPlayable",
    hasWork,
    run: async (stage, lane) => {
      runs.push(`${stage}:${lane}`);
    },
  });

  const select = orchestrator.requestStage("character-select");
  assert.deepEqual(runs, []);
  assert.equal(orchestrator.snapshot().pendingBackgroundStage, "character-select");

  const game = orchestrator.requestStage("game");
  await select;
  assert.deepEqual(runs, []);
  assert.equal(orchestrator.snapshot().pendingBackgroundStage, "game");

  await Promise.all([game, orchestrator.markFirstPlayable()]);
  assert.deepEqual(runs, ["game:background"]);
  assert.equal(orchestrator.snapshot().firstPlayable, true);
  assert.deepEqual(orchestrator.snapshot().completed, ["game:background"]);
});

test("off mode never starts background work", async () => {
  const runs = [];
  const orchestrator = new AssetPrewarmOrchestrator({
    backgroundMode: "off",
    hasWork,
    run: async (stage, lane) => runs.push(`${stage}:${lane}`),
  });

  await orchestrator.requestStage("game");
  await orchestrator.markFirstPlayable();
  assert.deepEqual(runs, []);
});

test("immediate mode aborts stale background work on a newer screen", async () => {
  const runs = [];
  let started;
  const selectStarted = new Promise((resolve) => {
    started = resolve;
  });
  const orchestrator = new AssetPrewarmOrchestrator({
    backgroundMode: "immediate",
    hasWork,
    run: async (stage, lane, signal) => {
      runs.push(`start:${stage}:${lane}`);
      if (stage !== "character-select") {
        runs.push(`done:${stage}:${lane}`);
        return;
      }
      started();
      await new Promise((resolve, reject) => {
        signal.addEventListener(
          "abort",
          () => {
            runs.push(`abort:${stage}:${lane}`);
            reject(new DOMException("aborted", "AbortError"));
          },
          { once: true },
        );
      });
    },
  });

  const select = orchestrator.requestStage("character-select");
  await selectStarted;
  const game = orchestrator.requestStage("game");
  await Promise.all([select, game]);
  assert.deepEqual(runs, [
    "start:character-select:background",
    "abort:character-select:background",
    "start:game:background",
    "done:game:background",
  ]);
});

test("critical work is deduplicated and does not wait for first playable", async () => {
  const runs = [];
  const orchestrator = new AssetPrewarmOrchestrator({
    backgroundMode: "afterPlayable",
    hasWork,
    run: async (stage, lane) => runs.push(`${stage}:${lane}`),
  });

  await Promise.all([
    orchestrator.requestStage("login"),
    orchestrator.requestStage("login"),
  ]);
  assert.deepEqual(runs, ["login:critical"]);
});
