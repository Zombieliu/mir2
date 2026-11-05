# Dubhe Wallet → Numeron test-card fulfillment

This optional adapter is isolated from the game Gateway binary. It implements
only the fixed `numeron-test-100` sandbox SKU: USD 1 (100 cents) for 100 test
credits. It does not accept arbitrary prices, identify a normal player by a
wallet address, or implement production billing.

## What is actually credited

The adapter creates one dedicated QA account and character per wallet through
the existing Gateway's ordinary `NewAccount`, `Login`, and `NewCharacter`
commands. It does not use `StartGame`, clone another player's account, or reuse
an existing player's character. Ownership is reserved before network I/O, so an
interrupted preparation can resume only its own reserved account. Account and
character names use a random, bounded QA namespace.

Once created, the QA account is locked with ban reason
`dubhe_wallet_sandbox_locked_v1`. Its credentials are never returned to the
wallet, stored in these adapter tables, or logged. A separate server-only
provisioning key derives the login password; it must differ from the shared
callback token and remain stable for interrupted provisioning recovery.

Fulfillment requires that exact lock marker in both account columns and the
authoritative JSON, one matching QA character, and no unexpired Gate 14 session
lease. This is an offline test recipient. It is not an ordinary online player
payment feature. Unlocking a recipient disables further fulfillment until an
operator intentionally restores its QA lock. Actual player billing requires an
authoritative online/offline credit command and its own merchant-order design.

The credited `credit` field is the existing game's real persistent points
field. Each successful transaction updates all of the following atomically:

- `accounts.raw_json.saves[character_index].credit` and `store_version`;
- `character_saves.credit`, `snapshot_json.credit`, and `save_version`;
- `character_state.credit` and its corresponding `save_version`;
- a unique order and Stripe PaymentIntent receipt in
  `dubhe_sandbox_payment_receipts`.

Before updating, all four credit copies and projection/save versions must agree.
Only the credit fields are replaced with `jsonb_set`; unrecognized account/save
fields and all unrelated player records are preserved. A missing or divergent
projection fails closed. Duplicate requests return the original receipt, and a
PaymentIntent cannot fulfill a second order or wallet.

## Payment boundary

The wallet's cards Worker verifies the official Stripe webhook signature and
retrieves the current test Checkout Session, PaymentIntent and customer. Only
after those checks may it invoke this authenticated receiver. A success-page
redirect or an arbitrary webhook body is not a game payment receipt.

The receiver only accepts exact `Bearer <64 lowercase hex>` authentication with
constant-time comparison, `mode=test`, and the fixed catalog. It has no card
number/CVC fields and does not call Stripe. The shared token grants only these
test preparation/fulfillment operations; it is not a Gateway operator token.

## Interfaces

`POST /prepare` accepts exactly:

```json
{
  "mode": "test",
  "requestId": "<64 lowercase hexadecimal characters>",
  "ownerAddress": "<0x and 64 lowercase hexadecimal characters>",
  "network": "testnet",
  "sku": "numeron-test-100",
  "amount": 100,
  "currency": "usd",
  "credits": 100
}
```

The response includes `mode`, `dedicatedTestAccount=true`, the immutable
catalog/owner, and `merchantOrderId`, `accountId`, `characterId`, `serverId`.
`characterId` is `account_id:character_index`. Reusing `requestId` returns the
same prepared order; changing its wallet/network is rejected.

`POST /fulfill` accepts exactly the same mode, owner, network and catalog plus
`orderId`, `merchantOrderId`, `accountId`, `characterId`, `serverId`,
`paymentIntentId`. It omits `requestId`. Every business field must match the
prepared order. Its response includes `delivered=true`, `orderId`,
`merchantOrderId`, `accountId`, `characterId`, `credits`, `balanceAfter`, and
`duplicate`.

`GET /health` exposes only readiness/mode/service name. JSON request bodies are
bounded to 16 KiB; duplicate keys, unknown fields, wrong types, non-JSON
requests, unbounded/chunked requests and non-test requests are rejected. No
incoming body, credential or database-driver message is logged.

## Installation and publication

The parent deployment must first verify the chosen Gateway's live database and
origin. Point both `NUMERON_SANDBOX_DATABASE_URL` and
`NUMERON_SANDBOX_GATEWAY_WS` at the same existing remote game. Do not replace the
Gateway with this worktree's older/newer source or restart gameplay to install
this adapter.

1. Copy `apps/wallet-sandbox-payments/` into a new versioned release directory.
2. Create a dedicated Python 3.10+ virtual environment and install the pinned
   `requirements.txt`.
