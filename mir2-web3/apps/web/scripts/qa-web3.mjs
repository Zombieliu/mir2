// qa-web3.mjs — QA loop for the web3-specific surface (unique to this port).
//
// Every other qa-*.mjs loop exercises the Crystal game (movement, combat, items,
// persistence, render). NONE of them touch the two things that make THIS a web3
// port: the Sui login (passkey / wallet → gateway token) and the on-chain smart
// mine (mine_batch / redeem PTBs → relayer → bag). This loop covers exactly that
// gap, driving the REAL client handlers in apps/web/app/page.tsx:
//
//   • requestSuiLoginToken / sendSuiLoginCommand / submitSuiLogin   (login)
//   • requestPasskeyLoginToken (@mysten/sui passkey)                (login: passkey)
//   • requestWalletLoginToken (@mysten/wallet-standard)            (login: wallet)
//   • OnchainMinePanel + onchainSwing / submitOnchainBatch / redeem (mine M1–M4)
//
// ─────────────────────────────────────────────────────────────────────────────
// IT RUNS IN TIERS, AS FAR AS THE ENVIRONMENT ALLOWS, AND DOCUMENTS EACH GATE.
// It NEVER fakes success: a step that cannot complete because a prerequisite is
// missing is recorded as `gated` (with the exact env/wallet it needs), not `pass`.
//
//   STEP 1  Web3 login UI presence ............ no prerequisites (always runs)
//   STEP 2  Passkey login token request ....... CDP WebAuthn virtual authenticator
//   STEP 3  Wallet  login token request ....... injected Wallet-Standard mock wallet
//   STEP 4  On-chain mine panel + mine action . NEXT_PUBLIC_ONCHAIN_MINE=1 build
//
// ─────────────────────────────────────────────────────────────────────────────
// WHY a real browser (CDP) and not a unit test: the Sui login and the on-chain
// mine are browser-only — WebAuthn (passkey), the Wallet-Standard registry, and
// @mysten/sui PTB signing only exist in a real page. This loop drives the actual
// login-screen buttons and the actual mine HUD, the same surface a player uses.
//
// ─────────────────────────────────────────────────────────────────────────────
// HOW THE GATES WORK (so the report's "gated" verdicts are actionable):
//
//   • Passkey login token (STEP 2): the harness installs a CDP **WebAuthn virtual
//     authenticator** so navigator.credentials.create/get return REAL ES256
//     signatures, and @mysten/sui's BrowserPasskeyProvider can run for real.
//     WebAuthn requires a registrable rp.id, and page.tsx uses
//     rp.id = window.location.hostname, so the harness defaults to a `localhost`
//     base URL (an IP host like 127.0.0.1 makes the browser reject the ceremony).
//     The token is only ISSUED if /api/passkey/login can sign it, which needs
//     MIR2_PASSKEY_AUTH_SECRET (or MIR2_ALLOW_DEV_PASSKEY_SECRET=1) on the web
//     server — otherwise that endpoint returns 500 "…secret is not set". The
//     harness records the exact HTTP status of the token request either way.
//       › 200 + token  → token issued (request fully exercised)
//       › 500          → GATED on MIR2_ALLOW_DEV_PASSKEY_SECRET=1 (the signature
//                        verified — 500 is thrown only AFTER verifyPersonalMessage)
//       › 401          → the passkey signature did not verify in this environment
//
//   • Wallet login token (STEP 3): a real browser wallet extension cannot run in
//     headless automation, so the harness injects a minimal **Wallet-Standard
//     mock wallet** (real registry registration + standard:connect +
//     sui:signPersonalMessage). This fully exercises the wallet client path
//     (discover → connect → pickSuiAccount → signPersonalMessage → POST
//     /api/passkey/login). The mock signs with a structurally-valid but NON-
//     cryptographic signature, so the server returns 401 ("invalid passkey
//     signature"). That 401 is the DOCUMENTED boundary — proof the whole client
//     pipeline ran end-to-end to the server — not a faked pass. A real funded
//     wallet extension is required to obtain a wallet token.
//
//   • suiLogin → enter game (STEPS 2/3, best-effort): a successfully issued token
//     is handed to sendSuiLoginCommand → the gateway, which verifies the HMAC with
//     the SAME secret. Reaching screen="select"/"game" therefore also needs the
//     gateway to share the secret. The harness attempts it when a token exists and
//     records whether the client reached the game.
//
//   • On-chain mine (STEP 4): the OnchainMinePanel only mounts when the web build
//     has NEXT_PUBLIC_ONCHAIN_MINE=1 (+ the 4 deployment ids). The harness enters
//     the world via ordinary password login (reliable, no Sui secret needed),
//     then looks for the panel:
//       › panel absent → GATED on NEXT_PUBLIC_ONCHAIN_MINE=1 at build time
//       › panel present → connect the (mock) signing wallet, read the vein stage,
//         drive mine swings to accumulate a batch, and observe the batch submit.
//         Actual on-chain SETTLEMENT (ore credited, vein stage advancing) needs a
//         FUNDED testnet wallet that can sign+execute + the relayer running, so the
//         mock wallet's sign-and-execute is intentionally gated and the resulting
//         error is recorded as the on-chain settlement boundary.
//
// ─────────────────────────────────────────────────────────────────────────────
// TESTNET / WALLET PREREQUISITES (to run the gated tiers to completion):
//
//   To exercise STEP 2 to a 200 token:   start the web dev server with
//       MIR2_ALLOW_DEV_PASSKEY_SECRET=1   (and the Rust gateway too, to enter game)
//   To exercise STEP 4 (panel + mine):   start the web dev server with
//       NEXT_PUBLIC_ONCHAIN_MINE=1 NEXT_PUBLIC_ONCHAIN_MINE_PACKAGE_ID=… (4 ids)
//   To exercise STEP 4 to a real SETTLEMENT, additionally:
//       • a Sui testnet wallet funded with SUI (faucet) — the local keystore alias
//         `mir2-onchain-mine` (0xf5fbee0e…b74f) is the project's funded testnet
//         deployer/admin wallet (key in ~/.sui, NEVER exported/committed). Its
//         private key cannot enter the browser, so true in-browser settlement is
//         driven by a real Wallet-Standard extension holding a funded address.
//       • the relayer + gateway /onchain/inject running (docs/ONCHAIN-M4-E2E-RUNBOOK.md).
//   The live testnet deployment ids are in onchain/deployments/testnet.json.
//
// See: docs/ONCHAIN-M4-E2E-RUNBOOK.md, docs/ONCHAIN-SMART-MINE-DESIGN.md, and the
// on-chain-mine initiative memory for the full setup.
//
// ─────────────────────────────────────────────────────────────────────────────
// Self-contained: reuses the proven CdpClient / launchChrome / login patterns from
// qa-persistence.mjs + qa-playthrough.mjs. Zero new dependencies (node built-ins).
//
// Usage:
//   node ./scripts/qa-web3.mjs --headed --baseUrl http://localhost:3010
//        [--gatewayWs ws://localhost:7111/ws]
//        [--account NAME --password PW --characterName NAME]
//        [--skipGameEntry true]   # skip STEP 4's world entry (UI/login tiers only)
//        [--runId my-run]
//
// Exit code is non-zero only if a HIGH-severity issue is recorded (a real defect:
// missing login UI, a client crash, etc.). `gated` tiers are reported, not failed,
// so the loop is safe to run in CI without the web3 env. A full report (json + md)
// lands under docs/generated/player-qa/web3-<runId>/.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

