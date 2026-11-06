import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import ts from "typescript";
function load(relative) {
  const code = ts.transpileModule(readFileSync(new URL(relative, import.meta.url), "utf8"), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const m = { exports: {} }; new Function("exports", "module", "require", code)(m.exports, m, () => { throw Error("Unexpected import"); }); return m.exports;
}
const hud = load("../lib/bevy-hud-ui.ts");
const world = { playerHp: 0, playerMaxHp: 0, playerMp: 17, playerMaxMp: 23, gold: 4294967295, credit: 0,
  playerExperience: 1, playerMaxExperience: 0, currentWeight: 7, maxWeight: 80,
  playerCrystalStats: [{ stat: 0, value: -3 }, { stat: 16, value: 80 }], playerWeights: { bag: 4294967295, wear: 0, hand: 34 } };
const player = { name: "Authority", level: 26, classKey: "warrior", genderKey: "male" };
test("real server Vec stat and u32 weights survive without estimates, omissions stay missing", () => {
  const projected = hud.projectAuthoritativeHudPlayer(world, player);
  assert.deepEqual(projected.crystalStats, world.playerCrystalStats); assert.equal(projected.weights.bag, 4294967295);
  assert.equal(projected.weights.wear, 0); assert.equal(projected.hp, 0); assert.equal(projected.maxHp, 0); assert.equal(projected.maxExperience, 0);
  const missing = hud.projectAuthoritativeHudPlayer({ ...world, playerCrystalStats: undefined, playerWeights: null }, player);
  assert.equal("crystalStats" in missing, false); assert.equal("weights" in missing, false);
  assert.deepEqual(hud.projectAuthoritativeHudPlayer({ ...world, playerCrystalStats: [] }, player).crystalStats, []);
  assert.equal(hud.projectAuthoritativeHudPlayer({ ...world, playerWeights: { ...world.playerWeights, wear: 4294967296 } }, player), null);
  const guild = hud.projectAuthoritativeHudPlayer({ ...world, stage5Systems: { guild: { name: "AuthorityGuild", rank: "Member" } } }, player);
  assert.equal(guild.guildName, "AuthorityGuild"); assert.equal(guild.guildRankName, "Member"); assert.equal(projected.guildName, undefined);
});
test("confirmed movement ACK only keeps fast path when authoritative stats and all weight fields match", () => {
  assert.equal(hud.sameHudSnapshotFields(world, { ...world, playerHp: 7 }), true);
  assert.equal(hud.sameHudSnapshotFields({ ...world, playerCrystalStats: [{ stat: 0, value: 9 }] }, world), false);
  assert.equal(hud.sameHudSnapshotFields({ ...world, playerWeights: { ...world.playerWeights, wear: 1 } }, world), false);
  assert.equal(hud.sameHudSnapshotFields({ ...world, playerWeights: { ...world.playerWeights, hand: 35 } }, world), false);
  assert.equal(hud.sameHudSnapshotFields({}, world), false);
  assert.deepEqual(hud.projectHudSnapshotFields({}), { playerCrystalStats: undefined, playerWeights: undefined });
  const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  assert.match(source, /const staticStateMatches\s*=\s*sameHudSnapshotFields\(snapshot, currentWorldFast\)/);
  assert.match(source, /\.\.\.projectHudSnapshotFields\(snapshot\)/);
  const file = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const variables = new Map();
  const visit = node => { if (ts.isVariableDeclaration(node) && ["staticStateMatches", "isMovementOnly"].includes(node.name.getText(file))) variables.set(node.name.getText(file), node.initializer.getText(file)); ts.forEachChild(node, visit); }; visit(file);
  const curSelf = { objectId: "1", x: 10, y: 10 }, snapSelf = { ...curSelf, x: 11 };
  const baseline = { ...world, inventoryCapacity: 46, stage5Systems: {}, entities: [curSelf], groundDrops: [], mapFileName: "Bichon" };
  const scope = { currentWorldFast: baseline, curSelf, snapSelf, sameHudSnapshotFields: hud.sameHudSnapshotFields,
    projectedSelfAppearance: curSelf, sameSelfAppearance: (appearance, self) => appearance === self,
    movementSnapshotWeightMatches: () => true, projectedWeight: null, sameProjectedList: () => true, sameAuthoritativeItemIdentities: () => true,
    itemIdentity: null, equipmentIdentity: null, questIdentity: null, skillIdentity: null, buffIdentity: null };
  const code = ts.transpileModule(`const snapshotSystems = snapshot.stage5Systems ?? {}; const staticStateMatches = ${variables.get("staticStateMatches")}; return ${variables.get("isMovementOnly")};`,
    { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const fast = (snapshot, currentWorldFast = baseline) => new Function("snapshot", ...Object.keys(scope), code)(snapshot, ...Object.values({ ...scope, currentWorldFast }));
  assert.equal(fast({ ...baseline, entities: [snapSelf] }), true, "actual confirmed-ACK eligibility retains pure transform fast path");
  const activeNpcDialog = { npcName: "Shopkeeper", text: "Buy goods" };
  assert.equal(fast({ ...baseline, activeNpcDialog, entities: [snapSelf] }), false,
    "new NPC dialog with confirmed movement must fall through to full projection");
  const currentWithDialog = { ...baseline, activeNpcDialog };
  assert.equal(fast({ ...baseline, entities: [snapSelf] }, currentWithDialog), false,
    "current NPC dialog removal must fall through to full projection");
  assert.equal(fast({ ...baseline, activeNpcDialog: { ...activeNpcDialog, text: "Updated goods" }, entities: [snapSelf] }, currentWithDialog), false,
    "current NPC dialog update must fall through to full projection");
  for (const delta of [{ playerCrystalStats: [{ stat: 0, value: 9 }] }, { playerWeights: { ...world.playerWeights, wear: 1 } },
    { playerWeights: { ...world.playerWeights, hand: 99 } }, { playerCrystalStats: undefined, playerWeights: undefined }]) {
    assert.equal(fast({ ...baseline, ...delta, entities: [snapSelf] }), false, "actual ACK path falls through to full projection for stats/weights/omission");
  }
});
test("source geometry Canvas2D paints without anchors and stale async loads cannot retake ownership", async () => {
  const draw = load("../lib/bevy-hud-bar-draw-plan.ts");
  let resolve, live, calls = 0;
  const canvas = { width: 1004, height: 8, style: { visibility: "hidden" }, getContext: () => ({ clearRect() {}, drawImage() { calls++; } }) };
  const slot = { left: 9, top: 759, width: 1004, height: 8 };
  const plan = { token: "e1", sequence: 1, state: "draw", current: 1, maximum: 2, slot, image: "original-ui/Prguse/8.png",
    source: { left: 0, top: 0, width: 500, height: 8 }, destination: { ...slot, width: 500 } };
  live = { lifetime: "r1-g7-p1000", experience: plan, weight: null };
  const controller = new draw.HudBarCanvasController(() => {}, () => new Promise(done => { resolve = done; }));
  const input = { lifetime: live.lifetime, presentation: { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: true },
    experience: { canvas, anchor: null, sourceGeometry: true, current: 1, maximum: 2, slot, plan },
    weight: { canvas: null, anchor: null, sourceGeometry: true, current: null, maximum: null, slot: null, plan: null }, readLivePlans: () => live };
  controller.update(input); assert.equal(canvas.style.visibility, "hidden");
  resolve({}); await new Promise(done => setImmediate(done)); assert.equal(canvas.style.visibility, "visible"); assert.equal(calls, 1);
  live = { ...live, lifetime: "r1-g8-p2000" }; controller.update(input); assert.equal(canvas.style.visibility, "hidden");
  controller.dispose();
});
