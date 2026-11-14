import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
import { createShellKeydownFixture } from "./lib/window-keydown-test-fixture.mjs";

const sources = {
  "player-ui-preferences": "../lib/player-ui-preferences.ts",
  "crystal-shortcut-actions": "../lib/crystal-shortcut-actions.ts",
  "shared-combat-mode-keys": "../lib/shared-combat-mode-keys.ts",
  "r16-guild-keyboard": "../lib/r16-guild-keyboard.ts",
};
const modules = new Map();
function load(name) {
  assert(Object.hasOwn(sources, name));
  if (modules.has(name)) return modules.get(name);
  const module = { exports: {} };
  const code = ts.transpileModule(readFileSync(new URL(sources[name], import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("exports", "module", "require", code)(module.exports, module, id => {
    if (id === "../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json") return {
      default: JSON.parse(readFileSync(new URL("../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json", import.meta.url), "utf8")),
    };
    return load(id.replace(/^\.\//, ""));
  });
  modules.set(name, module.exports); return module.exports;
}
class Element { isContentEditable = false; closest() { return null; } }
class Input extends Element {}
class Textarea extends Element {}
class Select extends Element {}
function fixture(changes = {}) {
  const flags = { open: true, hud: true, accept: true, capture: false, focus: true, visible: true, ...changes };
  const listeners = new Set(), calls = [];
  const window = {
    addEventListener(type, fn, capture) { assert.equal(type, "keydown"); assert.equal(capture, true); listeners.add(fn); },
    removeEventListener(type, fn) { assert.equal(type, "keydown"); listeners.delete(fn); },
  };
  const scope = {
    ...load("player-ui-preferences"), ...load("crystal-shortcut-actions"), ...load("r16-guild-keyboard"),
    HTMLElement: Element, HTMLInputElement: Input, HTMLTextAreaElement: Textarea, HTMLSelectElement: Select,
    screen: "game", window, stageExitDialogRef: { current: null },
    document: { get visibilityState() { return flags.visible ? "visible" : "hidden"; }, hasFocus: () => flags.focus },
    crystalKeyBindings: load("player-ui-preferences").defaultCrystalKeyBindings(),
    keyboardMovementDiagnosticsRef: { current: {} }, combatTildeRef: { current: false },
    getKeybindCaptureActive: () => flags.capture, npcShopBlocksWorldInput: () => !!flags.shop,
    stopNpcShopWorldInput() {}, bevyStorageUiActive: !!flags.storage, bevyStorageUiTransitioning: false,
    bevyMailComposeReady: !!flags.compose, bevyMailComposePending: false,
    isCurrentMailTextTarget: () => false,
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => flags.spells ? { modal: true } : null } },
    bevyQuestUiCapturesPointer: true,
    readBevyHudStatus: () => flags.hud ? { navigation: { questOpen: flags.open } } : null,
    dispatchBevyHudNavigation: action => { calls.push(action); return flags.accept; },
    bevyHeroUiActive: false, bevyHeroUiTransitioning: false, bevyHeroUiWorldBlocked: false,
    heroPointerLeaseRef: { current: null }, sceneInteractionReady: true,
    onCrystalGameplayShortcut() { throw Error("modal key must not invoke a gameplay command"); },
  };
  const binding = createShellKeydownFixture(scope); binding.render(); binding.commit();
  const event = extra => ({ target: new Element(), key: "q", code: "KeyQ", repeat: false, metaKey: false,
    altKey: false, ctrlKey: false, shiftKey: false, defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; }, ...extra });
  return { scope, flags, calls, event, binding,
    dispatch(extra = {}) { const e = event(extra); for (const listener of [...listeners]) listener(e); return e; } };
}

test("current shared diary closes through its configured key and the Native-only navigation dispatcher", () => {
  const f = fixture(), e = f.dispatch();
  assert.deepEqual(f.calls, [{ type: "closeQuest" }]); assert.equal(e.defaultPrevented, true);
  assert.equal(f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut.accepted, true);
  f.binding.unmount();
});
test("remapped Native binding works and its former key stays blocked by the modal", () => {
  const f = fixture();
  f.scope.crystalKeyBindings = f.scope.crystalKeyBindings.map(row => row.function === "Quests" ? { ...row, key: "J" } : row);
  f.binding.render(); f.binding.commit();
  f.dispatch(); assert.equal(f.calls.length, 0);
  f.dispatch({ key: "j", code: "KeyJ" }); assert.deepEqual(f.calls, [{ type: "closeQuest" }]);
});
test("repeats and declined navigation cannot toggle or fall through to the world", () => {
  const held = fixture(); assert.equal(held.dispatch({ repeat: true }).defaultPrevented, true); assert.equal(held.calls.length, 0);
  const declined = fixture({ accept: false });
  assert.equal(declined.dispatch().defaultPrevented, true); assert.deepEqual(declined.calls, [{ type: "closeQuest" }]);
  assert.equal(declined.scope.keyboardMovementDiagnosticsRef.current.lastShortcut.accepted, false);
});
test("typing, another modal, stale HUD authority and non-quest keys retain their input owner", () => {
  for (const target of [new Input(), new Textarea(), new Select(), Object.assign(new Element(), { isContentEditable: true })]) {
    const f = fixture(); assert.equal(f.dispatch({ target }).defaultPrevented, false); assert.equal(f.calls.length, 0);
  }
  for (const flags of [{ hud: false }, { open: false }, { capture: true }, { focus: false }, { visible: false }, { shop: true }, { spells: true }, { storage: true }, { compose: true }]) {
    const f = fixture(flags); f.dispatch(); assert.equal(f.calls.length, 0, JSON.stringify(flags));
  }
  for (const event of [{ key: "i", code: "KeyI" }, { metaKey: true }, { defaultPrevented: true }]) {
    const f = fixture(); f.dispatch(event); assert.equal(f.calls.length, 0);
  }
  const exit = fixture(); exit.scope.stageExitDialogRef.current = { open: true };
  exit.dispatch(); assert.equal(exit.calls.length, 0);
});
test("withdrawing current HUD before dispatch blocks a captured old modal handler", () => {
  const f = fixture(); f.flags.hud = false; f.dispatch(); assert.equal(f.calls.length, 0);
});