// localhost (NOT 127.0.0.1) is the default so the WebAuthn passkey ceremony has a
// valid registrable rp.id (browsers reject IP-literal rp.ids). :3010 is the port a
// local `npm run dev` commonly lands on; override with --baseUrl as needed.
const baseUrlRaw = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://localhost:3010";
// Point the client at the Rust gateway (:7111) for the real login/startGame path
// (the `?gatewayWs=` override is honored only on localhost — page.tsx:988).
const gatewayWsUrl = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? "ws://localhost:7111/ws";
const RUN_URL = buildRunUrl(baseUrlRaw, gatewayWsUrl);

const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const skipGameEntry = booleanArg(args.skipGameEntry ?? process.env.MIR2_QA_SKIP_GAME_ENTRY, false);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultName("ACC");
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultName("W3");
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9750 + (process.pid % 200));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "player-qa", `web3-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const STAGE_READY_TIMEOUT_MS = numberArg(args.stageReadyTimeoutMs, 25_000);
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const ENTER_GAME_TIMEOUT_MS = numberArg(args.enterGameTimeoutMs, 30_000);
const TOKEN_REQUEST_TIMEOUT_MS = numberArg(args.tokenRequestTimeoutMs, 25_000);
const MINE_BATCH_OUTCOME_TIMEOUT_MS = numberArg(args.mineBatchTimeoutMs, 30_000);

// The number of mine swings to drive in STEP 4 (one above the batch threshold so
// the auto-flush effect submits a batch). The client default batch size is 5.
const MINE_SWINGS = numberArg(args.mineSwings, 6);

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

// ---------------------------------------------------------------------------
// In-page bootstrap injected at document-start (Page.addScriptToEvaluateOnNewDocument)
// BEFORE the app loads, so the mock wallet is in the Wallet-Standard registry from
// module init and the fetch hook captures the very first /api/passkey/login call.
//
//   • window.fetch wrapper → records every POST /api/passkey/login (status + body)
//     into window.__qaWeb3.passkeyCalls — the authoritative token-request signal.
//   • a minimal Wallet-Standard wallet ("Mir2 QA Mock Wallet") with a real account,
//     standard:connect, sui:signPersonalMessage (structurally-valid but NON-crypto
//     dummy signature) and a sui:signAndExecuteTransaction that THROWS a clearly
//     labelled "on-chain gated" error (the documented settlement boundary).
//
// All values here are honest: the dummy signature yields a server 401 and the
// sign-and-execute throw yields a recorded batch error. Nothing fakes a pass.
// ---------------------------------------------------------------------------
const PAGE_BOOTSTRAP = `
(() => {
  if (window.__qaWeb3Installed) return;
  window.__qaWeb3Installed = true;

  var state = { passkeyCalls: [], walletEvents: [], mockWalletAddress: null, registered: false };
  window.__qaWeb3 = state;

  // ---- fetch instrumentation: capture the Sui login token request ----------
  var origFetch = window.fetch ? window.fetch.bind(window) : null;
  if (origFetch) {
    window.fetch = async function (input, init) {
      var url = typeof input === "string" ? input : (input && input.url) || "";
      var isPasskey = url.indexOf("/api/passkey/login") !== -1;
      var reqBody = null;
      if (isPasskey && init && typeof init.body === "string") {
        try { reqBody = JSON.parse(init.body); } catch (e) {}
      }
      try {
        var res = await origFetch(input, init);
        if (isPasskey) {
          var body = null;
          try { body = await res.clone().json(); } catch (e) {}
          state.passkeyCalls.push({
            at: Date.now(),
            status: res.status,
            ok: res.ok,
            address: (reqBody && reqBody.address) || null,
            hasToken: !!(body && body.token),
            accountId: (body && body.accountId) || null,
            error: (body && body.error) || null,
          });
        }
        return res;
      } catch (err) {
        if (isPasskey) {
          state.passkeyCalls.push({
            at: Date.now(), status: 0, ok: false, networkError: true,
            error: String((err && err.message) || err),
          });
        }
        throw err;
      }
    };
  }

  // ---- minimal Wallet-Standard mock wallet ---------------------------------
  // A fixed, clearly-synthetic Sui address (0x + 64 hex chars). It is NOT derived
  // from a real keypair, so the server cannot verify its signatures — exactly the
  // point: the wallet path runs end-to-end and the server honestly rejects it (401).
  var ADDRESS = "0x" + new Array(33).join("ab"); // "ab" × 32 = 64 hex chars
  state.mockWalletAddress = ADDRESS;

  function dummySignature() {
    // Sui Ed25519 serialized signature = flag(1) + sig(64) + pubkey(32) = 97 bytes.
    // All-zero bytes: well-formed length/flag, cryptographically invalid -> server 401.
    var bytes = new Uint8Array(97);
    var bin = "";
    for (var i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
    return btoa(bin);
  }
  function b64(u8) {
    var bin = "";
    for (var i = 0; i < u8.length; i++) bin += String.fromCharCode(u8[i]);
    return btoa(bin);
  }

  var account = {
    address: ADDRESS,
    publicKey: new Uint8Array(32),
    chains: ["sui:testnet"],
    features: ["sui:signPersonalMessage", "sui:signAndExecuteTransaction"],
    label: "Mir2 QA Mock",
  };

  var ICON =
    "data:image/svg+xml;base64," +
    btoa('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="#4466ff"/></svg>');

  var wallet = {
    version: "1.0.0",
    name: "Mir2 QA Mock Wallet",
    icon: ICON,
    chains: ["sui:testnet"],
    accounts: [account],
    features: {
      "standard:connect": {
        version: "1.0.0",
        connect: async function () {
          state.walletEvents.push({ type: "connect", at: Date.now() });
          return { accounts: wallet.accounts };
        },
      },
      "standard:disconnect": {
        version: "1.0.0",
        disconnect: async function () {},
      },
      "standard:events": {
        version: "1.0.0",
        on: function () { return function () {}; },
      },
      "sui:signPersonalMessage": {
        version: "1.0.0",
        signPersonalMessage: async function (input) {
          state.walletEvents.push({ type: "signPersonalMessage", at: Date.now() });
          var bytes = input && input.message ? b64(input.message) : "";
          return { bytes: bytes, signature: dummySignature() };
        },
      },
      "sui:signAndExecuteTransaction": {
        version: "2.0.0",
        signAndExecuteTransaction: async function () {
          state.walletEvents.push({ type: "signAndExecuteTransaction", at: Date.now() });
          throw new Error(
            "QA harness mock wallet: on-chain sign+execute is intentionally gated " +
              "(no funded testnet wallet in automation). This is the documented on-chain " +
              "settlement boundary, not a bug."
          );
        },
      },
    },
  };

  function register() {
    try {
      window.dispatchEvent(
        new CustomEvent("wallet-standard:register-wallet", {
          detail: function (api) { api.register(wallet); },
        })
      );
      state.registered = true;
    } catch (e) {}
  }
  window.addEventListener("wallet-standard:app-ready", function (event) {
    try { event.detail.register(wallet); state.registered = true; } catch (e) {}
  });
  register();
})();
`;

// ---------------------------------------------------------------------------
// CDP client — captures console, network failures, and WebSocket frames.
// (Same shape as qa-persistence.mjs's client.)
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleMessages = [];
    this.consoleErrors = [];
    this.networkFailures = [];
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
    this.webSocketFramesSent = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.handleMessage(event.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  handleMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      else resolve(message.result ?? {});
      return;
    }

    const m = message.method;
    const p = message.params ?? {};

    if (m === "Runtime.consoleAPICalled") {
      const entry = {
        source: "console",
        type: p.type ?? "log",
        text: (p.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
        at: Date.now(),
      };
      this.consoleMessages.push(entry);
      this.consoleMessages = this.consoleMessages.slice(-400);
      if (p.type === "error" || p.type === "warning") this.consoleErrors.push(entry);
    } else if (m === "Runtime.exceptionThrown") {
      const d = p.exceptionDetails;
      this.consoleErrors.push({
        source: "exception",
        type: "error",
        text: d?.exception?.description ?? d?.text ?? "runtime exception",
        at: Date.now(),
      });
    } else if (m === "Log.entryAdded") {
      const entry = p.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({
          source: entry.source ?? "log",
          type: "error",
          text: `${entry.text ?? ""}${entry.url ? ` (${entry.url})` : ""}`,
          at: Date.now(),
        });
      }
    } else if (m === "Network.requestWillBeSent") {
      if (p.requestId && p.request?.url) this.requestUrlById.set(p.requestId, p.request.url);
    } else if (m === "Network.responseReceived") {
      const r = p.response;
      const url = String(r?.url ?? "");
      if (r && r.status >= 400 && !url.includes("favicon")) {
        this.networkFailures.push({ url, status: r.status, at: Date.now() });
      }
    } else if (m === "Network.loadingFailed") {
      const url = this.requestUrlById.get(p.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) {
        this.networkFailures.push({ url, status: 0, errorText: p.errorText ?? "", at: Date.now() });
      }
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-800);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-800);
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) {
      throw new Error(`Evaluation failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`);
    }
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

// ---------------------------------------------------------------------------
// Run context + issue collection + step status.
// ---------------------------------------------------------------------------
function createContext(client) {
  const issues = [];
  const steps = [];
  return {
    client,
    issues,
    steps,
    seq: 0,
    log: (msg) => console.log(msg),
    addIssue(issue) {
      const id = `${issue.category}-${String(issues.length + 1).padStart(3, "0")}`;
      const full = { id, severity: "medium", ...issue };
      issues.push(full);
      console.log(`  ⚠ [${full.severity}/${full.category}] ${full.summary}`);
      return full;
    },
  };
}

// A step's verdict is one of pass | gated | fail. `gated` means a prerequisite
// (env/wallet/secret) is missing — recorded with the exact thing it needs, NOT a
// failure. `fail` is a real defect; it records a high-severity issue.
async function runStep(ctx, title, fn) {
  ctx.seq += 1;
  const id = ctx.seq;
  const step = { id, title, startedAt: Date.now(), verdict: "pass", notes: [], gates: [] };
  ctx.log(`\n▶ step ${id}: ${title}`);
  const api = {
    step,
    note: (msg) => {
      step.notes.push(msg);
      ctx.log(`  · ${msg}`);
    },
    gate: (summary, prerequisite, detail) => {
      step.verdict = step.verdict === "fail" ? "fail" : "gated";
      step.gates.push({ summary, prerequisite, detail });
      ctx.log(`  ⛒ gated: ${summary}  →  needs: ${prerequisite}`);
    },
    fail: (summary, detail, severity = "high") => {
      step.verdict = "fail";
      ctx.addIssue({ category: "web3", severity, step: id, summary, detail });
    },
  };
  try {
    await fn(api);
  } catch (err) {
    step.verdict = "fail";
    step.error = String(err?.message ?? err);
    ctx.addIssue({
      category: "flow",
      severity: "high",
      step: id,
      summary: `step crashed: ${title}`,
      detail: step.error,
    });
    ctx.log(`  ✖ ${step.error.slice(0, 300)}`);
  }
  const frameName = `${String(id).padStart(2, "0")}-${slug(title)}.png`;
  try {
    await captureFrame(ctx.client, path.join(framesDir, frameName));
    step.frame = `frames/${frameName}`;
  } catch {
    /* screenshot is best-effort */
  }
  step.endedAt = Date.now();
  step.durationMs = step.endedAt - step.startedAt;
  ctx.steps.push(step);
  ctx.log(`  = step ${id} verdict: ${step.verdict.toUpperCase()}`);
  return step;
}

// ---------------------------------------------------------------------------
// The four QA steps.
// ---------------------------------------------------------------------------

// STEP 1 — the web3 login surface exists and is wired. No env needed: these
// elements render on the plain login screen regardless of any wallet/secret.
async function stepLoginUiPresence(ctx) {
  await runStep(ctx, "web3 login UI present (Passkey + Wallet buttons, picker, Dubhe link)", async (api) => {
    const client = ctx.client;
    await navigate(client, RUN_URL);
    await waitForStageReady(client);
    // Get to the login screen (fresh load lands here; if it auto-advanced, note it).
    const screen0 = await readScreen(client);
    api.note(`initial screen: ${screen0}`);
    // The original-client login overlay (which contains .login-web3-actions) mounts a
    // beat AFTER the screen state flips to "login" — wait for it before probing.
    const overlayUp = await waitForSelectorSoft(client, ".login-web3-actions", 10_000);
    if (!overlayUp) {
      api.note(`client is on "${await readScreen(client)}"; .login-web3-actions did not render within 10s`);
    }

    const ui = await client.evaluate(`
      (() => {
        const actions = document.querySelector(".login-web3-actions");
        const btns = actions ? Array.from(actions.querySelectorAll("button")) : [];
        const label = (b) => (b?.textContent || "").trim();
        const passkey = btns.find((b) => /passkey/i.test(label(b))) || btns[0] || null;
        const wallet = btns.find((b) => /wallet/i.test(label(b))) || btns[1] || null;
        return {
          actionsPresent: Boolean(actions),
          buttonCount: btns.length,
          passkeyLabel: passkey ? label(passkey) : null,
          walletLabel: wallet ? label(wallet) : null,
          walletAriaControls: wallet ? wallet.getAttribute("aria-controls") : null,
        };
      })()
    `);
    api.step.ui = ui;
    api.note(`login-web3-actions present=${ui.actionsPresent} buttons=${ui.buttonCount} passkey="${ui.passkeyLabel}" wallet="${ui.walletLabel}"`);

    if (!ui.actionsPresent) {
      api.fail("the web3 login actions (.login-web3-actions) are missing from the login screen", ui);
      return;
    }
    if (!ui.passkeyLabel) api.fail("Passkey login button is missing", ui);
    if (!ui.walletLabel) api.fail("Wallet login button is missing", ui);

    // Open the wallet picker and inspect it: it must list the injected mock wallet
    // (proving Wallet-Standard discovery works) and show the Dubhe install link.
    await clickWeb3Button(client, "wallet");
    const pickerUp = await waitForSelectorSoft(client, "#login-wallet-picker", 5_000);
    const picker = await client.evaluate(`
      (() => {
        const el = document.querySelector("#login-wallet-picker");
        if (!el) return { present: false };
        const options = Array.from(el.querySelectorAll(".login-wallet-option")).map((b) => (b.textContent || "").trim());
        const empty = el.querySelector(".login-wallet-empty");
        const install = el.querySelector(".login-wallet-install");
        return {
          present: true,
          options,
          empty: empty ? (empty.textContent || "").trim() : null,
          installHref: install ? install.getAttribute("href") : null,
        };
      })()
    `);
    api.step.picker = { pickerUp, ...picker };
    if (!picker.present) {
      api.fail("wallet picker (#login-wallet-picker) did not open on toggle", { pickerUp });
    } else {
      api.note(`wallet picker options: ${JSON.stringify(picker.options)}; empty=${picker.empty ?? "—"}; dubheLink=${picker.installHref ?? "—"}`);
      const hasMock = picker.options.some((o) => /mir2 qa mock wallet/i.test(o));
      if (hasMock) {
        api.note("✓ injected Wallet-Standard mock wallet was discovered by the client registry");
      } else {
        // Discovery failure is a real signal worth surfacing (medium — the harness
        // mock, not a product defect, but it means the wallet path can't be driven).
        api.fail(
          "injected mock wallet not listed in the picker (Wallet-Standard discovery did not pick it up)",
          { options: picker.options },
          "medium",
        );
      }
      if (!picker.installHref) {
        api.note("note: Dubhe install link absent (only shown when no Dubhe wallet is detected)");
      }
    }
    // Close the picker again so it doesn't overlay later steps.
    await clickWeb3Button(client, "wallet").catch(() => {});
  });
}

// STEP 2 — passkey login token request, driven through a real WebAuthn ceremony
// via a CDP virtual authenticator.
async function stepPasskeyLogin(ctx) {
  await runStep(ctx, "passkey login → token request (WebAuthn virtual authenticator)", async (api) => {
    const client = ctx.client;
    const isLocalhost = /^https?:\/\/localhost(:|\/|$)/i.test(baseUrlRaw);
    if (!isLocalhost) {
      api.gate(
        `base URL host "${new URL(baseUrlRaw).host}" is not localhost; WebAuthn rejects IP-literal rp.ids`,
        "run with --baseUrl http://localhost:<port> so the passkey rp.id is valid",
        { baseUrl: baseUrlRaw },
      );
    }

    // Install a virtual platform authenticator (resident key + UV) so
    // navigator.credentials.create/get return real ES256 credentials/signatures.
    let authenticatorId = null;
    try {
      await client.send("WebAuthn.enable", { enableUI: false });
      const res = await client.send("WebAuthn.addVirtualAuthenticator", {
        options: {
          protocol: "ctap2",
          transport: "internal",
          hasResidentKey: true,
          hasUserVerification: true,
          isUserVerified: true,
          automaticPresenceSimulation: true,
        },
      });
      authenticatorId = res.authenticatorId ?? null;
      api.note(`virtual authenticator added (${authenticatorId})`);
    } catch (err) {
      api.gate(
        `could not add a WebAuthn virtual authenticator: ${String(err?.message ?? err)}`,
        "a CDP build that supports WebAuthn.addVirtualAuthenticator, or a real platform authenticator",
        null,
      );
    }

    await ensureOnLoginScreen(client, api);

    const before = ctx.client.webSocketFramesReceived.length;
    const sinceCalls = await passkeyCallCount(client);
    const t0 = Date.now();
    const clicked = await clickWeb3Button(client, "passkey");
    if (!clicked) {
      api.gate(
        "Passkey login button not present/clickable on the login screen (see step 1)",
        "the web3 login UI (.login-web3-actions) to be rendered",
        null,
      );
      return;
    }

    // Wait for EITHER a token request to be recorded, a login error to surface, or
    // the client to advance to select/game.
    const outcome = await waitForLoginOutcome(client, sinceCalls, TOKEN_REQUEST_TIMEOUT_MS);
    api.step.outcome = outcome;
    api.note(`passkey outcome after ${Date.now() - t0}ms: ${describeOutcome(outcome)}`);

    classifyTokenRequest(api, "passkey", outcome);

    // Best-effort: if a token was issued, see whether suiLogin reached the game.
    await maybeAssertEnteredGame(ctx, api, "passkey", outcome, before);

    if (authenticatorId) {
      await client.send("WebAuthn.removeVirtualAuthenticator", { authenticatorId }).catch(() => {});
    }
  });
}

// STEP 3 — wallet login token request, driven through the injected Wallet-Standard
// mock wallet. Exercises the full wallet client path to the server's signature
// check; the mock's non-cryptographic signature yields the documented 401 boundary.
async function stepWalletLogin(ctx) {
  await runStep(ctx, "wallet login → token request (injected Wallet-Standard mock)", async (api) => {
    const client = ctx.client;
    await ensureOnLoginScreen(client, api);

    // Make sure the mock wallet is discoverable before we try to log in with it.
    const mock = await client.evaluate(`
      (() => {
        const w = window.__qaWeb3 || {};
        return { registered: !!w.registered, address: w.mockWalletAddress || null };
      })()
    `);
    api.note(`mock wallet registered=${mock.registered} address=${mock.address}`);

    // Open the picker and click the mock wallet option.
    const opened = await clickWeb3Button(client, "wallet");
    if (!opened) {
      api.gate(
        "Wallet login button not present/clickable on the login screen (see step 1)",
        "the web3 login UI (.login-web3-actions) to be rendered",
        null,
      );
      return;
    }
    await waitForSelectorSoft(client, "#login-wallet-picker", 5_000);
    const clicked = await client.evaluate(`
      (() => {
        const opts = Array.from(document.querySelectorAll("#login-wallet-picker .login-wallet-option"));
        const mock = opts.find((b) => /mir2 qa mock wallet/i.test(b.textContent || "")) || opts[0];
        if (!mock) return false;
        mock.click();
        return true;
      })()
    `);
    if (!clicked) {
      api.gate(
        "no wallet option to click (mock wallet not discovered / no real wallet present)",
        "a Wallet-Standard wallet — a real browser extension, or the harness's injected mock",
        null,
      );
      return;
    }

    const sinceCalls = await passkeyCallCount(client);
    const before = ctx.client.webSocketFramesReceived.length;
    const t0 = Date.now();
    const outcome = await waitForLoginOutcome(client, sinceCalls, TOKEN_REQUEST_TIMEOUT_MS);
    api.step.outcome = outcome;
    api.note(`wallet outcome after ${Date.now() - t0}ms: ${describeOutcome(outcome)}`);

    classifyTokenRequest(api, "wallet", outcome);
    await maybeAssertEnteredGame(ctx, api, "wallet", outcome, before);
  });
}

// STEP 4 — on-chain mine: enter the world (password login), open the mine panel,
// connect the signing wallet, drive mine swings, and observe the batch submit /
// settlement boundary.
async function stepOnchainMine(ctx) {
  await runStep(ctx, "on-chain mine panel + mine action + settlement boundary", async (api) => {
    const client = ctx.client;

    if (skipGameEntry) {
      api.gate("--skipGameEntry set; not entering the world", "re-run without --skipGameEntry", null);
      return;
    }

    // Enter the world via ordinary password login. This is deliberately NOT the Sui
    // login (which is gated on the passkey secret): we only need to be in-game to
    // reach the mine HUD, and password login is the reliable path on any gateway.
    api.note("entering the world via password login (to reach the in-game mine HUD)");
    const entered = await enterWorldViaPassword(ctx, api);
    if (!entered) {
      api.gate(
        "could not enter the world via password login (gateway/login path unavailable)",
        `a running gateway at ${gatewayWsUrl} (and web server at ${baseUrlRaw})`,
        null,
      );
      return;
    }

    // The OnchainMinePanel mounts only when NEXT_PUBLIC_ONCHAIN_MINE=1 at build.
    const panelPresent = await waitForSelectorSoft(client, '[data-testid="onchain-mine-panel"]', 6_000);
    if (!panelPresent) {
      api.gate(
        "OnchainMinePanel did not mount in-game (web build has NEXT_PUBLIC_ONCHAIN_MINE unset)",
        "build/serve the web client with NEXT_PUBLIC_ONCHAIN_MINE=1 (+ the 4 NEXT_PUBLIC_ONCHAIN_MINE_* deployment ids)",
        null,
      );
      return;
    }
    api.note("✓ OnchainMinePanel mounted (NEXT_PUBLIC_ONCHAIN_MINE is enabled)");

    let panel = await readOnchainPanel(client);
    api.step.panelInitial = panel;
    api.note(`vein: ${panel.veinText ?? "—"}; wallet: ${panel.walletConnected ? panel.walletAddress : "not connected"}`);

    // Connect the signing wallet (the injected mock). connectOnchainWallet uses the
    // same Wallet-Standard wallet the login flow would.
    if (!panel.walletConnected) {
      const connectClicked = await clickPanelButton(client, ["连接 Sui 钱包", "Connect wallet"]);
      if (!connectClicked) {
        api.gate("no 'Connect wallet' control in the panel", "a panel build exposing the connect action", null);
      } else {
        const connected = await waitUntilSoft(
          client,
          `(() => { const el = document.querySelector('[data-testid="onchain-mine-panel"]'); return !!el && /签名钱包/.test(el.textContent || ""); })()`,
          10_000,
        );
        panel = await readOnchainPanel(client);
        if (connected && panel.walletConnected) {
          api.note(`✓ signing wallet connected: ${panel.walletAddress}`);
        } else {
          api.gate(
            "signing wallet did not connect (no Wallet-Standard wallet available to retain)",
            "a real funded Sui wallet extension (or the harness mock) able to connect for signing",
            { panel },
          );
          return;
        }
      }
    }

    // Drive mine swings. The dev "挥镐(调试) Swing" button funnels into the SAME
    // onchainSwing() as the in-world gesture (walk to the vein + attack it), so it is
    // a faithful mine action, just deterministic for automation.
    const confirmedBefore = panel.confirmedUnits;
    api.note(`driving ${MINE_SWINGS} mine swings (batch threshold ${panel.batchSize ?? "?"})`);
    let swungOk = 0;
    for (let i = 0; i < MINE_SWINGS; i += 1) {
      const ok = await clickPanelButton(client, ["挥镐", "Swing"]);
      if (ok) swungOk += 1;
      await delay(450);
    }
    api.step.swings = swungOk;
    api.note(`swing clicks accepted: ${swungOk}/${MINE_SWINGS}`);
    if (swungOk === 0) {
      api.gate(
        "the Swing control never accepted a click (it stays disabled without a connected signing wallet)",
        "a connected signing wallet so canAct is true",
        { panel },
      );
      return;
    }

    // At/above the batch threshold the client auto-submits a mine_batch. Observe the
    // outcome: either it reached the on-chain settlement boundary (mock wallet can't
    // sign+execute → recorded error) or, with a funded wallet + relayer, ore was
    // confirmed (confirmedUnits increments). Read pending/in-flight/error from the HUD.
    const settle = await waitForMineOutcome(client, confirmedBefore, MINE_BATCH_OUTCOME_TIMEOUT_MS);
    api.step.panelAfter = settle.panel;
    api.note(
      `after swings: pending=${settle.panel.pendingText ?? "—"} confirmed=${settle.panel.confirmedUnits} inFlight=${settle.panel.inFlightText ?? "—"} error=${settle.panel.lastError ?? "—"}`,
    );

    if (settle.panel.confirmedUnits > confirmedBefore) {
      // Real on-chain settlement reflected back (funded wallet + relayer present).
      api.note(`✓ on-chain settlement reflected: confirmed ore ${confirmedBefore} → ${settle.panel.confirmedUnits}`);
    } else if (settle.panel.lastError) {
      // The submit reached the on-chain boundary and stopped there — the expected,
      // documented gate without a funded testnet wallet + relayer.
      api.gate(
        `mine_batch submit reached the on-chain settlement boundary: "${truncate(settle.panel.lastError, 200)}"`,
        "a FUNDED Sui testnet wallet able to sign+execute mine_batch + the relayer/gateway /onchain/inject running (docs/ONCHAIN-M4-E2E-RUNBOOK.md)",
        { lastError: settle.panel.lastError },
      );
      api.note("✓ client-side mining state machine exercised (swings accumulated → batch submit attempted)");
    } else {
      api.gate(
        "no settlement and no batch error observed within the window (batch may not have flushed)",
        "a funded testnet wallet + relayer to drive a real settlement, or a longer --mineBatchTimeoutMs",
        { panel: settle.panel },
      );
    }
  });
}

// ---------------------------------------------------------------------------
// Token-request classification + login-outcome helpers.
// ---------------------------------------------------------------------------

// Turn the captured /api/passkey/login call (+ DOM login error) into a verdict.
function classifyTokenRequest(api, kind, outcome) {
  const call = outcome.lastCall;
  if (!call) {
    // No token request fired — the flow failed before the network call (the WebAuthn
    // ceremony for passkey, or wallet discovery/connect for wallet).
    const why = outcome.loginError ? `: ${truncate(outcome.loginError, 200)}` : "";
    if (kind === "passkey") {
      api.gate(
        `passkey ceremony did not produce a token request${why}`,
        "a working platform/virtual authenticator + @mysten/sui passkey support in this environment",
        { loginError: outcome.loginError },
      );
    } else {
      api.gate(
        `wallet flow did not produce a token request${why}`,
        "a Wallet-Standard wallet that connects + signs (real extension, or the harness mock)",
        { loginError: outcome.loginError },
      );
    }
    return;
  }

  api.note(`token request POST /api/passkey/login → HTTP ${call.status}${call.hasToken ? " (token issued)" : ""}${call.error ? ` "${truncate(call.error, 160)}"` : ""}`);

  if (call.status === 200 && call.hasToken) {
    api.note(`✓ ${kind} login token ISSUED (accountId=${call.accountId ?? "?"}) — token request fully exercised`);
    return;
  }
  if (call.status === 500) {
    // Reached token issuance — signature verified — but the server has no signing
    // secret. (For passkey this also proves the ceremony + crypto worked.)
    api.gate(
      `${kind} token request reached the server but it cannot sign tokens (HTTP 500: "${truncate(call.error, 160)}")`,
      "set MIR2_ALLOW_DEV_PASSKEY_SECRET=1 (or MIR2_PASSKEY_AUTH_SECRET) on the web server",
      { call },
    );
    return;
  }
  if (call.status === 401) {
    if (kind === "wallet") {
      // Expected, documented boundary: the mock signature is not cryptographically
      // valid. The whole client wallet pipeline ran to the server's verifier.
      api.gate(
        "wallet token request reached the server's signature verifier and was rejected (HTTP 401) — the harness mock signs with a non-cryptographic signature",
        "a REAL Sui wallet extension with a valid keypair to obtain a wallet token",
        { call },
      );
      api.note("✓ wallet client path fully exercised end-to-end (discover → connect → signPersonalMessage → POST /api/passkey/login)");
    } else {
      // For passkey a 401 means the WebAuthn-produced signature did not verify here.
      api.fail(
        "passkey signature did not verify server-side (HTTP 401) under the virtual authenticator",
        { call },
        "medium",
      );
    }
    return;
  }
  if (call.status === 400) {
    api.fail(`${kind} token request rejected as malformed (HTTP 400: "${truncate(call.error, 160)}")`, { call }, "medium");
    return;
  }
  if (call.networkError || call.status === 0) {
    api.gate(
      `${kind} token request failed at the network layer: "${truncate(call.error, 160)}"`,
      "a reachable web server hosting /api/passkey/login",
      { call },
    );
    return;
  }
  api.fail(`${kind} token request returned an unexpected status (HTTP ${call.status})`, { call }, "medium");
}

// If a token was issued, the client hands it to sendSuiLoginCommand → gateway.
// Check (best-effort) whether the client then reached select/game.
async function maybeAssertEnteredGame(ctx, api, kind, outcome, framesBefore) {
  const call = outcome.lastCall;
  if (!call || call.status !== 200 || !call.hasToken) return; // nothing to assert without a token
  const reached = await waitUntilSoft(
    ctx.client,
    `["select","game"].includes(window.__mir2Stage5?.state?.screen)`,
    ENTER_GAME_TIMEOUT_MS,
  );
  const screen = await readScreen(ctx.client);
  api.step.suiLoginScreen = screen;
  if (reached) {
    api.note(`✓ ${kind} suiLogin reached screen="${screen}" — full login path verified end-to-end`);
  } else {
    const loginFail = loginResultFrom(framesSince(ctx.client, framesBefore));
    api.gate(
      `${kind} token issued but the client did not reach the game (screen="${screen}"${loginFail != null ? `, gateway Login result=${loginFail}` : ""})`,
      "the GATEWAY must share the passkey secret (MIR2_ALLOW_DEV_PASSKEY_SECRET=1 / MIR2_PASSKEY_AUTH_SECRET) to verify the token",
      { screen, loginFail },
    );
  }
}

// Poll for a login outcome after clicking a web3 login button: a new token request
// recorded, or a DOM login error, or the client advancing past login.
async function waitForLoginOutcome(client, sinceCallCount, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastCall = null;
  let loginError = null;
  let screen = null;
  while (Date.now() < deadline) {
    const calls = await readPasskeyCalls(client);
    if (calls.length > sinceCallCount) {
      lastCall = calls[calls.length - 1];
      break;
    }
    loginError = await readLoginError(client);
    screen = await readScreen(client);
    if (loginError) break;
    if (screen === "select" || screen === "game") break;
    await delay(200);
  }
  // Give a just-fired request a beat to record its response, then re-read.
  await delay(300);
  const calls = await readPasskeyCalls(client);
  if (calls.length > sinceCallCount) lastCall = calls[calls.length - 1];
  loginError = loginError ?? (await readLoginError(client));
  screen = await readScreen(client);
  return { lastCall, loginError, screen };
}

function describeOutcome(o) {
  const parts = [];
  if (o.lastCall) parts.push(`call=HTTP ${o.lastCall.status}${o.lastCall.hasToken ? "+token" : ""}`);
  else parts.push("call=none");
  if (o.loginError) parts.push(`loginError="${truncate(o.loginError, 80)}"`);
  parts.push(`screen=${o.screen}`);
  return parts.join(" ");
}

// ---------------------------------------------------------------------------
// In-page readers.
// ---------------------------------------------------------------------------
async function readPasskeyCalls(client) {
  return (await client.evaluate(`JSON.stringify((window.__qaWeb3 && window.__qaWeb3.passkeyCalls) || [])`).then(safeJson)) ?? [];
}
async function passkeyCallCount(client) {
  // Snapshot how many token requests have fired so far, so an action's new request
  // can be distinguished from earlier ones.
  return (await client.evaluate(`((window.__qaWeb3 && window.__qaWeb3.passkeyCalls) || []).length`)) ?? 0;
}
async function readLoginError(client) {
  return client.evaluate(`
    (() => {
      // submitSuiLogin sets a raw "Passkey/Wallet login failed: …" string rendered on
      // the login screen; surface any visible "login failed" text.
      const nodes = Array.from(document.querySelectorAll("body *"));
      for (const n of nodes) {
        const t = (n.childElementCount === 0 ? n.textContent : "") || "";
        if (/login failed/i.test(t)) return t.trim().slice(0, 300);
      }
      return null;
    })()
  `);
}
async function readScreen(client) {
  return client.evaluate(`window.__mir2Stage5?.state?.screen ?? null`);
}

// Parse the OnchainMinePanel HUD text into a structured snapshot (the panel is
// presentation-only and exposes no bridge, so we read its rendered rows).
async function readOnchainPanel(client) {
  return client.evaluate(`
    (() => {
      const el = document.querySelector('[data-testid="onchain-mine-panel"]');
      if (!el) return { present: false };
      const text = (el.textContent || "").replace(/\\s+/g, " ").trim();
      const num = (re) => { const m = text.match(re); return m ? Number(m[1]) : null; };
      const grab = (re) => { const m = text.match(re); return m ? m[1].trim() : null; };
      const walletConnected = /签名钱包/.test(text);
      // The error row is the only div with word-break:break-all (the panel's red
      // error line). Match on that — NOT the hex color, which the DOM serializes to
      // rgb(255,159,159) so a "#ff9f9f" selector would never hit.
      let lastError = null;
      const errEl = el.querySelector('div[style*="break-all"]') || null;
      if (errEl) lastError = (errEl.textContent || "").trim() || null;
      return {
        present: true,
        text,
        veinText: grab(/矿脉 vein\\s*([^链攒乐]*?)(?:签名钱包|连接|攒挥|$)/) || grab(/\\(\\d+,\\d+\\)\\s*(满[^ ]*|裂[^ ]*|空[^ ]*|full[^ ]*|cracked[^ ]*|depleted[^ ]*)/i),
        walletConnected,
        walletAddress: grab(/签名钱包\\s*([0-9a-fA-Fx…]+)/),
        batchSize: num(/攒挥 pending\\s*\\d+\\s*\\/\\s*(\\d+)/),
        pendingText: grab(/(攒挥 pending\\s*\\d+\\s*\\/\\s*\\d+)/),
        pending: num(/攒挥 pending\\s*(\\d+)\\s*\\//),
        optimisticUnits: num(/乐观矿石\\(显示\\)\\s*~?(\\d+)/),
        confirmedUnits: num(/链上已确认\\s*(\\d+)\\s*矿石/) ?? 0,
        settledBatches: num(/(\\d+)\\s*批/),
        inFlightText: grab(/(在途批次\\s*[^链]*)/),
        lastError,
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// UI driving helpers.
// ---------------------------------------------------------------------------

// Click the Passkey or Wallet button in the .login-web3-actions row by intent.
// Waits for the login overlay; returns false (does NOT throw) if the web3 actions
// are absent, so callers can record a gate rather than crash.
async function clickWeb3Button(client, which) {
  await waitForSelectorSoft(client, ".login-web3-actions", 8_000);
  const ok = await client.evaluate(`
    (() => {
      const actions = document.querySelector(".login-web3-actions");
      if (!actions) return false;
      const btns = Array.from(actions.querySelectorAll("button"));
      const label = (b) => (b.textContent || "").trim();
      let target = null;
      if (${JSON.stringify(which)} === "passkey") target = btns.find((b) => /passkey/i.test(label(b))) || btns[0];
      else target = btns.find((b) => /wallet/i.test(label(b))) || btns[1];
      if (!target) return false;
      target.click();
      return true;
    })()
  `);
  await delay(150);
  return ok;
}

// Click a button inside the on-chain mine panel by any of the given label fragments.
async function clickPanelButton(client, labels) {
  return client.evaluate(`
    (() => {
      const el = document.querySelector('[data-testid="onchain-mine-panel"]');
      if (!el) return false;
      const wanted = ${JSON.stringify(labels)};
      const btns = Array.from(el.querySelectorAll("button"));
      const match = btns.find((b) => {
        const t = (b.textContent || "");
        return wanted.some((w) => t.includes(w));
      });
      if (!match || match.disabled) return false;
      match.click();
      return true;
    })()
  `);
}

// Poll the mine HUD until ore is confirmed (> before), a batch error appears, or
// the window elapses.
async function waitForMineOutcome(client, confirmedBefore, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let panel = await readOnchainPanel(client);
  while (Date.now() < deadline) {
    panel = await readOnchainPanel(client);
    if (panel.present && (panel.confirmedUnits > confirmedBefore || panel.lastError)) break;
    await delay(400);
  }
  return { panel };
}

// ---------------------------------------------------------------------------
// World-entry via password login (STEP 4) — compact port of the proven flow in
// qa-persistence.mjs. New account → login → new character → startGame → scene.
// ---------------------------------------------------------------------------
async function enterWorldViaPassword(ctx, api) {
  const client = ctx.client;
  await ensureOnLoginScreen(client, api);
  if (!(await waitForSelectorSoft(client, ".login-input.account", 8_000))) {
    api.note("login overlay never rendered (.login-input.account missing)");
    return false;
  }
  await fillInput(client, ".login-input.account", account);
  await fillInput(client, ".login-input.password", password);
  const filled = await waitUntilSoft(
    client,
    `(() => {
       const a = document.querySelector(".login-input.account");
       const p = document.querySelector(".login-input.password");
       return Boolean(a && p) && a.value === ${JSON.stringify(account)} && p.value === ${JSON.stringify(password)} &&
         window.__mir2Stage5?.state?.accountId === ${JSON.stringify(account)};
     })()`,
    6_000,
  );
  if (!filled) {
    api.note("login inputs did not accept the credentials");
    return false;
  }
  await delay(300);

  // New Account → createAccount() (opens the socket if needed).
  const rxBeforeAcct = Date.now();
  await clickSelector(client, ".login-button.account button");
  const opened = await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
  if (!opened) {
    api.note(`gateway socket never opened from the login screen (is the gateway up at ${gatewayWsUrl}?)`);
    return false;
  }
  await waitForReceivedPacket(client, ["NewAccount"], rxBeforeAcct, 8_000).catch(() => null);

  // OK → submitLogin().
  await clickSelector(client, ".login-button.ok button");
  const atSelect = await waitUntilSoft(
    client,
    "['select','game'].includes(window.__mir2Stage5?.state?.screen)",
    ENTER_GAME_TIMEOUT_MS,
  );
  if (!atSelect) {
    api.note(`login did not reach character-select (screen=${await readScreen(client)})`);
    return false;
  }

  // Ensure our character exists, then StartGame into it.
  const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
  if (!(await client.evaluate(haveOurChar))) {
    await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
    await waitUntilSoft(client, haveOurChar, 15_000);
  }
  const startIndex = await client.evaluate(
    `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
  );
  await sendCommand(client, { type: "startGame", characterIndex: startIndex });
  const inGame = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", ENTER_GAME_TIMEOUT_MS);
  if (!inGame) {
    api.note(`startGame did not enter the world (screen=${await readScreen(client)})`);
    return false;
  }
  // Let the scene settle so the in-game HUD (incl. the mine panel) has mounted.
  await waitUntilSoft(
    client,
    "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
    SCENE_READY_TIMEOUT_MS,
  );
  await delay(800);
  return true;
}

// Navigate back to a clean login screen if the client has drifted elsewhere, and
// wait for the login overlay DOM (it mounts a beat after screen flips to "login").
async function ensureOnLoginScreen(client, api) {
  const screen = await readScreen(client);
  if (screen !== "login") {
    // A fresh navigation resets to the login screen deterministically.
    await navigate(client, RUN_URL);
    await waitForStageReady(client);
  }
  await waitForSelectorSoft(client, ".login-input.account", 10_000);
  const after = await readScreen(client);
  if (after !== "login" && api) api.note(`expected login screen, got "${after}"`);
}

async function waitForStageReady(client, timeoutMs = STAGE_READY_TIMEOUT_MS) {
  await waitUntil(
    client,
    "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
    "client stage ready",
    timeoutMs,
  );
}

async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------
async function writeReport(ctx) {
  const bySeverity = { high: 0, medium: 0, low: 0 };
  const byCategory = {};
  for (const issue of ctx.issues) {
    bySeverity[issue.severity] = (bySeverity[issue.severity] ?? 0) + 1;
    byCategory[issue.category] = (byCategory[issue.category] ?? 0) + 1;
  }
  const verdicts = { pass: 0, gated: 0, fail: 0 };
  for (const s of ctx.steps) verdicts[s.verdict] = (verdicts[s.verdict] ?? 0) + 1;

  const summary = {
    runId,
    baseUrl: baseUrlRaw,
    gatewayWsUrl,
    account,
    characterName,
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    steps: ctx.steps.length,
    verdicts,
    issueCount: ctx.issues.length,
    bySeverity,
    byCategory,
    passed: bySeverity.high === 0,
  };

  const report = { summary, steps: ctx.steps, issues: ctx.issues };
  await fs.writeFile(path.join(outputDir, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
  await fs.writeFile(path.join(outputDir, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  await fs.writeFile(path.join(outputDir, "latest-web3.json"), `${JSON.stringify(report, null, 2)}\n`);
  await fs.writeFile(
    path.join(outputDir, "console.json"),
    `${JSON.stringify({ errors: ctx.client.consoleErrors, network: ctx.client.networkFailures }, null, 2)}\n`,
  );

  const md = [];
  md.push(`# Web3 QA report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrlRaw}\` → gateway \`${gatewayWsUrl}\``);
  md.push(`- Result: **${summary.passed ? "PASS" : "FAIL"}** (no high-severity issues = pass; \`gated\` tiers are not failures)`);
  md.push(`- Steps: ${summary.steps} — pass ${verdicts.pass}, gated ${verdicts.gated}, fail ${verdicts.fail}`);
  md.push(`- Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push("");
  md.push("## Step verdicts");
  md.push("");
  md.push("| # | Step | Verdict | Gate(s) |");
  md.push("|---|---|---|---|");
  for (const s of ctx.steps) {
    const gates = s.gates.length ? s.gates.map((g) => g.summary).join("; ") : "—";
    md.push(`| ${s.id} | ${escapePipes(s.title)} | ${s.verdict.toUpperCase()} | ${escapePipes(gates)} |`);
  }
  md.push("");
  md.push("## What's gated, and how to ungate it");
  md.push("");
  let anyGate = false;
  for (const s of ctx.steps) {
    for (const g of s.gates) {
      anyGate = true;
      md.push(`- **Step ${s.id}** — ${escapePipes(g.summary)}`);
      md.push(`  - needs: ${escapePipes(g.prerequisite)}`);
    }
  }
  if (!anyGate) md.push("_Nothing gated — every tier ran to completion._");
  md.push("");
  md.push("## Step detail");
  md.push("");
  for (const s of ctx.steps) {
    md.push(`### Step ${s.id} — ${s.title} → ${s.verdict.toUpperCase()}  (${s.durationMs}ms)`);
    if (s.error) md.push(`- error: \`${truncate(s.error, 300)}\``);
    for (const n of s.notes) md.push(`- ${escapePipes(n)}`);
    if (s.frame) md.push(`- frame: [\`${s.frame}\`](${s.frame})`);
    md.push("");
  }
  if (ctx.issues.length) {
    md.push("## Issues");
    md.push("");
    md.push("| # | Severity | Step | Summary |");
    md.push("|---|---|---|---|");
    const order = { high: 0, medium: 1, low: 2 };
    for (const i of [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9))) {
      md.push(`| ${i.id} | ${i.severity} | ${i.step ?? "—"} | ${escapePipes(i.summary)} |`);
    }
    md.push("");
  }
  md.push("## Prerequisites for full coverage");
  md.push("");
  md.push("```bash");
  md.push("# Web client with the web3 surface fully enabled:");
  md.push("cd mir2-web3/apps/web");
  md.push("NEXT_PUBLIC_ONCHAIN_MINE=1 \\");
  md.push("  NEXT_PUBLIC_ONCHAIN_MINE_PACKAGE_ID=0x40c9e7b726cd7ceed83d235fdfda2caa5e1fad02405969b8b2a87b92d76631c9 \\");
  md.push("  NEXT_PUBLIC_ONCHAIN_MINE_FRAMEWORK_PACKAGE_ID=0x89302436f6624fb9274ab0126737a599cb154b008687d71f6d8ce9e0d22ec3ce \\");
  md.push("  NEXT_PUBLIC_ONCHAIN_MINE_DAPP_HUB_ID=0x12a319387cf2d465d4f4181523d089a368e542fccbdd878e70a4db0df007bb78 \\");
  md.push("  NEXT_PUBLIC_ONCHAIN_MINE_DAPP_STORAGE_ID=0x927cb85c5f6110a6b0ec28f4770344f32b8ddc7f0c77108cd2c69bae9cec2eb9 \\");
  md.push("  MIR2_ALLOW_DEV_PASSKEY_SECRET=1 npm run dev    # serves on localhost:<port>");
  md.push("");
  md.push("# Gateway must share the passkey secret to verify Sui-login tokens (enter game):");
  md.push("MIR2_ALLOW_DEV_PASSKEY_SECRET=1 MIR2_GATEWAY_WEB_ADDR=127.0.0.1:7111 cargo +1.89.0 run -p mir2-gateway --bin mir2-gateway");
  md.push("");
  md.push("# Then point the harness at the localhost dev server:");
  md.push(`node ./scripts/qa-web3.mjs --headed --baseUrl ${baseUrlRaw} --gatewayWs ${gatewayWsUrl}`);
  md.push("```");
  md.push("");
  md.push("Real on-chain SETTLEMENT additionally needs a funded testnet Sui wallet + the relayer (see `docs/ONCHAIN-M4-E2E-RUNBOOK.md`).");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-persistence.mjs / qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-web3-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      ...(disableGpu ? ["--disable-gpu"] : ["--ignore-gpu-blocklist", "--enable-webgl"]),
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${VIEWPORT.width},${VIEWPORT.height}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome();
  return chrome;
}

// Open the page target at about:blank so the harness can install the document-start
// bootstrap (mock wallet + fetch hook) BEFORE the app ever loads, then navigate.
async function createPageTarget() {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  await delay(500);
  return target.webSocketDebuggerUrl;
}

async function waitForChrome() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
      if (response.ok) return;
    } catch {
      await delay(100);
    }
  }
  throw new Error("Timed out waiting for Chrome debug endpoint.");
}

async function setViewport(client, viewport) {
  await client.send("Emulation.setDeviceMetricsOverride", viewport);
  await client.send("Emulation.setVisibleSize", { width: viewport.width, height: viewport.height });
}

async function navigate(client, url) {
  let lastError;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await client.send("Page.navigate", { url });
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 20_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) return;
      lastError = new Error(`Chrome landed on ${currentUrl}`);
    } catch (error) {
      lastError = error;
    }
    await delay(500);
  }
  throw lastError ?? new Error("Page navigation failed.");
}

