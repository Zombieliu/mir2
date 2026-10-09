import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

// Controlled fixtures execute the actual transpiled component. This generic
// hook/context scheduler is not ReactDOM, concurrency, hydration or device proof.
const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const requireWeb = createRequire(path.join(web, "package.json"));
const ts = requireWeb("typescript");
const jsxRuntime = requireWeb("react/jsx-runtime");
const GOLDEN = {
  "en": {
    "install": "Install game",
    "fullscreen": "Full screen",
    "title": "Play without browser bars",
    "body": "Install Mir 2 on your Home Screen for a stable landscape, app-like game window.",
    "iosSteps": "Tap Share, then Add to Home Screen. Launch Mir 2 from its new Home Screen icon.",
    "androidSteps": "Choose Install game. If no prompt appears, open the browser menu and tap Install app.",
    "close": "Not now",
    "unavailable": "Full screen is unavailable here. Install the game for the cleanest view."
  },
  "pt": {
    "install": "Instalar jogo",
    "fullscreen": "Tela cheia",
    "title": "Jogue sem as barras do navegador",
    "body": "Instale o Mir 2 na tela inicial para jogar em uma janela estável e horizontal.",
    "iosSteps": "Toque em Compartilhar e em Adicionar à Tela de Início. Abra o Mir 2 pelo novo ícone.",
    "androidSteps": "Escolha Instalar jogo. Se nada aparecer, abra o menu do navegador e toque em Instalar app.",
    "close": "Agora não",
    "unavailable": "A tela cheia não está disponível aqui. Instale o jogo para obter a melhor visualização."
  },
  "zh": {
    "install": "安装游戏",
    "fullscreen": "进入全屏",
    "title": "隐藏浏览器导航栏",
    "body": "将 Mir 2 添加到主屏幕，即可使用稳定横屏的独立游戏窗口。",
    "iosSteps": "点击浏览器的分享按钮，再选“添加到主屏幕”，之后从桌面上的 Mir 2 图标启动。",
    "androidSteps": "点击“安装游戏”；若未出现系统提示，请打开浏览器菜单并选择“安装应用”。",
    "close": "暂不安装",
    "unavailable": "当前浏览器不能直接全屏，请安装到主屏幕以获得完整游戏画面。"
  }
};
const shellPath = path.join(web, "app/components/pwa-game-shell.tsx");
const shellSource = fs.readFileSync(shellPath, "utf8");
const compiled = ts.transpileModule(shellSource, {
  fileName: shellPath,
  reportDiagnostics: true,
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
    jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true },
});
assert.equal((compiled.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
const equalDeps = (a, b) => a !== undefined && b !== undefined &&
  a.length === b.length && a.every((value, i) => Object.is(value, b[i]));
const strictApi = (name, values) => new Proxy(values, { get(target, key) {
  if (!(key in target)) throw Error("Unsupported " + name + " API: " + String(key));
  return target[key];
}});
const CONTEXT = Symbol("controlled-context");
const PROVIDER = Symbol("controlled-provider");

class Hooks {
  constructor() {
    this.fibers = new Map(); this.queue = []; this.effects = [];
    this.dirty = true; this.root = null; this.current = null;
    this.contexts = new Set(); this.contextValues = new Map();
    this.lastContextValues = new Map(); this.renders = 0; this.tree = null;
    this.react = strictApi("React", {
      createContext: value => {
        const context = strictApi("Context", { [CONTEXT]: true, $$typeof: Symbol.for("react.context"), displayName: undefined, defaultValue: value,
          Provider: { [PROVIDER]: true, context: null } });
        context.Provider.context = context;
        this.contexts.add(context);
        return context;
      },
      useContext: context => {
        this.slot("context");
        if (!this.contexts.has(context)) throw Error("Unknown context");
        return this.contextValues.has(context) ? this.contextValues.get(context) : context.defaultValue;
      },
      useState: initial => {
        const slot = this.slot("state");
        if (!slot.initialized) {
          slot.initialized = true; slot.value = typeof initial === "function" ? initial() : initial;
          slot.setter = update => { this.queue.push({ slot, update }); };
        }
        return [slot.value, slot.setter];
      },
      useMemo: (factory, deps) => this.memo("memo", factory, deps),
      useCallback: (callback, deps) => this.memo("callback", () => callback, deps),
      useEffect: (callback, deps) => {
        const slot = this.slot("effect");
        if (!slot.committed || !equalDeps(slot.deps, deps)) {
          this.effects.push({ slot, callback, deps });
        }
      },
    });
  }
  slot(kind) {
    if (!this.current) throw Error("Hook outside component");
    const fiber = this.current, index = fiber.cursor++;
    const slot = fiber.hooks[index] ??= { kind };
    if (slot.kind !== kind) throw Error("Changed hook order");
    return slot;
  }
  memo(kind, factory, deps) {
    const slot = this.slot(kind);
    if (!slot.initialized || !equalDeps(slot.deps, deps)) {
      slot.initialized = true; slot.deps = deps?.slice(); slot.value = factory();
    }
    return slot.value;
  }
  visit(node, location = "root") {
    if (node == null || typeof node === "boolean") return null;
    if (typeof node === "string" || typeof node === "number") return node;
    if (Array.isArray(node)) return node.map((child, i) => this.visit(child, location + "." + i));
    if (!node || typeof node !== "object" || !("type" in node)) throw Error("Unsupported JSX node");
    const { type, props } = node;
    if (typeof type === "function") {
      let fiber = this.fibers.get(location);
      if (fiber && fiber.type !== type) { this.cleanup(fiber); this.fibers.delete(location); fiber = null; }
      if (!fiber) { fiber = { type, hooks: [], cursor: 0 }; this.fibers.set(location, fiber); }
      fiber.visited = true; fiber.cursor = 0;
      const previous = this.current; this.current = fiber;
      let rendered;
      try { rendered = type(props); } finally { this.current = previous; }
      if (fiber.expectedHooks !== undefined && fiber.expectedHooks !== fiber.cursor) throw Error("Changed hook count");
      fiber.expectedHooks = fiber.cursor;
      return this.visit(rendered, location + ".render");
    }
    if (type === Symbol.for("react.fragment")) return this.visit(props.children, location + ".fragment");
    let context;
    if (this.contexts.has(type)) context = type;
    else if (type?.[PROVIDER]) context = type.context;
    if (context) {
      if (!this.contexts.has(context)) throw Error("Unknown provider");
      const had = this.contextValues.has(context), previous = this.contextValues.get(context);
      this.contextValues.set(context, props.value); this.lastContextValues.set(context, props.value);
      const rendered = this.visit(props.children, location + ".provider");
      if (had) this.contextValues.set(context, previous); else this.contextValues.delete(context);
      return rendered;
    }
    if (typeof type !== "string") throw Error("Unsupported component/context type");
    return { type, props, children: this.visit(props.children, location + ".children") };
  }
  cleanup(fiber) {
    for (const slot of fiber.hooks) if (slot.kind === "effect" && typeof slot.cleanup === "function") slot.cleanup();
  }
  flush({ commit = true } = {}) {
    for (let pass = 0; pass < 40; pass++) {
      let changed = false;
      for (const { slot, update } of this.queue.splice(0)) {
        const value = typeof update === "function" ? update(slot.value) : update;
        if (!Object.is(slot.value, value)) { slot.value = value; changed = true; }
      }
      if (!this.dirty && !changed) return this.tree;
      this.dirty = false; this.effects = [];
      for (const fiber of this.fibers.values()) fiber.visited = false;
      this.tree = this.visit(this.root()); this.renders++;
      for (const [key, fiber] of this.fibers) if (!fiber.visited) {
        this.cleanup(fiber); this.fibers.delete(key);
      }
      if (commit) {
        for (const { slot, callback, deps } of this.effects) {
          if (typeof slot.cleanup === "function") slot.cleanup();
          slot.committed = true; slot.deps = deps?.slice();
          const cleanup = callback();
          if (cleanup !== undefined && typeof cleanup !== "function") throw Error("Invalid effect cleanup");
          slot.cleanup = cleanup;
        }
      }
      if (!this.queue.length) return this.tree;
    }
    throw Error("Controlled hook settle limit exceeded");
  }
  rerender() { this.dirty = true; return this.flush(); }
  unmount() { for (const fiber of this.fibers.values()) this.cleanup(fiber);
    this.fibers.clear(); this.queue = []; this.root = () => null; this.tree = null; }
}

class Target {
  constructor() { this.listeners = new Map(); this.calls = []; }
  addEventListener(type, callback) {
    if (typeof callback !== "function") throw Error("Unsupported listener");
    const set = this.listeners.get(type) ?? new Set(); set.add(callback); this.listeners.set(type, set);
    this.calls.push(["add", type, callback]);
  }
  removeEventListener(type, callback) { this.listeners.get(type)?.delete(callback); this.calls.push(["remove", type, callback]); }
  dispatch(type, fields = {}) {
    const event = { ...fields, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
    for (const callback of [...(this.listeners.get(type) ?? [])]) callback(event);
    return event;
  }
  count() { return [...this.listeners.values()].reduce((sum, set) => sum + set.size, 0); }
}
class Clock {
  constructor() { this.now = 0; this.next = 1; this.timers = new Map(); this.log = []; }
  setTimeout(callback, delay) {
    const id = this.next++; this.timers.set(id, { at: this.now + delay, callback });
    this.log.push(["set", id, delay]); return id;
  }
  clearTimeout(id) { this.log.push(["clear", id]); this.timers.delete(id); }
  advance(delta, flush) {
    const end = this.now + delta;
    for (;;) {
      const next = [...this.timers].filter(([, timer]) => timer.at <= end)
        .sort((a, b) => a[1].at - b[1].at || a[0] - b[0])[0];
      if (!next) break;
      this.now = next[1].at; this.timers.delete(next[0]); next[1].callback(); flush();
    }
    this.now = end;
  }
}
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function nodes(tree, predicate) {
  if (Array.isArray(tree)) return tree.flatMap(child => nodes(child, predicate));
  if (!tree || typeof tree !== "object") return [];
  return [...(predicate(tree) ? [tree] : []), ...nodes(tree.children, predicate)];
}
const directText = node => [node?.children].flat(Infinity).filter(child => typeof child === "string").join("");
const allText = tree => {
  if (Array.isArray(tree)) return tree.map(allText).join("");
  if (typeof tree === "string") return tree;
  return tree && typeof tree === "object" ? allText(tree.children) : "";
};
class App {
  constructor({ ios = false, navigatorPresent = true, commit = true, legacy = false,
    visible = true, language = "en", deniedStorage = false } = {}) {
    this.hooks = new Hooks(); this.clock = new Clock(); this.window = new Target(); this.document = new Target();
    this.pathname = "/"; this.visible = visible; this.language = language; this.legacy = legacy;
    this.facts = new Map([["(pointer: coarse)", true], ["(orientation: portrait)", false],
      ["(display-mode: standalone)", false], ["(display-mode: fullscreen)", false]]);
    this.media = []; this.storage = new Map(); this.deniedStorage = deniedStorage; this.promptCalls = 0;
    this.window.matchMedia = query => {
      const target = new Target();
      Object.defineProperty(target, "matches", { get: () => this.facts.get(query) ?? false });
      this.media.push({ query, target }); return target;
    };
    this.window.setTimeout = (callback, delay) => this.clock.setTimeout(callback, delay);
    this.window.clearTimeout = id => this.clock.clearTimeout(id);
    this.document.documentElement = { dataset: {}, requestFullscreen: undefined };
    this.document.fullscreenElement = null;
    this.navigator = { platform: ios ? "iPhone" : "Linux", userAgent: ios ? "iPhone" : "Android",
      maxTouchPoints: ios ? 1 : 0, standalone: false };
    Object.defineProperty(this.navigator, "language", { get: () => {
      if (legacy) return "en-US";
      throw Error("navigator.language must not select PWA copy");
    } });
    this.orientation = {};
    this.localStorage = {
      getItem: key => { if (this.deniedStorage) throw Error("storage denied"); return this.storage.get(key) ?? null; },
      setItem: (key, value) => { if (this.deniedStorage) throw Error("storage denied"); this.storage.set(key, value); },
    };
    const module = { exports: {} };
    const require = name => {
      if (name === "react") return this.hooks.react;
      if (name === "react/jsx-runtime") return jsxRuntime;
      if (name === "next/navigation") return strictApi("next/navigation", { usePathname: () => this.pathname });
      throw Error("Unsupported component import: " + name);
    };
    const globals = { module, exports: module.exports, require, window: this.window,
      document: this.document, localStorage: this.localStorage, screen: { orientation: this.orientation } };
    if (navigatorPresent) globals.navigator = this.navigator;
    vm.runInNewContext(compiled.outputText, globals, { filename: shellPath, timeout: 2000 });
    this.exports = module.exports;
    this.hooks.root = () => {
      const consumer = this.visible ? jsxRuntime.jsx(this.exports.PwaGameShell, { language: this.language }) : null;
      return legacy && !this.exports.PwaGameShellCapture ? consumer : jsxRuntime.jsx(this.exports.PwaGameShellCapture, { children: consumer });
    };
    this.hooks.flush({ commit });
  }
  get tree() { return this.hooks.tree; }
  class(name) { return nodes(this.tree, n => n.props.className === name)[0]; }
  click(name) { const button = this.class(name); assert.ok(button, "real rendered " + name);
    assert.equal(typeof button.props.onClick, "function"); button.props.onClick(); this.hooks.flush(); }
  locale(language) { this.language = language; this.hooks.rerender(); }
  consumer(visible) { this.visible = visible; this.hooks.rerender(); }
  route(pathname) { this.pathname = pathname; this.hooks.rerender(); }
  event(type, fields) { const event = this.window.dispatch(type, fields); this.hooks.flush(); return event; }
  mediaChange(query, value) { this.facts.set(query, value);
    for (const item of this.media.filter(r => r.query === query)) item.target.dispatch("change");
    this.hooks.flush(); }
  advance(ms) { this.clock.advance(ms, () => this.hooks.flush()); }
  listeners() { return this.window.count() + this.document.count() + this.media.reduce((n, item) => n + item.target.count(), 0); }
  context() { return [...this.hooks.lastContextValues.values()][0]; }
  async drain() { for (let i = 0; i < 8; i++) { await Promise.resolve(); this.hooks.flush(); } }
  installEvent(prompt = deferred(), choice = deferred()) {
    const event = this.event("beforeinstallprompt", {
      prompt: () => { this.promptCalls++; return prompt.promise; }, userChoice: choice.promise,
    });
    return { event, prompt, choice };
  }
}
const copyFor = language => GOLDEN[language === "zh-CN" ? "zh" : language === "pt-BR" ? "pt" : "en"];
function checkVisible(app, language, ios = false) {
  const expected = copyFor(language);
  assert.equal(directText(app.class("mir-pwa-action install")), expected.install);
  assert.equal(app.class("mir-pwa-actions").props["aria-label"], expected.title);
  if (!ios) assert.equal(directText(app.class("mir-pwa-action fullscreen")), expected.fullscreen);
  else assert.equal(app.class("mir-pwa-action fullscreen"), undefined);
  assert.equal(directText(nodes(app.tree, n => n.props.id === "mir-pwa-guide-title")[0]), expected.title);
  assert.equal(directText(nodes(app.tree, n => n.type === "p" && !n.props.className)[0]), expected.body);
  assert.equal(directText(app.class("mir-pwa-guide-steps")), ios ? expected.iosSteps : expected.androidSteps);
  assert.equal(directText(app.class("mir-pwa-guide-dismiss")), expected.close);
  assert.equal(directText(app.class("mir-pwa-guide-primary")), expected.install);
}

if (process.argv.includes("--baseline-locale-counterexample")) {
  test("actual explicit zh-CN prop overrides contrary navigator", () => {
    const app = new App({ legacy: true, language: "zh-CN" });
    app.advance(5000);
    checkVisible(app, "zh-CN");
  });
} else {
test("generic queued batching, lazy state, Object.is, callback deps and cleanup-before-setup", () => {
  const hooks = new Hooks(), order = [];
  let init = 0, set, callback, priorSetter, value = NaN, dependency = 0;
  function Probe() {
    const [state, setter] = hooks.react.useState(() => { init++; return value; });
    set = setter; if (priorSetter) assert.equal(setter, priorSetter); priorSetter = setter;
    callback = hooks.react.useCallback(() => dependency, [dependency]);
    hooks.react.useEffect(() => { order.push("setup" + dependency);
      const captured = dependency; return () => order.push("cleanup" + captured); }, [dependency]);
    return jsxRuntime.jsx("p", { children: String(state) });
  }
  hooks.root = () => jsxRuntime.jsx(Probe, {}); hooks.flush();
  const first = callback, renders = hooks.renders;
  set(NaN); hooks.flush(); assert.equal(hooks.renders, renders);
  set(() => 1); set(n => n + 1); assert.equal(hooks.renders, renders);
  hooks.flush(); assert.equal(allText(hooks.tree), "2"); assert.equal(hooks.renders, renders + 1);
  assert.equal(init, 1); assert.equal(callback, first);
  dependency = 1; hooks.rerender(); assert.notEqual(callback, first);
  assert.deepEqual(order, ["setup0", "cleanup0", "setup1"]);
  hooks.unmount(); assert.deepEqual(order, ["setup0", "cleanup0", "setup1", "cleanup1"]);
  assert.throws(() => hooks.react.useLayoutEffect, /Unsupported React API/);
});
test("generic Provider value propagation and independent consumer fibers", () => {
  const hooks = new Hooks(), context = hooks.react.createContext("default");
  let show = true, value = "first", init = 0;
  function Child() { const [state] = hooks.react.useState(() => ++init);
    return jsxRuntime.jsx("p", { children: hooks.react.useContext(context) + state }); }
  hooks.root = () => jsxRuntime.jsx(context.Provider, { value, children: show ? jsxRuntime.jsx(Child, {}) : null });
  hooks.flush(); assert.equal(allText(hooks.tree), "first1");
  value = "next"; hooks.rerender(); assert.equal(allText(hooks.tree), "next1");
  show = false; hooks.rerender(); show = true; hooks.rerender(); assert.equal(allText(hooks.tree), "next2");
  hooks.root = () => jsxRuntime.jsx(context, { value: "direct", children: jsxRuntime.jsx(Child, {}) });
  hooks.rerender(); assert.equal(allText(hooks.tree), "direct2");
  assert.throws(() => context.Consumer, /Unsupported Context API/);
  function Unknown() { return hooks.react.useContext({}); }
  hooks.root = () => jsxRuntime.jsx(Unknown, {}); assert.throws(() => hooks.rerender(), /Unknown context/);
});
test("frozen 24 copy values unchanged; canonical mapping observed via actual rendered fields", () => {
  const source = ts.createSourceFile(shellPath, shellSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const decl = source.statements.flatMap(s => ts.isVariableStatement(s) ? [...s.declarationList.declarations] : [])
    .find(d => d.name.getText(source) === "COPY");
  const actual = {};
  for (const language of decl.initializer.properties) {
    actual[language.name.text] = {};
    for (const property of language.initializer.properties) actual[language.name.text][property.name.text] = property.initializer.text;
  }
  assert.deepEqual(actual, GOLDEN);
  for (const language of ["en", "zh-CN", "pt-BR", "es"]) {
    for (const ios of [false, true]) {
      const app = new App({ language, ios });
      app.click("mir-pwa-action install"); checkVisible(app, language, ios);
      if (!ios) { app.click("mir-pwa-action fullscreen");
        assert.equal(directText(app.class("mir-pwa-guide-status")), copyFor(language).unavailable);
        assert.equal(app.context().status, "fullscreenUnavailable"); }
    }
  }
});
test("SSR initialization without navigator or effect commit", () => {
  const app = new App({ navigatorPresent: false, commit: false, language: "zh-CN" });
  assert.equal(app.tree, null); assert.equal(app.listeners(), 0); assert.equal(app.clock.timers.size, 0);
});
test("capture without consumer retains prompt; locale and consumer changes duplicate no listeners/timer", async () => {
  const app = new App({ visible: false }), held = app.installEvent();
  assert.equal(held.event.defaultPrevented, true); assert.equal(app.listeners(), 7);
  const scheduled = app.clock.log.filter(r => r[0] === "set").length;
  app.locale("pt-BR"); app.consumer(true); assert.equal(app.listeners(), 7);
  app.click("mir-pwa-action install"); assert.equal(app.promptCalls, 1);
  app.consumer(false); app.locale("zh-CN"); app.consumer(true);
  assert.equal(app.listeners(), 7); assert.equal(app.clock.log.filter(r => r[0] === "set").length, scheduled);
  held.prompt.resolve(); await app.drain(); held.choice.resolve({ outcome: "accepted", platform: "controlled" });
  await app.drain(); assert.equal(app.class("mir-pwa-guide"), undefined);
  assert.equal(directText(app.class("mir-pwa-action install")), GOLDEN.zh.install);
  for (const forbidden of ["language", "locale", "setLanguage", "setLocale", "translator", "copy", "localeStorage"]) assert.ok(!(forbidden in app.context()));
});
for (const outcome of ["accepted", "dismissed"]) test("install both real await boundaries and " + outcome, async () => {
  const app = new App(); app.advance(5000);
  const held = app.installEvent(); app.click("mir-pwa-action install");
  app.locale("zh-CN"); checkVisible(app, "zh-CN"); assert.equal(app.promptCalls, 1);
  held.prompt.resolve(); await app.drain(); app.locale("pt-BR"); checkVisible(app, "pt-BR");
  assert.equal(app.promptCalls, 1);
  held.choice.resolve({ outcome, platform: "controlled" }); await app.drain();
  assert.equal(Boolean(app.class("mir-pwa-guide")), outcome === "dismissed");
  app.click("mir-pwa-action install"); assert.equal(app.promptCalls, 1); checkVisible(app, "pt-BR");
});
test("fullscreen unsupported and deferred rejection render latest copy from semantic status", async () => {
  const app = new App(); app.click("mir-pwa-action fullscreen");
  assert.equal(app.context().status, "fullscreenUnavailable");
  app.locale("zh-CN"); assert.equal(directText(app.class("mir-pwa-guide-status")), GOLDEN.zh.unavailable);
  const request = deferred();
  app.document.documentElement.requestFullscreen = options => {
    assert.equal(options.navigationUI, "hide"); return request.promise;
  };
  app.click("mir-pwa-action fullscreen"); assert.equal(app.class("mir-pwa-guide-status"), undefined);
  app.locale("pt-BR"); request.reject(Error("controlled denial")); await app.drain();
  assert.equal(directText(app.class("mir-pwa-guide-status")), GOLDEN.pt.unavailable);
});
for (const lockOutcome of ["resolve", "reject"]) test("fullscreen request and landscape-lock awaits; advisory " + lockOutcome, async () => {
  const app = new App(), request = deferred(), lock = deferred(); let locks = 0;
  app.advance(5000);
  app.document.documentElement.requestFullscreen = () => request.promise;
  app.orientation.lock = orientation => { assert.equal(orientation, "landscape"); locks++; return lock.promise; };
  app.click("mir-pwa-action fullscreen"); app.locale("zh-CN"); checkVisible(app, "zh-CN");
  request.resolve(); await app.drain(); assert.equal(locks, 1);
  app.locale("pt-BR"); checkVisible(app, "pt-BR");
  app.document.fullscreenElement = app.document.documentElement;
  lock[lockOutcome](lockOutcome === "reject" ? Error("advisory denial") : undefined); await app.drain();
  assert.equal(app.tree, null); assert.equal(app.context().guideOpen, false); assert.equal(app.context().status, null);
  app.document.fullscreenElement = null; app.document.dispatch("fullscreenchange"); app.hooks.flush();
  assert.equal(directText(app.class("mir-pwa-action install")), GOLDEN.pt.install);
});
test("display and appinstalled events during consumer absence use root callbacks", () => {
  const app = new App({ visible: false }); app.advance(5000); app.installEvent();
  app.document.fullscreenElement = app.document.documentElement;
  app.document.dispatch("fullscreenchange"); app.hooks.flush(); assert.equal(app.context().fullscreenActive, true);
  app.mediaChange("(display-mode: standalone)", true);
  app.event("appinstalled"); assert.equal(app.context().guideOpen, false);
  assert.equal(app.document.documentElement.dataset.displayMode, "standalone");
  app.consumer(true); assert.equal(app.tree, null);
  app.mediaChange("(display-mode: standalone)", false);
  app.document.fullscreenElement = null; app.document.dispatch("fullscreenchange"); app.hooks.flush();
  app.click("mir-pwa-action install"); assert.equal(app.promptCalls, 0); assert.ok(app.class("mir-pwa-guide"));
});
test("route gate detaches original seven listeners; retains state and reattaches once", async () => {
  const app = new App(), held = app.installEvent();
  app.route("/other"); assert.equal(app.listeners(), 0); assert.equal(app.tree, null);
  const ignored = app.event("beforeinstallprompt", { prompt: () => { throw Error("offroot captured"); } });
  assert.equal(ignored.defaultPrevented, false); app.event("appinstalled");
  app.route("/"); assert.equal(app.listeners(), 7);
  app.click("mir-pwa-action install"); assert.equal(app.promptCalls, 1);
  held.prompt.resolve(); await app.drain(); held.choice.resolve({ outcome: "accepted" }); await app.drain();
  assert.equal(app.clock.log.filter(r => r[0] === "set" && r[2] === 5000).length, 2);
});
test("4999/5000 hint, 349/350 orientation, portrait/standalone and pointer suppression", () => {
  const app = new App();
  app.advance(4999); assert.equal(app.class("mir-pwa-guide"), undefined);
  app.advance(1); assert.ok(app.class("mir-pwa-guide"));
  app.mediaChange("(orientation: portrait)", true); assert.equal(app.class("mir-pwa-guide"), undefined);
  app.mediaChange("(orientation: portrait)", false);
  app.advance(349); assert.equal(app.class("mir-pwa-guide"), undefined);
  app.advance(1); assert.ok(app.class("mir-pwa-guide"));
  app.mediaChange("(display-mode: standalone)", true); assert.equal(app.tree, null);
  app.mediaChange("(display-mode: standalone)", false);
  app.mediaChange("(pointer: coarse)", false); assert.equal(app.tree, null);
});
test("storage dismissal exact key; denied storage has no added page-lifetime suppression", () => {
  const app = new App(); app.advance(5000); app.click("mir-pwa-guide-dismiss");
  assert.equal(app.storage.get("mir2.pwa.installHintDismissed.v1"), "1");
  app.mediaChange("(orientation: portrait)", true); app.mediaChange("(orientation: portrait)", false);
  app.advance(350); assert.equal(app.class("mir-pwa-guide"), undefined);
  const denied = new App({ deniedStorage: true }); denied.click("mir-pwa-action fullscreen");
  denied.click("mir-pwa-guide-dismiss"); assert.equal(denied.class("mir-pwa-guide"), undefined);
  assert.equal(denied.context().status, null);
  denied.advance(5000); assert.ok(denied.class("mir-pwa-guide"));
  denied.click("mir-pwa-guide-dismiss");
  denied.mediaChange("(orientation: portrait)", true); denied.mediaChange("(orientation: portrait)", false);
  denied.advance(350); assert.ok(denied.class("mir-pwa-guide"));
});
test("cleanup preserves original uncanceled350 timeout and cancels only owned hint", () => {
  const app = new App();
  app.mediaChange("(orientation: portrait)", true); app.mediaChange("(orientation: portrait)", false);
  const orientation = app.clock.log.find(r => r[0] === "set" && r[2] === 350)[1];
  const hint = app.clock.log.find(r => r[0] === "set" && r[2] === 5000)[1];
  app.route("/other");
  assert.ok(app.clock.timers.has(orientation)); assert.ok(!app.clock.timers.has(hint));
  app.advance(350); assert.equal(app.context().guideOpen, true); assert.equal(app.tree, null);
  app.hooks.unmount(); assert.equal(app.listeners(), 0);
});
test("Page/layout actual AST: one owner/mount/required language and persistent root placement", () => {
  const pageSource = fs.readFileSync(path.join(web, "app/page.tsx"), "utf8");
  const layoutSource = fs.readFileSync(path.join(web, "app/layout.tsx"), "utf8");
  const parse = (name, text) => ts.createSourceFile(name, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const page = parse("page.tsx", pageSource), layout = parse("layout.tsx", layoutSource);
  const collect = (root, predicate) => { const result = []; function walk(n) {
    if (predicate(n)) result.push(n); ts.forEachChild(n, walk); } walk(root); return result; };
  const imports = collect(page, n => ts.isImportDeclaration(n) && n.moduleSpecifier.text === "./components/pwa-game-shell");
  assert.equal(imports.length, 1);
  const mounts = collect(page, n => ts.isJsxSelfClosingElement(n) && n.tagName.getText(page) === "PwaGameShell");
  assert.equal(mounts.length, 1);
  assert.equal(mounts[0].attributes.properties.length, 1);
  assert.equal(mounts[0].attributes.properties[0].name.text, "language");
  assert.equal(mounts[0].attributes.properties[0].initializer.expression.getText(page), "language");
  const owner = collect(page, n => ts.isVariableDeclaration(n) && n.name.getText(page) === "[language, setLanguage]");
  assert.equal(owner.length, 1); assert.equal(owner[0].initializer.expression.getText(page), "useState");
  assert.equal(owner[0].initializer.typeArguments[0].getText(page), "Mir2Language");
  let fragment = mounts[0].parent; assert.ok(ts.isJsxFragment(fragment));
  const siblings = fragment.children.filter(n => ts.isJsxSelfClosingElement(n) || ts.isJsxElement(n));
  assert.equal(siblings[0], mounts[0]); assert.equal(siblings[1].tagName.getText(page), "OriginalClientShell");
  let guard = fragment.parent; assert.ok(ts.isConditionalExpression(guard));
  assert.equal(guard.condition.getText(page), "!isClientReady"); assert.equal(guard.whenTrue.kind, ts.SyntaxKind.NullKeyword);
  const capture = collect(layout, n => ts.isJsxElement(n) && n.openingElement.tagName.getText(layout) === "PwaGameShellCapture");
  assert.equal(capture.length, 1);
  const children = capture[0].children.filter(n => ts.isJsxExpression(n));
  assert.equal(children.length, 1); assert.equal(children[0].expression.getText(layout), "children");
  assert.equal(collect(layout, n => ts.isJsxSelfClosingElement(n) && n.tagName.getText(layout) === "PwaGameShell").length, 0);
});
}
