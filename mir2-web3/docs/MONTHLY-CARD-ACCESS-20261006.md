# Account monthly access — 2026-10-06

The user requested a monthly time-card implementation. This is a new account
access feature; it does not complete the older classic-gameplay P1–P8 Goal.
The implemented model is operator-issued activation codes. Online payment,
price, currency, refunds and a payment provider are not configured. The public
realm and Windows feed remain on their existing versions; paid access is OFF.

## Rules and ordinary player flow

- One code adds exactly 30 × 24 hours from redemption, by the server UTC clock.
- Every character on an account shares the expiry. Offline time counts.
- Active renewals extend the current expiry; expired accounts renew from now.
- Codes bind to the named account. They cannot be redeemed on another account.
- Unused codes have no deadline by default. An optional issuance validity
  setting applies only to newly issued, unused codes, not existing subscriptions.
- Login and character selection remain available without a valid card. Required
  servers deny StartGame until redeemed. The native character screen has a
  Monthly Card modal, expiry, code input/Ctrl+V, redeem and status refresh.
- Expiry invokes the existing authoritative LogOut/save/Zone departure path.
  It releases the route, active capacity, AOI registration and resume family.
  A dedicated one-second admission wake is not postponed by bootstrap grace.
- Redeem response-loss retry credits the same code only once. Native receipts
  correlate request IDs; a late receipt cannot settle a different redemption.

## Server configuration and rollout

`MIR2_MONTHLY_CARD_REQUIRED` defaults to `0`; accepted values are `0`, `false`,
`1`, `true`. `1` requires a separately generated private
`MIR2_MONTHLY_CARD_ISSUANCE_KEY` of at least 32 bytes and an authoritative File
or PostgreSQL account store. Use a random key, not the QA constants in tests.
The administration API additionally requires an explicit private
`MIR2_GATEWAY_OPERATOR_TOKEN` of at least 32 bytes; its developer fallback is
deliberately unavailable for issuance. Existing save recovery configuration
and its separate MAC key still apply.

Optional `MIR2_MONTHLY_CARD_CODE_VALIDITY_DAYS` accepts 1–3650. An absent value
means no unused-code expiration. Configure the same access policy and issuance
key on all active gateways. Keep the original key for issuer retries: rotating
it preserves existing code redemption but an old order cannot be re-issued
under a new key. Retain private keys outside the repository and client package.

The AccountStore schema is now 7. Legacy records have no implicit subscription.
Monthly expiry and hashed redemption receipts share the existing account-store
atomic transaction. File uses atomic publication; PostgreSQL uses existing
account `raw_json` and optimistic versions, without a new SQL column. Do not
run an old PostgreSQL account writer alongside the new writer: an old serializer
can strip unknown monthly fields. Back up the account store and upgrade all
writers together before enabling paid access. No live migration or paid
activation was performed in this implementation round.

Stage matching clients and gateways with REQUIRED=0 first. Establish the actual
price/payment process, distribute entitlements or an explicit trial policy, and
then enable REQUIRED=1. Never enable it first and strand current test accounts.

## Administration and transport

`POST /admin/monthly-cards/issue` takes `{accountId, requestId}` under operator
Bearer authorization. Each real order must keep a stable 12–96 character
`requestId` using letters, digits, `_`, `-`, `.`, `:`. A retry returns the same
account-bound code. A lost response must not create a new order ID.

The operator can run `scripts/issue-monthly-card.ps1 -ServiceBaseUri <HTTPS URL>
-AccountId <account> -OrderId <stable order>`. It reads the operator token only
from the process environment; never give that token to players. Its loopback
HTTP exception is explicit and intended only for an owned QA gateway.

Authenticated identity Bearer endpoints are `GET /v1/monthly-card` and
`POST /v1/monthly-card/redeem` with `{code}`. The account comes from verified
identity, never the request. Extra HTTP account/duration/expiry fields are
rejected. Responses use Cache-Control: no-store and bounded public error codes.
The ledger stores SHA-256 code hashes, not plaintext codes or issuance secrets.

Native authenticated WebSocket commands are `monthlyCardStatus` and
`redeemMonthlyCard {code, requestId}`. A `monthlyCard` envelope returns operation,
requestId, status/replayed or a public error. World packets remain unchanged.
TCP world entry and idle expiry use the same policy; TCP redemption uses the
authenticated HTTP endpoint. Wall-clock checks do not scan historical receipts
on each movement: admission reads the cached expiry, with an authoritative
refresh at expired boundaries and world entry/resume.

## Verification and remaining delivery

- Windows native binary compiled with Rust 1.95.0; monthly wire/redaction 1/1.
- Simulation monthly ledger 9/9, including boundary, renewal, independent
  account isolation, six-thread exactly-once redemption, atomic File reload,
  key rotation and failed-persist rollback.
- Real Axum HTTP/WebSocket + expired resume 3/3. A fresh QA account registers,
  logs in normally, creates a character, is denied without access, redeems,
  enters the shared Zone, expires on an actual idle server wake, is saved and
  logged out, renews over HTTP and enters again under a one-player limit.
- Initial failures were a missing QA save recovery key, delayed denied-entry
  capacity release and bootstrap timer deferral. Their diagnoses and corrected
  real-flow checks are recorded here; the fixes retain the actual policy,
  authentication and persistence gates.
- Existing native-resume 22/22 and account-store 22/22 regressions pass.
- Native state/input and receipt correlation 5/5 pass again on the final UI
  binary. The explicit offline GPU fixture runs all nine locales. Each open
  modal changes the captured frame from its closed baseline; missing glyphs,
  overflowing text nodes and nodes outside the viewport are all zero.
- Manual review of the first GPU screenshots caught the monthly layer behind
  the character-selection root: layout checks alone had incorrectly passed.
  The layer is now 2000 above the shell root at 1000, and the fixture additionally
  compares actual rendered open/closed frames. The initial occluded frame and
  layout report are retained as failure evidence, not counted as acceptance.
  Final Traditional Chinese, Arabic and Brazilian Portuguese frames were
  visually inspected. Full player interaction and human translation acceptance
  are still unexecuted.
- PostgreSQL was locally ignored because there is no running local service.
  Actual isolated PG16 CI explicitly executes four independent writers, single
  redemption credit, reopened-store persistence, renewal and hashed receipt
  storage: 1/1 passes. The complete [server acceptance run37464517808](https://github.com/Zombieliu/mir2/actions/runs/37464517808)
  succeeds on `cb330c515a3f16a3d5fb0acbeac0f8fe35b1aedc` with ledger9/9 and
  real HTTP/WS/resume3/3. The UI-only correction follows that server source.

[Durable screenshots, reports and execution receipts](generated/player-qa/monthly-card-20261006/README.md).

No public client/Gateway rollout, production account migration, subscription
grant or charging switch was performed. Finish the paired deployment and
ordinary player interaction with REQUIRED=0 before any paid activation. Price,
currency and payment provider are separate business inputs; the current feature
supports trusted operator-issued activation codes.

This feature grants account access only. It does not alter experience, drops,
game coins, daily rewards, mining or equipment upgrades.
