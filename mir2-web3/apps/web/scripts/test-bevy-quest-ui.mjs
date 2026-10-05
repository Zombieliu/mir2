import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function load(relative, dependencies = {}) {
  const file = new URL(relative, import.meta.url);
  const compiled = ts.transpileModule(readFileSync(file, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  const module = { exports: {} };
  new Function("require", "exports", "module", compiled.outputText)(
    (name) => {
      if (name in dependencies) return dependencies[name];
      if (name === "./quest-action-policy") return load("../lib/quest-action-policy.ts");
      throw new Error(`Unexpected dependency ${name}`);
    }, module.exports, module,
  );
  return module.exports;
}
const host = load("../lib/bevy-quest-ui.ts");
const stage = load("../app/components/original-client-stage-presentation.ts");
const quest = { questId: 2110002, title: "N2", summary: "Equip a weapon", objective: "Weapon",
  current: 1, required: 1, acceptNpcIndex: 0, finishNpcIndex: 0 };

function stageElements(presentation, overrides = {}) {
  const width = presentation.virtualWidth;
  const height = presentation.virtualHeight;
  const scale = presentation.scale;
  const frame = {
    dataset: { viewportSceneWidth: String(width), viewportSceneHeight: String(height) },
    clientWidth: width, clientHeight: height,
    getBoundingClientRect: () => ({ left: 7, top: 11, width: width * scale, height: height * scale }),
  };
  const canvas = {
    parentElement: frame, clientWidth: width, clientHeight: height,
    getBoundingClientRect: () => ({ left: 7, top: 11, width: width * scale, height: height * scale }),
  };
  Object.assign(frame, overrides.frame);
  Object.assign(canvas, overrides.canvas);
  return { frame, canvas };
}

test("844×390 touch stage, DPR and resize project the shell's actual CSS scale", () => {
  const first = stage.calculateMir2StagePresentation({ cssWidth: 844, cssHeight: 390,
    devicePixelRatio: 2, layout: "touch", input: "touch", screen: "game", wideMobile: true });
  const observed = stageElements(first);
  assert.deepEqual(host.readBevyQuestPresentation(observed.frame, observed.canvas, true), {
    logicalWidth: first.virtualWidth, logicalHeight: first.virtualHeight,
    stageCssScale: first.scale, touch: true,
  });
  assert.ok(first.scale < 1, "CSS scale is a stage transform, not the device pixel ratio");
  const resized = stage.calculateMir2StagePresentation({ cssWidth: 900, cssHeight: 390,
    devicePixelRatio: 2, layout: "touch", input: "keyboardMouse", screen: "game", wideMobile: false });
  const next = stageElements(resized);
  assert.deepEqual(host.readBevyQuestPresentation(next.frame, next.canvas, false), {
    logicalWidth: resized.virtualWidth, logicalHeight: resized.virtualHeight,
    stageCssScale: resized.scale, touch: false,
  });
  assert.notEqual(resized.virtualWidth, first.virtualWidth);
});

test("missing or inconsistent stage/canvas geometry cannot transfer UI ownership", () => {
  const presentation = stage.calculateMir2StagePresentation({ cssWidth: 844, cssHeight: 390,
    devicePixelRatio: 1, layout: "touch", input: "touch", screen: "game", wideMobile: true });
  const { frame, canvas } = stageElements(presentation);
  assert.equal(host.readBevyQuestPresentation(null, canvas, true), null);
  assert.equal(host.readBevyQuestPresentation(frame, null, true), null);
  const wrongFrame = stageElements(presentation, { frame: { clientWidth: frame.clientWidth - 1 } });
  assert.equal(host.readBevyQuestPresentation(wrongFrame.frame, wrongFrame.canvas, true), null);
  const wrongCanvas = stageElements(presentation, { canvas: { clientWidth: 1024 } });
  assert.equal(host.readBevyQuestPresentation(wrongCanvas.frame, wrongCanvas.canvas, true), null);
  const wrongTransform = stageElements(presentation, { canvas: {
    getBoundingClientRect: () => ({ left: 7, top: 11, width: 1, height: 1 }),
  } });
  assert.equal(host.readBevyQuestPresentation(wrongTransform.frame, wrongTransform.canvas, true), null);
});

test("static definitions never become available quests without an authoritative stage", () => {
  assert.deepEqual(host.projectBevyQuests([quest], new Map()), []);
  const [projected] = host.projectBevyQuests([quest], new Map([[quest.questId, "available"]]));
  assert.equal(projected.status, "notStarted");
  assert.equal(projected.acceptNpcIndex, 0);
  assert.equal(projected.finishNpcIndex, 0);
  assert.equal(host.projectBevyQuests([quest], new Map([[quest.questId, "inProgress"]]))[0].status, "inProgress",
    "full counters must not manufacture completion");
});

test("missing endpoints, source sections, counters and exact reward indices survive projection", () => {
  const [projected] = host.projectBevyQuests([{ ...quest, acceptNpcIndex: undefined, finishNpcIndex: undefined,
    group: "Chapter 1", minLevelNeeded: 3, taskDescriptionLines: ["Task source"],
    returnDescriptionLines: ["Return source"], completionDescriptionLines: ["Completion source"],
    rewards: { gold: 100, experience: 10, items: [{ name: "Sword", count: 1, icon: 7 }],
      selectItems: [{ name: "Armour", selectionIndex: 3, count: 2 }, { name: "Ring" }] },
  }], new Map([[quest.questId, "readyToTurnIn"]]));
  assert.equal(projected.acceptNpcIndex, null);
  assert.equal(projected.finishNpcIndex, null);
  assert.deepEqual(projected.detail.taskDescriptionLines, ["Task source"]);
  assert.deepEqual(projected.detail.returnDescriptionLines, ["Return source"]);
  assert.deepEqual(projected.detail.completionDescriptionLines, ["Completion source"]);
  assert.deepEqual(projected.rewards.filter((r) => r.type === "item").map((r) => r.selectionIndex), [undefined, 3, 1]);
  assert.equal(projected.objectives[0].current, 1);
  assert.equal(projected.objectives[0].target, 1);
});

test("malformed selectable rewards fail closed instead of becoming another valid choice", () => {
  for (const selectionIndex of [-1, 0.5, NaN, 0x80000000]) {
    assert.throws(() => host.projectBevyQuests([{ ...quest,
      rewards: { selectItems: [{ name: "Bad choice", selectionIndex }] },
    }], new Map([[quest.questId, "readyToTurnIn"]])));
  }
});

test("fixed and selectable reward previews retain their exact source through the Bevy DTO", () => {
  const { withQuestRewardTooltipSources } = load("../lib/quest-reward-presentation.ts");
  const fixed = { index: 658, name: "Raw fixed", durability: 0, slots: 0, item_type: 13,
    image: 532, stats: [{ stat: 27, value: 50 }], unknown_extension: { version: 2 } };
  const choice = { index: 659, name: "Raw choice", durability: 8000, slots: 1, item_type: 1, image: 7 };
  const rewards = withQuestRewardTooltipSources({ gold: 30,
    items: [{ name: "Fixed", itemIndex: 658, count: 11, icon: 532 }],
    selectItems: [{ name: "Choice", itemIndex: 659, count: 2, icon: 7, selectionIndex: 0 }],
  }, { rewards_fixed_item: [{ item: fixed }], rewards_select_item: [{ item: choice }] });
  const [projected] = host.projectBevyQuests([{ ...quest, rewards }], new Map([[quest.questId, "readyToTurnIn"]]));
  const items = JSON.parse(JSON.stringify(projected)).rewards.filter((entry) => entry.type === "item");
  assert.deepEqual(items.map((entry) => [entry.itemId, entry.quantity, entry.selectionIndex]), [
    ["658", 11, undefined], ["659", 2, 0],
  ]);
  assert.deepEqual(items[0].tooltipSource, rewards.items[0].tooltipSource);
  assert.deepEqual(items[1].tooltipSource, rewards.selectItems[0].tooltipSource);
  assert.equal(items[0].tooltipSource.userItem.count, 0, "cell quantity is separate from the preview UserItem");
  assert.equal(items[1].tooltipSource.userItem.unique_id, 0, "preview UID is never the reward/template ID");
  assert.equal(projected.rewards[1].tooltipSource, rewards.items[0].tooltipSource,
    "the projector preserves the source rather than rebuilding it from labels");
});

test("NPC links retain exact wire targets, disabled state and required-input fallback", () => {
  const dialog = host.projectBevyQuestDialog({ npcObjectId: "24", npcName: "Jane", title: "Hello", body: ["Body"], footer: "Bye",
    links: [{ text: "Accept", target: "@quest:accept:2110002", enabled: false }], input: { target: "@Input" } });
  assert.equal(dialog.npcObjectId, 24);
  assert.deepEqual(dialog.options, [{ optionId: "@quest:accept:2110002", label: "Accept", enabled: false }]);
  assert.equal(dialog.hasInput, true);
  assert.equal(host.projectBevyQuestDialog({ npcObjectId: "0", links: [], body: [] }).isOpen, false);
});

test("session and request sequences survive independent module loads", () => {
  const generation = host.nextQuestUiGeneration();
  const request = host.nextQuestRequestId();
  const remounted = load("../lib/bevy-quest-ui.ts");
  assert.ok(remounted.nextQuestUiGeneration() > generation);
  assert.notEqual(remounted.nextQuestRequestId(), request);
});

test("old generation and malformed renderer status cannot take UI ownership", () => {
  const status = { ready: true, error: null, capturesPointer: true, questLogOpen: true,
    generation: 12, revision: 9, openRevision: 3, frame: 8 };
  assert.equal(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => JSON.stringify(status) }, 13), null);
  assert.deepEqual(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => JSON.stringify(status) }, 12), status);
  assert.equal(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => "invalid" }, 12), null);
  assert.equal(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => JSON.stringify({ ...status, openRevision: undefined }) }, 12), null);
  assert.equal(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => JSON.stringify({ ...status, frame: undefined }) }, 12), null);
  const mpOrb = { supported: true, ready: false, generation: 12, revision: 9,
    mp: null, maxMp: null, hpOnly: null, slot: null, image: null,
    source: null, destination: null, layout: null };
  assert.deepEqual(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => JSON.stringify({ ...status, mpOrb }) }, 12)?.mpOrb,
    mpOrb, "an unready optional MP descriptor does not alter Quest ownership");
});

