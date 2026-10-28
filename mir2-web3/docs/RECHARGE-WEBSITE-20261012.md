# Independent recharge website — 2026-10-12

The user explicitly requested a separate website alongside the game, retaining
the previously selected Stripe TEST mode. It is implemented in
`apps/recharge-web` and actually served at
<https://165.154.65.136.sslip.io/recharge/>. It shares the invited Windows realm's
game accounts, character ownership and durable recharge ledger. No native
installer or game update is required for this website.

## Published scope

The lightweight page offers password login, real character selection, saved
Credits, pending Credits, monthly status/expiry, four server-authoritative USD
recharge offers and the last20 order states. It uses Stripe Hosted Checkout in
a separate tab. The monthly card remains the1000-Credit in-game item, with
parcel claim and ordinary item use; website recharge never adds time directly.
Mandatory monthly entry remains off.

All nine requested languages have complete dictionaries, including Arabic RTL.
Actual 390×844 browser checks report client width375 and scroll width375 for all
nine, with all input, payment-card and header bounds inside the viewport and
four disabled preview offers while logged out. The final desktop
Traditional-Chinese and actual phone Arabic viewport screenshots are retained. Geometry and
visible rendering are verified; this is not a claim of native game/laptop or
professional translation acceptance.

The browser holds its password-login bearer and session ID only in memory,
clears the password field, and revokes exactly its own grant on sign-out. It
stores only a language preference and an account-scoped unprivileged Checkout
nonce. An uncertain Checkout retains its nonce; another payment requires an
explicit new-order action. It renders server text without remote HTML and
accepts payment URLs only from the exact HTTPS Stripe Checkout host. Return URL
parameters cannot confirm payment. The server's existing signature checks,
frozen ownership/price metadata, rate limits and durable idempotency stay in
authority, following [Stripe fulfillment guidance](https://docs.stripe.com/checkout/fulfillment).

Every refresh checks the existing WSS active-character fence, even when no
pending balance is reported. An online character keeps its lease and receives
a clear saved-balance/pending-delivery notice. The site does not StartGame,
create characters, teleport, grant test currency or mutate another active save.

## Actual payment and game acceptance

One existing owned QA account starts with wallet0 and an already active monthly
card. Its authenticated USD1 offer creates one real TEST Checkout. Repeated
selection before payment returns the exact same Checkout URL. The browser
visibly shows Sandbox; the official4242 test card is submitted exactly once
after selecting USD. Read-only provider checks confirm this owned session is
complete/paid and its PaymentIntent succeeded for100 cents USD, with matching
order/account/character metadata and `livemode=false`. No real money moves in
[Stripe test mode](https://docs.stripe.com/testing).

The permanent provider callback and ordinary website status processing issue
exactly100 Credits, pending0, and an applied order. Browser logout/relogin
preserves100 Credits and the existing monthly expiry. Two ordinary public WSS
game login/StartGame/LogOut cycles each read100 Credits in UserInformation and
preserve the applied order and expiry. No callback replay or synthetic
settlement is used in this website trial.

The latest site is also tested while the same owned character is actually
playing through its normal WSS session. The online notice is visible. Website
sign-out returns200 with `accepted=true, affected=1`; the independent game
connection still completes a KeepAlive barrier and normal logout/save. Foreign
character status and Checkout each refuse503 `billingCharacterMissing` with no
wallet/order changes. Revoking another owned QA account's session returns
`accepted=false, affected=0`, and that account's grant remains valid. After
proper own-session revocation, the old bearer returns401.

Public probes verify all six static files plus the exact release manifest,
security headers and MIME types, canonical308, missing-file404, anonymous and
forged-bearer401, client-supplied amount/credit fields422, unsigned callback400,
and anonymous revocation401. Admin and other identity/monthly mutation neighbors
remain404. Twelve focused protocol/privacy/URL/idempotency/locale tests pass.

## Server changes and current native state

The final six public files total75,186 bytes. Release-manifest SHA256 is
`b78c2dd84a5e1323b04428986c3d20df9b60087208b4dd07f1c090fc4df73538`.
Only `/recharge/`, its canonical redirect, and three already authenticated HTTP
paths are added to Caddy: status, Checkout, and own-session revocation. Caddy
SHA256 becomes `b19f0c1911042fafe37fd0adc6e396a45c8b98f2f195d81b6969644cd061ad8a`.
All unrelated original route bytes are preserved. Root-private proxy backups
remain under `/var/backups/mir2-recharge-*`. No private Caddy content or provider
key is in public evidence or Git.

The initially observed Gateway already runs source
`75c5b2e1f65816e062d2032c9b941bb725152f7c`, PID3397094, binary
`22bee6075c24a5cf25d86f6fcf51a02d9f07c5d4208da2eed40d0072fa6be449`.
The website doesn't replace it or revert it to the older billing rollout.
Gateway, original realm PID3855184 and Caddy PID2107428 stay unchanged throughout
the guarded site activations. Gateway private configuration hash stays
`41d0939baa6f164a5db0605ef441647c4c8214a6b4ac1dd7c69ab1da65c647c0`.
The final static update is a symlink swap without a Caddy reload. PostgreSQL
remains SourceOfTruth and Redis remains healthy. After ordinary QA cleanup,
all six Gateway counters and TCP7200 are zero; NRestarts0.

Fresh final reads observe native R25/feed20, matching on origin and CDN. Its
manifest was created before the website deployment. Native manifest/CMS hashes
are respectively `a08a45735bffc7d6f1eaff9eb4edf4c210e1a27b278d7cb411f4afc20f141a9d`
and `0f88d31feb50b69970e43108a65245ef899eb7b72133d4ced04c342408acd9d3`.
The initial postflight wrongly reused older feed19 pins; that failure is
retained and excluded, and the operator was corrected to use explicit freshly
observed current pins. No native feed is written or rolled back by this task.

## Retained corrections and limits

The first browser run exposed a native-fetch receiver bug; an explicit wrapper
and a regression test fix it. The first bare-path redirect exposed Caddy's
relative-argument matcher ambiguity; explicit `*` fixes it to308. Initial
logout checks exposed that Crystal Disconnect is not identity revocation; the
site now calls the existing authenticated revocation endpoint. Failed provider
form-tool, before-proof schema, stale-feed and online-helper EOF/timeout
assumptions are retained and excluded. An Arabic full-page capture clipped the
page despite correct viewport rendering; the actual viewport screenshots and
explicit control-bound checks replace that excluded capture. In particular, a helper that observed
premature stdin EOF is not counted as the game-after-website proof. The
qualified run uses an explicit stop request, bounded draining and normal save.
No payment is resent to repair any of these attempts.

This round does not activate live Stripe, trial refunds/disputes, rebuild the
Windows client, change user saves or close broader P1–P7/Mentor/native human
acceptance. The expired heartbeat remains paused. The website source and
allowlisted receipts are committed in the root-owned publication branch;
current Gateway deployment is preserved as an existing dependency.

Evidence: [generated/player-qa/recharge-web-20261012/README.md](generated/player-qa/recharge-web-20261012/README.md).
