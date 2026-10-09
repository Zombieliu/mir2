import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';

const url = process.env.MIR2_SPECTATOR_UI_URL
  || 'http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0';
const expectLive = process.env.MIR2_SPECTATOR_EXPECT_LIVE === '1';
const expectAutoFollow = process.env.MIR2_SPECTATOR_EXPECT_AUTO_FOLLOW === '1';
const captureMode = new URL(url).searchParams.get('capture') === '1';
const maxFrameAgeMs = process.env.MIR2_SPECTATOR_UI_MAX_AGE_MS
  ? Number(process.env.MIR2_SPECTATOR_UI_MAX_AGE_MS) : null;
const directoryUrl = (process.env.MIR2_SPECTATOR_HTTP_BASE_URL
  || 'https://165.154.65.136.sslip.io/playtest') + '/spectator/matches';
const output = path.resolve(process.env.MIR2_SPECTATOR_UI_OUTPUT || 'artifacts/spectator-readiness');
const chrome = process.env.MIR2_CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const port = 9810 + (process.pid % 150);

async function calibrateClock() {
  const samples = [];
  for (let index = 0; index < 5; index++) {
    const startedAtMs = Date.now();
    const response = await fetch(directoryUrl, { cache: 'no-store', signal: AbortSignal.timeout(10_000) });
    assert.equal(response.status, 200);
    const data = await response.json();
    const finishedAtMs = Date.now();
    assert.ok(Number.isSafeInteger(data.generatedAtMs));
    samples.push({ serverAtMs: data.generatedAtMs, startedAtMs, finishedAtMs,
      roundTripMs: finishedAtMs - startedAtMs, publicDelayMs: data.publicDelayMs });
  }
  // The server stamp must lie between local send and receive. Intersect these
  // bounds rather than assuming either machine's clock is already correct.
  const lowerOffsetMs = Math.max(...samples.map((sample) => sample.serverAtMs - sample.finishedAtMs));
  const upperOffsetMs = Math.min(...samples.map((sample) => sample.serverAtMs - sample.startedAtMs));
  assert.ok(lowerOffsetMs <= upperOffsetMs, 'Clock changed during calibration');
  return { lowerOffsetMs, upperOffsetMs, samples };
}

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
  const resourceFailures = [];
  const websocketEndpoints = [];
  let status = null;
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = next++;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression, awaitPromise = false) => {
    const result = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise });
    if (result.exceptionDetails) throw new Error('Browser evaluation failed');
    return result.result?.value;
  };
  const click = async (testId) => {
    const point = await evaluate(`(() => {
      const box = document.querySelector('[data-testid="${testId}"]')?.getBoundingClientRect();
      return box && { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    })()`);
    assert.ok(point, `Missing clickable control: ${testId}`);
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
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
      if (message.method === 'Network.webSocketFrameReceived') {
        try {
          const frame = JSON.parse(message.params.response.payloadData);
          if (frame.type === 'spectatorStatus') status = frame.payload;
        } catch { /* HMR/ping */ }
      }
      if (message.method === 'Network.webSocketCreated') {
        const endpoint = new URL(message.params.url);
        endpoint.searchParams.delete('token');
        endpoint.searchParams.delete('spectateToken');
        websocketEndpoints.push(endpoint.toString());
      }
      if (message.method === 'Network.responseReceived' && message.params.response.status >= 400) {
        const response = message.params.response;
        const resourceUrl = new URL(response.url);
        resourceUrl.searchParams.delete('spectateToken');
        resourceFailures.push({ url: resourceUrl.toString(), status: response.status });
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
    try {
      await until(async () => {
        const state = await evaluate('document.querySelector("[data-testid=spectator-live-state]")?.dataset.state');
        return expectLive ? state === 'live' : state === 'waiting';
      }, expectLive ? 90_000 : 45_000);
    } catch (error) {
      const diagnostic = await evaluate(`({ gate: window.__mir2SceneGate,
        render: JSON.parse(window.render_game_to_text?.() || '{}'),
        liveState: document.querySelector('[data-testid=spectator-live-state]')?.dataset.state,
        badge: document.querySelector('[data-testid=spectator-live-state]')?.textContent })`);
      const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
      await fs.writeFile(path.join(output, 'connection-not-ready.png'), Buffer.from(shot.data, 'base64'));
      await fs.writeFile(path.join(output, 'connection-not-ready.json'), JSON.stringify({
        diagnostic, status, websocketEndpoints, resourceFailures, errors, sent,
      }, null, 2));
      throw error;
    }
    let liveFrames = null;
    if (expectLive) {
      const first = status;
      assert.equal(first?.readOnly, true);
      assert.equal(first?.replay?.active, false);
      assert.ok(Number.isFinite(first?.sequence));
      await until(async () => {
        const nextStatus = status;
        if (nextStatus?.sequence > first.sequence
            && nextStatus.capturedAtMs > first.capturedAtMs) {
          liveFrames = { firstSequence: first.sequence,
            nextSequence: nextStatus.sequence, capturedAtMs: nextStatus.capturedAtMs,
            mapFileName: nextStatus.map };
          return true;
        }
        return false;
      }, 15_000);
      try {
        await until(async () => await evaluate(`(() => {
          const gate = window.__mir2SceneGate;
          return gate?.sceneInteractionReady && gate.hasRenderPlayer && gate.hasMapRegion;
        })()`), 90_000);
      } catch (error) {
        const diagnostic = await evaluate(`({ gate: window.__mir2SceneGate,
          render: JSON.parse(window.render_game_to_text?.() || '{}') })`);
        const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
        await fs.writeFile(path.join(output, 'scene-not-ready.png'), Buffer.from(shot.data, 'base64'));
        await fs.writeFile(path.join(output, 'scene-not-ready.json'), JSON.stringify({
          actualLiveFrameAcceptance: true, visualSceneAcceptance: false,
          liveFrames, diagnostic, resourceFailures, errors,
        }, null, 2));
        throw error;
      }
    }
    let frameLatency = null;
    if (maxFrameAgeMs !== null) {
      assert.equal(expectLive, true, 'Frame age measurement needs a live source');
      assert.ok(Number.isFinite(maxFrameAgeMs) && maxFrameAgeMs >= 1_000 && maxFrameAgeMs <= 60_000);
      const beforeClock = await calibrateClock();
      const observations = [];
      const endAtMs = Date.now() + 30_000;
      let previousSequence = null;
      while (Date.now() < endAtMs) {
        const observed = await evaluate(`new Promise((resolve) => requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            const event = window.__mir2GatewayEventHistory?.find((item) => item.type === 'spectatorStatus');
            const current = event?.payload;
            const gate = window.__mir2SceneGate;
            resolve({ observedAtMs: Date.now(), receivedAtMs: event?.at,
              sequence: current?.sequence, capturedAtMs: current?.capturedAtMs,
              delayMs: current?.delayMs, map: current?.map, readOnly: current?.readOnly,
              directorAuthorized: current?.directorAuthorized,
              sceneReady: gate?.sceneInteractionReady && gate.hasRenderPlayer && gate.hasMapRegion,
              liveState: document.querySelector('[data-testid=spectator-live-state]')?.dataset.state });
          })))`, true);
        if (Number.isFinite(observed.sequence) && observed.sequence !== previousSequence) {
          assert.equal(observed.sceneReady, true);
          assert.equal(observed.liveState, 'live');
          assert.equal(observed.readOnly, true);
          assert.equal(observed.directorAuthorized, false);
          assert.ok(Number.isFinite(observed.capturedAtMs) && Number.isFinite(observed.receivedAtMs));
          observations.push(observed);
          previousSequence = observed.sequence;
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      const afterClock = await calibrateClock();
      const lowerOffsetMs = Math.min(beforeClock.lowerOffsetMs, afterClock.lowerOffsetMs);
      const upperOffsetMs = Math.max(beforeClock.upperOffsetMs, afterClock.upperOffsetMs);
      const midpointOffsetMs = (lowerOffsetMs + upperOffsetMs) / 2;
      for (const item of observations) {
        item.ageEstimateMs = item.observedAtMs + midpointOffsetMs - item.capturedAtMs;
        item.ageLowerBoundMs = item.observedAtMs + lowerOffsetMs - item.capturedAtMs;
        item.ageUpperBoundMs = item.observedAtMs + upperOffsetMs - item.capturedAtMs;
      }
      const ages = observations.map((item) => item.ageEstimateMs).sort((a, b) => a - b);
      frameLatency = { scope: 'server capture to browser-applied frame metadata after two animation frames with ready scene',
        platformPublishing: 'not-run', thresholdMs: maxFrameAgeMs,
        sampleCount: observations.length, clock: { before: beforeClock, after: afterClock },
        minEstimateMs: ages[0], maxEstimateMs: ages.at(-1),
        p95EstimateMs: ages[Math.max(0, Math.ceil(ages.length * 0.95) - 1)],
        maxUpperBoundMs: Math.max(...observations.map((item) => item.ageUpperBoundMs)), observations };
      frameLatency.passed = observations.length >= 20
        && observations.every((item) => item.ageLowerBoundMs >= item.delayMs - 2
          && item.ageUpperBoundMs <= maxFrameAgeMs);
      await fs.writeFile(path.join(output, 'frame-latency.json'), JSON.stringify(frameLatency, null, 2));
      assert.equal(frameLatency.passed, true, 'Measured browser frame age exceeded the target or enforced minimum');
    }
    const controls = { captureMode, defaultCollapsed: true, expandedAndClosed: false };
    let autoFollow = null;
    assert.equal(await evaluate("!!document.querySelector('[data-testid=spectator-overlay]')"), false);
    if (captureMode) {
      assert.equal(await evaluate("!!document.querySelector('[data-testid=spectator-controls-toggle]')"), false);
    } else {
      assert.equal(await evaluate("document.querySelector('[data-testid=spectator-controls-toggle]')?.getAttribute('aria-expanded')"), 'false');
      await click('spectator-controls-toggle');
      await until(async () => await evaluate("!!document.querySelector('[data-testid=spectator-overlay]')"));
      assert.ok(await evaluate("document.querySelector('[data-testid=spectator-read-only]')?.textContent?.includes('只读安全')"));
      if (expectAutoFollow) {
        assert.equal(expectLive, true);
        const selection = () => evaluate(`({
          objectId: document.querySelector('[data-testid=spectator-auto-follow]')?.dataset.objectId,
          mode: document.querySelector('[data-testid=spectator-auto-follow]')?.dataset.mode,
          renderPlayerId: window.__mir2Stage5?.state.playerObjectId })`);
        await until(async () => (await selection()).mode === 'automatic');
        const automatic = await selection();
        assert.equal(automatic.objectId, automatic.renderPlayerId);
        const ordinaryTarget = status.targets.find((target) => String(target.objectId) === automatic.objectId);
        assert.ok(ordinaryTarget, 'Selected camera must belong to a delivered player target');
        await evaluate(`(() => { const select = document.querySelector('[data-testid=spectator-target]');
          select.value = ${JSON.stringify(ordinaryTarget.name)};
          select.dispatchEvent(new Event('change', { bubbles: true })); })()`);
        await until(async () => (await selection()).mode === 'manual');
        const manual = await selection();
        assert.equal(manual.objectId, String(ordinaryTarget.objectId));
        assert.equal(manual.renderPlayerId, manual.objectId);
        await evaluate(`(() => { const select = document.querySelector('[data-testid=spectator-target]');
          select.value = ''; select.dispatchEvent(new Event('change', { bubbles: true })); })()`);
        await until(async () => (await selection()).mode === 'automatic');
        autoFollow = { scope: 'real public delayed frame and ordinary manual control acknowledgements',
          automatic, manual, returned: await selection(), bossPkAcceptance: false };
      }
      const captureHref = await evaluate("document.querySelector('[data-testid=spectator-capture-link]')?.href");
      assert.equal(new URL(captureHref).searchParams.get('capture'), '1');
      await click('spectator-controls-toggle');
      await until(async () => !(await evaluate("!!document.querySelector('[data-testid=spectator-overlay]')")));
      controls.expandedAndClosed = true;
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
      sceneGate: window.__mir2SceneGate,
      render: JSON.parse(window.render_game_to_text?.() || '{}')
    }))()`);
    assert.equal(state.overlay, false);
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
      url: safeUrl.toString(), expectLive, actualLiveFrameAcceptance: !!liveFrames,
      visualSceneAcceptance: expectLive && state.sceneGate?.sceneInteractionReady === true,
      liveFrames, state, controls, autoFollow, frameLatency, resourceFailures,
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
