#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
compose_file="${script_dir}/docker-compose.yml"

if [[ "${1:-}" != "--confirm" || -z "${2:-}" ]]; then
  echo "usage: $0 --confirm /absolute/path/to/mir2-backup.dump" >&2
  echo "restore replaces the current early-server database" >&2
  exit 2
fi

backup="$2"
if [[ "${backup}" != /* || ! -s "${backup}" ]]; then
  echo "backup must be an existing non-empty absolute path" >&2
  exit 2
fi

docker compose -f "${compose_file}" stop gateway zone
docker compose -f "${compose_file}" exec -T postgres \
  psql --username=mir2 --dbname=postgres --set=ON_ERROR_STOP=1 \
  --command="DROP DATABASE IF EXISTS mir2_restore;" \
  --command="CREATE DATABASE mir2_restore OWNER mir2;"
docker compose -f "${compose_file}" exec -T postgres \
  pg_restore --username=mir2 --dbname=mir2_restore --no-owner --exit-on-error \
  <"${backup}"

echo "backup restored into validation database mir2_restore"
echo "promote it manually after application checks; the live mir2 database was not overwritten"
