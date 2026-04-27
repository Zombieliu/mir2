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
- Audit log

The GM tools page is connected to the Rust Admin API through
`/api/admin/system-mail`. The Next route adds local operator headers server-side
and forwards `SendSystemMail` commands to `apps/admin-api`. With
`ADMIN_GATEWAY_MAIL_URL` configured, the command reaches the running gateway and
the player Mail panel can display, claim, and delete the delivered mail. Other
dashboard pages still use mock read data until real read models/projections are
implemented.

## Local Run

Start the Rust API:

```bash
ADMIN_API_ADDR=127.0.0.1:7420 cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Start the admin web:

```bash
ADMIN_API_BASE_URL=http://127.0.0.1:7420 ./node_modules/.bin/next start -p 3020
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
  --data '{"targetKind":"character","targetId":"AZ-1048","subject":"Smoke","body":"Queued through admin-web proxy.","reason":"local next route integration smoke","attachments":[{"itemId":"gold","count":100}]}'
```

Latest smoke screenshots:

- `docs/admin-web-dashboard-smoke.png`
- `docs/admin-web-gm-tools-smoke.png`

## Production Gaps

- Replace local env operator headers with real operator auth.
- Add approval and second-confirmation flows for dangerous commands.
- Back command/audit repositories with Postgres.
- Wire read models to real account, player, economy, server, and risk projections.
- Extend real command executors beyond system mail.
