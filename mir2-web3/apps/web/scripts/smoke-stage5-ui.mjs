import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const BASE_URL = process.env.MIR2_WEB_BASE_URL ?? process.argv[2] ?? "http://127.0.0.1:3002";
const OUTPUT_DIR = path.resolve(process.cwd(), "..", "..", "docs", "stage5-screenshots");
const CHROME_PATH = process.env.MIR2_CHROME_PATH ?? findChromePath();
const DEBUG_PORT = Number(process.env.MIR2_CHROME_DEBUG_PORT ?? 9400 + (process.pid % 1000));
const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

if (!CHROME_PATH) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH to run the Stage 5 UI smoke.");
}

class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
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
      if (message.error) {
        reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      } else {
        resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === "Runtime.consoleAPICalled" && message.params?.type === "error") {
      this.consoleErrors.push({
        source: "console",
        text: (message.params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
      });
    }

    if (message.method === "Runtime.exceptionThrown") {
      this.consoleErrors.push({
        source: "exception",
        text: message.params?.exceptionDetails?.text ?? "runtime exception",
      });
    }

    if (message.method === "Log.entryAdded") {
      const entry = message.params?.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({ source: entry.source ?? "log", text: entry.text ?? "" });
      }
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    const promise = new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
    });
    this.ws.send(payload);
    return promise;
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) {
      throw new Error(result.exceptionDetails.text ?? "Runtime.evaluate failed");
    }
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

