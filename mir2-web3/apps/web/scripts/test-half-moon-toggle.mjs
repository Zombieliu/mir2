import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const names = ["castSkill", "skillMatchesCrystalSpell", "normalizeCrystalToken", "stringOrFallback", "stringifyId"];
const declarations = new Map();
let toggleCase;
let projection;
function visit(node) {
  if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) declarations.set(node.name.text, node.getText(ast));
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression) && node.expression.text === "SpellToggle") toggleCase = node.getText(ast);
  if (ts.isVariableDeclaration(node) && node.name.getText(ast) === "knownSkills" && node.initializer?.getText(ast).startsWith("(snapshot.knownSkills")) projection = node.getText(ast);
  ts.forEachChild(node, visit);
}
visit(ast);
assert.equal(declarations.size, names.length);
assert.ok(toggleCase);
assert.ok(projection);
const javascript = ts.transpileModule(`${[...declarations.values()].join("\n")}\nfunction receive(payload) { switch ("SpellToggle") { ${toggleCase} } }\nfunction project(snapshot) { const ${projection}; return knownSkills; }\nreturn { castSkill, receive, project };`, {
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;

function fixture() {
  const sent = [];
  const worldRef = { current: { playerObjectId: "3", knownSkills: [{ key: "warrior-cleave", name: "半月弯刀", spell: "HalfMoon", castKind: "toggle", canUse: false }] } };
  const scope = { worldRef, combatIngressRef: { current: null }, get world() { return worldRef.current; }, send: command => sent.push(command),
    appendLog: () => assert.fail("HalfMoon must remain a usable toggle"), updateWorld: update => { worldRef.current = update(worldRef.current); } };
  const api = new Function("scope", `with(scope) { ${javascript} }`)(scope);
  return { ...api, sent, get skill() { return worldRef.current.knownSkills[0]; } };
}

test("actual page supports HalfMoon enable, disable and re-enable without becoming passive", () => {
  const f = fixture();
  f.castSkill(f.skill.key);
  assert.deepEqual(f.sent.at(-1), { type: "spellToggle", spell: "HalfMoon", toggleState: 1 });
  f.receive({ objectId: 3, spell: "HalfMoon", canUse: true });
  assert.equal(f.skill.canUse, true);
  f.castSkill(f.skill.key);
  assert.equal(f.sent.at(-1).toggleState, 0);
  f.receive({ objectId: 0, spell: "HalfMoon", canUse: false });
  assert.equal(f.skill.castKind, "toggle");
  assert.equal(f.skill.canUse, false);
  f.castSkill(f.skill.key);
  assert.equal(f.sent.at(-1).toggleState, 1);
  f.receive({ objectId: 3, spell: "HalfMoon", canUse: true });
  assert.equal(f.skill.castKind, "toggle");
  assert.equal(f.skill.canUse, true);
});

test("actual page rejects foreign, malformed and unrelated toggle acknowledgements", () => {
  const f = fixture(), original = f.skill;
  for (const payload of [{ objectId: 4, spell: "HalfMoon", canUse: true }, { objectId: "3", spell: "HalfMoon", canUse: true }, { spell: "HalfMoon", canUse: true }, { objectId: 3, spell: "HalfMoon" }, { objectId: 3, spell: "HalfMoon", canUse: 1 }, { objectId: 3, spell: 4, canUse: true }, { objectId: 3, spell: "Thrusting", canUse: true }]) {
    f.receive(payload);
    assert.equal(f.skill, original);
  }
});

test("actual snapshot projection preserves the authoritative toggle state separately from skill kind", () => {
  const f = fixture();
  for (const canUse of [false, true]) {
    const [skill] = f.project({ knownSkills: [{ ...f.skill, canUse, cooldownRemainingTicks: 0 }] });
    assert.equal(skill.canUse, canUse);
    assert.equal(skill.castKind, "toggle");
  }
});

test("passive Slaying acknowledgements never turn the skill into an active toggle", () => {
  const f = fixture();
  Object.assign(f.skill, { key: "slaying", name: "攻杀剑术", spell: "Slaying", castKind: "passive" });
  for (const canUse of [true, false]) {
    f.receive({ objectId: 3, spell: "Slaying", canUse });
    assert.equal(f.skill.castKind, "passive");
    assert.equal(f.skill.canUse, canUse);
  }
});
