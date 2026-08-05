import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/scene-asset-preloader.ts", import.meta.url));
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
new Function("exports", "module", compiled.outputText)(loadedModule.exports, loadedModule);

const { preloadSceneAssetUrls } = loadedModule.exports;

test("scene preload caps concurrency and stops queued work after interaction readiness", async () => {
  let active = 0;
  let maxActive = 0;
  let started = 0;
  const result = await preloadSceneAssetUrls(
    Array.from({ length: 100 }, (_, index) => `/tile-${index}.png`),
    2_000,
    {
      allowPartialReady: true,
      minLoaded: 6,
      concurrency: 4,
      loadCandidate: async () => {
        started += 1;
        active += 1;
        maxActive = Math.max(maxActive, active);
        await new Promise((resolve) => setTimeout(resolve, 2));
        active -= 1;
        return true;
      },
    },
  );

  assert.equal(result.ready, true);
  assert.equal(result.loaded, 6);
  assert.equal(maxActive, 4);
  assert.ok(started <= 9, `only a small bounded head should start, got ${started}`);
  assert.ok(result.pending >= 90, "the speculative tail must remain unstarted");
});

test("scene preload deduplicates URLs and uses candidate fallbacks", async () => {
  const attempted = [];
  const result = await preloadSceneAssetUrls(["/a.png", "/a.png"], 1_000, {
    minLoaded: 1,
    concurrency: 1,
    resolveCandidates: () => ["/primary.png", "/fallback.png"],
    loadCandidate: async (url) => {
      attempted.push(url);
      return url === "/fallback.png";
    },
  });

  assert.equal(result.total, 1);
  assert.equal(result.loaded, 1);
  assert.equal(result.visualReady, true);
  assert.deepEqual(attempted, ["/primary.png", "/fallback.png"]);
});

test("a timeout with no loaded image cannot masquerade as interaction-ready", async () => {
  const result = await preloadSceneAssetUrls(["/slow.png"], 5, {
    allowPartialReady: true,
    minLoaded: 1,
    loadCandidate: async () => await new Promise(() => undefined),
  });
  assert.equal(result.ready, false);
  assert.equal(result.interactionReady, false);
  assert.equal(result.status, "timeout");
  assert.equal(result.pending, 1);
});
