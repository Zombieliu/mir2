# apps/admin-api

Rust control-plane primitives for the Mir2 Web3 operations backend.

## Current Scope

This crate intentionally starts below HTTP:

- typed admin commands;
- operator permissions;
- command envelopes;
- validation;
- audit records;
- idempotency guard;
- executor trait;
- in-memory test control plane.

The goal is to make high-risk GM operations command/audit-first before exposing
Admin Web or HTTP routes.

## Current Implemented Commands

- `SendSystemMail`
- `GrantItem`
- `GrantCurrency`
- `KickPlayer`
- `BanAccount`

Only the command model and fake executor tests exist at this stage. The commands
are not yet wired to live account, gateway, or world services.

## Next Steps

1. Add persistent audit and command storage.
2. Add an Axum Admin API behind auth/RBAC.
3. Add a real executor for one safe command, likely `SendSystemMail`.
4. Add `apps/admin-web` after the command/audit boundary is stable.
