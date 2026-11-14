import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

function load(path, dependencies = {}) {
  const module = { exports: {} };
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText;
  new Function("exports", "module", "require", code)(module.exports, module, (name) => {
    if (name in dependencies) return dependencies[name];
    throw new Error(`Unexpected runtime dependency ${name}`);
  });
  return module.exports;
}
const bar = load("../lib/bevy-experience-bar.ts");
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 0.5, touch: true };
const slot = { left: 9, top: 759, width: 1004, height: 8 };
const snapshot = { generation: 5, revision: 9, inGame: true, hostVisible: true, presentation,
  experienceBarSlot: slot, player: { experience: 50, maxExperience: 100 } };
const painted = { supported: true, ready: true, generation: 5, revision: 9,
  experience: 50, maxExperience: 100, slot, image: "original-ui/Prguse/8.png",
  source: { left: 0, top: 0, width: 500, height: 8 },
  destination: { left: 9, top: 759, width: 500, height: 8 },
  layout: { left: 9, top: 759, width: 500, height: 8 } };
const status = { frame: 3, ready: false, generation: 5, revision: 9, experienceBar: painted };

test("full anchor includes stage and HUD scaling, without a DPR multiplier", () => {
  const frame = { contains: (node) => node === anchor,
    getBoundingClientRect: () => ({ left: 20, top: 40, width: 512, height: 384 }) };
  const anchor = { isConnected: true,
    getBoundingClientRect: () => ({ left: 24.5, top: 419.5, width: 502, height: 4 }) };
  assert.deepEqual(bar.readBevyExperienceBarSlot(frame, anchor, presentation), slot);
  anchor.getBoundingClientRect = () => ({ left: 24.5, top: 419.5, width: 251, height: 2 });
  assert.deepEqual(bar.readBevyExperienceBarSlot(frame, anchor, presentation),
    { left: 9, top: 759, width: 502, height: 4 });
  frame.getBoundingClientRect = () => ({ left: 20, top: 40, width: 600, height: 384 });
  assert.equal(bar.readBevyExperienceBarSlot(frame, anchor, presentation), null);
  anchor.isConnected = false;
  assert.equal(bar.readBevyExperienceBarSlot(frame, anchor, presentation), null);
});

test("safe values and optional support preserve old runtimes", () => {
  assert.deepEqual(bar.safeExperiencePair(0, 1), { experience: 0, maxExperience: 1 });
  for (const pair of [[NaN, 1], [1.5, 2], [Number.MAX_SAFE_INTEGER + 1, 2], [1, Infinity], [null, 1]]) {
    assert.equal(bar.safeExperiencePair(...pair), null);
  }
  assert.equal(bar.supportsBevyExperienceBar({ getMir2QuestUiStatus: () => "{}" }), false);
  assert.equal(bar.supportsBevyExperienceBar({ getMir2QuestUiStatus: () => JSON.stringify(status) }), true);
  assert.equal(bar.supportsBevyExperienceBar({ getMir2QuestUiStatus: () => "{" }), false);
  const legacy = bar.stripUnsupportedExperienceBar(snapshot, false);
  assert.equal("experienceBarSlot" in legacy, false);
  assert.equal("experience" in legacy.player, false);
  assert.equal("maxExperience" in legacy.player, false);
  assert.deepEqual(bar.stripUnsupportedExperienceBar(snapshot, true), snapshot);
});

test("packet provenance handles real max one, partial updates and generation changes", () => {
  const owned = bar.experienceAuthorityFromPacket(5, "1000", 0, 1);
  assert.deepEqual(owned, { generation: 5, playerObjectId: "1000", experience: 0, maxExperience: 1 });
  assert.equal(bar.experienceAuthorityFromPacket(5, "1000", undefined, 1), null);
  assert.equal(bar.experienceAuthorityFromPacket(5, "1000", 0, 0), null);
  assert.equal(bar.experienceAuthorityFromPacket(5, "1000", Number.MAX_SAFE_INTEGER + 1, 1), null);
  assert.deepEqual(bar.advanceExperienceAuthority(owned, 5, "1000", 0, 1, 1),
    { ...owned, experience: 1 });
  assert.equal(bar.advanceExperienceAuthority(owned, 5, "1000", 2, 1, 1), null);
  assert.equal(bar.advanceExperienceAuthority(owned, 6, "1000", 0, 1, 1), null);
  assert.equal(bar.matchingExperienceAuthority(owned, 5, "1001", 0, 1), null);
  assert.equal(bar.matchingExperienceAuthority(owned, 5, "1000", 0, 1)?.maxExperience, 1);
});

