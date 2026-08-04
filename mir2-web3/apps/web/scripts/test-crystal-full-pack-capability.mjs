import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../lib/crystal-full-pack-capability.ts", import.meta.url);
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
new Function("exports", "module", compiled.outputText)(module.exports, module);

const { shouldLoadCrystalFullPack } = module.exports;

assert.equal(shouldLoadCrystalFullPack({}), true);
assert.equal(shouldLoadCrystalFullPack({ remoteAssetBaseUrl: "https://assets.example.test/release" }), false);
assert.equal(
  shouldLoadCrystalFullPack({
    remoteAssetBaseUrl: "https://assets.example.test/release",
    releaseCapability: true,
  }),
  true,
);
assert.equal(
  shouldLoadCrystalFullPack({
    remoteAssetBaseUrl: "https://assets.example.test/release",
    releaseCapability: false,
  }),
  false,
);
assert.equal(
  shouldLoadCrystalFullPack({
    configuredValue: "1",
    remoteAssetBaseUrl: "https://assets.example.test/release",
  }),
  true,
);
assert.equal(shouldLoadCrystalFullPack({ configuredValue: "false" }), false);
assert.equal(
  shouldLoadCrystalFullPack({
    queryValue: "1",
    configuredValue: "0",
    remoteAssetBaseUrl: "https://assets.example.test/release",
  }),
  true,
);
assert.equal(
  shouldLoadCrystalFullPack({
    queryValue: "0",
    configuredValue: "1",
  }),
  false,
);

console.log("crystal full-pack capability tests passed");
