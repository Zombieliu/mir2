#!/usr/bin/env bash
set -euo pipefail

if [[ ! -s "${PGDATA}/PG_VERSION" ]]; then
  install -d -o postgres -g postgres "${PGDATA}"
  find "${PGDATA}" -mindepth 1 -maxdepth 1 -delete
  until PGPASSWORD="${REPLICATION_PASSWORD}" gosu postgres pg_basebackup \
    --host=postgres-primary \
    --port=5432 \
    --username=mir2_replicator \
    --pgdata="${PGDATA}" \
    --wal-method=stream \
    --write-recovery-conf \
    --checkpoint=fast
  do
    sleep 1
  done
fi

exec docker-entrypoint.sh postgres \
  -c hot_standby=on \
  -c max_wal_senders=10 \
  -c max_replication_slots=10
