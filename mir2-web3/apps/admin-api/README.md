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
  command records, audit records, admin outbox records, activity config,
  market price feeds, trade graph edges, zone runtime telemetry, operator
  records, and system-mail delivery receipts;
- account-store JSON import utility for migrating `.mir2-data/accounts.json`
  into Postgres-shaped tables;
- command idempotency guard through `AdminCommandRepository::insert_pending`;
- `SendSystemMail` domain executor, outbox record, live gateway delivery attempt,
  and account-store fallback;
- grant item, grant currency, kick player, and ban account executors;
- persistent approval records and approval events;
- Postgres-backed operator token auth selected by
  `ADMIN_OPERATOR_AUTH_BACKEND=postgres`, with `/admin/auth/me`,
  `admin_operators.token_hash`, and last-authenticated timestamps;
- strict high-risk approval matching by approval id, command id, command type,
  requesting operator, and a different deciding operator by default;
- Axum HTTP routes for health, command records, audit records, approval records,
  per-command status, event/timeline read models, outbox records, and the
  current GM commands;
- real admin read-model routes for dashboard, players, player detail, economy,
  service health, activity state, and risk state. These read the configured JSON
  account store or explicit Postgres account-store source, then overlay Gateway
  online presence from `ADMIN_GATEWAY_SESSIONS_URL` /
  `http://127.0.0.1:7110/admin/sessions`. When `ADMIN_DATABASE_URL` is set,
  Activities, Economy price feeds, Risk trade graph, Servers zone runtime, and
  Operators/RBAC also read/write Postgres projection/config tables;
- optional `ADMIN_OPERATOR_TOKEN` static Bearer validation for dev fallback;
- optional `ADMIN_OPERATOR_POLICY_PATH` policy-file auth that maps Bearer tokens
  to fixed operator identities and permissions.

