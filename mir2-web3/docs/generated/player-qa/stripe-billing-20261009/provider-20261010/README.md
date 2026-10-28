# Real Stripe sandbox credential and request verification

The user requested verification of the key they filled in a private local
file and subsequently set USD1/100 Credits, monthly1000/30-day pricing.
This checkpoint follows source `893197f314b228390362f3f061e3ef29fa197fcb`.
Provider verification completed at `2026-10-09T15:46:08.6931094Z`; documentation
uses the current local date2026-10-10. No key or signing secret appears here.

| Actual provider check | Result |
| --- | --- |
| Checkout, PaymentIntent, Charge and webhook list | All four return HTTP200 |
| Original Checkout request with `payment_method_types[0]` | HTTP400; parameter no longer supported by pinned Endive API |
| Checkout with `allowed_payment_method_types[0]=card` | HTTP200; test-mode, unpaid, matching probe metadata |
| Expire that exact unpaid Checkout | HTTP200, status `expired` |
| Create temporary test webhook with pinned API version | HTTP200, test-mode, signing secret returned but never printed |
| Delete that exact temporary webhook | HTTP200, `deleted=true` |
| Gateway billing regressions after matching request repair |36 pass,0 fail,997 filtered; no tests ignored |

The Gateway captured-form test requires the supported card filter and rejects
all deprecated `payment_method_types` keys; frozen amounts, identity, API
version and idempotency assertions remain. `gateway01.log` is unedited local
stdout, including existing compiler warnings and intentional blank EOF lines.
`RECEIPT.json` binds the two source files and that log without secrets or
provider object IDs. Original provider rejection remains in the private
verification record and is summarized without credentials in the receipt.

The retained user-only file now supplies test credentials, USD1/5/10/20 offers
and monthly1000, with `MIR2_STRIPE_ENABLED=0`. Its webhook secret is still empty:
the temporary endpoint was only a provisioning check and was deleted. No
permanent callback is claimed. No private file is added to Git or installers.

This verifies authentication and provisioning, not completed payment, refund,
game-wallet settlement, browser rendering or public/native end-to-end behavior.
The sandbox probes create no charge, game order, subscription or monthly item.
No live provider setting, installed build, public service/feed or player save
is changed. New-source Linux CI has not been inferred from the earlier passing
code4a26 runs; paired fleet schema8 rollout and full P1–P7 remain open.

Official references: [Checkout create](https://docs.stripe.com/api/checkout/sessions/create),
[dynamic payment methods](https://docs.stripe.com/payments/payment-methods/dynamic-payment-methods),
[Checkout expire](https://docs.stripe.com/api/checkout/sessions/expire),
[webhook create](https://docs.stripe.com/api/webhook_endpoints/create),
[webhook delete](https://docs.stripe.com/api/webhook_endpoints/delete).
