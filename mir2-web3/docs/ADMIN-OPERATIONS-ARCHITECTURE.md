# Admin Operations Architecture

Last updated: 2026-04-27

Status: implementation started.

Purpose: define the technical architecture for the MMORPG operations backend. This document complements `docs/TECH-MODERNIZATION-RFC.md` and should be read before implementing admin APIs, GM tools, content publishing, or direct production support workflows.

Implementation note: `apps/admin-api` now contains the first command/audit/RBAC primitives, repository traits, Axum HTTP routes, and a `SendSystemMail` domain executor. `apps/admin-web` now contains the first desktop operations UI and connects the GM mail form to the Rust API through a Next route. `SendSystemMail` now attempts live gateway delivery through `POST /admin/system-mail` before falling back to the persistent account store, so the default local flow is visible in the in-game Stage 5 mail panel and claimable through gameplay.

## First Principles

The operations backend is a production control plane, not a database CRUD screen.

Core principles:

- Admin tools must not bypass authoritative game logic.
- High-value state changes must be represented as audited commands.
- Every write must have actor, target, reason, request id, before/after where practical, and result.
- Read paths may use query models, replicas, or projections; write paths must go through Admin API and command execution.
- Permissions must be explicit and least-privilege.
- Dangerous operations need confirmation, optional approval, and rollback or compensation strategy.
- Admin tools must be safe when the target player is online, offline, reconnecting, or moving between zones.
- Content/config changes must be versioned and publishable/rollbackable.

## Target Shape

```text
Admin Web (NextJS)
        |
Admin API / Control Plane (Rust)
        |
Auth + RBAC + Audit + Approval
        |
Admin Command Model
        |
Command Bus / Outbox / Queue
        |
Game, World, Account, Content Services
        |
Postgres + Redis
```

The first implementation can be modular inside the existing workspace. The boundaries should still be designed as if the admin control plane, game services, and frontend are separable.

## Recommended Packages And Apps

Future target layout:

```text
apps/admin-web
apps/admin-api
apps/gateway
apps/simulation
packages/admin-types
packages/content
packages/db
```

Initial implementation may be smaller, but avoid coupling admin UI directly to game tables.

## Components

### Admin Web

Recommended stack:

- NextJS.
- Separate app from the player-facing web client when the surface becomes production-grade.
- Shared design tokens are fine; shared auth/session state with the player app is not.

Responsibilities:

- login through approved operator identity provider;
- account and character search;
- character profile inspection;
- inventory/storage/equipment/mail inspection;
- admin command forms;
- command status and audit views;
- content publish workflows;
- dashboards and operational views.

Current implementation:

- `apps/admin-web` is a separate NextJS app from the player-facing web client.
- Implemented pages: Dashboard, Player Management, Player Detail, Economy, Activities, World Monitor, Anti-Cheat, Mail/GM Tools, and Audit Log.
- The `Mail/GM Tools` page posts `SendSystemMail` through `/api/admin/system-mail`, which forwards to the Rust Admin API with server-side operator headers.
- The dashboard, GM tools, and audit pages read the Rust Admin API when available and show an offline state when the API is not running.

Admin Web should not contain hidden direct mutation endpoints or database credentials.

### Admin API

Recommended stack:

- Rust service in the same workspace.
- Strong typed request/response models.
- Server-side validation for every command.
- Structured logs and trace ids.

Responsibilities:

- authenticate operator identity;
- evaluate RBAC/permissions;
- validate request parameters;
- enforce rate limits and confirmation requirements;
- create command records;
- write audit records;
- dispatch commands to the appropriate executor;
- expose read-only query APIs and command status APIs.

Current implementation:

- `GET /health`
- `GET /admin/commands`
- `GET /admin/audit`
- `GET /admin/system-mail/outbox`
- `POST /admin/commands/send-system-mail`
- Write routes require operator headers and permissions.
- Command and audit persistence are represented by `AdminCommandRepository` and `AuditRepository`; current local implementation is in-memory.
- `SendSystemMail` writes command/audit records and then attempts live delivery to the running gateway through `ADMIN_GATEWAY_MAIL_URL` (default `http://127.0.0.1:7110/admin/system-mail`).
- If the gateway is unavailable, `SendSystemMail` falls back to writing persistent game mail through `ADMIN_ACCOUNT_STORE_PATH`, `MIR2_ACCOUNT_STORE_PATH`, or `.mir2-data/accounts.json`.

