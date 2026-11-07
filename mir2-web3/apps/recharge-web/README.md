# Numeron activities, player and recharge website

Public site: <https://165.154.65.136.sslip.io/recharge/>.

This independent, buildless website lives alongside `apps/web` in the same
repository. It uses the invited Windows realm's existing game accounts,
characters, enabled payment checkouts and durable recharge ledger. It doesn't create
another account database or collect card details. The existing game website
link is a separate browser client; log in here with your invited Windows game
account and select the character that should receive Credits.

The original deployment documented here uses **Stripe TEST**. The portal now
also supports an optional **WAFFO Merchant** channel. The authenticated status
lists enabled channels and each channel's actual test/live mode and offers;
players choose a channel before choosing an amount. An absent WAFFO channel is
not advertised or used automatically. This source change does not enable or
deploy a merchant account. The original Stripe approved
offers are USD1/5/10/20 for100/500/1000/2000 Credits. The30-day monthly item costs
1000 Credits in the in-game shop; claim its parcel and use the item to activate
it. Website payment never activates monthly time directly. Mandatory monthly
entry is off.

The activity center at `#activities` reserves closed/beta level rewards for
public-launch delivery. The player center at `#player` shows server-owned
characters, the saved wallet/monthly state, registered rights and verified beta
payment records. Reservations do not immediately issue mail, title or outfit.
The recharge center remains at `#recharge`. See
`docs/ACTIVITIES-PLAYER-PORTAL-20261012.md` for remaining delivery work and policy.

The portal supports Traditional Chinese, English, Brazilian Portuguese,
Russian, Hindi, Indonesian, Vietnamese, Thai and Arabic. It works on desktop
and phone; Arabic uses RTL layout. Recharge copy is in `public/locales.js`;
activity/player copy is in `public/activities.js`.

## Player flow

1. Log in with the game account and password, and select the receiving character.
2. Choose an enabled payment channel and an amount. The authenticated server
   supplies that channel's actual offer and quote.
3. Open the selected channel's checkout in a new tab. Use the currency shown in
   the order.
4. Keep the portal open and refresh after payment. A verified server callback
   records payment; the server issues pending Credits, and the portal displays
   the actual order state. Returning with `?paid` doesn't award anything.

When a character is playing, the website reports its saved wallet and explains
that the game handles pending Credits. It never steals the active character's
lease or interrupts play. Website sign-out revokes only the website grant.

## Payment channel contract

`paymentProviders` in billing status contains `{id, label, livemode, offers}`
for enabled channels. Supported IDs are `stripe` and `waffo`. The old
`checkoutEnabled` / `offers` fields can still be interpreted as the original
Stripe test catalogue when `paymentProviders` is absent. An explicit empty
provider list disables new checkouts. This portal requires the matching updated
server: status queries and WebSocket status requests send
`includePaymentProviders=true`, and checkout bodies send
`includePaymentProviders: true` plus the selected `provider`. The server keeps
its original response schema for shipped clients that do not opt in, including
clients whose parsers reject unknown response fields.

New checkout requests include the selected `provider`, and checkout responses
must echo that provider. Recent orders display their stored provider. The
session retry journal isolates account, character, provider and offer; existing
Stripe journal IDs are retained during upgrade. Once an attempt starts, its
channel and offer stay fixed through an uncertain result and retry. The player
must explicitly choose **Start another recharge** to create a separate intent.

Checkout URLs are validated for the selected channel: Stripe keeps its exact
`https://checkout.stripe.com/c/pay/` rule, and WAFFO uses only
`https://cashier.waffo.com/` with a nonempty order path. Credentials, other hosts,
other schemes and nonstandard ports are rejected. This follows the official
[WAFFO hosted Checkout integration](https://waffo.com/docs/en/developer-docs/integration/checkout/steps).
Other sandbox checkout hosts require a separately verified allowlist change.

## Development and tests

Run from the `mir2-web3` repository root:

```sh
node --test apps/recharge-web/tests/*.test.mjs
python -B -m unittest discover -s apps/recharge-web/tests -p test_deploy.py -v
node apps/recharge-web/scripts/preview.mjs
```

Preview is at `http://127.0.0.1:7347/`. It is a local visual preview; full
authentication/payment acceptance runs against the deployed site's same-origin
API routes. No dependency install, game assets or game/client compilation is
needed. Package into a new absolute output directory:

```sh
node apps/recharge-web/scripts/package.mjs /absolute/new/output-directory
```

`scripts/acceptance.py` verifies an already paid, operator-owned USD1 sandbox
browser trial, foreign-character rejection, sign-out, and two ordinary game
login/save cycles. It never creates a payment, replays a callback, provisions a
player or edits a save. Private cohort/key/browser-proof files are supplied as
arguments outside Git. `scripts/postflight.py` checks an existing trusted SSH
lifecycle reader and explicitly pinned current native manifests. Both scripts
have bounded network reads and redact identifiers. See their `--help` output.

## Deployment

The public files are served from an immutable release below
`/srv/mir2-recharge/releases/`; `current` switches atomically. `release.json`
records their exact sizes and hashes. The currently qualified release is
`b78c2dd84a5e1323b04428986c3d20df9b60087208b4dd07f1c090fc4df73538`.

Upload the public payload, `release.json`, `server/caddy.snippet` and
`server/deploy.py` to a new `/home/ubuntu/numeron-recharge-*` directory. Invoke
the operator without `--apply` first, with freshly read expected Caddy bytes,
current site hash, Gateway revision and service PID. Validate that result, then
invoke the identical arguments plus `--apply`. Later static updates use
`--expected-site-sha256`; Caddy isn't reloaded when its bytes remain identical.
`--enable-logout-route` is only the guarded migration from the initial two-route
portal to the existing authenticated self-session revocation endpoint.
`--enable-waffo-webhook` is a separate, explicit migration for an existing
activity-enabled site: it adds only the signed `/v1/billing/waffo/webhook` public
callback. Ordinary updates preserve its exact existing matcher. Merchant setup,
paired service versions and local evidence are in
[WAFFO recharge](../../docs/WAFFO-RECHARGE-20261105.zh-CN.md).
`repair_redirect.py` is the historical guarded repair for the initial relative
redirect ambiguity; it isn't part of routine updates.

The operator publishes the exact manifest and public-file buffers verified
before Caddy validation. Replacing or removing staging files after verification
cannot change the release. Incoming Windows CRLF snippets are normalized to LF;
the existing Caddy bytes, singleton checks, exact route changes and rollback
witnesses remain intact. The offline Python suite exercises these boundaries
without starting a service or contacting a server. See
`docs/RECHARGE-DEPLOYMENT-REGRESSION-20261014.md` for the qualified code scope;
these tests do not attest a new production rollout.

The proxy exposes exactly the existing bearer-protected status, Checkout and
session-revocation endpoints, plus the previously public signed callback and
cosmetic return page. It doesn't expose admin, credential-binding, session-list,
revoke-others or monthly-item mutation endpoints. Secrets stay in the existing
Gateway private configuration. Website deploys do not rewrite that file, restart
Gateway, publish a native update or require a new Windows installer.

Public proof and retained failures: [RECHARGE-WEBSITE-20261012.md](../../docs/RECHARGE-WEBSITE-20261012.md).
