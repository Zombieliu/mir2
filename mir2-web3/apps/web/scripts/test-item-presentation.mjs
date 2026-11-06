import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const compiled = ts.transpileModule(readFileSync(new URL("../lib/world-model/item-presentation.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);
const { projectItemPresentation } = module.exports;

test("a same-instance tooltip refresh invalidates the movement-only comparison", () => {
  const old = { tooltipSource: { info: { item_index: 221, stats: [{ stat: 5, value: 4 }] }, userItem: { unique_id: 0 } } };
  const changed = { tooltipSource: { info: { item_index: 221, stats: [{ stat: 5, value: 6 }] }, userItem: { unique_id: 0 } } };
  assert.notEqual(JSON.stringify(projectItemPresentation(old)), JSON.stringify(projectItemPresentation(changed)));
  assert.equal(JSON.stringify(projectItemPresentation(changed)), JSON.stringify(projectItemPresentation(structuredClone(changed))));
});

test("lossless Crystal metadata does not promote nested identity into command authority", () => {
  const source = { info: { image: 0, name: "WoodenSword" }, userItem: { unique_id: 123, slots: [null] } };
  const projected = projectItemPresentation({ tooltipSource: source, stateImage: 0, equipSlot: "weapon", grade: "common", addedAttack: -1 });
  assert.strictEqual(projected.tooltipSource, source);
  assert.equal(projected.stateImage, 0);
  assert.equal(projected.addedAttack, -1);
  assert.equal(Object.hasOwn(projected, "authoritativeUniqueId"), false);
  assert.equal(Object.hasOwn(projected, "uniqueId"), false);
});

test("an older snapshot without source metadata clears previous presentation", () => {
  const prior = projectItemPresentation({ tooltipSource: { info: { image: 30 } }, stateImage: 30 });
  const current = projectItemPresentation({ tooltipSource: null, stateImage: null });
  assert.notEqual(JSON.stringify(prior), JSON.stringify(current));
  assert.equal(current.tooltipSource, undefined);
  assert.equal(current.stateImage, undefined);
  assert.deepEqual(current, projectItemPresentation({}));
});
