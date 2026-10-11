# Public Stripe test mode — 2026-10-11

**Stripe test recharge is enabled on the invited public Gateway.** The user's
latest instruction, “你就用测试模式吧”, replaces the live-mode activation request.
Only an actual test server key and the matching permanent test endpoint secret
were installed. No real charge or bank payout occurs. Existing R24 clients can
open recharge without reinstalling; this round changes server configuration,
not the signed client release.

## Player flow

The four USD offers are $1 / $5 / $10 / $20 for 100 / 500 / 1000 / 2000 Credits.
The 30-day monthly card costs 1000 Credits in GameShop. Purchase delivers a
real item by mail; collect it into inventory and use it to add 30 days.
Purchase alone does not start the duration. Mandatory monthly entry remains
OFF independently of test recharge.

For a trial, choose the $10 offer, select USD if Checkout offers another
currency, and use Stripe's official test card `4242 4242 4242 4242`, a future
expiry such as `12/34`, and CVC `123`. Use fictional required contact fields;
do not enter a real card. Return to the game and use Check status before
purchasing the monthly item. [Stripe testing documentation](https://docs.stripe.com/testing)
explains that test payments do not move real funds.

## Actual configuration and lifecycle

Gateway stays at `4006d988b3922d0106278720abfa9b3d384c8d5e`, binary SHA256
`fc530898d8cbf563389fe708ac3a279846d28b56ae97e340af3ed7d100ad8202`.
Before applying configuration, all six counters, TCP7200 connections and
spectator viewers were zero. The service stopped normally with exit0. A new
root-private PostgreSQL dump plus state/config backups were taken under
`/var/backups/mir2-playtest-stripe-test-20261011-01`; no old save was restored.
PID3347791 becomes3369030. PostgreSQL remains SourceOfTruth; Redis, map/resource
configuration, service arguments, capacity51/66/51 and CPU/memory limits are
preserved. The original realm PID3855184 remains healthy and unchanged.

Only six billing assignments in `/etc/mir2-playtest/gateway.env` change:
enabled, test server key, endpoint signing secret, public return base URL,
loopback-return prohibition and the four offers. Every other environment byte
and non-transient running-process entry is preserved. Private configuration
SHA256 changes from `77f12d974ca32103673181669803c89283c68cf34a952c0a177b721c1d013729`
to `41d0939baa6f164a5db0605ef441647c4c8214a6b4ac1dd7c69ab1da65c647c0`.
Credentials are compared only in memory and excluded from Git, stdout and
public receipts. The temporary server transfer file is removed after success.

The permanent test endpoint is enabled at
`https://165.154.65.136.sslip.io/playtest/v1/billing/stripe/webhook`, pinned to
`2026-09-30.endive`, and subscribes only to the two handled Checkout completion
events and six refund/dispute events. Checkout/PaymentIntent/Charge/endpoint
test-mode API reads succeed before provision. The signing secret comes from
that endpoint, not the local Stripe CLI listener. Caddy remains at
`efbd0eaee9952dd38148c9b634f47889e99a9ddff632c5f2befebb6ce8339b28`.
Native recharge uses authenticated WSS; direct billing status/checkout/admin
HTTP neighbors remain404.

## Qualified public trial

One existing owned QA account uses ordinary password login, public WSS with
normal TLS verification and the genuine server-created $10 Checkout. The
browser visibly displays Sandbox; the official test card is submitted once.
Read-only Stripe API checks confirm this exact owned Checkout is complete/paid,
its PaymentIntent succeeded for1000 cents USD, and frozen order/account/character
metadata match. Ordinary status applies exactly1000 Credits with pending0.
No admin grant, direct save edit or simulated provider settlement is used.

Ordinary GameShop BUY for1000001 subtracts1000 Credits, returning a real
Gameshop parcel containing the exact monthly-card UID. CollectParcel yields
one GainedItem; ordinary inventory UseItem succeeds. The wallet becomes0,
the item is consumed, and expiry is `1794316965557` milliseconds, bounded by
the server time plus2,592,000,000 milliseconds. Repeating activation with the
consumed UID preserves that expiry and wallet. Normal logout/relogin preserves
the consumed card, applied payment order, zero balance and active monthly time.

The official Stripe CLI resends the same actual completion event twice to the
permanent endpoint. Pending webhook deliveries subsequently reach0; another
ordinary relogin proves no extra currency or time. A supplied bad signature
and a correctly signed wrong-live-mode body each return400/no-store. An
unsigned body returns400 `billingWebhookSignatureInvalid`. Return-page query
parameters cannot award Credits. Public signed feed19 and `latest.p7s` are
byte unchanged on both origin and CDN; nine final public checks pass. All six
Gateway counters and TCP7200 are zero after ordinary cleanup, NRestarts0.

## Retained corrections and scope

Original failed QA attempts are retained and excluded. In particular,
completed ordinary BUY requests mint a new server nonce per request; their
client requestId is not the gift retry contract. The first test incorrectly
required identical BUY replay success. Subsequent insufficient-credit receipts
did not debit anything. Resume used the confirmed existing parcel without
another purchase. The helper now forbids automatic BUY resend after an unknown
outcome. This round does not change that product behavior or claim BUY replay
idempotency. The corrected monthly debit assertion and ordinary2s application
heartbeat fix the separate test accounting/bootstrap failures.

GET Event retains the account's creation-time `2025-01-27.acacia` representation,
while deliveries use the permanent endpoint's pinned Endive representation.
The test uses official provider resend instead of relabeling an older API
snapshot and claiming it is the original delivery. This follows Stripe's
[event versioning and manual retry documentation](https://docs.stripe.com/webhooks).
All failed interpreter/browser/response-shape/signature-alias attempts remain
listed in the evidence. No game source change was necessary.

This qualification does not claim live-money acceptance, actual refund/dispute
execution, a Gateway cold restart after the paid trial, native human/laptop UI
acceptance, funded public gifting, or completion of P1–P7/Mentor. The expired
heartbeat stays paused. Test balances are real saved test-game currency; keep
them distinct from any future live economy. Live activation remains a separate
future request requiring an actual live key and matching live endpoint secret.

[Redacted receipts, exact executed QA sources and screenshot](generated/player-qa/stripe-test-20261011/README.md)
record the actual configuration and trial.
