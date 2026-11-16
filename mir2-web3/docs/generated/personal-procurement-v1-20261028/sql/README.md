# Procurement migration 0018: isolated SQL verification

This is an **embedded PostgreSQL WASM SQL/PLpgSQL test**, using the official
`@electric-sql/pglite` package pinned to **0.5.13**. It is not a native PostgreSQL
service, a Rust/libpq integration test, a production migration, or a concurrency,
crash-durability, backup or replication test.

Fresh result after schema-12 quota history protection: **44 passed, 0 failed**, using PostgreSQL **18.6** on
`wasm32-unknown-emscripten`. The exact tested migration has SHA256
`53b63e26318a345c9fc7dc571e6f6a50c9b4534829130a2ea4519446a7f03ac4`.

Official API and engine references:

- https://pglite.dev/docs/about
- https://pglite.dev/docs/api

`local-postgres-check.json` records the bounded read-only machine check. No
PostgreSQL Windows service, PATH binary (`psql`, `postgres`, `pg_ctl`), or standard
Program Files PostgreSQL installation directory was found. This is not an
exhaustive search for arbitrary installations. No database URL was inspected or
connected to, and no Docker command was used.

`test-migration.mjs` creates only a minimal temporary
`accounts(account_id TEXT PRIMARY KEY, raw_json JSONB NOT NULL)` table in an
in-memory PGlite database. It executes the unmodified migration from the actual
checkout, and records its SHA256 and a byte-for-byte tested copy. The harness does
not claim to validate the entire account JSON schema or other game migrations.

The cases cover migration reapplication; plain/legacy-null account writes and
deletion; blocked deletion/null/omission of procurement authority; retention of
all ten protected history collections (including quota quotes and one-time grant
receipts); rejection of a schema-11 writer omitting the new quota authority; allowed state progression and history
append; trigger rejection rolling back an earlier account debit and history
append; application-error rollback; and protection immediately after creation of
an initial ledger.

`results.json` is the authoritative result and engine-version record.
`sql-test-schema12.log` contains the current execution output; `sql-test.log` retains the earlier 37-case run. The database is closed by the
harness and no daemon or listener is started.

The exact official tarball has npm integrity
`sha512-4rZuxK1k07w6sSpJzjMri0B9+HuXuC7V9pxVB51pR0T9KPqXg6TD0mdaEwBA9rkcL4jdht4tGDf/wYzAiGhT7g==`.
`download-package.mjs` and `download-integrity.json` document its bounded,
resumable range download after ordinary single-stream installation stalled.
Installation uses `--ignore-scripts --no-audit --no-fund`.

To repeat in this directory after dependency installation:

```powershell
node test-migration.mjs 'C:/path/to/mir2-web3'
```

Each invocation creates a fresh in-memory database and reruns all assertions.
