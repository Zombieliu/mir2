import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import ts from "typescript";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const sourcePath = path.resolve(
  scriptDir,
  "..",
  "..",
  "..",
  "infra",
  "cloudflare",
  "mir2-domain-proxy",
  "src",
  "index.ts",
);
const source = await fs.readFile(sourcePath, "utf8");
const transpiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.ESNext,
    target: ts.ScriptTarget.ES2022,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (transpiled.diagnostics ?? []).filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
assert.deepEqual(errors, [], "domain proxy TypeScript transpiles without syntax errors");

const temporaryModule = path.join(scriptDir, `.domain-proxy-routing-${process.pid}-${Date.now()}.mjs`);
try {
  await fs.writeFile(temporaryModule, transpiled.outputText, "utf8");
  const worker = await import(`${pathToFileURL(temporaryModule).href}?v=${Date.now()}`);
  for (const assetPath of [
    "/original-ui/Title/30.png",
    "/original-map/WemadeMir2/Tiles/1504.png",
    "/generated/original-map-blend/WemadeMir2/Objects/1.png",
    "/generated/map-atlas/manifest.json",
    "/generated/crystal-packs/full/index.json",
    "/generated/crystal-packs/full/libraries/entities/example.json",
    "/generated/crystal-packs/full/pages/aa/example.png",
  ]) {
    assert.equal(worker.isStaticAssetRequest(new URL(`https://mir2.example${assetPath}`)), true, assetPath);
  }
  for (const applicationPath of ["/", "/api/asset-manifest", "/ws", "/generated/not-an-asset/file.json"]) {
    assert.equal(worker.isStaticAssetRequest(new URL(`https://mir2.example${applicationPath}`)), false, applicationPath);
  }
  console.log("domain proxy full-pack routing passed");
} finally {
  await fs.rm(temporaryModule, { force: true });
}
