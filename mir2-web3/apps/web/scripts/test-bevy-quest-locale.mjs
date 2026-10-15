import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import ts from "typescript";

const require = createRequire(import.meta.url);
const compilerPath = require.resolve("typescript");
const compilerBytes = readFileSync(compilerPath);
console.log(JSON.stringify({ fixture: "actual-source TypeScript transpile + controlled hook seams",
  compilerPath, compilerRealPath: realpathSync(compilerPath), bytes: compilerBytes.length,
  sha256: createHash("sha256").update(compilerBytes).digest("hex"), version: ts.version }));
const loaded = new Map();
function load(relative, dependencies = {}) {
  const file = new URL(relative, import.meta.url);
  const source = readFileSync(file, "utf8");
  const compiled = ts.transpileModule(source, { reportDiagnostics: true, compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } });
  assert.equal(compiled.diagnostics.filter(row => row.category === ts.DiagnosticCategory.Error).length, 0);
  const module = { exports: {} };
  new Function("require", "exports", "module", compiled.outputText)(name => {
    if (name in dependencies) return dependencies[name];
    if (name.startsWith("./")) return load(`../lib/${name.slice(2)}.ts`);
    throw new Error(`Unexpected runtime dependency ${name}`);
  }, module.exports, module);
  loaded.set(file.pathname, { actualPath: file.pathname, realPath: realpathSync(file), bytes: Buffer.byteLength(source),
    sha256: createHash("sha256").update(source).digest("hex") });
  return module.exports;
}
const host = load("../lib/bevy-quest-ui.ts");
const shape = { generation: 7, revision: 1, openRevision: 1, frame: 1, ready: true,
  error: null, capturesPointer: false, questLogOpen: true };
const runtimeStatus = value => ({ getMir2QuestUiStatus: () => JSON.stringify(value) });
test("exact capability probes independently of initial readiness; malformed v1 status fails closed", () => {
  for (const code of ["en", "zh-CN", "es", "pt-BR"]) {
    const runtime = runtimeStatus({ ...shape, ready: false, questLocaleVersion: 1, language: code });
    assert.equal(host.supportsBevyQuestLocale(runtime), true);
    assert.equal(host.readBevyQuestUiStatus(runtime, 7).language, code);
  }
  for (const invalid of [undefined, null, 4, {}, [], "", "en-US", "EN", "zh", "pt", "es-ES"]) {
    const runtime = runtimeStatus({ ...shape, questLocaleVersion: 1, language: invalid });
    assert.equal(host.supportsBevyQuestLocale(runtime), true);
    assert.equal(host.readBevyQuestUiStatus(runtime, 7), null);
  }
  for (const version of [undefined, null, 0, 2, "1"]) {
    const runtime = runtimeStatus({ ...shape, questLocaleVersion: version });
    assert.equal(host.supportsBevyQuestLocale(runtime), false);
    assert.ok(host.readBevyQuestUiStatus(runtime, 7));
    assert.equal(JSON.stringify(host.stripUnsupportedQuestLocale({ language: "es", revision: 8 }, false)), '{"revision":8}');
  }
  assert.equal(host.readBevyQuestUiStatus(runtimeStatus({ ...shape, language: "en", questLocaleVersion: 1 }), 8), null);
});

