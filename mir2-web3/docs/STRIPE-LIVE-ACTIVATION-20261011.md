# Stripe live activation — 2026-10-11

> Superseded by the user’s later instruction to use TEST mode. Public test
> activation and the paid sandbox/monthly trial now pass; live money remains
> off. This earlier live-readiness record is historical.
> [Current configuration and acceptance](STRIPE-TEST-ACTIVATION-20261011.md).

**Live Stripe payments are not enabled yet.** The operator explicitly
requested activation and filled the new private file, but its value is still
a test-mode server key. The verifier rejects it before contacting Stripe;
no live credentials, provider objects or payment orders have been installed.

## Actual completed work

The public callback
`https://165.154.65.136.sslip.io/playtest/v1/billing/stripe/webhook`
is now routed to the invited Gateway. The sole Caddy change appends that exact
path to the existing allowlist. The original parser validates the candidate
with the running Caddy environment; the values stay only in memory. Its
private root-only backup is
`/var/backups/mir2-caddy-stripe-live-20261011-01`.

Real verified HTTPS requests reject unsigned callbacks with400 and
`billingWebhookSignatureInvalid`. A forged signature receives503
`billingUnavailable` while the provider is disabled. Both responses are
`no-store`. Checkout/status HTTP paths and admin neighbors remain404; native
recharge already uses authenticated WebSocket operations. The styled return
page remains byte identical and cannot award currency.

All service PIDs, the running Gateway binary, base environment, unit files,
original realm, public signed feed19 and detached signature are preserved.
No Gateway restart, database mutation, installation or secret upload occurs.
Caddy changes from SHA256
`139d2f48a77e2af9f1ff0b0baec81aef47c6d084c1ce95f6df32c23889dda5d1`
to `efbd0eaee9952dd38148c9b634f47889e99a9ddff632c5f2befebb6ce8339b28`.

The fresh server read observes Gateway4006d988b3922d0106278720abfa9b3d384c8d5e,
PID3347791, binary84778528 bytes/SHA256
`fc530898d8cbf563389fe708ac3a279846d28b56ae97e340af3ed7d100ad8202`.
It already includes the R24 billing implementation: provider, WebSocket/HTTP,
recharge ledger and monthly-card files are byte identical to01a8c48a.
The unrelated movement integration is preserved; this round does not deploy
or independently qualify that integration's full gameplay scope.

## Credential handoff and remaining activation

The operator-only file outside Git is
`C:/Users/Administrator/.mir2-private/stripe-live.env`. Fill
`MIR2_STRIPE_SECRET_KEY` with a real live server key obtained from the
[Stripe live API keys page](https://dashboard.stripe.com/apikeys). Merely
renaming a test key's prefix does not create a live key. The file ACL permits
the current user and SYSTEM. Credential values never appear in receipts.
The private verifier and reusable [readiness script](../scripts/verify-stripe-live-readiness.py) reject test keys, then read the account,
Checkout, PaymentIntent, Charge and webhook endpoints without modifying them.
It requires live card-payment capability and `charges_enabled` before ready.
[Stripe's actual switch to live mode](https://docs.stripe.com/keys#switch-to-live-mode)
requires live credentials, with a matching endpoint signing secret.

After the correct key passes, provision the exact live callback with pinned
API version2026-09-30.endive and only the handled Checkout/refund/dispute
subscriptions. Keep its returned signing secret in private server configuration.
Do not copy the local Stripe CLI signing secret. Preserve the approved
USD1/5/10/20 offers for100/500/1000/2000 Credits, and1000 Credits for a30-day
consumable. Account-wide monthly entry remains independently disabled.

Enable only against the existing PostgreSQL SourceOfTruth and compatible
writers. Re-read all six counters, TCP7200 and spectator viewers, then normal
stop, take a fresh private database/state/config backup, install the bounded
billing environment and start the same Gateway revision. Never force users
out or restore old saves. Confirm ordinary native billing status/offer amounts,
valid live Hosted Checkout, unsigned/test-mode/replay refusal and persistent
idempotent settlement. A real funded payment is separate from creating an
unpaid live Checkout; no actual live charge is claimed by this record.
Refund/dispute Review records retain their existing operator-handled semantics;
this change does not add automatic refunds or delete spent currency/items.

## Evidence and limits

[Exact public receipts and script](generated/player-qa/stripe-live-20261011/README.md)
include the actual sole route apply, independent public probes, source file
comparison, test-key rejection and hashes. Earlier local preparatory errors
are retained and excluded. No new gameplay test, native-human acceptance,
refund/dispute end-to-end acceptance or full P1-P7/Mentor completion is claimed.
The expired heartbeat stays paused. Existing R24 Launcher needs no reinstall
for this server-only configuration change.
