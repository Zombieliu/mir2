#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
profile="${script_dir}/profile.json"

for command in docker jq; do
  if ! command -v "${command}" >/dev/null 2>&1; then
    echo "missing required command: ${command}" >&2
    exit 1
  fi
done

docker_info="$(docker info --format '{{json .}}')"
available_cpu="$(jq -r '.NCPU' <<<"${docker_info}")"
available_memory_bytes="$(jq -r '.MemTotal' <<<"${docker_info}")"
required_cpu="$(jq -r '.host.minimumCpu' "${profile}")"
required_memory_bytes="$(
  jq -r '.host.minimumUsableMemoryGiB * 1073741824 | ceil' "${profile}"
)"

if (( available_cpu < required_cpu )); then
  echo "early profile requires ${required_cpu} CPUs, Docker exposes ${available_cpu}" >&2
  exit 1
fi
if (( available_memory_bytes < required_memory_bytes )); then
  echo "early profile requires ${required_memory_bytes} usable bytes, Docker exposes ${available_memory_bytes}" >&2
  exit 1
fi

echo "early profile preflight passed: ${available_cpu} CPU / ${available_memory_bytes} bytes"