async function fillInput(client, selector, value) {
  const ok = await client.evaluate(`
    (() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText" }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not fill ${selector}`);
}

async function clickSelector(client, selector) {
  const ok = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!ok) throw new Error(`Could not click ${selector}`);
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(
      `(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, wsState: window.__mir2Stage5?.state?.wsState ?? null, body: document.body?.innerText?.slice(0, 200) ?? "" }))()`,
    )
    .catch((e) => ({ debugError: String(e) }));
  throw new Error(`Timed out waiting for ${label}; debug=${JSON.stringify(debug)}`);
}

async function waitUntilSoft(client, expression, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return true;
    await delay(120);
  }
  return false;
}

async function waitForSelectorSoft(client, selector, timeoutMs) {
  return waitUntilSoft(client, `document.querySelector(${JSON.stringify(selector)}) != null`, timeoutMs);
}

// Received frames captured at/after `sinceIndex` (frame-buffer index snapshot).
function framesSince(client, sinceIndex) {
  return client.webSocketFramesReceived.slice(sinceIndex);
}

async function waitForReceivedPacket(client, names, sinceTime, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    for (const f of client.webSocketFramesReceived.filter((x) => (x.at ?? 0) >= sinceTime)) {
      try {
        const o = JSON.parse(f.payloadData);
        if (o && names.includes(o.packet)) return o;
      } catch {
        /* non-JSON (HMR) frame */
      }
    }
    await delay(150);
  }
  return null;
}

