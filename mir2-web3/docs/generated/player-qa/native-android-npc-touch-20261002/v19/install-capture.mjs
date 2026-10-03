import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync, writeFileSync, mkdirSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
const qa = dirname(fileURLToPath(import.meta.url)), out = join(qa, 'device');
mkdirSync(out, {recursive: true});
const adb = '/Users/henryliu/Library/Android/sdk/platform-tools/adb';
const serial = 'emulator-5554';
const packageProof = JSON.parse(readFileSync(join(qa, 'package-v19.json')));
assert.equal(packageProof.source, '78e7d2309f6a6b92cea1737c76c5eedfd863e46e');
const commands = [];
function run(name, args, timeout = 30_000) {
  const bytes = execFileSync(adb, ['-s', serial, ...args], {timeout, maxBuffer: 20 * 1024 * 1024});
  writeFileSync(join(out, name), bytes);
  commands.push({name, args});
  return bytes.toString();
}
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
assert.match(run('avd.txt', ['emu', 'avd', 'name']), /^Mir2_API_31_ARM64/);
run('os.txt', ['shell', 'getprop', 'ro.build.fingerprint']);
run('api.txt', ['shell', 'getprop', 'ro.build.version.sdk']);
run('display.txt', ['shell', 'wm', 'size']);
run('density.txt', ['shell', 'wm', 'density']);
for (const artifact of packageProof.artifacts) {
  const digest = createHash('sha256').update(readFileSync(artifact.path)).digest('hex');
  assert.equal(digest, artifact.sha256);
  const app = artifact.variant === 'debug' ? 'com.mir2.web3' : 'com.mir2.web3.uipreview';
  run(`${artifact.variant}-stop-before.txt`, ['shell', 'am', 'force-stop', app]);
  assert.match(run(`${artifact.variant}-install.txt`, ['install', '-r', artifact.path], 60_000), /Success/);
  assert.match(run(`${artifact.variant}-package.txt`, ['shell', 'dumpsys', 'package', app]), /versionCode=19\b/);
}
for (const [name, app, scene] of [
  ['debug-login', 'com.mir2.web3', null],
  ['npcshop', 'com.mir2.web3.uipreview', 'npcshop'],
  ['npcshop-sell', 'com.mir2.web3.uipreview', 'npcshop-sell'],
  ['npcshop-repair', 'com.mir2.web3.uipreview', 'npcshop-repair'],
  ['npcshop-srepair', 'com.mir2.web3.uipreview', 'npcshop-srepair'],
]) {
  run(`${name}-stop.txt`, ['shell', 'am', 'force-stop', app]);
  const args = ['shell', 'am', 'start', '-W', '-n', `${app}/com.mir2.web3.MainActivity`];
  if (scene) args.push('--es', 'ui_scene', scene);
  assert.match(run(`${name}-start.txt`, args), /Status: ok/);
  await delay(3_000);
  const pid = run(`${name}-pid.txt`, ['shell', 'pidof', app]).trim();
  assert.match(pid, /^\d+$/);
  run(`${name}.png`, ['exec-out', 'screencap', '-p']);
  run(`${name}-pid.log`, ['logcat', '-d', `--pid=${pid}`, '-v', 'threadtime']);
  run(`${name}-ime.txt`, ['shell', 'dumpsys', 'input_method']);
  run(`${name}-resumed.txt`, ['shell', 'dumpsys', 'activity', 'activities']);
  run(`${name}-stop-after.txt`, ['shell', 'am', 'force-stop', app]);
  console.log(JSON.stringify({name, pid, scene, source: packageProof.source}));
}
writeFileSync(join(out, 'commands.json'), JSON.stringify({source: packageProof.source,
  versionCode: 19, offlineOnly: true, physicalDeviceAccepted: false, commands}, null, 2) + '\n');
