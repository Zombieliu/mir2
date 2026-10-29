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
const bar = load("../lib/bevy-weight-bar.ts");
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 0.5, touch: true };
const slot = { left: 919, top: 721, width: 76, height: 12 };
const snapshot = { generation: 5, revision: 9, inGame: true, hostVisible: true, presentation,
  weightBarSlot: slot, player: { currentWeight: 50, maxWeight: 100 } };
const painted = { supported: true, ready: true, generation: 5, revision: 9,
  currentWeight: 50, maxWeight: 100, slot, image: "original-ui/Prguse/76.png",
  source: { left: 0, top: 0, width: 37, height: 12 },
  destination: { left: 919, top: 721, width: 37, height: 12 },
  layout: { left: 919, top: 721, width: 37, height: 12 } };
const status = { frame: 3, generation: 5, revision: 9, weightBar: painted };

test("raw u16 pair grants legitimate zero and rejects absent/default-like maxima", () => {
  assert.deepEqual(bar.safeWeightPair(0, 1), { currentWeight: 0, maxWeight: 1 });
  for (const pair of [[undefined, 1], [null, 1], [0, undefined], [0, 0], [-1, 1],
    [0.5, 1], [1, 65536], [65536, 1], [NaN, 1], [Infinity, 1]]) {
    assert.equal(bar.safeWeightPair(...pair), null);
  }
  const owned = bar.weightAuthorityFromPacket(5, "1000", 0, 1);
  assert.deepEqual(owned, { generation: 5, playerObjectId: "1000", currentWeight: 0, maxWeight: 1 });
  assert.equal(bar.matchingWeightAuthority(owned, 5, "1000", 0, 1), owned);
  assert.equal(bar.matchingWeightAuthority(owned, 6, "1000", 0, 1), null);
  assert.equal(bar.matchingWeightAuthority(owned, 5, "1001", 0, 1), null);
  assert.equal(bar.matchingWeightAuthority(owned, 5, "1000", 1, 1), null);
});

test("real movement shortcut guard fails for weight-only and max-only snapshots", () => {
  const world = { currentWeight: 50, maxWeight: 100 };
  const owned = bar.weightAuthorityFromPacket(5, "1000", 50, 100);
  const guard = (packet) => bar.movementSnapshotWeightMatches(
    bar.projectSnapshotWeightPair(packet, owned, 5, "1000"), world);
  assert.equal(guard(world), true);
  assert.equal(guard({}), true, "missing-both retains the proven movement pair");
  assert.equal(guard({ ...world, currentWeight: 51 }), false);
  assert.equal(guard({ ...world, maxWeight: 101 }), false);
  assert.equal(guard({ currentWeight: undefined, maxWeight: 100 }), false);
});

test("raw packet projection preserves only a proven same-player pair through world and host", () => {
  let world = { currentWeight: 0, maxWeight: 0 };
  let authority = null;
  const apply = (packet, generation = 5, playerObjectId = "1000") => {
    const projection = bar.projectSnapshotWeightPair(packet, authority, generation, playerObjectId);
    const shortcutWeightMatches = bar.movementSnapshotWeightMatches(projection, world);
    authority = projection.authority;
    // This is the pair page.tsx puts into the normal world projection.
    world = { currentWeight: projection.currentWeight, maxWeight: projection.maxWeight };
    const hostPair = bar.matchingWeightAuthority(authority, generation, playerObjectId,
      world.currentWeight, world.maxWeight);
    return { projection, shortcutWeightMatches, hostPair };
  };
  assert.equal(apply({}).hostPair, null, "display defaults cannot seed authority");
  const raw = { currentWeight: 0, maxWeight: 1 };
  assert.equal(apply(raw).hostPair?.maxWeight, 1, "real empty bag is authoritative");
  assert.deepEqual(raw, { currentWeight: 0, maxWeight: 1 }, "packet is never mutated");
  const partial = apply({});
  assert.deepEqual(world, { currentWeight: 0, maxWeight: 1 });
  assert.equal(partial.shortcutWeightMatches, true);
  assert.equal(partial.hostPair?.currentWeight, 0);
  assert.equal(apply({ currentWeight: 2 }).hostPair, null, "one-sided value cannot borrow maximum");
  assert.deepEqual(world, { currentWeight: 2, maxWeight: 0 });
  assert.equal(apply({}).hostPair, null, "released pair cannot be revived from display state");
  assert.equal(apply({ currentWeight: 3, maxWeight: 10 }).hostPair?.currentWeight, 3);
  assert.equal(apply({}, 6).hostPair, null, "generation change clears continuation");
  assert.equal(apply({ currentWeight: 3, maxWeight: 10 }, 6).hostPair?.maxWeight, 10);
  assert.equal(apply({}, 6, "1001").hostPair, null, "player change clears continuation");
  assert.equal(apply({ currentWeight: 3, maxWeight: 10 }, 6, "1001").hostPair?.maxWeight, 10);
  assert.equal(apply({}, 6, null).hostPair, null, "logout/reset clears continuation");
  assert.equal(apply({ currentWeight: 0, maxWeight: 0 }, 7, "1002").hostPair, null);
  assert.equal(apply({ currentWeight: 65536, maxWeight: 10 }, 7, "1002").hostPair, null);
  assert.equal(apply({ currentWeight: 2, maxWeight: undefined }, 7, "1002").hostPair, null);
});

