import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const prewarmPolicyPath = fileURLToPath(
  new URL("../lib/asset-prewarm-policy.ts", import.meta.url),
);
const originalUiPath = fileURLToPath(
  new URL("../lib/original-ui.ts", import.meta.url),
);
const assetCachePacksPath = fileURLToPath(
  new URL("../lib/asset-cache-packs.ts", import.meta.url),
);
const assetCacheRegistrarPath = fileURLToPath(
  new URL("../app/components/asset-cache-registrar.tsx", import.meta.url),
);
const pagePath = fileURLToPath(new URL("../app/page.tsx", import.meta.url));
const originalClientShellPath = fileURLToPath(
  new URL("../app/original-client-shell.tsx", import.meta.url),
);
const sceneVisualLayersPath = fileURLToPath(
  new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
);
const playableSmokePath = fileURLToPath(
  new URL("./smoke-cache-metrics.mjs", import.meta.url),
);

function loadTypeScriptModule(sourcePath, dependencies = {}) {
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
  const requireDependency = (specifier) => {
    if (Object.hasOwn(dependencies, specifier)) return dependencies[specifier];
    throw new Error(`Unexpected dependency ${specifier} from ${sourcePath}`);
  };
  new Function("exports", "module", "require", compiled.outputText)(
    loadedModule.exports,
    loadedModule,
    requireDependency,
  );
  return loadedModule.exports;
}

const { resolveAssetPrewarmPolicy, shouldPrewarmRawMapFrames } =
  loadTypeScriptModule(prewarmPolicyPath);
const originalUiModule = loadTypeScriptModule(originalUiPath);
const { ASSET_CACHE_PACKS, selectAssetCachePacksForStage } =
  loadTypeScriptModule(assetCachePacksPath, {
    "./original-ui": originalUiModule,
  });

test("low tier bounds scene prewarm and disables background packs by default", () => {
  assert.deepEqual(resolveAssetPrewarmPolicy("low"), {
    tier: "low",
    criticalConcurrency: 3,
    backgroundConcurrency: 1,
    maxSceneFrames: 192,
    backgroundMode: "off",
  });
});

test("medium and high tiers preserve progressively more eager prewarming", () => {
  assert.deepEqual(resolveAssetPrewarmPolicy("medium"), {
    tier: "medium",
    criticalConcurrency: 5,
    backgroundConcurrency: 2,
    maxSceneFrames: 480,
    backgroundMode: "afterPlayable",
  });
  assert.deepEqual(resolveAssetPrewarmPolicy("high"), {
    tier: "high",
    criticalConcurrency: 8,
    backgroundConcurrency: 3,
    maxSceneFrames: null,
    backgroundMode: "afterPlayable",
  });
});

test("an explicit background mode remains available for deterministic QA", () => {
  assert.equal(
    resolveAssetPrewarmPolicy("low", "immediate").backgroundMode,
    "immediate",
  );
  assert.equal(resolveAssetPrewarmPolicy("high", "off").backgroundMode, "off");
});

test("initial critical prewarm contains only the lightweight login shell", () => {
  const criticalPacks = ASSET_CACHE_PACKS.filter(
    (pack) => pack.phase !== "background",
  );

  assert.deepEqual(
    criticalPacks.map((pack) => pack.name),
    ["login"],
  );
  assert.equal(criticalPacks[0].stage, "login");
  assert.equal(criticalPacks[0].cacheTier, "critical");
  assert.ok(criticalPacks[0].urls.length > 0);
  assert.equal(
    criticalPacks[0].urls.some((url) => url.includes("/original-ui/ChrSel/")),
    false,
  );
});

