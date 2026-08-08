import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(url) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, () => {
    throw new Error("Unexpected dependency while loading crystal-dialog-markup.ts");
  });
  return module.exports;
}

const { stripCrystalDialogMarkup, parseCrystalColourSpans } = loadTypeScriptModule(
  new URL("../lib/crystal-dialog-markup.ts", import.meta.url),
);

const premiumLine = "You dont have a {Premium Pass/LightSteelBlue}.";
assert.equal(stripCrystalDialogMarkup(premiumLine), premiumLine);
assert.equal(stripCrystalDialogMarkup("Hello <$USERNAME>,  welcome"), "Hello , welcome");
assert.equal(stripCrystalDialogMarkup("Gold: %ARG(0) coins"), "Gold: coins");
assert.deepEqual(parseCrystalColourSpans(premiumLine), [
  { text: "You dont have a " },
  { text: "Premium Pass", colour: "LightSteelBlue" },
  { text: "." },
]);
assert.deepEqual(parseCrystalColourSpans("Just plain text."), [{ text: "Just plain text." }]);
assert.deepEqual(parseCrystalColourSpans("{Yes/Green} or {No/Red}?"), [
  { text: "Yes", colour: "Green" },
  { text: " or " },
  { text: "No", colour: "Red" },
  { text: "?" },
]);
assert.deepEqual(parseCrystalColourSpans("{a/b/c}"), [{ text: "a", colour: "b" }]);

console.log("crystal-dialog-markup: 7 checks passed");
