import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

// Run the actual Page functions and independent diary-close callback. The fake
// HUD can decline while its previously published ready flag is still true.
const file = new URL("../app/page.tsx", import.meta.url);
const tree = ts.createSourceFile("page.tsx", readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const functions = new Map(); let navigate, close;
function visit(node) {
  if (ts.isFunctionDeclaration(node) && ["setShowQuestLog", "toggleQuestLogWindow"].includes(node.name?.text))
    functions.set(node.name.text, node.getText(tree));
  if (ts.isVariableDeclaration(node) && node.name.getText(tree) === "navigateSharedHud") navigate = node.initializer.getText(tree);
  if (ts.isCallExpression(node) && node.expression.getText(tree) === "useBevyQuestUi") {
    close = node.arguments[0].properties.find(p => p.name?.getText(tree) === "onCloseDiary").initializer.getText(tree);
  }
  ts.forEachChild(node, visit);
} visit(tree); assert.equal(functions.size, 2); assert(navigate && close);
const code = ts.transpileModule(`const navigateSharedHud=${navigate};const onCloseDiary=${close};
${[...functions.values()].join("\n")}
return { setOpen:setShowQuestLog, toggle:toggleQuestLogWindow, closeHost:onCloseDiary,
  snapshot:()=>({hostOpen:showQuestLog,refOpen:questLogOpenRef.current,revision:fallbackHudNavigationRevisionRef.current}) };`, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;

function fixture(changes = {}) {
  const flags = { ready: true, accepts: false, hostOpen: true, diaryOpen: false, projecting: false, ...changes };
  const actions = [], stateUpdates = []; let revision = 10, closes = 0;
  const scope = {
    flags, actions, stateUpdates, questLogOpenRef: { current: flags.hostOpen },
    fallbackHudNavigationRevisionRef: { current: revision }, hudProjectionRef: { current: flags.projecting },
    sharedHudNavigationRef: { current: { get ready() { return flags.ready; }, dispatch(action) { actions.push(action); return flags.accepts; } } },
    nextHudNavigationRevision: () => ++revision,
    clientProfile: { layout: "desktop" }, closeTouchSecondaryWindows() { assert.fail("desktop diary must not close another panel"); },
    bevyQuestUi: { readCurrentDiaryOpen: () => flags.diaryOpen,
      closeCurrentDiary() { closes++; return flags.closeAccepted ?? true; } },
  };
  const setup = `let showQuestLog=flags.hostOpen;const setShowQuestLogState=open=>{if(!flags.deferRender)showQuestLog=open;stateUpdates.push(open);};\n`;
  const view = new Function(...Object.keys(scope), setup + code)(...Object.values(scope));
  return { ...view, flags, actions, stateUpdates, closes: () => closes };
}

test("actual Page retains a reopen requested while the ready HUD declines pending navigation", () => {
  const f = fixture(); f.closeHost(); const closed = f.snapshot();
  assert.equal(closed.hostOpen, false); assert.equal(closed.refOpen, false);
  f.toggle();
  assert.equal(f.snapshot().hostOpen, true, "the accepted key must reach the host snapshot when Native cannot ingest navigation yet");
  assert.equal(f.snapshot().refOpen, true);
  assert(f.snapshot().revision > closed.revision, "the new explicit host-open supersedes its pending close");
  assert.deepEqual(f.actions, [{ type: "openQuest" }], "the current closed diary requests open, not a stale HUD toggle");
});

test("actual Page closes a currently open diary even before its React open projection arrives", () => {
  const f = fixture({ hostOpen: false, diaryOpen: true });
  assert.equal(f.toggle(), true); assert.equal(f.closes(), 1); assert.deepEqual(f.actions, []);
  const declined = fixture({ hostOpen: false, diaryOpen: true, closeAccepted: false });
  assert.equal(declined.toggle(), false); assert.equal(declined.closes(), 1); assert.deepEqual(declined.actions, []);
});

test("an accepted HUD open keeps Native navigation authority and avoids a fallback snapshot", () => {
  const f = fixture({ hostOpen: false, accepts: true }); const before = f.snapshot();
  f.setOpen(true);
  assert.deepEqual(f.actions, [{ type: "openQuest" }]); assert.deepEqual(f.stateUpdates, []);
  assert.equal(f.snapshot().refOpen, true); assert.equal(f.snapshot().revision, before.revision);
});

test("unavailable HUD and Native-to-React projections retain their existing handoffs", () => {
  const cold = fixture({ hostOpen: false, diaryOpen: null, ready: false }); cold.toggle();
  assert.equal(cold.snapshot().hostOpen, true); assert.deepEqual(cold.actions, []);
  const projected = fixture({ hostOpen: false, projecting: true, accepts: true }); const before = projected.snapshot();
  projected.setOpen(true); assert.equal(projected.snapshot().hostOpen, true);
  assert.equal(projected.snapshot().revision, before.revision); assert.deepEqual(projected.actions, []);
});

test("two pending diary keys use the latest requested edge before a React render", () => {
  const f = fixture({ hostOpen: false, diaryOpen: null, deferRender: true });
  f.toggle(); f.toggle();
  assert.deepEqual(f.stateUpdates, [true, false]);
  assert.equal(f.snapshot().refOpen, false);
  assert.deepEqual(f.actions, [{ type: "openQuest" }, { type: "closeQuest" }]);
});
