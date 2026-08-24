#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(projectRoot, "..");
const require = createRequire(import.meta.url);
const readProject = (relativePath) =>
  readFile(path.join(projectRoot, relativePath), "utf8");
const readRepository = (relativePath) =>
  readFile(path.join(repositoryRoot, relativePath), "utf8");

const [
  nativeMain,
  windowsBuild,
  tauriLib,
  tauriConfigSource,
  tauriFallback,
  tauriCapability,
  mobileLoader,
  capacitorConfigSource,
  androidManifest,
  mobilePackage,
  mobileBuild,
  mobileDeviceBuild,
  androidNativeBuild,
  clientWorkflow,
] = await Promise.all([
  readProject("apps/game-client/platform-windows/src/main.rs"),
  readProject("apps/game-client/platform-windows/build-windows.sh"),
  readProject("apps/mir2-launcher-tauri/src-tauri/src/lib.rs"),
  readProject("apps/mir2-launcher-tauri/src-tauri/tauri.conf.json"),
  readProject("apps/mir2-launcher-tauri/src/index.html"),
  readProject("apps/mir2-launcher-tauri/src-tauri/capabilities/default.json"),
  readProject("apps/mir2-mobile/scripts/build-web.mjs"),
  readProject("apps/mir2-mobile/capacitor.config.js"),
  readProject("apps/mir2-mobile/android/app/src/main/AndroidManifest.xml"),
  readProject("apps/mir2-mobile/package.json"),
  readProject("apps/mir2-mobile/build-mobile.sh"),
  readProject("apps/mir2-mobile/build-mobile-device.sh"),
  readProject("apps/game-client/platform-android/build-android.sh"),
  readRepository(".github/workflows/cross-platform-client.yml"),
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
assert.match(
  windowsBuild,
  /CARGO_HOME_DIR.*--remap-path-prefix=.*CARGO_HOME_DIR/s,
  "release artifacts must not expose the developer Cargo home path",
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

const capacitorConfig = require(
  path.join(projectRoot, "apps/mir2-mobile/capacitor.config.js"),
);
assert.notEqual(capacitorConfig.server?.cleartext, true, "cleartext HTTP must be disabled");
assert.notEqual(
  capacitorConfig.android?.allowMixedContent,
  true,
  "mixed content must be disabled",
);
assert.deepEqual(
  capacitorConfig.server?.allowNavigation,
  ["mir2.obelisk.build"],
  "the production game origin must remain inside the mobile app WebView",
);
assert.match(
  capacitorConfigSource,
  /createCapacitorConfig/,
  "Capacitor navigation policy must resolve the same build-time game origin as the loader",
);
assert.match(androidManifest, /android:allowBackup="false"/);
assert.match(androidManifest, /android:usesCleartextTraffic="false"/);

assert.match(mobilePackage, /cd ios\/App && xcodebuild/);
assert.match(mobileBuild, /cd "\$MOBILE\/ios\/App"/);
assert.match(mobileDeviceBuild, /ADB=\(adb -s "\$SERIAL"\)/);
assert.doesNotMatch(
  mobileDeviceBuild,
  /adb -s "\$SERIAL" install[^\n]*\|\| adb install/,
  "Android device verification must never silently switch devices",
);

for (const requiredAction of [
  "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
  "actions/setup-node@820762786026740c76f36085b0efc47a31fe5020",
  "actions/setup-java@b6effb05e454b25005698d916606bdc6ffcbf961",
  "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",
]) {
  assert.ok(
    clientWorkflow.includes(requiredAction),
    `cross-platform CI must use the reviewed immutable action revision ${requiredAction}`,
  );
}
assert.doesNotMatch(
  clientWorkflow,
  /uses:\s*actions\/(?:checkout|setup-node|setup-java|upload-artifact)@v\d+/,
  "cross-platform CI must not regress to mutable action tags",
);

assert.match(
  clientWorkflow,
  /mir2-web3\/apps\/game-client\/\*\*/,
  "cross-platform CI must watch the main repository's nested project paths",
);
assert.doesNotMatch(
  clientWorkflow,
  /fix\/local-parity-and-i18n/,
  "main CI must not retain the unrelated historical base branch trigger",
);
assert.match(
  androidNativeBuild,
  /^MANIFEST="\$\{SCRIPT_DIR\}\/Cargo\.toml"$/m,
  "Android native build must derive its manifest variable from the script directory",
);
assert.equal(
  [...androidNativeBuild.matchAll(/--manifest-path "\$\{MANIFEST\}"/g)].length,
  2,
  "Android native check and package commands must both use the trusted manifest variable",
);
assert.doesNotMatch(
  androidNativeBuild,
  /--manifest-path "(?!\$\{MANIFEST\})/,
  "Android native Cargo commands must not bypass the trusted manifest variable",
);

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
  'cargo "+$env:RUST_TOOLCHAIN" test --locked --manifest-path mir2-web3/apps/game-client/platform-windows/Cargo.toml',
);
assert.ok(atlasBuildStep >= 0, "Windows CI must generate the gitignored map atlas");
assert.ok(
  nativeTestStep > atlasBuildStep,
  "Windows CI must generate the map atlas before real-manifest native tests",
);

console.log("cross-platform client shell contracts: ok");
