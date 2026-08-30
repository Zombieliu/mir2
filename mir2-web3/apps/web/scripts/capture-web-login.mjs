#!/usr/bin/env node

// Capture the web client's login screen through a private headless Chrome
// session. This is evidence collection only: it never submits credentials or
// mutates the game. Paths are derived from this script's repository location,
// not from the machine on which the script happens to be launched.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "../../..");
const DEFAULT_OUTPUT = path.join(REPO_ROOT, "docs", "ref", "web-login.png");
const require = createRequire(path.join(REPO_ROOT, "package.json"));
const { decodeCdpMessage } = await import("./cdp-message.mjs");
let WS;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

class Cdp {
  constructor(url) {
    this.ws = new WS(url, { perMessageDeflate: false });
    this.ws.binaryType = "arraybuffer";
    this.id = 1;
    this.pending = new Map();
    this.ws.addEventListener("message", (event) => {
      void this.handleMessage(event.data).catch(() => {});
    });
  }

  connect() {
    return new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  async handleMessage(raw) {
    const message = await decodeCdpMessage(raw);
    if (!message.id || !this.pending.has(message.id)) return;
    const pending = this.pending.get(message.id);
    this.pending.delete(message.id);
    if (message.error) pending.reject(new Error(message.error.message || "CDP request failed"));
    else pending.resolve(message.result ?? {});
  }

  send(method, params = {}) {
    const id = this.id++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (result.exceptionDetails) throw new Error("login-state evaluation failed");
    return result.result?.value;
  }

  close() {
    try { this.ws.close(); } catch { /* best effort */ }
  }
}

function parseArgs(argv) {
  const args = {
    url: process.env.WEB_URL ?? "http://127.0.0.1:3002",
    output: process.env.WEB_LOGIN_OUT ?? DEFAULT_OUTPUT,
    chrome: process.env.MIR2_CHROME_PATH ?? defaultChromeCommand(),
    waitMs: numberEnv(process.env.WEB_LOGIN_WAIT_MS, 6000, "WEB_LOGIN_WAIT_MS"),
    port: null,
    mode: "run",
  };
  const valueFlags = new Map([
    ["--url", "url"],
    ["--output", "output"],
    ["--chrome", "chrome"],
    ["--wait-ms", "waitMs"],
    ["--port", "port"],
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") { args.mode = "help"; continue; }
    if (token === "--self-test") { args.mode = "self-test"; continue; }
    const equals = token.indexOf("=");
    const name = equals > 2 ? token.slice(0, equals) : token;
    const key = valueFlags.get(name);
    if (!key) throw new Error(`unknown argument: ${token}`);
    const value = equals > 2 ? token.slice(equals + 1) : argv[++index];
    if (value === undefined || value === "" || value.startsWith("--")) throw new Error(`${name} requires a value`);
    args[key] = value;
  }
  if (args.mode === "help" || args.mode === "self-test") return args;
  args.url = validateUrl(args.url);
  args.output = path.resolve(String(args.output));
  args.chrome = String(args.chrome).trim();
  if (!args.chrome) throw new Error("--chrome must not be empty");
  args.waitMs = positiveInteger(args.waitMs, "--wait-ms");
  if (args.port !== null) {
    args.port = positiveInteger(args.port, "--port");
    if (args.port > 65535) throw new Error("--port must be between 1 and 65535");
  }
  return args;
}

function defaultChromeCommand() {
  if (process.platform === "win32") return "chrome.exe";
  if (process.platform === "darwin") return "Google Chrome";
  return "google-chrome";
}

function validateUrl(value) {
  let parsed;
  try { parsed = new URL(String(value)); } catch { throw new Error(`--url must be a valid http(s) URL: ${value}`); }
  if (!["http:", "https:"].includes(parsed.protocol)) throw new Error(`--url must use http or https: ${value}`);
  return parsed.toString();
}

function numberEnv(value, fallback, name) {
  return value === undefined ? fallback : positiveInteger(value, name);
}

function positiveInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${flag} must be a positive integer; received ${value}`);
  return parsed;
}

function printHelp() {
  console.log(`Usage:
  node apps/web/scripts/capture-web-login.mjs [options]

Options:
  --url URL       Web URL (env: WEB_URL; default: http://127.0.0.1:3002)
  --output FILE   PNG path (env: WEB_LOGIN_OUT; default: repo/docs/ref/web-login.png)
  --chrome PATH   Chrome command/path (env: MIR2_CHROME_PATH)
  --wait-ms N     Render settle time (env: WEB_LOGIN_WAIT_MS; default: 6000)
  --port N        Fixed CDP port; otherwise an ephemeral local port is chosen
  --self-test     Validate arguments and path derivation without launching Chrome
  --help          Show this help

Safety:
  This script only captures a login screen. It never submits login, registration,
  password, account, or game commands. A successful capture is HANDOFF evidence,
  not human visual acceptance.`);
}

async function waitForCdp(port, child, timeoutMs) {
  const deadline = Date.now() + Math.max(10_000, timeoutMs);
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`Chrome exited before CDP became ready (code ${child.exitCode})`);
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return;
    } catch { /* Chrome is still starting. */ }
    await sleep(200);
  }
  throw new Error(`Chrome CDP did not become ready within ${timeoutMs}ms`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.mode === "help") { printHelp(); return; }
  if (args.mode === "self-test") {
    console.log(JSON.stringify({ ok: true, status: "HANDOFF", repoRoot: REPO_ROOT, defaultOutput: DEFAULT_OUTPUT, desktopTouched: false, accountMutation: false }, null, 2));
    return;
  }

  try {
    WS = require("next/dist/compiled/ws");
  } catch (error) {
    throw new Error(
      `Next's bundled WebSocket client is unavailable from ${REPO_ROOT}. ` +
        `Run this script from a checkout with web dependencies installed: ${error.message}`,
    );
  }

