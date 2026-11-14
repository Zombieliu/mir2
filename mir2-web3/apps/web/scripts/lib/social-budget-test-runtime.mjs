import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import ts from "typescript";

// Instantiate the published glue/WASM without starting its renderer. Each
// fixture owns a real isolated Native reducer, not a JS completion-rule clone.
const metadata = JSON.parse(readFileSync(new URL("../../lib/generated/bevy_runtime_version.json", import.meta.url), "utf8"));
const prefix = `../../public/bevy-runtime-releases/${metadata.version}/pkg-webgl2-shared/`;
const glueUrl = new URL(prefix + "mir2_bevy_runtime.js", import.meta.url);
const wasmBytes = readFileSync(new URL(prefix + "mir2_bevy_runtime_bg.wasm", import.meta.url));
const glue = readFileSync(glueUrl, "utf8");
for (const [name, bytes] of [["mir2_bevy_runtime.js", glue], ["mir2_bevy_runtime_bg.wasm", wasmBytes]]) {
  const row = metadata.files.find(row => row.path === "public/bevy-runtime/pkg-webgl2-shared/" + name);
  assert(row && createHash("sha256").update(bytes).digest("hex") === row.sha256, "actual immutable runtime must match its publication pin");
}
assert.match(glue, /export function processMir2SocialBudget\(/);
const module = new WebAssembly.Module(wasmBytes);
// The only import.meta occurrence is the unused asynchronous URL fallback;
// keep all generated executable glue, imports and WASM byte-for-byte otherwise.
assert.equal(glue.match(/import\.meta\.url/g)?.length, 1);
const glueCode = ts.transpileModule(glue.replace("import.meta.url", JSON.stringify(glueUrl.href)), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;
const factory = new Function("exports", "module", glueCode);
export function createSocialBudgetTestRuntime() {
  const m = { exports: {} };
  factory(m.exports, m);
  m.exports.initSync({ module });
  return m.exports;
}

/** Keep assertions about every existing guard inside the reserved-only finally. */
export function budgetedSendStatements(ts, declaration, ast) {
  const rows = declaration.body.statements;
  assert.equal(rows.length, 3, "sendRaw has one original ingress guard, custody declaration and guarded body");
  assert(ts.isIfStatement(rows[0]));
  assert.match(rows[0].getText(ast), /companionControl/);
  assert.equal(rows[1].getText(ast), "let socialBudgetTicket: SocialBudgetTicket | null = null;");
  assert(ts.isTryStatement(rows[2]) && rows[2].finallyBlock && !rows[2].catchClause);
  const final = rows[2].finallyBlock.statements;
  assert.equal(final.length, 1);
  assert.equal(final[0].getText(ast), "if (socialBudgetTicket) socialBudgetRef.current?.cancelUnsent(socialBudgetTicket);");
  return [rows[0], ...rows[2].tryBlock.statements];
}
