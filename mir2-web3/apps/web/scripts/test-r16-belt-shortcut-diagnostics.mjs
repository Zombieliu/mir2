import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

// Actual nested handlers and pure key/identity helpers only: no React, Core,
// renderer, socket, browser, credentials or running character is initialized.
const pureFiles = {
  "player-ui-preferences": "../lib/player-ui-preferences.ts",
  "crystal-shortcut-actions": "../lib/crystal-shortcut-actions.ts",
  "r16-guild-keyboard": "../lib/r16-guild-keyboard.ts",
  "shared-combat-mode-keys": "../lib/shared-combat-mode-keys.ts",
  "world-model/item-identity": "../lib/world-model/item-identity.ts",
};
const cache = new Map();
function pure(name) {
  assert(Object.hasOwn(pureFiles, name), "outside explicit pure input allowlist");
  if (cache.has(name)) return cache.get(name);
  const module = { exports: {} };
  const output = ts.transpileModule(readFileSync(new URL(pureFiles[name], import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("exports", "module", "require", output)(module.exports, module, id => {
    if (id === "../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json") return {
      default: JSON.parse(readFileSync(new URL("../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json", import.meta.url), "utf8")),
    };
    return pure(id.replace(/^\.\//, ""));
  });
  cache.set(name, module.exports); return module.exports;
}
function syntax(relative) {
  const text = readFileSync(new URL(relative, import.meta.url), "utf8");
  return ts.createSourceFile(relative, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
}
const page = syntax("../app/page.tsx"), shell = syntax("../app/original-client-shell.tsx");
function declaration(ast, name) {
  const found = [];
  function visit(node) { if (ts.isFunctionDeclaration(node) && node.name?.text === name) found.push(node); ts.forEachChild(node, visit); }
  visit(ast); assert.equal(found.length, 1, "sole actual declaration " + name); return found[0].getText(ast);
}
function functions(ast, names, scope) {
  const output = ts.transpileModule(names.map(name => declaration(ast, name)).join("\n"), {
    compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return new Function("scope", `with(scope){${output};return {${names.join(",")}};}`)(scope);
}
class Element { isContentEditable = false; closest() { return null; } }
class Input extends Element {}
class Textarea extends Element {}
class Select extends Element {}
function fixture(changes = {}) {
  const flags = { focus: true, visible: true, scene: true, item: true, identity: true, send: true, ...changes };
  const calls = [], logs = [];
  const item = { key: "private-item-canary", name: "private-name-canary", uniqueId: 424242, slot: 0, container: "belt", quantity: 1,
    ...(flags.identity ? { authoritativeUniqueId: 424242 } : {}) };
  const world = { beltItems: flags.item ? [item] : [], inventoryItems: [], storageItems: [],
    activeNpcDialog: flags.npcDialog ? { npcObjectId: 17 } : null,
    knownSkills: flags.learned ? [{ key: "Fencing", hotkey: 0, privateField: "private-skill-canary" }] : [] };
  const scope = {
    ...pure("player-ui-preferences"), ...pure("crystal-shortcut-actions"), ...pure("r16-guild-keyboard"),
    ...pure("shared-combat-mode-keys"), ...pure("world-model/item-identity"),
    HTMLElement: Element, HTMLInputElement: Input, HTMLTextAreaElement: Textarea, HTMLSelectElement: Select,
    world, worldRef: { current: world }, window: {}, screen: "game", screenRef: { current: flags.scene ? "game" : "login" },
    document: { get visibilityState() { return flags.visible ? "visible" : "hidden"; }, hasFocus: () => flags.focus,
      querySelector(selector) {
        assert.equal(selector, ".npc-dialog-panel");
        return flags.npcDialog && !flags.npcDialogDismissed ? {} : null;
      } },
    isSpectatorBrowserMode: () => !!flags.spectator, currentSocialReplyOwner: () => flags.ownerMissing ? null : {},
    referenceWindowsBlockGameplay: () => !!flags.referenceWindow, heroUiBlocksGameplay: () => !!flags.heroWindow,
    heroPetOpenRef: { current: false }, cashShopOpenRef: { current: false }, equipmentHostSuspendReasonRef: { current: null },
    beltShortcutDiagnosticsRef: { current: null }, equipmentCommandDiagnosticsRef: { current: null },
    appendLog: (...args) => logs.push(args), t: (_key, _args, fallback) => fallback,
    send: command => { calls.push(command.type); return flags.send; },
    keyboardMovementDiagnosticsRef: { current: { lastShortcut: null } }, combatTildeRef: { current: false },
    crystalKeyBindings: pure("player-ui-preferences").defaultCrystalKeyBindings(),
    getKeybindCaptureActive: () => !!flags.capture, npcShopBlocksWorldInput: () => !!flags.shop,
    stopNpcShopWorldInput: () => calls.push("stop"), bevyStorageUiActive: false, bevyStorageUiTransitioning: false,
    bevyMailComposeReady: false, bevyMailComposePending: false,
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => flags.spells ? { modal: true } : null } },
    bevyQuestUiCapturesPointer: false, heroPointerCallbacksRef: { current: { onBevyHeroKey: () => false } },
    heroBlocksWorldInput: () => false, heroPointerLeaseRef: { current: null },
    bevyHeroUiActive: false, bevyHeroUiTransitioning: false, bevyHeroUiWorldBlocked: false,
    sceneInteractionReady: !!flags.sceneReady, selectedEntity: null, isKeyboardMoveKey: () => false,
    onCombatKey: () => { calls.push("combat"); return { supported: true, handled: true }; },
  };
  const api = functions(page, ["handleCrystalGameplayShortcut", "useItem", "rejectEquipmentCommand"], scope);
  scope.onCrystalGameplayShortcut = (id, repeat) => { calls.push(id); return api.handleCrystalGameplayShortcut(id, repeat); };
  const handler = functions(shell, ["handleShortcutKey", "keyboardInputTargetIsEditable"], scope).handleShortcutKey;
  const event = { target: flags.editable ? new Input() : new Element(), key: "1", code: "Digit1", altKey: false, ctrlKey: false,
    shiftKey: false, metaKey: false, repeat: !!flags.repeat, defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; } };
  return { flags, scope, event, calls, logs, api, handler,
    read: () => functions(page, ["readBeltInputDiagnostics"], scope).readBeltInputDiagnostics() };
}

test("actual Belt1 routing records handled separately from one accepted use for empty and Fencing lists", () => {
  for (const learned of [false, true]) {
    const f = fixture({ learned }); f.handler(f.event);
    assert.deepEqual(f.calls, ["Belt1", "useItem", "combat"]);
    const keyboard = f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut;
    assert(keyboard, "Belt must be visible in the existing shortcut diagnostic");
    assert.deepEqual(keyboard.actions, ["Belt1"]); assert.equal(keyboard.handled, true);
    assert.equal(keyboard.focused, true); assert.equal(keyboard.visible, true); assert.equal(keyboard.editable, false);
    assert.equal(keyboard.gameplayCallbackPresent, true);
    assert.equal(keyboard.accepted, undefined, "handled is not a use acknowledgement");
    const report = f.read(); assert.equal(report.shortcut.stage, "use"); assert.equal(report.shortcut.useAccepted, true);
    assert.equal(report.shortcut.itemPresent, true); assert.equal(report.shortcut.identityValid, true);
  }
});
test("actual repeat remains handled without attempting an item, Core request or fallback", () => {
  const f = fixture({ learned: true, repeat: true }); f.handler(f.event);
  assert.deepEqual(f.calls, ["Belt1", "combat"]); assert.equal(f.event.defaultPrevented, true);
  assert.equal(f.read().shortcut.stage, "repeat"); assert.equal(f.read().shortcut.reason, "repeat");
  assert.equal(f.read().shortcut.useAccepted, null);
});
test("actual Shell records focus editable and existing UI refusals before Page without using an item", () => {
  for (const [changes, reason] of [[{ focus: false }, "document"], [{ visible: false }, "document"], [{ editable: true }, "editable"],
    [{ capture: true }, "keybind-capture"], [{ shop: true }, "npc-shop"], [{ spells: true }, "spells-modal"]]) {
    const f = fixture(changes); f.handler(f.event);
    assert.equal(f.calls.includes("Belt1"), false, reason); assert.equal(f.calls.includes("useItem"), false, reason);
    assert.equal(f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut.reason, reason);
    assert.equal(f.read().shortcut, null);
  }
  for (const [property, reason] of [["metaKey", "meta-key"], ["defaultPrevented", "prevented-before-shortcut"]]) {
    const f = fixture(); f.event[property] = true; f.handler(f.event);
    assert.equal(f.calls.length, 0); assert.equal(f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut.reason, reason);
    assert.equal(f.read().shortcut, null);
  }
});
test("actual Page preserves Belt handled even when its item is missing invalid or locally refused", () => {
  for (const [changes, reason, present, valid, sent] of [
    [{ item: false }, "itemMissing", false, null, 0], [{ identity: false }, "useRejected", true, false, 0],
    [{ send: false }, "useRejected", true, true, 1],
  ]) {
    const f = fixture(changes); f.handler(f.event); const report = f.read();
    assert.equal(f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut.handled, true);
    assert.equal(report.shortcut.reason, reason); assert.equal(report.shortcut.itemPresent, present);
    assert.equal(report.shortcut.identityValid, valid); assert.equal(f.calls.filter(x => x === "useItem").length, sent);
    if (!valid && present) assert.deepEqual(report.equipment, { stage: "rejected", reason: "invalidIdentity" });
  }
});
test("actual Page diagnostics preserve existing scene owner and window gates and do not add scene-ready authority", () => {
  for (const [changes, reason] of [[{ scene: false }, "scene"], [{ ownerMissing: true }, "owner"],
    [{ spectator: true }, "playerMode"], [{ referenceWindow: true }, "referenceWindows"], [{ heroWindow: true }, "heroUi"]]) {
    const f = fixture(changes); f.handler(f.event); assert.equal(f.calls.includes("useItem"), false);
    assert.equal(f.read().shortcut.stage, "blocked"); assert.equal(f.read().shortcut.reason, reason);
  }
  const f = fixture({ sceneReady: false }); f.handler(f.event);
  assert.equal(f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut.sceneInteractionReady, false);
  assert.equal(f.calls.filter(x => x === "useItem").length, 1, "diagnostics must not invent a scene readiness gate");
});
test("ordinary NPC dialogue claims both physical forms of Belt1", () => {
  for (const code of ["Digit1", "Numpad1"]) {
    const f = fixture({ npcDialog: true });
    f.event.code = code;
    f.handler(f.event);
    assert.equal(f.calls.includes("useItem"), false);
    assert.equal(f.read().shortcut.stage, "blocked");
    assert.equal(f.read().shortcut.reason, "npcDialog");
    assert.equal(f.read().shortcut.gates.npcDialog, false);
  }
  const dismissed = fixture({ npcDialog: true, npcDialogDismissed: true });
  dismissed.handler(dismissed.event);
  assert.equal(dismissed.read().shortcut.reason, "accepted", "visible close releases input before the server snapshot");
  assert.equal(dismissed.calls.filter(type => type === "useItem").length, 1);
});
test("actual public Belt getter is inert finite and excludes identities names arbitrary refusal text or private fields", () => {
  const f = fixture({ learned: true }); f.handler(f.event);
  const before = [...f.calls], one = f.read(), two = f.read(); assert.deepEqual(one, two); assert.deepEqual(f.calls, before);
  f.api.rejectEquipmentCommand("private-reason-canary");
  assert.deepEqual(f.read().equipment, { stage: "rejected", reason: "other" });
  const json = JSON.stringify({ keyboard: f.scope.keyboardMovementDiagnosticsRef.current.lastShortcut, host: f.read() });
  for (const value of ["424242", "private-item-canary", "private-name-canary", "private-skill-canary", "private-reason-canary", "accountId", "uniqueId", "password"])
    assert.equal(json.includes(value), false, value);
  assert.deepEqual(f.calls, before, "reading/reject-label diagnostics never submits a second item action");
  assert.equal(typeof shell.text, "string"); assert(shell.text.includes("data-belt-input-status="));
  assert(page.text.includes("readBeltInputDiagnostics={readBeltInputDiagnostics}"));
  f.scope.beltShortcutDiagnosticsRef.current.privateField = "private-field-canary";
  f.scope.beltShortcutDiagnosticsRef.current.gates.privateField = "private-gate-canary";
  f.scope.equipmentCommandDiagnosticsRef.current.privateField = "private-equipment-canary";
  assert.equal(JSON.stringify(f.read()).includes("canary"), false, "public getter explicitly projects every field");
});