test("renderer restart, first NPC input capture and stopped-loop fallback preserve one UI owner", (t) => {
  let now = 100;
  t.mock.method(performance, "now", () => now);
  const references = [];
  const state = { current: undefined };
  let refIndex = 0;
  let mount;
  let tick;
  const react = {
    useRef(initial) { return references[refIndex++] ??= { current: initial }; },
    useState(initial) {
      state.current ??= initial;
      return [state.current, (next) => { state.current = typeof next === "function" ? next(state.current) : next; }];
    },
    useEffect(effect) { mount = effect; },
    useCallback(fn) { return fn; },
  };
  const hook = load("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
    "./bevy-hp-orb": load("../lib/bevy-hp-orb.ts"),
    "./bevy-experience-bar": load("../lib/bevy-experience-bar.ts"),
    "./bevy-weight-bar": load("../lib/bevy-weight-bar.ts"),
    "./bevy-hud-bar-draw-plan": load("../lib/bevy-hud-bar-draw-plan.ts") }).useBevyQuestUi;
  let model = { generation: 101, inGame: true, hostVisible: true, questLogOpen: false,
    blocksGameplayKeys: false, turnInBlocked: false, profile: "newcomer-v2", quests: [],
    presentation: { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 0.8, touch: false },
    completedKnown: true, completedQuestIds: [], dialog: host.projectBevyQuestDialog(null), player: { hp: 10, maxHp: 10, level: 1 } };
  const submitted = [];
  let applied = { ready: true, error: null, capturesPointer: false, questLogOpen: false, generation: 101, revision: 0, openRevision: 0, frame: 1 };
  let sink;
  const runtime = {
    setMir2QuestUiSnapshot(json) {
      const next = JSON.parse(json);
      assert.ok(next.revision > (submitted.at(-1)?.revision ?? 0), "cleanup and successor snapshots share a monotonic clock");
      submitted.push(next);
      return true;
    },
    setMir2QuestUiIntentSink(callback) { sink = callback; },
    clearMir2QuestUiIntentSink() {},
    getMir2QuestUiStatus: () => JSON.stringify(applied),
  };
  const previousWindow = globalThis.window;
  globalThis.window = { setInterval(fn) { tick = fn; return 1; }, clearInterval() {} };
  let cleanup;
  try {
    const render = () => {
      refIndex = 0;
      hook({ requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime }, snapshot: () => model,
        onIntent: () => ({ accepted: true }), onOpenChange(open) { model = { ...model, questLogOpen: open }; } });
    };
    render();
    cleanup = mount();
    applied = { ...applied, revision: submitted.at(-1).revision, openRevision: submitted.at(-1).openRevision };
    cleanup();
    render();
    cleanup = mount();
    assert.deepEqual(submitted.map((value) => value.revision), [1, 2, 3]);
    applied = { ...applied, revision: submitted.at(-1).revision, openRevision: submitted.at(-1).openRevision };
    tick();
    assert.equal(state.current.ready, true);
    assert.equal(state.current.capturesPointer, false);
    const appliedRevisionBeforeHealthUpdate = applied.revision;
    model = { ...model, player: { ...model.player, hp: 9 } };
    tick();
    assert.ok(submitted.at(-1).revision > appliedRevisionBeforeHealthUpdate);
    assert.equal(applied.revision, appliedRevisionBeforeHealthUpdate, "Bevy has not consumed the HP update yet");
    assert.equal(state.current.ready, true, "ordinary authoritative updates must not flash the compatibility UI");
    assert.equal(JSON.parse(sink(JSON.stringify({ generation: 101, type: "acceptQuest" }))).accepted, true,
      "a live, unchanged layout remains usable while the next model is being applied");
    model = { ...model, dialog: host.projectBevyQuestDialog({ npcObjectId: "24", npcName: "Jane",
      title: "Hello", body: [], footer: "", links: [] }) };
    tick();
    assert.equal(applied.capturesPointer, false, "Bevy has not consumed the new NPC model yet");
    assert.equal(state.current.ready, true, "an incoming NPC model must not replace the healthy renderer");
    assert.equal(state.current.capturesPointer, true, "the first NPC frame must already block world input");
    now += 2001;
    assert.equal(JSON.parse(sink(JSON.stringify({ generation: 101, type: "acceptQuest" }))).accepted, false,
      "suspended intervals cannot leave a healthy flag authorizing queued input before the next tick");
    tick();
    assert.equal(state.current.ready, false, "a frozen ready status must release UI ownership");
    assert.equal(state.current.capturesPointer, false);
    assert.match(state.current.error, /stopped responding/);
    assert.equal(JSON.parse(sink(JSON.stringify({ generation: 101, type: "acceptQuest" }))).accepted, false,
      "queued UI input cannot act through a stopped renderer");
    applied = { ...applied, frame: 2, revision: submitted.at(-1).revision };
    tick();
    assert.equal(state.current.ready, true, "a resumed event loop may recover after a suspended browser tab");
    assert.equal(state.current.error, null);
    const validRevision = submitted.at(-1).revision;
    const submissionCount = submitted.length;
    model = { ...model, presentation: null };
    tick();
    assert.equal(state.current.ready, false);
    assert.equal(state.current.capturesPointer, false);
    assert.equal(submitted.length, submissionCount + 1, "geometry loss must explicitly hide the Rust surface");
    assert.equal(submitted.at(-1).presentation, null, "do not invent geometry for a hidden surface");
    assert.equal(submitted.at(-1).hostVisible, false);
    assert.equal(submitted.at(-1).inGame, true, "a resize must not impersonate session exit");
    tick();
    assert.equal(submitted.length, submissionCount + 1, "unchanged hidden models are not repeatedly sent");
    assert.equal(JSON.parse(sink(JSON.stringify({ generation: 101, type: "acceptQuest" }))).accepted, false);
    model = { ...model, presentation: { logicalWidth: 1664, logicalHeight: 768, stageCssScale: 0.5, touch: true } };
    tick();
    assert.equal(submitted.at(-1).generation, 101, "layout changes do not reset the session");
    assert.ok(submitted.at(-1).revision > validRevision, "recovery requires a newer model revision");
    assert.equal(state.current.ready, false, "old ready status cannot reclaim the canvas before applying new geometry");
    applied = { ...applied, frame: 3, revision: submitted.at(-1).revision };
    tick();
    assert.equal(state.current.ready, true);
    model = { ...model, generation: 102, inGame: false, presentation: null };
    tick();
    assert.equal(submitted.at(-1).generation, 102, "logout during invalid geometry must reach the current session");
    assert.equal(submitted.at(-1).inGame, false);
    assert.equal(submitted.at(-1).hostVisible, false);
  } finally {
    cleanup?.();
    if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
  }
});

test("packaged CJK font matches its pinned upstream bytes and retains its license", () => {
  const bytes = readFileSync(new URL("../public/original-ui/fonts/NotoSansCJKsc-Regular.otf", import.meta.url));
  assert.equal(bytes.length, 16437364);
  assert.equal(createHash("sha256").update(bytes).digest("hex"), "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b");
  assert.match(readFileSync(new URL("../public/original-ui/fonts/OFL-NotoSansCJK.txt", import.meta.url), "utf8"), /SIL OPEN FONT LICENSE Version 1\.1/);
});
