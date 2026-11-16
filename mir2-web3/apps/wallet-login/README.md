# Isolated playtest wallet login service

This route-only Next application imports the existing, tested Player Web wallet
verification and channel exchange handlers. It has no game page or public media.
Unknown paths return 404. `/health` exposes only readiness and the build's full
source revision; it does not prove a wallet approval or Gateway connectivity.

Use Node 22 and run `npm ci --no-audit --no-fund` in this directory. From the
`mir2-web3` project root, a clean committed checkout can be built and packaged:

```sh
node scripts/package-wallet-login-release.mjs --revision "$(git rev-parse HEAD)"
```

The package contains `RELEASE.json` and the Next standalone tree. Its metadata
identifies the relative server entry. Set `HOSTNAME=127.0.0.1` and `PORT` when
starting that entry with Node. The operating system service supplies private
environment variables at runtime; no `.env` files or credentials are packaged.

Configure `MIR2_ENV=staging`, `MIR2_GATEWAY_HTTP_URL=http://127.0.0.1:7210`,
`MIR2_PASSKEY_ALLOWED_ORIGINS=<the dedicated HTTPS login origin>`, and the
**existing test Gateway** `MIR2_PASSKEY_AUTH_SECRET`. Do not rotate identity or
recovery keys or use another realm's secret. The configured origin serves these
exact POST paths:

- `/api/wallet/dubhe/session`
- `/api/wallet/dubhe/session/status`
- `/api/channels/session/exchange`

Relay, wallet and network use the shared handler's defaults unless runtime
`MIR2_DUBHE_CONNECT_RELAY_URL`, `MIR2_DUBHE_WALLET_ORIGIN` and
`MIR2_DUBHE_CONNECT_NETWORK` are provided. Native clients require the supported
Dubhe Wallet origin and an explicit `server.auth_http_url` with no path suffix.

The package command refuses dirty tracked inputs, requires its own source and
lockfile to be tracked, and embeds the full checked Git revision. Archive SHA-256
and per-file hashes are computed from the resulting bytes, not accepted from
environment or adjacent files. Building requires no Docker or remote database.
