import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(process.env.MIR2_PAGE_TEST_SOURCE ?? new URL("../app/page.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
let screenCommit, currentSurface, edit;
function visit(node) {
  if (ts.isCallExpression(node) && ["useEffect", "useLayoutEffect"].includes(node.expression.getText(ast))) {
    const callback = node.arguments[0];
    if (callback?.getText(ast).includes("screenRef.current = screen;")) screenCommit = { phase: node.expression.getText(ast), callback };
  }
  if (ts.isFunctionDeclaration(node) && node.name?.text === "authSurfaceCurrent") currentSurface = node;
  if (ts.isJsxAttribute(node) && node.name.getText(ast) === "onAccountIdChange") edit = node.initializer.expression;
  ts.forEachChild(node, visit);
}
visit(ast); assert(screenCommit && currentSurface && edit);
function compile(text, environment) {
  const js = ts.transpileModule(text, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText;
  return new Function(...Object.keys(environment), js)(...Object.values(environment));
}
function committedLogin({ epoch = 7, renderEpoch = 7, visible = true, busy = false, screen = "login" } = {}) {
  const environment = { screen, screenRef: { current: "boot" }, document: { visibilityState: visible ? "visible" : "hidden" },
    loginAuthRef: { current: { epoch, surface: "login" } }, authRenderEpoch: renderEpoch,
    loginBusyRef: { current: busy }, accountIdRef: { current: "" }, setAccountId: value => { environment.account = value; } };
  const guard = compile(currentSurface.getText(ast) + "\nreturn authSurfaceCurrent;", environment);
  const change = compile("return (" + edit.getText(ast) + ");", { ...environment, authSurfaceCurrent: guard });
  // React commits layout effects before visible fields can receive an event;
  // passive effects have not run at this first-edit boundary.
  if (screenCommit.phase === "useLayoutEffect") compile("return (" + screenCommit.callback.getText(ast) + ");", environment)();
  change("PlayerAtFirstPaint"); return environment;
}
test("the actual Page accepts the first edit on a freshly committed visible login", () => {
  const result = committedLogin(); assert.equal(result.accountIdRef.current, "PlayerAtFirstPaint");
  assert.equal(result.account, "PlayerAtFirstPaint");
});
for (const [name, options] of [
  ["retired epoch", { renderEpoch: 6 }], ["hidden document", { visible: false }],
  ["pending login", { busy: true }], ["different screen", { screen: "select" }],
]) test(`the actual edit guard still rejects ${name}`, () => {
  const result = committedLogin(options); assert.equal(result.accountIdRef.current, ""); assert.equal(result.account, undefined);
});
