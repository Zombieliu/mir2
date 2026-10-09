#!/usr/bin/env node
// Read-only spectator footage. All profiles, frames and receipts live in a new
// output directory; this never attaches to the user's browser or desktop.
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

const defaults = {
  url: 'http://127.0.0.1:3211/spectate?spectateMap=0&bevyRuntime=0&capture=1',
  output: path.join(os.tmpdir(), 'mir2-promo-captures'), seconds: 22, fps: 10,
  width: 1440, height: 900, 'wait-ms': 45000, 'max-age-ms': 15000,
  chrome: process.env.MIR2_CHROME_PATH || (process.platform === 'win32'
    ? 'C:/Program Files/Google/Chrome/Application/chrome.exe' : 'google-chrome'),
  ffmpeg: process.env.MIR2_FFMPEG_PATH || 'ffmpeg',
  ffprobe: process.env.MIR2_FFPROBE_PATH || 'ffprobe',
  'directory-url': 'https://165.154.65.136.sslip.io/playtest/spectator/matches',
};
const help = `Usage: node tools/promo-studio/record-spectator.mjs [options]
  --url URL             Public /spectate page (capture=1 is enforced)
  --output DIRECTORY    Parent for a new timestamped capture directory (default: system temp)
  --seconds N           Wall-clock capture length, 1..60 (default 22)
  --fps N               Screenshot sampling rate, 8..12 (default 10)
  --width N --height N  Browser viewport (default 1440x900)
  --wait-ms N           Scene readiness timeout (default 45000, maximum 90000)
  --max-age-ms N        Calibrated live frame-age limit (default 15000)
  --directory-url URL   Public spectator directory for clock calibration
  --chrome PATH --ffmpeg PATH --ffprobe PATH
  --require-live        Return exit code 2 if the recorded source is not live

Outputs: spectator.mp4, evidence.json, readiness.png, sample JPEG frames,
actual-timestamp ffconcat input and encoder logs. Output is silent 30fps H.264;
it holds screenshots for their actual elapsed time and never accelerates them.
Stale/replay footage is retained and explicitly labeled in evidence.json.
Only a new owned headless Chrome/profile is opened and closed. No player
inputs, account login, replay controls, auth tokens, or director controls.
`;

function options() {
  const result = { ...defaults, 'require-live': false };
  for (let i = 2; i < process.argv.length; i++) {
    const key = process.argv[i].replace(/^--?/, '');
    if (key === 'help' || key === 'h') { console.log(help); return null; }
    if (key === 'require-live') { result[key] = true; continue; }
    if (!(key in defaults) || !process.argv[i + 1]) throw new Error(`Unknown/missing option: ${key}`);
    const value = process.argv[++i];
    result[key] = typeof defaults[key] === 'number' ? Number(value) : value;
  }
  for (const [key, min, max] of [['seconds', 1, 60], ['fps', 8, 12],
    ['width', 800, 3840], ['height', 600, 2160], ['wait-ms', 1000, 90000],
    ['max-age-ms', 1000, 60000]]) {
    if (!Number.isFinite(result[key]) || result[key] < min || result[key] > max)
      throw new Error(`${key} must be between ${min} and ${max}`);
  }
  if (!Number.isInteger(result.width) || !Number.isInteger(result.height)
    || result.width % 2 || result.height % 2) throw new Error('Viewport dimensions must be even integers');
  const url = new URL(result.url);
  if (!['http:', 'https:'].includes(url.protocol) || url.pathname.replace(/\/$/, '') !== '/spectate')
    throw new Error('Capture URL must be an HTTP(S) /spectate page');
  if (url.username || url.password || [...url.searchParams.keys()].some((key) =>
    /token|password|secret|account|auth/i.test(key))) throw new Error('Credential-bearing URLs are not supported');
  url.searchParams.set('capture', '1');
  result.url = url.toString();
  const directory = new URL(result['directory-url']);
  if (!['http:', 'https:'].includes(directory.protocol) || directory.username || directory.password
    || directory.search || !directory.pathname.endsWith('/spectator/matches'))
    throw new Error('Clock URL must be the credential-free public spectator/matches endpoint');
  return result;
}

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, Math.max(0, ms)));
function safeUrl(value) {
  try {
    const url = new URL(value);
    url.username = ''; url.password = '';
    for (const key of [...url.searchParams.keys()])
      if (/token|password|secret|account|auth/i.test(key)) url.searchParams.set(key, '[redacted]');
    return url.toString();
  } catch { return String(value).slice(0, 300); }
}

