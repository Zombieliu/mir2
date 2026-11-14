import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import test from "node:test";
import ts from "typescript";

const file = new URL("../lib/r16-standing-sprites.ts", import.meta.url);
const pins = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-visuals.json", import.meta.url)));
const code = ts.transpileModule(readFileSync(file, "utf8"), { compilerOptions: {
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, esModuleInterop: true,
} }).outputText;
function helper() {
  const reads = [], module = { exports: {} };
  new Function("require", "module", "exports", code)(name => {
    reads.push(name); return JSON.parse(readFileSync(new URL("../lib/" + name.slice(2), import.meta.url)));
  }, module, module.exports);
  return { ...module.exports, reads };
}
const h = helper();
const group = index => h.r16StandingGroup(pins.assetReleaseId, "/original-ui/CArmour/00/" + index + ".png");
const controller = () => new AbortController();
const payload = g => JSON.parse(readFileSync(new URL("../lib/generated/r16-standing/" + g.gender + "-" + g.direction + ".json", import.meta.url)));

test("actual loaded release and exact default standing paths gate the optimization", () => {
  for (const release of [null, undefined, "older-release"]) assert.equal(h.r16StandingGroup(release, "/original-ui/CArmour/00/17.png"), null);
  for (const path of ["/original-ui/CArmour/01/17.png", "/original-ui/CHair/00/17.png", "/original-ui/AArmour/00/17.png",
    "/original-ui/CArmour/00/017.png", "/original-ui/CArmour/00/32.png", "/original-ui/CArmour/00/807.png", "/original-ui/CArmour/00/840.png"])
    assert.equal(h.r16StandingGroup(pins.assetReleaseId, path), null, path);
  for (const [base, gender] of [[0, "male"], [808, "female"]]) for (let dir = 0; dir < 8; dir++) {
    const keys = Array.from({ length: 4 }, (_, phase) => group(base + dir * 4 + phase).key);
    assert.equal(new Set(keys).size, 1); assert.equal(group(base + dir * 4).gender, gender);
    assert.equal(group(base + dir * 4).direction, dir);
  }
});

test("one actual lazy import serves four phases; unused directions and gender do not load", async () => {
  const h = helper(), g = group(17), a = controller(), b = controller();
  assert.deepEqual(h.reads, ["./generated/r16-entry-visuals.json"]);
  const [one, two] = await Promise.all([h.readR16StandingSprites(g, a.signal), h.readR16StandingSprites(group(18), b.signal)]);
  assert.equal(one, two); assert.equal(Object.keys(one).length, 8);
  assert.deepEqual(h.reads.filter(path => path.includes("/r16-standing/")), ["./generated/r16-standing/male-4.json"]);
});

test("both sexes, eight directions and all four phases retain the original PNG bytes and dimensions", async () => {
  const h = helper(), reader = h.createR16StandingSpriteReader();
  for (const [base, gender] of [[0, "male"], [808, "female"]]) {
    const bytes = readFileSync(new URL("../public" + pins.players[gender].path, import.meta.url));
    assert.equal(createHash("sha256").update(bytes).digest("hex"), pins.players[gender].sha256);
    const source = JSON.parse(bytes);
    for (let dir = 0; dir < 8; dir++) {
      const images = await reader(group(base + dir * 4), controller().signal);
      assert.equal(Object.keys(images).length, 8);
      for (const library of ["CArmour/00", "CHair/00"]) for (let phase = 0; phase < 4; phase++) {
        const index = base + dir * 4 + phase, path = "/original-ui/" + library + "/" + index + ".png";
        const f = source.libraries[library].frames.find(frame => frame.index === index), image = images[path];
        assert.equal(image.png, source.images[path]); assert.equal(image.width, f.width); assert.equal(image.height, f.height);
        const raw = Buffer.from(image.png.split(",")[1], "base64");
        assert.equal(raw.readUInt32BE(16), f.width); assert.equal(raw.readUInt32BE(20), f.height);
      }
    }
  }
  assert.equal(h.reads.filter(path => path.includes("/r16-standing/")).length, 16);
});

test("retiring one consumer does not cancel another consumer or publish to the retired owner", async () => {
  let complete, imports = 0;
  const reader = h.createR16StandingSpriteReader(() => { imports++; return new Promise(resolve => { complete = resolve; }); });
  const a = controller(), b = controller(), g = group(17);
  const one = reader(g, a.signal), two = reader(g, b.signal);
  await Promise.resolve(); a.abort(); assert.equal(await one, null); complete(payload(g));
  assert.equal(Object.keys(await two).length, 8); assert.equal(imports, 1);
});

test("a bounded observer can time out while a late chunk remains reusable", async () => {
  let complete, imports = 0;
  const reader = h.createR16StandingSpriteReader(() => { imports++; return new Promise(resolve => { complete = resolve; }); }, { timeoutMs: 5 });
  const g = group(17); assert.equal(await reader(g, controller().signal), null);
  complete(payload(g)); assert.equal(Object.keys(await reader(g, controller().signal)).length, 8); assert.equal(imports, 1);
});

test("chunk failures back off, stop after two attempts and leave the ordinary image path available", async () => {
  let calls = 0, now = 0;
  const reader = h.createR16StandingSpriteReader(async () => { calls++; throw Error("chunk failed"); }, { now: () => now });
  const g = group(17);
  assert.equal(await reader(g, controller().signal), null);
  assert.equal(await reader(g, controller().signal), null); assert.equal(calls, 1);
  now = 5000; assert.equal(await reader(g, controller().signal), null); assert.equal(calls, 2);
  now = 50000; assert.equal(await reader(g, controller().signal), null); assert.equal(calls, 2);
  assert.equal(await reader({ ...g, direction: 9 }, controller().signal), null); assert.equal(calls, 2);
});