  const port = args.port ?? 14_500 + Math.floor(Math.random() * 150);
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-login-"));
  const child = spawn(args.chrome, [
    `--remote-debugging-port=${port}`,
    `--user-data-dir=${userDataDir}`,
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--window-size=1024,768",
  ], { stdio: "ignore", windowsHide: true });
  let cdp;
  try {
    await waitForCdp(port, child, args.waitMs);
    const targetResponse = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" });
    if (!targetResponse.ok) throw new Error(`Chrome could not create a tab (HTTP ${targetResponse.status})`);
    const target = await targetResponse.json();
    if (!target.webSocketDebuggerUrl) throw new Error("Chrome returned no CDP WebSocket URL");
    cdp = new Cdp(target.webSocketDebuggerUrl);
    await cdp.connect();
    await cdp.send("Runtime.enable");
    await cdp.send("Page.enable");
    await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false });
    await cdp.send("Page.navigate", { url: args.url });
    await sleep(args.waitMs);
    const state = await cdp.evaluate(`(() => {
      const s = window.__mir2Stage5?.state || {};
      return { screen: s.screen || "?", hasAccount: Boolean(document.querySelector(".login-input.account")), hasPass: Boolean(document.querySelector(".login-input.password")), bodyClass: document.body.className };
    })()`);
    if (!state?.hasAccount || !state?.hasPass) throw new Error(`login controls were not visible; screen=${state?.screen ?? "?"}`);
    const screenshot = await cdp.send("Page.captureScreenshot", { format: "png" });
    fs.mkdirSync(path.dirname(args.output), { recursive: true });
    fs.writeFileSync(args.output, Buffer.from(screenshot.data, "base64"));
    console.log(JSON.stringify({ ok: true, status: "HANDOFF", state, saved: args.output, desktopTouched: false, accountMutation: false, acceptance: "human-or-visual-model-review-required" }, null, 2));
  } finally {
    cdp?.close();
    try { child.kill(); } catch { /* best effort */ }
  }
}

main().catch((error) => {
  console.error(JSON.stringify({ ok: false, status: "BLOCKED", error: String(error?.message ?? error) }, null, 2));
  process.exitCode = 1;
});
