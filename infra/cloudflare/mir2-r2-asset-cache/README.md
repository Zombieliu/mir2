# Mir2 R2 Asset Cache Worker

Serves `assets.mir2.obelisk.build/*` from the `mir2-web3-assets` R2 bucket with
Cloudflare Worker Cache API in front of R2.

The URL path maps directly to the R2 object key. For example:

```text
https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c/original-ui/Monster/003/52.png
```

serves:

```text
mir2/v/37596e16d64fde7c/original-ui/Monster/003/52.png
```

The Worker keeps immutable game assets cached at the edge, supports CORS for
the Player Web Service Worker, stamps CORS headers on both fresh R2 responses
and Cache API hits, and bypasses the cache for byte-range requests.

External origins (local development, Vercel previews, itch.io, CrazyGames, and
other distribution channels) must use the browser-safe alias:

```text
https://assets.mir2.obelisk.build/hotlink-ok/mir2/v/<release>/<asset path>
```

Cloudflare reserves a `hotlink-ok` path segment as a selective Hotlink
Protection bypass. The Worker strips only that leading segment and maps the
remaining path to the same R2 object and Cache API key as the canonical URL.
The public `r2.dev` bucket URL remains the final browser fallback if the custom
asset Worker is unavailable.