test("deferred scene stages expose deterministic follow-up pack selections", () => {
  assert.deepEqual(
    selectAssetCachePacksForStage("login").map((pack) => pack.name),
    ["login"],
  );
  assert.deepEqual(
    selectAssetCachePacksForStage("character-select").map((pack) => pack.name),
    ["character-select", "login-audio"],
  );
  assert.deepEqual(
    selectAssetCachePacksForStage("game").map((pack) => pack.name),
    ["bichon-spawn", "hud-core"],
  );

  const deferredPacks = ASSET_CACHE_PACKS.filter(
    (pack) => pack.stage !== "login",
  );
  assert.ok(deferredPacks.every((pack) => pack.phase === "background"));
  assert.ok(deferredPacks.every((pack) => pack.cacheTier === "background"));
  assert.equal(
    deferredPacks.some((pack) =>
      pack.urls.includes("/generated/map-atlas/manifest.json"),
    ),
    false,
    "production prewarm must not redownload the legacy mutable atlas manifest",
  );
});

test("the UI lifecycle requests each deferred prewarm stage", () => {
  const registrarSource = readFileSync(assetCacheRegistrarPath, "utf8");
  const pageSource = readFileSync(pagePath, "utf8");

  assert.match(registrarSource, /selectAssetCachePacksForStage\(stage,/);
  assert.match(registrarSource, /installedStagePrewarm\?\.\("login"\)/);
  assert.match(registrarSource, /new AssetPrewarmOrchestrator/);
  assert.match(registrarSource, /const includeSceneFrames = lane === "background"/);
  assert.match(registrarSource, /run\.status !== "cancelled"/);
  assert.match(registrarSource, /if \(!manifestResponse\.ok\)/);
  assert.doesNotMatch(registrarSource, /pack\.name !== "login-audio"/);
  assert.match(registrarSource, /void configureServiceWorkerInBackground\(/);
  assert.doesNotMatch(registrarSource, /await registration\.update\(\)/);
  assert.doesNotMatch(registrarSource, /await navigator\.serviceWorker\.ready/);
  assert.match(pageSource, /screen === "select" \? "character-select"/);
  assert.match(pageSource, /screen === "game" \? "game" : "login"/);
  assert.match(pageSource, /__mir2AssetCachePrewarmStage\?\.\(stage\)/);
  assert.match(pageSource, /signalAssetFirstPlayable\(detail\)/);
  assert.match(pageSource, /if \(screen !== "game"\)/);
  assert.match(pageSource, /const shouldBootBevyRuntime = screen === "game" && assetFirstPlayable/);
  assert.match(pageSource, /setAssetFirstPlayable\(true\)/);
});

test("only the low tier prewarms raw map frames for the DOM compatibility path", () => {
  assert.equal(shouldPrewarmRawMapFrames("low"), true);
  assert.equal(shouldPrewarmRawMapFrames("medium"), false);
  assert.equal(shouldPrewarmRawMapFrames("high"), false);
});

test("heavy map and effect metadata loaders stay behind the game screen boundary", () => {
  const shellSource = readFileSync(originalClientShellPath, "utf8");
  const visualLayersSource = readFileSync(sceneVisualLayersPath, "utf8");

  assert.match(
    shellSource,
    /useEffect\(\(\) => \{\s+if \(screen !== "game"\) return;\s+if \(!mapAtlasRequested/,
  );
  assert.match(visualLayersSource, /function useEffectAssets\(enabled: boolean\)/);
  assert.match(visualLayersSource, /if \(!enabled\) return;/);
  assert.match(visualLayersSource, /useEffectAssets\(screen === "game"\)/);
});

test("playable cache acceptance waits for lifecycle background work to settle", () => {
  const smokeSource = readFileSync(playableSmokePath, "utf8");
  assert.match(smokeSource, /window\.__mir2AssetOrchestrator\?\.snapshot\?\.\(\)/);
  assert.match(smokeSource, /!playableMode \|\| status\?\.orchestratorReady/);
  assert.match(smokeSource, /coldAssetOrchestratorSettled/);
  assert.match(smokeSource, /warmAssetOrchestratorSettled/);
  assert.match(smokeSource, /playableMode \? "" : "--disable-gpu"/);
  assert.match(smokeSource, /coldMapRendererAvailable/);
  assert.match(smokeSource, /noLegacyMapAtlasManifest/);
});
