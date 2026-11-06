import assert from "node:assert/strict";
import test from "node:test";
import { verifyStorageRuntimeExports } from "./verify-storage-runtime-exports.mjs";

// Public Rust js_name contract; fixtures contain no renderer, Core, files or startup.
const names = [
  "getMir2StorageUiCapabilities", "setMir2StorageUiSnapshot", "getMir2StorageUiStatus",
  "setMir2StorageUiIntentSink", "clearMir2StorageUiIntentSink", "withdrawMir2StorageUiSnapshot",
  "setMir2StorageUiPointerEdge",
];
const wrapper = name => "export function " + name + "(value) { return wasm." + name + "(value); }";
const javascript = (selected = names) => selected.map(wrapper).join("\n");
function uint(value) {
  const bytes = [];
  do { const next = value & 127; value >>>= 7; bytes.push(next | (value ? 128 : 0)); } while (value);
  return bytes;
}
const vector = rows => [...uint(rows.length), ...rows.flat()];
const string = text => { const bytes = [...new TextEncoder().encode(text)]; return [...uint(bytes.length), ...bytes]; };
const section = (id, bytes) => [id, ...uint(bytes.length), ...bytes];
function wasm(selected = names, { memoryExport = null, startTrap = false } = {}) {
  const exportRows = selected.map(name => [...string(name), name === memoryExport ? 2 : 0, 0]);
  const body = startTrap ? [0, 0, 11] : [0, 11]; // zero locals; unreachable/end or end
  return Uint8Array.from([
    0, 97, 115, 109, 1, 0, 0, 0,
    ...section(1, [1, 96, 0, 0]), // one () -> () function type
    ...section(3, [1, 0]),       // one defined function
    ...(memoryExport ? section(5, [1, 0, 1]) : []), // optional one-page memory
    ...section(7, vector(exportRows)),
    ...(startTrap ? section(8, [0]) : []),
    ...section(10, [1, ...uint(body.length), ...body]),
  ]);
}
const verify = (jsText = javascript(), wasmBytes = wasm(), label = "memory-renderer") =>
  verifyStorageRuntimeExports({ jsText, wasmBytes, label });

test("seven direct generated functions and seven WASM function exports satisfy the independent Storage contract", () => {
  const result = verify();
  assert.deepEqual(result.exports, names); assert.equal(result.storageExportCount, 7);
  assert.equal(result.javascriptCheckedStatically, true); assert.equal(result.wasmCheckedAsModule, true);
  assert.equal(result.rendererInstantiated, false); assert.equal(result.capabilityValuesEvaluated, false);
});

test("every missing JS Storage export fails even when its name appears in comments and strings", () => {
  for (const absent of names) {
    const js = javascript(names.filter(name => name !== absent))
      + "\n// " + wrapper(absent) + "\nconst text = " + JSON.stringify(wrapper(absent)) + ";";
    assert.throws(() => verify(js), /expected exactly one JS export/);
  }
});

test("a same-name local function or nested exported-looking text cannot stand in for a real named export", () => {
  const name = names[0], rest = javascript(names.slice(1));
  assert.throws(() => verify(rest + "\nfunction " + name + "() { return wasm." + name + "(); }"), /JS export/);
  assert.throws(() => verify(rest + "\nfunction local() { function " + name + "() {} }"), /JS export/);
});

test("variables, classes, aliases and re-exports cannot replace the direct bindgen function", () => {
  const name = names[0], rest = javascript(names.slice(1));
  for (const substitute of [
    "export const " + name + " = 1;",
    "export const " + name + " = () => wasm." + name + "();",
    "export class " + name + " {}",
    "function other() {} export { other as " + name + " };",
    "export { " + name + " } from './must-not-be-imported.mjs';",
    "export default function " + name + "() { return wasm." + name + "(); }",
  ]) assert.throws(() => verify(rest + "\n" + substitute), /JS binding|JS export|direct synchronous/);
});

test("duplicate direct or secondary named JS exports are rejected", () => {
  for (const duplicate of [wrapper(names[0]), "export { " + names[0] + " };"])
    assert.throws(() => verify(javascript() + "\n" + duplicate), /syntax errors|exactly one JS export/);
  assert.throws(() => verify(javascript() + "\nfunction " + names[0] + "() {}"), /syntax errors|exactly one JS binding/);
  assert.throws(() => verify(javascript() + "\nimport { other as " + names[0] + " } from './never-load.mjs';"), /syntax errors|exactly one JS binding/);
});

test("wrapper calls must bind each public Storage name to its matching WASM export", () => {
  for (const name of names) {
    const rest = javascript(names.filter(n => n !== name));
    for (const body of ["return wasm.wrongName();", "return true;",
      "wasm." + name + "(); return wasm." + name + "();"])
      assert.throws(() => verify(rest + "\nexport function " + name + "() { " + body + " }"), /wrapper must call/);
  }
});

