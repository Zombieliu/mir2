import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const require = createRequire(import.meta.url);
const componentSource = readFileSync(new URL("../app/components/original-client-combat-mode-controls.tsx", import.meta.url), "utf8");
const code = ts.transpileModule(componentSource, { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX,
} }).outputText;

function fixture(overrides = {}) {
  let expanded = false;
  const calls = [];
  const module = { exports: {} };
  new Function("require", "module", "exports", code)(name => name === "react" ? {
    useId: () => "combat-options", useState: () => [expanded, update => {
      expanded = typeof update === "function" ? update(expanded) : update;
    }],
  } : require(name), module, module.exports);
  const props = { mode: 0, shortcut: "Ctrl + H", enabled: true, inSafeZone: false,
    onSelectMode: mode => calls.push(mode), ...overrides };
  return { calls, props, render: () => module.exports.OriginalClientCombatModeControls(props) };
}
function nodes(tree, predicate) {
  if (!tree || typeof tree !== "object") return [];
  const children = [tree.props?.children].flat(Infinity);
  return [...(predicate(tree) ? [tree] : []), ...children.flatMap(child => nodes(child, predicate))];
}
const buttons = tree => nodes(tree, node => node.type === "button");
const markup = f => renderToStaticMarkup(f.render());

test("visible control teaches the actual mode shortcut, Shift attack, and safe-zone restriction", () => {
  const f = fixture({ inSafeZone: true, shortcut: "Alt + P" });
  assert.match(markup(f), /攻击：和平/);
  assert.match(markup(f), /Alt \+ P/);
  assert.doesNotMatch(markup(f), /Ctrl \+ H/);
  assert.match(markup(f), /Shift \+ 左键点击玩家攻击/);
  assert.match(markup(f), /安全区内禁止攻击玩家/);
  assert.doesNotMatch(markup(f), /<select/);
});

test("mode choices send one selection and wait for the authoritative mode prop", () => {
  const f = fixture();
  buttons(f.render())[0].props.onClick();
  const choices = buttons(f.render()).slice(1);
  assert.deepEqual(choices.map(button => button.props.children), ["和平", "组队", "行会", "敌对行会", "善恶", "全体"]);
  assert.deepEqual(choices.map(button => button.props["aria-pressed"]), [true, false, false, false, false, false]);
  choices[5].props.onClick();
  assert.deepEqual(f.calls, [5]);
  assert.match(markup(f), /攻击：和平/);
  assert.equal(buttons(f.render()).length, 1);
  f.props.mode = 5;
  assert.match(markup(f), /攻击：全体/);
});

test("all six displayed modes map to the existing numeric protocol values", () => {
  for (let mode = 0; mode < 6; mode++) {
    const f = fixture();
    buttons(f.render())[0].props.onClick();
    buttons(f.render())[mode + 1].props.onClick();
    assert.deepEqual(f.calls, [mode]);
  }
});

test("unknown and disabled mode controls do not issue requests", () => {
  for (const mode of [null, -1, 6, 1.5, NaN]) {
    const f = fixture({ mode });
    assert.match(markup(f), /同步中/);
    assert.equal(buttons(f.render())[0].props.disabled, true);
    assert.deepEqual(f.calls, []);
  }
  const f = fixture();
  buttons(f.render())[0].props.onClick();
  f.props.enabled = false;
  const disabled = buttons(f.render());
  assert.ok(disabled.every(button => button.props.disabled));
  disabled[6].props.onClick();
  assert.deepEqual(f.calls, []);
});

test("foreground pointer and compatibility mouse edges do not bubble into the game", () => {
  const f = fixture(), tree = f.render();
  assert.equal(tree.props["data-ui-interactive"], "true");
  for (const name of ["onPointerDown", "onPointerUp", "onMouseDown", "onMouseUp", "onClick", "onDoubleClick"]) {
    let stopped = false;
    tree.props[name]({ stopPropagation: () => { stopped = true; } });
    assert.equal(stopped, true, name);
  }
  for (const name of ["onKeyDown", "onKeyUp"]) for (const key of ["Enter", " "]) {
    let stopped = false;
    tree.props[name]({ key, stopPropagation: () => { stopped = true; } });
    assert.equal(stopped, true, `${name} ${key}`);
  }
  buttons(tree)[0].props.onClick();
  f.render().props.onKeyDown({ key: "Escape", stopPropagation() {} });
  assert.equal(buttons(f.render()).length, 1);
});

test("focused mode buttons consume movement keydowns while keyups can release an existing world hold", () => {
  const tree = fixture().render();
  for (const key of ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"]) {
    let stopped = false;
    tree.props.onKeyDown({ key, stopPropagation: () => { stopped = true; } });
    assert.equal(stopped, true, `${key} must not start world movement`);
    stopped = false;
    tree.props.onKeyUp({ key, stopPropagation: () => { stopped = true; } });
    assert.equal(stopped, false, `${key} release must still retire an existing world hold`);
  }
});
