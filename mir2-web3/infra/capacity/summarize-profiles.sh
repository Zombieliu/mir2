#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
output="${repo_root}/docs/generated/capacity/matrix.json"

shopt -s nullglob
evidence_files=("${repo_root}"/docs/generated/capacity/*/latest.json)
if (( ${#evidence_files[@]} == 0 )); then
  echo "no capacity evidence found under docs/generated/capacity" >&2
  exit 1
fi

tmp_output="${output}.tmp"
jq -s '
  {
    schemaVersion: 1,
    generatedAtUnixMs: ([.[].generatedAtUnixMs] | max),
    profileCount: length,
    profiles: map(
      . as $report
      | $report.recommendation.maxTestedCombinedPlayers as $dense_players
      | $report.recommendation.maxTestedCombinedZones as $zones
      | $report.recommendation.maxTestedCombinedPlayersPerZone as $players_per_zone
      | {
          profile: $report.hardware.label,
          hardware: $report.hardware,
          safeNetworkBudgetMbps: $report.recommendation.safeNetworkBudgetMbps,
          computeOnlyMaxTestedPlayers: $report.recommendation.maxTestedComputePlayers,
          denseZone: (
            [
              $report.singleZone[]
              | select(.players == $dense_players)
              | {
                  players,
                  p95Ms,
                  modeledEgressMbps,
                  rssAfterBytes
                }
            ] | first
          ),
          distributedZones: (
            [
              $report.multiZone[]
              | select(.zones == $zones and .playersPerZone == $players_per_zone)
              | {
                  zones,
                  playersPerZone,
                  totalPlayers,
                  parallelP95Ms,
                  modeledEgressMbps
                }
            ] | first
          ),
          distributedResultAtTestedEdge: (
            $report.recommendation.maxTestedCombinedTotalPlayers
            == ([$report.multiZone[].totalPlayers] | max)
          ),
          status: $report.recommendation.status,
          evidencePath: (
            "docs/generated/capacity/"
            + $report.hardware.label
            + "/latest.json"
          )
        }
    )
  }
' "${evidence_files[@]}" > "${tmp_output}"
mv "${tmp_output}" "${output}"

jq -e '
  .schemaVersion == 1
  and .profileCount == (.profiles | length)
  and .profileCount > 0
  and all(.profiles[];
    .denseZone != null
    and .distributedZones != null
    and .status == "benchmark-only-not-production-certified"
  )
' "${output}" >/dev/null

echo "Capacity matrix written to ${output}"
