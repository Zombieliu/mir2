import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createHash } from "node:crypto";

const root = resolve(import.meta.dirname, "../../..");
export function assertReviewedSources(files) {
  const baseline = JSON.parse(readFileSync(resolve(root, "packages/tooling/data/native-i18n/source-baseline.json"), "utf8"));
  for (const file of files ?? Object.keys(baseline.sha256)) {
    const actual = createHash("sha256").update(readFileSync(resolve(root, "packages/game-data/data/generated", file))).digest("hex");
    if (actual !== baseline.sha256[file]) throw new Error(`Translation source changed; review affected copy before updating source-baseline.json: ${file}`);
  }
}
