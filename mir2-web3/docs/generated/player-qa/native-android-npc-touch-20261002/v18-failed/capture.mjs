import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const adb = '/Users/henryliu/Library/Android/sdk/platform-tools/adb';
const serial = 'emulator-5554';
const app = 'com.mir2.web3.uipreview';
const commands = [];
mkdirSync(root, { recursive: true });
function run(name, args, binary = false) {
  const bytes = execFileSync(adb, ['-s', serial, ...args], { timeout: 30_000,
    maxBuffer: 20 * 1024 * 1024 });
  writeFileSync(join(root, name), bytes);
  commands.push({ name, args });
  return binary ? bytes : bytes.toString();
}
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const avd = run('avd.txt', ['emu', 'avd', 'name']);
if (!avd.startsWith('Mir2_API_31_ARM64')) throw new Error('Wrong task AVD');
const pkg = run('package.txt', ['shell', 'dumpsys', 'package', app]);
if (!/versionCode=18\b/.test(pkg)) throw new Error('This diagnostic requires the original installed v18');
run('display.txt', ['shell', 'wm', 'size']);
run('density.txt', ['shell', 'wm', 'density']);
run('stop-before.txt', ['shell', 'am', 'force-stop', app]);
run('start.txt', ['shell', 'am', 'start', '-W', '-n', `${app}/com.mir2.web3.MainActivity`,
  '--es', 'ui_scene', 'npcshop-srepair']);
await delay(3_000);
const pid = run('pid.txt', ['shell', 'pidof', app]).trim();
if (!/^\d+$/.test(pid)) throw new Error('Expected one native application PID');
async function capture(name) {
  run(`${name}.png`, ['exec-out', 'screencap', '-p'], true);
  run(`${name}-pid.log`, ['logcat', '-d', `--pid=${pid}`, '-v', 'threadtime']);
  run(`${name}-resumed.txt`, ['shell', 'dumpsys', 'activity', 'activities']);
}
await capture('before');
run('tap.txt', ['shell', 'input', 'tap', '1278', '158']);
await delay(2_000);
await capture('after-short-tap');
run('hold.txt', ['shell', 'input', 'swipe', '1278', '158', '1278', '158', '600']);
await delay(2_000);
await capture('after-600ms-hold');
run('stop-after.txt', ['shell', 'am', 'force-stop', app]);
writeFileSync(join(root, 'commands.json'), JSON.stringify({
  apkSource: 'aa1f2c4ada37ed880b71da5168d0b080583524a3', installedVersion: 18,
  offlineOnly: true, closeTargetPhysicalPixels: [1278, 158], commands,
}, null, 2) + '\n');
console.log(JSON.stringify({ root, pid, captured: 3, assertions: 'Original frames require visual inspection' }));
