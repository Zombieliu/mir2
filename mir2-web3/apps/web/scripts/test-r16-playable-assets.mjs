import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { R16_PLAYABLE_KIND, R16_MONSTER_LIBRARIES, R16_UI_ASSET_PATHS, assertR16PlayableAssetPath,
  R16_COMMON_UI_ASSET_PATHS, R16_FIRSTPLAY_ASSET_PATHS, R16_FIRSTPLAY_LIBRARIES, R16_FIRSTPLAY_MONSTER_RAW_PATHS,
  R16_FOREST_CAVE_BASE_SHA256, R16_FOREST_CAVE_SCOPE, R16_FOREST_CAVE_PATHS,
  R16_ROUTE_ASSET_PATHS, R16_ROUTE_SCOPE,
  collectR16PlayableAssets, materializeR16PlayableAssetFiles, readR16RegularFile, selectR16PlayableAssetsClosure, validateR16PlayableManifest, verifyR16PlayableAssetFiles } from "./lib/r16-playable-assets.mjs";
import { copySelectedPublic, copyPortableStandalone, selectThinPublicEntry } from "./lib/portable-next-standalone.mjs";

const hash = value => createHash("sha256").update(value).digest("hex");
const sort = values => [...new Set(values)].sort();
const parent = path.resolve(process.env.MIR2_PORTABLE_TEST_ROOT ?? os.tmpdir());
await fs.mkdir(parent, { recursive: true });
async function fixture(t, { uiScope = "r16", physical = true } = {}) {
  const directory = await fs.mkdtemp(path.join(parent, "r16-playable-test-")), manifestPath = path.join(directory, "manifest.json"), root = `${manifestPath}.public`;
  t.after(async () => {
    const absolute = path.resolve(directory), relative = path.relative(parent, absolute);
    assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative) && path.dirname(absolute) === parent
      && path.basename(absolute).startsWith("r16-playable-test-"));
    await fs.rm(absolute, { recursive: true, force: true });
  });
  const data = new Map();
  const add = (file, contents = Buffer.from(`image:${file}`)) => { data.set(file, Buffer.from(contents)); return file; };
  const common = uiScope === "common";
  const requiredPaths = { ui: [...(common ? R16_COMMON_UI_ASSET_PATHS : R16_UI_ASSET_PATHS)], map0: [add("original-map/WemadeMir2/Tiles/1.png")], map0141: [], monsters: [],
    ...(common ? { firstplay: [...R16_FIRSTPLAY_ASSET_PATHS] } : {}) };
  for (const file of requiredPaths.ui) add(file);
  if (common) {
    for (const file of requiredPaths.firstplay) add(file);
    for (const file of R16_FIRSTPLAY_MONSTER_RAW_PATHS) requiredPaths.monsters.push(add(file));
  }
  requiredPaths.map0141.push(add("generated/crystal-packs/full/index.json", '{ "kind": "fixture-index" }\n'),
    add("generated/crystal-packs/full/libraries/maps/123-abc.json", '{ "kind": "fixture-library" }\n'));
  for (let i = 0; i < 6; i++) { const bytes = Buffer.from(`full-pack-fixture-${i}`), sha = hash(bytes);
    requiredPaths.map0141.push(add(`generated/crystal-packs/full/pages/${sha.slice(0, 2)}/${sha}.png`, bytes)); }
  for (const library of R16_MONSTER_LIBRARIES) requiredPaths.monsters.push(add(`original-ui/${library}/0.png`),
    add(`original-ui/${library}/meta.json`, '{\n "count": 1, "id": 9007199254740993\n}\n'));
  const sourcePins = { map0MapSha256: "1".repeat(64), map0141MapSha256: "2".repeat(64),
    fullPackIndexSha256: hash(data.get("generated/crystal-packs/full/index.json")) };
  if (common) Object.assign(sourcePins, { presentSoundsSha256: "3".repeat(64), soundEventsSha256: "4".repeat(64),
    soundIndexSha256: hash(data.get("original-ui/sound-index.generated.json")) });
  for (const category of Object.keys(requiredPaths)) { requiredPaths[category] = sort(requiredPaths[category]); sourcePins[`${category}PathsSha256`] = hash(JSON.stringify(requiredPaths[category])); }
  const files = [...data].map(([file, bytes]) => ({ path: file, bytes: bytes.length, sha256: hash(bytes) })).sort((a, b) => a.path < b.path ? -1 : 1);
  const manifest = { schemaVersion: common ? 2 : 1, kind: R16_PLAYABLE_KIND, ...(common ? { uiScope } : {}), profileId: "platinum_fiveclass", maps: ["0", "0141"],
    monsterLibraries: [...R16_MONSTER_LIBRARIES], requiredPaths, sourcePins, files, totalBytes: files.reduce((n, file) => n + file.bytes, 0) };
  if (physical) for (const [relative, bytes] of data) { const file = path.join(root, relative); await fs.mkdir(path.dirname(file), { recursive: true }); await fs.writeFile(file, bytes); }
  const bytes = Buffer.from(JSON.stringify(manifest));
  await fs.writeFile(manifestPath, bytes);
  return { directory, root, manifest, data, options: { manifestPath, manifestSha256: hash(bytes) } };
}

