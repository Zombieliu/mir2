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
      if (name === "./bevy-quest-world-context") return load("../lib/bevy-quest-world-context.ts");
      if (["./bevy-quest-world-actions", "./bevy-quest-route-plan", "./quest-collision-cache",
        "./quest-world-lifecycle", "./quest-route-execution", "./bevy-quest-world-controls"].includes(name))
        return load(`../lib/${name.slice(2)}.ts`);
      if (name === "./bevy-bag-model") return load("../lib/bevy-bag-model.ts");
      if (name === "./world-model/item-identity") return load("../lib/world-model/item-identity.ts");
      throw new Error(`Unexpected dependency ${name}`);
    }, module.exports, module,
  );
  return module.exports;
}
const host = load("../lib/bevy-quest-ui.ts");
const worldContext = load("../lib/bevy-quest-world-context.ts");
const worldActions = load("../lib/bevy-quest-world-actions.ts");
const routePlan = load("../lib/bevy-quest-route-plan.ts");
const collision = load("../lib/quest-collision-cache.ts");
const worldLifecycle = load("../lib/quest-world-lifecycle.ts");
const routeExecution = load("../lib/quest-route-execution.ts");
const worldControls = load("../lib/bevy-quest-world-controls.ts");
const stage = load("../app/components/original-client-stage-presentation.ts");
const quest = { questId: 2110002, title: "N2", summary: "Equip a weapon", objective: "Weapon",
  current: 1, required: 1, acceptNpcIndex: 0, finishNpcIndex: 0 };

const questIdentity = { connectionGeneration: 2, sessionGeneration: 3, sceneRevision: 4,
  playerObjectId: 1000, mapFileName: "0" };
const questSupplyPlayer = { level: 5, className: "Taoist", gender: "female", gold: 0,
  currentWeight: 0, currentWeightKnown: false, maxWeight: 100 };
function completeQuestWorld() {
  return { connected: true, playerObjectId: "1000", mapFileName: "0.map", selectedObjectId: "44",
    entities: [{ objectId: "1000", kind: "selfPlayer", name: "Player", x: 1, y: 1,
      level: 5, classKey: "Taoist", genderKey: "female" },
      { objectId: "44", kind: "monster", name: "Hen", x: 2, y: 1, dead: false, hp: 12, maxHp: 20 }],
    groundDrops: [{ objectId: "80", name: "Gold", x: 3, y: 1, quantity: 2, sourceMonster: "Hen" }],
    inventoryCapacity: 46, maxBagSlots: 40, gold: 0, inventoryItems: [], beltItems: [], equipmentItems: [],
    currentWeight: 0, maxWeight: 100 };
}
function completeQuestDraft(generation = 7, mapIndex = null) {
  return worldContext.projectBevyQuestWorldDraft(completeQuestWorld(), questIdentity, questIdentity,
    generation, questSupplyPlayer, [], mapIndex);
}
function stampOf(context) {
  return Object.fromEntries(["generation", "revision", "connectionGeneration", "sessionGeneration",
    "sceneRevision", "playerObjectId", "mapFileName"].map(key => [key, context[key]]));
}

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

test("Quest world context requires a complete same-player projection and exact version capability", () => {
  const identity = { connectionGeneration: 2, sessionGeneration: 3, sceneRevision: 4,
    playerObjectId: 1000, mapFileName: "0" };
  const world = completeQuestWorld();
  const player = { level: 5, className: "Taoist", gender: "female", gold: 0,
    currentWeight: 0, currentWeightKnown: false, maxWeight: 100 };
  const draft = worldContext.projectBevyQuestWorldDraft(world, identity, identity, 7, player, [{ spell: "SoulFireBall" }]);
  assert.ok(draft);
  assert.equal(draft.inventory.gold, 0, "explicit zero is valid, not missing inventory");
  const stamp = worldContext.stampBevyQuestWorldContext(draft, 9);
  assert.ok(worldContext.sameQuestWorldStamp(stamp, { ...stamp }));
  assert.equal(worldContext.sameQuestWorldStamp({ ...stamp, mapFileName: "1" }, stamp), false);
  assert.equal(worldContext.sameQuestWorldStamp({ ...stamp, revision: 10 }, stamp), false);
  assert.equal(worldContext.sameQuestWorldDraft({ ...draft, inventory: { ...draft.inventory, gold: 1 } }, draft), false);
  assert.equal(worldContext.projectBevyQuestWorldDraft({ ...world, connected: false }, identity, identity, 7, player), null);
  assert.equal(worldContext.projectBevyQuestWorldDraft({ ...world, entities: [{ ...world.entities[0], kind: "player" }] }, identity, identity, 7, player), null);
  assert.equal(worldContext.projectBevyQuestWorldDraft({ ...world, inventoryCapacity: undefined }, identity, identity, 7, player), null);
  assert.equal(worldContext.projectBevyQuestWorldDraft(world, identity, { ...identity, sceneRevision: 5 }, 7, player), null);
  assert.equal(worldContext.projectBevyQuestWorldDraft(world, identity, identity, 7, { ...player, gender: undefined }), null);
  assert.equal(worldContext.supportsBevyQuestWorldContext(null), false);
  assert.equal(worldContext.supportsBevyQuestWorldContext({ getMir2QuestWorldContextVersion: () => "1" }), false);
  assert.equal(worldContext.supportsBevyQuestWorldContext({ getMir2QuestWorldContextVersion: () => { throw Error("old"); } }), false);
  assert.equal(worldContext.supportsBevyQuestWorldContext({ getMir2QuestWorldContextVersion: () => 1 }), false);
  assert.equal(worldContext.supportsBevyQuestWorldContext({ getMir2QuestWorldContextVersion: () => "2" }), false);
  assert.equal(worldContext.supportsBevyQuestWorldContext({ getMir2QuestWorldContextVersion: () => 2 }), true);
});

