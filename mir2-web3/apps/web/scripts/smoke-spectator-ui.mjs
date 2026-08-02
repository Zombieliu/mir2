import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const baseUrl = process.env.MIR2_SPECTATOR_UI_URL
  ?? "http://127.0.0.1:3000/spectate?spectateToken=local-spectator-director&spectateDelayMs=0&spectateMode=director";
const outputDir = path.resolve(process.env.MIR2_SPECTATOR_UI_OUTPUT ?? "artifacts/spectator");
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = Number(process.env.MIR2_CHROME_DEBUG_PORT ?? 9600 + (process.pid % 300));

class CdpClient {
  constructor(url) {
    this.url = url;
    this.nextId = 1;
    this.pending = new Map();
    this.errors = [];
  }

  async connect() {
    this.ws = new WebSocket(this.url);
    this.ws.addEventListener("message", (event) => this.onMessage(event.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  onMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id && this.pending.has(message.id)) {
      const pending = this.pending.get(message.id);
      this.pending.delete(message.id);
      return message.error
        ? pending.reject(new Error(message.error.message))
        : pending.resolve(message.result ?? {});
    }
    if (message.method === "Runtime.exceptionThrown") {
      this.errors.push(message.params?.exceptionDetails?.exception?.description ?? message.params?.exceptionDetails?.text);
    }
    if (message.method === "Runtime.consoleAPICalled" && message.params?.type === "error") {
      this.errors.push((message.params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "));
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const promise = new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
    this.ws.send(JSON.stringify({ id, method, params }));
    return promise;
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.text ?? "evaluation failed");
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

async function waitUntil(check, label, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await check()) return;
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`timed out waiting for ${label}`);
}

async function main() {
  if (!chromePath) throw new Error("Chrome not found; set MIR2_CHROME_PATH");
  await fs.mkdir(outputDir, { recursive: true });
  const userDataDir = path.join(os.tmpdir(), `mir2-spectator-ui-${process.pid}-${Date.now()}`);
  const chrome = spawn(chromePath, [
    "--headless=new",
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${userDataDir}`,
    "--disable-gpu",
    "--no-first-run",
    "--no-default-browser-check",
    "about:blank",
  ], { stdio: "ignore" });
  let client;
  try {
    await waitUntil(async () => {
      try {
        return (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).ok;
      } catch {
        return false;
      }
    }, "Chrome CDP");
    const response = await fetch(
      `http://127.0.0.1:${debugPort}/json/new?${encodeURIComponent(baseUrl)}`,
      { method: "PUT" },
    );
    if (!response.ok) throw new Error(`create Chrome target failed: ${response.status}`);
    const target = await response.json();
    client = new CdpClient(target.webSocketDebuggerUrl);
    await client.connect();
    await client.send("Page.enable");
    await client.send("Runtime.enable");
    await client.send("Emulation.setDeviceMetricsOverride", {
      width: 1440,
      height: 900,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await waitUntil(
      () => client.evaluate("Boolean(document.querySelector('[data-testid=\"spectator-overlay\"]'))"),
      "spectator overlay",
    );
    await waitUntil(
      () => client.evaluate("window.render_game_to_text?.().includes('\"connection\":\"open\"') === true"),
      "open spectator socket",
    );
    await client.evaluate("window.advanceTime?.(500)");
    const ui = await client.evaluate(`(() => {
      const status = JSON.parse(window.render_game_to_text());
      const map = document.querySelector('[data-testid="spectator-map"]');
      const target = document.querySelector('[data-testid="spectator-target"]');
      return {
        status,
        readOnlyLabel: document.querySelector('[data-testid="spectator-read-only"]')?.textContent?.trim(),
        mapOptions: Array.from(map?.options ?? []).map((option) => option.textContent?.trim()),
        targetOptions: Array.from(target?.options ?? []).map((option) => option.textContent?.trim()),
        directorVisible: Boolean(document.querySelector('[data-testid="spectator-director"]')),
        replayVisible: Boolean(document.querySelector('[data-testid="spectator-replay"]')),
      };
    })()`);
    if (ui.readOnlyLabel !== "● 只读安全") throw new Error(`unexpected read-only label: ${ui.readOnlyLabel}`);
    if (ui.status?.mode !== "spectator" || ui.status?.status?.readOnly !== true) {
      throw new Error(`render_game_to_text missing spectator state: ${JSON.stringify(ui.status)}`);
    }
    if (!ui.directorVisible) throw new Error("director control is missing");

    await client.evaluate("document.querySelector('[data-testid=\"spectator-director\"]')?.click()");
    await waitUntil(
      () => client.evaluate("JSON.parse(window.render_game_to_text()).status?.director === false"),
      "director toggle",
    );

    const capture = await client.send("Page.captureScreenshot", {
      format: "png",
      captureBeyondViewport: false,
    });
    const screenshotPath = path.join(outputDir, "spectator-ui.png");
    await fs.writeFile(screenshotPath, Buffer.from(capture.data, "base64"));
    if (client.errors.length) throw new Error(`browser errors: ${client.errors.join(" | ")}`);

    const report = {
      schema: "obelisk.mir2.spectator-ui-smoke.v1",
      generatedAt: new Date().toISOString(),
      url: baseUrl.replace(/spectateToken=[^&]+/, "spectateToken=[redacted]"),
      readOnlyLabel: ui.readOnlyLabel,
      mapOptions: ui.mapOptions,
      targetOptions: ui.targetOptions,
      directorVisible: ui.directorVisible,
      replayVisible: ui.replayVisible,
      renderState: ui.status,
      criticalBrowserErrors: client.errors,
      screenshot: path.relative(process.cwd(), screenshotPath),
    };
    await fs.writeFile(
      path.join(outputDir, "spectator-ui-smoke.json"),
      `${JSON.stringify(report, null, 2)}\n`,
    );
    console.log(JSON.stringify(report, null, 2));
  } finally {
    client?.close();
    chrome.kill();
    await fs.rm(userDataDir, { recursive: true, force: true }).catch(() => {});
  }
}

function findChromePath() {
  const candidates =
    process.platform === "darwin"
      ? [
          "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
          "/Applications/Chromium.app/Contents/MacOS/Chromium",
          "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ]
      : process.platform === "win32"
        ? [
            path.join(process.env.ProgramFiles ?? "", "Google", "Chrome", "Application", "chrome.exe"),
            path.join(process.env.ProgramFiles ?? "", "Microsoft", "Edge", "Application", "msedge.exe"),
          ]
        : ["/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser"];
  return candidates.find((candidate) => candidate && fsSync.existsSync(candidate));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