function harness(version = 1) {
  let mount, timer, sink, state, calls = 0;
  const refs = []; let index = 0;
  const react = { useRef(initial) { return refs[index++] ??= { current: initial }; },
    useState(initial) { state ??= initial; return [state, next => { state = typeof next === "function" ? next(state) : next; }]; },
    useEffect(fn) { mount = fn; }, useCallback(fn) { return fn; } };
  const hook = load("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host }).useBevyQuestUi;
  let model = { generation: 7, language: "en", inGame: true, hostVisible: true, questLogOpen: true,
    blocksGameplayKeys: false, turnInBlocked: false, profile: "crystal", quests: [],
    presentation: { logicalWidth: 600, logicalHeight: 320, stageCssScale: 1, touch: true },
    completedKnown: true, completedQuestIds: [], dialog: host.projectBevyQuestDialog(null),
    player: { hp: 10, maxHp: 10, level: 1 } };
  let status = { ...shape, ready: false, revision: 0, openRevision: 0,
    ...(version === undefined ? {} : { questLocaleVersion: version }), ...(version === 1 ? { language: "zh-CN" } : {}) };
  const sent = [];
  const runtime = { getMir2QuestUiStatus: () => JSON.stringify(status),
    setMir2QuestUiSnapshot(json) { sent.push(JSON.parse(json)); return true; },
    setMir2QuestUiIntentSink(fn) { sink = fn; }, clearMir2QuestUiIntentSink() {} };
  const oldWindow = globalThis.window;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() {} };
  const owner = hook({ requested: true, runtimeGeneration: 3, runtimeRef: { current: runtime },
    snapshot: () => model, onIntent() { calls++; return { accepted: true }; }, onOpenChange() {} });
  const cleanup = mount();
  return { sent, owner, tick: () => timer(), state: () => state, calls: () => calls,
    setLanguage(language) { model = { ...model, language }; },
    setStatus(update) { status = { ...status, ...update }; },
    acknowledge() { const current = sent.at(-1); status = { ...status, ready: true, frame: status.frame + 1,
      revision: current.revision, openRevision: current.openRevision,
      ...(version === 1 ? { language: current.language ?? "zh-CN" } : {}) }; timer(); },
    click: () => JSON.parse(sink(JSON.stringify({ generation: 7, type: "acceptQuest" }))),
    close() { cleanup?.(); if (oldWindow === undefined) delete globalThis.window; else globalThis.window = oldWindow; } };
}
test("real hook requires live language and its newest revision before polling or input acknowledgement", () => {
  const h = harness();
  try {
    assert.equal(h.sent.at(-1).language, "en");
    assert.equal(h.state().ready, false);
    h.acknowledge(); assert.equal(h.state().ready, true); assert.equal(h.click().accepted, true);
    const enRevision = h.sent.at(-1).revision, openRevision = h.sent.at(-1).openRevision;
    h.setLanguage("pt-BR");
    assert.equal(h.click().accepted, false, "live change precedes the next polling interval");
    h.owner.refresh();
    const ptRevision = h.sent.at(-1).revision;
    assert.ok(ptRevision > enRevision); assert.equal(h.state().ready, false);
    assert.equal(h.sent.at(-1).openRevision, openRevision);
    assert.equal(h.click().accepted, false, "old English acknowledgement does not accept Portuguese");
    h.setLanguage("en");
    assert.equal(h.click().accepted, false, "return to same language cannot revive its old revision");
    h.owner.refresh();
    assert.ok(h.sent.at(-1).revision > ptRevision);
    assert.equal(h.state().ready, false); assert.equal(h.click().accepted, false);
    h.setStatus({ revision: h.sent.at(-1).revision, language: "pt-BR", frame: 10 }); h.tick();
    assert.equal(h.state().ready, false); assert.equal(h.click().accepted, false);
    h.acknowledge(); assert.equal(h.state().ready, true); assert.equal(h.click().accepted, true);
    assert.equal(h.calls(), 2);
    for (const code of ["es", "zh-CN", "pt-BR", "en"]) {
      h.setLanguage(code); h.owner.refresh(); assert.equal(h.state().ready, false);
      assert.equal(h.sent.at(-1).openRevision, openRevision);
      h.acknowledge(); assert.equal(h.state().ready, true);
    }
    h.setStatus({ language: null, frame: 100 }); h.tick();
    assert.equal(h.state().ready, false); assert.equal(h.click().accepted, false);
  } finally { h.close(); }
});
test("older and unknown capability keep omitted DTO and their existing readiness shape", () => {
  for (const version of [0, 2, "1", null]) {
    const h = harness(version);
    try { assert.equal(Object.hasOwn(h.sent.at(-1), "language"), false);
      h.acknowledge(); assert.equal(h.state().ready, true);
      h.setLanguage("es"); h.owner.refresh();
      assert.equal(Object.hasOwn(h.sent.at(-1), "language"), false); assert.equal(h.click().accepted, true);
    } finally { h.close(); }
  }
});
test("actual used source bindings are disclosed", () => {
  assert.ok([...loaded.keys()].some(p => p.endsWith("/use-bevy-quest-ui.ts")));
  console.log(JSON.stringify({ actualUsedSources: [...loaded.values()] }));
});