test("legacy capability strips both nested fields and the slot", () => {
  assert.equal(bar.supportsBevyWeightBar({ getMir2QuestUiStatus: () => "{}" }), false);
  assert.equal(bar.supportsBevyWeightBar({ getMir2QuestUiStatus: () => JSON.stringify(status) }), true);
  assert.equal(bar.supportsBevyWeightBar({ getMir2QuestUiStatus: () => "{" }), false);
  const legacy = bar.stripUnsupportedWeightBar(snapshot, false);
  assert.equal("weightBarSlot" in legacy, false);
  assert.equal("currentWeight" in legacy.player, false);
  assert.equal("maxWeight" in legacy.player, false);
  assert.deepEqual(bar.stripUnsupportedWeightBar(snapshot, true), snapshot);
});

test("full DOM slot is measured once through stage scale, including y 721", () => {
  const frame = { contains: (node) => node === anchor,
    getBoundingClientRect: () => ({ left: 20, top: 40, width: 512, height: 384 }) };
  const anchor = { isConnected: true,
    getBoundingClientRect: () => ({ left: 479.5, top: 400.5, width: 38, height: 6 }) };
  assert.deepEqual(bar.readBevyWeightBarSlot(frame, anchor, presentation), slot);
  anchor.isConnected = false;
  assert.equal(bar.readBevyWeightBarSlot(frame, anchor, presentation), null);
});

test("current image, crop, layout and live DOM pair gate ownership", () => {
  assert.deepEqual(bar.currentBevyWeightBar(status, snapshot, true), painted);
  assert.equal(bar.currentBevyWeightBar(status, snapshot, false), null);
  assert.equal(bar.currentBevyWeightBar(status, { ...snapshot, generation: 6 }, true), null);
  assert.equal(bar.currentBevyWeightBar(status, { ...snapshot, revision: 10 }, true), null);
  assert.equal(bar.currentBevyWeightBar(status, { ...snapshot, player: { currentWeight: 51, maxWeight: 100 } }, true), null);
  assert.equal(bar.currentBevyWeightBar({ ...status, weightBar: { ...painted, image: "original-ui/UI_32bit/473.png" } }, snapshot, true), null);
  assert.equal(bar.currentBevyWeightBar({ ...status, weightBar: { ...painted, ready: false } }, snapshot, true), null);
  assert.equal(bar.currentBevyWeightBar({ ...status, weightBar: { ...painted, layout: { ...painted.layout, top: 719 } } }, snapshot, true), null);
  const anchor = { getAttribute: (key) => key === "data-current-weight" ? "50" : "100" };
  assert.equal(bar.matchesBevyWeightBarView(painted, slot, 50, 100, anchor), true);
  anchor.getAttribute = (key) => key === "data-current-weight" ? "51" : "100";
  assert.equal(bar.matchesBevyWeightBarView(painted, slot, 50, 100, anchor), false);
});

