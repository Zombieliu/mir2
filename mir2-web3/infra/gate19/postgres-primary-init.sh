#!/usr/bin/env bash
set -euo pipefail

psql --set ON_ERROR_STOP=1 --username "${POSTGRES_USER}" --dbname "${POSTGRES_DB}" <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'mir2_replicator') THEN
    CREATE ROLE mir2_replicator WITH REPLICATION LOGIN PASSWORD 'mir2-gate19-replication';
  END IF;
END
$$;
SQL

printf '%s\n' \
  'host replication mir2_replicator 0.0.0.0/0 scram-sha-256' \
  'host all all 0.0.0.0/0 scram-sha-256' \
  >>"${PGDATA}/pg_hba.conf"
