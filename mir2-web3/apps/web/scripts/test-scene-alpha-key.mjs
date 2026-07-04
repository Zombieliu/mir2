// Behavioral tests for lib/scene-alpha-key.ts (map object black-key + edge feather +
// dither-shadow resolve).
//
// Covers the pure flood-fill algorithm (edge-connected black -> transparent, interior
// black preserved, brightness feather), the pass-2 dither-shadow resolve (checkerboard ->
// uniform translucent shadow, while thin/solid dark art is left intact), AND the mechanism
// the off-thread worker depends on:
// the algorithm is serialized via `.toString()` and re-evaluated, so we re-evaluate it here
// and assert it behaves identically to the directly-imported function.
//
// Pure logic only: no DOM, no Worker. The .ts source is transpiled in-memory via the
// `typescript` devDependency (same harness as the other test-*.mjs scripts).

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
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

const { alphaKeyMapObjectPixels } = loadTypeScriptModule(
  new URL("../lib/scene-alpha-key.ts", import.meta.url),
);

// Build a w*h RGBA buffer from a per-pixel [r,g,b,a] factory.
function makePixels(width, height, at) {
  const pixels = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const [r, g, b, a] = at(x, y);
      const off = (y * width + x) * 4;
      pixels[off] = r;
      pixels[off + 1] = g;
      pixels[off + 2] = b;
      pixels[off + 3] = a;
    }
  }
  return pixels;
}
const alphaAt = (pixels, width, x, y) => pixels[(y * width + x) * 4 + 3];

let groups = 0;

// 1) All-black opaque: every pixel is edge-connected dark -> all alpha keyed to 0.
{
  const w = 4;
  const h = 4;
  const px = makePixels(w, h, () => [0, 0, 0, 255]);
  const changed = alphaKeyMapObjectPixels(px, w, h);
  assert.equal(changed, 16, "all 16 black pixels should change");
  for (let i = 0; i < w * h; i += 1) {
    assert.equal(px[i * 4 + 3], 0, "every black pixel should become fully transparent");
  }
  groups += 1;
}

// 2) Bright center surrounded by black: ring keyed out, center (brightness > feather) kept.
{
  const w = 3;
  const h = 3;
  const px = makePixels(w, h, (x, y) => (x === 1 && y === 1 ? [200, 200, 200, 255] : [0, 0, 0, 255]));
  const changed = alphaKeyMapObjectPixels(px, w, h);
  assert.equal(changed, 8, "the 8 surrounding black pixels should change");
  assert.equal(alphaAt(px, w, 1, 1), 255, "bright center must stay fully opaque");
  assert.equal(alphaAt(px, w, 0, 0), 0, "a corner black pixel must be keyed out");
  groups += 1;
}

// 3) Bright opaque border ring, black interior: fill has no dark border seed, so the
//    interior black is preserved (the key must not bleed through an opaque edge).
{
  const w = 5;
  const h = 5;
  const px = makePixels(w, h, (x, y) => {
    const onBorder = x === 0 || y === 0 || x === w - 1 || y === h - 1;
    return onBorder ? [200, 200, 200, 255] : [0, 0, 0, 255];
  });
  const changed = alphaKeyMapObjectPixels(px, w, h);
  assert.equal(changed, 0, "no pixel should change when the border is fully opaque");
  assert.equal(alphaAt(px, w, 2, 2), 255, "interior black must be preserved");
  groups += 1;
}

// 4) The off-thread worker injects the algorithm via `.toString()` and re-evaluates it.
//    Reconstruct it the same way and assert identical behavior on a mixed input.
{
  const reconstructed = new Function(`return (${alphaKeyMapObjectPixels.toString()})`)();
  assert.equal(typeof reconstructed, "function", ".toString() must reconstruct a callable");

  const w = 6;
  const h = 6;
  const factory = (x, y) => {
    const v = (x * 37 + y * 91) % 100; // deterministic mixed brightness
    return [v, v, v, 255];
  };
  const direct = makePixels(w, h, factory);
  const viaString = makePixels(w, h, factory);
  const changedDirect = alphaKeyMapObjectPixels(direct, w, h);
  const changedString = reconstructed(viaString, w, h);
  assert.equal(changedString, changedDirect, "reconstructed fn must report the same change count");
  for (let i = 0; i < w * h * 4; i += 1) {
    assert.equal(viaString[i], direct[i], `pixel byte ${i} must match the reconstructed result`);
  }
  groups += 1;
}