test("the actual Page capture retires raw Quest skills for self NewMagic only on the current connection", () => {
  const page = ts.createSourceFile("page.tsx", readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8"),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let capture;
  const visit = (node) => {
    if (ts.isFunctionDeclaration(node) && node.name?.text === "captureSpellsGatewayEvent") capture = node;
    ts.forEachChild(node, visit);
  };
  visit(page);
  assert.ok(capture);
  const compiled = ts.transpileModule(capture.getText(page), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  const skills = { current: true };
  const observe = new Function("equipmentConnectionGenerationRef", "questSkillsBaselineCurrentRef",
    "currentSpellsOwner", "combatRawRef", "worldRef", "combatMastersRef",
    `${compiled.outputText}\nreturn captureSpellsGatewayEvent;`)(
    { current: 2 }, skills, () => null, { current: null },
    { current: { playerObjectId: "1000", mapFileName: "0" } }, { current: new Map() });
  observe({ type: "packet", packet: "NewMagic", payload: { hero: false, spell: "SoulFireBall" } }, 1);
  assert.equal(skills.current, true, "a retired connection must not alter the current player baseline");
  observe({ type: "packet", packet: "NewMagic", payload: { hero: true, spell: "SoulFireBall" } }, 2);
  assert.equal(skills.current, true, "hero skill changes do not invalidate the player's known skills");
  observe({ type: "packet", packet: "NewMagic", payload: { hero: false, spell: "SoulFireBall" } }, 2);
  assert.equal(skills.current, false, "self learning stays unknown until the next complete authority baseline");
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
    "./bevy-quest-world-context": worldContext,
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
  const runtimeHolder = { current: runtime };
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  let visibilityListener;
  globalThis.document = { visibilityState: "visible",
    addEventListener(name, listener) { assert.equal(name, "visibilitychange"); visibilityListener = listener; },
    removeEventListener(name, listener) {
      assert.equal(name, "visibilitychange");
      assert.equal(listener, visibilityListener);
      visibilityListener = undefined;
    } };
  globalThis.window = { setInterval(fn) { tick = fn; return 1; }, clearInterval() {} };
  let cleanup;
  let hookView;
  try {
    const render = () => {
      refIndex = 0;
      hookView = hook({ requested: true, runtimeGeneration: 1, runtimeRef: runtimeHolder, snapshot: () => model,
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
    assert.ok(submitted.every((value) => !Object.hasOwn(value, "worldContext")),
      "an old bundle never receives an unknown world field");
    runtime.getMir2QuestWorldContextVersion = () => 2;
    const draft = completeQuestDraft(101);
    model = { ...model, worldDraft: draft, presentation: { ...model.presentation, touch: false } };
    tick();
    const context = submitted.at(-1).worldContext;
    assert.equal(context.revision, submitted.at(-1).revision);
    applied = { ...applied, frame: 4, revision: context.revision };
    tick();
    let paintedRects = [{ type: "attackQuestTarget", left: 10, top: 20, width: 80, height: 30 }];
    runtime.getMir2QuestWorldControlRects = () => JSON.stringify({ version: 1,
      stamp: submitted.at(-1)?.worldContext ? stampOf(submitted.at(-1).worldContext) : null,
      logicalWidth: model.presentation?.logicalWidth ?? 0, logicalHeight: model.presentation?.logicalHeight ?? 0,
      rects: paintedRects });
    tick();
    assert.equal(state.current.worldControlsReady, true);
    assert.equal(worldControls.questWorldControlAt(hookView.readLiveWorldControls(), 20, 30), true);
    assert.equal(worldControls.questWorldControlAt(hookView.readLiveWorldControls(), 500, 500), false,
      "a blank portion of the stage must stay available to ordinary world input");
    const measuredReader = runtime.getMir2QuestWorldControlRects;
    runtime.getMir2QuestWorldControlRects = () => "malformed";
    assert.equal(hookView.readLiveWorldControls(), null, "an unavailable measurement withdraws live controls immediately");
    assert.equal(worldControls.questWorldControlAt(hookView.readLiveWorldControlBlockers(), 20, 30), true,
      "a failed measurement preserves only the last actually painted blocker");
    runtime.getMir2QuestWorldControlRects = measuredReader;
    const mapIntent = { ...stampOf(context), type: "openDestinationMap" };
    assert.equal(JSON.parse(sink(JSON.stringify(mapIntent))).accepted, true);
    model = { ...model, worldDraft: { ...draft, inventory: { ...draft.inventory, gold: 1 } } };
    assert.equal(hookView.readLiveWorldControls(), null, "a full live draft mutation retires old hit rectangles synchronously");
    assert.equal(worldControls.questWorldControlAt(hookView.readLiveWorldControlBlockers(), 20, 30), true,
      "the last actually painted button still swallows a gesture while its action proof is stale");
    assert.equal(worldControls.questWorldControlAt(hookView.readLiveWorldControlBlockers(), 500, 500), false,
      "stale paint never blocks blank canvas");
    assert.equal(JSON.parse(sink(JSON.stringify(mapIntent))).accepted, false,
      "a live inventory change retires the applied map proof before the next polling tick");
    model = { ...model, worldDraft: draft };
    const originalQuests = model.quests;
    model = { ...model, quests: [{ questIndex: 23, status: "inProgress" }] };
    assert.equal(hookView.readLiveWorldControls(), null, "a changed Quest source cannot reuse an old tracker rectangle");
    assert.equal(JSON.parse(sink(JSON.stringify(mapIntent))).accepted, false);
    model = { ...model, quests: originalQuests };
    runtimeHolder.current = { ...runtime };
    assert.equal(hookView.readLiveWorldControls(), null);
    assert.equal(hookView.readLiveWorldControlBlockers(), null, "the painted cache belongs to the exact runtime object");
    assert.equal(JSON.parse(sink(JSON.stringify(mapIntent))).accepted, false,
      "an old sink cannot act after a runtime replacement");
    runtimeHolder.current = runtime;
    globalThis.document.visibilityState = "hidden";
    assert.equal(hookView.readLiveWorldControls(), null);
    assert.equal(hookView.readLiveWorldControlBlockers(), null, "hidden paint cannot capture a new gesture");
    assert.equal(JSON.parse(sink(JSON.stringify(mapIntent))).accepted, false,
      "visibility blocks an old callback immediately, before either interval or listener runs");
    visibilityListener();
    assert.equal(submitted.at(-1).hostVisible, false);
    assert.equal(Object.hasOwn(submitted.at(-1), "worldContext"), false);
    assert.equal(state.current.ready, false);
    globalThis.document.visibilityState = "visible";
    visibilityListener();
    const resumed = submitted.at(-1).worldContext;
    assert.ok(resumed.revision > context.revision);
    const resumedIntent = { ...stampOf(resumed), type: "openDestinationMap" };
    assert.equal(hookView.readLiveWorldControls(), null, "new revision cannot borrow the previously applied rectangles");
    assert.equal(JSON.parse(sink(JSON.stringify(resumedIntent))).accepted, false,
      "visibility recovery must wait for the new world context to be applied");
    applied = { ...applied, frame: 5, revision: resumed.revision };
    tick();
    assert.equal(JSON.parse(sink(JSON.stringify(mapIntent))).accepted, false,
      "the old stamp cannot revive after visibility recovery");
    assert.equal(JSON.parse(sink(JSON.stringify(resumedIntent))).accepted, true);
    paintedRects = [];
    tick();
    assert.equal(hookView.readLiveWorldControls(), null, "an actual layout with no controls authorizes no button");
    assert.deepEqual(hookView.readLiveWorldControlBlockers()?.rects, [],
      "an applied empty layout replaces old painted rectangles instead of keeping a stale button");
    model = { ...model, generation: 102, inGame: false, presentation: null };
    assert.equal(hookView.readLiveWorldControlBlockers(), null, "session exit immediately withdraws paint blockers");
    tick();
    assert.equal(submitted.at(-1).generation, 102, "logout during invalid geometry must reach the current session");
    assert.equal(submitted.at(-1).inGame, false);
    assert.equal(submitted.at(-1).hostVisible, false);
  } finally {
    cleanup?.();
    if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
    if (previousDocument === undefined) delete globalThis.document; else globalThis.document = previousDocument;
  }
});

test("v2 Quest world preserves nullable facts, canonical IDs and unknown current map", () => {
  const world = completeQuestWorld();
  world.entities[1] = { ...world.entities[1], dead: undefined, hp: undefined, maxHp: undefined };
  world.bigMapIndex = 0;
  const draft = worldContext.projectBevyQuestWorldDraft(world, questIdentity, questIdentity, 7, questSupplyPlayer);
  assert.ok(draft);
  assert.equal(draft.mapIndex, null, "an absent authoritative map index must not become image index or map 0");
  assert.deepEqual([draft.entities[1].dead, draft.entities[1].hp, draft.entities[1].maxHp], [null, null, null]);
  assert.equal(draft.entities[0].objectId, 1000);
  assert.equal(draft.selectedObjectId, 44);
  assert.deepEqual(draft.groundDrops[0], { objectId: 80, name: "Gold", x: 3, y: 1, quantity: 2, sourceMonster: "Hen" });
  const project = value => worldContext.projectBevyQuestWorldDraft(value, questIdentity, questIdentity, 7, questSupplyPlayer);
  assert.equal(project({ ...world, entities: [...world.entities, world.entities[1]] }), null);
  assert.equal(project({ ...world, entities: [world.entities[0], { ...world.entities[1], kind: "selfPlayer" }] }), null);
  assert.equal(project({ ...world, groundDrops: [...world.groundDrops, world.groundDrops[0]] }), null);
  assert.equal(project({ ...world, groundDrops: [{ ...world.groundDrops[0], objectId: "44" }] }), null);
  assert.equal(project({ ...world, selectedObjectId: undefined }), null);
  assert.equal(project({ ...world, groundDrops: undefined }), null);
  for (const invalid of ["0", "044", "4e1", "4294967296", 44]) {
    assert.equal(project({ ...world, entities: [world.entities[0], { ...world.entities[1], objectId: invalid }] }), null);
  }
  assert.equal(project({ ...world, groundDrops: [{ ...world.groundDrops[0], quantity: 0 }] }), null);
  assert.equal(project({ ...world, entities: [world.entities[0], ...Array.from({ length: 512 }, (_, index) => ({ ...world.entities[1], objectId: String(index + 2000) }))] }), null);
  assert.equal(project({ ...world, groundDrops: Array.from({ length: 513 }, (_, index) => ({ ...world.groundDrops[0], objectId: String(index + 5000) })) }), null);
  for (const changed of [
    { ...draft, mapIndex: 0 }, { ...draft, selectedObjectId: null },
    { ...draft, entities: draft.entities.map(row => ({ ...row, x: row.x + 1 })) },
    { ...draft, groundDrops: draft.groundDrops.map(row => ({ ...row, y: row.y + 1 })) },
  ]) assert.equal(worldContext.sameQuestWorldDraft(changed, draft), false);
});

test("local Quest actions require exact seven-field stamp, role and nested route payload", () => {
  const stamp = stampOf(worldContext.stampBevyQuestWorldContext(completeQuestDraft(), 9));
  const route = { ...stamp, type: "navigateQuestRoute", questIndex: 23, resetEpoch: 4,
    mapIndex: 0, x: 5, y: 6, routeTarget: { type: "huntRegion", monsterIndex: 8, radius: 2 } };
  const candidates = [{ ...stamp, type: "openDestinationMap" }, route,
    { ...stamp, type: "attackTarget", objectId: 44 }, { ...stamp, type: "attackQuestTarget", objectId: 44 },
    { ...stamp, type: "pickUpObject", objectId: 80, x: 3, y: 1 }, { ...stamp, type: "pickUpTile", x: 1, y: 1 }];
  for (const value of candidates) {
    assert.deepEqual(worldActions.parseQuestWorldAction(value), value);
    assert.equal(worldActions.parseQuestWorldAction({ ...value, extra: true }), null);
    for (const key of Object.keys(stamp)) {
      const missing = { ...value }; delete missing[key];
      assert.equal(worldActions.parseQuestWorldAction(missing), null, `missing stamp ${key}`);
    }
  }
  for (const invalid of [
    { ...route, resetEpoch: 5 }, { ...route, mapIndex: null }, { ...route, x: 0.5 },
    { ...route, playerObjectId: 0 }, { ...route, mapFileName: "0.map" },
    { ...route, routeTarget: { type: "entrance", radius: 2 } },
    { ...route, routeTarget: { type: "huntRegion", monsterIndex: 8, radius: 2, vendor: "potions" } },
    { ...route, routeTarget: { type: "supply", vendor: "unknown" } },
    { ...stamp, type: "attackTarget", objectId: 0 }, { ...stamp, type: "attackTarget", objectId: "44" },
    { ...stamp, type: "attackQuestTarget", objectId: 44, x: 2 },
    { ...stamp, type: "pickUpObject", objectId: 80, x: 3 },
    { ...stamp, type: "pickUpTile", x: 1, y: 1, objectId: 80 },
    { ...stamp, type: "attack", objectId: 44 },
  ]) assert.equal(worldActions.parseQuestWorldAction(invalid), null);
  assert.ok(worldActions.parseQuestWorldAction({ ...route, routeTarget: { type: "entrance" } }));
  assert.ok(worldActions.parseQuestWorldAction({ ...route, routeTarget: { type: "supply", vendor: "poison" } }));
});

test("ordinary map packets retire same-map transitions and images never supply map authority", () => {
  const observed = worldLifecycle.observeQuestMapPacket(null, "MapInformation", { fileName: "0.map", mapIndex: 39, bigMap: 7 }, 2, 3, 4, "0");
  assert.equal(observed.retired, false);
  assert.equal(worldLifecycle.currentQuestMapIndex(observed.authority, questIdentity), 39);
  const sameMap = worldLifecycle.observeQuestMapPacket(observed.authority, "MapChanged", { fileName: "0.map", mapIndex: 39 }, 2, 3, 4, "0");
  assert.equal(sameMap.retired, true);
  assert.equal(sameMap.sceneRevision, 5, "same filename/index re-entry still retires object IDs and movement");
  assert.equal(worldLifecycle.currentQuestMapIndex(sameMap.authority, questIdentity), null);
  assert.equal(worldLifecycle.currentQuestMapIndex(sameMap.authority, { ...questIdentity, sceneRevision: 5 }), 39);
  for (const payload of [{ fileName: "0.map", bigMap: 39 }, { fileName: "0.map", mapIndex: null },
    { fileName: "0.map", mapIndex: -1 }, { mapIndex: 39 }, { fileName: "0.map", mapIndex: "39" }]) {
    const missing = worldLifecycle.observeQuestMapPacket(observed.authority, "MapInformation", payload, 2, 3, 4, "0");
    assert.equal(missing.authority, null);
    assert.equal(missing.retired, true);
  }
  const mismatch = worldLifecycle.observeQuestMapPacket(observed.authority, "MapInformation", { fileName: "1.map", mapIndex: 0 }, 2, 3, 4, "0");
  assert.equal(worldLifecycle.currentQuestMapIndex(mismatch.authority, questIdentity), null);
  assert.equal(worldLifecycle.currentQuestMapIndex(observed.authority, { ...questIdentity, connectionGeneration: 9 }), null);
});

test("Quest handoffs retire each lifetime dimension and attack roles keep raw life evidence", () => {
  const captured = { identity: questIdentity, generation: 7, runtime: {}, runtimeGeneration: 1, socket: {}, questSource: "quests-a" };
  assert.equal(worldLifecycle.sameQuestWorldLifetime(captured, { ...captured, identity: { ...questIdentity } }), true);
  for (const changed of [{ ...captured, socket: {} }, { ...captured, runtime: {} },
    { ...captured, runtimeGeneration: 2 }, { ...captured, generation: 8 },
    { ...captured, questSource: "quests-b" }, { ...captured, identity: { ...questIdentity, sceneRevision: 5 } },
    { ...captured, identity: { ...questIdentity, sessionGeneration: 9 } },
    { ...captured, identity: { ...questIdentity, playerObjectId: 1001 } }, null]) {
    assert.equal(worldLifecycle.sameQuestWorldLifetime(captured, changed), false);
  }
  const target = completeQuestWorld().entities[1];
  const valid = (row, role = "attackQuestTarget", selected = null) => worldLifecycle.questAttackTargetCurrent(row, 44, role, selected, "Hen");
  assert.equal(valid(target), true);
  assert.equal(valid(target, "attackTarget", "44"), true);
  assert.equal(valid(target, "attackTarget", "45"), false);
  assert.equal(valid({ ...target, dead: undefined }), false);
  assert.equal(valid({ ...target, dead: null }), false);
  assert.equal(valid({ ...target, dead: true }), false);
  assert.equal(valid({ ...target, hp: null }), false, "known positive max HP needs positive current HP");
  assert.equal(valid({ ...target, hp: 0 }), false);
  assert.equal(valid({ ...target, maxHp: 0 }), false);
  assert.equal(valid({ ...target, maxHp: -1 }), false);
  assert.equal(valid({ ...target, hp: null, maxHp: null }), true, "explicit raw alive may authorize only the quest role");
  assert.equal(valid({ ...target, hp: null, maxHp: null }, "attackTarget", "44"), false);
  assert.equal(valid({ ...target, kind: "player" }), false);
  assert.equal(valid({ ...target, name: "Other" }), false);
  assert.equal(valid({ ...target, objectId: "45" }), false);
});

function routeFixture() {
  const action = { ...stampOf(worldContext.stampBevyQuestWorldContext(completeQuestDraft(7, 0), 9)),
    type: "navigateQuestRoute", questIndex: 23, resetEpoch: 4, mapIndex: 0, x: 4, y: 1,
    routeTarget: { type: "entrance" } };
  const geometry = { mapFileName: "0", width: 8, height: 4, fingerprint: "a".repeat(64), blockedBits: new Uint8Array(4) };
  const origin = { x: 1, y: 1 };
  const result = { version: 1, ok: true, stamp: stampOf(action), mapIndex: 0, mapFileName: "0", origin,
    destination: { x: 4, y: 1 }, steps: [{ x: 2, y: 1 }, { x: 3, y: 1 }, { x: 4, y: 1 }], radius: 0 };
  return { action, geometry, origin, result };
}
function blockedGeometry(geometry, point) {
  const blockedBits = new Uint8Array(geometry.blockedBits);
  const bit = point.x * geometry.height + point.y;
  blockedBits[bit >>> 3] |= 1 << (bit & 7);
  return { ...geometry, blockedBits };
}

test("the shared route adapter verifies a complete JS-fake WASM response against collision and exact endpoint", () => {
  const { action, geometry, origin, result } = routeFixture();
  let queried;
  const runtime = { getMir2QuestRoutePlan(json, bits) { queried = JSON.parse(json); assert.equal(bits, geometry.blockedBits); return JSON.stringify(result); } };
  const planned = routePlan.planBevyQuestRoute(runtime, action, geometry, origin, [], []);
  assert.deepEqual(planned, { origin, destination: result.destination, steps: result.steps, radius: 0 });
  assert.deepEqual(queried.stamp, stampOf(action));
  assert.deepEqual(queried.routeTarget, action.routeTarget);
  assert.equal(queried.width, 8);
  const check = (response, geom = geometry, occupied = [], edges = [], routed = action) =>
    routePlan.planBevyQuestRoute({ getMir2QuestRoutePlan: () => JSON.stringify(response) }, routed, geom, origin, occupied, edges);
  for (const invalid of [{ ...result, ok: false }, { ...result, version: 2 },
    { ...result, stamp: { ...result.stamp, revision: 10 } }, { ...result, mapIndex: 1 },
    { ...result, origin: { x: 0, y: 1 } }, { ...result, steps: result.steps.slice(0, 2) },
    { ...result, steps: [{ x: 4, y: 1 }] },
    { ...result, destination: { x: 3, y: 1 }, steps: result.steps.slice(0, 2), radius: 1 },
    { ...result, destination: origin, steps: [], radius: 3 },
  ]) assert.equal(check(invalid), null, "no partial endpoint or invented radius may authorize an entrance route");
  assert.equal(check(result, blockedGeometry(geometry, { x: 3, y: 1 })), null);
  assert.equal(check(result, geometry, [{ x: 3, y: 1 }]), null);
  assert.equal(check(result, geometry, [], [{ from: { x: 2, y: 1 }, to: { x: 3, y: 1 } }]), null);
  assert.equal(check(result, { ...geometry, mapFileName: "1" }), null);
  assert.equal(routePlan.planBevyQuestRoute({}, action, geometry, origin, [], []), null);
  assert.equal(routePlan.planBevyQuestRoute({ getMir2QuestRoutePlan: () => "bad" }, action, geometry, origin, [], []), null);
  const arrived = { ...result, destination: origin, steps: [] };
  const originAction = { ...action, x: origin.x, y: origin.y };
  assert.ok(check(arrived, geometry, [], [], originAction));
  assert.equal(check(arrived, blockedGeometry(geometry, origin), [], [], originAction), null);
  assert.equal(check(arrived, geometry, [origin], [], originAction), null);
  const hunt = { ...action, x: 5, routeTarget: { type: "huntRegion", monsterIndex: 8, radius: 2 } };
  assert.ok(check({ ...result, radius: 2 }, geometry, [], [], hunt));
  assert.equal(check({ ...result, radius: 3 }, geometry, [], [], hunt), null);
  const supply = { ...action, x: 5, routeTarget: { type: "supply", vendor: "potions" } };
  assert.ok(check({ ...result, radius: 2 }, geometry, [], [], supply));
  assert.equal(check({ ...result, radius: 1 }, geometry, [], [], supply), null);
});

test("route execution follows bends, validates each 2/3-tile run and replans deviations", () => {
  const plan = { origin: { x: 1, y: 1 }, destination: { x: 4, y: 3 }, radius: 0,
    steps: [{ x: 2, y: 1 }, { x: 3, y: 1 }, { x: 4, y: 1 }, { x: 4, y: 2 }, { x: 4, y: 3 }] };
  const visited = [];
  const free = (from, to) => { visited.push([from, to]); return false; };
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, plan.origin, 3, free), { type: "move", point: { x: 4, y: 1 }, direction: "right", mode: "run" });
  assert.equal(visited.length, 3, "a 3-tile run checks all three authoritative edges");
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, plan.origin, 2, () => false), { type: "move", point: { x: 3, y: 1 }, direction: "right", mode: "run" });
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, { x: 3, y: 1 }, 2, () => false), { type: "move", point: { x: 4, y: 1 }, direction: "right", mode: "walk" }, "turning paths cannot be flattened into a straight run");
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, { x: 4, y: 1 }, 2, () => false), { type: "move", point: plan.destination, direction: "down", mode: "run" });
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, plan.origin, 3, (_from, to) => to.x === 3), { type: "move", point: { x: 2, y: 1 }, direction: "right", mode: "walk" });
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, { x: 2, y: 1 }, 1, () => true), { type: "replan" });
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, { x: 1, y: 2 }, 3, () => false), { type: "replan" });
  assert.deepEqual(routeExecution.nextQuestRouteStep(plan, plan.destination, 3, () => false), { type: "arrived" });
});

