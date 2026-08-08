#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
report_path="${1:-${repo_root}/docs/generated/player-qa/platinum-176/latest-party-boss.json}"

cd "${repo_root}"
MIR2_PLATINUM_PARTY_BOSS_REPORT="${report_path}" \
  cargo +1.89.0 test \
    -p mir2-simulation \
    --test platinum_176_party_boss \
    -- \
    --nocapture

node - "${report_path}" <<'NODE'
const fs = require("node:fs");
const reportPath = process.argv[2];
const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
const passed = Object.values(report.assertions).every((value) => value === true);
console.log(JSON.stringify({
  ok: passed,
  reportPath,
  boss: report.boss?.name,
  soloTtkMs: report.solo?.bossKilled ? report.solo.elapsedMs : null,
  partyTtkMs: report.party?.bossKilled ? report.party.elapsedMs : null,
  partyDamageByClass: report.party?.damageByClass,
}, null, 2));
if (!passed) process.exitCode = 1;
NODE