test("R16 source UI closure is exactly 319 including Wizard layers and the native NPC sale/repair skin", async () => {
  assert.equal(R16_UI_ASSET_PATHS.length, 319);
  for (const first of [600, 880]) for (let i = 0; i < 16; i++) assert(R16_UI_ASSET_PATHS.includes(`original-ui/ChrSel/${first + i}.png`));
  const source = await fs.readFile(new URL("../app/components/original-client-game-shop.tsx", import.meta.url), "utf8");
  assert.match(source, /original-ui\/Prguse2\/351\.png/);
  assert.match(source, /sprite\(290, 291, 292\)/); assert.match(source, /sprite\(293, 294, 295\)/);
  const added = ["original-ui/Prguse2/351.png", ...Array.from({ length: 6 }, (_, i) => `original-ui/Title/${290 + i}.png`)];
  const options = { runtimeVersion: "fixture", runtimeFiles: new Set() };
  for (const file of added) {
    assert(R16_UI_ASSET_PATHS.includes(file));
    assert.equal(selectThinPublicEntry(file, false, options), false, "ordinary thin selection is unchanged");
    assert.equal(selectThinPublicEntry(file, false, { ...options, r16PlayableFiles: new Set(R16_UI_ASSET_PATHS) }), true);
  }
  for (const index of [360, 361, 362]) assert.equal(selectThinPublicEntry(`original-ui/Prguse2/${index}.png`, false, options), true);
  assert.throws(() => assertR16PlayableAssetPath("original-ui/Title/296.png"), /outside/);
});

test("no opt-in preserves the existing selector and empty closure", async () => {
  assert.deepEqual(await selectR16PlayableAssetsClosure("unused"), { files: new Set(), entries: [], bytes: 0, manifest: null, manifestSha256: null });
  assert.equal(selectThinPublicEntry("original-ui/ChrSel/600.png", false, { runtimeVersion: "fixture", runtimeFiles: new Set() }), false);
  assert.equal(selectThinPublicEntry("generated/native-map-keyed/manifest.json", false, { runtimeVersion: "fixture", runtimeFiles: new Set() }), false);
});

test("forest/cave frames require an explicit append scope and retain finite library boundaries", () => {
  assert.equal(R16_FOREST_CAVE_BASE_SHA256, "e7e13ea2aeb9cde489f6bc2a3d23927bcb72e0e4eb0a0ddfdbdc2289a7fdeb18");
  assert.deepEqual(R16_FOREST_CAVE_SCOPE.maps, ["D001"]);
  assert.deepEqual(Object.values(R16_FOREST_CAVE_PATHS).map(paths => paths.length), [216, 832, 244]);
  for (const paths of Object.values(R16_FOREST_CAVE_PATHS)) for (const file of paths) {
    assert.throws(() => assertR16PlayableAssetPath(file), /outside/);
    assert.equal(assertR16PlayableAssetPath(file, { forestCave: true }), file);
  }
  for (const file of ["original-ui/Monster/022/244.png", "original-ui/CWeapon/05/832.png",
    "original-ui/NPC/04/42.png", "original-ui/NPC/83/12.png", "original-ui/Monster/023/0.png"])
    assert.throws(() => assertR16PlayableAssetPath(file, { forestCave: true }), /outside/);
});

test("legacy common manifests cannot silently add forest/cave frames or a later map", async t => {
  const { manifest } = await fixture(t, { uiScope: "common", physical: false });
  for (const file of [R16_FOREST_CAVE_PATHS.forestPlayer[0], R16_FOREST_CAVE_PATHS.caveMonster[0]]) {
    const broken = structuredClone(manifest);
    broken.requiredPaths.monsters = sort([...broken.requiredPaths.monsters, file]);
    broken.sourcePins.monstersPathsSha256 = hash(JSON.stringify(broken.requiredPaths.monsters));
    assert.throws(() => validateR16PlayableManifest(broken), /outside/);
  }
  const wrongMap = structuredClone(manifest); wrongMap.maps.push("D001");
  assert.throws(() => validateR16PlayableManifest(wrongMap), /wrong R16 scope/);
  const wrongAppend = structuredClone(manifest); wrongAppend.schemaVersion = 3;
  assert.throws(() => validateR16PlayableManifest(wrongAppend), /fields/);
});

