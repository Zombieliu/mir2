# Local Stripe sandbox preparation — 2026-10-10

The user's "how do I try it?" follow-up is **not yet ready for payment**.
The previous provider credential evidence remains valid, but the currently
configured private `MIR2_STRIPE_SECRET_KEY` entry is empty. The operator was
asked to refill the same private file, never send the key in conversation.
No test account, Checkout payment, wallet award or monthly activation is claimed.

## Implemented and checked

- An explicit `MIR2_STRIPE_ALLOW_LOOPBACK_TEST_RETURN=1` option permits an HTTP
  cosmetic return URL only with a test key and literal `127.0.0.1` or `[::1]`.
  Default and live-key behavior still require HTTPS. Numeric aliases, DNS names,
  LAN/public hosts, credentials, query/fragment and malformed flags fail closed.
- Gateway billing **38/38** passes, including the two new configuration guards.
  Existing authenticated webhook, immutable tuple, Source settlement, replay,
  expired selection and private RPC assertions remain in the same run.
- The Gateway builds on Rust1.95.0. A compatible native binary from unchanged
  fd660 code is prepared with an executable-adjacent loopback configuration,
  independent profile and read-only installed assets. Neither binary is launched.
- Exact parent fd660 Stripe CI **37957284779** succeeds, verified through the
  authenticated GitHub API. It does **not** cover the new loopback guard.
- A read-only security review found no blocking guard issue. It did not execute
  tests, start services or access the key.

## Transport and current blocker

Automatic tool approval rejected the proposed Cloudflare Quick Tunnel startup
with `CreateProcess … rejected: blocked by policy`, without a detailed reason.
No tunnel/relay started and that action was not retried through another tool.
The safer planned transport is official Stripe CLI **1.53.1** outbound event
forwarding to `http://127.0.0.1:7121/v1/billing/stripe/webhook`, without publishing
local services. Its official Windows archive SHA256 was verified as
`fa159ae2774d62dcfd0db8f7f8675e3274223ae3e92fae4d182f4c9cfb58fa23`.
The listener helper refused to start because the private key entry is empty.
It produced no provider request, process or signing secret.

Prepared helpers and Chinese player instructions stay outside Git. They use
USD1/5/10/20 =100/500/1000/2000 Credits, monthly1000/30 days, separate file
account/recovery state and clean child environments without inherited databases,
autologin or QA privileges. Gateway TCP7120 and Web7121 remain unbound.
Monthly access is required only in this planned sandbox so selection renewal
can be exercised before ordinary world entry.

## Remaining acceptance

Refill the test key, start the official listener with pinned Endive event
version, then start the isolated Gateway. A genuine ordinary player must create
the USD10 order, complete Hosted Checkout with a Stripe test card, observe the
actual signed callback/readback and1000 Credits, buy/activate the exact card UID,
check30 days and repeat/normal logout/relogin. No simulated webhook or browser
return substitutes for these gates. Native UI/human acceptance, refund,
production callback/live credentials and all-writer paired rollout remain open.

Public R22/Gatewaya41/feed17, installations/saves and unfinished P1–P7 are
unchanged. [Exact hashes, logs and limits](RECEIPT.json) bind this preparation.
`gateway38.log` and `gateway-build.log` preserve original tool output bytes;
their terminal blank lines are intentional raw-evidence whitespace exceptions.
