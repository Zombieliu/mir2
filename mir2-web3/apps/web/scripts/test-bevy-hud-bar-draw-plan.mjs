import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../lib/bevy-hud-bar-draw-plan.ts", import.meta.url), "utf8");
const code = ts.transpileModule(source, { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
} }).outputText;
const module = { exports: {} };
new Function("exports", "module", "require", code)(module.exports, module, () => {
  throw new Error("Unexpected runtime dependency");
});
const draw = module.exports;
function loadTs(relative, dependencies = {}) {
  const compiled = ts.transpileModule(readFileSync(new URL(relative, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const loaded = { exports: {} };
  new Function("exports", "module", "require", compiled)(loaded.exports, loaded, name => {
    if (name in dependencies) return dependencies[name];
    throw new Error(`Unexpected dependency ${name}`);
  });
  return loaded.exports;
}
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 0.5, touch: false };
const expSlot = { left: 9, top: 759, width: 1004, height: 8 };
const weightSlot = { left: 919, top: 721, width: 76, height: 12 };
const makeSnapshot = (commit, changes = {}) => ({
  generation: 5, revision: 10, inGame: true, hostVisible: true, presentation,
  dialog: { isOpen: false, hasInput: false },
  player: { experience: 50, maxExperience: 100, currentWeight: 50, maxWeight: 100 },
  experienceBarSlot: expSlot, weightBarSlot: weightSlot, hudBarPlan: commit, ...changes,
});
const sprite = (name, token, sequence, image, current = 50, maximum = 100, slot = name === "experience" ? expSlot : weightSlot) => {
  const width = name === "experience" ? 500 : 37;
  return { token, sequence, state: "draw", current, maximum, slot, image,
    source: { left: 0, top: 0, width, height: slot.height },
    destination: { left: slot.left, top: slot.top, width, height: slot.height } };
};
const plan = (commit, changes = {}) => ({ version: 1, generation: 5, revision: 9,
  lifetime: commit.lifetime,
  experience: sprite("experience", commit.experienceToken, commit.experienceSequence, "original-ui/Prguse/8.png"),
  weight: sprite("weight", commit.weightToken, commit.weightSequence, "original-ui/Prguse/76.png"),
  ...changes });
const capability = value => ({ getMir2HudBarDrawPlanVersion: () => 1,
  getMir2HudBarDrawPlan: () => JSON.stringify(value) });
const deferred = () => { let resolve; let reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject }; };
const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
function canvas() {
  const calls = [];
  const context = { calls, clearRect: (...args) => calls.push(["clear", ...args]),
    drawImage: (...args) => calls.push(["draw", ...args]), imageSmoothingEnabled: true,
    globalAlpha: 0, globalCompositeOperation: "copy" };
  return { width: 1004, height: 8, style: {}, context, getContext: kind => kind === "2d" ? context : null };
}
const anchor = (current, maximum, name) => ({ getAttribute: key => ({
  [name === "experience" ? "data-experience" : "data-current-weight"]: String(current),
  [name === "experience" ? "data-max-experience" : "data-max-weight"]: String(maximum),
})[key] ?? null });
const update = (lifetime, exp, weight, expCanvas, weightCanvas, overrides = {}) => ({
  lifetime, presentation,
  readLivePlans: overrides.readLivePlans ?? (() => lifetime ? { lifetime, experience: exp, weight } : null),
  experience: { canvas: expCanvas, anchor: anchor(exp?.current ?? 50, exp?.maximum ?? 100, "experience"),
    current: exp?.current ?? 50, maximum: exp?.maximum ?? 100, slot: exp?.slot ?? expSlot,
    readSlot: () => overrides.expMeasured ?? exp?.slot ?? expSlot, plan: exp },
  weight: { canvas: weightCanvas, anchor: anchor(weight?.current ?? 50, weight?.maximum ?? 100, "weight"),
    current: weight?.current ?? 50, maximum: weight?.maximum ?? 100, slot: weight?.slot ?? weightSlot,
    readSlot: () => overrides.weightMeasured ?? weight?.slot ?? weightSlot, plan: weight },
});

test("exact capability, bounded records, and independent applied tokens", () => {
  assert.equal(draw.supportsHudBarDrawPlan(null), false);
  assert.equal(draw.supportsHudBarDrawPlan({ getMir2HudBarDrawPlanVersion: () => "1",
    getMir2HudBarDrawPlan: () => "{}" }), false);
  assert.equal(draw.supportsHudBarDrawPlan({ getMir2HudBarDrawPlanVersion: () => 1 }), false);
  const clock = draw.createHudBarVisualClock();
  const first = draw.nextHudBarPlanCommit(clock, 3, "1000", makeSnapshot(null));
  assert.equal(first.experienceSequence, 1);
  assert.equal(first.weightSequence, 1);
  const movement = draw.nextHudBarPlanCommit(clock, 3, "1000", makeSnapshot(null, { revision: 99 }));
  assert.deepEqual(movement, first, "movement-only revision does not change visual identity");
  const weightChanged = draw.nextHudBarPlanCommit(clock, 3, "1000", makeSnapshot(null, {
    player: { experience: 50, maxExperience: 100, currentWeight: 51, maxWeight: 100 },
  }));
  assert.equal(weightChanged.experienceToken, first.experienceToken);
  assert.notEqual(weightChanged.weightToken, first.weightToken);
  const slotChanged = draw.nextHudBarPlanCommit(clock, 3, "1000", makeSnapshot(null, {
    experienceBarSlot: { ...expSlot, top: 758 },
    player: { experience: 50, maxExperience: 100, currentWeight: 51, maxWeight: 100 },
  }));
  assert.notEqual(slotChanged.experienceToken, first.experienceToken);
  assert.equal(slotChanged.weightToken, weightChanged.weightToken);
  assert.equal(draw.nextHudBarPlanCommit(clock, 3, "1001", makeSnapshot(null)).experienceSequence, 1);
  assert.equal(draw.nextHudBarPlanCommit(clock, 3, "p1", makeSnapshot(null)), null);
  const raw = plan(first);
  const read = draw.readHudBarDrawPlan(capability(raw));
  assert.equal(read.version, 1);
  assert.equal(draw.currentHudBarPlans(read, makeSnapshot(first), 10, true).experience.token, first.experienceToken);
  assert.equal(draw.currentHudBarPlans(read, makeSnapshot(weightChanged, {
    player: { experience: 50, maxExperience: 100, currentWeight: 51, maxWeight: 100 },
  }), 10, true).experience.token, first.experienceToken);
  assert.equal(draw.currentHudBarPlans(read, makeSnapshot(weightChanged, {
    player: { experience: 50, maxExperience: 100, currentWeight: 51, maxWeight: 100 },
  }), 10, true).weight, null);
  assert.equal(draw.currentHudBarPlans(read, makeSnapshot(first), 10, false), null);
  assert.equal(draw.currentHudBarPlans(read, makeSnapshot(first, { generation: 6 }), 10, true), null);
  assert.equal(draw.currentHudBarPlans(read, makeSnapshot(first), 8, true), null);
  assert.equal(draw.readHudBarDrawPlan(capability({ ...raw, lifetime: "é" })), null);
  assert.equal(draw.readHudBarDrawPlan(capability({ ...raw, version: 2 })), null);
  assert.equal(draw.readHudBarDrawPlan(capability({ ...raw, experience: { ...raw.experience,
    source: { ...raw.experience.source, width: Infinity } } })).experience, null);
});

test("known zero and floored-positive empty own a cleared native backing without an asset", () => {
  const owners = [];
  const loads = [];
  const controller = new draw.HudBarCanvasController((name, owns) => owners.push([name, owns]),
    path => { loads.push(path); return Promise.reject(new Error("should not load")); });
  const expCanvas = canvas(); const weightCanvas = canvas(); weightCanvas.width = 76; weightCanvas.height = 12;
  const emptyExp = { ...sprite("experience", "e1", 1, null, 1, 10000), state: "known-empty",
    image: null, source: { left: 0, top: 0, width: 0, height: 8 },
    destination: { left: 9, top: 759, width: 0, height: 8 } };
  const emptyWeight = { ...sprite("weight", "w1", 1, null, 0, 1), state: "known-empty",
    image: null, source: { left: 0, top: 0, width: 0, height: 12 },
    destination: { left: 919, top: 721, width: 0, height: 12 } };
  controller.update(update("r3-g5-p1000", emptyExp, emptyWeight, expCanvas, weightCanvas));
  assert.deepEqual(owners, [["experience", true], ["weight", true]]);
  assert.equal(expCanvas.context.calls.some(row => row[0] === "draw"), false);
  assert.equal(weightCanvas.context.calls.some(row => row[0] === "draw"), false);
  assert.deepEqual(loads, []);
  controller.dispose();
  assert.equal(expCanvas.style.visibility, "hidden");
});

test("a Canvas2D acquisition exception withdraws only the affected output", () => {
  const owners = [];
  const controller = new draw.HudBarCanvasController((name, owns) => owners.push([name, owns]));
  const expCanvas = canvas(); const weightCanvas = canvas();
  expCanvas.getContext = () => { throw new Error("context lost"); };
  const empty = { ...sprite("weight", "w1", 1, null, 0, 1), state: "known-empty",
    image: null, source: { left: 0, top: 0, width: 0, height: 12 },
    destination: { left: 919, top: 721, width: 0, height: 12 } };
  controller.update(update("r3-g5-p1000", sprite("experience", "e1", 1,
    "original-ui/Prguse/8.png"), empty, expCanvas, weightCanvas));
  assert.equal(expCanvas.style.visibility, "hidden");
  assert.equal(weightCanvas.style.visibility, "visible");
  assert.deepEqual(owners, [["weight", true]]);
  controller.dispose();
});

test("precommit withdrawal retires an in-flight decode until a new own token arrives", async () => {
  const pending = deferred(); const owners = [];
  const controller = new draw.HudBarCanvasController((name, owns) => owners.push([name, owns]),
    () => pending.promise);
  const expCanvas = canvas(); const weightCanvas = canvas();
  const first = sprite("experience", "e1", 1, "original-ui/Prguse/8.png");
  controller.update(update("r3-g5-p1000", first, null, expCanvas, weightCanvas));
  controller.withdraw("experience");
  pending.resolve({}); await flush();
  assert.equal(expCanvas.style.visibility, "hidden", "old decode cannot retake before the DOM commits");
  controller.update(update("r3-g5-p1000", first, null, expCanvas, weightCanvas));
  assert.equal(expCanvas.style.visibility, "hidden", "movement-only refresh cannot revive the retired token");
  const next = { ...first, token: "e2", sequence: 2 };
  controller.update(update("r3-g5-p1000", next, null, expCanvas, weightCanvas));
  assert.equal(expCanvas.style.visibility, "visible", "a new validated own token can reuse the decoded image");
  assert.deepEqual(owners, [["experience", true]]);
  controller.dispose();
});

test("actual hook sends raw fields only behind exact plan capability and keeps sibling plan through movement", () => {
  const priorWindow = globalThis.window; const priorPerformance = globalThis.performance;
  let effect, state, timer, now = 100;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
  globalThis.performance = { now: () => now };
  try {
    const refs = []; let index = 0;
    const react = { useRef(value) { return refs[index++] ??= { current: value }; },
      useCallback(fn) { return fn; }, useEffect(fn) { effect = fn; },
      useState(value) { state ??= value; return [state, next => {
        state = typeof next === "function" ? next(state) : next;
      }]; } };
    const host = loadTs("../lib/bevy-quest-ui.ts", { "./quest-action-policy": { questEndpoint: value => value } });
    const hook = loadTs("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
      "./bevy-hp-orb": loadTs("../lib/bevy-hp-orb.ts"),
      "./bevy-experience-bar": loadTs("../lib/bevy-experience-bar.ts"),
      "./bevy-weight-bar": loadTs("../lib/bevy-weight-bar.ts"),
      "./bevy-hud-bar-draw-plan": draw });
    let live = { ...makeSnapshot(null), hudBarPlayerObjectId: "1000", hudBarRuntimeLifetime: 3,
      questLogOpen: false, blocksGameplayKeys: false, turnInBlocked: false, profile: "crystal",
      quests: [], completedKnown: false, completedQuestIds: [] };
    const sent = []; let appliedPlan = null;
    let hostStatus = { frame: 1, ready: false, error: null, capturesPointer: false,
      questLogOpen: false, generation: 5, revision: 0, openRevision: 0,
      experienceBar: { supported: false, ready: false }, weightBar: { supported: false, ready: false } };
    const runtime = { setMir2QuestUiSnapshot(json) { sent.push(JSON.parse(json)); return true; },
      setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {},
      getMir2QuestUiStatus: () => JSON.stringify(hostStatus),
      getMir2HudBarDrawPlanVersion: () => 1,
      getMir2HudBarDrawPlan: () => JSON.stringify(appliedPlan) };
    const owner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 3,
      runtimeRef: { current: runtime }, snapshot: () => live,
      onIntent: () => ({ accepted: false }), onOpenChange() {} });
    const cleanup = effect();
    assert.equal(sent.length, 1);
    assert.equal(sent[0].player.experience, 50);
    assert.equal(sent[0].player.currentWeight, 50);
    assert.deepEqual(sent[0].experienceBarSlot, expSlot);
    assert.deepEqual(sent[0].weightBarSlot, weightSlot);
    assert.equal(sent[0].hudBarPlan.lifetime, "r3-g5-p1000");
    assert.equal("hudBarPlayerObjectId" in sent[0], false);
    appliedPlan = { ...plan(sent[0].hudBarPlan), revision: sent[0].revision };
    hostStatus = { ...hostStatus, frame: 2, revision: sent[0].revision };
    owner.refresh();
    assert.equal(state.hudBarPlans?.experience?.token, sent[0].hudBarPlan.experienceToken);
    assert.equal(state.hudBarPlans?.weight?.token, sent[0].hudBarPlan.weightToken);
    live = { ...live, player: { ...live.player, hp: 51 } };
    owner.refresh();
    assert.equal(sent.at(-1).hudBarPlan.experienceToken, sent[0].hudBarPlan.experienceToken);
    assert.equal(sent.at(-1).hudBarPlan.weightToken, sent[0].hudBarPlan.weightToken);
    assert.equal(state.hudBarPlans?.experience?.token, sent[0].hudBarPlan.experienceToken);
    live = { ...live, player: { ...live.player, currentWeight: 51 } };
    owner.refresh();
    assert.notEqual(sent.at(-1).hudBarPlan.weightToken, sent[0].hudBarPlan.weightToken);
    assert.equal(state.hudBarPlans?.experience?.token, sent[0].hudBarPlan.experienceToken);
    assert.equal(state.hudBarPlans?.weight, null);
    now += 2001; owner.refresh();
    assert.equal(state.hudBarPlans, null, "stopped host heartbeat withdraws both outputs");
    cleanup();
    assert.equal(timer, null);
  } finally { globalThis.window = priorWindow; globalThis.performance = priorPerformance; }
});

