import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

const componentRoot = new URL("../app/components/", import.meta.url);

async function source(name) {
  return readFile(new URL(name, componentRoot), "utf8");
}

test("bounded original-client controls do not leave visible empty click handlers", async () => {
  const files = await Promise.all([
    source("original-client-overlays.tsx"),
    source("original-client-map-panels.tsx"),
    source("original-client-game-shop.tsx"),
    source("original-client-dialogs.tsx"),
  ]);
  const combined = files.join("\n");

  assert.doesNotMatch(combined, /onClick=\{\(\)\s*=>\s*undefined\}/);
  assert.match(files[0], /disabled\?: boolean/);
  assert.match(files[0], /disabled=\{disabled\}/);
  assert.match(files[0], /onClick=\{onToggleQuestLog\} active=\{showQuestLog\}/);
  assert.doesNotMatch(files[0], /onOpenCharacterTab\("stats2"\)/);
  assert.match(files[1], /value=\{search\}/);
  assert.match(files[1], /searchRef\.current\?\.focus\(\)/);
  assert.match(files[2], /setPage\(\(current\) => Math\.max\(0, current - 1\)\)/);
  assert.match(files[2], /setPage\(\(current\) => Math\.min\(pageCount - 1, current \+ 1\)\)/);
  assert.equal((files[2].match(/disabled=\{currentPage <= 0\}/g) ?? []).length, 2);
  assert.equal((files[2].match(/disabled=\{currentPage >= pageCount - 1\}/g) ?? []).length, 2);
  assert.match(files[3], /\$\{currentPage \+ 1\} \/ \$\{pageCount\}/);
  assert.match(files[3], /onSelect=\{\(\) => setSelectedIndex\(index\)\}/);
});

test("unsupported actions use disabled semantics instead of pretending to reach a backend", async () => {
  const [mapPanels, shop, dialogs] = await Promise.all([
    source("original-client-map-panels.tsx"),
    source("original-client-game-shop.tsx"),
    source("original-client-dialogs.tsx"),
  ]);

  assert.match(mapPanels, /positionBar\} label=.* disabled \/>/);
  assert.match(mapPanels, /upButton\} label=.* disabled \/>/);
  assert.match(mapPanels, /downButton\} label=.* disabled \/>/);
  assert.match(mapPanels, /teleportButton\} label=.* disabled active \/>/);
  assert.match(shop, /positionBar\} label=.* disabled \/>/);
  assert.match(dialogs, /readButton\}\s*label=.*\s*disabled/);
  assert.match(dialogs, /blockListButton\} label=.* disabled \/>/);
  assert.match(dialogs, /bugReportButton\} label=.* disabled \/>/);
});

const jsx = (type, props) => ({ type, props });
const jsxRuntime = { jsx, jsxs: jsx, Fragment: Symbol("fragment") };
const asset = new Proxy({}, { get: () => asset });

function findRendered(node, predicate) {
  if (!node || typeof node !== "object") return null;
  if (predicate(node)) return node;
  const children = node.props?.children;
  for (const child of Array.isArray(children) ? children : [children]) {
    const found = findRendered(child, predicate);
    if (found) return found;
  }
  return null;
}

async function transpiledComponent(name, dependencies) {
  const compiled = ts.transpileModule(await source(name), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
    reportDiagnostics: true,
  });
  assert.deepEqual((compiled.diagnostics ?? []).filter((entry) => entry.category === ts.DiagnosticCategory.Error), []);
  const componentModule = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(
    componentModule.exports,
    componentModule,
    (id) => {
      if (dependencies[id] !== undefined) return dependencies[id];
      throw new Error(`Unexpected ${name} dependency: ${id}`);
    },
  );
  return componentModule.exports;
}

