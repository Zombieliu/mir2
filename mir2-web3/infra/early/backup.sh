#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/../.." && pwd)"
compose_file="${script_dir}/docker-compose.yml"
backup_dir="${MIR2_EARLY_BACKUP_DIR:-${repo_root}/.mir2-data/early-backups}"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
output="${backup_dir}/mir2-${timestamp}.dump"
temporary="${output}.partial"

mkdir -p "${backup_dir}"
trap 'rm -f "${temporary}"' EXIT
docker compose -f "${compose_file}" exec -T postgres \
  pg_dump --username=mir2 --dbname=mir2 --format=custom --no-owner >"${temporary}"
test -s "${temporary}"
mv "${temporary}" "${output}"
trap - EXIT

echo "early PostgreSQL backup written to ${output}"
echo "copy this file off-host; a backup kept only on the game server is not accepted"