3. Create an unprivileged service account and a root-readable private environment
   file based on `.env.example`. Keep the two independent secret values out of
   source, command-line arguments and logs. The provisioning key is server-only;
   only the callback token is shared with the wallet Worker.
4. With a separate database-administrator connection, apply `migration.sql`,
   then the optional least-privilege `runtime_access.sql`. Run both with error
   stopping and transaction boundaries. Existing game tables must already exist.
   The latter creates fixed role `mir2_wallet_sandbox` as **NOLOGIN**, and private
   schema `dubhe_wallet_sandbox`. It leaves normal game tables and their owners
   untouched. Only after both scripts pass, privately assign this role's password
   and enable LOGIN; put its connection string in the service environment file.
   Do not provide administrator credentials to the running service.
5. Set `NUMERON_SANDBOX_ENABLED=true` and `NUMERON_SANDBOX_MODE=test`. Install a
   service based on
   `numeron-wallet-sandbox-payments.service.example` and verify loopback health
   on port 7431. Runtime deliberately binds only `127.0.0.1`.
6. Add a narrowly scoped Caddy route that strips the chosen HTTPS path prefix
   and proxies only that prefix to `127.0.0.1:7431`. Preserve and validate the
   current Caddy configuration before reloading it. No port 7431 firewall opening
   is required. Configure the wallet Worker's `NUMERON_SANDBOX_URL` to the exact
   HTTPS prefix and its callback token to the same private token.
7. Verify a real paid sandbox order, the stored game credit fields, replay
   behavior and Ops correlation. Record the actual published HTTPS URL in the
   deployment receipt; no URL is claimed deployed by this document.

### Database permission boundary

The runtime role uses database-default
`search_path=dubhe_wallet_sandbox,public`. Its protected, automatically updatable
security-barrier views expose only rows whose QA account ID is already reserved
in `public.dubhe_sandbox_wallets`. Character views additionally restrict the
recorded character index once provisioning finishes. View check options and
column-specific grants limit updates to lock, credit, JSON, version and time
fields. The role has no direct SELECT/UPDATE rights on the public game tables,
and no schema/table/function creation rights. The installer refuses an existing
PUBLIC/base-table policy that would let this role bypass that boundary; it does
not weaken or globally rewrite the game's privilege policy.

Reservation still needs to detect an unowned randomly chosen QA account name.
The restricted `public.dubhe_sandbox_candidate_exists(text)` SECURITY DEFINER
function returns only a boolean for the exact QA namespace. It uses fixed
`search_path=pg_catalog`, a schema-qualified game table, and denies PUBLIC
execution. It cannot disclose any account record or check an ordinary player
name. Every other game query goes through the protected views.

The additive adapter tables grant only needed SELECT/INSERT/UPDATE access;
payment receipts cannot be updated or deleted by the runtime role. Serving HTTP
never migrates. Migration is an explicit administrator operation; a runtime
credential must not have DDL or broad game-table privileges. Reapplying
`runtime_access.sql` deliberately returns the role to NOLOGIN until deployment
reconfirms access and re-enables it.

Disabling/removing the adapter service or its Caddy route stops test purchases;
it does not roll back existing game state. Keep the original receipt table for
deduplication. Do not drop/reinitialize it during a retry or release rollback.

## Tests and evidence

From the adapter directory:

```sh
python -m unittest test_service -v
```

The five validation/HTTP/WebSocket tests run without a database. Set
`WALLET_SANDBOX_TEST_DSN` to an explicitly disposable PostgreSQL instance to run
the twelve transaction tests as well. Each database test creates and removes its
own random disposable database; it never reuses the game's database. The fixtures model
the live authoritative column types and test concurrent preparation, interrupted
provisioning recovery, concurrent duplicate delivery, concurrent cross-wallet
PaymentIntent reuse, forged fields, online or
unlocked recipients, divergent projections, transaction rollback after receipt
insertion, and preservation of unrelated JSON/player data. Two tests log in as
the actual restricted database role: preparation and concurrent fulfillment
succeed, while direct game-table/ordinary-player/credential/DDL access and
adoption of an unowned QA account are denied. The test DSN therefore needs
CREATE DATABASE permission and must refer to an explicitly disposable instance.

Local verification on 2026-11-06 used an isolated PostgreSQL 14.18 instance,
psycopg 3.3.8, websockets 17.4 and Python 3.14.3. Real remote PostgreSQL 16 schema
inspection confirmed the required game columns and Gate 14 session-lease
columns. Live payment/Gateway provisioning/deployment acceptance remains the
parent integration task and is not implied by local tests.
