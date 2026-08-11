#!/usr/bin/env node
// Build the Capacitor `www` directory for the mobile shell.
//
// The game client is a Next.js app served from a deployed origin (Vercel/R2);
// a bare Node standalone server cannot run inside a mobile WebView. This shell
// ships a loader page that boots the full-screen Capacitor WebView pointed at
// the configured game URL (remote deploy by default).
//
// Environment:
//   MIR2_MOBILE_GAME_URL  the game origin the WebView loads (default production)
//   MIR2_GATEWAY_WS_URL   the authoritative gateway WebSocket URL

import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const mobileRoot = path.resolve(scriptDir, "..");
const wwwRoot = path.join(mobileRoot, "www");

export const PRODUCTION_GAME_URL = "https://mir2.obelisk.build";
export const PRODUCTION_GATEWAY_WS_URL = "wss://165.154.65.136.sslip.io/ws";

export function resolveMobileConfig(environment = process.env) {
  const gameUrl = validateSecureUrl(
    environment.MIR2_MOBILE_GAME_URL ?? PRODUCTION_GAME_URL,
    "MIR2_MOBILE_GAME_URL",
    new Set(["https:"]),
  );
  const gatewayWs = validateSecureUrl(
    environment.MIR2_GATEWAY_WS_URL ?? PRODUCTION_GATEWAY_WS_URL,
    "MIR2_GATEWAY_WS_URL",
    new Set(["wss:"]),
  );
  return { gameUrl, gatewayWs };
}

function validateSecureUrl(value, name, schemes) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch (error) {
    throw new Error(`${name} is not a valid URL: ${error.message}`);
  }
  if (!schemes.has(parsed.protocol) || !parsed.hostname || parsed.username || parsed.password) {
    throw new Error(`${name} must use ${[...schemes].join(" or ")} without credentials`);
  }
  return parsed.toString();
}

export function renderLoader({ gameUrl, gatewayWs }) {
  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
    <title>Mir2</title>
    <style>
      html, body {
        margin: 0; height: 100%;
        background: #0b0e14; color: #d7dae0;
        font-family: system-ui, sans-serif;
        display: flex; flex-direction: column;
        align-items: center; justify-content: center; gap: 12px;
      }
      .spinner {
        width: 28px; height: 28px;
        border: 3px solid #2a3140; border-top-color: #e8b24a;
        border-radius: 50%; animation: spin 0.8s linear infinite;
      }
      @keyframes spin { to { transform: rotate(360deg); } }
    </style>
    <script>
      window.addEventListener("DOMContentLoaded", () => {
        const url = ${JSON.stringify(gameUrl)};
        const gateway = ${JSON.stringify(gatewayWs)};
        const target = new URL(url);
        if (gateway) target.searchParams.set("gatewayWs", gateway);
        window.location.replace(target.toString());
      });
    </script>
  </head>
  <body>
    <div class="spinner"></div>
    <p>Loading Mir2…</p>
  </body>
</html>
`;
}

export async function buildMobileLoader(environment = process.env) {
  const config = resolveMobileConfig(environment);
  await fs.mkdir(wwwRoot, { recursive: true });
  await fs.writeFile(path.join(wwwRoot, "index.html"), renderLoader(config), "utf8");
  console.log(`[mir2-mobile] wrote ${path.join(wwwRoot, "index.html")}`);
  console.log(`[mir2-mobile] game URL: ${config.gameUrl}`);
  console.log(`[mir2-mobile] gateway WS: ${config.gatewayWs}`);
}

if (path.resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  await buildMobileLoader();
}