async function main() {
  await fs.mkdir(OUTPUT_DIR, { recursive: true });
  const userDataDir = path.join(os.tmpdir(), `mir2-stage5-ui-${process.pid}-${Date.now()}`);
  const chrome = spawn(
    CHROME_PATH,
    [
      "--headless=new",
      `--remote-debugging-port=${DEBUG_PORT}`,
      `--user-data-dir=${userDataDir}`,
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      "about:blank",
    ],
    { stdio: "ignore" },
  );

  let client;
  const screenshots = [];
  try {
    await waitForChrome(DEBUG_PORT);
    const target = await createTarget(DEBUG_PORT, "about:blank");
    client = new CdpClient(target.webSocketDebuggerUrl);
    await client.connect();
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Emulation.setDeviceMetricsOverride", VIEWPORT);
    await client.send("Page.navigate", { url: BASE_URL });
    await waitForSelector(client, ".login-overlay", 15_000);
    screenshots.push(await screenshot(client, "stage5-login.png"));

    const accountId = `stage5-${process.pid}-${Date.now()}`;
    await setInputValue(client, ".login-input.account", accountId);
    await setInputValue(client, ".login-input.password", "stage5-pass");
    await clickSelector(client, ".login-button.account button");
    await delay(1_200);
    await clickSelector(client, ".login-button.ok button");
    await waitForSelector(client, ".select-overlay", 15_000);
    screenshots.push(await screenshot(client, "stage5-select.png"));

    const characterName = `S${Date.now().toString(36).slice(-8)}`;
    await sendGatewayCommand(client, {
      type: "newCharacter",
      name: characterName,
      gender: "Male",
      class: "Warrior",
    });
    await delay(500);
    await sendGatewayCommand(client, { type: "startGame", characterIndex: 0 });
    await waitForSelector(client, ".game-ui-scene", 15_000);
    await waitForSelector(client, ".hud-button.inventory button", 10_000);
    await waitForStage5State(client, (state) => state?.mapFileName === "0", "starter map", 15_000);
    screenshots.push(await screenshot(client, "stage5-game.png"));

    await clickSelector(client, ".hud-button.inventory button");
    await waitForSelector(client, ".inventory-window", 10_000);
    screenshots.push(await screenshot(client, "stage5-inventory.png"));

    await clickSelector(client, ".hud-button.character button");
    await waitForSelector(client, ".character-window", 10_000);
    screenshots.push(await screenshot(client, "stage5-character.png"));

    await clickButtonByImageAlt(client, "Store Item");
    await waitForSelector(client, ".storage-window", 10_000);
    screenshots.push(await screenshot(client, "stage5-storage.png"));

    await waitForSelector(client, ".entity-nameplate.npc", 10_000);
    await clickFirst(client, ".entity-nameplate.npc");
    await waitForSelector(client, ".npc-dialog-panel", 10_000);
    screenshots.push(await screenshot(client, "stage5-npc.png"));
    await clickOptional(client, ".npc-dialog-close");

    await waitForSelector(client, ".entity-nameplate.monster", 10_000);
    await clickFirst(client, ".entity-nameplate.monster");
    await delay(800);
    screenshots.push(await screenshot(client, "stage5-combat.png"));

    await clickAllOptional(client, ".storage-close button, .inventory-close button, .character-close button");
    await delay(300);
    await sendGatewayCommand(client, { type: "transferMap", key: "crystal:1:315:82" });
    await waitForStage5State(client, (state) => state?.mapFileName === "1", "mapFileName 1", 15_000);
    screenshots.push(await screenshot(client, "stage5-map-transfer-1.png"));

    await sendGatewayCommand(client, { type: "stage5Command", action: "group.create", args: ["Miner"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "guild.create", args: ["BichonGuard"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "social.friend", args: ["Miner"] });
    await sendGatewayCommand(client, {
      type: "stage5Command",
      action: "mail.send",
      args: ["Scout", "Reward", "Take this", "5"],
    });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mail.claim", args: ["1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.start", args: ["Trader"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.offerGold", args: ["1"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "trade.accept", args: [] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "conquest.start", args: ["Sabuk"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "conquest.owner", args: [] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "hero.recruit", args: ["Aide"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "mine", args: ["2"] });
    await sendGatewayCommand(client, { type: "stage5Command", action: "craft", args: ["crafted-blade"] });
    await waitForStage5State(
      client,
      (state) =>
        state?.stage5Systems?.guild?.name === "BichonGuard" &&
        state?.stage5Systems?.hero?.name === "Aide" &&
        state?.stage5Systems?.profession?.craftedItems?.includes("crafted-blade"),
      "stage5 broad systems",
      15_000,
    );
    screenshots.push(await screenshot(client, "stage5-systems.png"));

    if (client.consoleErrors.length > 0) {
      throw new Error(
        `Browser critical console errors:\n${client.consoleErrors
          .map((entry) => `- ${entry.source}: ${entry.text}`)
          .join("\n")}`,
      );
    }

    const stage5State = await client.evaluate("window.__mir2Stage5?.state ?? null");
    const manifest = {
      baseUrl: BASE_URL,
      generatedAt: new Date().toISOString(),
      viewport: VIEWPORT,
      screenshots,
      stage5Systems: stage5State?.stage5Systems ?? null,
      criticalConsoleErrors: client.consoleErrors,
    };
    const manifestPath = path.join(OUTPUT_DIR, "stage5-ui-smoke-manifest.json");
    await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    console.log(`Stage 5 UI smoke captured ${screenshots.length} screenshots.`);
    console.log(`Wrote ${manifestPath}`);
  } finally {
    client?.close();
    chrome.kill();
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

async function createTarget(port, url) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(url)}`, {
    method: "PUT",
  });
  if (!response.ok) {
    throw new Error(`Chrome target creation failed: ${response.status}`);
  }
  return response.json();
}

async function waitForChrome(port) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) return;
    } catch {
      await delay(200);
    }
  }
  throw new Error(`Chrome did not open CDP port ${port}`);
}

async function waitForSelector(client, selector, timeoutMs) {
  await waitUntil(
    async () => Boolean(await client.evaluate(`Boolean(document.querySelector(${JSON.stringify(selector)}))`)),
    timeoutMs,
    `selector ${selector}`,
  );
}

async function waitForText(client, selector, text, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let currentText = "";
  while (Date.now() < deadline) {
    currentText = await client.evaluate(
      `Array.from(document.querySelectorAll(${JSON.stringify(selector)})).map((node) => node.textContent).join(" | ")`,
    );
    if (currentText.includes(text)) return;
    await delay(200);
  }
  throw new Error(`Timed out waiting for text ${text} in ${selector}; current text: ${currentText}`);
}

async function waitUntil(predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await delay(200);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function clickSelector(client, selector) {
  const clicked = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click ${selector}`);
}

async function sendGatewayCommand(client, command) {
  const sent = await client.evaluate(`
    (() => {
      const api = window.__mir2Stage5;
      if (!api || typeof api.send !== "function") return false;
      return api.send(${JSON.stringify(command)}) === true;
    })()
  `);
  if (!sent) throw new Error(`Could not send gateway command ${JSON.stringify(command)}`);
}

async function waitForStage5State(client, predicate, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let state = null;
  while (Date.now() < deadline) {
    state = await client.evaluate("window.__mir2Stage5?.state ?? null");
    if (predicate(state)) return;
    await delay(200);
  }
  throw new Error(`Timed out waiting for Stage 5 state ${label}; current state: ${JSON.stringify(state)}`);
}

async function setInputValue(client, selector, value) {
  const updated = await client.evaluate(`
    (() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new Event("input", { bubbles: true }));
      return true;
    })()
  `);
  if (!updated) throw new Error(`Could not set input ${selector}`);
}

async function clickFirst(client, selector) {
  const clicked = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click first ${selector}`);
}

async function clickOptional(client, selector) {
  await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (node) node.click();
    })()
  `);
}

async function clickAllOptional(client, selector) {
  await client.evaluate(`
    (() => {
      for (const node of Array.from(document.querySelectorAll(${JSON.stringify(selector)}))) {
        node.click();
      }
    })()
  `);
}

async function clickFirstByText(client, selector, text) {
  const clicked = await client.evaluate(`
    (() => {
      const node = Array.from(document.querySelectorAll(${JSON.stringify(selector)}))
        .find((entry) => entry.textContent.includes(${JSON.stringify(text)}));
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click ${selector} containing ${text}`);
}

async function clickButtonByImageAlt(client, alt) {
  const clicked = await client.evaluate(`
    (() => {
      const image = Array.from(document.querySelectorAll("img"))
        .find((entry) => entry.alt === ${JSON.stringify(alt)});
      const button = image?.closest("button");
      if (!button) return false;
      button.click();
      return true;
    })()
  `);
  if (!clicked) throw new Error(`Could not click button with image alt ${alt}`);
}

async function screenshot(client, fileName) {
  await delay(200);
  const result = await client.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  const filePath = path.join(OUTPUT_DIR, fileName);
  await fs.writeFile(filePath, Buffer.from(result.data, "base64"));
  return path.relative(path.resolve(process.cwd(), "..", ".."), filePath).replaceAll(path.sep, "/");
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function findChromePath() {
  const candidates =
    process.platform === "win32"
      ? [
          path.join(process.env.ProgramFiles ?? "", "Google", "Chrome", "Application", "chrome.exe"),
          path.join(process.env["ProgramFiles(x86)"] ?? "", "Google", "Chrome", "Application", "chrome.exe"),
          path.join(process.env.ProgramFiles ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
          path.join(process.env["ProgramFiles(x86)"] ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
        ]
      : ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser"];
  return candidates.find((candidate) => candidate && fsSync.existsSync(candidate));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
