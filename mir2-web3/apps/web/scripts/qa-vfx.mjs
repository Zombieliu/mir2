// qa-vfx.mjs — deterministic 验收 of skill/spell ANIMATION assets (③ + ④).
//
// The other harnesses can't settle these: qa-magic-skills needs a cast to fire, but a fresh
// character has no spell book and @GIVESKILL is GM/test-gated (no-op locally) → 0 casts; and
// qa-combat-survival depends on a flaky on-demand monster spawn. So neither reliably answers
// "do skill animations actually render". This probe does — over HTTP, no browser, no game — by
// checking the two asset gates the rendering actually depends on:
//
//   ③ Spell-effect animation (fireball / lightning / heal frames):
//      lib/crystal-magic-effects.ts loads /original-effects/effects.generated.json. If that
//      manifest is empty (or 404), loadEffectAssets returns empty maps → resolveSpellEffect
//      returns null for EVERY spell → the client falls to the procedural CSS fallback
//      (lib/vfx-fallback.ts → FallbackVfxNode). So: manifest empty ⇒ "procedural-only, real
//      Crystal effect frames absent". (The exporter scripts/export-crystal-magic-effects.mjs
//      that would populate it does not exist in the repo.)
//
//   ④ Cast-pose / actor action frames:
//      A magic cast sets attackAnimation:"range" on the caster (app/page.tsx) → the actor
//      renderer plays the "attackRange" pose at frame base +96; a melee attack plays
//      "attackMelee" at +136 (app/components/original-client-scene-rendering.tsx). The web does
//      NOT use Crystal's dedicated MirAction.Spell (frame 296) cast pose. A frame renders only
//      if BOTH its meta.json geometry entry AND its PNG byte are served. Committed actor-lib
//      meta is truncated (maxIdx ~255) while higher PNGs are R2-backfilled — so a frame whose
//      PNG serves 200 but whose meta entry is absent CANNOT render (the renderer has no
//      geometry to place it). This probe reports, per Crystal MirAction, meta-present × byte-served.
//
// Crystal frame bases are from Crystal/Client/MirObjects/Frames.cs (Player FrameSet) — the 1:1
// authority. Usage:  node ./scripts/qa-vfx.mjs [--baseUrl http://127.0.0.1:3026]