test("async and generator substitutions are not the known synchronous bindgen shape", () => {
  const name = names[0], rest = javascript(names.slice(1));
  assert.throws(() => verify(rest + "\nexport async function " + name + "() { return wasm." + name + "(); }"), /direct synchronous/);
  assert.throws(() => verify(rest + "\nexport function* " + name + "() { yield wasm." + name + "(); }"), /direct synchronous/);
});

test("malformed JS syntax and unresolved wildcard export shapes fail closed", () => {
  assert.throws(() => verify(javascript() + "\nexport function broken("), /syntax errors/);
  assert.throws(() => verify(javascript() + "\nexport * from './never-import.mjs';"), /wildcard re-export/);
});

test("every missing WASM Storage export fails even with all seven JS wrappers present", () => {
  for (const absent of names)
    assert.throws(() => verify(javascript(), wasm(names.filter(name => name !== absent))), /expected exactly one WASM export/);
});

test("a WASM memory with a required name cannot masquerade as a function export", () => {
  for (const name of names)
    assert.throws(() => verify(javascript(), wasm(names, { memoryExport: name })), /WASM export must be a function/);
});

test("duplicate WASM export names and invalid binary input are rejected without instantiation", () => {
  assert.throws(() => verify(javascript(), wasm([...names, names[0]])), /cannot compile WASM metadata/);
  assert.throws(() => verify(javascript(), Uint8Array.of(0, 1, 2)), /cannot compile WASM metadata/);
});

test("valid JS imports/top-level effects and a trapping WASM start are parsed but never executed", () => {
  const before = globalThis.__storageRuntimeExportFixtureExecuted;
  const js = "import './nonexistent-do-not-load.mjs';\n"
    + "globalThis.__storageRuntimeExportFixtureExecuted = true;\nthrow Error('must not evaluate generated JS');\n"
    + javascript() + "\nexport function bootMir2Runtime() { throw Error('must not boot'); }";
  const result = verify(js, wasm(names, { startTrap: true }));
  assert.equal(globalThis.__storageRuntimeExportFixtureExecuted, before);
  assert.equal(result.rendererInstantiated, false);
});

test("legacy WebGL2 fallback is accepted without evaluating or requiring compiled/startup capability values", () => {
  const js = "const legacyCapability = {schemaVersion:1, storageUiAbiVersion:0, storageIntentAbiVersion:0, compiled:false, startup:false};\n"
    + javascript();
  for (const label of ["webgpu", "webgl2", "webgl2-shared"]) {
    const result = verify(js, wasm(), label);
    assert.equal(result.label, label); assert.equal(result.capabilityValuesEvaluated, false);
    assert.equal(result.storageExportCount, 7);
  }
});

test("capability wrapper supports the real bindgen try/finally string-return shape", () => {
  const name = names[0];
  const js = javascript(names.slice(1)) + "\nexport function " + name + "() {\n"
    + " let deferred0, deferred1; try { const ret = wasm." + name + "(); deferred0=ret[0]; deferred1=ret[1];"
    + " return getStringFromWasm0(ret[0],ret[1]); } finally { wasm.__wbindgen_free(deferred0,deferred1,1); } }";
  assert.equal(verify(js).storageExportCount, 7);
});


test("a dead nested function/class body cannot supply the outer wrapper's WASM call", () => {
  const name = names[0], rest = javascript(names.slice(1));
  for (const inner of [
    "function hidden() { return wasm." + name + "(); }",
    "const hidden = () => wasm." + name + "();",
    "const hidden = { method() { return wasm." + name + "(); } };",
    "const hidden = { get value() { return wasm." + name + "(); } };",
    "class Hidden { method() { return wasm." + name + "(); } }",
  ]) assert.throws(() => verify(rest + "\nexport function " + name + "() { " + inner + " return false; }"), /wrapper must call/);
});

test("wrapper parameters and local bindings cannot shadow the real wasm module", () => {
  const name = names[0], rest = javascript(names.slice(1));
  for (const [parameters, declarations] of [
    ["wasm", ""], ["{ wasm }", ""], ["[wasm]", ""],
    ["", "const wasm = {};"], ["", "let { source: wasm } = {};"],
    ["", "try {} catch (wasm) { return wasm." + name + "(); }"],
    ["", "function wasm() {}"], ["", "class wasm {}"],
  ]) assert.throws(() => verify(rest + "\nexport function " + name + "(" + parameters + ") { "
    + declarations + " return wasm." + name + "(); }"), /wrapper must not shadow wasm/);
});