async function realChatFrame() {
  const react = {
    useEffect() {},
    useRef: (current) => ({ current }),
    useState: (initial) => [initial, () => {}],
  };
  const crystalChatType = { Normal: 0, Shout: 1, WhisperIn: 2, Group: 3, Guild: 4,
    Mentor: 5, Relationship: 6, Hint: 7, LineMessage: 8, Announcement: 9, System: 10 };
  class EmptyChatHistory { History = []; receiveChat() {} }
  return transpiledComponent("original-client-panels.tsx", {
    react,
    "react/jsx-runtime": jsxRuntime,
    "../../lib/crystal-chat-history": { CrystalChatHistory: EmptyChatHistory, CrystalChatType: crystalChatType },
    "../../lib/original-ui": { ORIGINAL_UI: asset },
    "../../lib/content-profile": { IS_PLATINUM_176_PROFILE: false },
    "./original-client-audio-settings": { OriginalAudioSettingsControls: () => null },
    "./crystal-gdi-text": { CrystalGdiTextImage: () => null, findCrystalGdiTextAsset: () => null },
    "./original-client-inventory-utils": { originalItemIconPath: () => "" },
    "./original-client-item-tooltip": { OriginalItemTooltip: () => null },
    "./original-client-overlays": { SpriteButton: () => null },
  });
}

function renderedChatInput(panels, props) {
  const tree = panels.ChatFrame({
    t: (key, _params, fallback) => fallback ?? key,
    logs: [], chatMessage: "draft", activeFilter: "all", hiddenFilters: [],
    expanded: true, showSettings: false, transparent: false,
    onChatMessageChange() {}, onSendChat() {}, onCloseSettings() {},
    onToggleHiddenFilter() {}, onToggleAllHiddenFilters() {}, onToggleTransparent() {},
    ...props,
  });
  const input = findRendered(tree, (node) => node.type === "input" && node.props?.className === "chat-textbox");
  assert.ok(input, "real ChatFrame renders its chat textbox");
  return input;
}

function keydown(input, key, nativeEvent, onPreventDefault = () => {}) {
  input.props.onKeyDown({ key, nativeEvent, preventDefault: onPreventDefault });
}

test("real ChatFrame ignores composing Enter and legacy 229, then submits ordinary Enter", async () => {
  const panels = await realChatFrame();
  const sent = [], changed = [];
  let prevented = 0;
  const input = renderedChatInput(panels, {
    chatMessage: "draft", onSendChat: () => sent.push("sent"),
    onChatMessageChange: (value) => changed.push(value),
  });
  assert.equal(input.props.value, "draft");
  assert.equal(input.props["data-chat-prefix"], "");
  assert.equal(input.props["data-chat-visible"], true);
  assert.equal(input.props["aria-hidden"], false);
  input.props.onChange({ target: { value: "draft next" } });
  assert.deepEqual(changed, ["draft next"]);

  const prevent = () => { prevented += 1; };
  keydown(input, "Enter", { isComposing: true, keyCode: 13 }, prevent);
  keydown(input, "Enter", { isComposing: false, keyCode: 229 }, prevent);
  keydown(input, "Enter", { keyCode: 229 }, prevent);
  keydown(input, "a", { isComposing: false, keyCode: 13 }, prevent);
  assert.equal(sent.length, 0);
  assert.equal(prevented, 0, "composition and non-Enter retain browser default behavior");

  keydown(input, "Enter", { isComposing: false, keyCode: 13 }, prevent);
  keydown(input, "Enter", { keyCode: 13 }, prevent);
  assert.equal(sent.length, 2, "fresh ordinary Enter works after composition and with absent flag");
  assert.equal(prevented, 0, "ordinary Enter still does not call preventDefault");
});

