import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";

import {
  PRODUCTION_GAME_URL,
  PRODUCTION_GATEWAY_WS_URL,
  renderLoader,
  resolveMobileConfig,
} from "./build-web.mjs";

const require = createRequire(import.meta.url);
const { createCapacitorConfig } = require("./capacitor-config.cjs");

test("mobile loader defaults to stable secure production endpoints", () => {
  const config = resolveMobileConfig({});
  assert.equal(config.gameUrl, `${PRODUCTION_GAME_URL}/`);
  assert.equal(config.gatewayWs, PRODUCTION_GATEWAY_WS_URL);
  assert.doesNotMatch(renderLoader(config), /vercel\.app/);
});

test("mobile loader rejects insecure or credential-bearing endpoints", () => {
  assert.throws(
    () => resolveMobileConfig({ MIR2_MOBILE_GAME_URL: "http://example.com" }),
    /must use https:/,
  );
  assert.throws(
    () => resolveMobileConfig({ MIR2_GATEWAY_WS_URL: "ws://example.com/ws" }),
    /must use wss:/,
  );
  assert.throws(
    () => resolveMobileConfig({ MIR2_MOBILE_GAME_URL: "https://user:pass@example.com" }),
    /without credentials/,
  );
});

test("Capacitor keeps the configured HTTPS game host inside the app WebView", () => {
  const production = createCapacitorConfig({});
  assert.deepEqual(production.server.allowNavigation, ["mir2.obelisk.build"]);

  const staging = createCapacitorConfig({
    MIR2_MOBILE_GAME_URL: "https://staging.example.com/game?channel=qa",
  });
  assert.deepEqual(staging.server.allowNavigation, ["staging.example.com"]);

  assert.throws(
    () => createCapacitorConfig({ MIR2_MOBILE_GAME_URL: "http://example.com" }),
    /must use https:/,
  );
});