// 5) Dithered shadow (a checkerboard of near-black opaque + transparent texels) resolves to a
//    uniform translucent black: dots AND gaps become rgb 0 / alpha 120, so a crisp canvas shows a
//    smooth ~50% shadow instead of a hard halftone. The checker sits on a transparent margin so
//    pass 1 (border black-key) is a no-op and this isolates the pass-2 dither resolve.
{
  const w = 9;
  const h = 9;
  const inChecker = (x, y) => x >= 2 && x <= 6 && y >= 2 && y <= 6;
  const px = makePixels(w, h, (x, y) =>
    inChecker(x, y) && (x + y) % 2 === 0 ? [8, 8, 8, 255] : [0, 0, 0, 0],
  );
  const changed = alphaKeyMapObjectPixels(px, w, h);
  assert.ok(changed > 0, "dithered shadow must be rewritten");
  const off = (x, y) => (y * w + x) * 4;
  // a dither dot (was opaque near-black, all four neighbours transparent) -> translucent black
  assert.equal(alphaAt(px, w, 4, 4), 120, "shadow dot must become translucent (alpha 120)");
  assert.equal(px[off(4, 4)], 0, "shadow dot must be pure black");
  // a dither gap (was transparent, ringed by four dots) -> filled with the same translucent black
  assert.equal(alphaAt(px, w, 4, 5), 120, "shadow gap must be filled to alpha 120");
  assert.equal(px[off(4, 5)], 0, "filled gap must be pure black");
  // the transparent margin outside the checker is untouched
  assert.equal(alphaAt(px, w, 0, 0), 0, "margin must stay transparent");
  assert.equal(alphaAt(px, w, 8, 8), 0, "margin must stay transparent");
  groups += 1;
}

// 6) A 1px-wide near-black vertical line on a transparent field is NOT a dither (its texels keep
//    dark up/down neighbours), so the all-four rule must leave it untouched — proving thin solid
//    features are never eroded. The line avoids the image border so pass 1 also leaves it.
{
  const w = 5;
  const h = 7;
  const px = makePixels(w, h, (x, y) => (x === 2 && y >= 1 && y <= 5 ? [8, 8, 8, 255] : [0, 0, 0, 0]));
  const changed = alphaKeyMapObjectPixels(px, w, h);
  assert.equal(changed, 0, "a thin dark line must not be altered");
  for (let y = 1; y <= 5; y += 1) {
    assert.equal(alphaAt(px, w, 2, y), 255, "line pixel must stay fully opaque");
    assert.equal(px[(y * w + 2) * 4], 8, "line pixel colour must be preserved");
  }
  groups += 1;
}

// 7) A solid near-black block on a transparent field keeps dark neighbours on its interior and
//    edges, so the all-four rule leaves every pixel intact (no 1px translucent halo).
{
  const w = 6;
  const h = 6;
  const inBlock = (x, y) => x >= 1 && x <= 4 && y >= 1 && y <= 4;
  const px = makePixels(w, h, (x, y) => (inBlock(x, y) ? [8, 8, 8, 255] : [0, 0, 0, 0]));
  const changed = alphaKeyMapObjectPixels(px, w, h);
  assert.equal(changed, 0, "a solid dark block must be preserved");
  assert.equal(alphaAt(px, w, 2, 2), 255, "block interior stays opaque");
  assert.equal(alphaAt(px, w, 1, 1), 255, "block corner stays opaque");
  groups += 1;
}

console.log(`scene alpha-key tests passed (${groups} groups)`);
