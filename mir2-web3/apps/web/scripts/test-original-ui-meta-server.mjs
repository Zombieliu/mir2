#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../lib/original-ui-meta-server.ts", import.meta.url);
const source = readFileSync(sourcePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourcePath),
});
const module = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  module.exports,
  module,
  (request) => {
    if (request === "server-only") return {};
    throw new Error(`Unexpected test require ${request}`);
  },
);

const { readStaticOriginalUiLibraryMeta } = module.exports;
const originalFetch = globalThis.fetch;
const originalPublicBase = process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
const originalServerBase = process.env.MIR2_ASSET_BASE_URL;

function meta(count, frameCount) {
  return {
    version: 1,
    count,
    frames: Array.from({ length: frameCount }, (_, index) => ({ index })),
  };
}

try {
  process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = "https://assets.example.test";
  delete process.env.MIR2_ASSET_BASE_URL;

  const completeRequests = [];
  globalThis.fetch = async (url, init) => {
    completeRequests.push({ url: String(url), cache: init?.cache });
    return Response.json(meta(2, 2));
  };
  const complete = await readStaticOriginalUiLibraryMeta(
    new Request("https://game.example.test/api/original-ui-meta"),
    "Monster/003",
  );
  assert.equal(complete.frames.length, 2);
  assert.deepEqual(completeRequests, [{
    url: "https://game.example.test/original-ui/Monster/003/meta.json",
    cache: "no-store",
  }]);

  const fallbackRequests = [];
  globalThis.fetch = async (url, init) => {
    fallbackRequests.push({ url: String(url), cache: init?.cache });
    return Response.json(fallbackRequests.length === 1 ? meta(2, 1) : meta(2, 2));
  };
  const fallback = await readStaticOriginalUiLibraryMeta(
    new Request("https://game.example.test/api/original-ui-meta"),
    "Monster/010",
  );
  assert.equal(fallback.frames.length, 2);
  assert.deepEqual(fallbackRequests, [
    {
      url: "https://game.example.test/original-ui/Monster/010/meta.json",
      cache: "no-store",
    },
    {
      url: "https://assets.example.test/original-ui/Monster/010/meta.json",
      cache: "force-cache",
    },
  ]);
} finally {
  globalThis.fetch = originalFetch;
  if (originalPublicBase === undefined) delete process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  else process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = originalPublicBase;
  if (originalServerBase === undefined) delete process.env.MIR2_ASSET_BASE_URL;
  else process.env.MIR2_ASSET_BASE_URL = originalServerBase;
}

console.log("original UI metadata server cache tests passed");