async function run(binary, args, logPath, timeoutMs = 90000) {
  return await new Promise((resolve, reject) => {
    const child = spawn(binary, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = ''; let stderr = '';
    child.stdout.on('data', (data) => { stdout += data; });
    child.stderr.on('data', (data) => { stderr += data; });
    const timer = setTimeout(() => { child.kill(); reject(new Error(`${path.basename(binary)} timed out`)); }, timeoutMs);
    child.once('error', (error) => { clearTimeout(timer); reject(error); });
    child.once('close', async (code) => {
      clearTimeout(timer);
      try {
        if (logPath) await fs.writeFile(logPath, stderr + stdout);
        if (code !== 0) reject(new Error(`${path.basename(binary)} exited ${code}: ${stderr.slice(-800)}`));
        else resolve(stdout);
      } catch (error) { reject(error); }
    });
  });
}

async function calibrate(directoryUrl) {
  const samples = [];
  for (let i = 0; i < 3; i++) {
    const startedAtMs = Date.now();
    const response = await fetch(directoryUrl, { cache: 'no-store', signal: AbortSignal.timeout(10000) });
    if (!response.ok) throw new Error(`Public directory HTTP ${response.status}`);
    const data = await response.json();
    const finishedAtMs = Date.now();
    if (!Number.isSafeInteger(data.generatedAtMs)) throw new Error('Public directory has no server clock');
    samples.push({ startedAtMs, finishedAtMs, serverAtMs: data.generatedAtMs,
      publicDelayMs: data.publicDelayMs,
      matches: (data.matches || []).map(({ mapFileName, recordingId, latestSequence, latestCapturedAtMs, playerCount, entityCount }) =>
        ({ mapFileName, recordingId, latestSequence, latestCapturedAtMs, playerCount, entityCount })) });
  }
  const lowerOffsetMs = Math.max(...samples.map((item) => item.serverAtMs - item.finishedAtMs));
  const upperOffsetMs = Math.min(...samples.map((item) => item.serverAtMs - item.startedAtMs));
  if (lowerOffsetMs > upperOffsetMs) throw new Error('Server clock changed during calibration');
  return { lowerOffsetMs, upperOffsetMs, samples };
}

// Read diagnostic projections only. This does not advance the simulation or
// dispatch DOM/game inputs. The current page can render through DOM/WebGL with
// bevyRuntime=0, so scene proof uses its real composited screenshot, not the
// potentially hidden Bevy canvas alone.
const sceneExpression = `(() => {
  let render = {};
  try { render = JSON.parse(window.render_game_to_text?.() || '{}'); } catch {}
  const event = window.__mir2GatewayEventHistory?.find(item => item.type === 'spectatorStatus');
  const status = render.status || event?.payload;
  const gate = window.__mir2SceneGate;
  const badge = document.querySelector('[data-testid="spectator-live-state"]');
  const stage = document.querySelector('.client-stage-frame');
  const rect = stage?.getBoundingClientRect();
  const visible = element => !!element && getComputedStyle(element).display !== 'none'
    && getComputedStyle(element).visibility !== 'hidden' && element.getBoundingClientRect().width > 0;
  return { observedAtMs: Date.now(), receivedAtMs: event?.at, screen: gate?.screen,
    liveState: badge?.dataset.state, badge: badge?.textContent, sceneGate: gate,
    sceneReady: !!(gate?.sceneInteractionReady && gate.hasRenderPlayer && gate.hasMapRegion),
    overlayVisible: visible(document.querySelector('[data-testid="spectator-overlay"]')),
    controlsVisible: visible(document.querySelector('[data-testid="spectator-controls-toggle"]')),
    accountTriggerVisible: visible(document.querySelector('.identity-security-trigger')),
    stage: rect ? { x: rect.x, y: rect.y, width: rect.width, height: rect.height } : null,
    canvases: [...document.querySelectorAll('.game-world-composite canvas')].map(c =>
      ({ id: c.id, className: c.className, width: c.width, height: c.height, visible: visible(c) })),
    renderedPlayer: render.player || (window.__mir2Stage5?.state ?
      { objectId: window.__mir2Stage5.state.playerObjectId } : null),
    entityCount: render.visibleEntities?.length ?? render.world?.entityCount,
    status: status ? { map: status.map, recordingId: status.recordingId,
      sequence: status.sequence, capturedAtMs: status.capturedAtMs, delayMs: status.delayMs,
      readOnly: status.readOnly, directorAuthorized: status.directorAuthorized,
      targetCount: status.targets?.length, replayActive: !!status.replay?.active } : null };
})()`;

async function main(config) {
  const name = new Date().toISOString().replace(/[:.]/g, '-') + '-' + randomUUID().slice(0, 8);
  const output = path.resolve(config.output, name);
  await fs.mkdir(path.join(output, 'frames'), { recursive: true });
  const profile = await fs.mkdtemp(path.join(output, 'chrome-profile-'));
  const evidence = { schema: 'mir2.promo-spectator-capture.v1', generatedAt: new Date().toISOString(),
    source: { url: safeUrl(config.url), directoryUrl: config['directory-url'],
      kind: 'actual-public-spectator-browser', syntheticFixture: false, captureMode: true },
    output, viewport: { width: config.width, height: config.height },
    requestedSeconds: config.seconds, samplingFps: config.fps, outputFps: 30,
    audio: 'none', publishing: 'not-run', profile, frames: [], observations: [],
    statusPackets: [], console: [], runtimeErrors: [], resourceFailures: [],
    websocketEndpoints: [], websocketSentTypes: [], playerCommandsSent: [],
    captureCssOnly: ['nextjs-portal', '.identity-security-trigger'],
    cleanup: { ownedChromeClosed: false, profileRetained: true } };
  let browser; let socket; let requestId = 1; let latestStatus; let browserClosed = false;
  const pending = new Map(); const requests = new Map();
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = requestId++;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP ${method} timeout`)); }, 15000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const result = await send('Runtime.evaluate', { expression, returnByValue: true });
    if (result.exceptionDetails) throw new Error(`Browser evaluation: ${result.exceptionDetails.text}`);
    return result.result?.value;
  };
  const observe = async () => {
    const sample = await evaluate(sceneExpression);
    if (!sample.status && latestStatus) sample.status = latestStatus;
    evidence.observations.push(sample);
    return sample;
  };
  const save = async () => fs.writeFile(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2));
  console.log(JSON.stringify({ phase: 'opening-owned-browser', output }));
  try {
    browser = spawn(config.chrome, ['--headless=new', '--remote-debugging-port=0',
      '--remote-debugging-address=127.0.0.1', `--user-data-dir=${profile}`,
      '--no-first-run', '--no-default-browser-check', '--disable-background-networking',
      '--disable-renderer-backgrounding', '--disable-background-timer-throttling',
      '--hide-scrollbars', `--window-size=${config.width},${config.height}`, 'about:blank'],
    { stdio: 'ignore', windowsHide: true });
    let spawnError;
    browser.once('error', (error) => { spawnError = error; });
    browser.once('exit', () => { browserClosed = true; });
    let port;
    const bootDeadline = performance.now() + 15000;
    while (performance.now() < bootDeadline && !port) {
      if (spawnError) throw spawnError;
      if (browserClosed) throw new Error('Owned Chrome exited during startup');
      try { port = Number((await fs.readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]); }
      catch { await pause(150); }
    }
    if (!port) throw new Error('Owned Chrome did not publish its debugging port');
    const response = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`,
      { method: 'PUT', signal: AbortSignal.timeout(5000) });
    if (!response.ok) throw new Error(`Owned Chrome target HTTP ${response.status}`);
    const target = await response.json();
    socket = new WebSocket(target.webSocketDebuggerUrl);
    socket.addEventListener('message', ({ data }) => {
      const message = JSON.parse(String(data));
      if (pending.has(message.id)) {
        const item = pending.get(message.id); pending.delete(message.id); clearTimeout(item.timer);
        if (message.error) item.reject(new Error(message.error.message)); else item.resolve(message.result || {});
      }
      const params = message.params;
      if (message.method === 'Runtime.exceptionThrown') evidence.runtimeErrors.push(params.exceptionDetails.text);
      if (message.method === 'Runtime.consoleAPICalled') evidence.console.push({ atMs: Date.now(),
        type: params.type, args: params.args.map((arg) => String(arg.value ?? arg.description ?? '').slice(0, 500)) });
      if (message.method === 'Network.requestWillBeSent') requests.set(params.requestId, safeUrl(params.request.url));
      if (message.method === 'Network.responseReceived' && params.response.status >= 400)
        evidence.resourceFailures.push({ atMs: Date.now(), url: safeUrl(params.response.url), status: params.response.status });
      if (message.method === 'Network.loadingFailed') evidence.resourceFailures.push({ atMs: Date.now(),
        url: requests.get(params.requestId), error: params.errorText, canceled: params.canceled || false });
      if (message.method === 'Network.webSocketCreated') evidence.websocketEndpoints.push(safeUrl(params.url));
      if (message.method === 'Network.webSocketFrameSent') {
        try {
          const type = JSON.parse(params.response.payloadData)?.type;
          if (type && !evidence.websocketSentTypes.includes(type)) evidence.websocketSentTypes.push(type);
          if (/^(login|passkeyLogin|register|newCharacter|deleteCharacter|startGame|walk|run|turn|chat|attack|magic|useItem|keepAlive|logOut)$/i.test(type)
            && !evidence.playerCommandsSent.includes(type)) evidence.playerCommandsSent.push(type);
        } catch { /* HMR text is not a game command. */ }
      }
      if (message.method === 'Network.webSocketFrameReceived') {
        try {
          const frame = JSON.parse(params.response.payloadData);
          if (frame.type === 'spectatorStatus') {
            const status = frame.payload;
            latestStatus = { receivedAtMs: Date.now(), map: status.map, recordingId: status.recordingId,
              sequence: status.sequence, capturedAtMs: status.capturedAtMs, delayMs: status.delayMs,
              readOnly: status.readOnly, directorAuthorized: status.directorAuthorized,
              targetCount: status.targets?.length, replayActive: !!status.replay?.active };
            evidence.statusPackets.push(latestStatus);
          }
        } catch { /* Other protocols are not spectator metadata. */ }
      }
    });
    socket.addEventListener('close', () => {
      for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error('Owned CDP socket closed')); }
      pending.clear();
    });
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Owned CDP connection timeout')), 10000);
      socket.addEventListener('open', () => { clearTimeout(timer); resolve(); }, { once: true });
      socket.addEventListener('error', () => { clearTimeout(timer); reject(new Error('Owned CDP connection error')); }, { once: true });
    });
    await send('Runtime.enable'); await send('Page.enable'); await send('Network.enable');
    await send('Emulation.setDeviceMetricsOverride', { width: config.width, height: config.height, deviceScaleFactor: 1, mobile: false });
    await send('Page.addScriptToEvaluateOnNewDocument', { source: `(() => {
      const install = () => { if (!document.documentElement || document.getElementById('promo-capture-css')) return;
        const style = document.createElement('style'); style.id = 'promo-capture-css';
        style.textContent = 'nextjs-portal,.identity-security-trigger{display:none!important}';
        document.documentElement.append(style); };
      new MutationObserver(install).observe(document, { childList: true, subtree: true }); install();
    })()` });
    await send('Page.navigate', { url: config.url });
    try { evidence.clockBefore = await calibrate(config['directory-url']); }
    catch (error) { evidence.clockBeforeError = error.message; }
    const readyDeadline = performance.now() + config['wait-ms'];
    let ready;
    while (performance.now() < readyDeadline) {
      const sample = await observe();
      if (sample.sceneReady && sample.stage?.width >= 600 && sample.canvases?.length
        && !sample.overlayVisible && !sample.controlsVisible && !sample.accountTriggerVisible) { ready = sample; break; }
      await pause(500);
    }
    const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    await fs.writeFile(path.join(output, 'readiness.png'), Buffer.from(shot.data, 'base64'));
    if (!ready) throw new Error('Actual game scene not ready; readiness screenshot and diagnostics retained');
    const crop = { x: Math.max(0, Math.floor(ready.stage.x)), y: Math.max(0, Math.floor(ready.stage.y)),
      width: Math.min(config.width, Math.floor(ready.stage.width)), height: Math.floor(ready.stage.height * 0.7) };
    const pixelText = await run(config.ffmpeg, ['-hide_banner', '-i', path.join(output, 'readiness.png'),
      '-vf', `crop=${crop.width}:${crop.height}:${crop.x}:${crop.y},signalstats,metadata=print`,
      '-frames:v', '1', '-f', 'null', '-'], path.join(output, 'scene-pixel-check.log'), 30000);
    // ffmpeg emits metadata on stderr, which is retained in the log.
    const pixelLog = pixelText + await fs.readFile(path.join(output, 'scene-pixel-check.log'), 'utf8');
    const stat = key => Number(pixelLog.match(new RegExp(`lavfi\\.signalstats\\.${key}=([0-9.]+)`))?.[1]);
    evidence.renderedSceneProof = { sceneGate: ready.sceneGate, stage: ready.stage, canvases: ready.canvases,
      method: 'actual composited browser screenshot, upper game scene luminance', crop,
      luminanceMin: stat('YMIN'), luminanceMax: stat('YMAX'), luminanceAverage: stat('YAVG') };
    evidence.renderedSceneProof.nonblank = evidence.renderedSceneProof.luminanceMax - evidence.renderedSceneProof.luminanceMin > 24
      && evidence.renderedSceneProof.luminanceAverage > 8;
    if (!evidence.renderedSceneProof.nonblank) throw new Error('Game scene screenshot is blank or cannot be verified');
    if (ready.status?.readOnly !== true || ready.status?.directorAuthorized !== false || evidence.playerCommandsSent.length)
      throw new Error('Public read-only spectator boundary could not be verified');
    console.log(JSON.stringify({ phase: 'capturing', liveState: ready.liveState, sequence: ready.status.sequence, seconds: config.seconds }));
    const step = 1000 / config.fps;
    const scheduledStart = performance.now();
    const scheduledEnd = scheduledStart + config.seconds * 1000;
    let nextSample = scheduledStart; let nextObservation = scheduledStart;
    while (performance.now() < scheduledEnd) {
      await pause(nextSample - performance.now());
      if (performance.now() >= scheduledEnd && evidence.frames.length) break;
      const startedAt = performance.now();
      const frame = await send('Page.captureScreenshot', { format: 'jpeg', quality: 92, captureBeyondViewport: false });
      const finishedAt = performance.now();
      const buffer = Buffer.from(frame.data, 'base64');
      const file = `frames/frame-${String(evidence.frames.length).padStart(5, '0')}.jpg`;
      await fs.writeFile(path.join(output, file), buffer);
      evidence.frames.push({ file, monotonicAtMs: (startedAt + finishedAt) / 2,
        observedAtMs: Date.now(), screenshotLatencyMs: finishedAt - startedAt,
        bytes: buffer.length, sha256: createHash('sha256').update(buffer).digest('hex') });
      if (finishedAt >= nextObservation) { await observe(); nextObservation = finishedAt + 1000; }
      // Missed slots are dropped, never queued or turned into accelerated video.
      nextSample = scheduledStart + (Math.floor((performance.now() - scheduledStart) / step) + 1) * step;
    }
    const endedAt = performance.now();
    await observe();
    try { evidence.clockAfter = await calibrate(config['directory-url']); }
    catch (error) { evidence.clockAfterError = error.message; }
    const firstFrameAt = evidence.frames[0]?.monotonicAtMs;
    if (!firstFrameAt || evidence.frames.length < 2) throw new Error('Not enough actual screenshot frames');
    evidence.actualWallSeconds = (endedAt - firstFrameAt) / 1000;
    const concat = ['ffconcat version 1.0'];
    for (let i = 0; i < evidence.frames.length; i++) {
      const frame = evidence.frames[i];
      frame.startSeconds = (frame.monotonicAtMs - firstFrameAt) / 1000;
      frame.holdSeconds = ((evidence.frames[i + 1]?.monotonicAtMs ?? endedAt) - frame.monotonicAtMs) / 1000;
      concat.push(`file '${frame.file}'`, 'option framerate 1000', `duration ${frame.holdSeconds.toFixed(6)}`);
    }
    concat.push(`file '${evidence.frames.at(-1).file}'`, 'option framerate 1000');
    await fs.writeFile(path.join(output, 'frames.ffconcat'), concat.join('\n') + '\n');
    const clocks = [evidence.clockBefore, evidence.clockAfter].filter(Boolean);
    const lowerOffsetMs = clocks.length ? Math.min(...clocks.map(item => item.lowerOffsetMs)) : null;
    const upperOffsetMs = clocks.length ? Math.max(...clocks.map(item => item.upperOffsetMs)) : null;
    const observations = evidence.observations.filter(item => item.observedAtMs >= evidence.frames[0].observedAtMs);
    for (const item of evidence.observations) if (Number.isFinite(item.status?.capturedAtMs) && clocks.length) {
      item.frameAgeLowerMs = item.observedAtMs + lowerOffsetMs - item.status.capturedAtMs;
      item.frameAgeUpperMs = item.observedAtMs + upperOffsetMs - item.status.capturedAtMs;
    }
    const first = observations[0]?.status; const last = observations.at(-1)?.status;
    const advanced = first && last && first.recordingId === last.recordingId
      && last.sequence > first.sequence && last.capturedAtMs > first.capturedAtMs;
    const readOnly = observations.length > 1 && observations.every(item =>
      item.status?.readOnly === true && item.status.directorAuthorized === false) && !evidence.playerCommandsSent.length;
    const agesPass = clocks.length === 2 && observations.every(item =>
      Number.isFinite(item.frameAgeUpperMs) && item.frameAgeUpperMs <= config['max-age-ms']
      && item.frameAgeLowerMs >= (item.status?.delayMs || 0) - 2);
    const allLive = observations.length > 1 && observations.every(item =>
      item.liveState === 'live' && item.sceneReady && !item.status?.replayActive);
    const replay = observations.some(item => item.status?.replayActive);
    const stale = observations.some(item => item.liveState === 'stale' || item.frameAgeLowerMs > config['max-age-ms']);
    const live = !!(advanced && readOnly && agesPass && allLive);
    evidence.freshness = { label: live ? 'live-delayed' : replay ? 'replay' : stale ? 'archived-stale' : 'unverified-or-buffering',
      liveAccepted: live, sequenceAdvanced: !!advanced, firstSequence: first?.sequence, lastSequence: last?.sequence,
      firstCapturedAtMs: first?.capturedAtMs, lastCapturedAtMs: last?.capturedAtMs,
      maxAllowedAgeMs: config['max-age-ms'], maxObservedAgeUpperMs: Math.max(...observations.map(item => item.frameAgeUpperMs || 0)),
      clockCalibratedBeforeAndAfter: clocks.length === 2, readOnly, allLive, agesPass,
      badgeStates: [...new Set(observations.map(item => item.liveState))],
      labelingRequired: live ? 'Actual public spectator gameplay; delayed feed' : 'Archived/stale or unverified spectator capture; do not label LIVE' };
    evidence.captureTiming = { method: 'actual midpoint screenshot timestamps; ffconcat holds; fps filter duplicates to 30fps',
      capturedFrames: evidence.frames.length, achievedSamplingFps: evidence.frames.length / evidence.actualWallSeconds,
      maxHoldSeconds: Math.max(...evidence.frames.map(item => item.holdSeconds)),
      minHoldSeconds: Math.min(...evidence.frames.map(item => item.holdSeconds)),
      maxScreenshotLatencyMs: Math.max(...evidence.frames.map(item => item.screenshotLatencyMs)) };
    await save();
    console.log(JSON.stringify({ phase: 'encoding', frames: evidence.frames.length, actualWallSeconds: evidence.actualWallSeconds,
      freshness: evidence.freshness.label }));
    evidence.video = path.join(output, 'spectator.mp4');
    await run(config.ffmpeg, ['-hide_banner', '-y', '-f', 'concat', '-safe', '0', '-i', path.join(output, 'frames.ffconcat'),
      '-t', evidence.actualWallSeconds.toFixed(6), '-vf', 'fps=30,format=yuv420p', '-an', '-c:v', 'libx264',
      '-preset', 'fast', '-crf', '18', '-movflags', '+faststart', evidence.video], path.join(output, 'encode.log'));
    evidence.probe = JSON.parse(await run(config.ffprobe, ['-v', 'error', '-show_format', '-show_streams', '-of', 'json', evidence.video]));
    evidence.encodedDurationSeconds = Number(evidence.probe.format.duration);
    evidence.durationPreserved = Math.abs(evidence.encodedDurationSeconds - evidence.actualWallSeconds) <= 1 / 30 + 0.005;
    if (!evidence.durationPreserved) throw new Error('Encoded duration does not preserve actual capture time');
    evidence.completed = true;
  } catch (error) {
    evidence.completed = false; evidence.error = error.message; process.exitCode = 1;
  } finally {
    if (socket?.readyState === WebSocket.OPEN && !browserClosed) {
      try { await send('Browser.close'); } catch { /* Closing Chrome normally can close CDP before its response. */ }
    }
    socket?.close();
    const deadline = performance.now() + 3000;
    while (browser && !browserClosed && performance.now() < deadline) await pause(100);
    if (browser && !browserClosed) { browser.kill(); await pause(250); }
    evidence.cleanup.ownedChromeClosed = !!browserClosed;
    await save();
  }
  if (evidence.completed && config['require-live'] && !evidence.freshness.liveAccepted) process.exitCode = 2;
  console.log(JSON.stringify({ completed: evidence.completed, video: evidence.video, evidence: path.join(output, 'evidence.json'),
    freshness: evidence.freshness?.label, actualWallSeconds: evidence.actualWallSeconds,
    encodedDurationSeconds: evidence.encodedDurationSeconds, durationPreserved: evidence.durationPreserved,
    consoleErrors: evidence.console.filter(item => item.type === 'error').length, runtimeErrors: evidence.runtimeErrors.length,
    resourceFailures: evidence.resourceFailures.length, ownedChromeClosed: evidence.cleanup.ownedChromeClosed,
    error: evidence.error }));
}

try { const config = options(); if (config) await main(config); }
catch (error) { console.error(error.message); process.exitCode = 1; }
