// Behavioral tests for lib/tutorial-steps.ts — the pure state machine behind the
// net-new beginner tutorial overlay.
//
// We drive the reducer with synthetic action/window/navigation events and assert
// that steps advance exactly when their trigger is satisfied (and not before),
// that count-gated steps require the full count, that aggregated action types
// work, and that skip / finish reach a terminal `done` state.
//
// Pure logic only: no DOM, no React. The .ts source is transpiled in-memory via
// the `typescript` devDependency (same harness as the other test-*.mjs scripts).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

const mod = loadTypeScriptModule(new URL("../lib/tutorial-steps.ts", import.meta.url));

const {
  TUTORIAL_STEPS,
  TUTORIAL_VERSION,
  createTutorialState,
  currentStep,
  pickText,
  reduceTutorial,
  tutorialCompletionStorageKey,
  tutorialStepsForInput,
} = mod;
const INPUT_PROFILES = ["keyboardMouse", "touch", "gamepad"];

let passed = 0;
function check(label, fn) {
  fn();
  passed += 1;
}

// Helper: index of a step id in the canonical list.
function indexOf(id) {
  const i = TUTORIAL_STEPS.findIndex((s) => s.id === id);
  assert.ok(i >= 0, `step "${id}" must exist`);
  return i;
}

// Helper: replay a sequence of events from a fresh state.
function run(events, input = "keyboardMouse") {
  return events.reduce((state, event) => reduceTutorial(state, event), createTutorialState(input));
}

check("initial state starts at step 0, not done", () => {
  const state = createTutorialState();
  assert.equal(state.stepIndex, 0);
  assert.equal(state.done, false);
  assert.equal(currentStep(state).id, "welcome");
});

check("each input profile has its own intro and completion flow", () => {
  const expectedFirst = {
    keyboardMouse: "welcome",
    touch: "touch-welcome",
    gamepad: "gamepad-welcome",
  };
  for (const input of INPUT_PROFILES) {
    const steps = tutorialStepsForInput(input);
    const state = createTutorialState(input);
    assert.equal(state.input, input);
    assert.equal(currentStep(state).id, expectedFirst[input]);
    assert.equal(steps[0].trigger.kind, "manual");
    assert.equal(steps.at(-1).trigger.kind, "manual");
  }
});

check("completion storage is versioned and isolated per input profile", () => {
  const keys = INPUT_PROFILES.map(tutorialCompletionStorageKey);
  assert.equal(new Set(keys).size, INPUT_PROFILES.length);
  for (let index = 0; index < INPUT_PROFILES.length; index += 1) {
    assert.equal(keys[index], `mir2:tutorialCompleted:v${TUTORIAL_VERSION}:${INPUT_PROFILES[index]}`);
  }
});

check("manual step does not advance on actions, only on next", () => {
  // step 0 (welcome) is manual.
  const afterAction = run([{ kind: "action", type: "walk" }]);
  assert.equal(afterAction.stepIndex, 0, "manual step ignores actions");
  const afterNext = reduceTutorial(afterAction, { kind: "next" });
  assert.equal(afterNext.stepIndex, 1, "next advances a manual step");
});

check("count-gated step requires the full count", () => {
  const moveIdx = indexOf("move");
  // advance to the move step.
  let state = createTutorialState();
  while (state.stepIndex < moveIdx) state = reduceTutorial(state, { kind: "next" });
  assert.equal(currentStep(state).id, "move");

  state = reduceTutorial(state, { kind: "action", type: "walk" });
  assert.equal(state.stepIndex, moveIdx, "1 walk is not enough (needs 3)");
  state = reduceTutorial(state, { kind: "action", type: "walk" });
  assert.equal(state.stepIndex, moveIdx, "2 walks is not enough");
  state = reduceTutorial(state, { kind: "action", type: "walk" });
  assert.equal(state.stepIndex, moveIdx + 1, "3rd walk advances");
});

check("unrelated actions never advance a step", () => {
  const moveIdx = indexOf("move");
  let state = createTutorialState();
  while (state.stepIndex < moveIdx) state = reduceTutorial(state, { kind: "next" });
  state = reduceTutorial(state, { kind: "action", type: "chat" });
  state = reduceTutorial(state, { kind: "action", type: "useItem" });
  assert.equal(state.stepIndex, moveIdx, "irrelevant actions are ignored");
});

check("aggregated action types satisfy a single step (attack family)", () => {
  const attackIdx = indexOf("attack");
  let state = createTutorialState();
  while (state.stepIndex < attackIdx) state = reduceTutorial(state, { kind: "next" });
  // attack step accepts attack/rangeAttack/magic/castSkill; any one satisfies it.
  state = reduceTutorial(state, { kind: "action", type: "castSkill" });
  assert.equal(state.stepIndex, attackIdx + 1, "castSkill counts toward the attack step");
});

check("window trigger advances only on the matching window opening", () => {
  const invIdx = indexOf("inventory");
  let state = createTutorialState();
  while (state.stepIndex < invIdx) state = reduceTutorial(state, { kind: "next" });
  // wrong window does nothing.
  state = reduceTutorial(state, { kind: "window", window: "character", open: true });
  assert.equal(state.stepIndex, invIdx, "opening a different window does not advance");
  // closing event ignored.
  state = reduceTutorial(state, { kind: "window", window: "inventory", open: false });
  assert.equal(state.stepIndex, invIdx, "close events are ignored");
  // correct open advances.
  state = reduceTutorial(state, { kind: "window", window: "inventory", open: true });
  assert.equal(state.stepIndex, invIdx + 1, "opening the inventory advances");
});

