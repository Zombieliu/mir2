#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"

profile_label="${GATE16_PROFILE_LABEL:-2c2g}"
cpu_cores="${GATE16_PROFILE_CPU_CORES:-2}"
memory_bytes="${GATE16_PROFILE_MEMORY_BYTES:-2147483648}"
history_steps="${MIR2_GATE16_HISTORY_STEPS:-700,10000,100000}"
output_dir="${repo_root}/docs/generated/gate16"
output_file="${output_dir}/v4-checkpoint-baseline.json"
image="mir2-gate16-v4-baseline:${profile_label}"
container_name="mir2-gate16-v4-baseline-${profile_label}"

mkdir -p "${output_dir}"

docker build \
  --target gate16-v4-baseline \
  --tag "${image}" \
  "${repo_root}"

docker run --rm \
  --name "${container_name}" \
  --cpus "${cpu_cores}" \
  --memory "${memory_bytes}" \
  --memory-swap "${memory_bytes}" \
  --pids-limit 512 \
  --network none \
  --read-only \
  --tmpfs /tmp:size=128m,mode=1777 \
  --user "$(id -u):$(id -g)" \
  --volume "${output_dir}:/evidence" \
  --env GATE16_PROFILE_LABEL="${profile_label}" \
  --env GATE16_PROFILE_CPU_CORES="${cpu_cores}" \
  --env GATE16_PROFILE_MEMORY_BYTES="${memory_bytes}" \
  --env MIR2_GATE16_HISTORY_STEPS="${history_steps}" \
  --env MIR2_GATE16_BASELINE_OUT=/evidence/v4-checkpoint-baseline.json \
  "${image}"

jq -e \
  --arg profile_label "${profile_label}" \
  --arg cpu_cores "${cpu_cores}" \
  --arg memory_bytes "${memory_bytes}" \
  '.schemaVersion == 1
    and .build == "release"
    and .checkpointVersion == 4
    and .environment.profileLabel == $profile_label
    and .environment.requestedCpuCores == $cpu_cores
    and .environment.requestedMemoryBytes == $memory_bytes
    and (.results | length) > 0
    and all(.results[];
      .success == true
      and .completedCommands == .requestedCommands
      and .checkpointEntries == .requestedCommands
      and .checkpointBytes > 0
      and .replicationHead.version == 5
      and .replicationHead.mutationCoverage == "commandJournal"
      and .replicationHead.promotionReady == false
      and .replicationHead.baseSnapshotId == null
      and .replicationHead.entryCount == .requestedCommands
      and .replicationHead.nextSequence == .requestedCommands
      and .replicationHeadBytes < 1024
      and .replicationHeadLatencyUs.count == 100
      and .replicationHeadLatencyUs.p95 < 100000
      and .commandLatencyMs.p95 != null
      and .activeTelemetry.checkpoint.exportsTotal == 1
      and .standbyTelemetry.checkpoint.installsTotal == 1
      and .standbyTelemetry.checkpoint.replayLastEntries == .checkpointEntries)' \
  "${output_file}" >/dev/null

echo "Gate16 v4 baseline written to ${output_file}"
