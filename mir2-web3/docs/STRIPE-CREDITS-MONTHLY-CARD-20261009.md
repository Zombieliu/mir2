# Stripe Credits and monthly-card Candidate — 2026-10-09

The authorized change connects one-time Stripe Hosted Checkout payments to
the existing character **Credits** wallet and sells a **30-day monthly-card
consumable** for Credits. This is an unpublished Candidate. Public R22,
Gateway a41 and signed feed17 remain unchanged. No real Stripe account,
charge, webhook registration, production database migration or native human
acceptance is claimed by local fixtures.

## 2026-10-10 provider credential and pricing follow-up

The operator supplied a sandbox server key in a user-only private file outside
Git. Real Stripe authenticated Checkout, PaymentIntent, Charge and webhook
list requests return200. The pinned Endive API rejects `payment_method_types`
with400; `allowed_payment_method_types[0]=card` succeeds while retaining the
card-only policy. The Gateway request and captured HTTP form regression are
corrected, and all36 billing tests pass. The unpaid provider probe is expired;
a temporary webhook/signing-secret creation is verified and its endpoint
deleted. There is no real charge, game-wallet award or completed payment test.

Confirmed operator pricing is **USD1 =100 Credits**, with a **30-day card
costing1000 Credits** (USD10). Private test configuration supplies USD1/5/10/20
offers with100/500/1000/2000 Credits and keeps billing disabled. No prices or
keys are installed into public services. A permanent matching webhook, public
test purchase/refund, live credentials, paired deployment and native acceptance
remain open. [Evidence and original failure](generated/player-qa/stripe-billing-20261009/provider-20261010/README.md).
The earlier fixture/CI results below remain valid within their stated scope.

## Player flow

1. Log in normally. Open **Recharge** in the native GameShop, or use the
   monthly-card panel on the character-selection screen.
2. Select an operator-configured recharge offer. The server freezes its
   currency, minor-unit amount, Credits, owner and character in a durable order.
3. The native client opens only a validated `https://checkout.stripe.com/c/pay/`
   URL through the Windows browser API. It passes no shell command or secret.
4. Payment is confirmed by a signed raw-body webhook and a server-to-Stripe
   read of the actual Checkout object. The browser return page never awards
   Credits. A verified order is first **PaidPending**; the serialized Source
   owner consumes it into the current character checkpoint. Ordinary Tick
   polls at five-second intervals; explicit refresh and purchases check sooner.
5. Use Credits to buy **30-day monthly card**, then use its exact item UID.
   The card extends account access to `max(now, current_expiry) + 30 days`.
   Offline time counts. Reusing the same UID returns its receipt without
   consuming another item or extending time again.

Expired accounts can still log in, purchase and activate from character
selection before StartGame. An in-world normal GameShop purchase uses the
existing parcel/mail delivery and claim path. The offline purchase goes to the
owned bag; an unclaimed paid card can also be activated before world entry.
Credits retain the existing per-character scope; monthly access is account-wide.
This is a consumable purchase, not automatic Stripe subscription renewal.

The native billing UI has translations for all nine existing locales. Exact
monthly-card name and canonical tooltip translations are included. Model,
keyboard, response-correlation and compile evidence does not establish visual
or human acceptance on another Windows machine.

## Server configuration

Payments and the monthly product are disabled by default. The operator has
confirmed USD1/100 Credits and monthly1000 pricing; it is saved only in private
test configuration. Provision the matching callback and deploy all compatible
writers before enabling. Production provider acceptance is still pending.

| Setting | Purpose |
| --- | --- |
| `MIR2_STRIPE_ENABLED=1` | Enable the payment provider; absent/`0` leaves gameplay available |
| `MIR2_STRIPE_SECRET_KEY` | Server-only test or live Stripe secret/restricted key |
| `MIR2_STRIPE_WEBHOOK_SECRET` | Server-only webhook endpoint signing secret |
| `MIR2_BILLING_PUBLIC_BASE_URL` | HTTPS origin/base for the cosmetic return page |
| `MIR2_STRIPE_RECHARGE_OFFERS` | JSON array of frozen server offers |
| `MIR2_MONTHLY_CARD_CREDIT_PRICE` | Positive Credits price, identical on Gateway and every Source |
| `MIR2_MONTHLY_CARD_REQUIRED` | Existing independent entry policy; selling cards alone does not enable the gate |
| `MIR2_MONTHLY_CARD_ISSUANCE_KEY` | Existing operator-code key; at least32 bytes and required when entry policy is enabled |

Each offer object has `id`, `label`, lower-case three-letter `currency`,
`amountMinor` and `credits`. Accept one to twelve unique offers. Both amounts
are positive integers; monetary amounts are provider minor units, not floats.
ISK/UGX amounts must be a multiple of100 as required by Stripe. Configure
actual accepted currencies, provider limits and business prices in Stripe
test mode first. Keep keys in the server's secret configuration, never a
client build, repository, installer or conversation.

Durable storage and a positive monthly-card price are required to create
Checkout or process payment webhooks. **Live keys additionally require
PostgreSQL SourceOfTruth**; JSON-file storage is for local/test evidence only.
Provider configuration errors fail billing closed without preventing ordinary
login/gameplay. The existing monthly entry policy remains independent.

