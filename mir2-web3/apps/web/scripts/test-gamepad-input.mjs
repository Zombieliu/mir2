import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(relativePath) {
  const sourcePath = new URL(relativePath, import.meta.url);
  const source = readFileSync(sourcePath, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(sourcePath),
  });
  const module = { exports: {} };
  new Function("exports", "module", compiled.outputText)(module.exports, module);
  return module.exports;
}

const input = loadTypeScriptModule("../app/components/original-client-gamepad-input.ts");
const navigation = loadTypeScriptModule("../app/components/original-client-gamepad-navigation.ts");

const buttons = Array.from({ length: 17 }, () => ({ pressed: false, value: 0 }));
assert.deepEqual(input.mir2GamepadVector({ axes: [0.1, -0.1], buttons }), { x: 0, y: 0, force: 0 });

const right = input.mir2GamepadVector({ axes: [1, 0], buttons });
assert.deepEqual(right, { x: 1, y: 0, force: 1 });

const dpadButtons = buttons.map((button) => ({ ...button }));
dpadButtons[input.MIR2_GAMEPAD_BUTTON.dpadUp].pressed = true;
assert.deepEqual(input.mir2GamepadVector({ axes: [0, 0], buttons: dpadButtons }), {
  x: 0,
  y: -1,
  force: 1,
});
assert.equal(
  input.mir2GamepadSpatialDirection(input.mir2GamepadVector({ axes: [0.8, 0.2], buttons })),
  "right",
);
assert.equal(
  input.mir2GamepadSpatialDirection(input.mir2GamepadVector({ axes: [0.2, -0.8], buttons })),
  "up",
);

dpadButtons[input.MIR2_GAMEPAD_BUTTON.primary].pressed = true;
const gamepad = { axes: [0, 0], buttons: dpadButtons };
assert.equal(input.mir2GamepadButtonPressed(gamepad, [], input.MIR2_GAMEPAD_BUTTON.primary), true);
assert.equal(
  input.mir2GamepadButtonPressed(
    gamepad,
    input.mir2GamepadButtons(gamepad),
    input.MIR2_GAMEPAD_BUTTON.primary,
  ),
  false,
);

const current = { left: 100, top: 100, width: 20, height: 20 };
const candidates = [
  { left: 20, top: 100, width: 20, height: 20 },
  { left: 140, top: 100, width: 20, height: 20 },
  { left: 105, top: 20, width: 20, height: 20 },
];
assert.equal(navigation.findMir2SpatialTarget(current, candidates, "right"), 1);
assert.equal(navigation.findMir2SpatialTarget(current, candidates, "left"), 0);
assert.equal(navigation.findMir2SpatialTarget(current, candidates, "up"), 2);
assert.equal(navigation.findMir2SpatialTarget(current, candidates, "down"), -1);

console.log("gamepad input tests passed");
