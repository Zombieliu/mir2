// Reproducible complete source-library closure for the active classic profile.
// Run with --dataDir <Crystal/Data>; --export writes missing exact-source PNGs.
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { createHash } from "node:crypto";
import sharp from "sharp";
import { parseLibrary, allPresentFrameIndices, decodeFrameRgba, decodeMaskFrameRgba } from "./crystal-library.mjs";

const root = path.resolve(import.meta.dirname, "../../..");
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
const dataDir = option("--dataDir", process.env.CRYSTAL_CLIENT_DATA_DIR);
if (!dataDir) throw new Error("Pass --dataDir or CRYSTAL_CLIENT_DATA_DIR");
const outputDir = path.resolve(option("--outputDir", path.join(root, "apps/web/public/original-ui")));
const reportPath = path.resolve(option("--report", path.join(root, "docs/generated/player-qa/profile-actor-assets-20260921/closure.json")));
const json = async (file) => JSON.parse(await readFile(file, "utf8"));
const previousReport = args.includes("--verify") && existsSync(reportPath) ? await json(reportPath) : null;
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const profile = await json(path.join(root, "packages/game-data/data/content_profiles/platinum_176.json"));
const monsters = (await json(path.join(root, "packages/game-data/data/generated/crystal_monster_manifest.json"))).monsters;
const items = (await json(path.join(root, "packages/game-data/data/generated/crystal_item_manifest.json"))).items;
// Crystal Client/MirObjects/MonsterObject.cs BodyLibrary dispatch; never Monster/950.
function monsterLibrary(image) {
  if (image === 900 || image === 902) return "Dragon";
  if (image === 901) return null; // EvilMirBody has no independent BodyLibrary.
  if (image >= 940 && image <= 944) return `Siege/${String(image - 940).padStart(2, "0")}`;
  if (image >= 950 && image <= 964) return `Gate/${String(image - 950).padStart(2, "0")}`;
  if (image >= 10000 && image <= 10014) return `Pet/${String(image - 10000).padStart(2, "0")}`;
  if ([903, 904, 905].includes(image)) return "Monster/247";
  return `Monster/${String(image).padStart(3, "0")}`;
}
const requiredMonsters = monsters.filter((m) => profile.monsterWhitelist.includes(m.name));
// BoneFamiliar is an ordinary Taoist V2 summon, not a profile spawn whitelist entry.
const bone = monsters.find((m) => m.name === "BoneFamiliar");
if (!bone) throw new Error("Missing imported BoneFamiliar template");
const actorRequirements = [...requiredMonsters, bone].map((m) => ({ name: m.name, image: m.image, library: monsterLibrary(m.image), reason: m === bone ? "newcomer-v2-summon" : "profile-whitelist" }));
const equipment = items.filter((i) => profile.itemWhitelist.includes(i.name) && [1, 2].includes(i.item_type)).map((i) => ({ name: i.name, shape: i.shape, requiredGender: i.required_gender, library: `${i.item_type === 1 ? "CWeapon" : "CArmour"}/${String(i.shape).padStart(2, "0")}` }));
const priority = ["Monster/005", "Monster/007", "Monster/009", "Monster/019", "Monster/022", "Monster/069", "Monster/027", "Monster/029", "Monster/030", "Monster/078"];
const required = [...new Set([...priority, ...actorRequirements.map((r) => r.library).filter(Boolean), "CArmour/00", ...equipment.map((r) => r.library)])];
const report = {
  schema: "mir2-profile-actor-library-closure/1", generatedAt: new Date().toISOString(), profileId: profile.profileId, profileVersion: profile.version,
  policy: "Export every drawable source frame, including masks and both player genders, retaining source FrameSet. Empty source slots are recorded, never fabricated. Full source libraries cover all source actions; this is asset closure, not visual acceptance.",
  sourceDispatch: ["Crystal/Client/MirObjects/MonsterObject.cs:150-208", "Crystal/Client/MirGraphics/MLibrary.cs:91-100"],
  actorRequirements, equipment, libraries: [], unknown: [],
};
for (const name of profile.monsterWhitelist) if (!requiredMonsters.some((m) => m.name === name)) report.unknown.push({ type: "missing-monster-template", name });
for (const actor of actorRequirements) if (!actor.library) report.unknown.push({ type: "no-independent-body-library", ...actor });
for (const library of required) {
  const sourcePath = path.join(dataDir, `${library}.Lib`);
  if (!existsSync(sourcePath)) { report.unknown.push({ type: "missing-source-library", library }); continue; }
  const source = await readFile(sourcePath);
  const parsed = parseLibrary(source);
  const indices = allPresentFrameIndices(parsed).filter((i) => parsed.frames[i].width > 0 && parsed.frames[i].height > 0);
  const present = new Set(indices);
  let existingMeta = null;
  try { existingMeta = await json(path.join(outputDir, library, "meta.json")); } catch {}
  const existing = new Set(existingMeta?.frames?.map((frame) => frame.index) ?? []);
  report.libraries.push({ library, sourceBytes: source.length, sourceSha256: sha256(source), sourceCount: parsed.count, drawableFrames: indices.length,
    sourceEmptySlots: Array.from({ length: parsed.count }, (_, i) => i).filter((i) => !present.has(i)),
    sourceZeroGeometrySlots: Array.from({ length: parsed.count }, (_, i) => i).filter((i) => {
      const offset = source.readInt32LE((parsed.version >= 3 ? 12 : 8) + i * 4);
      return offset > 0 && offset + 4 <= source.length && (source.readInt16LE(offset) <= 0 || source.readInt16LE(offset + 2) <= 0);
    }),
    initialMetadataFrames: existing.size, initialMissingFrames: indices.filter((i) => !existing.has(i)).length,
    sourceFrameSetActions: parsed.frameSet?.actions ?? parsed.frameSet, indices });
  const previous = previousReport?.libraries.find((lib) => lib.library === library && lib.sourceSha256 === sha256(source));
  if (previous) Object.assign(report.libraries.at(-1), { initialMetadataFrames: previous.initialMetadataFrames, initialMissingFrames: previous.initialMissingFrames, exportCompleted: previous.exportCompleted });
}
report.capacity = {
  sourceBytes: report.libraries.reduce((n, lib) => n + lib.sourceBytes, 0),
  drawableFrames: report.libraries.reduce((n, lib) => n + lib.drawableFrames, 0),
  initialMissingFrames: report.libraries.reduce((n, lib) => n + lib.initialMissingFrames, 0),
  note: "Source compressed bytes do not predict PNG size. PNG plus filesystem allocation and metadata must be allowed separately. Export uses compression level 6 and sequential libraries.",
};
await mkdir(path.dirname(reportPath), { recursive: true });
const save = async () => writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
await save();
console.log(JSON.stringify({ phase: "plan", libraries: required.length, unknown: report.unknown, ...report.capacity }));
if (args.includes("--export")) {
  for (const lib of report.libraries) {
    const result = spawnSync(process.execPath, [path.join(import.meta.dirname, "export-crystal-ui.mjs"), "--dataDir", dataDir, "--outputDir", outputDir,
      "--libraries", lib.library, "--fullLibraries", lib.library, "--skipExisting", "true", "--concurrency", "16"],
    { stdio: "inherit", env: { ...process.env, MIR2_CRYSTAL_UI_PNG_DEFLATE_LEVEL: "6" } });
    if (result.status !== 0) throw new Error(`Export failed: ${lib.library} (${result.status})`);
    lib.exportCompleted = true;
    await save();
  }
}
if (args.includes("--export") || args.includes("--verify")) {
  const summary = await json(path.join(outputDir, "manifest.generated.json"));
  for (const lib of report.libraries) {
    const meta = await json(path.join(outputDir, lib.library, "meta.json"));
    const source = parseLibrary(await readFile(path.join(dataDir, `${lib.library}.Lib`)));
    const failures = [];
    if (meta.sourceLibrary?.sha256 !== lib.sourceSha256) failures.push("source hash mismatch");
    if (JSON.stringify(summary.libraries[lib.library]) !== JSON.stringify(meta)) failures.push("global/local manifest mismatch");
    if (JSON.stringify(meta.frameSet) !== JSON.stringify(source.frameSet)) failures.push("source FrameSet mismatch");
    const byIndex = new Map(meta.frames.map((frame) => [frame.index, frame]));
    for (const index of lib.sourceZeroGeometrySlots) {
      if (byIndex.has(index)) failures.push(`zero-geometry source has drawable metadata ${index}`);
      if (existsSync(path.join(outputDir, lib.library, `${index}.png`))) failures.push(`zero-geometry source has fabricated PNG ${index}`);
    }
    let pngBytes = 0;
    const fullyTransparentSourceFrames = [];
    for (const index of lib.indices) {
      const frame = byIndex.get(index);
      if (!frame) { failures.push(`missing metadata ${index}`); continue; }
      const file = path.join(outputDir, lib.library, `${index}.png`);
      if (!existsSync(file)) { failures.push(`missing PNG ${index}`); continue; }
      const bytes = await readFile(file);
      pngBytes += bytes.length;
      if (sha256(bytes) !== frame.pngSha256) failures.push(`PNG hash ${index}`);
      const original = source.frames[index];
      const decoded = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
      if (!decoded.data.some((value, byte) => byte % 4 === 3 && value > 0)) fullyTransparentSourceFrames.push(index);
      if (decoded.info.width !== original.width || decoded.info.height !== original.height || !decoded.data.equals(decodeFrameRgba(source, original))) failures.push(`source PNG pixels/geometry ${index}`);
      for (const key of ["width", "height", "x", "y", "shadowX", "shadowY"]) if (frame[key] !== original[key]) failures.push(`source metadata ${index} ${key}`);
      if (sha256(decoded.data) !== frame.rgbaSha256) failures.push(`RGBA hash ${index}`);
      if (frame.hasMask !== Boolean(original.maskRgba)) failures.push(`source mask declaration ${index}`);
      if (frame.hasMask) {
        const maskFile = path.join(outputDir, lib.library, `${index}.mask.png`);
        if (!existsSync(maskFile)) failures.push(`missing mask ${index}`);
        else {
          const mask = await sharp(maskFile).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
          if (mask.info.width !== original.maskWidth || mask.info.height !== original.maskHeight || !mask.data.equals(decodeMaskFrameRgba(source, original))) failures.push(`source mask pixels/geometry ${index}`);
        }
      }
    }
    lib.verification = { checkedFrames: lib.indices.length, fullyTransparentSourceFrames, allPngPixelsDecodedAndMatchedToSource: failures.length === 0, pngBytes, failures };
    console.log(JSON.stringify({ phase: "verify-library", library: lib.library, ...lib.verification }));
  }
  report.verified = report.unknown.length === 0 && report.libraries.every((lib) => lib.verification.failures.length === 0);
  await save();
  console.log(JSON.stringify({ phase: "verification", verified: report.verified, pngBytes: report.libraries.reduce((n, lib) => n + lib.verification.pngBytes, 0) }));
  if (!report.verified) process.exitCode = 1;
}