test("map route closes only its exact monster libraries and legacy background alternatives", () => {
  assert.deepEqual(R16_ROUTE_SCOPE.monsterLibraries, ["Monster/015", "Monster/019", "Monster/020", "Monster/021", "Monster/022", "Monster/023"]);
  assert.equal(R16_ROUTE_ASSET_PATHS.mapRouteMonsters.length, 969);
  assert.equal(R16_ROUTE_ASSET_PATHS.transitionSamples.length, 7);
  for (const paths of Object.values(R16_ROUTE_ASSET_PATHS)) for (const file of paths) {
    assert.throws(() => assertR16PlayableAssetPath(file, { forestCave: true }), /outside/);
    assert.equal(assertR16PlayableAssetPath(file, { forestCave: true, mapRoute: true }), file);
  }
  for (const file of ["original-ui/Monster/015/22.png", "original-ui/Monster/019/224.png",
    "original-ui/Monster/022/244.png", "original-ui/Monster/102/0.png", "debug/map-samples/smtile-999.png"])
    assert.throws(() => assertR16PlayableAssetPath(file, { forestCave: true, mapRoute: true }), /outside/);
});

test("manifest path and SHA are a required pair and bind exact bytes", async t => {
  const f = await fixture(t);
  for (const options of [{ manifestPath: f.options.manifestPath }, { manifestSha256: f.options.manifestSha256 },
    { ...f.options, manifestPath: "relative.json" }, { ...f.options, manifestSha256: "0".repeat(64) }])
    await assert.rejects(selectR16PlayableAssetsClosure(f.root, options), /manifest requires|pin mismatch/);
  assert.equal((await selectR16PlayableAssetsClosure(f.root, f.options)).entries.length, f.manifest.files.length);
});

test("traversal, absolute, secret, unsupported and duplicate-case paths are refused", async t => {
  for (const value of ["../x", "/original-ui/ChrSel/0.png", "C:/outside.png", "C:\\outside.png", "original-ui//ChrSel/0.png",
    "original-ui/ChrSel/../0.png", "original-ui/.env", "original-ui/private.key", "generated/crystal-packs/full/private.json",
    "original-map/WemadeMir2/Objects/0.png:secret", "original-ui/Monster/102/0.png"])
    assert.throws(() => assertR16PlayableAssetPath(value), /unsafe|outside/);
  const f = await fixture(t), duplicate = structuredClone(f.manifest);
  duplicate.files.push({ ...duplicate.files[0] }); duplicate.totalBytes += duplicate.files[0].bytes;
  assert.throws(() => validateR16PlayableManifest(duplicate), /duplicate/);
  const aliased = structuredClone(f.manifest); aliased.files.push({ ...aliased.files[0], path: aliased.files[0].path.toUpperCase() });
  assert.throws(() => validateR16PlayableManifest(aliased), /outside|duplicate/);
});

test("missing requirements, entries and physical files all fail closed", async t => {
  const f = await fixture(t);
  const missing = structuredClone(f.manifest); missing.files.shift();
  assert.throws(() => validateR16PlayableManifest(missing), /required file/);
  const missingUi = structuredClone(f.manifest); missingUi.requiredPaths.ui.shift();
  missingUi.sourcePins.uiPathsSha256 = hash(JSON.stringify(missingUi.requiredPaths.ui));
  assert.throws(() => validateR16PlayableManifest(missingUi), /R16 UI/);
  await fs.unlink(path.join(f.root, "original-ui/ChrSel/600.png"));
  await assert.rejects(selectR16PlayableAssetsClosure(f.root, f.options), /ENOENT/);
});

test("equal-length byte tampering and declared size mismatches fail", async t => {
  const f = await fixture(t), file = "original-ui/ChrSel/600.png", before = f.data.get(file);
  await fs.writeFile(path.join(f.root, file), Buffer.alloc(before.length, 120));
  await assert.rejects(verifyR16PlayableAssetFiles(f.root, f.manifest), /size\/SHA256/);
  const wrong = structuredClone(f.manifest); wrong.files[0].bytes++; wrong.totalBytes++;
  await assert.rejects(verifyR16PlayableAssetFiles(f.root, wrong), /size\/SHA256/);
});