### Game Command Executors

Executors convert approved admin commands into authoritative game changes.

Examples:

- account service handles ban/unban/account notes;
- character service handles offline grants or profile edits;
- world/zone service handles online kick, teleport, live stat updates, and online item grant events;
- content service handles config bundle publish and rollback.

Executors should be idempotent where possible. Commands should have stable command ids and dedupe behavior.

Current executor boundary:

- `SystemMailDomain` models the mail-service handoff.
- `SystemMailExecutor` converts an approved `SendSystemMail` command into a domain `SystemMailRequest`.
- `InMemorySystemMailOutbox` is a local stand-in for the future mail/account service queue.
- `AccountStoreSystemMailDomain` is the first real game-state executor: it maps `gold` attachments into mail gold, repeats item attachments by count, posts to the live gateway when available, and records whether delivery used `gateway_live` or `account_store_fallback`.
- This keeps the command/audit/write path real while deferring Postgres-backed repositories, real auth, approvals, and broader game-state commands.

### Live Gateway Mail Flow

Local smoke topology:

```text
Admin Web :3020
        |
Next /api/admin/system-mail
        |
Admin API :7420
        |
POST /admin/commands/send-system-mail
        |
Gateway live mail endpoint :7110/admin/system-mail
        |
SimulationConfig.account_store / Stage5SystemsState.mail
        |
Player WebSocket world snapshot / Mail panel / mail.claim
```

Required local environment:

```bash
MIR2_ACCOUNT_STORE_PATH=.mir2-data/admin-live-smoke.json
ADMIN_ACCOUNT_STORE_PATH=.mir2-data/admin-live-smoke.json
ADMIN_GATEWAY_MAIL_URL=http://127.0.0.1:7110/admin/system-mail
ADMIN_API_BASE_URL=http://127.0.0.1:7420
```

Verified local behavior:

- Admin Web POST to `/api/admin/system-mail` returns a command id from Rust Admin API.
- `GET /admin/system-mail/outbox` reports `deliveryMode: "gateway_live"`, `deliveredCount: 1`, and the generated mail id.
- Gateway WebSocket world snapshots expose the mail at `payload.stage5Systems.mail`.
- Sending `stage5Command` `mail.claim` through the game socket marks the mail claimed and transfers attachments into player state.

### Postgres

Postgres stores:

- operator audit logs;
- admin command records;
- command results;
- approval records;
- account/character authoritative state;
- content bundle metadata;
- support notes;
- risk flags.

Use normal game tables only through approved service code or migrations. Do not make admin UI write game state tables directly.

### Redis

Redis may store:

- short-lived admin session cache;
- command status cache;
- online player routing;
- rate limits;
- locks for command execution;
- queue/stream state for early command dispatch.

Redis is not authoritative for admin command history or valuable game state.

## Auth And Permissions

Use external identity where possible:

- Google Workspace;
- Auth0;
- Clerk;
- Keycloak;
- another OIDC/OAuth2 provider.

Minimum roles:

- `viewer`: read-only dashboards and account/character lookup.
- `support`: support notes, safe read-only investigation, limited kick if needed.
- `gm`: grant items/currency through approved forms, send mail, teleport/kick, event support.
- `ops_admin`: content publish, high-risk account actions, economy interventions.
- `super_admin`: permission management and emergency operations.

Permissions should be action-based, not only role-name based.

Examples:

```text
account.read
account.ban
character.read
character.kick
inventory.read
inventory.grant_item
currency.grant
mail.send_system
content.publish
content.rollback
audit.read
permission.manage
```

## Audit Model

Every admin write should produce an immutable audit record.

Suggested fields:

```text
audit_id
command_id
operator_id
operator_email
operator_role_snapshot
permission
target_type
target_id
target_account_id
target_character_id
reason
request_payload_hash
request_payload_redacted
before_snapshot_ref
after_snapshot_ref
status
error_code
created_at
completed_at
client_ip
user_agent
trace_id
approval_id
```

