# apps/admin-api

Rust Admin API and command/audit control-plane primitives for the Mir2 Web3
operations backend.

## Current Scope

The crate now contains:

- typed admin commands;
- operator permissions;
- command envelopes;
- validation;
- persistent-storage-ready command repository trait;
- persistent-storage-ready audit repository trait;
- Postgres-backed command, audit, and admin outbox repository adapters activated
  by `ADMIN_DATABASE_URL`;
- in-memory command/audit repositories for local tests and smoke runs;
- Postgres schema migration for accounts, characters, character saves, admin
  command records, audit records, and admin outbox records;
- account-store JSON import utility for migrating `.mir2-data/accounts.json`
  into Postgres-shaped tables;
- command idempotency guard through `AdminCommandRepository::insert_pending`;
- `SendSystemMail` domain executor, outbox record, live gateway delivery attempt,
  and account-store fallback;
- grant item, grant currency, kick player, and ban account executors;
- persistent approval records and approval events;
- Axum HTTP routes for health, command records, audit records, approval records,
  per-command status, event/timeline read models, outbox records, and the
  current GM commands;
- optional `ADMIN_OPERATOR_TOKEN` static Bearer validation;
- optional `ADMIN_OPERATOR_POLICY_PATH` policy-file auth that maps Bearer tokens
  to fixed operator identities and permissions.

The current `SendSystemMail` executor is connected to live local gameplay state
when `ADMIN_GATEWAY_MAIL_URL` points at the gateway `POST /admin/system-mail`
endpoint. If gateway delivery is unavailable, it falls back to the configured
account store path. Command/audit repositories are in-memory unless
`ADMIN_DATABASE_URL` is set. With `ADMIN_DATABASE_URL`, the API applies
`infra/postgres/migrations/0001_core.sql` on startup and stores command/audit
records in Postgres. Real auth, approvals, and broader GM executors remain
production gaps have moved to real OIDC/session auth, multi-approver policy, and
additional command executors.

If `MIR2_ACCOUNT_STORE_DATABASE_URL` is also set, fallback account-store writes
mirror the resulting JSON account store into Postgres `accounts`, `characters`,
and `character_saves`. This is a migration bridge; JSON remains the runtime
source of truth until a dedicated Postgres gameplay repository replaces it.

Set `MIR2_ACCOUNT_STORE_BACKEND=postgres` to make the fallback account store load
from and save to Postgres directly. This mode is explicit opt-in and uses
Postgres row locks plus `store_version` / `save_version` increments for source
of truth writes.

## HTTP Routes

Default bind:

```bash
ADMIN_API_ADDR=127.0.0.1:7420 cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Postgres-backed Admin API. Successful commands also append a pending
`admin.command.succeeded` row to `admin_outbox`:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_ACCOUNT_STORE_BACKEND=postgres \
ADMIN_API_ADDR=127.0.0.1:7420 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Import the current JSON account store into Postgres-shaped tables:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin import-account-store -- .mir2-data/accounts.json
```

Dispatch pending admin outbox messages to NATS:

```bash
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
NATS_ADDR=127.0.0.1:4222 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin dispatch-admin-outbox -- --once
```

Routes:

- `GET /health`
- `GET /admin/commands`
- `GET /admin/commands/:command_id/status`
- `GET /admin/audit`
- `GET /admin/approvals`
- `GET /admin/events`
- `GET /admin/timeline`
- `GET /admin/system-mail/outbox`
- `POST /admin/commands/send-system-mail`
- `POST /admin/commands/grant-item`
- `POST /admin/commands/grant-currency`
- `POST /admin/commands/kick-player`
- `POST /admin/commands/ban-account`
- `POST /admin/approvals`
- `POST /admin/approvals/:approval_id/approve`
- `POST /admin/approvals/:approval_id/reject`

Write routes require operator headers:

```text
x-operator-id
x-operator-email
x-operator-role
x-operator-permissions
```

For local GM mail smoke, include `mail_send_system` in
`x-operator-permissions`.

If `ADMIN_OPERATOR_TOKEN` is set, requests must also include
`Authorization: Bearer <token>`. If `ADMIN_OPERATOR_POLICY_PATH` is set, the
Bearer token selects the operator from that JSON policy file and header-supplied
operator identity is ignored. Approval self-approval is forbidden by default;
set `ADMIN_APPROVAL_ALLOW_SELF=true` only for local smoke runs.

## Current Implemented Commands

- `SendSystemMail`: HTTP + RBAC + command repository + audit repository + domain
  outbox.
- `GrantItem`: approval-gated item grant routed through audited system mail.
- `GrantCurrency`: approval-gated gold grant routed through audited system mail.
- `KickPlayer`: session-routing kick through the gateway admin endpoint.
- `BanAccount`: approval-gated account ban persisted in the account store and
  enforced by simulation login/start-game.

## Verification

```bash
cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1
cargo +1.89.0 fmt --check
```

## Next Steps

1. Add OIDC/session middleware and production RBAC policy management.
2. Add multi-operator approval policy and operator audit retention rules.
3. Expand GM executors beyond mail/grant/kick/ban.
4. Add production deployment health checks, rate limits, and dashboards.
