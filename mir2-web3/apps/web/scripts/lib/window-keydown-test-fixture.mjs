import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";

const options = { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS };
const hookSource = readFileSync(new URL("../../lib/use-window-keydown.ts", import.meta.url), "utf8");
const hookCode = ts.transpileModule(hookSource, { compilerOptions: options }).outputText;
const shell = ts.createSourceFile("shell.tsx", readFileSync(new URL("../../app/original-client-shell.tsx", import.meta.url), "utf8"),
  ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const declarations = new Map(), bindings = [];
function visit(node) {
  if (ts.isFunctionDeclaration(node) && ["handleShortcutKey", "keyboardInputTargetIsEditable"].includes(node.name?.text)) {
    assert.equal(declarations.has(node.name.text), false);
    declarations.set(node.name.text, node.getText(shell));
  }
  if (ts.isCallExpression(node) && node.expression.getText(shell) === "useWindowKeydown") bindings.push(node);
  ts.forEachChild(node, visit);
}
visit(shell);
assert.equal(declarations.size, 2, "execute the actual Shell shortcut and editable declarations");
assert.equal(bindings.length, 1, "execute the actual Shell's sole keydown binding");
assert.equal(bindings[0].arguments.length, 1);
const shellCode = ts.transpileModule([...declarations.values()].join("\n")
  + "\nreturn (" + bindings[0].arguments[0].getText(shell) + ");", { compilerOptions: options }).outputText;

/** Actual Shell declarations/binding and actual hook, with distinct render and
 * layout commit phases. The supplied window owns event ordering and assertions. */
export function createShellKeydownFixture(scope) {
  const slots = [];
  let cursor = 0, pending = null, unmounted = false;
  const react = {
    useRef(initial) {
      const index = cursor++;
      slots[index] ??= { kind: "ref", value: { current: initial } };
      assert.equal(slots[index].kind, "ref");
      return slots[index].value;
    },
    useLayoutEffect(setup, deps) {
      assert.ok(pending, "layout effects must be observed during render");
      pending.push({ index: cursor++, setup, deps: deps && [...deps] });
    },
  };
  const module = { exports: {} };
  new Function("exports", "module", "require", "window", hookCode)(module.exports, module, name => {
    assert.equal(name, "react"); return react;
  }, scope.window);
  return {
    render(changes = {}) {
      assert.equal(unmounted, false);
      Object.assign(scope, changes);
      cursor = 0; pending = [];
      const handler = new Function(...Object.keys(scope), shellCode)(...Object.values(scope));
      module.exports.useWindowKeydown(handler);
    },
    commit() {
      assert.equal(unmounted, false);
      assert.ok(pending);
      const changed = pending.filter(effect => {
        const old = slots[effect.index];
        return !old || !effect.deps || !old.deps || old.deps.length !== effect.deps.length
          || !effect.deps.every((value, index) => Object.is(value, old.deps[index]));
      });
      pending = null;
      for (const effect of changed) slots[effect.index]?.cleanup?.();
      for (const effect of changed) slots[effect.index] = {
        kind: "effect", deps: effect.deps, cleanup: effect.setup(),
      };
    },
    unmount() {
      assert.equal(unmounted, false);
      unmounted = true; pending = null;
      for (const slot of slots) if (slot?.kind === "effect") slot.cleanup?.();
    },
  };
}