test("a selected junction or symlink is never followed", async t => {
  const f = await fixture(t), original = path.join(f.root, "original-ui/ChrSel"), target = path.join(f.directory, "elsewhere");
  await fs.rename(original, target);
  await fs.symlink(target, original, process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(verifyR16PlayableAssetFiles(f.root, f.manifest), /non-regular directory/);
});

test("packaging requires its physical sidecar and does not fall back to another public tree", async t => {
  const f = await fixture(t), elsewhere = path.join(f.directory, "unbound-public");
  await fs.rename(f.root, elsewhere);
  await assert.rejects(selectR16PlayableAssetsClosure(elsewhere, f.options), /ENOENT/);
  await fs.symlink(elsewhere, f.root, process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(selectR16PlayableAssetsClosure(elsewhere, f.options), /real directory/);
});

test("the named development full-pack source is materialized as a finite physical closure", async t => {
  const f = await fixture(t), source = path.join(f.root, "generated/crystal-packs/full"), physical = path.join(f.directory, "physical-full");
  for (const destination of [f.root, path.join(f.root, "nested"), path.join(f.root, "..still-inside")])
    await assert.rejects(materializeR16PlayableAssetFiles(f.root, destination, f.manifest), /overlaps/);
  await fs.rename(source, physical);
  await fs.writeFile(path.join(physical, "not-selected.json"), "do not package this");
  await fs.symlink(physical, source, process.platform === "win32" ? "junction" : "dir");
  const destination = path.join(f.directory, "physical-copy");
  assert.equal((await materializeR16PlayableAssetFiles(f.root, destination, f.manifest)).verified, true);
  assert.equal((await fs.lstat(path.join(destination, "generated/crystal-packs/full"))).isSymbolicLink(), false);
  await assert.rejects(fs.stat(path.join(destination, "generated/crystal-packs/full/not-selected.json")), /ENOENT/);
  await assert.rejects(materializeR16PlayableAssetFiles(f.root, destination, f.manifest), /EEXIST/);
});

test("verified finite opt-in copies only selected additions and preserves JSON bytes", async t => {
  const f = await fixture(t), closure = await selectR16PlayableAssetsClosure(f.root, f.options), destination = path.join(f.directory, "copy");
  const select = (relative, directory) => selectThinPublicEntry(relative, directory, { runtimeVersion: "fixture", runtimeFiles: new Set(), r16PlayableFiles: closure.files });
  await fs.writeFile(path.join(f.root, "original-ui/ChrSel/unselected.png"), "do not copy");
  const result = await copySelectedPublic({ sourceRoot: f.root, destinationRoot: destination, selection: select, preservePublicFiles: closure.files });
  assert.equal(result.jsonCompactions.length, 0);
  assert.equal((await verifyR16PlayableAssetFiles(destination, f.manifest)).verified, true);
  await assert.rejects(fs.stat(path.join(destination, "original-ui/ChrSel/unselected.png")), /ENOENT/);
  const stand = path.join(f.directory, "standalone"), app = path.join(stand, "app/public");
  await fs.mkdir(path.dirname(app), { recursive: true });
  await fs.cp(f.root, app, { recursive: true });
  const standalone = path.join(f.directory, "standalone-copy");
  await copyPortableStandalone({ sourceRoot: stand, destinationRoot: standalone, appRelativePath: "app", publicSelection: select, preservePublicFiles: closure.files });
  await verifyR16PlayableAssetFiles(path.join(standalone, "app/public"), f.manifest);
});

test("ordinary JSON compaction remains enabled outside byte-preserved opt-in", async t => {
  const f = await fixture(t), destination = path.join(f.directory, "compact");
  const copy = await copySelectedPublic({ sourceRoot: f.root, destinationRoot: destination, selection: () => true });
  assert(copy.jsonCompactions.length >= 13);
  await assert.rejects(verifyR16PlayableAssetFiles(destination, f.manifest), /size\/SHA256/);
});

test("common UI is a complete finite catalog with original scope unchanged and no world sprite wildcard", () => {
  assert.equal(R16_UI_ASSET_PATHS.length, 319);
  assert.equal(R16_COMMON_UI_ASSET_PATHS.length, 4274);
  assert.equal(hash(JSON.stringify(R16_COMMON_UI_ASSET_PATHS)), "c847655c191f2bde24ba84eace1ce542aaa9a9a60849c448f3a1f1cc287dac75");
  assert(R16_UI_ASSET_PATHS.every(file => R16_COMMON_UI_ASSET_PATHS.includes(file)));
  const counts = { Prguse: 733, Prguse2: 403, UI_32bit: 5, ChrSel: 238, Title: 360, Gate: 72, Help: 43, Items: 1004,
    StateItem: 219, BuffIcon: 266, MagIcon: 225, MagIcon2: 225, GuildSkill: 49, MapLinkIcon: 18, MMap: 228,
    Cursors: 11, Sound: 148, fonts: 17, "gdi-text": 9 };
  for (const [library, count] of Object.entries(counts))
    assert.equal(R16_COMMON_UI_ASSET_PATHS.filter(file => file.startsWith(`original-ui/${library}/`)).length, count, library);
  for (const file of [...R16_COMMON_UI_ASSET_PATHS, ...R16_FIRSTPLAY_ASSET_PATHS]) assertR16PlayableAssetPath(file);
  for (const file of ["original-ui/DNItems/0.png", "original-ui/Prguse/2444.png", "original-ui/Cursors/arbitrary.CUR",
    "original-ui/Sound/arbitrary.wav", "original-ui/fonts/private.key", "debug/map-samples/another.png",
    "original-ui/NPC/02/0.png", "original-ui/CArmour/02/0.png"])
    assert.throws(() => assertR16PlayableAssetPath(file), /outside/);
  assert.equal(R16_FIRSTPLAY_LIBRARIES.length, 14); assert.equal(R16_FIRSTPLAY_ASSET_PATHS.length, 5667);
  assert.equal(hash(JSON.stringify(R16_FIRSTPLAY_ASSET_PATHS)), "d779921fbc65769396388c1f7f18a8eaf72fb5bf3161c21384dc85f8f1749f15");
  const previousFirstplay = R16_FIRSTPLAY_ASSET_PATHS.filter(file => !file.startsWith("original-ui/Monster/016/"));
  assert.equal(previousFirstplay.length, 5644);
  assert.equal(hash(JSON.stringify(previousFirstplay)), "1dc586291ae589af389d08df4c9eacaa5d8f845c745d19c3422c30edfe6fb711");
  const previousLibraries = new Set(["000", "003", "004", "005", "139"]);
  const previousMonsterFallback = R16_FIRSTPLAY_MONSTER_RAW_PATHS.filter(file => previousLibraries.has(file.split("/")[2]));
  assert.equal(previousMonsterFallback.length, 1098);
  assert.equal(hash(JSON.stringify(previousMonsterFallback)), "077d11b59453e6f3189134658a9c149dd173b9a496ed97ab6bc0c550f7eac831");
  assert.equal(R16_FIRSTPLAY_MONSTER_RAW_PATHS.length, 2602);
  for (const file of ["original-ui/CArmour/00/1615.png", "original-ui/CArmour/01/1615.png", "original-ui/CHair/00/1439.png",
    "original-ui/CWeapon/00/831.png", "original-ui/NPC/94/11.png", "debug/map-samples/smtile-0.png"])
    assert(R16_FIRSTPLAY_ASSET_PATHS.includes(file));
});

test("ordinary newcomer death/revive retains all 22 Monster/016 frames without expanding the R16 respawn set", async t => {
  const prefix = "original-ui/Monster/016/", publicRoot = fileURLToPath(new URL("../public/", import.meta.url));
  const bytes = await readR16RegularFile(publicRoot, `${prefix}meta.json`);
  assert.equal(hash(bytes), "3744a0e09c49255a299ee0d61261155fa03522c9923118b098a8a112abe043ca");
  const meta = JSON.parse(bytes);
  assert.equal(meta.version, 3); assert.equal(meta.count, 22);
  assert.deepEqual(meta.frames.map(frame => frame.index).sort((a, b) => a - b), Array.from({ length: 22 }, (_, i) => i));
  const paths = sort([...meta.frames.map(frame => `${prefix}${frame.index}.png`), `${prefix}meta.json`]);
  assert.deepEqual(R16_FIRSTPLAY_ASSET_PATHS.filter(file => file.startsWith(prefix)), paths);
  assert.equal(R16_MONSTER_LIBRARIES.includes("Monster/016"), false, "the active R16 respawn whitelist stays exact");
  const entries = [];
  for (const frame of meta.frames) {
    const file = `${prefix}${frame.index}.png`;
    assert.equal(frame.path, `/${file}`); assert(frame.width > 0 && frame.height > 0);
    const png = await readR16RegularFile(publicRoot, file);
    assert.deepEqual([...png.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
    entries.push([frame.index, png.length, hash(png)]);
  }
  assert.equal(entries.reduce((sum, entry) => sum + entry[1], 0), 849872);
  assert.equal(hash(JSON.stringify(entries)), "69a56fc1c20cb30906f29589a5581ff35a57a3254f258a4012d37b3435d046cf");
  const f = await fixture(t, { uiScope: "common", physical: false });
  for (const file of paths) {
    const broken = structuredClone(f.manifest);
    broken.requiredPaths.firstplay = broken.requiredPaths.firstplay.filter(value => value !== file);
    broken.sourcePins.firstplayPathsSha256 = hash(JSON.stringify(broken.requiredPaths.firstplay));
    const removed = broken.files.find(entry => entry.path === file);
    broken.files = broken.files.filter(entry => entry.path !== file); broken.totalBytes -= removed.bytes;
    assert.throws(() => validateR16PlayableManifest(broken), /missing\/extra required first-play asset/, file);
  }
});

test("common fonts are exactly the bundled Windows families and licenses with no font wildcard", async () => {
  const publicRoot = fileURLToPath(new URL("../public/", import.meta.url));
  const sourceRoot = fileURLToPath(new URL("../../game-client/platform-windows/assets/fonts/", import.meta.url));
  const families = ["NotoSans", "NotoSansDevanagari", "NotoSansThai", "NotoSansArabic"];
  const mappings = [
    ...families.flatMap(family => [`${family}-Regular.ttf`, `${family}-Bold.ttf`, `OFL-${family}.txt`].map(name => [name, name])),
    ["NotoSansTC.ttf", "NotoSansTC.ttf"], ["OFL.txt", "OFL-NotoSansTC.txt"],
  ];
  const prefix = "original-ui/fonts/";
  const paths = mappings.map(([, destination]) => `${prefix}${destination}`);
  assert.deepEqual(R16_COMMON_UI_ASSET_PATHS.filter(file => file.startsWith(prefix)),
    sort([...paths, `${prefix}NotoSansCJKsc-Regular.otf`, `${prefix}OFL-NotoSansCJK.txt`, `${prefix}README.md`]));
  const options = { runtimeVersion: "fixture", runtimeFiles: new Set() };
  let addedBytes = 0;
  for (const [source, destination] of mappings) {
    const file = `${prefix}${destination}`;
    const original = await readR16RegularFile(sourceRoot, source), copied = await readR16RegularFile(publicRoot, file);
    assert.deepEqual(copied, original, `${destination} must retain the exact bundled Windows bytes`);
    addedBytes += copied.length;
    if (destination.endsWith(".txt")) assert.match(copied.toString("utf8"), /SIL OPEN FONT LICENSE/);
    else assert.deepEqual([...copied.subarray(0, 4)], [0, 1, 0, 0], `${destination} TrueType header`);
    assert.equal(selectThinPublicEntry(file, false, options), true, "existing thin font/support-file selection is unchanged");
    assert.equal(selectThinPublicEntry(file, false, { ...options, r16PlayableFiles: new Set(paths) }), true);
  }
  assert.equal(mappings.length, 14); assert.equal(addedBytes, 14_282_608);
  for (const name of ["NotoSans-ExtraBold.ttf", "NotoSansArabic-Italic.ttf", "OFL.txt", "unselected.woff2"])
    assert.throws(() => assertR16PlayableAssetPath(`${prefix}${name}`), /outside/);
});

test("Monster/007 full raw fallback includes the complete eight-direction Standing animation pinned to its real source metadata", async () => {
  const prefix = "original-ui/Monster/007/", publicRoot = fileURLToPath(new URL("../public/", import.meta.url));
  const meta = JSON.parse(await readR16RegularFile(publicRoot, `${prefix}meta.json`));
  assert.equal(meta.version, 3); assert.equal(meta.count, 448); assert.equal(meta.frames.length, 448);
  assert.deepEqual(meta.sourceLibrary, { path: "Monster/007.Lib",
    sha256: "8e054422bc794a0bc91cbc1c423e8ae638b90ceaf0d18f6a1eab16e1b163867e", bytes: 1055507 });
  assert.equal(meta.frameSet.count, meta.frameSet.actions.length);
  const standing = meta.frameSet.actions.filter(action => action.actionName === "Standing");
  assert.equal(standing.length, 1);
  const { actionId, start, count, skip } = standing[0];
  assert.deepEqual({ actionId, start, count, skip }, { actionId: 0, start: 0, count: 4, skip: 0 });
  const indices = Array.from({ length: 8 }, (_, direction) =>
    Array.from({ length: count }, (_, frame) => start + direction * (count + skip) + frame)).flat();
  assert.deepEqual(indices, Array.from({ length: 32 }, (_, index) => index));
  const paths = indices.map(index => `${prefix}${index}.png`);
  assert.deepEqual(R16_FIRSTPLAY_MONSTER_RAW_PATHS.filter(file => file.startsWith(prefix)),
    sort(Array.from({ length: meta.count }, (_, index) => `${prefix}${index}.png`)),
    "the active raw renderer needs every action, including the complete Standing animation");
  assert(paths.every(file => R16_FIRSTPLAY_MONSTER_RAW_PATHS.includes(file)));
  const options = { runtimeVersion: "fixture", runtimeFiles: new Set() };
  for (const [offset, file] of paths.entries()) {
    const frame = meta.frames.find(frame => frame.index === indices[offset]);
    assert(frame && frame.width > 0 && frame.height > 0, file);
    assert.equal(frame.path, `/${file}`); assert.match(frame.pngSha256, /^[a-f0-9]{64}$/);
    assert.equal(hash(await readR16RegularFile(publicRoot, file)), frame.pngSha256, file);
    assertR16PlayableAssetPath(file);
    assert.equal(selectThinPublicEntry(file, false, options), false, "ordinary thin selection stays unchanged");
    assert.equal(selectThinPublicEntry(file, false, { ...options, r16PlayableFiles: new Set(R16_FIRSTPLAY_MONSTER_RAW_PATHS) }), true);
  }
});

test("all thirteen R16 monster profiles have complete physical raw fallbacks matching pinned source metadata and PNG hashes", async t => {
  const witnesses = {
    "Monster/000": [128, "e566086c2ede14e28713fde237597b9642184798e6ad7ae8a416b4ecae90c9d5"],
    "Monster/003": [232, "51d9d6d5f5f8d57429eb42bae70d66fe29aed1c893c0f46925f002fdaa78e33d"],
    "Monster/004": [232, "6308a6979f6cdca712c5e63567fc6e0646492ef0275a45a5129f6ae3c65c78d6"],
    "Monster/005": [234, "8f9326da8bfcee08f94bdf8a78b0905d778dacfd333dfd9283ced77bcc7c388f"],
    "Monster/006": [224, "5e033d4341d536f8d9d277ddb5d4ca688b67b2d9b0d2b2e6a5234116d1f355bd"],
    "Monster/007": [448, "5b9c525446c288efc2606faa86273d4372b54bee93dd5659f3edb0d1b0c51881"],
    "Monster/009": [224, "b4c87c811b0a84a2e521a29d09920a65a6bb0ea3f97b955d9fe875bfc76df11c"],
    "Monster/010": [164, "16aea2cabc63f934e28f7486f138deddfd05cb920884e3f3bd7a03703a196152"],
    "Monster/011": [176, "b326762d814b8c1fd50e611a58dd62433df8301b2b106c9c7bcbe430ae9f2d35"],
    "Monster/012": [224, "70b6c94e502bbe1a53d4f7a2f91bc9e31955338822a82e9c6e033aaddc6d6686"],
    "Monster/013": [22, "425c72e02858db35435b5f95596d1d6b9ac18695d1c082bcf0c55c31a97153d1"],
    "Monster/014": [22, "c7f49d12bc7b89c3b5c610446fab96401b73d1e8909bc1cbf04b12cea82dded9"],
    "Monster/139": [272, "73066c96bc03a05ea949a06ce0a6ffae38839bbadc81d83db3212a8d165b3d54"],
  };
  assert.deepEqual(Object.keys(witnesses), R16_MONSTER_LIBRARIES);
  const publicRoot = fileURLToPath(new URL("../public/", import.meta.url)), all = [];
  let totalBytes = 0;
  for (const [library, [count, metaSha256]] of Object.entries(witnesses)) {
    const prefix = `original-ui/${library}/`, bytes = await readR16RegularFile(publicRoot, `${prefix}meta.json`);
    assert.equal(hash(bytes), metaSha256, `${library} source metadata pin`);
    const meta = JSON.parse(bytes);
    assert.equal(meta.version, 3); assert.equal(meta.count, count); assert.equal(meta.frames.length, count);
    assert.equal(meta.sourceLibrary.path, `${library}.Lib`); assert.match(meta.sourceLibrary.sha256, /^[a-f0-9]{64}$/);
    assert(Number.isSafeInteger(meta.sourceLibrary.bytes) && meta.sourceLibrary.bytes > 0);
    assert.deepEqual(meta.frames.map(frame => frame.index).sort((a, b) => a - b), Array.from({ length: count }, (_, index) => index));
    const paths = meta.frames.map(frame => `${prefix}${frame.index}.png`);
    assert.deepEqual(R16_FIRSTPLAY_MONSTER_RAW_PATHS.filter(file => file.startsWith(prefix)), sort(paths), `${library} complete action set`);
    assert.deepEqual((await fs.readdir(path.join(publicRoot, prefix))).filter(file => file.endsWith(".png")).sort(),
      meta.frames.map(frame => `${frame.index}.png`).sort(), `${library} physical catalog has no omitted PNGs`);
    for (const frame of meta.frames) {
      const file = `${prefix}${frame.index}.png`;
      assert.equal(frame.path, `/${file}`); assert.match(frame.pngSha256, /^[a-f0-9]{64}$/);
      assert(frame.width > 0 && frame.height > 0); assert.equal(frame.hasMask, false); assert.equal(frame.maskPath, null);
      const png = await readR16RegularFile(publicRoot, file);
      assert.equal(hash(png), frame.pngSha256, `${file} source PNG pin`); totalBytes += png.length;
      assertR16PlayableAssetPath(file); all.push(file);
    }
  }
  assert.deepEqual(R16_FIRSTPLAY_MONSTER_RAW_PATHS, sort(all)); assert.equal(all.length, 2602);
  assert.equal(totalBytes, 10957186);
  t.diagnostic(`Verified 13 source metadata SHA256 pins and all ${all.length} physical PNG hashes (${totalBytes} bytes).`);
});

test("common manifests reject any missing raw frame across all thirteen profiles even with otherwise consistent file and path pins", async t => {
  const f = await fixture(t, { uiScope: "common", physical: false });
  assert.equal(R16_FIRSTPLAY_MONSTER_RAW_PATHS.length, 2602);
  const files = new Map(f.manifest.files.map(entry => [entry.path, entry]));
  for (const file of R16_FIRSTPLAY_MONSTER_RAW_PATHS) {
    assert(f.manifest.requiredPaths.monsters.includes(file), `${file} must be required before removal`);
    const monsters = f.manifest.requiredPaths.monsters.filter(value => value !== file);
    const broken = { ...f.manifest, requiredPaths: { ...f.manifest.requiredPaths, monsters },
      sourcePins: { ...f.manifest.sourcePins, monstersPathsSha256: hash(JSON.stringify(monsters)) },
      files: f.manifest.files.filter(entry => entry.path !== file), totalBytes: f.manifest.totalBytes - files.get(file).bytes };
    assert.throws(() => validateR16PlayableManifest(broken), /incomplete first-play monster raw fallback/, file);
  }
  t.diagnostic(`Independently removed and rejected all ${R16_FIRSTPLAY_MONSTER_RAW_PATHS.length} required raw frames.`);
});

test("common scope requires every pinned UI/first-play/raw family while schema1 stays exact", async t => {
  const f = await fixture(t, { uiScope: "common", physical: false });
  assert.equal(validateR16PlayableManifest(f.manifest), f.manifest);
  for (const category of ["ui", "firstplay", "monsters"]) {
    const broken = structuredClone(f.manifest);
    const file = category === "monsters" ? "original-ui/Monster/000/127.png" : broken.requiredPaths[category][0];
    broken.requiredPaths[category] = broken.requiredPaths[category].filter(value => value !== file);
    broken.sourcePins[`${category}PathsSha256`] = hash(JSON.stringify(broken.requiredPaths[category]));
    assert.throws(() => validateR16PlayableManifest(broken), /R16 UI|first-play/);
  }
  for (const pin of ["presentSoundsSha256", "soundEventsSha256", "soundIndexSha256"]) {
    const broken = structuredClone(f.manifest); delete broken.sourcePins[pin];
    assert.throws(() => validateR16PlayableManifest(broken), /common source pin/);
  }
  const wrongSoundIndex = structuredClone(f.manifest); wrongSoundIndex.sourcePins.soundIndexSha256 = "0".repeat(64);
  assert.throws(() => validateR16PlayableManifest(wrongSoundIndex), /sound index pin/);
  const mislabeled = structuredClone(f.manifest); mislabeled.schemaVersion = 1;
  assert.throws(() => validateR16PlayableManifest(mislabeled), /fields|scope/);
  const invented = structuredClone(f.manifest); invented.uiScope = "full-world";
  assert.throws(() => validateR16PlayableManifest(invented), /fields|scope/);
  await assert.rejects(collectR16PlayableAssets({ webRoot: "unused", uiScope: "full-world" }), /unsupported UI scope/);
});

test("the complete raw catalog has an explicit 22k engineering file bound while 360MiB and 8MiB byte guards stay unchanged", async t => {
  const f = await fixture(t, { uiScope: "common", physical: false });
  // The active single-image renderer's measured full closure is 21,019 files;
  // 22,000 is the smallest next-thousand engineering cap. These synthetic
  // allowed-family entries exercise that structural cap, not asset selection.
  const atLimit = structuredClone(f.manifest);
  while (atLimit.files.length < 22_000) {
    const file = { path: `original-ui/Monster/000/${100_000 + atLimit.files.length}.png`, bytes: 1, sha256: "1".repeat(64) };
    atLimit.files.push(file); atLimit.requiredPaths.monsters.push(file.path); atLimit.totalBytes++;
  }
  atLimit.files.sort((a, b) => a.path < b.path ? -1 : 1);
  atLimit.requiredPaths.monsters = sort(atLimit.requiredPaths.monsters);
  atLimit.sourcePins.monstersPathsSha256 = hash(JSON.stringify(atLimit.requiredPaths.monsters));
  assert.equal(validateR16PlayableManifest(atLimit), atLimit);
  const tooMany = structuredClone(atLimit);
  tooMany.files.push({ path: "original-ui/Monster/000/999999.png", bytes: 1, sha256: "1".repeat(64) });
  assert.throws(() => validateR16PlayableManifest(tooMany), /finite file count/);
  const tooLarge = structuredClone(f.manifest); tooLarge.files[0].bytes = 360 * 1024 * 1024;
  tooLarge.totalBytes = tooLarge.files.reduce((n, file) => n + file.bytes, 0);
  assert.throws(() => validateR16PlayableManifest(tooLarge), /byte total\/budget/);
  const oversizedManifest = Buffer.alloc(8 * 1024 * 1024 + 1, 32);
  await fs.writeFile(f.options.manifestPath, oversizedManifest);
  await assert.rejects(selectR16PlayableAssetsClosure(f.root,
    { manifestPath: f.options.manifestPath, manifestSha256: hash(oversizedManifest) }), /non-regular\/empty\/oversize file/);
});

test("the explicit raw Sound source fills only selected missing clips into a physical SHA-verified sidecar", async t => {
  const f = await fixture(t, { uiScope: "common" }), soundRoot = path.join(f.directory, "raw-Sound");
  const selected = "original-ui/Sound/107.wav";
  await fs.mkdir(soundRoot);
  await fs.rename(path.join(f.root, selected), path.join(soundRoot, "107.wav"));
  await fs.writeFile(path.join(soundRoot, "unselected.wav"), "not selected");
  await fs.writeFile(path.join(soundRoot, "private.key"), "never selected");
  const destination = path.join(f.directory, "common-physical-copy");
  const result = await materializeR16PlayableAssetFiles(f.root, destination, f.manifest, { soundRoot });
  assert.equal(result.verified, true); assert.equal(result.files, f.manifest.files.length);
  assert.deepEqual(await fs.readFile(path.join(destination, selected)), f.data.get(selected));
  assert.equal((await fs.lstat(path.join(destination, "original-ui/Sound"))).isSymbolicLink(), false);
  await assert.rejects(fs.stat(path.join(destination, "original-ui/Sound/unselected.wav")), /ENOENT/);
  await assert.rejects(fs.stat(path.join(destination, "original-ui/Sound/private.key")), /ENOENT/);
  await assert.rejects(fs.stat(path.join(f.root, selected)), /ENOENT/, "source public is unchanged");
});
