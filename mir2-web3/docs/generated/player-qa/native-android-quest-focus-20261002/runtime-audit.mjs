import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
const qa='mir2-web3/docs/generated/player-qa/native-android-quest-focus-20261002/';
const read=file=>readFileSync(qa+file,'utf8');
const source='afea4b358680bb1d1a2ec8362ca6ffb538c6d648';
const artifact=JSON.parse(read('raw/v22/package-v22.json'));
const touch=JSON.parse(read('raw/v22/touch-device/commands.json'));
assert.equal(artifact.source,source);assert.equal(touch.source,source);
assert.equal(touch.versionCode,22);
assert.equal(touch.apkSha256,artifact.artifacts.find(f=>f.variant==='preview').sha256);
assert.deepEqual(touch.stages.map(s=>[s.name,s.tap.x,s.tap.y]),
 [['quests-ingress',1120,170],['npc-ingress',263,319]]);
const pids=new Set(),entries=[];
for(const [directory,scene,suffix] of [
 ['device','debug-login',''],['device','quests-ingress',''],['device','npc-ingress',''],
 ['touch-device','quests-ingress','-after'],['touch-device','npc-ingress','-after'],
]){
 const pid=Number(read('raw/v22/'+directory+'/'+scene+'-pid.txt').trim());
 assert(Number.isInteger(pid)&&pid>0&&!pids.has(pid));pids.add(pid);
 const path='raw/v22/'+directory+'/'+scene+suffix+'-pid.log';
 const lines=read(path).split(/\r?\n/);
 const producer=lines.filter(l=>/\sI event\s/.test(l)&&l.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
 if(scene!=='debug-login'){
  assert.equal(producer.length,1);assert.match(producer[0],/count = 1; quest_id = Some\(2100004\)/);
  assert(producer[0].includes('dialog_open = '+(scene==='npc-ingress')));
 }
 entries.push({directory,scene,pid,log:path,
  gl506:lines.filter(l=>l.includes('GL error 0x506')).length,
  missingPaths:lines.filter(l=>l.includes('Path not found')).length,
  fatalOrPanic:lines.filter(l=>/FATAL EXCEPTION|panicked at|Fatal signal|thread .*panicked/.test(l)),producer});
}
assert.deepEqual(entries.map(e=>e.gl506),[0,43,0,0,24]);
assert.equal(entries.reduce((n,e)=>n+e.missingPaths,0),0);
assert.equal(entries.flatMap(e=>e.fatalOrPanic).length,0);
const manifest=JSON.parse(read('manifest.json'));
const frames=manifest.files.filter(f=>f.file.endsWith('.png'));assert.equal(frames.length,7);
for(const frame of frames){
 const bytes=readFileSync(qa+frame.file);
 assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);
}
const proof={source,versionCode:22,distinctPids:5,entries,gl506:67,
 logCounting:'One latest cumulative log per distinct PID; canonical event only, no before/after double count',
 originalFramesInspectedByPrimary:7,
 screenshotObservations:{normalLogin:'Empty credentials; Test server not configured',
  receivedQuestList:'Shared Daily/title/ready status visible, enlarged but still small',
  actualQuestRowTap:'PASS bounded row tap opens the actual shared detail window',
  questPairPosition:'PASS bounded both windows visible with retained gap, not independently overlapping each other',
  phoneHudAndBeltOcclusion:'FAIL: enlarged diary/detail cover part of status card and belt/chat controls',
  questPhoneReadability:'FAIL: tiny text, dark low-contrast glyphs and below-phone-size targets remain; scaling is not reflow',
  receivedNpcDialogue:'PASS bounded received title/body/Exit visible',
  actualNpcExitTap:'PASS bounded dialogue disappears and HP/MP/belt/joystick/action chrome restores; no fresh Menu tap this leaf'},
 zeroErrorRendererGate:'FAIL',gpuRendererFixThisLeaf:false,
 olderV21Gl506:33,olderV20Gl506:73,olderV19Gl506:135,olderEvidenceUnchanged:true,
 actualJavaJniAuthenticatedQuest:false,questOrRewardSuccess:false,realHttpsWss:false,zoneSave:false,
 fullPhoneUiAccepted:false,completeResourcesAccepted:false,physicalDeviceAccepted:false,wholeGoal:'Active',
 next:'Protect the shared HUD/belt/chat and both thumb zones, then reflow fonts/touch rows and scroll/paging; bounded GPU and full Windows denominator remain'};
writeFileSync(qa+'runtime-audit.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({distinctPids:5,gl506:67,renderer:'FAIL',questPair:'PASS bounded',hudOcclusion:'FAIL',readability:'FAIL',wholeGoal:'Active'}));
