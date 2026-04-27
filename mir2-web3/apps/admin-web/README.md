# apps/admin-web

NextJS operations dashboard for the Mir2 Web3 MMORPG backend.

## Current Scope

Implemented desktop-first pages:

- Dashboard
- Player management
- Player detail
- Economy analysis
- Activity configuration
- World/server monitor
- Anti-cheat and risk
- Mail and GM tools
- Approvals
- Audit log
- Timeline

The GM tools page is connected to the Rust Admin API through server actions that
add local operator headers server-side. `SendSystemMail` submits to the Rust API,
redirects with `commandId`, and reloads command status plus the matching mail
outbox receipt. With `ADMIN_GATEWAY_MAIL_URL` configured, the command reaches the
running gateway and the player Mail panel can display, claim, and delete the
delivered mail. The GM tools page also posts grant item, grant gold, kick player,
and ban account commands directly to the Rust Admin API through server actions.
Approvals, Audit, and Timeline read from the Rust API and ClickHouse-backed event
projection when available. Dashboard, Players, Player Detail, Economy, Servers,
Activities, and Risk now read Rust `/admin/read/*` endpoints. Those endpoints
derive account/player/economy/risk data from the configured JSON account store
or explicit Postgres account-store source. Gateway online presence is read from
`GET /admin/sessions` and overlaid onto Dashboard, Players, Player Detail, and
Servers. Activities, Economy price feeds, and Risk trade graph now write and
read real Postgres projection tables through Rust Admin API routes. Deeper zone
telemetry still shows honest empty/unwired state instead of mock numbers.

## Local Run

Start the Rust API:

```bash
ADMIN_API_ADDR=127.0.0.1:7420 \
ADMIN_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_ACCOUNT_STORE_BACKEND=postgres \
MIR2_ACCOUNT_STORE_DATABASE_URL=postgres://mir2:mir2_dev_password@127.0.0.1:5432/mir2 \
MIR2_GATEWAY_REDIS_CACHE_URL=redis://127.0.0.1:6379 \
NATS_ADDR=127.0.0.1:4222 \
ADMIN_OUTBOX_NATS_MODE=jetstream \
ADMIN_OUTBOX_NATS_STREAM=MIR2_ADMIN \
ADMIN_OUTBOX_REDPANDA_URL=http://127.0.0.1:8082 \
cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Start the admin web:

```bash
ADMIN_API_BASE_URL=http://127.0.0.1:7420 \
ADMIN_OPERATOR_TOKEN=local-dev-token \
ADMIN_OPERATOR_ID=local-gm \
ADMIN_OPERATOR_EMAIL=gm.local@mir2.dev \
ADMIN_OPERATOR_ROLE=ops_admin \
ADMIN_OPERATOR_PERMISSIONS=account_read,account_ban,character_read,character_kick,inventory_read,inventory_grant_item,currency_grant,mail_send_system,content_publish,audit_read,approval_manage \
./node_modules/.bin/next dev -p 3020
```

For development:

```bash
npm install
npm run dev
```

## Verification

```bash
./node_modules/.bin/tsc --noEmit
./node_modules/.bin/next build
curl -sS http://127.0.0.1:7420/health
curl -sS -X POST http://127.0.0.1:3020/api/admin/system-mail \
  -H 'content-type: application/json' \
  --data '{"targetKind":"character","targetId":"Scout","subject":"Smoke","body":"Queued through admin-web proxy.","reason":"local next route integration smoke","attachments":[{"itemId":"gold","count":100}]}'
```

Latest smoke screenshots:

- `docs/admin-web-dashboard-smoke.png`
- `docs/admin-web-gm-tools-smoke.png`

## Production Gaps

- Replace local env operator headers with real operator auth.
- Replace local self-approval smoke mode with real multi-operator approval policy.
- Add deeper zone runtime projections and production-grade operator workflows.
- Extend real command executors beyond mail/grant/kick/ban.
