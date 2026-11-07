# Main website and game entry points

The public marketing origin is `https://numeron.obelisk.build`. The existing
game and promotion-service origin remains `https://mir2.obelisk.build`:

- `/` and the 16 locale paths serve the marketing website in this directory.
- `/game` serves the browser game from `apps/web`.
- On the marketing origin, `/<locale>/promotions` is a localized preparation
  notice. `/promotions?lang=<locale>` redirects to that notice; unsupported or
  missing languages fall back to `zh-CN`.
- Existing nested `/promotions/...` links still forward to `apps/web`. Its
  promotion service and `/api/promotions` are unchanged.

The two applications share this repository but build separately. The existing
`numeron-legend-of-rebirth` Vercel project still serves the marketing application;
the existing `mir2-web3-web` project owns the canonical domain and forwards only
the marketing route allowlist. `/marketing-assets/_next/static` separates the
marketing bundles from the game's `/_next` bundles. Public marketing images are
served without the shared Next image optimizer.

Desktop navigation, mobile navigation, the footer and guide links use
`/<locale>/promotions` with a localized "in preparation" label. Language menus
retain this page when switching locales. The preparation notice has no signup,
wallet connection, commission collection or withdrawal controls. It explains
the planned single referral level, separate player/promoter roles, unsettled
eligibility and payout rules, and the third-party scope of wallet exchange
services. It does not enable or disable game, login, wallet or payment services.

`RECOVERY-MANIFEST.json` records the immutable Vercel deployment and hashes of
the recovered original source, before integration edits. It is provenance, not
a checksum list for the modified application.

The `Promotion Website Release` workflow provides `unified-context`,
`unified-stage` and `unified-promote`. Staging creates two production candidates
without assigning public aliases. Promotion requires both candidates to be
ready with the same exact commit and reviewed production baselines. Explicit
deployment root metadata, when present, must match; modern Git deployments may
omit it. Before either alias changes, the official Vercel CLI authenticates
with the existing CI identity to verify the actual candidate home, navigation,
footer, download link, game page, PWA manifest and game revision. The CLI uses
Vercel's automation bypass mechanism while deployment protection remains enabled.
An explicit `unified_source_sha` lets a corrected release tool promote the same
already reviewed candidates without rebuilding their unchanged applications. The
marketing alias is promoted first because unique deployment URLs are protected.
Public route and asset checks follow the game switch; a failure restores only
projects still pointing at this release's candidates. No project environment
variables, signing secrets or game databases are changed by this release path.

Marketing deployment overrides are `NEXT_PUBLIC_SITE_URL`,
`NEXT_PUBLIC_GAME_URL`, `GAME_HEALTH_URL` and `EXPLORER_ORIGIN`; other existing
project configuration, including download URLs, is inherited. Native standalone
builds set `MIR2_NEXT_STANDALONE=1` and retain the root game entry point without
the marketing rewrites. This integration does not publish Windows packages.

For local integration, run this app on port 3080 and set
`MIR2_MARKETING_ORIGIN=http://127.0.0.1:3080` when building or starting `apps/web`.
Production uses the public marketing alias by default. No Docker is required.
