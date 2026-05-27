# Mir2 Domain Proxy

Cloudflare Worker route for `mir2.obelisk.build`.

The Worker keeps the public domain on Cloudflare while forwarding traffic to the
current Vercel player-web deployment. It also routes `mir2.obelisk.build/ws*`
to the UCloud Gateway origin so the browser can use the same Cloudflare TLS
certificate through `wss://mir2.obelisk.build/ws`.

Heavy Crystal assets are served same-origin from the `mir2-web3-assets` R2
bucket. The Worker maps:

```text
/original-map/*
/original-ui/*
/generated/original-map-blend/*
```

to:

```text
mir2/v/$MIR2_ASSET_VERSION/<same path without leading slash>
```

Set `MIR2_ASSET_VERSION` and `MIR2_ASSET_OBJECT_PREFIX` to the same release
version used by `/api/asset-manifest`, the R2 upload, and the Vercel build.
Missing R2 objects return a JSON 404 with the exact object key, so Bevy and DOM
asset failures are diagnosable from the browser network panel.

Deploy:

```bash
MIR2_ASSET_VERSION=<github-sha-short> \
CLOUDFLARE_API_TOKEN=... \
npx wrangler deploy
```

The Vercel preview deployment is protected by SSO, so the Worker expects a
Cloudflare secret named `VERCEL_BYPASS_SECRET`. Generate it in the Vercel
project protection settings, then store it with:

```bash
printf '%s' "$VERCEL_BYPASS_SECRET" | npx wrangler secret put VERCEL_BYPASS_SECRET
```

Current origin:

```text
https://mir2-web3-web.vercel.app
```

Current Gateway origin:

```text
https://165.154.65.136.sslip.io
```

For a permanent named origin, add a Cloudflare DNS record:

```text
A gateway.obelisk.build 165.154.65.136
```

Start as DNS-only until Caddy has issued the origin certificate, then switch to
proxied if Cloudflare edge protection is desired. After that, update
`GATEWAY_ORIGIN_URL` to `https://gateway.obelisk.build`.