test("decode, sibling change, multiple moves, failure reuse and stale generation cleanup", async () => {
  const pending = new Map(); const attempts = []; const owners = [];
  const controller = new draw.HudBarCanvasController((name, owns) => owners.push([name, owns]),
    (path, signal) => { const job = deferred(); pending.set(path, { ...job, signal }); attempts.push(path); return job.promise; });
  const expCanvas = canvas(); const weightCanvas = canvas(); weightCanvas.width = 76; weightCanvas.height = 12;
  const commit = { lifetime: "r3-g5-p1000", experienceToken: "e1", experienceSequence: 1,
    weightToken: "w1", weightSequence: 1 };
  const first = plan(commit);
  controller.update(update(commit.lifetime, first.experience, first.weight, expCanvas, weightCanvas));
  assert.deepEqual(attempts, ["original-ui/Prguse/8.png", "original-ui/Prguse/76.png"]);
  assert.equal(expCanvas.style.visibility, "hidden");
  const expImage = { close: () => owners.push(["exp-image", "closed"]) };
  pending.get("original-ui/Prguse/8.png").resolve(expImage); await flush();
  assert.equal(expCanvas.style.visibility, "visible");
  assert.equal(expCanvas.context.calls.filter(row => row[0] === "draw").length, 1);
  const weightImage = { close: () => owners.push(["weight-image", "closed"]) };
  pending.get("original-ui/Prguse/76.png").resolve(weightImage); await flush();
  assert.equal(weightCanvas.style.visibility, "visible");
  const expDraws = expCanvas.context.calls.filter(row => row[0] === "draw").length;
  controller.update(update(commit.lifetime, first.experience, first.weight, expCanvas, weightCanvas));
  assert.equal(expCanvas.context.calls.filter(row => row[0] === "draw").length, expDraws,
    "movement-only host refresh does not repaint or decode the same visual token");
  controller.withdraw("experience");
  assert.equal(expCanvas.style.visibility, "hidden", "pre-commit raw edge hides old pixels synchronously");
  assert.equal(weightCanvas.style.visibility, "visible", "sibling remains independently owned");
  const rawChanged = update(commit.lifetime, first.experience, first.weight, expCanvas, weightCanvas);
  rawChanged.experience.anchor = anchor(51, 100, "experience");
  controller.update(rawChanged);
  assert.equal(expCanvas.style.visibility, "hidden", "old plan cannot retake against changed live DOM pair");
  const mid = sprite("weight", "w2", 2, "original-ui/UI_32bit/473.png", 60);
  const recoveredExp = { ...first.experience, token: "e2", sequence: 2 };
  controller.update(update(commit.lifetime, recoveredExp, mid, expCanvas, weightCanvas));
  assert.equal(expCanvas.style.visibility, "visible", "weight change keeps EXP independently owned");
  assert.equal(weightCanvas.style.visibility, "hidden");
  pending.get("original-ui/UI_32bit/473.png").reject(new Error("missing")); await flush();
  controller.update(update(commit.lifetime, first.experience, first.weight, expCanvas, weightCanvas));
  controller.update(update(commit.lifetime, first.experience, mid, expCanvas, weightCanvas));
  assert.equal(attempts.filter(path => path.endsWith("473.png")).length, 1, "failed path is not polled/retried");
  const moved1 = sprite("experience", "e3", 3, "original-ui/Prguse/8.png", 50, 100,
    { ...expSlot, top: 758 });
  controller.update(update(commit.lifetime, moved1, mid, expCanvas, weightCanvas));
  assert.equal(expCanvas.style.top, "758px");
  const moved2 = sprite("experience", "e4", 4, "original-ui/Prguse/8.png", 50, 100,
    { ...expSlot, top: 757 });
  controller.update(update(commit.lifetime, moved2, mid, expCanvas, weightCanvas));
  assert.equal(expCanvas.style.top, "757px");
  assert.equal(attempts.filter(path => path.endsWith("8.png")).length, 1);
  controller.update(update(commit.lifetime, moved2, mid, expCanvas, weightCanvas,
    { expMeasured: { ...expSlot, top: 756 } }));
  assert.equal(expCanvas.style.visibility, "hidden", "live geometry mismatch withdraws immediately");
  const late = sprite("weight", "w3", 3, "original-ui/UI_32bit/472.png", 90);
  controller.update(update(commit.lifetime, moved2, late, expCanvas, weightCanvas));
  const lateJob = pending.get("original-ui/UI_32bit/472.png");
  controller.update(update("r4-g6-p1000", null, null, expCanvas, weightCanvas));
  assert.equal(lateJob.signal.aborted, true);
  let lateClosed = false; lateJob.resolve({ close: () => { lateClosed = true; } }); await flush();
  assert.equal(lateClosed, true);
  assert.equal(weightCanvas.style.visibility, "hidden");
  controller.dispose();
});

