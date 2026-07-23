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
- Dubhe Node telemetry and admission console
- Anti-cheat and risk
- Mail and GM tools
- Approvals
- Operators
- Audit log
- Timeline

The GM tools page is connected to the Rust Admin API through server actions. The
server side forwards an operator bearer token from the `admin_operator_token`
cookie or `ADMIN_OPERATOR_TOKEN`, and the shell resolves the active identity via
`GET /admin/auth/me`. `SendSystemMail` submits to the Rust API, redirects with
`commandId`, and reloads command status plus the matching mail outbox receipt.
With `ADMIN_GATEWAY_MAIL_URL` configured, the command reaches the running
gateway and the player Mail panel can display, claim, and delete the delivered
mail. The GM tools page also posts grant item, grant gold, kick player, and ban
account commands directly to the Rust Admin API through server actions.
Approvals, Audit, and Timeline read from the Rust API and ClickHouse-backed event
projection when available. Approvals now hide self-approval actions for the
requesting operator and expect a peer approver for high-risk commands.
Dashboard, Players, Player Detail, Economy, Servers, Activities, and Risk now
read Rust `/admin/read/*` endpoints. Those endpoints derive
account/player/economy/risk data from the configured JSON account store or
explicit Postgres account-store source. Gateway online presence is read from
`GET /admin/sessions` and overlaid onto Dashboard, Players, Player Detail, and
Servers. Activities, Economy price feeds, and Risk trade graph now write and
read real Postgres projection tables through Rust Admin API routes. Dashboard
also reads `/admin/gameplay-events/summary` so operators can see gameplay
command volume, lag, last event time, readiness alerts, and top command kinds
from the ClickHouse-backed event projection. Servers zone runtime and
Operators/RBAC also read and write real Postgres records.

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
ADMIN_OPERATOR_AUTH_BACKEND=postgres \
cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Seed at least one operator with a token before enabling
`ADMIN_OPERATOR_AUTH_BACKEND=postgres`. The Operators page can rotate or create
tokens after an authenticated operator with `permission_manage` exists.

Start the admin web:

```bash
ADMIN_API_BASE_URL=http://127.0.0.1:7420 \
ADMIN_OPERATOR_TOKEN=r254-lead-token \
./node_modules/.bin/next dev -p 3020
```

Open `http://127.0.0.1:3020/login` to switch tokens. The cookie takes precedence
over `ADMIN_OPERATOR_TOKEN`, so multiple local operators can test the
requester/approver split from the same browser session by logging out and back
in with another token.

For shared staging rollout, use `docs/ADMIN-STAGING-RUNBOOK.md` and
`infra/staging.env.example`.

For development:

```bash
npm install
npm run dev
```

### Dubhe Node console

Open `http://127.0.0.1:3020/dubhe-nodes`. The console combines two deliberately
separate data classes:

- live, signed Zone Host telemetry from `/healthz` and `/v1/heartbeat`;
- public Sui testnet registration and committed Gate 13 acceptance evidence.

By default it probes ports `19100`, `19101`, and `29100`. Override the endpoints
and operations links when running another topology:

```bash
DUBHE_NODE_OPERATOR_URLS=http://127.0.0.1:29100 \
DUBHE_NODE_GRAFANA_URL=http://127.0.0.1:13000 \
DUBHE_NODE_PROMETHEUS_URL=http://127.0.0.1:19090 \
npm run dev
```

The page cryptographically verifies Ed25519-ZIP215 heartbeat signatures and
checks that the live identity matches the active Sui registration. Key rotation
and revocation remain intentionally read-only in the web UI; they require the
owner capability and the audited CLI lifecycle.

The UI-local public snapshot exists because the production Next.js build does
not import JSON from outside the Admin Web root. Verify that it still matches
the authoritative deployment and Gate 13 files after regenerating evidence:

```bash
npm run check:dubhe-node-snapshot
```

## Verification

```bash
./node_modules/.bin/tsc --noEmit
./node_modules/.bin/next build
npm run check:dubhe-node-snapshot
curl -sS http://127.0.0.1:7420/health
curl -sS -X POST http://127.0.0.1:3020/api/admin/system-mail \
  -H 'content-type: application/json' \
  --data '{"targetKind":"character","targetId":"Scout","subject":"Smoke","body":"Queued through admin-web proxy.","reason":"local next route integration smoke","attachments":[{"itemId":"gold","count":100}]}'
```

Latest smoke screenshots:

- `docs/admin-web-dashboard-smoke.png`
- `docs/admin-web-gm-tools-smoke.png`
- `output/playwright/admin-dashboard-gameplay-events.png`
- `output/playwright/admin-dashboard-gameplay-readiness-degraded.png`

## Production Gaps

- Replace local bearer-token operators with production IdP/session auth.
- Extend approval policy from one peer approval to production quorum/workflow
  rules.
- Extend real command executors beyond mail/grant/kick/ban.
