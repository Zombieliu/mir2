# Mir2 telemetry edge

Cloudflare Worker source for `https://telemetry.obelisk.build`. It proxies the
Vercel Admin Web origin without caching private responses, rewrites redirects
back to the public hostname, and normalizes forwarded Server Action origins.

Deploy from this directory with an authenticated Wrangler installation:

```bash
npx wrangler deploy
```

The Worker contains no telemetry or dashboard secrets. Those remain encrypted
Vercel runtime variables; the browser only receives the authenticated console
snapshot.
