# Main website and game entry points

The canonical public origin is `https://mir2.obelisk.build`:

- `/` and the 16 locale paths serve the marketing website in this directory.
- `/game` serves the browser game from `apps/web`.
- `/promotions` and `/api/promotions` stay in `apps/web` on their existing origin.

The two applications share this repository but build separately. The existing
`numeron-legend-of-rebirth` Vercel project still serves the marketing application;
the existing `mir2-web3-web` project owns the canonical domain and forwards only
the marketing route allowlist. `/marketing-assets/_next/static` separates the
marketing bundles from the game's `/_next` bundles. Public marketing images are
served without the shared Next image optimizer.

Desktop navigation, mobile navigation and the footer use a normal link to
`/promotions?lang=<locale>`. Crossing application boundaries requires a full
navigation so each application loads its own root layout. The promotion page
maps the requested language to its supported languages and preserves campaign
selection when its language changes.

`RECOVERY-MANIFEST.json` records the immutable Vercel deployment and hashes of
the recovered original source, before integration edits. It is provenance, not
a checksum list for the modified application.

The `Promotion Website Release` workflow provides `unified-context`,
`unified-stage` and `unified-promote`. Staging creates two production candidates
without assigning public aliases. Promotion requires both candidates to be
ready with the same exact commit, roots and reviewed production baselines. The
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
