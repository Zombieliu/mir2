#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const originalAssetBase = process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = "https://assets.example.test/mir2/v/test";

try {
  const sourcePath = new URL("../lib/asset-url.ts", import.meta.url);
  const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
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
      throw new Error(`Unexpected test require ${request}`);
    },
  );
  const { originalAssetPath } = module.exports;

  for (const root of ["Monster", "NPC", "CArmour", "CHair", "CWeapon", "AArmour", "ARWeapon"]) {
    assert.equal(
      originalAssetPath(`/original-ui/${root}/003/153.png`),
      `/original-ui/${root}/003/153.png`,
      `${root} must prefer the actor library retained on the app origin`,
    );
  }
  assert.equal(
    originalAssetPath("/original-ui/Items/12.png"),
    "/api/remote-asset/original-ui/Items/12.png",
    "pruned non-actor UI assets must keep using the configured R2 proxy",
  );
  assert.equal(
    originalAssetPath("/api/remote-asset/original-ui/Monster/005/153.png"),
    "/api/remote-asset/original-ui/Monster/005/153.png",
  );
  assert.equal(originalAssetPath("https://cdn.example.test/frame.png"), "https://cdn.example.test/frame.png");
} finally {
  if (originalAssetBase === undefined) delete process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  else process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = originalAssetBase;
}

console.log("asset URL routing tests passed");
