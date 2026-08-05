import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const args = parseArgs(process.argv.slice(2));
const baseUrl = String(
  args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "https://mir2.obelisk.build",
).replace(/\/+$/, "");
const timeoutMs = positiveInteger(
  args.timeoutMs ?? process.env.MIR2_SCENE_WARM_TIMEOUT_MS,
  180_000,
);
const requestModule = loadTypeScriptModule(
  new URL("../lib/scene-blueprint-request.ts", import.meta.url),
);
const { createCrystalSceneBlueprintRequestKey, createCrystalSceneBlueprintRequestUrl } =
  requestModule;
const targets = [
  { label: "bichon-current-spawn", mapFileName: "0", centerX: 344, centerY: 74, width: 56, height: 68 },
  { label: "bichon-legacy-spawn", mapFileName: "0", centerX: 330, centerY: 270, width: 56, height: 68 },
  { label: "bichon-critical-route", mapFileName: "0", centerX: 307, centerY: 232, width: 56, height: 68 },
];

const results = [];
for (const target of targets) {
  const relativeUrl = createCrystalSceneBlueprintRequestUrl(target);
  const url = new URL(relativeUrl, baseUrl);
  const startedAt = performance.now();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort("scene-warm-timeout"), timeoutMs);
  try {
    const response = await fetch(url, {
      signal: controller.signal,
      headers: { accept: "application/json" },
    });
    const body = await response.json();
    const cells = body?.originalMapRegion?.cells?.length ?? 0;
    if (!response.ok || cells <= 0) {
      throw new Error(
        `${target.label} returned HTTP ${response.status}, cells=${cells}, error=${body?.error ?? "none"}`,
      );
    }
    results.push({
      label: target.label,
      key: createCrystalSceneBlueprintRequestKey(target),
      status: response.status,
      elapsedMs: Math.round(performance.now() - startedAt),
      sceneCache: response.headers.get("x-mir2-scene-cache"),
      vercelCache: response.headers.get("x-vercel-cache"),
      age: response.headers.get("age"),
      cells,
      sprites: Object.keys(body?.originalMapRegion?.sprites ?? {}).length,
    });
  } finally {
    clearTimeout(timer);
  }
}

console.log(JSON.stringify({ ok: true, baseUrl, results }, null, 2));

function loadTypeScriptModule(url) {
  const sourcePath = fileURLToPath(url);
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
  return loadedModule.exports;
}

function positiveInteger(value, fallback) {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const token = values[index];
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    const next = values[index + 1];
    parsed[key] = next && !next.startsWith("--") ? values[++index] : true;
  }
  return parsed;
}