test("wrong source, direction or missing phase cannot become a startup image map", async () => {
  const g = group(17);
  for (const alter of [p => { p.assetReleaseId = "old"; }, p => { p.sourceSha256 = "0".repeat(64); },
    p => { p.gender = "female"; }, p => { p.direction = 0; }, p => { delete p.images[Object.keys(p.images)[0]]; }]) {
    const p = payload(g); alter(p);
    const read = h.createR16StandingSpriteReader(async () => p);
    assert.equal(await read(g, controller().signal), null);
  }
});

test("startup PNGs retain each layer's geometry, reject mismatched dimensions and yield to the complete pack", () => {
  const file = new URL("../app/components/original-client-self-sprite.tsx", import.meta.url);
  const ast = ts.createSourceFile(file.pathname, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const functions = ast.statements.filter(node => ts.isFunctionDeclaration(node) && ["selfSpritePose", "selfStandingWarmFrames"].includes(node.name?.text));
  const code = ts.transpileModule(functions.map(node => node.getText(ast).replace(/^export /, "")).join("\n"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const { selfSpritePose, selfStandingWarmFrames } = new Function("originalAssetPath", code + "\nreturn {selfSpritePose,selfStandingWarmFrames};")(path => path);
  const g = group(17), p = payload(g), keys = Object.keys(p.images), bodyPath = keys[1], hairPath = keys[5];
  const frame = path => ({ path, ...p.images[path], x: path === bodyPath ? 7 : -13, y: -40, shadowX: 1, shadowY: 2, maskPath: null });
  const sprite = { body: frame(bodyPath), hair: frame(hairPath), mount: null, rearWeapons: [], frontWeapons: [], nameplateTop: -42,
    preloadFrames: keys.slice(0, 4).map(frame) };
  const pose = selfSpritePose(sprite, null, p.images);
  assert.equal(pose.layers[0].source, p.images[bodyPath].png); assert.equal(pose.layers[1].source, p.images[hairPath].png);
  assert.equal(pose.layers[0].x, 7); assert.equal(pose.layers[1].x, -13);
  assert.equal(selfStandingWarmFrames(sprite, null, p.images).length, 4);
  assert(selfStandingWarmFrames(sprite, null, p.images).every(frame => frame.source.startsWith("data:")));
  assert.equal(selfSpritePose({ ...sprite, body: { ...sprite.body, width: 999 } }, null, p.images).layers[0].source, bodyPath);
  assert.equal(selfSpritePose(sprite, { [bodyPath]: "full-pack" }, p.images).layers[0].source, "full-pack");
});

test("the release prop comes from the accepted scene, not the build's fixed-release hint", () => {
  const file = new URL("../app/page.tsx", import.meta.url);
  const ast = ts.createSourceFile(file.pathname, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const bindings = [];
  const visit = node => {
    if (ts.isJsxAttribute(node) && node.name.text === "sceneAssetReleaseId") bindings.push(node.initializer.getText(ast));
    ts.forEachChild(node, visit);
  }; visit(ast);
  assert.deepEqual(bindings, ["{loadedSceneAssetReleaseRef.current}"]);
});

test("the mounted observer retries a failed or expired read once without waiting for a turn, and retirement clears it", async () => {
  const file = new URL("../app/components/original-client-self-sprite.tsx", import.meta.url);
  const ast = ts.createSourceFile(file.pathname, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let effect;
  const visit = node => {
    if (ts.isCallExpression(node) && node.expression.getText(ast) === "useEffect" && node.arguments[0].getText(ast).includes("readR16StandingSprites")) effect = node.arguments[0];
    ts.forEachChild(node, visit);
  }; visit(ast); assert(effect);
  const compiled = ts.transpileModule("const effect = " + effect.getText(ast) + ";", { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const settle = async () => { for (let i = 0; i < 3; i++) await Promise.resolve(); };
  for (const succeeds of [true, false]) {
    let calls = 0; const timers = new Map(), updates = [], g = group(17), images = payload(g).images;
    const cleanup = new Function("group", "completeBody", "readR16StandingSprites", "setInitial", "setTimeout", "clearTimeout", compiled + "\nreturn effect();")(
      g, false, async () => ++calls === 2 && succeeds ? images : null, next => updates.push(next),
      (fn, ms) => { assert.equal(ms, 5100); timers.set(1, fn); return 1; }, id => timers.delete(id));
    await settle(); assert.equal(calls, 1); assert.equal(timers.size, 1);
    const callback = timers.get(1); timers.delete(1); callback(); await settle();
    assert.equal(calls, 2); assert.equal(timers.size, 0); assert.equal(updates.length, succeeds ? 1 : 0);
    if (succeeds) assert.deepEqual(updates[0], { key: g.key, images }); cleanup();
  }
  let calls = 0; const timers = new Map();
  const cleanup = new Function("group", "completeBody", "readR16StandingSprites", "setInitial", "setTimeout", "clearTimeout", compiled + "\nreturn effect();")(
    group(17), false, async () => { calls++; return null; }, () => assert.fail("retired observer published"),
    fn => { timers.set(1, fn); return 1; }, id => timers.delete(id));
  await settle(); assert.equal(timers.size, 1); cleanup(); assert.equal(timers.size, 0); assert.equal(calls, 1);
});
