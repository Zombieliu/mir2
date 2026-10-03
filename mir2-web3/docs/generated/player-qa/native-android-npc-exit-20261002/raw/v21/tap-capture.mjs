import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const qa = dirname(fileURLToPath(import.meta.url));
const out = join(qa, 'touch-device');
mkdirSync(out, {recursive: false});
const source = process.argv[2];
assert.match(source ?? '', /^[a-f0-9]{40}$/);
const proof = JSON.parse(readFileSync(join(qa, 'package-v21.json')));
assert.equal(proof.source, source);
const adb = '/Users/henryliu/Library/Android/sdk/platform-tools/adb';
const serial = 'emulator-5554';
const app = 'com.mir2.web3.uipreview';
const preview = proof.artifacts.find(entry => entry.variant === 'preview');
assert.equal(createHash('sha256').update(readFileSync(preview.path)).digest('hex'), preview.sha256);
const commands = [];
function run(name, args, timeout = 30_000) {
  const bytes = execFileSync(adb, ['-s', serial, ...args], {timeout, maxBuffer: 24 * 1024 * 1024});
  writeFileSync(join(out, name), bytes, {flag: 'wx'});
  commands.push({name, args});
  return bytes.toString();
}
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
assert.match(run('avd.txt', ['emu', 'avd', 'name']), /^Mir2_API_31_ARM64/);
assert.match(run('api.txt', ['shell', 'getprop', 'ro.build.version.sdk']), /^31\s*$/);
// wm reports the panel's native portrait dimensions, not the rotated app frame.
assert.match(run('display.txt', ['shell', 'wm', 'size']), /Physical size: 1080x2340/);
run('density.txt', ['shell', 'wm', 'density']);
run('os.txt', ['shell', 'getprop', 'ro.build.fingerprint']);
assert.match(run('package.txt', ['shell', 'dumpsys', 'package', app]), /versionCode=21\b/);
const installedPath = run('package-path.txt', ['shell', 'pm', 'path', app]).trim().replace(/^package:/, '');
assert.match(installedPath, /^\/data\/app\/[A-Za-z0-9_+~.\-=/]+\/base\.apk$/);
assert.equal(run('installed-apk-sha256.txt', ['shell', 'sha256sum', installedPath]).trim().split(/\s+/)[0], preview.sha256);
const stages = [];
try {
  for (const [name, x, y] of [
    ['npc-ingress', 263, 319],
  ]) {
    run(`${name}-stop-before.txt`, ['shell', 'am', 'force-stop', app]);
    assert.match(run(`${name}-start.txt`, ['shell', 'am', 'start', '-W', '-n', `${app}/com.mir2.web3.MainActivity`, '--es', 'ui_scene', name]), /Status: ok/);
    await delay(3_000);
    const pid = run(`${name}-pid.txt`, ['shell', 'pidof', app]).trim();
    assert.match(pid, /^\d+$/);
    run(`${name}-before.png`, ['exec-out', 'screencap', '-p']);
    const frame = readFileSync(join(out, `${name}-before.png`));
    assert.equal(frame.readUInt32BE(16), 2340);
    assert.equal(frame.readUInt32BE(20), 1080);
    run(`${name}-before-pid.log`, ['logcat', '-d', `--pid=${pid}`, '-v', 'threadtime']);
    run(`${name}-tap.txt`, ['shell', 'input', 'tap', String(x), String(y)]);
    await delay(2_000);
    run(`${name}-after.png`, ['exec-out', 'screencap', '-p']);
    run(`${name}-after-pid.log`, ['logcat', '-d', `--pid=${pid}`, '-v', 'threadtime']);
    run(`${name}-ime.txt`, ['shell', 'dumpsys', 'input_method']);
    run(`${name}-resumed.txt`, ['shell', 'dumpsys', 'activity', 'activities']);
    run(`${name}-stop-after.txt`, ['shell', 'am', 'force-stop', app]);
    const stage = {name, pid: Number(pid), tap: {x, y}, source, screenshotBefore: `${name}-before.png`, screenshotAfter: `${name}-after.png`};
    stages.push(stage);
    console.log(JSON.stringify(stage));
  }
} finally {
  run('preview-final-stop.txt', ['shell', 'am', 'force-stop', app]);
  run('debug-final-stop.txt', ['shell', 'am', 'force-stop', 'com.mir2.web3']);
  writeFileSync(join(out, 'commands.json'), JSON.stringify({source, versionCode: 21,
    apkSha256: preview.sha256, offlineOnly: true, physicalDeviceAccepted: false,
    noTransactionButtonTapped: true, stages, commands}, null, 2) + '\n', {flag: 'wx'});
}
