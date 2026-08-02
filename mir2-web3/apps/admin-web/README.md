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
- Identity security (sessions, credentials, recovery and security audit)

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
MIR2_GATEWAY_ADMIN_URL=http://127.0.0.1:7110 \
MIR2_GATEWAY_ADMIN_OPERATOR_TOKEN=replace-with-random-32-byte-token \
./node_modules/.bin/next dev -p 3020
```

Open `http://127.0.0.1:3020/login` to switch tokens. The cookie takes precedence
over `ADMIN_OPERATOR_TOKEN`, so multiple local operators can test the
requester/approver split from the same browser session by logging out and back
in with another token.

`/identity-security` is server-rendered and keeps the Gateway operator token
out of the browser. Operators can search one account, inspect redacted
credentials and active/revoked sessions, review the security audit trail, and
revoke one or every session. Production Gateway startup rejects an operator
token shorter than 32 characters.

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

The current v3 heartbeat also signs the node's active Zone workload list. The
console expands the aggregate `Zones` capacity bar into Zone id, map scope,
explicit map-file membership, and live session count. A `single` topology is
shown as `All game maps`; configured groups show every map in the group; and
dynamic `map:<file>` Zones expose their derived map file. Older nodes that only
publish aggregate counts remain visible, but their map details are marked
unverified instead of being inferred by the UI. Aggregate-only v2 heartbeats
remain accepted during rolling upgrades.

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

### Remote Home Node telemetry

The production console is available at
`https://telemetry.obelisk.build/dubhe-nodes`. Vercel runs the Next.js
application, while the Cloudflare `mir2-telemetry-domain-proxy` Worker provides
the custom domain, TLS edge, no-store policy, and response security headers.
The application also requires `ADMIN_DASHBOARD_TOKEN`; the same server-side
guard protects both pages and `/api/dubhe-nodes`, including direct Vercel URLs.

Production reads the UCloud collector through its authenticated operator API:

```dotenv
DUBHE_HOME_TELEMETRY_URL=https://relay-hk.obelisk.build/home/telemetry
DUBHE_HOME_RELAY_URL=https://relay-hk.obelisk.build
DUBHE_HOME_TELEMETRY_OPERATOR_TOKEN=<collector read token>
ADMIN_DASHBOARD_TOKEN=<independent dashboard login token>
ADMIN_API_BASE_URL=https://relay-hk.example.com/home/admin
ADMIN_OPERATOR_TOKEN=<server-side Admin API operator token>
ADMIN_API_PROXY_TOKEN=<TLS reverse-proxy token>
ADMIN_API_TIMEOUT_MS=8000
```

Hosted deployments deliberately keep `ADMIN_DASHBOARD_TOKEN` separate from
`ADMIN_OPERATOR_TOKEN`: the browser cookie authenticates the dashboard, while
the server-only operator and proxy tokens authenticate Vercel-to-Admin-API
requests. The reverse proxy must reject `/home/admin/*` when
`X-Dubhe-Admin-Proxy-Token` does not match.

`GET /v1/operator` returns every signed production admission, its assigned
Zone, certified capacity, and the latest verified Home Node report. This keeps
an admitted but offline node visible. The console deliberately distinguishes
assigned workload from current activity: `primary` means all game maps even
when no player is online; `map:<file>:line:<n>` identifies one explicit map
line; Sessions and Active Zones remain zero while the node is idle.

### Global network telemetry

`/network` is the privacy-preserving global operations view. It refreshes every
five seconds and aggregates the authenticated Home Node fleet into regional
centroids, capacity, Sessions, active Zones, maps, Relay RTT, packet loss, and
Commonware finalized placement. Selecting a region opens
`/network/region/<region-code>` with its node and workload detail; a selected
node can continue into `/service-trace`.

The browser only calls `/api/network`. Raw home IPs and advertised endpoints
never enter the response model. A node-reported coarse region is preferred; if
the desktop agent has not supplied one, the UI can display the official Relay
region as an explicitly labelled fallback rather than pretending it is the
node's physical location.

### Player service trace

`/service-trace` is the authenticated operations view for answering “which
node is serving this player?”. Search accepts an account, character name,
`account:index` player id, or live object id. The page joins the
account/character read model, Gateway Session cache and retained transition
history, the real Gateway's embedded Gate15 finalized lease, Relay and
service-node identity, and Dubhe node telemetry down to Zone, map and line.

The browser only calls `/api/service-trace`; operator and telemetry tokens stay
server-side. Account ids and private endpoints are redacted by default.
Revealing protected endpoints requires `server_control`, and every query is
written to the Admin audit store.

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
npm run check:dubhe-network
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