function astSource(relative, kind) {
  const text = readFileSync(new URL(relative, import.meta.url), "utf8");
  return ts.createSourceFile(relative, text, ts.ScriptTarget.Latest, true, kind);
}
function walkAst(node, match) {
  if (match(node)) return node;
  return ts.forEachChild(node, child => walkAst(child, match));
}
function actualShellCanvasInput(scope) {
  const file = astSource("../app/original-client-shell.tsx", ts.ScriptKind.TSX);
  const call = walkAst(file, node => ts.isCallExpression(node)
    && node.expression.getText(file) === "hudBarControllerRef.current?.update");
  assert.ok(call && call.arguments.length === 1, "production shell controller update is present");
  const code = ts.transpileModule(`const actual = ${call.arguments[0].getText(file)};`, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return new Function(...Object.keys(scope), `${code}\nreturn actual;`)(...Object.values(scope));
}
function actualPacketClause(name, scope) {
  const file = astSource("../app/page.tsx", ts.ScriptKind.TSX);
  const helper = walkAst(file, node => ts.isFunctionDeclaration(node)
    && node.name?.text === "withdrawHudBarDrawPlanOutputs");
  const clause = walkAst(file, node => ts.isCaseClause(node)
    && ts.isStringLiteral(node.expression) && node.expression.text === name);
  assert.ok(helper && clause, `production ${name} clause and withdrawal dispatcher exist`);
  const text = `${helper.getText(file)}\nfunction actualPacketClause() { switch (${JSON.stringify(name)}) {\n${clause.getText(file)}\n} }`;
  const code = ts.transpileModule(text, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText;
  return new Function(...Object.keys(scope), `${code}\nactualPacketClause();`)(...Object.values(scope));
}
function emptyPlan(name) {
  const slot = name === "experience" ? expSlot : weightSlot;
  return { token: name === "experience" ? "e1" : "w1", sequence: 1, state: "known-empty",
    current: 0, maximum: 1, slot, image: null,
    source: { left: 0, top: 0, width: 0, height: slot.height },
    destination: { left: slot.left, top: slot.top, width: 0, height: slot.height } };
}

test("production shell update retains same-identity successful and failed asset attempts across a modal gap", async () => {
  for (const fail of [false, true]) {
    let attempts = 0;
    const controller = new draw.HudBarCanvasController(() => {}, async () => {
      attempts++;
      if (fail) throw new Error("fixture image failure");
      return {};
    });
    const expCanvas = canvas(); const weightCanvas = canvas();
    const exp = sprite("experience", "e1", 1, "original-ui/Prguse/8.png");
    const identity = "r3-g5-p1000";
    const scope = { imageMayOwn: true, bevyHudSourceGeometry: null, bevyHudBarIdentity: identity,
      bevyHudBarPlans: { lifetime: identity, experience: exp, weight: null },
      bevyHudBarReadLivePlans: () => ({ lifetime: identity, experience: exp, weight: null }),
      presentation, world: { playerExperience: 50, playerMaxExperience: 100,
        currentWeight: null, maxWeight: null }, frame: {},
      experienceAnchor: anchor(50, 100, "experience"), weightAnchor: null,
      experienceSlot: expSlot, weightSlot: null,
      experienceDrawCanvasRef: { current: expCanvas }, weightDrawCanvasRef: { current: weightCanvas },
      readBevyExperienceBarSlot: () => expSlot, readBevyWeightBarSlot: () => null };
    controller.update(actualShellCanvasInput(scope)); await flush();
    controller.update(actualShellCanvasInput({ ...scope, imageMayOwn: false, bevyHudBarPlans: null }));
    assert.equal(expCanvas.style.visibility, "hidden");
    controller.update(actualShellCanvasInput(scope)); await flush();
    assert.equal(attempts, 1, `${fail ? "failed" : "successful"} path record survives real shell modal wiring`);
    controller.dispose();
  }
});

test("production shell identity edges abort old loads and reject their late result", async () => {
  for (const nextIdentity of ["r4-g5-p1000", "r3-g6-p1000", "r3-g5-p1001"]) {
    const pending = deferred(); let attempts = 0; let oldClosed = false; let signal;
    const controller = new draw.HudBarCanvasController(() => {}, (_path, currentSignal) => {
      attempts++; signal = currentSignal; return pending.promise;
    });
    const expCanvas = canvas(); const weightCanvas = canvas();
    const exp = sprite("experience", "e1", 1, "original-ui/Prguse/8.png");
    const first = "r3-g5-p1000";
    const scope = { imageMayOwn: true, bevyHudSourceGeometry: null, bevyHudBarIdentity: first,
      bevyHudBarPlans: { lifetime: first, experience: exp, weight: null },
      bevyHudBarReadLivePlans: () => ({ lifetime: first, experience: exp, weight: null }),
      presentation, world: { playerExperience: 50, playerMaxExperience: 100,
        currentWeight: null, maxWeight: null }, frame: {},
      experienceAnchor: anchor(50, 100, "experience"), weightAnchor: null,
      experienceSlot: expSlot, weightSlot: null,
      experienceDrawCanvasRef: { current: expCanvas }, weightDrawCanvasRef: { current: weightCanvas },
      readBevyExperienceBarSlot: () => expSlot, readBevyWeightBarSlot: () => null };
    controller.update(actualShellCanvasInput(scope));
    controller.update(actualShellCanvasInput({ ...scope, bevyHudBarIdentity: nextIdentity,
      bevyHudBarPlans: null }));
    assert.equal(signal?.aborted, true, `identity ${nextIdentity} aborts old generation lease`);
    pending.resolve({ close() { oldClosed = true; } }); await flush();
    assert.equal(oldClosed, true);
    assert.equal(expCanvas.style.visibility, "hidden");
    assert.equal(attempts, 1);
    controller.dispose();
  }
});

test("production source geometry needs a current full HUD plan and yields raw weight differences to its Rust sprite fallback", () => {
  const exp = emptyPlan("experience"), weight = emptyPlan("weight"), identity = "r3-g5-p1000";
  const fullPlan = { experienceSprite: { current: 0, maximum: 1 }, weightSprite: { current: 4294967295, maximum: 80 } };
  const scope = { imageMayOwn: true, bevyHudSourceGeometry: { experienceBar: expSlot, weightBar: weightSlot }, bevyHudBarIdentity: identity,
    bevyHudBarPlans: { lifetime: identity, experience: exp, weight }, readBevyHudStatus: () => ({ plan: fullPlan }),
    bevyHudBarReadLivePlans: () => ({ lifetime: identity, experience: exp, weight }), presentation,
    world: { playerExperience: 0, playerMaxExperience: 1, currentWeight: 0, maxWeight: 1 }, frame: {}, experienceAnchor: null, weightAnchor: null,
    experienceSlot: expSlot, weightSlot, experienceDrawCanvasRef: { current: canvas() }, weightDrawCanvasRef: { current: canvas() },
    readBevyExperienceBarSlot() { throw Error("must not read DOM"); }, readBevyWeightBarSlot() { throw Error("must not read DOM"); } };
  const actual = actualShellCanvasInput(scope); assert.equal(actual.experience.sourceGeometry, true); assert.deepEqual(actual.experience.readSlot(), expSlot);
  assert.equal(actual.experience.plan, exp); assert.equal(actual.weight.plan, null, "u32 authoritative weight is not painted by a mismatched u16 plan");
  assert.equal(actualShellCanvasInput({ ...scope, readBevyHudStatus: () => null }).experience.plan, null, "static geometry has no value authority");
});

test("raw-edge retirement survives a same-lifetime null-plan gap and pending old decode", async () => {
  const pending = deferred();
  const controller = new draw.HudBarCanvasController(() => {}, () => pending.promise);
  const expCanvas = canvas(); const weightCanvas = canvas();
  const exp = sprite("experience", "e1", 1, "original-ui/Prguse/8.png");
  controller.update(update("r3-g5-p1000", exp, null, expCanvas, weightCanvas));
  controller.withdraw("experience");
  controller.update(update("r3-g5-p1000", null, null, expCanvas, weightCanvas));
  controller.update(update("r3-g5-p1000", exp, null, expCanvas, weightCanvas));
  pending.resolve({}); await flush();
  assert.equal(expCanvas.style.visibility, "hidden", "old token remains retired across plan-null gap");
  controller.dispose();
});

test("actual progression packet clauses withdraw synchronously, only for affected bars", () => {
  const experience = loadTs("../lib/bevy-experience-bar.ts");
  const priorWindow = globalThis.window; const priorEvent = globalThis.CustomEvent;
  try {
    globalThis.CustomEvent = class { constructor(type, init) { this.type = type; this.detail = init.detail; } };
    for (const packet of ["UserInformation", "GainExperience", "LevelChanged"]) {
      const observed = [];
      const controller = new draw.HudBarCanvasController((name, owns) => observed.push([name, owns]));
      const expCanvas = canvas(); const weightCanvas = canvas();
      controller.update(update("r3-g5-p1000", emptyPlan("experience"), emptyPlan("weight"), expCanvas, weightCanvas));
      const worldRef = { current: { playerObjectId: "1000", playerExperience: 0, playerMaxExperience: 1 } };
      const bevyQuestGenerationRef = { current: 5 };
      const experienceAuthorityRef = { current: experience.experienceAuthorityFromPacket(5, "1000", 0, 1) };
      const oldAuthority = experienceAuthorityRef.current;
      const edges = [];
      globalThis.window = { dispatchEvent(event) {
        edges.push({ detail: event.detail, authority: experienceAuthorityRef.current,
          world: worldRef.current });
        if (event.detail.experience) controller.withdraw("experience");
        if (event.detail.weight) controller.withdraw("weight");
      } };
      const scope = { payload: packet === "UserInformation" ? { objectId: "1001", experience: 0,
          maxExperience: 1, location: { x: 1, y: 2 } } : packet === "GainExperience"
          ? { amount: 1 } : { experience: 0, maxExperience: 2, level: 2 },
        worldRef, bevyQuestGenerationRef, experienceAuthorityRef,
        experienceAuthorityFromPacket: experience.experienceAuthorityFromPacket,
        advanceExperienceAuthority: experience.advanceExperienceAuthority,
        stringifyId: value => String(value), numberOrZero: value => typeof value === "number" ? value : 0,
        numberOrUndefined: value => typeof value === "number" ? value : undefined,
        stringOrNull: value => typeof value === "string" ? value : null,
        stringOrFallback: (value, fallback) => typeof value === "string" ? value : fallback,
        t: () => "self", Date, lastSelfMovementAckRef: { current: null },
        lastSelfPacketMovementAckRef: { current: null },
        updateWorld: () => {
          assert.equal(expCanvas.style.visibility, "hidden", `${packet} retired EXP before world scheduling`);
          assert.equal(weightCanvas.style.visibility, packet === "UserInformation" ? "hidden" : "visible");
        }, resetBevyMovementShadow() {}, screenRef: { current: "login" }, setScreen() {},
        completeGatewayReconnect() {}, markMir2CacheMilestone() {}, appendCrystalGameEntryChat() {},
        gameBusRef: { current: { emit() {} } }, appendLog() {} };
      actualPacketClause(packet, scope);
      assert.equal(edges.length, 1, `${packet} uses production synchronous event path`);
      assert.equal(edges[0].authority, oldAuthority, `${packet} dispatches before authority assignment`);
      assert.equal(edges[0].world, worldRef.current, `${packet} dispatches before world scheduling`);
      assert.equal(edges[0].detail.experience, true);
      assert.equal(edges[0].detail.weight, packet === "UserInformation");
      controller.dispose();
    }
  } finally { globalThis.window = priorWindow; globalThis.CustomEvent = priorEvent; }
});

test("actual direct packet clauses preserve unchanged bars and retire partial or shrinking EXP", () => {
  const experience = loadTs("../lib/bevy-experience-bar.ts");
  const priorWindow = globalThis.window; const priorEvent = globalThis.CustomEvent;
  globalThis.CustomEvent = class { constructor(type, init) { this.type = type; this.detail = init.detail; } };
  try {
    const cases = [
      ["UserInformation", { objectId: "1000", experience: 0, maxExperience: 1, location: { x: 1, y: 2 } }, 0],
      ["UserInformation", { objectId: "1000", experience: 0, location: { x: 1, y: 2 } }, 1],
      ["GainExperience", { amount: 0 }, 1],
      ["GainExperience", { amount: 0 }, 0, true],
      ["GainExperience", { amount: 1 }, 1],
      ["LevelChanged", { experience: 0, maxExperience: 1, level: 1 }, 0],
      ["LevelChanged", { experience: 0, maxExperience: 2, level: 2 }, 1],
    ];
    for (const [packet, payload, expected, unknownAuthority] of cases) {
      const edges = [];
      globalThis.window = { dispatchEvent(event) { edges.push(event.detail); } };
      const worldRef = { current: { playerObjectId: "1000", playerExperience: 0, playerMaxExperience: 1 } };
      const experienceAuthorityRef = { current: unknownAuthority ? null
        : experience.experienceAuthorityFromPacket(5, "1000", 0, 1) };
      actualPacketClause(packet, { payload, worldRef, experienceAuthorityRef,
        bevyQuestGenerationRef: { current: 5 },
        experienceAuthorityFromPacket: experience.experienceAuthorityFromPacket,
        advanceExperienceAuthority: experience.advanceExperienceAuthority,
        stringifyId: value => String(value), numberOrZero: value => typeof value === "number" ? value : 0,
        numberOrUndefined: value => typeof value === "number" ? value : undefined,
        stringOrNull: value => typeof value === "string" ? value : null,
        stringOrFallback: (value, fallback) => typeof value === "string" ? value : fallback,
        t: () => "self", Date, lastSelfMovementAckRef: { current: null },
        lastSelfPacketMovementAckRef: { current: null }, updateWorld() {},
        resetBevyMovementShadow() {}, screenRef: { current: "login" }, setScreen() {},
        completeGatewayReconnect() {}, markMir2CacheMilestone() {}, appendCrystalGameEntryChat() {},
        gameBusRef: { current: { emit() {} } }, appendLog() {} });
      assert.equal(edges.length, expected, `${packet} ${JSON.stringify(payload)} edge count`);
      if (expected) assert.deepEqual(edges[0], { experience: true, weight: false });
    }
  } finally { globalThis.window = priorWindow; globalThis.CustomEvent = priorEvent; }
});

test("real hook, shell mapping and controller validate host freshness and live own commit before late take", async () => {
  const scenarios = [
    ["stale-no-tick", false], ["fresh-no-tick", true], ["new-frame-no-tick", true],
    ["status-throws", false], ["plan-getter-throws", false], ["frame-regressed", false],
    ["hook-cleanup", false], ["runtime-replaced", false], ["capability-lost", false],
    ["hidden", false], ["own-raw-changed", false], ["player-changed", false],
    ["generation-changed", false], ["runtime-generation-changed", false],
    ["presentation-changed", false], ["sibling-raw-changed", true],
  ];
  for (const [scenario, expectedTake] of scenarios) {
    const priorWindow = globalThis.window; const priorPerformance = globalThis.performance;
    let effect, state, timer, now = 100, cleanup, controller, intervalCalls = 0;
    globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
    globalThis.performance = { now: () => now };
    try {
      const refs = []; let index = 0;
      const react = { useRef(value) { return refs[index++] ??= { current: value }; },
        useCallback(fn) { return fn; }, useEffect(fn) { effect = fn; },
        useState(value) { state ??= value; return [state, next => {
          state = typeof next === "function" ? next(state) : next;
        }]; } };
      const host = loadTs("../lib/bevy-quest-ui.ts", { "./quest-action-policy": { questEndpoint: value => value } });
      const hook = loadTs("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
        "./bevy-hp-orb": loadTs("../lib/bevy-hp-orb.ts"),
        "./bevy-experience-bar": loadTs("../lib/bevy-experience-bar.ts"),
        "./bevy-weight-bar": loadTs("../lib/bevy-weight-bar.ts"),
        "./bevy-hud-bar-draw-plan": draw });
      let live = { ...makeSnapshot(null), hudBarPlayerObjectId: "1000", hudBarRuntimeLifetime: 3,
        questLogOpen: false, blocksGameplayKeys: false, turnInBlocked: false, profile: "crystal",
        quests: [], completedKnown: false, completedQuestIds: [] };
      const sent = []; let appliedPlan = null;
      let status = { frame: 1, ready: true, error: null, capturesPointer: false,
        questLogOpen: false, generation: 5, revision: 0, openRevision: 0,
        experienceBar: { supported: false, ready: false }, weightBar: { supported: false, ready: false } };
      const runtime = { setMir2QuestUiSnapshot(json) { sent.push(JSON.parse(json)); return true; },
        setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {},
        getMir2QuestUiStatus: () => JSON.stringify(status), getMir2HudBarDrawPlanVersion: () => 1,
        getMir2HudBarDrawPlan: () => JSON.stringify(appliedPlan) };
      const runtimeRef = { current: runtime };
      const owner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 3,
        runtimeRef, snapshot: () => live, onIntent: () => ({ accepted: false }), onOpenChange() {} });
      cleanup = effect();
      appliedPlan = { ...plan(sent[0].hudBarPlan), revision: sent[0].revision };
      status = { ...status, frame: 2, revision: sent[0].revision };
      owner.refresh();
      assert.ok(state.hudBarPlans?.experience, `${scenario}: real hook selected applied EXP`);
      const pending = deferred(); const expCanvas = canvas(); const weightCanvas = canvas();
      let takes = 0;
      controller = new draw.HudBarCanvasController((name, owns) => {
        if (name === "experience" && owns) takes++;
      }, path => path.endsWith("/8.png") ? pending.promise : Promise.resolve({}));
      const shellScope = () => ({ imageMayOwn: true, bevyHudSourceGeometry: null, bevyHudBarIdentity: "r3-g5-p1000",
        bevyHudBarPlans: state.hudBarPlans, bevyHudBarReadLivePlans: owner.readLiveHudBarPlans,
        presentation, world: { playerExperience: 50, playerMaxExperience: 100,
          currentWeight: 50, maxWeight: 100 }, frame: {},
        experienceAnchor: anchor(50, 100, "experience"), weightAnchor: anchor(50, 100, "weight"),
        experienceSlot: expSlot, weightSlot,
        experienceDrawCanvasRef: { current: expCanvas }, weightDrawCanvasRef: { current: weightCanvas },
        readBevyExperienceBarSlot: () => expSlot, readBevyWeightBarSlot: () => weightSlot });
      controller.update(actualShellCanvasInput(shellScope()));
      await flush();
      if (scenario === "stale-no-tick") now = 3101;
      if (scenario === "fresh-no-tick") now = 1100;
      if (scenario === "new-frame-no-tick") {
        now = 2600; status = { ...status, frame: 3 };
        controller.update(actualShellCanvasInput(shellScope()));
        now = 3101;
      }
      if (scenario === "status-throws") runtime.getMir2QuestUiStatus = () => { throw new Error("host lost"); };
      if (scenario === "plan-getter-throws") runtime.getMir2HudBarDrawPlan = () => { throw new Error("plan lost"); };
      if (scenario === "frame-regressed") status = { ...status, frame: 1 };
      if (scenario === "hook-cleanup") { cleanup(); cleanup = null; }
      if (scenario === "runtime-replaced") runtimeRef.current = {};
      if (scenario === "capability-lost") runtime.getMir2HudBarDrawPlanVersion = () => 0;
      if (scenario === "hidden") live = { ...live, hostVisible: false };
      if (scenario === "own-raw-changed") live = { ...live, player: { ...live.player, experience: 51 } };
      if (scenario === "player-changed") live = { ...live, hudBarPlayerObjectId: "1001" };
      if (scenario === "generation-changed") live = { ...live, generation: 6 };
      if (scenario === "runtime-generation-changed") live = { ...live, hudBarRuntimeLifetime: 4 };
      if (scenario === "presentation-changed") live = { ...live,
        presentation: { ...presentation, stageCssScale: 0.75 } };
      if (scenario === "sibling-raw-changed") live = { ...live,
        player: { ...live.player, currentWeight: 51 } };
      pending.resolve({}); await flush();
      assert.equal(takes > 0, expectedTake, `${scenario}: no interval fired before decode`);
      assert.equal(expCanvas.style.visibility, expectedTake ? "visible" : "hidden");
      assert.equal(intervalCalls, 0);
      assert.equal(typeof timer, scenario === "hook-cleanup" ? "object" : "function");
    } finally {
      controller?.dispose(); cleanup?.();
      globalThis.window = priorWindow; globalThis.performance = priorPerformance;
    }
  }
});