check("skipStep advances one step without performing the action", () => {
  const moveIdx = indexOf("move");
  let state = createTutorialState();
  while (state.stepIndex < moveIdx) state = reduceTutorial(state, { kind: "next" });
  state = reduceTutorial(state, { kind: "skipStep" });
  assert.equal(state.stepIndex, moveIdx + 1);
  assert.equal(state.done, false);
});

check("skipAll terminates immediately", () => {
  const state = reduceTutorial(createTutorialState(), { kind: "skipAll" });
  assert.equal(state.done, true);
});

check("back never goes below 0", () => {
  let state = createTutorialState();
  state = reduceTutorial(state, { kind: "back" });
  assert.equal(state.stepIndex, 0);
});

check("next on the final step reaches done", () => {
  for (const input of INPUT_PROFILES) {
    let state = createTutorialState(input);
    const steps = tutorialStepsForInput(input);
    // Walk all the way through with next.
    for (let i = 0; i < steps.length; i += 1) {
      state = reduceTutorial(state, { kind: "next" });
    }
    assert.equal(state.done, true, `${input} finishes after its final step`);
  }
});

check("a finished tutorial ignores further events", () => {
  let state = reduceTutorial(createTutorialState(), { kind: "skipAll" });
  const before = state;
  state = reduceTutorial(state, { kind: "action", type: "walk" });
  state = reduceTutorial(state, { kind: "next" });
  assert.deepEqual(state, before, "done state is frozen");
});

check("every profile step has English and Chinese text plus a valid trigger", () => {
  for (const input of INPUT_PROFILES) {
    for (const step of tutorialStepsForInput(input)) {
      assert.ok(step.title.en && step.body.en, `step ${step.id} needs English title/body`);
      assert.ok(step.title["zh-CN"] && step.body["zh-CN"], `step ${step.id} needs Chinese title/body`);
      assert.ok(
        ["action", "window", "manual"].includes(step.trigger.kind),
        `step ${step.id} has a known trigger kind`,
      );
      // pickText returns the locale string when present, English otherwise.
      assert.equal(pickText(step.title, "zh-CN"), step.title["zh-CN"]);
      assert.equal(pickText(step.title, "es"), step.title.es ?? step.title.en);
    }
  }
});

check("touch tutorial covers every requested mobile control group", () => {
  const touchSteps = tutorialStepsForInput("touch");
  const ids = new Set(touchSteps.map((step) => step.id));
  for (const requiredId of [
    "touch-move",
    "touch-run",
    "touch-attack",
    "touch-approach",
    "touch-pick",
    "touch-quick",
    "touch-panels",
    "touch-replay",
  ]) {
    assert.ok(ids.has(requiredId), `${requiredId} is present`);
  }
});

check("touch and gamepad semantic actions advance their matching steps", () => {
  let touch = reduceTutorial(createTutorialState("touch"), { kind: "next" });
  assert.equal(currentStep(touch).id, "touch-move");
  touch = reduceTutorial(touch, { kind: "action", type: "touch:move" });
  assert.equal(currentStep(touch).id, "touch-run");

  let gamepad = reduceTutorial(createTutorialState("gamepad"), { kind: "next" });
  assert.equal(currentStep(gamepad).id, "gamepad-move");
  gamepad = reduceTutorial(gamepad, { kind: "action", type: "gamepad:move" });
  assert.equal(currentStep(gamepad).id, "gamepad-actions");
  gamepad = reduceTutorial(gamepad, { kind: "action", type: "gamepad:primary" });
  assert.equal(currentStep(gamepad).id, "gamepad-quick");
});

check("spotlight selectors point only at known stable DOM hooks", () => {
  // These are the existing class hooks the overlay highlights. Keep this set in
  // sync with the real anchors in the HUD/scene components so a typo (which would
  // silently degrade to a card-only step) fails the suite instead.
  const KNOWN_HOOKS = new Set([
    ".hud-button.inventory",
    ".hud-button.character",
    ".hud-button.quest",
    ".hud-button.menu",
    ".belt-dialog",
    ".mir-mobile-stick-shell",
    ".mir-mobile-action.run",
    ".mir-mobile-action.primary",
    ".mir-mobile-action.approach",
    ".mir-mobile-action.pick",
    ".mir-mobile-action.quick",
    ".mir-mobile-panel-row",
  ]);
  for (const input of INPUT_PROFILES) {
    for (const step of tutorialStepsForInput(input)) {
      if (step.spotlight === undefined) continue;
      assert.equal(typeof step.spotlight, "string");
      assert.ok(step.spotlight.length > 0, `step ${step.id} spotlight is non-empty`);
      assert.ok(KNOWN_HOOKS.has(step.spotlight), `step ${step.id} spotlight "${step.spotlight}" is a known hook`);
    }
  }
});

check("first and last steps are manual intro/summary cards", () => {
  assert.equal(TUTORIAL_STEPS[0].trigger.kind, "manual");
  assert.equal(TUTORIAL_STEPS[TUTORIAL_STEPS.length - 1].trigger.kind, "manual");
});

console.log(`tutorial-flow: ${passed} checks passed (${TUTORIAL_STEPS.length} steps)`);