The current `SendSystemMail` executor is connected to live local gameplay state
when `ADMIN_GATEWAY_MAIL_URL` points at the gateway `POST /admin/system-mail`
endpoint. If gateway delivery is unavailable, it falls back to the configured
account store path. When `ADMIN_DATABASE_URL` is set, the resulting
`gateway_live` or `account_store_fallback` receipt is also written to
`admin_system_mail_receipts`, and `GET /admin/system-mail/outbox` merges those
persisted receipts with in-memory receipts for Admin Web status readback.
Command/audit repositories are in-memory unless
`ADMIN_DATABASE_URL` is set. With `ADMIN_DATABASE_URL`, the API applies
`infra/postgres/migrations/0001_core.sql` on startup and stores command, audit,
approval, outbox, projection, and operator records in Postgres. Production gaps
have moved to external IdP/session auth, richer RBAC administration, support
workflows, multi-step approval policy, and additional command executors.

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
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379 \
NATS_ADDR=127.0.0.1:4222 \
ADMIN_OUTBOX_NATS_MODE=jetstream \
ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN \
ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082 \
ADMIN_GATEWAY_SESSIONS_URL=http://127.0.0.1:7110/admin/sessions \
ADMIN_OPERATOR_AUTH_BACKEND=postgres \
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
ADMIN_OUTBOX_NATS_MODE=jetstream \
ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN \
ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin dispatch-admin-outbox -- --once
```

Routes:

- `GET /health`
- `GET /admin/auth/me`
- `GET /admin/commands`
- `GET /admin/commands/:command_id/status`
- `GET /admin/audit`
- `GET /admin/approvals`
- `GET /admin/events`
- `GET /admin/timeline`
- `GET /admin/system-mail/outbox`
- `GET /admin/read/dashboard`
- `GET /admin/read/players`
- `GET /admin/read/players/:player_id`
- `GET /admin/read/service-trace?query=<account|character|player-id|object-id>`
- `GET /admin/read/commonware-network`
- `GET /admin/read/economy`
- `GET /admin/read/activities`
- `GET /admin/read/servers`
- `GET /admin/read/risk`
- `GET /admin/read/operators`
- `POST /admin/activities`
- `POST /admin/economy/price-feeds`
- `POST /admin/risk/trade-edges`
- `POST /admin/servers/zones`
- `POST /admin/operators`
- `POST /admin/commands/send-system-mail`
- `POST /admin/commands/grant-item`
- `POST /admin/commands/grant-currency`
- `POST /admin/commands/kick-player`
- `POST /admin/commands/ban-account`
- `POST /admin/approvals`
- `POST /admin/approvals/:approval_id/approve`
- `POST /admin/approvals/:approval_id/reject`

Operator authentication:

- `ADMIN_OPERATOR_AUTH_BACKEND=postgres` requires
  `Authorization: Bearer <operator-token>`. The token is resolved from
  `admin_operators` and caller-supplied identity headers are ignored.
- `GET /admin/auth/me` returns the resolved operator, role, permissions, and auth
  source. Admin Web uses this for the top-bar identity and login state.
- Without Postgres auth, dev/local fallback can use `ADMIN_OPERATOR_POLICY_PATH`
  or `ADMIN_OPERATOR_TOKEN` plus local operator headers:

```text
x-operator-id
x-operator-email
x-operator-role
x-operator-permissions
```

For local GM mail smoke, include `mail_send_system` in
`x-operator-permissions`. Activity, market price, trade graph, and zone runtime
projection writes require `content_publish`; operator/RBAC writes require
`permission_manage`.

If `ADMIN_OPERATOR_TOKEN` is set, requests must also include
`Authorization: Bearer <token>`. If `ADMIN_OPERATOR_POLICY_PATH` is set, the
Bearer token selects the operator from that JSON policy file and header-supplied
operator identity is ignored. Approval self-approval is forbidden by default,
and command submission requires an approved record for the same command id,
command type, and requesting operator. Set `ADMIN_APPROVAL_ALLOW_SELF=true` only
for local smoke runs.

## Current Implemented Commands

- `SendSystemMail`: HTTP + RBAC + command repository + audit repository + domain
  outbox.
- `GrantItem`: approval-gated item grant routed through audited system mail.
- `GrantCurrency`: approval-gated gold grant routed through audited system mail.
- `KickPlayer`: session-routing kick through the gateway admin endpoint.
- `BanAccount`: approval-gated account ban persisted in the account store and
  enforced by simulation login/start-game.

## Verification

`/admin/read/service-trace` requires `character_read`. It resolves identity
against the account read model, then joins the protected Gateway trace endpoint
with the real Gateway's embedded Gate15 Commonware lease. Results are
endpoint-redacted by default and every read appends a completed
`CharacterRead` audit record. Passing `sensitive=true` additionally requires
`server_control`.

`/admin/read/commonware-network` also requires `character_read`. It exposes the
real Commonware primary placement as a read-only, endpoint-free telemetry
model. A temporary Commonware outage returns `status: unavailable` without
inventing a placement height, allowing Admin Web to label the network as
degraded while the game Session remains online.

Production configuration:

```dotenv
ADMIN_GATEWAY_SERVICE_TRACE_URL=http://mir2-gateway:7110/admin/session-trace
MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=<shared internal gateway operator token>
# Optional fallback when the Gateway response has no embedded Gate15 data:
ADMIN_COMMONWARE_GATEWAY_URL=http://gate14-gateway:9500
```

```bash
cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1
cargo +1.89.0 fmt --check
```

Latest live local acceptance also covered Admin Web login, Operators,
Approvals, peer-approved GM grant, Servers heartbeat, Audit, Timeline, and
persisted system-mail receipt readback through Postgres/Redpanda/ClickHouse.
For shared staging rollout, use `docs/ADMIN-STAGING-RUNBOOK.md` and
`infra/staging.env.example`.

## Next Steps

1. Add external IdP/session middleware and production RBAC policy management.
2. Add multi-step/quorum approval policy and operator audit retention rules.
3. Expand GM executors beyond mail/grant/kick/ban.
4. Add production deployment health checks, rate limits, and dashboards.