test("threshold image and f32 crop validation cover zero, boundaries and overmax", () => {
  for (const [currentWeight, maxWeight, image, width] of [
    [0, 100, "original-ui/Prguse/76.png", 0],
    [50, 100, "original-ui/Prguse/76.png", 37],
    [51, 100, "original-ui/UI_32bit/473.png", 37],
    [75, 100, "original-ui/UI_32bit/473.png", 55],
    [76, 100, "original-ui/UI_32bit/472.png", 56],
    [100, 100, "original-ui/UI_32bit/472.png", 74],
    [200, 100, "original-ui/UI_32bit/472.png", 74],
    [1, 74, "original-ui/Prguse/76.png", 1],
  ]) {
    const expected = { ...painted, currentWeight, maxWeight, image,
      source: { ...painted.source, width },
      destination: { ...painted.destination, width },
      layout: { ...painted.layout, width } };
    assert.deepEqual(bar.currentBevyWeightBar({ ...status, weightBar: expected },
      { ...snapshot, player: { currentWeight, maxWeight } }, true), expected);
  }
});

test("hook keeps old runtimes clean and retires a stale selected sprite", () => {
  const previousWindow = globalThis.window;
  const previousPerformance = globalThis.performance;
  let effect, state, timer;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
  globalThis.performance = { now: () => 100 };
  try {
    const refs = []; let index = 0;
    const react = { useRef(value) { return refs[index++] ??= { current: value }; },
      useCallback(fn) { return fn; }, useEffect(fn) { effect = fn; },
      useState(value) { state ??= value; return [state, (next) => {
        state = typeof next === "function" ? next(state) : next;
      }]; } };
    const host = load("../lib/bevy-quest-ui.ts", { "./quest-action-policy": { questEndpoint: (name) => name } });
    const hook = load("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
      "./bevy-hp-orb": load("../lib/bevy-hp-orb.ts"),
      "./bevy-experience-bar": load("../lib/bevy-experience-bar.ts"),
      "./bevy-weight-bar": bar,
      "./bevy-hud-bar-draw-plan": load("../lib/bevy-hud-bar-draw-plan.ts") });
    const live = { ...snapshot, questLogOpen: false, blocksGameplayKeys: false, turnInBlocked: false,
      profile: "crystal", quests: [], completedKnown: false, completedQuestIds: [],
      dialog: { isOpen: false, hasInput: false, lines: [], options: [] } };
    const submitted = []; let supported = false;
    let currentStatus = { ...status, frame: 1, revision: 0, error: null, ready: false,
      capturesPointer: false, questLogOpen: false, openRevision: 0, weightBar: undefined };
    const runtime = { setMir2QuestUiSnapshot(json) { submitted.push(JSON.parse(json)); return true; },
      getMir2QuestUiStatus() { return JSON.stringify(supported ? currentStatus : { ...currentStatus, weightBar: undefined }); },
      setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {} };
    const owner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 1,
      runtimeRef: { current: runtime }, snapshot: () => live,
      onIntent: () => ({ accepted: false }), onOpenChange() {} });
    const cleanup = effect();
    let sent = submitted.at(-1);
    assert.equal(Object.hasOwn(sent, "weightBarSlot"), false);
    assert.equal(Object.hasOwn(sent.player, "currentWeight"), false);
    assert.equal(Object.hasOwn(sent.player, "maxWeight"), false);
    assert.equal(state.weightBar, null);
    supported = true;
    currentStatus = { ...currentStatus, weightBar: { ...painted, ready: false } };
    timer();
    sent = submitted.at(-1);
    assert.deepEqual(sent.weightBarSlot, slot);
    assert.deepEqual([sent.player.currentWeight, sent.player.maxWeight], [50, 100]);
    assert.equal(state.weightBar, null);
    currentStatus = { ...currentStatus, frame: 2, generation: sent.generation,
      revision: sent.revision, weightBar: { ...painted, revision: sent.revision } };
    owner.refresh();
    assert.equal(state.weightBar?.currentWeight, 50);
    currentStatus = { ...currentStatus, frame: 3,
      weightBar: { ...painted, revision: sent.revision, image: "original-ui/UI_32bit/473.png" } };
    owner.refresh();
    assert.equal(state.weightBar, null);
    cleanup();
  } finally {
    globalThis.window = previousWindow;
    globalThis.performance = previousPerformance;
  }
});
