import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
const qa='mir2-web3/docs/generated/player-qa/native-android-quest-workspace-20261002/';
const read=file=>readFileSync(qa+file,'utf8');
const source='ef5e56d1c8e51abd1fad3ee79be9a4ce1aa604e4';
const artifact=JSON.parse(read('raw/v23/package-v23.json'));
const touch=JSON.parse(read('raw/v23/touch-device/commands.json'));
const detail=JSON.parse(read('raw/v23/detail-device/commands.json'));
assert.equal(artifact.source,source);
for(const journal of [touch,detail]){assert.equal(journal.source,source);assert.equal(journal.versionCode,23);assert.equal(journal.apkSha256,artifact.artifacts.find(f=>f.variant==='preview').sha256);}
assert.deepEqual(touch.stages.map(s=>[s.name,s.tap.x,s.tap.y]),[['quests-ingress',1350,170],['npc-ingress',263,319]]);
assert.deepEqual(detail.stages.map(s=>[s.name,s.tap]),[['diary',null],['pair',{x:1350,y:170}],['detail-only',{x:1197,y:909}]]);
const entries=[],pids=new Set();
for(const [directory,scene,pidName,logName] of [
 ['device','debug-login','debug-login-pid.txt','debug-login-pid.log'],
 ['device','quests-ingress','quests-ingress-pid.txt','quests-ingress-pid.log'],
 ['device','npc-ingress','npc-ingress-pid.txt','npc-ingress-pid.log'],
 ['touch-device','quests-ingress','quests-ingress-pid.txt','quests-ingress-after-pid.log'],
 ['touch-device','npc-ingress','npc-ingress-pid.txt','npc-ingress-after-pid.log'],
 ['detail-device','detail-only','pid.txt','detail-only-pid.log'],
]){
 const pid=Number(read('raw/v23/'+directory+'/'+pidName).trim());assert(Number.isInteger(pid)&&pid>0&&!pids.has(pid));pids.add(pid);
 const log='raw/v23/'+directory+'/'+logName,lines=read(log).split(/\r?\n/);
 const producer=lines.filter(l=>/\sI event\s/.test(l)&&l.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
 if(scene!=='debug-login'){assert.equal(producer.length,1);assert.match(producer[0],/count = 1; quest_id = Some\(2100004\)/);assert(producer[0].includes('dialog_open = '+(scene==='npc-ingress')));}
 entries.push({directory,scene,pid,log,gl506:lines.filter(l=>l.includes('GL error 0x506')).length,
  missingPaths:lines.filter(l=>l.includes('Path not found')).length,fatalOrPanic:lines.filter(l=>/FATAL EXCEPTION|panicked at|Fatal signal|thread .*panicked/.test(l)),producer});
}
assert.deepEqual(entries.map(e=>e.gl506),[0,7,20,8,0,3]);
assert.equal(entries.reduce((n,e)=>n+e.missingPaths,0),0);assert.equal(entries.flatMap(e=>e.fatalOrPanic).length,0);
const frames=JSON.parse(read('manifest.json')).files.filter(f=>f.file.endsWith('.png'));assert.equal(frames.length,10);
for(const frame of frames){const bytes=readFileSync(qa+frame.file);assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);}
const proof={source,versionCode:23,distinctPids:6,entries,gl506:38,
 logCounting:'One latest cumulative log per distinct PID, canonical event only; no before/after or diary/pair/detail double count',originalFramesInspectedByPrimary:10,
 screenshotObservations:{normalLogin:'Empty credentials; Test server not configured',
  questRowTap:'PASS bounded actual row opens the shared detail at the new screen position',
  protectedWidePhoneChrome:'PASS bounded diary, pair and independent detail leave HP/MP, all six belt targets, chat and thumb controls visible and separate',
  independentDetailLifetime:'PASS bounded actual shared diary Close leaves detail visible and protected HUD lane retained',
  npcExit:'PASS bounded actual shared Exit hides dialogue and restores default HUD/control presentation',
  questPhoneReadability:'FAIL: diary/body/footer/rewards still too small or dark, targets below phone-size; common workspace fitting is not reflow'},
 zeroErrorRendererGate:'FAIL',gpuRendererFixThisLeaf:false,fullPhoneUiGate:'FAIL',compactImeSidebarAcceptance:false,
 olderV22Gl506:67,olderV21Gl506:33,olderV20Gl506:73,olderV19Gl506:135,olderEvidenceUnchanged:true,
 actualJavaJniAuthenticatedQuest:false,questOrRewardSuccess:false,realHttpsWss:false,zoneSave:false,
 fullPhoneUiAccepted:false,completeResourcesAccepted:false,physicalDeviceAccepted:false,wholeGoal:'Active',
 next:'Readable phone reflow, minimum 48dp controls, explicit scrolling/paging; bounded framebuffer diagnosis and full NI-11-20, real network and physical gates remain'};
writeFileSync(qa+'runtime-audit.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({distinctPids:6,gl506:38,renderer:'FAIL',protectedChrome:'PASS bounded',readability:'FAIL',wholeGoal:'Active'}));
