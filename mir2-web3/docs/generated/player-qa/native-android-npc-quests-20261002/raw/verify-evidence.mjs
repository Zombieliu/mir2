import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {dirname,join,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const qa=dirname(fileURLToPath(import.meta.url)), mode=process.argv[2]??'working';
assert(['working','index','HEAD'].includes(mode));
const git=args=>execFileSync('git',args,{maxBuffer:32*1024*1024});
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const json=path=>JSON.parse(readFileSync(join(qa,path)));
const manifest=json('manifest.json'), diagnosis=json('diagnosis.json'), pkg=json('raw/package-baseline.json');
const pathInRepo=relative(process.cwd(),qa);
function bytes(path){
 const disk=readFileSync(join(qa,path));
 if(mode==='working')return disk;
 const stored=git(['show',(mode==='index'?':':'HEAD:')+pathInRepo+'/'+path]);
 assert(stored.equals(disk),path+' Git/disk byte mismatch');return stored;
}
assert.equal(manifest.source,'2c9ad6f96ebc47e07056abbf5a6117c278836083');
assert.equal(diagnosis.source,manifest.source);assert.equal(pkg.source,manifest.source);
assert.equal(new Set(manifest.payloads.map(row=>row.path)).size,manifest.payloads.length);
for(const row of manifest.payloads){
 assert(!row.path.includes('..'));assert(!/\.(apk|aab|jks|keystore|p12|pem)$/i.test(row.path));
 const original=bytes(row.path);assert.equal(original.length,row.bytes,row.path);assert.equal(hash(original),row.sha256,row.path);
}
const walk=dir=>readdirSync(dir,{withFileTypes:true}).flatMap(item=>{
 assert(!item.isSymbolicLink());const path=join(dir,item.name);return item.isDirectory()?walk(path):[relative(qa,path)];
});
assert.deepEqual([...walk(join(qa,'raw')),'diagnosis.json'].sort(),manifest.payloads.map(row=>row.path).sort());
git(['merge-base','--is-ancestor',diagnosis.frozenWindows,manifest.source]);
assert.equal(diagnosis.latestObservedWindows,'6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4');
const sourceChanges=git(['diff-tree','--no-commit-id','--name-only','-r',manifest.source]).toString().trim().split('\n');
assert.equal(sourceChanges.length,5);assert.deepEqual(sourceChanges.sort(),diagnosis.sourceScope.toSorted());
const gates=json('raw/baseline-source-2/gate-commands.json');
assert.equal(gates.length,6);assert.equal(pkg.sourceFiles.length,41);
for(const gate of gates){
 assert.equal(gate.exitCode,0);assert(gate.sourceUnchanged);
 for(const input of pkg.sourceFiles){
  assert.equal(gate.sourceHashesBefore[input.path],input.sha256);
  assert.equal(gate.sourceHashesAfter[input.path],input.sha256);
  assert.equal(hash(git(['show',manifest.source+':'+input.path])),input.sha256);
 }
}
for(const [name,count,ignored] of [['android-normal-final',327,0],['android-preview-final',352,0],['shared-bevy-final',1254,10]]){
 assert(readFileSync(join(qa,'raw/baseline-source-2/'+name+'.log'),'utf8').includes('test result: ok. '+count+' passed; 0 failed; '+ignored+' ignored;'));
}
const java=json('raw/baseline-source-2/java-results.json');
for(const variant of ['Debug','UiPreview']){
 const results=java.filter(row=>row.variant===variant);assert.equal(results.length,5);assert.equal(results.reduce((n,row)=>n+row.tests,0),41);
 for(const row of results){assert.equal(row.failures+row.errors+row.skipped,0);assert.equal(hash(readFileSync(join(qa,'raw/baseline-source-2/java-xml',variant,row.file))),row.sha256);}
}
for(const renderer of ['mir2-web3/third_party/bevy_render/src/view/window/mod.rs','mir2-web3/third_party/bevy_render/src/renderer/mod.rs'])
 assert(git(['show',manifest.source+':'+renderer]).equals(git(['show','277ab0a6566b5241c418844e5366865547cd03e0:'+renderer])));
for(const a of pkg.artifacts){
 const config=readFileSync(join(qa,'raw/'+a.variant+'-BuildConfig.java.txt'),'utf8');
 assert.match(config,/VERSION_CODE = 30/);assert.match(config,/MIR2_GATEWAY_URL = ""/);
 assert.match(config,new RegExp('UI_PREVIEW = '+(a.variant==='preview')));
 const badging=readFileSync(join(qa,'raw/'+a.variant+'-badging.txt'),'utf8');
 for(const pattern of [/versionCode='30'/,/sdkVersion:'31'/,/targetSdkVersion:'35'/,/native-code: 'arm64-v8a'/])assert.match(badging,pattern);
 assert.equal(readFileSync(join(qa,'raw/install-'+a.variant+'-sha-after.txt'),'utf8').trim().split(/\s+/)[0],a.sha256);
}
const installation=json('raw/installation.json');assert.equal(installation.versionCode,30);assert.equal(installation.oldVersionCode,29);assert(installation.dataPreserving);
for(const command of installation.commands)assert.equal(command.exitCode,0);
const audit=json('raw/source-and-package-baseline.json');assert.equal(audit.source,manifest.source);assert.equal(audit.selectedResourceCountEach,6647);assert.equal(audit.metadataCountEach,3);
assert(audit.rendererFilesPublishedBaselineIdentical);assert.equal(audit.fullResourcePackAccepted,false);
assert(readFileSync(join(qa,'raw/npc-red.log'),'utf8').includes('test result: FAILED. 0 passed; 3 failed;'));
assert.equal(hash(readFileSync(join(qa,'raw/npc-red-source.rs'))),json('raw/npc-red.json').phoneSourceSha256);
assert(readFileSync(join(qa,'raw/phone-suite-final.log'),'utf8').includes('test result: FAILED. 23 passed; 1 failed;'));
assert.equal(hash(readFileSync(join(qa,'raw/phone-suite-failed-source.rs'))),json('raw/phone-suite-final.json').phoneSourceSha256);
assert(readFileSync(join(qa,'raw/phone-suite-final-2.log'),'utf8').includes('test result: ok. 24 passed; 0 failed;'));
assert.equal(json('raw/phone-suite-final-2.json').phoneSourceSha256,hash(git(['show',manifest.source+':mir2-web3/apps/game-client/client-bevy/src/quest_phone.rs'])));
const previousGates=json('raw/baseline-source/gate-commands.json');
const failedPreview=previousGates.find(row=>row.name==='android-preview-final');
assert.equal(failedPreview.exitCode,101);
assert.equal(hash(readFileSync(join(qa,'raw/scene-count-failed-source.rs'))),failedPreview.sourceHashesAfter['mir2-web3/apps/game-client/platform-android/src/ui_preview.rs']);
const frames=json('raw/manual-frame-ledger.json');
assert.equal(frames.length,diagnosis.manualFrameCount);
for(const frame of frames){
 assert(frame.primaryAgentViewedOriginal);assert.equal(frame.source,manifest.source);
 const png=readFileSync(join(qa,'raw',frame.path));assert.equal(hash(png),frame.sha256);
 assert.equal(png.readUInt32BE(16),2340);assert.equal(png.readUInt32BE(20),1080);
}
const npc=json('raw/ui-npc-quests/commands.json'), confirm=json('raw/ui-quest-confirmation/commands.json'), baseline=json('raw/baseline-v30/commands.json');
for(const capture of [npc,confirm,baseline]){assert.equal(capture.versionCode,30);assert.equal(capture.source??capture.installedSource,manifest.source);assert(capture.offlineOnly);for(const command of capture.commands)assert.equal(command.exitCode,0);}
const canonical=[...baseline.scenes.map(row=>({pid:row.pid,gl506:row.latestCumulative.gl506,fatalOrPanic:row.latestCumulative.fatalOrPanic})),{pid:npc.pid,...npc.latestTotals},{pid:confirm.pid,...confirm.latestTotals}];
assert.equal(new Set(canonical.map(row=>row.pid)).size,5);
assert.equal(canonical.reduce((n,row)=>n+row.gl506,0),diagnosis.canonicalGl506);assert(canonical.every(row=>row.fatalOrPanic===0));
const geometry=json('raw/npc-geometry.json');assert.equal(geometry.controls.length,18);assert(geometry.controls.every(row=>row.widthDp>=48 && row.heightDp>=48));
assert.equal(geometry.scrollAreas.length,2);assert(geometry.fontDp.every(size=>size===14||size===16));
assert.deepEqual(json('raw/original-checkouts-before.json').records,json('raw/original-checkouts-after.json').records);
for(const field of ['realNetworkAccepted','fullResourcePackAccepted','zeroErrorRendererAccepted','physicalDeviceAccepted','wholeWindowsParityAccepted'])assert.equal(diagnosis[field],false);
assert.equal(diagnosis.goalStatus,'active');
console.log(JSON.stringify({mode,source:manifest.source,originalPayloads:manifest.payloads.length,frames:frames.length,canonicalGl506:diagnosis.canonicalGl506,result:'PASS',wholeGoalComplete:false}));