test("an old hook cleanup cannot erase the successor's live plan reader", () => {
  const priorWindow = globalThis.window; const priorPerformance = globalThis.performance;
  let effect, state, now = 100;
  globalThis.window = { setInterval() { return 1; }, clearInterval() {} };
  globalThis.performance = { now: () => now };
  let cleanup1, cleanup2;
  try {
    const refs = []; let index = 0;
    const react = { useRef(value) { return refs[index++] ??= { current: value }; },
      useCallback(fn) { return fn; }, useEffect(fn) { effect = fn; },
      useState(value) { state ??= value; return [state, next => {
        state = typeof next === "function" ? next(state) : next;
      }]; } };
    const host = loadTs("../lib/bevy-quest-ui.ts", { "./quest-action-policy": { questEndpoint: value => value } });
    const hook = loadTs("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
      "./bevy-hp-orb": loadTs("../lib/bevy-hp-orb.ts"),
      "./bevy-experience-bar": loadTs("../lib/bevy-experience-bar.ts"),
      "./bevy-weight-bar": loadTs("../lib/bevy-weight-bar.ts"),
      "./bevy-hud-bar-draw-plan": draw });
    const makeRuntime = () => {
      const sent = []; let applied = null;
      let status = { frame: 1, ready: true, error: null, capturesPointer: false,
        questLogOpen: false, generation: 5, revision: 0, openRevision: 0,
        experienceBar: { supported: false, ready: false }, weightBar: { supported: false, ready: false } };
      const runtime = { setMir2QuestUiSnapshot(json) { sent.push(JSON.parse(json)); return true; },
        setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {},
        getMir2QuestUiStatus: () => JSON.stringify(status), getMir2HudBarDrawPlanVersion: () => 1,
        getMir2HudBarDrawPlan: () => JSON.stringify(applied) };
      return { runtime, sent, applied: value => { applied = value; }, status: value => { status = value; } };
    };
    const first = makeRuntime(); const second = makeRuntime();
    const live = lifetime => ({ ...makeSnapshot(null), hudBarPlayerObjectId: "1000",
      hudBarRuntimeLifetime: lifetime, questLogOpen: false, blocksGameplayKeys: false,
      turnInBlocked: false, profile: "crystal", quests: [], completedKnown: false,
      completedQuestIds: [] });
    const firstOwner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 3,
      runtimeRef: { current: first.runtime }, snapshot: () => live(3),
      onIntent: () => ({ accepted: false }), onOpenChange() {} });
    cleanup1 = effect();
    first.applied({ ...plan(first.sent[0].hudBarPlan), revision: first.sent[0].revision });
    first.status({ frame: 2, ready: true, error: null, capturesPointer: false,
      questLogOpen: false, generation: 5, revision: first.sent[0].revision, openRevision: 0,
      experienceBar: { supported: false, ready: false }, weightBar: { supported: false, ready: false } });
    firstOwner.refresh();
    assert.equal(firstOwner.readLiveHudBarPlans()?.lifetime, "r3-g5-p1000");
    index = 0; now = 200;
    const secondOwner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 4,
      runtimeRef: { current: second.runtime }, snapshot: () => live(4),
      onIntent: () => ({ accepted: false }), onOpenChange() {} });
    cleanup2 = effect();
    second.applied({ ...plan(second.sent[0].hudBarPlan), revision: second.sent[0].revision });
    second.status({ frame: 2, ready: true, error: null, capturesPointer: false,
      questLogOpen: false, generation: 5, revision: second.sent[0].revision, openRevision: 0,
      experienceBar: { supported: false, ready: false }, weightBar: { supported: false, ready: false } });
    secondOwner.refresh();
    assert.equal(secondOwner.readLiveHudBarPlans()?.lifetime, "r4-g5-p1000");
    cleanup1(); cleanup1 = null;
    assert.equal(secondOwner.readLiveHudBarPlans()?.lifetime, "r4-g5-p1000");
    cleanup2(); cleanup2 = null;
    assert.equal(secondOwner.readLiveHudBarPlans(), null);
  } finally {
    cleanup1?.(); cleanup2?.();
    globalThis.window = priorWindow; globalThis.performance = priorPerformance;
  }
});
