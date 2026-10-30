# NUMERON: Legend of Rebirth official website

Standalone Next.js App Router site for the public-facing homepage of
**Legend of Rebirth / 传奇重生**, a NUMERON series title. The
game client remains in `apps/web`; this application has its own build and
deployment lifecycle.

## Local development

```bash
npm install
npm run dev
```

The site is served at `http://localhost:3080` and redirects to `/zh-CN`.

## Environment

- `NEXT_PUBLIC_SITE_URL`: canonical website origin used by metadata, robots,
  and sitemap generation.
- `NEXT_PUBLIC_GAME_URL`: game client opened only after the localized launch
  and safety page. Defaults to `https://mir2.obelisk.build`.
- `NEXT_PUBLIC_WINDOWS_DOWNLOAD_URL`: optional HTTPS URL for the signed Windows
  installer. Without it, the localized Download page truthfully keeps Windows
  unavailable instead of linking a development artifact.
- `NEXT_PUBLIC_ANDROID_DOWNLOAD_URL`: optional HTTPS URL for a signed Android
  package or store listing. Without it, Android points to the supported PWA
  installation path and identifies the Capacitor APK as Candidate-only.
- `GAME_HEALTH_URL`: optional server-side health endpoint used by the public
  launch and status pages. Defaults to `${NEXT_PUBLIC_GAME_URL}/health`.
- `NEXT_PUBLIC_EXPLORER_URL`: optional public Atlas URL used by navigation and
  homepage links. Use `{locale}` in a custom URL once localized Atlas routes
  are available. It currently defaults to the implemented same-origin
  `/zh-CN/explore` path.
- `NEXT_PUBLIC_LIVE_STREAM_URL`: optional public live-channel URL. When set,
  the header exposes a `LIVE` indicator and the Watch page links to the real
  stream; without it, the page is explicitly marked as a demo feed.
- `NEXT_PUBLIC_LIVE_EMBED_URL`: optional provider embed URL (for example a
  YouTube embed URL) rendered directly inside the Watch page.
- `NEXT_PUBLIC_TOKEN_CHECKOUT_URL`: optional external checkout origin for paid
  Token plans. Without it, paid plans route to the non-billing Founder Access
  registration flow and prices remain explicitly marked as prototype prices.
- `FOUNDER_LEAD_WEBHOOK_URL` and `FOUNDER_LEAD_WEBHOOK_TOKEN`: optional private
  lead sink. Without a webhook, the form returns an explicit preview result and
  stores no personal data.
- `ATTRIBUTION_WEBHOOK_URL` and `ATTRIBUTION_WEBHOOK_TOKEN`: optional private
  sink for consent-free first-party campaign and CTA events. Without it,
  attribution remains in the visitor's browser and server events are discarded.
- `NEXT_PUBLIC_SUPPORT_EMAIL`: public support mailbox.
- `NEXT_PUBLIC_DISCORD_URL`, `NEXT_PUBLIC_YOUTUBE_URL`, and
  `NEXT_PUBLIC_BILIBILI_URL`: verified community links. Unconfigured channels
  are labeled unavailable rather than linked to placeholders.
- `EXPLORER_ORIGIN`: private origin used by the website rewrite to serve the
  standalone `apps/explorer` application. Defaults to `http://127.0.0.1:3090`.

Supported locale routes are `zh-CN`, `zh-TW`, `en`, `ja`, `ko`, `es`, `pt-BR`,
`de`, `fr`, `ru`, `pl`, `tr`, `th`, `vi`, `id`, and `ar`. Arabic renders RTL.
Core copy lives in `lib/site-copy.ts`; additional static market packs live in
`lib/market-copy.ts`. Regional campaign artwork should remain separate from the
shared base site so it can be versioned and rolled back independently.

The public Watch prototype is available at `/{locale}/watch`; the AI service
membership prototype is available at `/{locale}/membership`. Atlas is served
through the same website origin at `/zh-CN/explore` and rewritten to the
standalone `apps/explorer` application.