function fakeCollisionChunk(url, width = 258, height = 3, overrides = {}) {
  const query = new URL(url, "https://fixture.invalid").searchParams;
  const bounds = { minX: Number(query.get("minX")), maxX: Math.min(Number(query.get("maxX")), width - 1),
    minY: Number(query.get("minY")), maxY: Math.min(Number(query.get("maxY")), height - 1) };
  const cells = [{ x: 1, y: 2 }, { x: 256, y: 1 }].filter(cell => cell.x >= bounds.minX && cell.x <= bounds.maxX && cell.y >= bounds.minY && cell.y <= bounds.maxY);
  return { schemaVersion: 1, source: "crystalMap", mapFileName: query.get("map"), mapWidth: width,
    mapHeight: height, geometryFingerprint: "a".repeat(64), bounds, blockedCells: cells, ...overrides };
}

test("collision cache uses injected fake chunks, reuses only complete geometry and discards partial failures", async () => {
  let release;
  const first = new Promise(resolve => { release = resolve; });
  const requested = [];
  const cache = new collision.QuestCollisionCache(async url => {
    requested.push(url);
    if (requested.length === 1) await first;
    return { ok: true, json: async () => fakeCollisionChunk(url) };
  });
  const pending = cache.load("0"), second = cache.load("0");
  assert.equal(pending, second, "concurrent actions share a geometry promise without carrying a player lifetime");
  assert.equal(requested.length, 1);
  release();
  const geometry = await pending;
  assert.equal(requested.length, 2);
  assert.deepEqual([geometry.width, geometry.height, geometry.fingerprint], [258, 3, "a".repeat(64)]);
  assert.equal(geometry.blockedBits.length, Math.ceil(258 * 3 / 8));
  for (const point of [{ x: 1, y: 2 }, { x: 256, y: 1 }]) {
    const bit = point.x * 3 + point.y;
    assert.notEqual(geometry.blockedBits[bit >>> 3] & 1 << (bit & 7), 0, "x-major cell indexing");
  }
  const yMajor = 2 * 258 + 1;
  assert.equal(geometry.blockedBits[yMajor >>> 3] & 1 << (yMajor & 7), 0);
  assert.equal(Object.hasOwn(geometry, "stamp"), false);
  assert.equal(await cache.load("0"), geometry);
  assert.equal(requested.length, 2);
  for (const fail of ["unavailable", "fingerprint", "bounds", "dimension", "duplicate"]) {
    let failed = true, calls = 0;
    const subject = new collision.QuestCollisionCache(async url => {
      calls++;
      const row = fakeCollisionChunk(url);
      const later = row.bounds.minX === 256;
      if (failed && fail === "unavailable" && later) return { ok: false, json: async () => row };
      if (failed && fail === "fingerprint" && later) row.geometryFingerprint = "b".repeat(64);
      if (failed && fail === "bounds" && later) row.bounds.maxX--;
      if (failed && fail === "dimension" && later) row.mapHeight++;
      if (failed && fail === "duplicate") row.blockedCells.push(...row.blockedCells);
      return { ok: true, json: async () => row };
    });
    await assert.rejects(subject.load("0"));
    const oldCalls = calls; failed = false;
    const recovered = await subject.load("0");
    assert.equal(calls, oldCalls + 2, `${fail} must not leave a partially cached first chunk`);
    assert.equal(recovered.fingerprint, "a".repeat(64));
  }
  await assert.rejects(cache.load("../0"));
  await assert.rejects(cache.load("0.map"));
});