test("real GameUiScene send closure keeps empty, prefix, formatting and draft reset behavior", async () => {
  const panels = await realChatFrame();
  const slots = [];
  let hook = 0;
  const react = {
    memo: (component) => component,
    useState(initial) {
      const index = hook++;
      if (!(index in slots)) slots[index] = initial;
      return [slots[index], (value) => { slots[index] = typeof value === "function" ? value(slots[index]) : value; }];
    },
    useRef(initial) { const index = hook++; return slots[index] ??= { current: initial }; },
    useEffect() { hook++; },
    useLayoutEffect() { hook++; },
  };
  const placeholders = new Proxy({}, { get: () => () => null });
  const scene = await transpiledComponent("original-client-game-ui-scene.tsx", new Proxy({
    react,
    "react/jsx-runtime": jsxRuntime,
    "./original-client-panels": panels,
    "../../lib/content-profile": { IS_PLATINUM_176_PROFILE: false },
    "./original-client-map-panels": { hasOriginalMiniMapAsset: () => false },
  }, { get: (target, id) => Object.hasOwn(target, id) ? target[id] : placeholders }));
  const sent = [], changes = [];
  let draft = "";
  const props = {
    t: (key, _params, fallback) => fallback ?? key,
    locale: "en", runtimeMessage: "",
    world: { miniMapIndex: 0, activeNpcDialog: null, questLog: [], equipmentItems: [],
      beltItems: [], interactionHints: [], connected: true, mapTitle: "", mapFileName: "",
      inventoryItems: [] },
    player: null, logs: [], showInventory: false, showCharacter: false, showQuestLog: false,
    activeInventoryTab: "bag", activeCharacterTab: "stats", storageServiceOpenVersion: 0,
    npcShopService: null, npcRepairService: null, inputProfile: "touch", gamepadFamily: "none",
    onChatMessageChange(value) { draft = value; changes.push(value); },
    onSendChat(message) { sent.push(message); },
  };
  function render() {
    hook = 0;
    const tree = scene.GameUiScene({ ...props, chatMessage: draft });
    const frame = findRendered(tree, (node) => node.type === panels.ChatFrame);
    const bar = findRendered(tree, (node) => node.type === panels.ChatFilterBar);
    assert.ok(frame && bar, "production scene creates ChatFrame and filter bindings");
    return { frame, bar, input: renderedChatInput(panels, frame.props) };
  }

  let view = render();
  assert.equal(view.input.props["data-chat-visible"], false);
  keydown(view.input, "Enter", { isComposing: false, keyCode: 13 });
  assert.deepEqual(sent, [], "empty draft does not send");
  assert.deepEqual(changes, [], "empty no-op does not reset draft");

  view.bar.props.onSelectFilter("shout");
  view = render();
  assert.equal(view.frame.props.activeFilter, "shout");
  assert.equal(view.input.props.value, "!");
  assert.equal(view.input.props["data-chat-prefix"], "!");
  assert.equal(view.input.props["data-chat-visible"], true);
  keydown(view.input, "Enter", { isComposing: false, keyCode: 13 });
  assert.deepEqual(sent, [], "prefix-only draft does not send");
  assert.equal(draft, "!", "prefix-only no-op retains draft");

  draft = "hello";
  view = render();
  keydown(view.input, "Enter", { isComposing: false, keyCode: 229 });
  assert.deepEqual(sent, [], "legacy final composition Enter does not reach scene send closure");
  assert.equal(draft, "hello", "composition does not reset draft");
  keydown(view.input, "Enter", { isComposing: false, keyCode: 13 });
  assert.deepEqual(sent, ["!hello"], "production scene applies active filter prefix");
  assert.equal(draft, "!", "production scene resets draft to active prefix");

  draft = "!already";
  view = render();
  keydown(view.input, "Enter", { isComposing: false, keyCode: 13 });
  assert.deepEqual(sent, ["!hello", "!already"], "existing prefix is not duplicated");
  assert.equal(draft, "!");

  view = render();
  view.bar.props.onSelectFilter("all");
  view = render();
  assert.equal(draft, "");
  assert.equal(view.input.props["data-chat-visible"], false);
  draft = "plain  ";
  view = render();
  keydown(view.input, "Enter", { isComposing: false, keyCode: 13 });
  assert.deepEqual(sent, ["!hello", "!already", "plain"], "ordinary all-chat formatting still trims trailing space");
  assert.equal(draft, "", "ordinary all-chat send clears draft");
});
