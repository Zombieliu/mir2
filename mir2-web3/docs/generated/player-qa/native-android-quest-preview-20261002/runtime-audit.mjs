import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
const qa='mir2-web3/docs/generated/player-qa/native-android-quest-preview-20261002/';
const read=file=>readFileSync(qa+file,'utf8');
const source='7b62eda33cbf6c10a41c22516f1e6400dfe92202';
const artifact=JSON.parse(read('raw/v20/package-v20.json'));
const touch=JSON.parse(read('raw/v20/touch-device/commands.json'));
assert.equal(artifact.source,source);
assert.equal(touch.source,source);
assert.equal(touch.apkSha256,artifact.artifacts.find(file=>file.variant==='preview').sha256);
assert.deepEqual(touch.stages.map(stage=>[stage.name,stage.tap.x,stage.tap.y]),
 [['quests-ingress',943,168],['npc-ingress',263,319]]);
const pids=new Set(),entries=[];
for(const [directory,scene,suffix] of [
 ['device','debug-login',''],['device','quests-ingress',''],['device','npc-ingress',''],
 ['touch-device','quests-ingress','-after'],['touch-device','npc-ingress','-after'],
]){
 const pid=Number(read('raw/v20/'+directory+'/'+scene+'-pid.txt').trim());
 assert(Number.isInteger(pid)&&pid>0&&!pids.has(pid));pids.add(pid);
 const path='raw/v20/'+directory+'/'+scene+suffix+'-pid.log';
 const lines=read(path).split(/\r?\n/);
 const canonical=lines.filter(line=>/\sI event\s/.test(line));
 const producer=canonical.filter(line=>line.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
 if(scene!=='debug-login'){
  assert.equal(producer.length,1);
  assert.match(producer[0],/count = 1; quest_id = Some\(2100004\)/);
  assert(producer[0].includes('dialog_open = '+(scene==='npc-ingress')));
 }
 entries.push({directory,scene,pid,log:path,
  gl506:lines.filter(line=>line.includes('GL error 0x506')).length,
  missingPaths:lines.filter(line=>line.includes('Path not found')).length,
  fatalOrPanic:lines.filter(line=>/FATAL EXCEPTION|panicked at|Fatal signal|thread .*panicked/.test(line)),producer});
}
for(const file of ['raw/v20/device/debug-login.png','raw/v20/device/quests-ingress.png','raw/v20/device/npc-ingress.png',
 ...['quests-ingress','npc-ingress'].flatMap(scene=>['before','after'].map(stage=>'raw/v20/touch-device/'+scene+'-'+stage+'.png'))]){
 const image=readFileSync(qa+file);
 assert.equal(image.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 assert.equal(image.readUInt32BE(16),2340);assert.equal(image.readUInt32BE(20),1080);
}
const gl506=entries.reduce((n,entry)=>n+entry.gl506,0);
assert.deepEqual(entries.map(entry=>entry.gl506),[11,10,52,0,0]);
assert.equal(gl506,73);
assert.equal(entries.reduce((n,entry)=>n+entry.missingPaths,0),0);
assert.equal(entries.flatMap(entry=>entry.fatalOrPanic).length,0);
const proof={source,versionCode:20,distinctPids:5,entries,gl506,
 logCounting:'One latest cumulative log per distinct PID, no touch-before/after double count; only canonical event for duplicate Rust markers',
 originalFramesInspectedByPrimary:7,
 screenshotObservations:{normalLogin:'Empty credentials; Test server not configured',
  receivedQuestList:'PASS bounded Daily/title/ready-to-turn-in status visible',
  actualQuestRowTap:'PASS bounded detail opens; definition/turn-in text and 2/2 progress visible',
  receivedNpcDialogue:'PASS bounded title/body/Exit visible',
  actualNpcExitTap:'PASS bounded dialogue window disappears',
  hudRestorationAfterNpcExit:'FAIL - original after frame has no restored HUD',
  questPhoneReadability:'FAIL - narrow windows and small text; not 48dp or full phone layout acceptance'},
 zeroErrorRendererGate:'FAIL',rendererFixThisLeaf:false,olderV19Gl506:135,olderEvidenceUnchanged:true,
 actualJavaJniAuthenticatedQuest:false,questOrRewardSuccess:false,realHttpsWss:false,zoneSave:false,
 fullPhoneUiAccepted:false,completeResourcesAccepted:false,physicalDeviceAccepted:false,wholeGoal:'Active'};
writeFileSync(qa+'runtime-audit.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({distinctPids:5,gl506,zeroErrorRendererGate:'FAIL',hudRestore:'FAIL',phoneReadability:'FAIL',wholeGoal:'Active'}));