import { NPC_SHOP_RUNTIME_EXPORTS, verifyNpcShopRuntimeExports,
  verifyNpcShopRuntimeExportFiles } from "./verify-storage-runtime-exports.mjs";
import { mkdtempSync, writeFileSync, unlinkSync, rmdirSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const npcNames = [
  "getMir2NpcShopUiCapabilities", "setMir2NpcShopUiSnapshot", "getMir2NpcShopUiStatus",
  "setMir2NpcShopUiIntentSink", "clearMir2NpcShopUiIntentSink", "withdrawMir2NpcShopUiSnapshot",
  "setMir2NpcShopUiPointerEdge",
];
const npcVerify = (jsText = javascript(npcNames), wasmBytes = wasm(npcNames), label = "memory-npc-shop") =>
  verifyNpcShopRuntimeExports({ jsText, wasmBytes, label });

test("NPC Shop seven fixed direct wrappers and function exports form an independent static contract", () => {
  assert.equal(Object.isFrozen(NPC_SHOP_RUNTIME_EXPORTS), true);
  assert.deepEqual(NPC_SHOP_RUNTIME_EXPORTS, npcNames);
  const result = npcVerify();
  assert.deepEqual(result, { label: "memory-npc-shop", npcShopExportCount: 7, exports: npcNames,
    javascriptCheckedStatically: true, wasmCheckedAsModule: true, rendererInstantiated: false,
    capabilityValuesEvaluated: false });
  assert.throws(() => npcVerify(javascript(), wasm()), /expected exactly one JS export/);
  assert.throws(() => verify(javascript(npcNames), wasm(npcNames)), /expected exactly one JS export/);
  assert.throws(() => verifyNpcShopRuntimeExports({ jsText: "", wasmBytes: wasm([]), requiredNames: [] }),
    /expected exactly one JS export/);
  const capability = npcNames[0];
  const bindgenStringWrapper = javascript(npcNames.slice(1)) + "\nexport function " + capability + "() {"
    + "let a,b; try { const ret=wasm." + capability + "(); a=ret[0]; b=ret[1]; return getStringFromWasm0(a,b); }"
    + "finally { wasm.__wbindgen_free(a,b,1); } }";
  assert.equal(npcVerify(bindgenStringWrapper).npcShopExportCount, 7);
});

test("every missing NPC Shop JS export is rejected despite comments, strings and planner-name substitution", () => {
  for (const absent of npcNames) {
    const js = javascript(npcNames.filter(name => name !== absent))
      + "\n// " + wrapper(absent) + "\nconst text = " + JSON.stringify(wrapper(absent)) + ";"
      + "\n" + wrapper("getMir2NpcGoldBuyPlan");
    assert.throws(() => npcVerify(js), /expected exactly one JS export/);
  }
});

test("NPC Shop pretend, duplicate and asynchronous exports cannot replace a direct bindgen wrapper", () => {
  const name = npcNames[0], rest = javascript(npcNames.slice(1));
  for (const substitute of [
    "function " + name + "() { return wasm." + name + "(); }",
    "export const " + name + " = () => wasm." + name + "();",
    "export class " + name + " {}",
    "function other() {} export { other as " + name + " };",
    "export { " + name + " } from './must-not-load.mjs';",
    "export default function " + name + "() { return wasm." + name + "(); }",
    "export async function " + name + "() { return wasm." + name + "(); }",
    "export function* " + name + "() { yield wasm." + name + "(); }",
  ]) assert.throws(() => npcVerify(rest + "\n" + substitute), /JS binding|JS export|direct synchronous/);
  for (const duplicate of [wrapper(name), "export { " + name + " };"])
    assert.throws(() => npcVerify(javascript(npcNames) + "\n" + duplicate), /syntax errors|exactly one JS export/);
  assert.throws(() => npcVerify(javascript(npcNames) + "\nexport function broken("), /syntax errors/);
});

test("each NPC Shop wrapper must make exactly one call to its own WASM export", () => {
  for (const name of npcNames) {
    const rest = javascript(npcNames.filter(n => n !== name));
    for (const body of ["return wasm.wrongName();", "return true;",
      "wasm." + name + "(); return wasm." + name + "();"])
      assert.throws(() => npcVerify(rest + "\nexport function " + name + "() { " + body + " }"), /wrapper must call/);
  }
});

test("NPC Shop parameters, destructuring, catch and local declarations cannot shadow wasm", () => {
  const name = npcNames[0], rest = javascript(npcNames.slice(1));
  for (const [parameters, declarations] of [
    ["wasm", ""], ["{ wasm }", ""], ["[wasm]", ""],
    ["", "const wasm = {};"], ["", "let { source: wasm } = {};"],
    ["", "try {} catch (wasm) { return wasm." + name + "(); }"],
    ["", "function wasm() {}"], ["", "class wasm {}"],
  ]) assert.throws(() => npcVerify(rest + "\nexport function " + name + "(" + parameters + ") { "
    + declarations + " return wasm." + name + "(); }"), /wrapper must not shadow wasm/);
});

test("NPC Shop calls inside deferred function and class bodies do not prove the outer wrapper", () => {
  const name = npcNames[0], rest = javascript(npcNames.slice(1));
  for (const inner of [
    "function hidden() { return wasm." + name + "(); }",
    "const hidden = () => wasm." + name + "();",
    "const hidden = { method() { return wasm." + name + "(); } };",
    "const hidden = { get value() { return wasm." + name + "(); } };",
    "class Hidden { method() { return wasm." + name + "(); } }",
  ]) assert.throws(() => npcVerify(rest + "\nexport function " + name + "() { " + inner + " return false; }"), /wrapper must call/);
});

test("every missing NPC Shop WASM export is rejected even with complete JavaScript wrappers", () => {
  for (const absent of npcNames)
    assert.throws(() => npcVerify(javascript(npcNames), wasm(npcNames.filter(name => name !== absent))),
      /expected exactly one WASM export/);
});

test("NPC Shop nonfunction, duplicate and malformed WASM exports fail closed without instantiation", () => {
  for (const name of npcNames)
    assert.throws(() => npcVerify(javascript(npcNames), wasm(npcNames, { memoryExport: name })), /WASM export must be a function/);
  assert.throws(() => npcVerify(javascript(npcNames), wasm([...npcNames, npcNames[0]])), /cannot compile WASM metadata/);
  assert.throws(() => npcVerify(javascript(npcNames), Uint8Array.of(0, 1, 2)), /cannot compile WASM metadata/);
});

test("NPC Shop generated imports, top-level effects, disabled capabilities and a trapping start remain unevaluated", () => {
  const before = globalThis.__npcShopRuntimeExportFixtureExecuted;
  const js = "import './must-not-load.mjs';\n"
    + "globalThis.__npcShopRuntimeExportFixtureExecuted = true;\nthrow Error('must not evaluate generated JS');\n"
    + "const capability = {schemaVersion:1,npcShopUiAbiVersion:0,npcShopIntentAbiVersion:0,compiled:false,startup:false};\n"
    + javascript(npcNames);
  for (const label of ["webgpu", "webgl2", "webgl2-shared"]) {
    const result = npcVerify(js, wasm(npcNames, { startTrap: true }), label);
    assert.equal(result.label, label); assert.equal(result.npcShopExportCount, 7);
    assert.equal(result.rendererInstantiated, false); assert.equal(result.capabilityValuesEvaluated, false);
    assert.equal(globalThis.__npcShopRuntimeExportFixtureExecuted, before);
  }
});

test("NPC Shop file verifier accepts valid UTF8 bytes and refuses malformed UTF8 before wrapper acceptance", () => {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-npc-export-test-"));
  const jsPath = path.join(root, "generated.js"), wasmPath = path.join(root, "generated_bg.wasm");
  // Direct owned leaves only; no recursive deletion or generated code import.
  let jsCreated = false, wasmCreated = false;
  try {
    const jsBytes = Buffer.from("// 商店\n" + javascript(npcNames), "utf8"), wasmBytes = wasm(npcNames);
    writeFileSync(jsPath, jsBytes, { flag: "wx" }); jsCreated = true;
    writeFileSync(wasmPath, wasmBytes, { flag: "wx" }); wasmCreated = true;
    const result = verifyNpcShopRuntimeExportFiles({ jsPath, wasmPath, label: "owned-npc-fixture" });
    assert.equal(result.npcShopExportCount, 7); assert.deepEqual(result.exports, npcNames);
    assert.equal(result.jsPath, path.resolve(jsPath)); assert.equal(result.wasmPath, path.resolve(wasmPath));
    assert.equal(result.jsBytes, jsBytes.length); assert.equal(result.wasmBytes, wasmBytes.length);
    assert.equal(result.javascriptCheckedStatically, true); assert.equal(result.rendererInstantiated, false);
    writeFileSync(jsPath, Buffer.from([0xc3, 0x28]));
    assert.throws(() => verifyNpcShopRuntimeExportFiles({ jsPath, wasmPath }),
      error => error instanceof TypeError && error.code === "ERR_ENCODING_INVALID_ENCODED_DATA");
  } finally {
    if (jsCreated) unlinkSync(jsPath);
    if (wasmCreated) unlinkSync(wasmPath);
    rmdirSync(root);
  }
});