Sensitive values such as passwords, private tokens, and payment data must be redacted.

## Admin Command Model

Commands should be explicit and typed.

Example command categories:

- `BanAccount`
- `UnbanAccount`
- `KickPlayer`
- `SendSystemMail`
- `GrantItem`
- `GrantCurrency`
- `GrantExperience`
- `TeleportCharacter`
- `SetCharacterFlag`
- `PublishContentBundle`
- `RollbackContentBundle`
- `AddSupportNote`

Example execution flow:

```text
Admin Web
  -> POST /admin/commands/grant-item
Admin API
  -> authenticate operator
  -> authorize inventory.grant_item
  -> validate item id/count/target
  -> require reason
  -> create admin_command row
  -> create audit pending row
  -> dispatch command
Executor
  -> route to online zone or offline character service
  -> apply through authoritative game/state code
  -> write result
  -> finalize audit
Admin Web
  -> displays command result and audit id
```

Avoid:

```text
Admin Web -> direct SQL update inventory/equipment/currency
```

## Online And Offline Targets

Commands must handle both player states.

Online player:

- resolve current gateway/world/zone route;
- dispatch command to the owning service;
- mutate through live authoritative state;
- persist result;
- push client event if needed.

Offline player:

- load or lock authoritative saved state;
- apply command through the same domain rules where practical;
- persist result;
- mark pending notifications for next login where needed.

Player moving between zones:

- command should use route versioning or retry on stale route;
- duplicate command execution must be prevented by command id.

## Content And Config Publishing

Content changes should not be ad hoc DB edits.

Recommended flow:

```text
source config / DSL
  -> compiler / validator
  -> content bundle
  -> staging validation
  -> admin approval
  -> publish version
  -> world/zone reload or rolling update
```

Content bundles should include:

- version id;
- source commit or artifact hash;
- validation result;
- publish operator;
- rollback pointer;
- affected systems.

This should eventually cover:

- NPC DSL scripts;
- quest data;
- item data;
- monster data;
- drop tables;
- shop data;
- events;
- localization.

## MVP Scope

Phase 1 admin MVP:

- operator login;
- RBAC skeleton;
- account lookup;
- character lookup;
- inventory/equipment/storage read-only views;
- online state view;
- kick player;
- ban/unban account;
- send system mail;
- grant item;
- grant currency;
- immutable audit log;
- command status view.

Defer:

- full content publishing;
- advanced economy analytics;
- fraud/risk engine;
- multi-step approvals;
- support ticket integration;
- live event scheduler.

## Later Scope

Phase 2:

- content bundle publish/rollback;
- NPC/quest/drop/shop config management;
- auction/trade/mail audit views;
- economy dashboards;
- map population dashboards;
- command approval workflows;
- support notes and case history.

Phase 3:

- anomaly detection;
- automated rollback triggers;
- event scheduler;
- GM live tools;
- region/zone management;
- moderation queues;
- payment/refund correlation where applicable.

## Safety Requirements

Before production use:

- all admin routes require auth;
- all write routes require permissions;
- all write routes require reason text;
- all writes create audit records;
- dangerous writes require confirmation;
- command idempotency is tested;
- offline and online target paths are tested;
- audit logs cannot be modified through normal admin UI;
- production database credentials are not present in Admin Web;
- rate limits protect high-risk routes.

## Suggested First Engineering Task

Do not start with UI.

Start with the command and audit model:

1. Define admin command types.
2. Define audit schema.
3. Define permission names.
4. Define command execution states.
5. Implement a small fake executor for tests.
6. Add one safe end-to-end command such as `SendSystemMail`.

Only after that should the Admin Web form be built.

## Open Questions

- Which OIDC provider should operators use?
- Should `apps/admin-api` be a separate binary immediately or share an initial crate with gateway?
- Should command dispatch start with Postgres outbox or Redis Streams?
- Should high-risk commands require approval from another operator in MVP?
- How should support notes relate to player-visible account notes?
- What retention policy should audit logs use?
- Which admin commands must be available for launch day?
