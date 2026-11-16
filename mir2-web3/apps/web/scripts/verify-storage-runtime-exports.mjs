#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// Independent Storage and NPC Shop ABIs. Existing release manifest/capability keys stay unchanged.
export const STORAGE_RUNTIME_EXPORTS = Object.freeze([
  "getMir2StorageUiCapabilities",
  "setMir2StorageUiSnapshot",
  "getMir2StorageUiStatus",
  "setMir2StorageUiIntentSink",
  "clearMir2StorageUiIntentSink",
  "withdrawMir2StorageUiSnapshot",
  "setMir2StorageUiPointerEdge",
]);
export const NPC_SHOP_RUNTIME_EXPORTS = Object.freeze([
  "getMir2NpcShopUiCapabilities",
  "setMir2NpcShopUiSnapshot",
  "getMir2NpcShopUiStatus",
  "setMir2NpcShopUiIntentSink",
  "clearMir2NpcShopUiIntentSink",
  "withdrawMir2NpcShopUiSnapshot",
  "setMir2NpcShopUiPointerEdge",
]);
const hasModifier = (node, kind) => node.modifiers?.some(modifier => modifier.kind === kind) === true;

function checkJavascript(text, label, requiredNames) {
  const required = new Set(requiredNames);
  assert.equal(typeof text, "string", "jsText must be a string");
  const ast = ts.createSourceFile(label + ".js", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  assert.equal(ast.parseDiagnostics.length, 0, label + ": generated JS has syntax errors");
  const bindings = new Map(), exports = new Map();
  const binding = (name, node) => {
    if (!required.has(name)) return;
    const rows = bindings.get(name) ?? []; rows.push(node); bindings.set(name, rows);
  };
  const exported = (name, node) => {
    if (!required.has(name)) return;
    const rows = exports.get(name) ?? []; rows.push(node); exports.set(name, rows);
  };
  function bindingNames(name, node, isExported) {
    if (ts.isIdentifier(name)) {
      binding(name.text, node); if (isExported) exported(name.text, node);
    } else if (ts.isObjectBindingPattern(name) || ts.isArrayBindingPattern(name)) {
      for (const element of name.elements) if (ts.isBindingElement(element)) bindingNames(element.name, node, isExported);
    }
  }
  for (const statement of ast.statements) {
    const namedExport = hasModifier(statement, ts.SyntaxKind.ExportKeyword)
      && !hasModifier(statement, ts.SyntaxKind.DefaultKeyword);
    if (ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement)) {
      if (statement.name) {
        binding(statement.name.text, statement);
        if (namedExport) exported(statement.name.text, statement);
      }
    } else if (ts.isVariableStatement(statement)) {
      for (const declaration of statement.declarationList.declarations) bindingNames(declaration.name, declaration, namedExport);
    } else if (ts.isImportDeclaration(statement) && statement.importClause) {
      const clause = statement.importClause;
      if (clause.name) binding(clause.name.text, clause);
      if (clause.namedBindings) {
        if (ts.isNamespaceImport(clause.namedBindings)) binding(clause.namedBindings.name.text, clause.namedBindings);
        else for (const element of clause.namedBindings.elements) binding(element.name.text, element);
      }
    } else if (ts.isExportDeclaration(statement)) {
      // Bindgen emits direct named functions. Re-exports cannot prove this file's wrappers.
      assert.ok(statement.exportClause, label + ": generated JS wildcard re-export is unsupported");
      if (ts.isNamedExports(statement.exportClause)) {
        for (const element of statement.exportClause.elements) exported(element.name.text, statement);
      } else exported(statement.exportClause.name.text, statement);
    }
  }
  for (const name of requiredNames) {
    const declarations = bindings.get(name) ?? [], outward = exports.get(name) ?? [];
    assert.equal(outward.length, 1, label + ": expected exactly one JS export " + name);
    assert.equal(declarations.length, 1, label + ": expected exactly one JS binding " + name);
    const fn = outward[0];
    assert.ok(ts.isFunctionDeclaration(fn) && fn === declarations[0] && fn.body
      && !fn.asteriskToken && !hasModifier(fn, ts.SyntaxKind.AsyncKeyword),
    label + ": expected a direct synchronous generated function export " + name);
    // Verify this wrapper's scope only, without treating deferred inner code as its call.
    const bindsWasm = binding => {
      if (ts.isIdentifier(binding)) return binding.text === "wasm";
      return (ts.isObjectBindingPattern(binding) || ts.isArrayBindingPattern(binding))
        && binding.elements.some(element => ts.isBindingElement(element) && bindsWasm(element.name));
    };
    let shadowsWasm = fn.parameters.some(parameter => bindsWasm(parameter.name)), matchingCalls = 0;
    function visit(node) {
      if (ts.isVariableDeclaration(node) && bindsWasm(node.name)
        || (ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) && node.name?.text === "wasm") shadowsWasm = true;
      // Declaration names above bind in this scope; their bodies and expression names do not.
      if (ts.isFunctionDeclaration(node) || ts.isFunctionExpression(node) || ts.isArrowFunction(node)
        || ts.isMethodDeclaration(node) || ts.isGetAccessorDeclaration(node) || ts.isSetAccessorDeclaration(node)
        || ts.isConstructorDeclaration(node) || ts.isClassDeclaration(node) || ts.isClassExpression(node)) return;
      if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression)
        && ts.isIdentifier(node.expression.expression) && node.expression.expression.text === "wasm"
        && node.expression.name.text === name) matchingCalls++;
      ts.forEachChild(node, visit);
    }
    visit(fn.body);
    assert.equal(shadowsWasm, false, label + ": wrapper must not shadow wasm");
    assert.equal(matchingCalls, 1, label + ": wrapper must call wasm." + name + " exactly once");
  }
}

