#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
report_path="${1:-${repo_root}/docs/generated/player-qa/platinum-176/latest-combat-milestones.json}"

case "${report_path}" in
  /*) ;;
  *) report_path="${repo_root}/${report_path}" ;;
esac

cd "${repo_root}"

MIR2_PLATINUM_COMBAT_REPORT="${report_path}" \
  cargo +1.89.0 test \
  -p mir2-simulation \
  --test platinum_176_combat_milestones \
  -- \
  --nocapture

node - "${report_path}" <<'NODE'
const fs = require("node:fs");

const reportPath = process.argv[2];
const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
const failedAssertions = Object.entries(report.assertions)
  .filter(([, passed]) => passed !== true)
  .map(([name]) => name);

if (report.cases.length !== 15 || failedAssertions.length > 0) {
  throw new Error(
    `combat certificate failed: cases=${report.cases.length}, failedAssertions=${failedAssertions.join(",")}`,
  );
}

const directCases = report.cases.filter((entry) => entry.damageModel === "direct");
const dotCases = report.cases.filter(
  (entry) => entry.damageModel === "damage-over-time-first-observed-pulse",
);
console.log(
  JSON.stringify(
    {
      ok: true,
      reportPath,
      profileVersion: report.profileVersion,
      cases: report.cases.length,
      directCases: directCases.length,
      damageOverTimeCases: dotCases.length,
    },
    null,
    2,
  ),
);
NODE