test("world control reader requires actual current stamped rectangles and claims only button hits", () => {
  const stamp = stampOf(worldContext.stampBevyQuestWorldContext(completeQuestDraft(), 9));
  const rect = { type: "attackQuestTarget", left: 100.25, top: 70.125, width: 80.5, height: 30.25 };
  const result = { version: 1, stamp, logicalWidth: 1024, logicalHeight: 768, rects: [rect] };
  const read = value => worldControls.readQuestWorldControls({ getMir2QuestWorldControlRects: () => JSON.stringify(value) }, stamp, 1024, 768);
  const controls = read(result);
  assert.deepEqual(controls, result);
  assert.equal(worldControls.questWorldControlAt(controls, 100.25, 70.125), true);
  assert.equal(worldControls.questWorldControlAt(controls, 180.75, 75), false, "right edge is outside the actual button");
  assert.equal(worldControls.questWorldControlAt(controls, 500, 500), false, "a transparent stage never becomes one giant input blocker");
  assert.equal(worldControls.questWorldControlAt(controls, NaN, 75), false);
  assert.equal(worldControls.questWorldControlAt(null, 100.25, 75), false);
  for (const changed of [{ ...result, version: 2 }, { ...result, stamp: null },
    { ...result, stamp: { ...stamp, revision: 10 } }, { ...result, stamp: { ...stamp, sceneRevision: 5 } },
    { ...result, stamp: { ...stamp, extra: true } }, { ...result, logicalWidth: 1023 },
    { ...result, rects: [] }, { ...result, rects: [{ ...rect, type: "attack" }] },
    { ...result, rects: [{ ...rect, width: 0 }] }, { ...result, rects: [{ ...rect, width: NaN }] },
    { ...result, rects: [{ ...rect, left: -1 }] },
    { ...result, rects: [{ ...rect, left: 1024 }] }, { ...result, rects: [{ ...rect, extra: true }] },
    { ...result, rects: Array(129).fill(rect) }, { ...result, extra: true }]) assert.equal(read(changed), null);
  assert.equal(worldControls.readQuestWorldControls({}, stamp, 1024, 768), null);
  assert.equal(worldControls.readQuestWorldControls({ getMir2QuestWorldControlRects: () => "malformed" }, stamp, 1024, 768), null);
  assert.equal(worldControls.readQuestWorldControls({ getMir2QuestWorldControlRects: () => { throw Error("old"); } }, stamp, 1024, 768), null);
  const empty = { ...result, rects: [] };
  assert.deepEqual(worldControls.readQuestWorldControls({ getMir2QuestWorldControlRects: () => JSON.stringify(empty) },
    stamp, 1024, 768, true), empty, "only the explicit painted-layout reader may accept measured empty controls");
  assert.equal(worldControls.readQuestWorldControls({ getMir2QuestWorldControlRects: () => JSON.stringify({ ...empty,
    stamp: { ...stamp, revision: 10 } }) }, stamp, 1024, 768, true), null,
    "allowEmpty never relaxes the exact applied stamp");
});