test("ownership requires current generation, revision, model, asset and post-layout crop", () => {
  assert.deepEqual(bar.currentBevyExperienceBar(status, snapshot, true), painted);
  const reject = (s = status, p = snapshot, fresh = true) =>
    assert.equal(bar.currentBevyExperienceBar(s, p, fresh), null);
  reject(status, snapshot, false);
  reject(status, { ...snapshot, generation: 6 });
  reject(status, { ...snapshot, revision: 10 });
  reject(status, { ...snapshot, inGame: false });
  reject(status, { ...snapshot, hostVisible: false });
  reject(status, { ...snapshot, presentation: { ...presentation, logicalWidth: NaN } });
  reject(status, { ...snapshot, experienceBarSlot: { ...slot, width: 300 } });
  reject(status, { ...snapshot, experienceBarSlot: null });
  reject(status, { ...snapshot, player: { experience: 51, maxExperience: 100 } });
  reject(status, { ...snapshot, player: { experience: 50, maxExperience: 0 } });
  reject(status, { ...snapshot, player: { experience: 50, maxExperience: null } });
  reject({ ...status, experienceBar: { ...painted, ready: false } });
  reject({ ...status, experienceBar: { ...painted, image: "other.png" } });
  reject({ ...status, experienceBar: { ...painted, layout: { ...painted.layout, top: 750 } } });
  reject({ ...status, experienceBar: { ...painted, source: { ...painted.source, width: 501 } } });
  reject({ ...status, experienceBar: { ...painted, slot: { ...slot, left: 20 } } });
  assert.equal(bar.matchesBevyExperienceBarView(painted, slot, 50, 100), true);
  assert.equal(bar.matchesBevyExperienceBarView(painted, slot, 51, 100), false);
  const visible = { getAttribute: (name) => name === "data-experience" ? "51" : "100" };
  assert.equal(bar.matchesBevyExperienceBarView(painted, slot, 50, 100, visible), false,
    "selector HUD can commit the new visible value before the parent props update");
  visible.getAttribute = (name) => name === "data-experience" ? "50" : "100";
  assert.equal(bar.matchesBevyExperienceBarView(painted, slot, 50, 100, visible), true);
});

test("known empty and over-maximum crops remain valid, including genuine max one", () => {
  const empty = { ...painted, experience: 0, maxExperience: 1,
    source: { ...painted.source, width: 0 },
    destination: { ...painted.destination, width: 0 },
    layout: { ...painted.layout, width: 0 } };
  assert.deepEqual(bar.currentBevyExperienceBar({ ...status, experienceBar: empty },
    { ...snapshot, player: { experience: 0, maxExperience: 1 } }, true), empty);
  const full = { ...painted, experience: 200, source: { ...painted.source, width: 1001 },
    destination: { ...painted.destination, width: 1001 }, layout: { ...painted.layout, width: 1001 } };
  assert.deepEqual(bar.currentBevyExperienceBar({ ...status, experienceBar: full },
    { ...snapshot, player: { experience: 200, maxExperience: 100 } }, true), full);
});

