# mir2 Player Web

`apps/web` is the Next.js host for the browser client. It connects to the Rust
Gateway, projects server packets into the Bevy WASM runtime, and provides the
login, character selection, HUD, audio, asset-cache, and diagnostics surfaces.

Use the repository-level PowerShell scripts for normal Windows development.
The lower-level `npm` commands in this document are diagnostic escape hatches,
not a second onboarding path.

## Supported Windows Start

Run from `E:\mir2\mir2-web3` (or the equivalent clone directory):

```powershell
.\scripts\bootstrap-developer.ps1
.\scripts\start-developer.ps1 -OpenBrowser
```

The standard endpoints are:

| Service | Address |
| --- | --- |
| Player Web | `http://127.0.0.1:3002/` |
| Gateway WebSocket | `ws://127.0.0.1:7110/ws` |
| Gateway health | `http://127.0.0.1:7110/health` |
| Crystal TCP | `127.0.0.1:7000` |

`start-developer.ps1` builds and starts the Gateway, waits for its health
endpoint, selects the tracked prebuilt Bevy runtime, injects the matching
WebSocket URL, and then runs the Web dev server. Press `Ctrl+C` in that terminal
to stop the Web server and the Gateway process started by the script.

For the complete setup and troubleshooting flow, see:

- [Windows local development](../../docs/LOCAL-DEVELOPMENT-WINDOWS.md)
- [Developer handoff](../../docs/DEVELOPER-HANDOFF.md)
- [Asset consumer setup](../../docs/ASSET-CONSUMER-SETUP.md)

## Login Flow

For a clean local data directory:

1. Enter a unique account name and password.
2. Choose `New Account`.
3. Choose `Login` with the same credentials.
4. Create a character.
5. Choose `Start Game`.

Account creation does not log the account in automatically. Reusing an
existing account can surface the protocol message `creation disabled`; use a
new account name or log in with the existing password.

## Mobile PWA And Fullscreen

The hosted player is installable as a Progressive Web App. Installation is the
supported way to remove mobile browser address and navigation bars; a normal
iPhone browser tab cannot reliably hide them with CSS or JavaScript.

- iPhone/iPad: open the player, tap Share, choose `Add to Home Screen`, then
  launch `Mir 2` from the new Home Screen icon.
- Android Chrome/Samsung Internet: choose `Install game` in the player. If the
  system prompt is unavailable, use the browser menu and choose `Install app`.
- Android browser fallback: the `Full screen` action uses the Fullscreen API
  from a user gesture and requests landscape orientation when the browser
  permits it.

The PWA manifest requests fullscreen landscape presentation and falls back to
the platform's standalone app mode. `viewport-fit=cover`, dynamic viewport
height, and safe-area insets keep the game and touch controls inside notches,
rounded corners, and gesture areas.

iOS treats an installed Home Screen web app as a separate storage context.
Players may need to log in once after installation; do not assume a browser-tab
wallet or login session is copied into the installed game. Always regression
test Passkey and wallet return flows from the installed icon as well as from a
normal browser tab.

## Asset Modes

Do not treat "repository assets", "private developer bundle", and "R2 CDN" as
the same distribution.

| Mode | What it uses | Intended use |
| --- | --- | --- |
| Starter | Git-tracked UI/map PNGs, starter Crystal pack, and prebuilt Bevy WASM | Clone-and-run onboarding and ordinary gameplay work |
| Private GitHub bundle | A checksummed private Release installed into `public/generated/crystal-packs/full` | Full offline parity development |
| R2 CDN | An immutable, versioned remote release selected by an asset base URL | Hosted acceptance, cache, and low-storage testing |

### Starter

No asset environment variable is required:

```powershell
.\scripts\start-developer.ps1 -OpenBrowser
```

The repository contains real Starter PNGs. It is not metadata-only. A request
for `/generated/crystal-packs/full/index.json` may return 404 when no private
full pack is installed; the client then falls back to Starter assets. Append
`?crystalFullPack=0` when intentionally testing only that fallback.

### Private GitHub Developer Bundle

The repository pins the approved private Release in
`config/developer-assets.json`. After authenticating an account that can read
the private repository, let the installer download and verify every split part:

```powershell
gh auth login
.\scripts\install-developer-assets.ps1 -Download
.\scripts\start-developer.ps1 -OpenBrowser
```

