// Integrity plus explicitly bounded manual observations, not inferred gameplay.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
const [qa] = process.argv.slice(2);
assert(qa,'provide the exact v12 QA directory');
const read = name => fs.readFileSync(path.join(qa,name),'utf8');
const proof = JSON.parse(read('package-v12.json'));
assert.equal(proof.source,'f04a7439938579480b592b5d4e7381014a612dff');
assert.equal(read('device-api.txt').trim(),'31');
assert.match(read('device-density.txt'),/Physical density: 440/);
for(const variant of ['debug','preview']) {
  assert.match(read(`${variant}-install.log`),/Success/);
  assert.match(read(`${variant}-installed-version.txt`),/versionCode=12/);
  assert.match(read(`${variant}-installed-version.txt`),/versionName=0.1.9-player-ingress/);
  const entries = execFileSync('unzip',['-Z1',proof.artifacts.find(a=>a.variant===variant).path],
    {encoding:'utf8',maxBuffer:16*1024*1024});
  for(const index of [470,471,472,473]) assert(!entries.includes(`assets/original-ui/UI_32bit/${index}.png\n`));
}
const names = ['debug-login.png','preview-before-install-launcher-anr.png',
  'preview-after-start.png','preview-character.png','preview-character-stats1.png',
  'preview-character-stats2.png','preview-character-closed.png',
  'preview-inventory.png','preview-inventory-closed.png'];
const captures = names.map(name => {
  const png = fs.readFileSync(path.join(qa,name));
  assert.equal(png.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  const width=png.readUInt32BE(16),height=png.readUInt32BE(20);
  assert.deepEqual([width,height],name==='preview-before-install-launcher-anr.png'?[1080,2340]:[2340,1080]);
  return {name,width,height,bytes:png.length,sha256:crypto.createHash('sha256').update(png).digest('hex')};
});
const logs = fs.readdirSync(qa).filter(name=>
  /^(debug-login|preview-character-final|preview-inventory-final)-pid\d+\.log$/.test(name)).map(name=>{
  const lines = read(name).split('\n');
  return {name,capturedLines:lines.length,errorFatalPanicSignalLines:lines.filter(line=>
    /ERROR\b|panicked|FATAL EXCEPTION|Fatal signal|SIGSEGV|SIGABRT/.test(line)).length,
    missingWeightBarLines:lines.filter(line=>/Path not found: original-ui\/UI_32bit\/47[0-3]\.png/.test(line)).length};
});
assert.equal(logs.length,3);
assert(logs.some(log=>log.missingWeightBarLines>0));
assert(read('preview-character-final-pid6451.log').includes('ANDROID_PLAYER_INGRESS_OFFLINE queued owner42 hp80 mp20 gold12352 credit503 weights66/25/9 xp12.5%; NOT LIVE GAMEPLAY'));
assert(read('preview-inventory-final-pid6653.log').includes('ANDROID_PLAYER_INGRESS_OFFLINE queued owner42 hp80 mp20 gold12352 credit503 weights66/25/9 xp12.5%; NOT LIVE GAMEPLAY'));
const starts = ['start-debug.log','start-preview-character.log','start-preview-inventory.log'].map(name=>{
  const text=read(name); return {name,status:text.match(/Status: (\S+)/)?.[1],waitTimeMs:Number(text.match(/WaitTime: (\d+)/)?.[1])};
});
const result = {source:proof.source,versionCode:12,deviceApi:31,
  fingerprint:read('device-fingerprint.txt').trim(),physicalSize:read('device-size.txt').trim(),density:440,
  captures,logs,starts,observationMethod:'Manual original ADB frames and verified shared-tab/close taps; hashes certify bytes, not semantic acceptance.',
  observed:{offlineStatsI: {hp:'80/200',mp:'20/100',ac:'2-7',dc:'12-24'},
    offlineStatsII:{experience:'12.5%',bag:'66/100',wear:'25/50',hand:'9/25'},
    offlineBagAndHudGold:12352,offlineBagAvailableWeight:34,
    statsTabsAndBothClosesRespond:true,ordinaryPanelsClearPhoneHudAndThumbs:true,
    sourceMarkerCredit503:true,renderedCreditValueAccepted:false,
    originalWeightBarsAccepted:false,fullPhoneUiAccepted:false,launchPerformanceAccepted:false,
    authenticatedGameplayAccepted:false,physicalDeviceAccepted:false},
  retainedFailures:['UI_32bit/471 and473 are missing from the installed v12 preview; all four Windows weight-bar frames470..473 are absent from both APKs.',
    'Pixel Launcher ANR was retained before and after preview start; only its non-destructive Wait action was selected. A premature capture stopped at the no-preview-process guard, before a game-tab tap.',
    'Normal and character launch waits timed out; later visible frames do not erase those failures.',
    'All48dp, drag/compact/IME/login/services/live/physical and complete resource release remain open.']};
fs.writeFileSync(path.join(qa,'runtime-v12.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({source:result.source,captureCount:captures.length,logs,starts,observed:result.observed},null,2));
