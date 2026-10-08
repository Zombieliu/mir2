import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';

const url = process.env.MIR2_SPECTATOR_UI_URL
  || 'http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0';
const expectLive = process.env.MIR2_SPECTATOR_EXPECT_LIVE === '1';
const output = path.resolve(process.env.MIR2_SPECTATOR_UI_OUTPUT || 'artifacts/spectator-readiness');
const chrome = process.env.MIR2_CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const port = 9810 + (process.pid % 150);

async function until(check, timeout = 45_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    try { if (await check()) return; } catch { /* startup */ }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error('Spectator browser acceptance timed out');
}

async function main() {
  await fs.mkdir(output, { recursive: true });
  const profile = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-watch-readiness-'));
  const browser = spawn(chrome, ['--headless=new', `--remote-debugging-port=${port}`,
    `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', 'about:blank'],
    { stdio: 'ignore', windowsHide: true });
  let socket;
  let next = 1;
  const pending = new Map();
  const sent = [];
  const errors = [];
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = next++;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const result = await send('Runtime.evaluate', { expression, returnByValue: true });
    if (result.exceptionDetails) throw new Error('Browser evaluation failed');
    return result.result?.value;
  };
  try {
    await until(async () => (await fetch(`http://127.0.0.1:${port}/json/version`)).ok);
    const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json();
    socket = new WebSocket(target.webSocketDebuggerUrl);
    socket.addEventListener('message', ({ data }) => {
      const message = JSON.parse(String(data));
      if (message.id && pending.has(message.id)) {
        const item = pending.get(message.id);
        pending.delete(message.id);
        if (message.error) item.reject(new Error(message.error.message));
        else item.resolve(message.result || {});
      }
      if (message.method === 'Runtime.exceptionThrown') errors.push(message.params.exceptionDetails.text);
      if (message.method === 'Network.webSocketFrameSent') {
        try { sent.push(JSON.parse(message.params.response.payloadData)?.type); } catch { /* HMR/ping */ }
      }
    });
    await new Promise((resolve, reject) => {
      socket.addEventListener('open', resolve, { once: true });
      socket.addEventListener('error', reject, { once: true });
    });
    await send('Runtime.enable');
    await send('Page.enable');
    await send('Network.enable');
    await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
    await send('Page.navigate', { url });
    await until(async () => {
      const state = await evaluate('document.querySelector("[data-testid=spectator-live-state]")?.dataset.state');
      return expectLive ? state === 'live' : state === 'waiting';
    }, expectLive ? 90_000 : 45_000);
    let liveFrames = null;
    if (expectLive) {
      const first = await evaluate("JSON.parse(window.render_game_to_text?.() || '{}')");
      assert.equal(first.status?.readOnly, true);
      assert.equal(first.status?.replay?.active, false);
      assert.ok(Number.isFinite(first.status?.sequence));
      await until(async () => {
        const nextState = await evaluate("JSON.parse(window.render_game_to_text?.() || '{}')");
        const nextStatus = nextState.status;
        if (nextStatus?.sequence > first.status.sequence
            && nextStatus.capturedAtMs > first.status.capturedAtMs
            && nextState.world?.entityCount > 0 && nextState.world?.mapFileName) {
          liveFrames = { firstSequence: first.status.sequence,
            nextSequence: nextStatus.sequence, capturedAtMs: nextStatus.capturedAtMs,
            mapFileName: nextState.world.mapFileName, entityCount: nextState.world.entityCount };
          return true;
        }
        return false;
      }, 15_000);
    }
    for (const key of ['ArrowRight', 'w', '1']) {
      await send('Input.dispatchKeyEvent', { type: 'keyDown', key });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', key });
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
    const state = await evaluate(`(() => ({
      badge: document.querySelector('[data-testid="spectator-live-state"]')?.textContent,
      state: document.querySelector('[data-testid="spectator-live-state"]')?.dataset.state,
      overlay: !!document.querySelector('[data-testid="spectator-overlay"]'),
      readOnly: document.querySelector('[data-testid="spectator-read-only"]')?.textContent,
      render: JSON.parse(window.render_game_to_text?.() || '{}')
    }))()`);
    assert.equal(state.overlay, true);
    assert.equal(state.state, expectLive ? 'live' : 'waiting');
    assert.deepEqual(errors, []);
    for (const type of ['login', 'startGame', 'walk', 'run', 'attack', 'magic', 'useItem', 'keepAlive']) {
      assert.equal(sent.includes(type), false, `player command ${type} must not leave spectator UI`);
    }
    const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    const screenshot = path.join(output, expectLive ? 'spectator-live.png' : 'spectator-waiting.png');
    await fs.writeFile(screenshot, Buffer.from(shot.data, 'base64'));
    const safeUrl = new URL(url);
    safeUrl.searchParams.delete('spectateToken');
    const report = { schema: 'mir2.playtest-spectator-readiness.v1', generatedAt: new Date().toISOString(),
      url: safeUrl.toString(), expectLive, actualLiveFrameAcceptance: !!liveFrames, liveFrames, state,
      playerCommandsSent: [], screenshot, platformPublishing: 'not-run' };
    await fs.writeFile(path.join(output, 'readiness.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify({ expectLive, state: state.state, screenshot, platformPublishing: 'not-run' }));
  } finally {
    socket?.close();
    browser.kill();
    // Keep this task's isolated profile as inspectable evidence. No recursive
    // filesystem deletion or interaction with the user's browser profile.
  }
}

main().catch((error) => { console.error(error.message); process.exitCode = 1; });