The installer validates part sizes and SHA-256 values, reconstructs the
archive, validates the final archive, and extracts it to:

```text
apps/web/public/generated/crystal-packs/full
```

That directory is intentionally ignored by Git. Once installed, leave
`NEXT_PUBLIC_MIR2_ASSET_BASE_URL` empty so the Web app uses the local pack.

Maintainers create the private payload with:

```powershell
.\scripts\package-developer-assets.ps1
```

The packaging and release procedure is documented in
[Asset consumer setup](../../docs/ASSET-CONSUMER-SETUP.md).

### R2 CDN

Use the immutable release root supplied by the maintainer. Do not copy an old
hash from documentation:

```powershell
$AssetBaseUrl = "https://assets.example.com/mir2/v/<version>"
.\scripts\verify-developer-setup.ps1 -AssetBaseUrl $AssetBaseUrl -SkipBuild
.\scripts\start-developer.ps1 -AssetBaseUrl $AssetBaseUrl -OpenBrowser
```

The release root must expose the same public path layout as the Web app,
including `original-ui/`, `original-map/`,
`generated/original-map-blend/`, and
`generated/crystal-packs/full/index.json`. The Bevy JS/WASM pair remains
same-origin because both files must come from the same build.

## Environment Variables

Copy `.env.example` to `.env.local` only when a persistent override is useful.
The normal start script injects local values without requiring this file.

| Variable | Purpose |
| --- | --- |
| `NEXT_PUBLIC_MIR2_GATEWAY_WS_URL` | Browser-visible Gateway WebSocket override |
| `NEXT_PUBLIC_MIR2_ASSET_BASE_URL` | Browser-visible immutable R2/CDN release root |
| `MIR2_ASSET_BASE_URL` | Server-side alias for the asset release root |
| `MIR2_R2_PROXY_BASE` | Optional same-origin development proxy target |
| `MIR2_PASSKEY_AUTH_SECRET` | Production/staging token secret; must match Gateway |

Do not put credentials, GitHub tokens, R2 access keys, or upload secrets in a
tracked env file.

The repository-local `scripts/start-developer.ps1` and `scripts/dev.ps1 up`
commands automatically opt both Web and Gateway into the insecure development
Passkey secret. Production and staging still require an explicit matching
`MIR2_PASSKEY_AUTH_SECRET` on both services.

## Verification

Run the repository verifier after onboarding or before handoff:

```powershell
.\scripts\verify-developer-setup.ps1
```

For a faster iteration that skips the production Web build:

```powershell
.\scripts\verify-developer-setup.ps1 -SkipBuild
```

The verifier checks the Crystal submodule handoff branch, tracked Starter and
prebuilt runtime files, Gateway compilation, asset-release safety tests,
TypeScript, and (unless skipped) a production Web build. With
`-AssetBaseUrl` it also probes the remote full-pack index.

For the focused PWA contract and icon checks:

```powershell
npm run test:pwa
```

## Manual Web Start

Use this only when the Gateway is already running and the standard script is
not suitable for a focused Web diagnosis:

```powershell
$env:NEXT_PUBLIC_MIR2_GATEWAY_WS_URL = "ws://127.0.0.1:7110/ws"
npm ci
npm run dev -- --hostname 127.0.0.1 --port 3002
```

The supported package manager for clean onboarding is `npm` with the committed
`package-lock.json`. Node.js 22 or newer is required.

## Common Diagnostics

- Gateway unavailable: open `http://127.0.0.1:7110/health` and inspect
  `.mir2-data/developer-logs/gateway.out.log` and `gateway.err.log`.
- Wrong Gateway port: use `7110` for the standard WebSocket endpoint; historical
  QA commands may use a different port.
- Stale browser assets: test with `?assetCache=0` or run
  `window.__mir2AssetCacheReset({ reload: false })` in DevTools.
- Missing local full index: this is expected in Starter mode; reinstall the
  private bundle only when full offline coverage is required.
- Missing remote full index: verify the immutable R2 root with
  `verify-developer-setup.ps1 -AssetBaseUrl <url> -SkipBuild`.
- First playable frame: a clean start can take roughly 35-60 seconds while the
  Gateway, Atlas, WASM, and initial scene assets warm up.

For cache debugging, `?cacheLog=1` enables structured console events and
`?cacheDebug=1` adds the QA overlay. The service worker is normally disabled
in local development unless an asset base URL is supplied or `?assetCache=1`
is used.
