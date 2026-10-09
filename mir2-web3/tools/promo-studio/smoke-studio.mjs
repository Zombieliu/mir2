#!/usr/bin/env node
// Real local UI/API/MP4 verification using a new, owned headless browser.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const base = new URL(process.env.PROMO_STUDIO_URL || 'http://127.0.0.1:3921');
assert.ok(['127.0.0.1', 'localhost'].includes(base.hostname) && base.protocol === 'http:');
const here = path.dirname(fileURLToPath(import.meta.url));
const output = path.resolve(process.env.PROMO_STUDIO_UI_OUTPUT || path.join(here, 'artifacts', 'browser-qa'));
const chromePath = process.env.MIR2_CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const uploadPath = path.join(here, 'assets', 'bichon-reunion-ai.png');
const port = 9890 + process.pid % 100;
const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(fn, timeout = 30000) { const deadline = Date.now() + timeout; while (Date.now() < deadline) { if (await fn()) return; await pause(250); } throw new Error('Local studio acceptance timed out'); }
async function main() {
  await fs.mkdir(output, { recursive: true });
  const profile = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-promo-ui-'));
  const child = spawn(chromePath, ['--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check', 'about:blank'], { windowsHide: true, stdio: 'ignore' });
  const receipt = { schema: 'mir2.promo-studio-ui.v1', generatedAt: new Date().toISOString(), url: base.origin, checks: {}, errors: [], failedResources: [], publishing: 'not-run' };
  let socket; let next = 1; const pending = new Map();
  const send = (method, params = {}) => new Promise((resolve, reject) => { const id = next++; const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout ${method}`)); }, 20000); pending.set(id, { resolve, reject, timer }); socket.send(JSON.stringify({ id, method, params })); });
  const evaluate = async (expression, awaitPromise = false) => { const result = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise }); if (result.exceptionDetails) throw new Error(result.exceptionDetails.text); return result.result?.value; };
  const click = async (selector) => { const point = await evaluate(`(() => { const e=document.querySelector(${JSON.stringify(selector)});e?.scrollIntoView({block:'center',behavior:'instant'});const r=e?.getBoundingClientRect();return r&&{x:r.x+r.width/2,y:r.y+r.height/2};})()`); assert.ok(point); await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 }); await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 }); };
  const shot = async (filename) => { const result = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false }); await fs.writeFile(path.join(output, filename), Buffer.from(result.data, 'base64')); };
  try {
    await until(async () => { try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1500) })).ok; } catch { return false; } });
    const target = await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })).json(); socket = new WebSocket(target.webSocketDebuggerUrl);
    socket.addEventListener('message', ({ data }) => { const message = JSON.parse(String(data)); if (pending.has(message.id)) { const p = pending.get(message.id); pending.delete(message.id); clearTimeout(p.timer); message.error ? p.reject(new Error(message.error.message)) : p.resolve(message.result || {}); } if (message.method === 'Runtime.exceptionThrown') receipt.errors.push(message.params.exceptionDetails.text); if (message.method === 'Network.responseReceived' && message.params.response.status >= 400 && !message.params.response.url.endsWith('/favicon.ico')) receipt.failedResources.push({url: message.params.response.url, status: message.params.response.status}); });
    await new Promise((resolve, reject) => { socket.addEventListener('open', resolve, { once: true }); socket.addEventListener('error', reject, { once: true }); });
    await send('Runtime.enable'); await send('Page.enable'); await send('Network.enable');
    await send('Emulation.setDeviceMetricsOverride', {width:1440,height:1100,deviceScaleFactor:1,mobile:false}); await send('Page.navigate', {url: base.href});
    await until(() => evaluate(`document.querySelectorAll('.campaign').length === 3 && document.querySelectorAll('.asset').length >= 2 && !document.querySelector('#create-button').disabled`));
    receipt.checks.loadedRealData = true; receipt.checks.desktopNoOverflow = await evaluate('document.documentElement.scrollWidth<=innerWidth'); assert.ok(receipt.checks.desktopNoOverflow); await shot('studio-desktop.png');
    await click('[data-campaign="mine-after-work"]'); assert.equal(await evaluate('document.querySelector("#create-button").disabled'), true); assert.match(await evaluate('document.querySelector("#material-check").textContent'), /挖礦/); receipt.checks.missingMiningMaterialBlocksRender = true;
    await click('[data-campaign="return-to-bichon"]');
    const previous = await evaluate('document.querySelectorAll(".asset").length');
    await evaluate(`document.querySelector('#provenance').value='ai_creative';document.querySelector('#asset-tag').value='bichon';document.querySelector('#asset-source').value='Owned browser upload acceptance; original AI image';`);
    const root = await send('DOM.getDocument'); const input = await send('DOM.querySelector', {nodeId:root.root.nodeId,selector:'#file-input'}); await send('DOM.setFileInputFiles', { nodeId: input.nodeId, files: [uploadPath] });
    await until(() => evaluate(`document.querySelectorAll('.asset').length > ${previous} && document.querySelector('#upload-progress').hidden`)); receipt.checks.realBrowserFileUpload = true;
    // This acceptance data directory retains the original browser-margin clip.
    // Select the full game-stage derivative, preserving all gameplay/HUD edges.
    await evaluate(`for(const label of document.querySelectorAll('.asset')){if(label.textContent.includes('比奇測試服 · 真實觀戰錄影')){const input=label.querySelector('input');if(input.checked)input.click();}}`);
    await evaluate(`document.querySelector('#language').value='en';document.querySelector('#language').dispatchEvent(new Event('change',{bubbles:true}));`);
    assert.match(await evaluate('document.querySelector("#poster-title").textContent'), /adventure/); receipt.checks.englishStoryboard = true;
    await click('#brief-button'); await until(() => evaluate(`!document.querySelector('#brief-button').disabled`)); assert.match(await evaluate('document.querySelector("#notice").textContent'), /預設/); receipt.checks.truthfulTemplateMode = true;
    await evaluate(`document.querySelector('#voice').value='system';`);
    const before = await (await fetch(new URL('/api/status', base))).json();
    await click('#create-button'); let job;
    await until(async () => { const data = await (await fetch(new URL('/api/status', base))).json(); job = data.jobs.find((item) => !before.jobs.some((old) => old.id === item.id)); return !!job; }); receipt.jobId = job.id;
    assert.ok(['queued','running'].includes(job.status)); receipt.checks.realAsyncJobCreated = true;
    await until(() => evaluate('!document.querySelector("#create-button").disabled'));
    assert.equal(await evaluate('document.querySelector("#create-button").disabled'), false); receipt.checks.editorUsableDuringEncoding = true;
    await shot('studio-encoding.png'); console.log(JSON.stringify({phase:'real-encoding',jobId:job.id}));
    await until(async () => { const data = await (await fetch(new URL('/api/status', base))).json(); job = data.jobs.find((item) => item.id === job.id); if(job.status==='error')throw new Error(job.error); return job.status==='completed'; }, 240000);
    await until(() => evaluate(`document.querySelector('[data-preview="${job.id}/vertical"]') !== null`));
    await click(`[data-preview="${job.id}/vertical"]`); await until(() => evaluate(`document.querySelector('#video-preview').readyState>=2`));
    const playback = await evaluate(`(async()=>{const video=document.querySelector('#video-preview');video.muted=true;await video.play();return {width:video.videoWidth,height:video.videoHeight,duration:video.duration};})()`, true); await pause(800); assert.ok(await evaluate('document.querySelector("#video-preview").currentTime>0')); assert.equal(playback.width,720); assert.equal(playback.height,1280); receipt.checks.actualMp4Playback = playback;
    await shot('studio-playback.png');
    const range = await fetch(new URL(job.outputs[0].videoUrl,base), {headers:{Range:'bytes=0-1023'}}); assert.equal(range.status,206); assert.equal((await range.arrayBuffer()).byteLength,1024); receipt.checks.videoRange = true;
    const bundle = await (await fetch(new URL(job.outputs[0].metadataUrl,base))).json(); assert.equal(bundle.voice.mode,'system'); assert.ok(bundle.voice.installedVoices.length); assert.ok(bundle.posting.title); receipt.checks.postingAndSystemVoice = true;
    await send('Emulation.setDeviceMetricsOverride',{width:430,height:932,deviceScaleFactor:1,mobile:true}); await evaluate('window.scrollTo(0,0)'); await pause(400); receipt.checks.mobileNoOverflow = await evaluate('document.documentElement.scrollWidth<=innerWidth'); assert.ok(receipt.checks.mobileNoOverflow); await shot('studio-mobile.png');
    assert.deepEqual(receipt.errors, []); assert.deepEqual(receipt.failedResources, []); receipt.completed = true; receipt.outputs = job.outputs;
  } catch (error) {
    receipt.failure = error.message;
    try { receipt.lastNotice = await evaluate('document.querySelector("#notice")?.textContent'); await shot('studio-failure.png'); } catch {}
    throw error;
  } finally {
    try { await send('Browser.close'); } catch {} socket?.close();
    for(const p of pending.values()){clearTimeout(p.timer);p.reject(new Error('Owned browser cleanup'));} pending.clear();
    if (child.exitCode === null) await Promise.race([new Promise((r) => child.once('exit',r)),pause(5000)]); if(child.exitCode===null)child.kill(); receipt.ownedBrowserClosed = true;
    await fs.writeFile(path.join(output,'evidence.json'),JSON.stringify(receipt,null,2));
    console.log(JSON.stringify({completed:receipt.completed||false,output,evidence:'evidence.json',jobId:receipt.jobId}));
  }
}
main().catch((error)=>{console.error(error.stack);process.exitCode=1;});
