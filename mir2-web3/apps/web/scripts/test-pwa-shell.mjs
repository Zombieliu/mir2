import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";
import ts from "typescript";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(webRoot, "app", "manifest.ts");
const manifestSource = readFileSync(manifestPath, "utf8");
const compiled = ts.transpileModule(manifestSource, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    esModuleInterop: true,
  },
  fileName: manifestPath,
});
const manifestModule = { exports: {} };
new Function("exports", "module", compiled.outputText)(manifestModule.exports, manifestModule);
const manifest = manifestModule.exports.default();

assert.equal(manifest.id, "/");
assert.equal(manifest.start_url, "/");
assert.equal(manifest.scope, "/");
assert.equal(manifest.display, "fullscreen");
assert.equal(manifest.orientation, "landscape");
assert.equal(manifest.background_color, "#000000");
assert.ok(manifest.icons.some((icon) => icon.sizes === "192x192" && icon.purpose === "any"));
assert.ok(manifest.icons.some((icon) => icon.sizes === "512x512" && icon.purpose === "any"));
assert.ok(manifest.icons.some((icon) => icon.sizes === "512x512" && icon.purpose === "maskable"));

const iconExpectations = [
  ["pwa/icon-192.png", 192, 192],
  ["pwa/icon-512.png", 512, 512],
  ["pwa/icon-maskable-512.png", 512, 512],
  ["pwa/apple-touch-icon.png", 180, 180],
];
for (const [relativePath, width, height] of iconExpectations) {
  const iconPath = path.join(webRoot, "public", relativePath);
  assert.equal(existsSync(iconPath), true, `${relativePath} exists`);
  const metadata = await sharp(iconPath).metadata();
  assert.equal(metadata.width, width, `${relativePath} width`);
  assert.equal(metadata.height, height, `${relativePath} height`);
  assert.equal(metadata.format, "png", `${relativePath} format`);
}

const layoutSource = readFileSync(path.join(webRoot, "app", "layout.tsx"), "utf8");
assert.match(layoutSource, /manifest:\s*"\/manifest\.webmanifest"/);
assert.match(layoutSource, /appleWebApp:\s*\{/);
assert.match(layoutSource, /"apple-mobile-web-app-capable":\s*"yes"/);
assert.match(layoutSource, /viewportFit:\s*"cover"/);
assert.match(layoutSource, /<PwaGameShell\s*\/>/);

const shellSource = readFileSync(path.join(webRoot, "app", "components", "pwa-game-shell.tsx"), "utf8");
for (const contract of [
  "beforeinstallprompt",
  "appinstalled",
  "requestFullscreen",
  "display-mode: standalone",
  "navigatorWithStandalone.standalone",
  'orientation.lock("landscape")',
  "standalone || fullscreenActive",
]) {
  assert.ok(shellSource.includes(contract), `PWA shell includes ${contract}`);
}

const cssSource = ["globals.css", "pwa-game-shell.css"]
  .map((fileName) => readFileSync(path.join(webRoot, "app", fileName), "utf8"))
  .join("\n");
assert.match(cssSource, /height:\s*100dvh/);
assert.match(cssSource, /safe-area-inset-top/);
assert.match(cssSource, /safe-area-inset-bottom/);
assert.match(cssSource, /mir-cache-progress-panel/);
assert.match(cssSource, /orientation:\s*portrait/);

console.log("PWA shell contract passed");
