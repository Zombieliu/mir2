import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(import.meta.dirname, "../../..");
const SOURCE = "packages/game-data/data/r16-gameplay-baseline.json";
const NODE_OUTPUT = "apps/ai-player/src/generated-gameplay-baseline.mjs";
const WEB_OUTPUT = "apps/web/lib/generated/r16-gameplay-baseline.json";
const KEYS = ["schema", "id", "contentProfile", "questGuidance", "questCadence", "onboarding"];
const fail = reason => { throw Error(`R16 gameplay baseline: ${reason}`); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** This contract is intentionally R16-specific. A new game/version requires an
 * explicit reviewed baseline change, never an inherited environment override. */
export function validateR16GameplayBaseline(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || !same(Object.keys(value).sort(), [...KEYS].sort())) fail("exact public fields are required");
  if (value.schema !== 1 || value.id !== "r16-platinum-fiveclass-newcomer-v2"
    || value.contentProfile !== "platinum_fiveclass" || value.questGuidance !== "newcomer-v2"
    || value.questCadence !== "newcomer-v2" || value.onboarding !== "native") fail("canonical Windows R16 gameplay contract drifted");
  return Object.fromEntries(KEYS.map(key => [key, value[key]]));
}

export function projectR16GameplayBaseline(value) {
  const baseline = validateR16GameplayBaseline(value), json = JSON.stringify(baseline, null, 2);
  return {
    nodeModule: `// Generated from ${SOURCE}; run packages/tooling/scripts/generate-r16-gameplay-baseline.mjs.\nexport const R16_GAMEPLAY_BASELINE = Object.freeze(${json});\n`,
    webJson: `${json}\n`,
  };
}

export function assertR16GameplayBaselineProjections(value, actual) {
  const expected = projectR16GameplayBaseline(value);
  if (!actual || actual.nodeModule !== expected.nodeModule || actual.webJson !== expected.webJson)
    fail("generated projections are stale or modified");
  return expected;
}

/** Fixed relative inputs only. Importing the generator has no write effects. */
export function buildR16GameplayBaseline({ readSource = relative => readFileSync(resolve(ROOT, relative), "utf8") } = {}) {
  const baseline = validateR16GameplayBaseline(JSON.parse(readSource(SOURCE)));
  for (const [file, field] of [["newcomer-v2.json", "questGuidance"], ["newcomer-journey-v2.json", "questCadence"]]) {
    const document = JSON.parse(readSource(`config/quest-guidance/${file}`));
    if (document.schema !== 1 || document.profile !== baseline[field]) fail(`Native ${field} configuration drifted`);
  }
  return { baseline, ...projectR16GameplayBaseline(baseline) };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length > 3 || process.argv.slice(2).some(arg => arg !== "--check")) fail("only --check is supported");
  const generated = buildR16GameplayBaseline(), checking = process.argv.includes("--check");
  if (checking) assertR16GameplayBaselineProjections(generated.baseline, {
    nodeModule: readFileSync(resolve(ROOT, NODE_OUTPUT), "utf8"), webJson: readFileSync(resolve(ROOT, WEB_OUTPUT), "utf8"),
  });
  else for (const [relative, contents] of [[NODE_OUTPUT, generated.nodeModule], [WEB_OUTPUT, generated.webJson]]) {
    mkdirSync(dirname(resolve(ROOT, relative)), { recursive: true }); writeFileSync(resolve(ROOT, relative), contents, "utf8");
  }
  console.log(JSON.stringify({ checked: checking, baseline: generated.baseline, projections: [NODE_OUTPUT, WEB_OUTPUT] }));
}