Register the HTTPS endpoint `POST /v1/billing/stripe/webhook` with pinned
Stripe API version **2026-09-30.endive** for `checkout.session.completed`,
`checkout.session.async_payment_succeeded`, `charge.refunded` and the
`charge.dispute.*` events handled by the provider. The request signature is
checked before parsing JSON, with bounded body/header sizes and a five-minute
timestamp tolerance. Amount, currency, test/live mode, session, payment intent,
owner, character and order are checked against the frozen tuple.

Authenticated HTTP surfaces are `GET /v1/billing/status` and
`POST /v1/billing/checkout`, using a real issued IdentityService Bearer token.
Native WebSocket operations are `billingStatus`, `billingCheckout`,
`billingBuyMonthlyCard` and `billingActivateMonthlyCard`; account identity
comes from the verified socket, never a client account field. No raw admin,
Passkey account shortcut or debug world command is added to production clients.

## Financial and ownership rules

- A stable request ID binds one order and one immutable offer. Stripe
  idempotency keys derive from that durable order. An old unbound order after
  twenty hours is left unconfirmed instead of recreating Checkout after
  Stripe's key retention window. Provider requests have timeouts and no redirects.
- Duplicate payment events, buy requests and card activation requests reuse
  permanent receipts. A callback marks PaidPending rather than overwriting
  the live owner's whole save. The serialized owner commits Credits, receipt
  and current checkpoint together, preserving unsaved state and Zone pools.
- Private Source billing mutations require `billingCreditsMonthlyCardV1` and
  execute once. A dropped response never triggers endpoint fallback/replay.
  Unknown publication/commit outcomes freeze or retain reconciliation state;
  they are not reported as rolled back. Offline route leases are held for
  their configured300-second fence on unknown outcomes, not indefinitely.
- New orders reserve `u32` wallet capacity against all unapplied nonterminal
  orders. Later ordinary earnings can still fill that capacity. Whole paid
  orders that fit are applied; the others remain pending without clipping.
  If none fits, no character revision or unsaved snapshot is published.
  Spending existing Credits allows a later Tick/refresh to finish settlement.
- Character deletion rejects unsettled orders, unapplied Review orders,
  remaining Credits with payment history and unredeemed monthly-card units.
  Financial receipts survive permitted character deletion. Provider lookup
  remains possible for an archived character without a roster/save entry.
- The custom monthly item and GameShop entry use index1000001; original
  Crystal manifests are unchanged. Card UIDs occupy a reserved exact integer
  namespace. Key-only UseItem aliases cannot consume one. Paid units cannot
  be dropped, sold, mailed, traded, stored or copied to another carrier.
- Refund/dispute events record **Review** for operator handling. They do not
  erase acquired items, rewind saves, subtract already-spent Credits or cancel
  paid access automatically. Cancellation/expiry release and an operator
  reconciliation console are not implemented; abandoned Prepared/CheckoutBound
  reservations and unconfirmed outcomes need support review. This limitation
  must be resolved operationally before accepting live payments.

The account business JSON uses schema**8**, adding recharge and item receipts.
Older accounts upgrade with empty financial authority; legacy-schema injected
paid ledgers are rejected. All writing Gateway/Source processes must be upgraded
together before enabling payments. The RPC capability guard prevents billing
calls to old Sources, but cannot make old whole-account JSON writers safe.
Take a private database backup and keep payments disabled during rollout.
Per-account PostgreSQL CAS and frozen provider binding are tested; no global
SQL unique provider-ID constraint or cross-process JSON-file authority is claimed.

## Verification and release gates

Local final simulation evidence: **26 recharge +9 item/card +9 existing code
tests passed**. The PostgreSQL test compiles but is default-ignored locally;
its actual multi-writer run now passes in isolated CI37946216332 at exact
code commit`4a26a543110a4810296ba1992f60b811d6509424`, never a production database.
That Stripe workflow passes35 Source/1 PostgreSQL/36 Gateway/4 shared tests;
adjacent monthly workflow37946216237 passes9 Source/1 PostgreSQL/4 Gateway checks.
The repeated checks are not summed as new unique coverage. Gateway and CI results are recorded in
[the evidence checkpoint](generated/player-qa/stripe-billing-20261009/README.md).
Windows host billing tests5, fresh client-rlib model tests10 and isolated URL
checks2 were run; final native `--tests` check passes. No screenshot was taken.

The new Stripe acceptance workflow uses an explicitly named isolated Postgres16
database, four independently loaded writers, real local HTTP/WS/private TCP
transport fixtures, durable restart checks and original assertions. It retains
failed runs/logs. It never contacts Stripe or publishes a Gateway/client/feed.
The adjacent monthly-card acceptance toolchain is aligned to installed1.95.

Release still requires live credentials, exact-source Linux and Windows
builds, a Stripe test-mode purchase/refund/retry story through the public HTTPS
endpoint, normal owner drain/backup and paired schema-compatible rollout, plus
native human purchase/claim/use/expired-renewal acceptance. Current installers
do not contain this Candidate. This feature does not close the original P1–P7
goal or its missed deadline.

Official provider references: [Hosted Checkout](https://docs.stripe.com/checkout/quickstart),
[fulfillment](https://docs.stripe.com/checkout/fulfillment),
[webhook verification](https://docs.stripe.com/webhooks),
[idempotent requests](https://docs.stripe.com/api/idempotent_requests),
[API versions](https://docs.stripe.com/api/versioning),
[currency minor units](https://docs.stripe.com/currencies).
