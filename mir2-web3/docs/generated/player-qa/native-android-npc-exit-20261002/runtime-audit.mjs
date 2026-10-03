import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
const qa='mir2-web3/docs/generated/player-qa/native-android-npc-exit-20261002/';
const read=file=>readFileSync(qa+file,'utf8');
const source='545a34abb34415e61a6dd2f274f48914f0d0b698';
const artifact=JSON.parse(read('raw/v21/package-v21.json'));
assert.equal(artifact.source,source);
for(const directory of ['touch-device','menu-device']){
 const touch=JSON.parse(read('raw/v21/'+directory+'/commands.json'));
 assert.equal(touch.source,source);assert.equal(touch.versionCode,21);
 assert.equal(touch.apkSha256,artifact.artifacts.find(f=>f.variant==='preview').sha256);
 assert.deepEqual(touch.stages.map(s=>[s.name,s.tap.x,s.tap.y]),[['npc-ingress',263,319]]);
 if(directory==='menu-device')assert.deepEqual(touch.stages[0].menuTap,{x:2210,y:958});
}
const pids=new Set(),entries=[];
for(const [directory,scene,suffix] of [
 ['device','debug-login',''],['device','npc-ingress',''],
 ['touch-device','npc-ingress','-after'],['menu-device','npc-ingress','-menu'],
]){
 const pid=Number(read('raw/v21/'+directory+'/'+scene+'-pid.txt').trim());
 assert(Number.isInteger(pid)&&pid>0&&!pids.has(pid));pids.add(pid);
 const path='raw/v21/'+directory+'/'+scene+suffix+'-pid.log';
 const lines=read(path).split(/\r?\n/);
 const producer=lines.filter(l=>/\sI event\s/.test(l)&&l.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
 if(scene!=='debug-login'){
  assert.equal(producer.length,1);
  assert.match(producer[0],/count = 1; quest_id = Some\(2100004\)/);
  assert(producer[0].includes('dialog_open = true'));
 }
 entries.push({directory,scene,pid,log:path,
  gl506:lines.filter(l=>l.includes('GL error 0x506')).length,
  missingPaths:lines.filter(l=>l.includes('Path not found')).length,
  fatalOrPanic:lines.filter(l=>/FATAL EXCEPTION|panicked at|Fatal signal|thread .*panicked/.test(l)),producer});
}
const manifest=JSON.parse(read('manifest.json'));
const frames=manifest.files.filter(file=>file.file.endsWith('.png'));
assert.equal(frames.length,7);
for(const frame of frames){
 const bytes=readFileSync(qa+frame.file);
 assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);
}
assert.deepEqual(entries.map(e=>e.gl506),[33,0,0,0]);
assert.equal(entries.reduce((n,e)=>n+e.missingPaths,0),0);
assert.equal(entries.flatMap(e=>e.fatalOrPanic).length,0);
const proof={source,versionCode:21,distinctPids:4,entries,gl506:33,
 logCounting:'One latest cumulative log per distinct PID; canonical event only; no before/after double count',
 originalFramesInspectedByPrimary:7,
 screenshotObservations:{normalLogin:'Empty credentials; Test server not configured',
  receivedNpcDialogue:'PASS bounded title/body/Exit visible',
  actualNpcExitTap:'PASS dialogue disappears in two fresh processes',
  hudRestorationAfterNpcExit:'PASS bounded HP/MP, belt, joystick and phone actions actually visible in both after frames',
  restoredMenuButtonTap:'PASS actual Menu tap opens phone panel rail',
  questPhoneReadability:'NOT RERUN or repaired here; v20 narrow/small text FAIL preserved'},
 cause:'Feature-only npc-ingress pinned a manual NpcDialog UiPanel; its closed model still blocked shared action/HUD guards',
 fix:'Only received-preview panel initialization releases pin. Open dialogue remains modal; legacy npc specimen unchanged',
 sourceRegression:'Compiled 0/1 red, then 1/1 green; included in 338 preview tests',
 zeroErrorRendererGate:'FAIL',rendererFixThisLeaf:false,olderV20Gl506:73,olderV19Gl506:135,olderEvidenceUnchanged:true,
 actualJavaJniAuthenticatedQuest:false,questOrRewardSuccess:false,realHttpsWss:false,zoneSave:false,
 fullPhoneUiAccepted:false,completeResourcesAccepted:false,physicalDeviceAccepted:false,wholeGoal:'Active'};
writeFileSync(qa+'runtime-audit.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({distinctPids:4,gl506:33,renderer:'FAIL',hudRestore:'PASS bounded',menuTap:'PASS bounded',wholeGoal:'Active'}));
