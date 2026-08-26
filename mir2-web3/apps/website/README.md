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
- `NEXT_PUBLIC_GAME_URL`: external URL opened by the play buttons. Defaults to
  `https://mir2.obelisk.build`.
- `NEXT_PUBLIC_EXPLORER_URL`: optional public Atlas URL used by navigation and
  homepage links. Use `{locale}` in a custom URL once localized Atlas routes
  are available. It currently defaults to the implemented same-origin
  `/zh-CN/explore` path.
- `NEXT_PUBLIC_LIVE_STREAM_URL`: optional public live-channel URL. When set,
  the header exposes a `LIVE` indicator and the Watch page links to the real
  stream; without it, the page is explicitly marked as a demo feed.
- `NEXT_PUBLIC_TOKEN_CHECKOUT_URL`: optional external checkout origin for paid
  Token plans. Without it, purchase controls remain disabled and prices are
  explicitly presented as a product prototype.
- `EXPLORER_ORIGIN`: private origin used by the website rewrite to serve the
  standalone `apps/explorer` application. Defaults to `http://127.0.0.1:3090`.

Supported locale routes are `zh-CN`, `en`, `ja`, and `ko`. Locale copy lives in
`lib/site-copy.ts`; regional campaign artwork should remain separate from the
shared base site so it can be versioned and rolled back independently.

The public Watch prototype is available at `/{locale}/watch`; the AI service
membership prototype is available at `/{locale}/membership`. Atlas is served
through the same website origin at `/zh-CN/explore` and rewritten to the
standalone `apps/explorer` application.
