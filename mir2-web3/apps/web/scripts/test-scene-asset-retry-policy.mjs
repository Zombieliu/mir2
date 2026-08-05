import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/scene-asset-retry-policy.ts", import.meta.url));
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

const {
  buildSceneAssetCandidateUrls,
  sceneAssetRetryUrl,
  sceneAssetStalledRetryDecision,
} = loadedModule.exports;

test("scene image candidates are stable, deduplicated, and strip legacy cache busters", () => {
  const candidates = buildSceneAssetCandidateUrls({
    url: "/original-ui/Prguse/2090.png?skin=gold&mir2ImgRetry=13&mir2ImgRetryTs=old",
    pageUrl: "https://mir2.obelisk.build/game",
    remoteAssetBaseUrls: [
      "https://public-r2.example.test/release-1/",
      "https://public-r2.example.test/release-1",
      "https://assets.example.test/release-1",
    ],
    isRemoteBackedPath: (path) => path.startsWith("/original-ui/"),
  });

  assert.deepEqual(candidates, [
    "/original-ui/Prguse/2090.png?skin=gold",
    "https://public-r2.example.test/release-1/original-ui/Prguse/2090.png?skin=gold",
    "https://assets.example.test/release-1/original-ui/Prguse/2090.png?skin=gold",
  ]);
  assert.ok(candidates.every((candidate) => !candidate.includes("mir2ImgRetry")));
});

test("retry selection rotates through a finite stable candidate set", () => {
  const candidates = ["/primary.png", "https://fallback.test/primary.png"];
  assert.equal(sceneAssetRetryUrl(candidates, 1), candidates[0]);
  assert.equal(sceneAssetRetryUrl(candidates, 2), candidates[1]);
  assert.equal(sceneAssetRetryUrl(candidates, 13), candidates[0]);
  assert.equal(sceneAssetRetryUrl([], 1), null);
});

test("stalled image rescue waits thirty seconds and is bounded to one rewrite", () => {
  assert.equal(
    sceneAssetStalledRetryDecision({ elapsedMs: 29_999, retryCount: 0 }),
    "wait",
  );
  assert.equal(
    sceneAssetStalledRetryDecision({ elapsedMs: 30_000, retryCount: 0 }),
    "retry",
  );
  assert.equal(
    sceneAssetStalledRetryDecision({ elapsedMs: 30_000, retryCount: 1 }),
    "fail",
  );
  assert.equal(
    sceneAssetStalledRetryDecision({ elapsedMs: 60_000, retryCount: 0, loadState: "retrying" }),
    "skip",
  );
});