test("actual Page queued movement preserves replacement intent and retires stale Quest routes before ordinary A* or send", () => {
  const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const names = ["trySendQueuedCrystalMove", "observeQuestWorldActions", "cancelQuestRoute", "cancelPendingPickup"];
  const declarations = [];
  function visit(node) {
    if (ts.isFunctionDeclaration(node) && node.name && names.includes(node.name.text)) declarations.push(node.getText(ast));
    ts.forEachChild(node, visit);
  }
  visit(ast);
  assert.equal(declarations.length, names.length);
  const code = ts.transpileModule(declarations.join("\n"), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  for (const scenario of ["observeLifetime", "observeMap", "observeReplacement", "observeStaleReplacement",
    "predictionReplacement", "predictionRouteReplacement", "predictionPickupLifetime", "unchanged",
    ...["connectionGeneration", "sessionGeneration", "sceneRevision", "playerObjectId", "mapFileName",
      "generation", "runtime", "runtimeGeneration", "socket", "questSource"].map(field => "predictionLifetime:" + field)] ) {
    const queued = { kind: "target", targetX: 5, targetY: 1, requestedMode: "walk", requestedAt: 1, consumeAfterSend: true };
    const replacement = { ...queued, targetX: 8, requestedAt: 2 };
    const lifetime = { identity: questIdentity, generation: 7, runtime: {}, runtimeGeneration: 1, socket: {}, questSource: "quests" };
    let live = lifetime, observations = 0, ordinaryPlans = 0, questPlans = 0, predictions = 0, clears = 0;
    const sends = [], marker = {}, ref = current => ({ current });
    const route = { queued, lifetime, action: { mapIndex: 39 } };
    const deps = {
      queuedMoveIntentRef: ref(queued), questRouteRunRef: ref(route), pendingPickupRef: ref(null),
      questAttackHandoffRef: ref(null), questMapAuthorityRef: ref({ ...questIdentity, mapIndex: 39 }),
      movementPlanRef: ref(marker), worldRef: ref({ entities: [], groundDrops: [], mapTransfers: [], mapFileName: "0" }),
      questWindowOpenRef: ref(false), runtimeRef: ref({}), bevyQuestGenerationRef: ref(7), questWorldBlockedRef: ref(false),
      pendingSelfMoveRef: ref(null), pendingSelfTurnRef: ref(null), crystalRunPrimedUntilRef: ref(0),
      movementInputBlockedUntilRef: ref(0), movementBlockedStepsRef: ref([]), nextMoveSendAtRef: ref(0),
      directionStepNextAtRef: ref(0), directionStepVisualUntilRef: ref(0), pendingTransferRef: ref(null),
      doorOpenRequestUntilRef: ref(new Map()),
      document: { visibilityState: "visible", hasFocus: () => true },
      MOVEMENT_PENDING_ACTION_MAX_AGE_MS: 1000, MOVEMENT_QUEUED_DIRECTION_MAX_AGE_MS: 1000,
      CRYSTAL_CORRECTION_BLOCK_MS: 100, CRYSTAL_DOOR_OPEN_RETRY_MS: 100,
      sceneInputDeferredForInitialAssets: () => false, readBevyQuestUiStatus: () => null,
      currentQuestMapIndex: worldLifecycle.currentQuestMapIndex, sameQuestWorldLifetime: worldLifecycle.sameQuestWorldLifetime,
      questAttackTargetCurrent: worldLifecycle.questAttackTargetCurrent,
      currentQuestWorldLifetime: () => {
        observations++;
        if (observations === 1) {
          if (scenario === "observeLifetime" || scenario === "observeStaleReplacement") live = { ...lifetime, socket: {} };
          if (scenario === "observeMap") deps.questMapAuthorityRef.current = { ...deps.questMapAuthorityRef.current, mapIndex: 40 };
          if (scenario === "observeReplacement" || scenario === "observeStaleReplacement") deps.queuedMoveIntentRef.current = replacement;
        }
        return live;
      },
      currentAuthoritativeSelf: () => ({ x: 1, y: 1, direction: "east" }),
      canSendMovement: () => true, readSelfMovementControllerState: () => ({}),
      crystalEffectiveMovementMode: mode => mode,
      questRouteMovement: () => { questPlans++; return { point: { x: 2, y: 1 }, direction: "east", mode: "walk" }; },
      crystalMovementActionTowardWithRouteHints: () => { ordinaryPlans++; throw Error("old Quest queue reached ordinary A*"); },
      crystalMovementActionForDirection: () => { throw Error("unexpected direction intent"); },
      recentMovementBlockedSteps: () => [], originalMapClosedDoorIndexOnMovementPath: () => null,
      transferKeyForWorldTile: () => null, crystalSelfMovementTraits: () => ({}),
      crystalMovementProfile: () => ({ durationMs: 100, phaseCount: 4 }),
      createPendingSelfMove: input => ({ ...input, mode: "walk", to: { x: 2, y: 1 }, sentAt: input.now }),
      clearLegacySelfMovementCoordinateSources: () => {}, clearLocalSelfPrediction: () => { clears++; },
      setPredictedPlayerMotion: () => {
        predictions++;
        if (scenario === "predictionReplacement") deps.queuedMoveIntentRef.current = replacement;
        if (scenario === "predictionRouteReplacement") {
          deps.queuedMoveIntentRef.current = replacement;
          deps.questRouteRunRef.current = { ...route, queued: replacement };
        }
        if (scenario.startsWith("predictionLifetime:")) {
          const field = scenario.split(":")[1];
          live = Object.hasOwn(lifetime.identity, field)
            ? { ...lifetime, identity: { ...lifetime.identity, [field]: field === "mapFileName" ? "1" : lifetime.identity[field] + 1 } }
            : { ...lifetime, [field]: field === "runtime" || field === "socket" ? {}
              : field === "questSource" ? "changed quests" : lifetime[field] + 1 };
        }
        if (scenario === "predictionPickupLifetime") live = { ...lifetime, runtime: {} };
      },
      send: command => { sends.push(command); return true; }, scheduleMovementConfirmTick: () => {},
      observeBevyMovementShadowCommand: () => {}, bumpCorrectionCounter: () => {},
      rememberBlockedDirectionAtSource: () => {}, sendCrystalTurn: () => { throw Error("unexpected turn"); },
    };
    if (scenario === "predictionPickupLifetime") {
      deps.questRouteRunRef.current = null;
      deps.pendingPickupRef.current = { queued, lifetime, objectId: "80", x: 3, y: 1, name: "Gold", sourceMonster: "Hen" };
      deps.worldRef.current.groundDrops = [{ ...deps.pendingPickupRef.current }];
      deps.crystalMovementActionTowardWithRouteHints = () => { ordinaryPlans++; return { point: { x: 2, y: 1 }, direction: "east", mode: "walk" }; };
    }
    const api = new Function(...Object.keys(deps), code + ";return {" + names.join(",") + "};")(...Object.values(deps));
    const accepted = api.trySendQueuedCrystalMove(10);
    assert.equal(accepted, scenario === "unchanged", scenario);
    assert.equal(sends.length, scenario === "unchanged" ? 1 : 0, scenario + " sends only current proof");
    assert.equal(ordinaryPlans, scenario === "predictionPickupLifetime" ? 1 : 0, scenario + " cannot fall back to old ordinary A*");
    const replaced = scenario.includes("Replacement");
    assert.equal(deps.queuedMoveIntentRef.current, replaced ? replacement : null, scenario + " retains only the new manual queue");
    if (replaced) assert.equal(deps.movementPlanRef.current, marker, scenario + " cancellation cannot clear new movement state");
    if (scenario.startsWith("observe")) {
      assert.equal(questPlans, 0, scenario + " identity is rechecked immediately after observation");
      assert.equal(predictions, 0, scenario + " cannot create prediction from retired input");
    } else if (scenario !== "unchanged") {
      assert.equal(deps.pendingSelfMoveRef.current, null, scenario + " removes obsolete prediction before send");
      assert.equal(clears, 1, scenario);
    }
    if (scenario === "predictionRouteReplacement") assert.equal(deps.questRouteRunRef.current?.queued, replacement,
      "the old send attempt cannot cancel a synchronously installed new route");
    if (scenario === "predictionPickupLifetime") assert.equal(deps.pendingPickupRef.current, null);
    if (["observeLifetime", "observeMap", "observeStaleReplacement"].includes(scenario) || scenario.startsWith("predictionLifetime:"))
      assert.equal(deps.questRouteRunRef.current, null, scenario + " retires old route authority");
  }
});

test("packaged CJK font matches its pinned upstream bytes and retains its license", () => {
  const bytes = readFileSync(new URL("../public/original-ui/fonts/NotoSansCJKsc-Regular.otf", import.meta.url));
  assert.equal(bytes.length, 16437364);
  assert.equal(createHash("sha256").update(bytes).digest("hex"), "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b");
  assert.match(readFileSync(new URL("../public/original-ui/fonts/OFL-NotoSansCJK.txt", import.meta.url), "utf8"), /SIL OPEN FONT LICENSE Version 1\.1/);
});
