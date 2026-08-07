#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
output_path="${1:-${repo_root}/docs/generated/player-qa/platinum-176/latest-progression.json}"
acceptance_dir="${2:-${repo_root}/acceptance/platinum-176}"

cd "${repo_root}"
MIR2_PLATINUM_PROGRESSION_REPORT="${output_path}" \
MIR2_PLATINUM_ACCEPTANCE_DIR="${acceptance_dir}" \
  cargo +1.89.0 test \
    -p mir2-simulation \
    --test platinum_176_progression \
    all_three_classes_progress_level_by_level_through_source_backed_hunting_routes \
    -- \
    --exact \
    --nocapture \
    --test-threads=1

node -e '
  const fs = require("node:fs");
  const reportPath = process.argv[1];
  const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
  const assertions = Object.values(report.assertions ?? {});
  if (report.schema !== "mir2-platinum-176-progression/1") {
    throw new Error(`unexpected progression report schema: ${report.schema}`);
  }
  if (report.classes?.length !== 3 || assertions.length === 0 || !assertions.every(Boolean)) {
    throw new Error("progression certification report is incomplete or failed");
  }
  console.log(JSON.stringify({
    ok: true,
    reportPath,
    acceptanceDir: process.argv[2],
    profileId: report.profileId,
    profileVersion: report.profileVersion,
    acceptanceLevel: report.acceptanceLevel,
    classCount: report.classes.length,
    transitionsPerClass: report.classes.map((entry) => entry.levelTransitions.length),
    routeMapCount: report.routeMapCount,
    assertions: report.assertions
  }, null, 2));
' "${output_path}" "${acceptance_dir}"
