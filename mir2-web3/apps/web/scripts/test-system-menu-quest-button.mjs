import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");

const menuSource = fs.readFileSync(
  path.join(webRoot, "app/components/original-client-system-menu.tsx"),
  "utf8",
);
const sceneSource = fs.readFileSync(
  path.join(webRoot, "app/components/original-client-game-ui-scene.tsx"),
  "utf8",
);
const extraWindowsSource = fs.readFileSync(
  path.join(webRoot, "app/components/original-client-extra-windows.tsx"),
  "utf8",
);
const questWindowSource = fs.readFileSync(
  path.join(webRoot, "app/components/original-client-quest-log-window.tsx"),
  "utf8",
);
const shellSource = fs.readFileSync(
  path.join(webRoot, "app/original-client-shell.tsx"),
  "utf8",
);
const shellTypesSource = fs.readFileSync(
  path.join(webRoot, "app/components/original-client-shell-types.ts"),
  "utf8",
);
const pageSource = fs.readFileSync(path.join(webRoot, "app/page.tsx"), "utf8");

assert.match(menuSource, /data-system-menu-action="quest"/);
assert.match(menuSource, /style=\{\{ left: "3px", top: "107px" \}\}/);
assert.match(menuSource, /aria-pressed=\{questLogOpen\}/);
assert.match(menuSource, /onToggleQuestLog\(\);/);

assert.match(sceneSource, /questLogOpen=\{showQuestLog\}/);
assert.match(
  sceneSource,
  /onToggleQuestLog=\{\(\) => \{\s*onToggleQuestLog\(\);\s*setShowSystemMenu\(false\);\s*\}\}/,
);

assert.match(shellTypesSource, /showQuestLog: boolean;/);
assert.match(shellTypesSource, /onToggleQuestLog: \(\) => void;/);
assert.match(shellSource, /showQuestLog,/);
assert.match(shellSource, /onToggleQuestLog,/);
assert.match(pageSource, /showQuestLog=\{showQuestLog\}/);
assert.match(
  pageSource,
  /onToggleQuestLog=\{\(\) => setShowQuestLog\(\(current\) => !current\)\}/,
);

// The game stage can be replaced by Fast Refresh or a screen-shell remount.
// Re-resolve the portal host after each registry render instead of retaining a
// detached node from the first mount.
assert.match(
  extraWindowsSource,
  /const nextStageRoot = document\.querySelector<HTMLElement>\("\.client-stage-frame"\);/,
);
assert.match(
  extraWindowsSource,
  /setStageRoot\(\(current\) => current === nextStageRoot \? current : nextStageRoot\);/,
);
assert.doesNotMatch(
  extraWindowsSource,
  /setStageRoot\(document\.querySelector<HTMLElement>\("\.client-stage-frame"\)\);\s*\}, \[\]\);/,
);

// The task diary reuses the Crystal mail frame, but owns its title/content
// geometry. Never paint the MAIL title over the localized task title.
assert.doesNotMatch(questWindowSource, /<img style=\{style\.title\} src=\{FRAME\.title\}/);
assert.match(questWindowSource, /style=\{style\.contentBackdrop\}/);
assert.match(questWindowSource, /data-testid="quest-window-drag-handle"/);
assert.match(questWindowSource, /onPointerDown=\{onDragPointerDown\}/);
assert.match(questWindowSource, /onPointerMove=\{onDragPointerMove\}/);
assert.match(questWindowSource, /setPointerCapture\(event\.pointerId\)/);
assert.match(questWindowSource, /QUEST_WINDOW_POSITION_STORAGE_KEY/);
assert.match(questWindowSource, /clampQuestWindowPosition/);
assert.match(questWindowSource, /left: 10,\s*top: 278,\s*width: 292,/);
assert.match(questWindowSource, /left: 10,\s*top: 402,\s*width: 292,/);

console.log("system menu quest button tests passed");
