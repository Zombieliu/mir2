import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../app/components/original-client-device-profile.ts", import.meta.url);
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

const { resolveMir2ClientProfile } = module.exports;

assert.deepEqual(resolveMir2ClientProfile({}), {
  layout: "desktop",
  input: "keyboardMouse",
  layoutForced: false,
  inputForced: false,
});

assert.deepEqual(resolveMir2ClientProfile({ coarsePointer: true, touchPoints: 5 }), {
  layout: "touch",
  input: "touch",
  layoutForced: false,
  inputForced: false,
});

assert.deepEqual(resolveMir2ClientProfile({ coarsePointer: false, touchPoints: 10 }), {
  layout: "desktop",
  input: "keyboardMouse",
  layoutForced: false,
  inputForced: false,
});

assert.deepEqual(resolveMir2ClientProfile({ userAgent: "Mozilla/5.0 (Xbox; Xbox Series X)" }), {
  layout: "tv",
  input: "gamepad",
  layoutForced: false,
  inputForced: false,
});

assert.deepEqual(resolveMir2ClientProfile({ search: "?layout=tv&input=gamepad" }), {
  layout: "tv",
  input: "gamepad",
  layoutForced: true,
  inputForced: true,
});

assert.deepEqual(
  resolveMir2ClientProfile({
    search: "?layout=desktop&input=keyboardMouse",
    coarsePointer: true,
    gamepadConnected: true,
  }),
  {
    layout: "desktop",
    input: "keyboardMouse",
    layoutForced: true,
    inputForced: true,
  },
);

assert.deepEqual(resolveMir2ClientProfile({ search: "?mobileControls=1" }), {
  layout: "touch",
  input: "touch",
  layoutForced: true,
  inputForced: true,
});

assert.equal(
  resolveMir2ClientProfile({ search: "?layout=unknown&input=unknown", gamepadConnected: true }).input,
  "gamepad",
);

console.log("device profile tests passed");
