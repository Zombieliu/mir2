#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

const [
  nativeMain,
  windowsBuild,
  tauriLib,
  tauriConfigSource,
  tauriFallback,
  tauriCapability,
  mobileLoader,
  capacitorConfigSource,
  mobilePackage,
  mobileBuild,
  mobileDeviceBuild,
  clientWorkflow,
] = await Promise.all([
  read("apps/game-client/platform-windows/src/main.rs"),
  read("apps/game-client/platform-windows/build-windows.sh"),
  read("apps/mir2-launcher-tauri/src-tauri/src/lib.rs"),
  read("apps/mir2-launcher-tauri/src-tauri/tauri.conf.json"),
  read("apps/mir2-launcher-tauri/src/index.html"),
  read("apps/mir2-launcher-tauri/src-tauri/capabilities/default.json"),
  read("apps/mir2-mobile/scripts/build-web.mjs"),
  read("apps/mir2-mobile/capacitor.config.json"),
  read("apps/mir2-mobile/package.json"),
  read("apps/mir2-mobile/build-mobile.sh"),
  read("apps/mir2-mobile/build-mobile-device.sh"),
  read(".github/workflows/cross-platform-client.yml"),
]);

assert.doesNotMatch(
  nativeMain,
  /unwrap_or_else\(\|_\|\s*"demo"\.to_owned\(\)\)/,
  "native production paths must never fall back to demo credentials",
);

assert.match(
  windowsBuild,
  /package-assets\.sh/,
  "the Windows artifact must package its runtime atlases and maps",
);
assert.match(
  windowsBuild,
  /--remap-path-prefix/,
  "release artifacts must not expose the build checkout path",
);

assert.doesNotMatch(
  tauriLib,
  /CARGO_MANIFEST_DIR|Command::new\("node"\)|\.mir2-thin-client/,
  "desktop release bundles must not depend on the source checkout or system Node",
);
assert.match(
  tauriLib,
  /https:\/\/mir2\.obelisk\.build/,
  "desktop release bundles must default to the stable production origin",
);
const tauriConfig = JSON.parse(tauriConfigSource);
assert.equal(
  tauriConfig.build?.frontendDist,
  "../src",
  "desktop builds must embed the committed local startup fallback",
);
assert.match(tauriFallback, /<!doctype html>/i);
assert.doesNotMatch(
  tauriFallback,
  /<script|https?:\/\//i,
  "the desktop startup fallback must remain inert and self-contained",
);
assert.doesNotMatch(
  tauriCapability,
  /shell:allow-open/,
  "the remote game origin must not inherit an unnecessary shell capability",
);

assert.match(
  mobileLoader,
  /https:\/\/mir2\.obelisk\.build/,
  "mobile shells must default to the stable production origin",
);
assert.doesNotMatch(
  mobileLoader,
  /vercel\.app/,
  "mobile shells must not ship a protected Vercel preview URL",
);

const capacitorConfig = JSON.parse(capacitorConfigSource);
assert.notEqual(capacitorConfig.server?.cleartext, true, "cleartext HTTP must be disabled");
assert.notEqual(
  capacitorConfig.android?.allowMixedContent,
  true,
  "mixed content must be disabled",
);

assert.match(mobilePackage, /cd ios\/App && xcodebuild/);
assert.match(mobileBuild, /cd "\$MOBILE\/ios\/App"/);
assert.match(mobileDeviceBuild, /ADB=\(adb -s "\$SERIAL"\)/);
assert.doesNotMatch(
  mobileDeviceBuild,
  /adb -s "\$SERIAL" install[^\n]*\|\| adb install/,
  "Android device verification must never silently switch devices",
);

for (const requiredAction of [
  "actions/checkout@v7",
  "actions/setup-node@v7",
  "actions/setup-java@v5",
  "actions/upload-artifact@v7",
]) {
  assert.ok(
    clientWorkflow.includes(requiredAction),
    `cross-platform CI must use the Node 24 action ${requiredAction}`,
  );
}

for (const requiredGate of [
  "ubuntu-latest",
  "windows-latest",
  "macos-latest",
  "wasm32-unknown-unknown",
  "assembleDebug",
  "xcodebuild",
]) {
  assert.match(
    clientWorkflow,
    new RegExp(requiredGate),
    `cross-platform CI is missing the ${requiredGate} gate`,
  );
}

const atlasBuildStep = clientWorkflow.indexOf("npm run assets:map-atlas:build");
const nativeTestStep = clientWorkflow.indexOf(
  'cargo "+$env:RUST_TOOLCHAIN" test --locked --manifest-path apps/game-client/platform-windows/Cargo.toml',
);
assert.ok(atlasBuildStep >= 0, "Windows CI must generate the gitignored map atlas");
assert.ok(
  nativeTestStep > atlasBuildStep,
  "Windows CI must generate the map atlas before real-manifest native tests",
);

console.log("cross-platform client shell contracts: ok");
