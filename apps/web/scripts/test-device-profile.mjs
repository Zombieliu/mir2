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
  new Function("exports", "module", "require", compiled.outputText)(
    module.exports,
    module,
    require,
  );
  return module.exports;
}

const gamepadInput = loadTypeScriptModule(
  new URL("../app/components/original-client-gamepad-input.ts", import.meta.url),
);
const { resolveMir2ClientProfile } = loadTypeScriptModule(
  new URL("../app/components/original-client-device-profile.ts", import.meta.url),
  {
    "./original-client-gamepad-input": gamepadInput,
  },
);

function baseProfile(profile) {
  const { gamepad: _gamepad, ...base } = profile;
  return base;
}

assert.deepEqual(baseProfile(resolveMir2ClientProfile({})), {
  layout: "desktop",
  input: "keyboardMouse",
  layoutForced: false,
  inputForced: false,
});

assert.deepEqual(baseProfile(resolveMir2ClientProfile({ coarsePointer: true, touchPoints: 5 })), {
  layout: "touch",
  input: "touch",
  layoutForced: false,
  inputForced: false,
});

assert.deepEqual(baseProfile(resolveMir2ClientProfile({ coarsePointer: false, touchPoints: 10 })), {
  layout: "desktop",
  input: "keyboardMouse",
  layoutForced: false,
  inputForced: false,
});

const xboxPlatform = resolveMir2ClientProfile({
  userAgent: "Mozilla/5.0 (Xbox; Xbox Series X)",
});
assert.deepEqual(baseProfile(xboxPlatform), {
  layout: "tv",
  input: "gamepad",
  layoutForced: false,
  inputForced: false,
});
assert.equal(xboxPlatform.gamepad.family, "xbox");
assert.equal(xboxPlatform.gamepad.mappingMode, "platform");

assert.deepEqual(baseProfile(resolveMir2ClientProfile({ search: "?layout=tv&input=gamepad" })), {
  layout: "tv",
  input: "gamepad",
  layoutForced: true,
  inputForced: true,
});

assert.deepEqual(
  baseProfile(resolveMir2ClientProfile({
    search: "?layout=desktop&input=keyboardMouse",
    coarsePointer: true,
    gamepadConnected: true,
  })),
  {
    layout: "desktop",
    input: "keyboardMouse",
    layoutForced: true,
    inputForced: true,
  },
);

assert.deepEqual(baseProfile(resolveMir2ClientProfile({ search: "?mobileControls=1" })), {
  layout: "touch",
  input: "touch",
  layoutForced: true,
  inputForced: true,
});

assert.equal(
  resolveMir2ClientProfile({ search: "?layout=unknown&input=unknown", gamepadConnected: true }).input,
  "gamepad",
);

const dualSense = resolveMir2ClientProfile({
  gamepadConnected: true,
  gamepadId: "DualSense Wireless Controller (STANDARD GAMEPAD Vendor: 054c Product: 0ce6)",
  gamepadMapping: "standard",
});
assert.equal(dualSense.input, "gamepad");
assert.equal(dualSense.gamepad.family, "playstation");
assert.equal(dualSense.gamepad.mappingMode, "standard");
assert.equal(dualSense.gamepad.connected, true);
assert.equal(dualSense.gamepad.supported, true);

const forcedPlayStation = resolveMir2ClientProfile({
  search: "?layout=tv&input=gamepad&controller=playstation",
});
assert.equal(forcedPlayStation.gamepad.family, "playstation");
assert.equal(forcedPlayStation.gamepad.mappingMode, "platform");

const unverifiedGeneric = resolveMir2ClientProfile({
  gamepadConnected: true,
  gamepadId: "Mystery USB Controller",
  gamepadMapping: "",
});
assert.equal(unverifiedGeneric.gamepad.family, "generic");
assert.equal(unverifiedGeneric.gamepad.mappingMode, "unverified");
assert.equal(unverifiedGeneric.gamepad.supported, false);

console.log("device profile tests passed");