import fs from "node:fs/promises";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));
const baseUrl = (args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3026").replace(/\/$/, "");
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(args.output ?? path.join(process.cwd(), "docs", "generated", "vfx-qa", `run-${runId}`));

const EFFECTS_MANIFEST_URL = "/original-effects/effects.generated.json";

// Crystal Player FrameSet action bases (Crystal/Client/MirObjects/Frames.cs:155-199). `webUse`
// flags how the web actor renderer reaches the frame (scene-rendering.tsx): magic cast →
// attackRange(+96); melee → attackMelee(+136). Crystal's real Spell(296) cast pose is NOT used.
const PLAYER_ACTIONS = [
  { action: "Standing", base: 0, webUse: "idle" },
  { action: "Walking", base: 32, webUse: "move" },
  { action: "Running", base: 80, webUse: "move" },
  { action: "AttackRange1", base: 96, webUse: "★ magic cast pose (web maps cast→attackRange +96)" },
  { action: "Attack1", base: 136, webUse: "★ melee attack pose (attackMelee +136)" },
  { action: "Attack2", base: 184, webUse: "melee2" },
  { action: "Attack3", base: 232, webUse: "melee3" },
  { action: "Spell", base: 296, webUse: "Crystal's REAL cast pose — NOT used by web" },
  { action: "Struck", base: 360, webUse: "hit reaction" },
  { action: "Die", base: 384, webUse: "death" },
  { action: "Attack4", base: 416, webUse: "melee4" },
];

// Representative committed actor body libraries (warrior / taoist share CArmour; archer ARArmour).
const ACTOR_LIBS = ["CArmour/00", "AArmour/00", "ARArmour/00"];

async function httpStatus(url) {
  try {
    const res = await fetch(`${baseUrl}${url}`, { method: "GET" });
    // drain so the socket frees; we only need status.
    await res.arrayBuffer().catch(() => undefined);
    return res.status;
  } catch (err) {
    return { error: String(err?.message ?? err) };
  }
}

async function fetchJson(url) {
  const res = await fetch(`${baseUrl}${url}`);
  if (!res.ok) return { status: res.status, json: null };
  return { status: res.status, json: await res.json().catch(() => null) };
}

function served(status) {
  return status === 200 || status === 206;
}

const issues = [];
const scorecard = {
  spellEffectAtlas: { label: "③ Spell-effect animation: real Crystal atlas exported (not procedural-only)", status: "skip", note: "" },
  castPoseRenders: { label: "④ Cast-pose frames resolvable (attackRange +96 / attackMelee +136: meta + bytes)", status: "skip", note: "" },
  realCastPose: { label: "④b Crystal's real cast pose (Spell +296) renderable + used", status: "skip", note: "" },
  actionFrameMeta: { label: "④c Action-frame meta complete (struck/die/attack4 not truncated)", status: "skip", note: "" },
};
function score(key, status, note) {
  scorecard[key].status = status;
  if (note) scorecard[key].note = note;
  const icon = status === "pass" ? "✓" : status === "fail" ? "✗" : status === "gap" ? "▱" : "·";
  console.log(`  ${icon} [${status}] ${scorecard[key].label}${note ? ` — ${note}` : ""}`);
}

async function main() {
  console.log(`qa-vfx: target=${baseUrl}`);
  await fs.mkdir(outputDir, { recursive: true });
  const facts = {};

  // --- ③ spell-effect atlas -------------------------------------------------------------------
  console.log("\n▶ ③ spell-effect animation atlas");
  const manifest = await fetchJson(EFFECTS_MANIFEST_URL);
  const m = manifest.json ?? {};
  const counts = {
    available: (m.available ?? []).length,
    spell_effects: (m.spell_effects ?? []).length,
    ground_effects: (m.ground_effects ?? []).length,
    map_effects: (m.map_effects ?? []).length,
    spell_effect_enum: (m.spell_effect_enum ?? []).length,
  };
  facts.effectsManifest = { status: manifest.status, counts };
  const anyReal = counts.available + counts.spell_effects + counts.ground_effects + counts.map_effects > 0;
  if (manifest.status === 404) {
    score("spellEffectAtlas", "fail", "manifest 404 → resolveSpellEffect null for all → procedural CSS fallback only");
  } else if (!anyReal) {
    score("spellEffectAtlas", "fail", `manifest is an EMPTY stub (available=0, spell_effects=0) → resolveSpellEffect returns null for EVERY spell → 100% procedural CSS fallback (lib/vfx-fallback). No real Crystal effect frame renders. Exporter export-crystal-magic-effects.mjs is missing.`);
    issues.push({ severity: "high", area: "③", summary: "no real spell-effect atlas: effects.generated.json is empty; every spell renders the procedural fallback, never the real fireball/lightning/heal frames" });
  } else {
    score("spellEffectAtlas", "pass", `real atlas present: ${counts.spell_effects} spell effects across ${counts.available} libraries`);
  }

  // --- ④ actor cast-pose / action frames ------------------------------------------------------
  console.log("\n▶ ④ cast-pose / actor action-frame resolution (meta × bytes)");
  const grid = {}; // action -> { lib -> {meta, png} }
  for (const lib of ACTOR_LIBS) {
    const meta = await fetchJson(`/original-ui/${lib}/meta.json`);
    const frames = (meta.json && (meta.json.frames ?? meta.json)) || {};
    for (const act of PLAYER_ACTIONS) {
      const metaPresent = Boolean(frames[String(act.base)]);
      const png = await httpStatus(`/original-ui/${lib}/${act.base}.png`);
      grid[act.action] ??= { base: act.base, webUse: act.webUse, libs: {} };
      grid[act.action].libs[lib] = { meta: metaPresent, png: typeof png === "number" ? png : 0 };
    }
  }
  facts.actionFrameGrid = grid;

  const renderable = (action) => {
    const e = grid[action];
    // renders only if meta present AND bytes served, in the representative warrior lib.
    const w = e?.libs["CArmour/00"];
    return Boolean(w && w.meta && served(w.png));
  };
  const bytesOnly = (action) => {
    const w = grid[action]?.libs["CArmour/00"];
    return Boolean(w && !w.meta && served(w.png));
  };

  // ④ cast/attack poses the web actually uses
  const castPose = renderable("AttackRange1");
  const meleePose = renderable("Attack1");
  if (castPose && meleePose) {
    score("castPoseRenders", "pass", "attackRange(+96) + attackMelee(+136) both have meta + bytes → the cast/attack pose ANIMATES (does not fall back to standing)");
  } else {
    score("castPoseRenders", castPose || meleePose ? "gap" : "fail", `attackRange(+96)=${castPose ? "ok" : grid.AttackRange1?.libs["CArmour/00"]?.meta ? "no-bytes" : "no-meta"}, attackMelee(+136)=${meleePose ? "ok" : "missing"}`);
  }

  // ④b Crystal's real Spell(296) cast pose — web doesn't map to it AND meta is truncated
  const realPose = renderable("Spell");
  score("realCastPose", "gap", realPose
    ? "Spell(296) frames resolve, but the web maps casts to attackRange(+96), so the faithful cast pose is unused"
    : `Crystal's real cast pose Spell(296) is ${bytesOnly("Spell") ? "byte-present but META-TRUNCATED (no geometry)" : "absent"} AND unused by the web (casts reuse attackRange +96) — not faithful`);

  // ④c action-frame meta completeness (the truncation gap)
  const gappedByteOnly = ["Spell", "Struck", "Die", "Attack4"].filter(bytesOnly);
  if (gappedByteOnly.length === 0) {
    score("actionFrameMeta", "pass", "no meta truncation detected on the sampled actions");
  } else {
    score("actionFrameMeta", "gap", `META-TRUNCATED (PNG bytes serve 200 but meta entry absent → can't render): ${gappedByteOnly.map((a) => `${a}(${grid[a].base})`).join(", ")} — actor-sprite-lib meta truncation persists on this branch; struck/death poses can't render`);
    issues.push({ severity: "medium", area: "④", summary: `actor action-frame meta truncated for ${gappedByteOnly.join("/")}: PNG bytes are backfilled (200) but committed meta tops out ~255, so these poses fall back instead of rendering` });
  }

  // --- report ---------------------------------------------------------------------------------
  const pass = Object.values(scorecard).filter((c) => c.status === "pass").length;
  const fail = Object.values(scorecard).filter((c) => c.status === "fail").length;
  const gap = Object.values(scorecard).filter((c) => c.status === "gap").length;

  await fs.writeFile(path.join(outputDir, "report.json"), JSON.stringify({ runId, baseUrl, scorecard, facts, issues }, null, 2));
  const md = [`# Skill-animation 验收 (③ spell VFX + ④ cast pose) — run ${runId}`, "", `- Target: \`${baseUrl}\``, `- Scorecard: **${pass} pass / ${fail} fail / ${gap} gap**`, ""];
  md.push("## ③ Spell-effect atlas");
  md.push("```json", JSON.stringify(facts.effectsManifest, null, 2), "```");
  md.push("## ④ Action-frame resolution (CArmour/00 — warrior body)");
  md.push("| Crystal MirAction | frame | web use | meta | PNG | renders |");
  md.push("|---|---|---|---|---|---|");
  for (const [action, e] of Object.entries(grid)) {
    const w = e.libs["CArmour/00"];
    md.push(`| ${action} | ${e.base} | ${e.webUse} | ${w.meta ? "✓" : "✗"} | ${w.png} | ${w.meta && served(w.png) ? "✓" : "✗"} |`);
  }
  md.push("", "## Scorecard");
  for (const c of Object.values(scorecard)) md.push(`- [${c.status}] ${c.label} — ${c.note}`);
  if (issues.length) {
    md.push("", "## Issues");
    for (const i of issues) md.push(`- **[${i.severity}/${i.area}]** ${i.summary}`);
  }
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));

  console.log(`\nqa-vfx done: ${pass} pass / ${fail} fail / ${gap} gap. Report: ${path.join(outputDir, "report.md")}`);
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (!a.startsWith("--")) continue;
    const key = a.slice(2);
    const next = argv[i + 1];
    if (next === undefined || next.startsWith("--")) out[key] = "true";
    else {
      out[key] = next;
      i += 1;
    }
  }
  return out;
}

main().catch((err) => {
  console.error("qa-vfx fatal:", err);
  process.exitCode = 1;
});
