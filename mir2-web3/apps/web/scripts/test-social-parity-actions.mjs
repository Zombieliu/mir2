import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
function evaluate(source, fileName, deps = {}) {
  const result = ts.transpileModule(source, { fileName,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
  });
  const module = { exports: {} };
  new Function("require", "module", "exports", result.outputText)(id => {
    assert.ok(Object.hasOwn(deps, id), `Unexpected dependency ${id}`);
    return deps[id];
  }, module, module.exports);
  return module.exports;
}
const actions = evaluate(read("../lib/social-parity-actions.ts"), "social-parity-actions.ts");
const guildSource = read("../app/components/original-client-guild-window.tsx");
const guildAst = ts.createSourceFile("original-client-guild-window.tsx", guildSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const rankFunctions = guildAst.statements.filter(node => ts.isFunctionDeclaration(node) && node.name?.text === "RankCard");
assert.equal(rankFunctions.length, 1, "exercise the sole production RankCard function");
const permissionDeclarations = guildAst.statements.filter(ts.isVariableStatement)
  .flatMap(node => [...node.declarationList.declarations])
  .filter(node => ts.isIdentifier(node.name) && node.name.text === "PERMISSION_KEYS");
assert.equal(permissionDeclarations.length, 1);

let activeHost;
const hooks = {
  useState(initial) {
    const host = activeHost, index = host.cursor++;
    if (!(index in host.state)) host.state[index] = typeof initial === "function" ? initial() : initial;
    return [host.state[index], next => {
      const value = typeof next === "function" ? next(host.state[index]) : next;
      if (!Object.is(value, host.state[index])) { host.state[index] = value; host.dirty = true; }
    }];
  },
  useEffect(effect, dependencies) {
    const host = activeHost, index = host.cursor++, old = host.dependencies[index];
    if (!old || dependencies.some((value, i) => !Object.is(value, old[i]))) {
      host.dependencies[index] = [...dependencies]; host.effects.push(effect);
    }
  },
};
const jsx = { jsx: (type, props) => ({ type, props }), jsxs: (type, props) => ({ type, props }) };
function ActionButton() { throw new Error("Inspect the actual RankCard button handler without replacing it"); }
const { R16_GUILD_LAYOUT: G } = evaluate(read("../lib/r16-guild-layout.ts"), "r16-guild-layout.ts");
const { guildRect } = evaluate(read("../app/components/original-client-guild-controls.tsx"), "original-client-guild-controls.tsx", {
  react: hooks, "react/jsx-runtime": jsx, "../../lib/r16-guild-layout": { R16_GUILD_LAYOUT: G },
  "./original-client-overlays": { SpriteButton() { throw new Error("The RankCard fixture never mounts the source-art button"); } },
});
const scope = { ...hooks, ...actions, G, guildRect, ActionButton, permissionLabel: (_t, key) => key,
  style: new Proxy({}, { get: () => ({}) }),
};
const cardOutput = ts.transpileModule(`const ${permissionDeclarations[0].getText(guildAst)};\n${rankFunctions[0].getText(guildAst)}`, {
  fileName: "rank-card.tsx",
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
}).outputText;
const RankCard = new Function("scope", "require", "exports", `with(scope) { ${cardOutput}; return RankCard; }`)(scope, id => {
  assert.equal(id, "react/jsx-runtime"); return jsx;
}, {});
function mount(initial) {
  const host = { cursor: 0, state: [], dependencies: [], effects: [], dirty: false };
  let props = initial, tree;
  const render = (next = props) => {
    props = next;
    for (let round = 0; round < 8; round++) {
      activeHost = host; host.cursor = 0; host.dirty = false;
      tree = RankCard(props);
      for (const effect of host.effects.splice(0)) effect();
      if (!host.dirty) return tree;
    }
    throw new Error("Unstable actual RankCard render");
  };
  render();
  return { render, get tree() { return tree; }, get props() { return props; } };
}
function nodes(tree, predicate) {
  const found = [];
  const visit = node => {
    if (Array.isArray(node)) return node.forEach(visit);
    if (!node || typeof node !== "object" || !node.props) return;
    if (predicate(node)) found.push(node);
    visit(node.props.children);
  };
  visit(tree); return found;
}
const single = (tree, predicate) => {
  const found = nodes(tree, predicate); assert.equal(found.length, 1); return found[0];
};
const nameInput = tree => single(tree, node => node.type === "input" && node.props.type !== "checkbox");
const option = (tree, key) => single(tree, node => node.props["data-guild-rank-permission"] === key);
const save = tree => single(tree, node => node.type === ActionButton);
const fixture = (extra = {}) => ({ t: (_key, _args, fallback) => fallback,
  rank: { index: 7, name: "Officer", permissions: ["CanRecruit"] },
  onRenameRank: () => true, onToggleRankPermission: () => true, ...extra });
const editName = (card, value) => { nameInput(card.tree).props.onChange({ target: { value } }); card.render(); };
function nameKey(card, key, extra = {}) {
  const observed = { stopped: 0, prevented: 0, blurred: 0 };
  const event = { key, keyCode: 0, nativeEvent: { isComposing: false },
    stopPropagation() { observed.stopped++; }, preventDefault() { observed.prevented++; },
    currentTarget: { blur() { observed.blurred++; } }, ...extra };
  nameInput(card.tree).props.onKeyDown(event); card.render();
  return observed;
}

test("rename emits one ordinary type3 preserving the exact Native UTF16 name", () => {
  for (const [rankIndex, name] of [[0, "All"], [255, "  Officer  "], [7, "😀".repeat(10)], [7, "   "], [7, "x\0y"]]) {
    assert.deepEqual(actions.guildRankRenameCommand(rankIndex, name), {
      type: "editGuildMember", changeType: 3, rankIndex, name: "", rankName: name,
    });
  }
  assert.equal(Object.hasOwn(actions, "guildRankSaveCommands"), false, "there is no nine-command batch fallback");
});

test("rename rejects non-u8 identity and names outside Native 3..20 UTF16/backslash rule", () => {
  for (const rankIndex of [-1, 256, 2147483647, NaN, Infinity, 1.5, "7", null, undefined])
    assert.equal(actions.guildRankRenameCommand(rankIndex, "Officer"), null);
  for (const name of ["", "ab", "x".repeat(21), "😀".repeat(11), "A\\B", null, 7, {}, undefined])
    assert.equal(actions.guildRankRenameCommand(7, name), null);
});

test("each exact permission key emits a single ordinary type5 for that checkbox only", () => {
  const keys = ["CanChangeRank", "CanRecruit", "CanKick", "CanStoreItem", "CanRetrieveItem", "CanAlterAlliance", "CanChangeNotice", "CanActivateBuff"];
  assert.deepEqual(actions.GUILD_RANK_PERMISSION_KEYS, keys);
  for (const rankIndex of [0, 255]) for (const [bit, key] of keys.entries()) for (const enabled of [false, true]) {
    assert.deepEqual(actions.guildRankPermissionCommand(rankIndex, key, enabled), {
      type: "editGuildMember", changeType: 5, rankIndex, name: String(enabled), rankName: String(bit),
    });
  }
  for (const key of ["canRecruit", "CanUnknown", "", null, 1, {}])
    assert.equal(actions.guildRankPermissionCommand(7, key, true), null);
  for (const enabled of ["true", "false", 0, 1, null, {}, undefined])
    assert.equal(actions.guildRankPermissionCommand(7, "CanRecruit", enabled), null);
  for (const rankIndex of [-1, 256, 2147483647, NaN, Infinity, 1.5, "7", null])
    assert.equal(actions.guildRankPermissionCommand(rankIndex, "CanRecruit", true), null);
});

test("actual checkbox submits once immediately, stays server-owned, and never submits rename", () => {
  const calls = [], renames = [];
  const card = mount(fixture({ onToggleRankPermission: (...args) => { calls.push(args); return true; },
    onRenameRank: (...args) => { renames.push(args); return true; } }));
  assert.equal(option(card.tree, "CanRecruit").props.checked, true);
  assert.equal(option(card.tree, "CanKick").props.checked, false);
  option(card.tree, "CanKick").props.onChange({ target: { checked: true } });
  card.render();
  assert.deepEqual(calls, [[7, "CanKick", true]]);
  assert.equal(option(card.tree, "CanKick").props.checked, false, "send is not an authoritative options update");
  assert.equal(save(card.tree).props.disabled, false, "Native allows saving a valid unchanged name; a checkbox never invokes it");
  assert.deepEqual(renames, []);
  card.render({ ...card.props, rank: { ...card.props.rank, permissions: ["CanRecruit", "CanKick"] } });
  assert.equal(option(card.tree, "CanKick").props.checked, true);
});

test("actual Save renames only and retains the untrimmed draft on local rejection and option receipt", () => {
  const renames = [], options = [];
  const card = mount(fixture({ onRenameRank: (...args) => { renames.push(args); return false; },
    onToggleRankPermission: (...args) => { options.push(args); return false; } }));
  editName(card, "  Leader  ");
  assert.equal(save(card.tree).props.disabled, false);
  save(card.tree).props.onClick(); card.render();
  assert.deepEqual(renames, [[7, "  Leader  "]]);
  assert.deepEqual(options, []);
  assert.equal(nameInput(card.tree).props.value, "  Leader  ");
  card.render({ ...card.props, rank: { ...card.props.rank, permissions: ["CanKick"] } });
  assert.equal(nameInput(card.tree).props.value, "  Leader  ", "unrelated option receipt cannot reset the rename draft");
  assert.equal(option(card.tree, "CanKick").props.checked, true);
});

test("actual Enter saves the exact untrimmed draft once and never submits any checkbox", () => {
  const renames = [], options = [];
  const card = mount(fixture({ onRenameRank: (...args) => { renames.push(args); return true; },
    onToggleRankPermission: (...args) => { options.push(args); return true; } }));
  editName(card, "  Leader  ");
  assert.deepEqual(nameKey(card, "Enter"), { stopped: 1, prevented: 1, blurred: 0 });
  assert.deepEqual(renames, [[7, "  Leader  "]]); assert.deepEqual(options, []);
  assert.equal(nameInput(card.tree).props.value, "  Leader  ");
  assert.equal(save(card.tree).props.nativeSave, true, "the selected editor requests the source Save skin");
});

test("actual composing and keyCode229 Enter/Escape neither save nor clear/blur the draft", () => {
  const calls = [], card = mount(fixture({ onRenameRank: (...args) => { calls.push(args); return true; } }));
  editName(card, "職級草稿");
  for (const key of ["Enter", "Escape"]) for (const extra of [
    { nativeEvent: { isComposing: true } }, { keyCode: 229 },
  ]) {
    assert.deepEqual(nameKey(card, key, extra), { stopped: 1, prevented: 0, blurred: 0 });
    assert.equal(nameInput(card.tree).props.value, "職級草稿");
  }
  assert.deepEqual(calls, []);
});

test("actual Escape clears the Native draft and blurs, retains the cleared dirty value and sends nothing", () => {
  const calls = [], card = mount(fixture({ onRenameRank: (...args) => { calls.push(args); return true; } }));
  editName(card, "Veterans");
  assert.deepEqual(nameKey(card, "Escape"), { stopped: 1, prevented: 1, blurred: 1 });
  assert.equal(nameInput(card.tree).props.value, ""); assert.equal(save(card.tree).props.disabled, true);
  card.render({ ...card.props, rank: { ...card.props.rank, name: "Other editor" } });
  assert.equal(nameInput(card.tree).props.value, "", "an external same-rank rename cannot overwrite the cleared dirty draft");
  nameKey(card, "Enter"); assert.deepEqual(calls, []);
});

test("actual Enter retains the Save validity and Host rename/authority barriers", () => {
  const calls = [];
  for (const extra of [
    { onRenameRank: undefined }, { rankEditPending: { renameRankIndices: [7], permission: false } },
    { rankEditPending: { renameRankIndices: [], permission: false, editableRankIndices: [8] } },
    { rank: { index: 256, name: "Officer", permissions: [] } },
  ]) {
    const card = mount(fixture({ onRenameRank: (...args) => { calls.push(args); return true; }, ...extra }));
    assert.deepEqual(nameKey(card, "Enter"), { stopped: 1, prevented: 1, blurred: 0 });
  }
  const card = mount(fixture({ onRenameRank: (...args) => { calls.push(args); return true; } }));
  for (const value of ["", "ab", "A\\B", "x".repeat(21)]) { editName(card, value); nameKey(card, "Enter"); }
  assert.deepEqual(calls, []);
});

test("actual dirty draft survives external same-rank names and options until an exact authoritative echo", () => {
  const card = mount(fixture()); editName(card, "  Veterans  ");
  card.render({ ...card.props, rank: { ...card.props.rank, name: "Other editor", permissions: ["CanKick"] } });
  assert.equal(nameInput(card.tree).props.value, "  Veterans  ");
  assert.equal(option(card.tree, "CanKick").props.checked, true);
  card.render({ ...card.props, rank: { ...card.props.rank, name: "  Veterans  " } });
  assert.equal(nameInput(card.tree).props.value, "  Veterans  ");
  card.render({ ...card.props, rank: { ...card.props.rank, name: "New authoritative name" } });
  assert.equal(nameInput(card.tree).props.value, "New authoritative name", "matching echo makes the draft clean again");
});

test("actual pristine editor follows authoritative names; local send success alone never clears dirty", () => {
  const calls = [], card = mount(fixture({ onRenameRank: (...args) => { calls.push(args); return true; } }));
  card.render({ ...card.props, rank: { ...card.props.rank, name: "Remote first" } });
  assert.equal(nameInput(card.tree).props.value, "Remote first");
  editName(card, "Mine next"); save(card.tree).props.onClick(); card.render();
  assert.deepEqual(calls, [[7, "Mine next"]]);
  card.render({ ...card.props, rank: { ...card.props.rank, name: "Remote after send" } });
  assert.equal(nameInput(card.tree).props.value, "Mine next");
  assert.deepEqual(nameKey(card, "ArrowLeft"), { stopped: 1, prevented: 0, blurred: 0 });
  assert.equal(calls.length, 1, "editing/navigation keys do not emit another rename");
});

test("Host rename and option barriers are independent and generate no hidden action", () => {
  const renames = [], options = [];
  const props = fixture({ onRenameRank: (...args) => { renames.push(args); return true; },
    onToggleRankPermission: (...args) => { options.push(args); return true; },
    rankEditPending: { renameRankIndices: [7], permission: false } });
  const card = mount(props);
  assert.equal(nameInput(card.tree).props.disabled, true);
  assert.equal(save(card.tree).props.disabled, true);
  save(card.tree).props.onClick();
  assert.deepEqual(renames, []);
  option(card.tree, "CanRecruit").props.onChange({ target: { checked: false } });
  assert.deepEqual(options, [[7, "CanRecruit", false]]);
  card.render({ ...props, rankEditPending: { renameRankIndices: [8], permission: true } });
  editName(card, "Leader");
  assert.equal(nameInput(card.tree).props.disabled, false);
  assert.equal(save(card.tree).props.disabled, false);
  assert.equal(option(card.tree, "CanRecruit").props.disabled, true);
  option(card.tree, "CanRecruit").props.onChange({ target: { checked: false } });
  assert.equal(options.length, 1);
  save(card.tree).props.onClick();
  assert.deepEqual(renames, [[7, "Leader"]]);
});

test("missing callbacks or invalid rank/name cannot invoke a fallback or a batch", () => {
  const calls = [];
  for (const props of [fixture({ onRenameRank: undefined, onToggleRankPermission: undefined }),
    fixture({ rank: { index: 256, name: "Officer", permissions: [] },
      onRenameRank: (...args) => calls.push(args), onToggleRankPermission: (...args) => calls.push(args) })]) {
    const card = mount(props); editName(card, "Leader");
    assert.equal(save(card.tree).props.disabled, true); save(card.tree).props.onClick();
    for (const key of actions.GUILD_RANK_PERMISSION_KEYS) {
      assert.equal(option(card.tree, key).props.disabled, true);
      option(card.tree, key).props.onChange({ target: { checked: true } });
    }
  }
  const card = mount(fixture({ onRenameRank: (...args) => calls.push(args) }));
  for (const name of ["ab", "A\\B", "x".repeat(21)]) {
    editName(card, name); assert.equal(save(card.tree).props.disabled, true); save(card.tree).props.onClick();
  }
  assert.deepEqual(calls, []);
});

test("ExtraWindows forwards the independent callbacks and Host barriers without the old batch prop", () => {
  const text = read("../app/components/original-client-extra-windows.tsx");
  const ast = ts.createSourceFile("extra-windows.tsx", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const elements = [];
  const visit = node => {
    if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(ast) === "GuildWindow") elements.push(node);
    ts.forEachChild(node, visit);
  };
  visit(ast); assert.equal(elements.length, 1);
  const attributes = new Map(elements[0].attributes.properties.filter(ts.isJsxAttribute)
    .map(node => [node.name.getText(ast), node.initializer?.getText(ast)]));
  for (const key of ["onRenameRank", "onToggleRankPermission", "onCreateRank", "rankEditPending", "viewerRankIndex", "guildScope"])
    assert.equal(attributes.get(key), `{guild.${key}}`);
  assert.equal(attributes.has("onSaveRank"), false);
});