// Most recent `Login` (failure) packet's result code in a frame slice, or null.
function loginResultFrom(frames) {
  for (let i = frames.length - 1; i >= 0; i -= 1) {
    try {
      const o = JSON.parse(frames[i].payloadData);
      if (o && o.packet === "Login") return o.result ?? o.payload?.result ?? "unknown";
    } catch {
      /* non-JSON frame */
    }
  }
  return null;
}

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve)).catch(() => undefined);
  if (chrome.userDataDir) {
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
function buildRunUrl(rawBaseUrl, wsUrl) {
  const url = new URL(rawBaseUrl);
  if (!url.searchParams.has("gatewayWs")) url.searchParams.set("gatewayWs", wsUrl);
  return url.toString();
}

function safeJson(s) {
  try {
    return JSON.parse(s);
  } catch {
    return null;
  }
}

function truncate(s, n) {
  s = String(s ?? "");
  return s.length > n ? `${s.slice(0, n)}…` : s;
}

function escapePipes(s) {
  return String(s ?? "").replace(/\|/g, "\\|").replace(/\n/g, " ");
}

function slug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = "true";
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

function numberArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function defaultName(prefix) {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `${prefix}${suffix}`.slice(0, 12).toUpperCase();
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
async function main() {
  await fs.mkdir(framesDir, { recursive: true });
  console.log(`qa-web3: target=${baseUrlRaw} gateway=${gatewayWsUrl} headed=${headed}`);
  console.log(`qa-web3: account=${account} char=${characterName}`);
  console.log(`qa-web3: output=${outputDir}`);

  const chrome = await launchChrome();
  let client;
  let ctx;
  try {
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await client.send("Page.enable");
    await client.send("Page.bringToFront");
    // Install the document-start bootstrap (mock wallet + fetch hook) for EVERY
    // document, before the app loads.
    await client.send("Page.addScriptToEvaluateOnNewDocument", { source: PAGE_BOOTSTRAP });
    await setViewport(client, VIEWPORT);

    ctx = createContext(client);
    ctx.startedAt = Date.now();

    await stepLoginUiPresence(ctx);
    await stepPasskeyLogin(ctx);
    await stepWalletLogin(ctx);
    await stepOnchainMine(ctx);

    await writeReport(ctx);

    const verdicts = ctx.steps.reduce((acc, s) => ((acc[s.verdict] = (acc[s.verdict] ?? 0) + 1), acc), {});
    const high = ctx.issues.filter((i) => i.severity === "high").length;
    const passed = high === 0;
    console.log(
      `\nqa-web3 ${passed ? "PASS" : "FAIL"}: steps pass ${verdicts.pass ?? 0} / gated ${verdicts.gated ?? 0} / fail ${verdicts.fail ?? 0}; ${ctx.issues.length} issue(s) (high ${high}).`,
    );
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
    if (!passed) process.exitCode = 1;
  } catch (error) {
    console.error("qa-web3 fatal:", error);
    if (ctx) {
      try {
        ctx.addIssue({ category: "flow", severity: "high", step: null, summary: "fatal harness error", detail: String(error?.message ?? error) });
        await writeReport(ctx);
      } catch {
        /* ignore */
      }
    }
    process.exitCode = 1;
  } finally {
    client?.close();
    await stopChrome(chrome).catch(() => {});
  }
}

main();