/** Static wrapper + WASM export metadata only; never evaluates JS or instantiates WASM. */
function verifyRuntimeExports({ jsText, wasmBytes, label = "renderer" }, requiredNames, countField) {
  assert.equal(typeof label, "string");
  assert.ok(wasmBytes instanceof Uint8Array || wasmBytes instanceof ArrayBuffer, "wasmBytes must be bytes");
  checkJavascript(jsText, label, requiredNames);
  let module;
  try { module = new WebAssembly.Module(wasmBytes); }
  catch (cause) { throw new Error(label + ": cannot compile WASM metadata", { cause }); }
  const rows = WebAssembly.Module.exports(module);
  for (const name of requiredNames) {
    const matches = rows.filter(row => row.name === name);
    assert.equal(matches.length, 1, label + ": expected exactly one WASM export " + name);
    assert.equal(matches[0].kind, "function", label + ": WASM export must be a function " + name);
  }
  return { label, [countField]: requiredNames.length, exports: [...requiredNames],
    javascriptCheckedStatically: true, wasmCheckedAsModule: true, rendererInstantiated: false,
    capabilityValuesEvaluated: false };
}

export function verifyStorageRuntimeExports({ jsText, wasmBytes, label = "renderer" }) {
  return verifyRuntimeExports({ jsText, wasmBytes, label }, STORAGE_RUNTIME_EXPORTS, "storageExportCount");
}

export function verifyNpcShopRuntimeExports({ jsText, wasmBytes, label = "renderer" }) {
  return verifyRuntimeExports({ jsText, wasmBytes, label }, NPC_SHOP_RUNTIME_EXPORTS, "npcShopExportCount");
}

function verifyRuntimeExportFiles({ jsPath, wasmPath, label = "renderer" }, verifyExports) {
  assert.equal(typeof jsPath, "string"); assert.equal(typeof wasmPath, "string");
  const jsBytes = readFileSync(jsPath), wasmBytes = readFileSync(wasmPath);
  const jsText = new TextDecoder("utf-8", { fatal: true }).decode(jsBytes);
  return { ...verifyExports({ jsText, wasmBytes, label }),
    jsPath: path.resolve(jsPath), wasmPath: path.resolve(wasmPath), jsBytes: jsBytes.length, wasmBytes: wasmBytes.length };
}

export function verifyStorageRuntimeExportFiles({ jsPath, wasmPath, label = "renderer" }) {
  return verifyRuntimeExportFiles({ jsPath, wasmPath, label }, verifyStorageRuntimeExports);
}

export function verifyNpcShopRuntimeExportFiles({ jsPath, wasmPath, label = "renderer" }) {
  return verifyRuntimeExportFiles({ jsPath, wasmPath, label }, verifyNpcShopRuntimeExports);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    assert.ok(process.argv.length === 4 || process.argv.length === 5,
      "Usage: node verify-storage-runtime-exports.mjs <generated.js> <generated.wasm> [packageId]");
    const result = verifyStorageRuntimeExportFiles({ jsPath: process.argv[2], wasmPath: process.argv[3], label: process.argv[4] ?? "renderer" });
    console.log(JSON.stringify(result));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
