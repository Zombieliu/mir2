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
- in-memory command/audit repositories for local tests and smoke runs;
- command idempotency guard through `AdminCommandRepository::insert_pending`;
- `SendSystemMail` domain executor and in-memory outbox;
- Axum HTTP routes for health, command records, audit records, outbox records, and
  `SendSystemMail`.

The current `SendSystemMail` executor intentionally queues to a domain outbox
instead of mutating live game state. This keeps the write path production-shaped
without pretending live account/world/mail services are already connected.

## HTTP Routes

Default bind:

```bash
ADMIN_API_ADDR=127.0.0.1:7420 cargo +1.89.0 run --locked -p mir2-admin-api --bin mir2-admin-api
```

Routes:

- `GET /health`
- `GET /admin/commands`
- `GET /admin/audit`
- `GET /admin/system-mail/outbox`
- `POST /admin/commands/send-system-mail`

Write routes require operator headers:

```text
x-operator-id
x-operator-email
x-operator-role
x-operator-permissions
```

For local GM mail smoke, include `mail_send_system` in
`x-operator-permissions`.

## Current Implemented Commands

- `SendSystemMail`: HTTP + RBAC + command repository + audit repository + domain
  outbox.
- `GrantItem`: typed model and validation only.
- `GrantCurrency`: typed model and validation only.
- `KickPlayer`: typed model and validation only.
- `BanAccount`: typed model and validation only.

## Verification

```bash
cargo +1.89.0 test --locked -p mir2-admin-api -- --test-threads=1
cargo +1.89.0 fmt --check
```

## Next Steps

1. Add Postgres-backed command and audit repository implementations.
2. Replace local header auth with OIDC/session middleware and RBAC policy loading.
3. Connect `SendSystemMail` outbox to the real mail/account service boundary.
4. Add typed executors for grant item, grant currency, kick, and ban.