test("hook strips old DTO fields and leases only current painted frames across loss, reset and cleanup", () => {
  const previousWindow = globalThis.window;
  const previousPerformance = globalThis.performance;
  let now = 100, effect, state, timer;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
  globalThis.performance = { now: () => now };
  try {
    const refs = []; let index = 0;
    const react = { useRef(value) { return refs[index++] ??= { current: value }; },
      useCallback(fn) { return fn; }, useEffect(fn) { effect = fn; },
      useState(value) { state ??= value; return [state, (next) => {
        state = typeof next === "function" ? next(state) : next;
      }]; } };
    const host = load("../lib/bevy-quest-ui.ts", { "./quest-action-policy": { questEndpoint: (name) => name } });
    const hp = load("../lib/bevy-hp-orb.ts");
    const hook = load("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
      "./bevy-hp-orb": hp, "./bevy-experience-bar": bar,
      "./bevy-weight-bar": load("../lib/bevy-weight-bar.ts"),
      "./bevy-hud-bar-draw-plan": load("../lib/bevy-hud-bar-draw-plan.ts") });
    let live = { ...snapshot, questLogOpen: false, blocksGameplayKeys: false, turnInBlocked: false,
      profile: "crystal", quests: [], completedKnown: false, completedQuestIds: [],
      dialog: { isOpen: false, hasInput: false, lines: [], options: [] } };
    let supported = false;
    let currentStatus = { ...status, frame: 1, revision: 0, error: null, capturesPointer: false,
      questLogOpen: false, openRevision: 0, experienceBar: undefined };
    const submitted = [];
    const runtime = { setMir2QuestUiSnapshot(json) { submitted.push(JSON.parse(json)); return true; },
      getMir2QuestUiStatus() { return JSON.stringify(supported ? currentStatus
        : { ...currentStatus, experienceBar: undefined }); },
      setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {} };
    const owner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 1,
      runtimeRef: { current: runtime }, snapshot: () => live,
      onIntent: () => ({ accepted: false }), onOpenChange() {} });
    const cleanup = effect();
    let sent = submitted.at(-1);
    assert.equal(Object.hasOwn(sent, "experienceBarSlot"), false);
    assert.equal(Object.hasOwn(sent.player, "experience"), false);
    assert.equal(Object.hasOwn(sent.player, "maxExperience"), false);
    assert.equal(state.experienceBar, null);

    const acknowledge = (experience = live.player.experience) => {
      sent = submitted.at(-1);
      const width = Math.floor(Math.fround(1001 * Math.fround(experience / live.player.maxExperience)));
      currentStatus = { ...currentStatus, frame: currentStatus.frame + 1,
        generation: sent.generation, revision: sent.revision,
        experienceBar: { ...painted, generation: sent.generation, revision: sent.revision,
          experience, maxExperience: live.player.maxExperience, slot: sent.experienceBarSlot,
          source: { ...painted.source, width },
          destination: { ...painted.destination, width: width * (slot.width / 1004) },
          layout: { ...painted.layout, width: width * (slot.width / 1004) } } };
      owner.refresh();
    };
    supported = true;
    currentStatus = { ...currentStatus, experienceBar: { ...painted, ready: false } };
    timer();
    sent = submitted.at(-1);
    assert.deepEqual(sent.experienceBarSlot, slot);
    assert.deepEqual([sent.player.experience, sent.player.maxExperience], [50, 100]);
    assert.equal(state.experienceBar, null, "capability alone cannot own without current paint acknowledgement");
    acknowledge();
    assert.equal(state.experienceBar?.experience, 50);

    live = { ...live, player: { ...live.player, experience: 51 } };
    owner.refresh();
    assert.equal(state.experienceBar, null, "new model immediately retires the old crop");
    acknowledge();
    assert.equal(state.experienceBar?.experience, 51);
    live = { ...live, hostVisible: false };
    owner.refresh();
    assert.equal(state.experienceBar, null, "explicit host hiding retires the image");
    live = { ...live, hostVisible: true };
    owner.refresh();
    acknowledge();
    assert.equal(state.experienceBar?.experience, 51);

    live = { ...live, presentation: null };
    owner.refresh();
    assert.equal(submitted.at(-1).hostVisible, false);
    assert.equal(state.experienceBar, null, "layout loss transfers the image back to HTML");
    live = { ...live, presentation };
    owner.refresh();
    acknowledge();
    assert.equal(state.experienceBar?.experience, 51);

    now += 2001;
    owner.refresh();
    assert.equal(state.experienceBar, null, "stopped frames expire the paint lease");
    live = { ...live, generation: 6 };
    owner.refresh();
    assert.equal(state.experienceBar, null, "a new generation cannot borrow the previous paint");
    acknowledge();
    assert.equal(state.experienceBar?.generation, 6);
    cleanup();
    const retiredRevision = submitted.at(-1).revision;
    assert.equal(submitted.at(-1).inGame, false);
    owner.refresh();
    assert.equal(submitted.at(-1).revision, retiredRevision, "cleanup prevents stale republishing");
  } finally {
    if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
    if (previousPerformance === undefined) delete globalThis.performance; else globalThis.performance = previousPerformance;
  }
});
