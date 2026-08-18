import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const originalAssetBaseUrl = process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;

const sourcePath = new URL("../lib/original-scene-sprite-meta.ts", import.meta.url);
const source = readFileSync(sourcePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    esModuleInterop: true,
    module: ts.ModuleKind.CommonJS,
    resolveJsonModule: true,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourcePath),
});

function loadSpriteMetaModule(assetBaseUrl) {
  if (assetBaseUrl === undefined) {
    delete process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  } else {
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = assetBaseUrl;
  }
  const module = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(
    module.exports,
    module,
    (request) => {
      if (request.endsWith("manifest.generated.json")) {
        return {
          libraries: {
            "Title/000": {},
          },
        };
      }
      if (request.endsWith("source-libraries.generated.json")) {
        return {
          libraries: {
            "Monster/000": {},
            "Monster/139": {},
            "Map/2": {},
          },
        };
      }
      throw new Error(`Unexpected test require ${request}`);
    },
  );
  return module.exports;
}

const localOnlyModule = loadSpriteMetaModule(undefined);
assert.equal(localOnlyModule.originalSceneSpriteLibraryExists("Monster/000"), true);
assert.equal(localOnlyModule.originalSceneSpriteLibraryExists("Monster\\139"), true);
assert.equal(localOnlyModule.originalSceneSpriteLibraryExists("Map/2"), false);

const { fetchOriginalSceneSpriteMeta } = loadSpriteMetaModule("https://assets.example.test");
const originalFetch = globalThis.fetch;

try {
  const bundledRequests = [];
  globalThis.fetch = async (url) => {
    bundledRequests.push(String(url));
    return new Response("{}", { status: 200 });
  };
  const bundledResponse = await fetchOriginalSceneSpriteMeta("Monster/000");
  assert.equal(bundledResponse.status, 200);
  assert.deepEqual(bundledRequests, ["/original-ui/Monster/000/meta.json"]);

  const fallbackRequests = [];
  globalThis.fetch = async (url) => {
    fallbackRequests.push(String(url));
    return new Response("{}", { status: fallbackRequests.length === 1 ? 404 : 200 });
  };
  const fallbackResponse = await fetchOriginalSceneSpriteMeta("Monster/139");
  assert.equal(fallbackResponse.status, 200);
  assert.deepEqual(fallbackRequests, [
    "/original-ui/Monster/139/meta.json",
    "/api/original-ui-meta?library=Monster%2F139",
  ]);

  const truncatedRequests = [];
  globalThis.fetch = async (url) => {
    truncatedRequests.push(String(url));
    return truncatedRequests.length === 1
      ? Response.json({ count: 2, frames: [{ index: 0 }] })
      : Response.json({ count: 2, frames: [{ index: 0 }, { index: 1 }] });
  };
  const truncatedResponse = await fetchOriginalSceneSpriteMeta("Monster/139");
  assert.equal(truncatedResponse.status, 200);
  assert.deepEqual(truncatedRequests, [
    "/original-ui/Monster/139/meta.json",
    "/api/original-ui-meta?library=Monster%2F139",
  ]);

  const exportedOnlyRequests = [];
  globalThis.fetch = async (url) => {
    exportedOnlyRequests.push(String(url));
    return new Response("{}", { status: 404 });
  };
  const exportedOnlyResponse = await fetchOriginalSceneSpriteMeta("Title/000");
  assert.equal(exportedOnlyResponse.status, 404);
  assert.deepEqual(exportedOnlyRequests, ["/original-ui/Title/000/meta.json"]);
} finally {
  globalThis.fetch = originalFetch;
  if (originalAssetBaseUrl === undefined) {
    delete process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  } else {
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = originalAssetBaseUrl;
  }
}

console.log("original scene sprite metadata tests passed");
