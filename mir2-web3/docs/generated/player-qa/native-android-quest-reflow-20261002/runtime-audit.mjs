import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
const root='mir2-web3/docs/generated/player-qa/native-android-quest-reflow-20261002/';
const read=file=>readFileSync(root+file,'utf8');
const source='9a6bff0db9a7ff3eed003c72c90310ec632572d0';
const pkg=JSON.parse(read('raw/v24/package-v24.json'));assert.equal(pkg.source,source);
const preview=pkg.artifacts.find(entry=>entry.variant==='preview');
const journals=['wide-touch','wide-touch-retry'].map(dir=>JSON.parse(read(`raw/v24/${dir}/commands.json`)));
for(const journal of journals){assert.equal(journal.source,source);assert.equal(journal.versionCode,24);assert.equal(journal.apkSha256,preview.sha256);assert.equal(journal.noTransactionButtonTapped,true);}
assert.equal(journals[0].steps.length,1);assert.equal(journals[1].steps.length,7);
assert.deepEqual(journals[1].steps.map(s=>s.name),['pair','detail-swipe','detail-scroll-next','detail-scroll-back','detail-only','detail-scroll-top','all-closed']);
const entries=[],pids=new Set();
for(const [directory,scene,pidFile,logFile] of [
 ['device','debug-login','debug-login-pid.txt','debug-login-pid.log'],
 ['device','quests-ingress','quests-ingress-pid.txt','quests-ingress-pid.log'],
 ['device','npc-ingress','npc-ingress-pid.txt','npc-ingress-pid.log'],
 ['wide-touch','pair','pid.txt','pair-pid.log'],
 ['wide-touch-retry','all-closed','pid.txt','all-closed-pid.log'],
]){
 const prefix='raw/v24/'+directory+'/',pid=Number(read(prefix+pidFile).trim());assert(Number.isInteger(pid)&&pid>0&&!pids.has(pid));pids.add(pid);
 const log=prefix+logFile,lines=read(log).split(/\r?\n/),producer=lines.filter(line=>/\sI event\s/.test(line)&&line.includes('ANDROID_QUEST_PREVIEW_RECEIVED_MODELS_APPLIED_NOT_LIVE'));
 if(scene!=='debug-login'){assert.equal(producer.length,1);assert.match(producer[0],/count = 1; quest_id = Some\(2100004\)/);assert(producer[0].includes('dialog_open = '+(scene==='npc-ingress')));}
 entries.push({directory,scene,pid,log,gl506:lines.filter(line=>line.includes('GL error 0x506')).length,
  missingPaths:lines.filter(line=>line.includes('Path not found')).length,
  fatalOrPanic:lines.filter(line=>/FATAL EXCEPTION|panicked at|Fatal signal|thread .*panicked/.test(line)),producer});
}
assert.deepEqual(entries.map(entry=>entry.gl506),[26,8,0,28,20]);
assert.equal(entries.reduce((n,e)=>n+e.missingPaths,0),0);assert.equal(entries.flatMap(e=>e.fatalOrPanic).length,0);
const metrics=read('raw/v24/device/quests-ingress-pid.log').split(/\r?\n/).filter(line=>/\sI event\s/.test(line));
const controls=metrics.filter(line=>line.includes('ANDROID_PHONE_QUEST_CONTROL')).map(line=>{
 const match=line.match(/size_dp = Vec2\(([\d.]+), ([\d.]+)\)/);assert(match);return{width:Number(match[1]),height:Number(match[2])};
});assert.equal(controls.length,6);assert(controls.every(node=>node.width>=47.99&&node.height>=47.99));
const texts=metrics.filter(line=>line.includes('ANDROID_PHONE_QUEST_TEXT')).map(line=>{
 const font=Number(line.match(/font_dp = ([\d.]+)/)?.[1]),glyphs=Number(line.match(/glyphs = (\d+)/)?.[1]),alphaMasks=Number(line.match(/alpha_masks = (\d+)/)?.[1]);
 assert(font>=13.99);assert.equal(glyphs,alphaMasks);return{font,glyphs,alphaMasks};
});assert.equal(texts.length,8);
const manifest=JSON.parse(read('manifest.json')),frames=manifest.files.filter(file=>file.file.endsWith('.png'));assert.equal(frames.length,13);
for(const frame of frames){const bytes=readFileSync(root+frame.file);assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);}
const proof={source,versionCode:24,distinctPids:5,entries,gl506:82,
 logCounting:'Only each distinct PID latest cumulative log; canonical diagnostic events exclude RustStdoutStderr duplicates; no frame/scroll double counting',
 originalFramesInspectedByPrimary:13,firstDiaryRuntimeMetrics:{controls,texts,allSixControlsMinimum48Dp:true,body14DpHeading16Dp:true,allObservedGlyphsAlphaMask:true,pairMetricsNotFreshlyLogged:true},
 observations:{normalLogin:'Empty credentials; Test server not configured',
  actualRow:'PASS bounded: real row tap opens two independent shared windows',
  actualSwipe:'PASS bounded: actual vertical swipe advances full source detail content without selecting another row',
  actualScrollButtons:'PASS bounded: actual Down reaches progress/rewards/choice labels; Up returns; no reward or operation tapped',
  closeDiary:'PASS bounded: actual close leaves detail visible, wider, with retained scroll and protected HUD',
  closeDetail:'PASS bounded: actual close removes the last quest window and restores default HUD/belt/chat/thumb layout',
  readability:'PASS bounded captured wide-phone diary and detail: readable wrapped full titles/body/progress/reward labels; not all quest surfaces, locales or devices',
  npc:'Initial NPC dialogue remains visible; no fresh Exit/Menu/combat acceptance this leaf'},
 zeroErrorRendererGate:'FAIL',fullPhoneUiGate:'OPEN/FAIL renderer and remaining surfaces',gpuRendererFixThisLeaf:false,
 remaining:'NPC quest list, confirmations/alerts, all nine locale reflow cases, compact/IME overlap and physical/multitouch acceptance still open; bounded GPU and complete NI-11-20 remain',
 retainedOldGl506:{v23:38,v22:67,v21:33,v20:73,v19:135},olderEvidenceUnchanged:true,
 actualJavaJniAuthenticatedQuest:false,realHttpsWss:false,questOrRewardSuccess:false,zoneSave:false,
 completeResourcesAccepted:false,physicalDeviceAccepted:false,wholeGoal:'Active'};
writeFileSync(root+'runtime-audit.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({rawArtifacts:manifest.rawArtifacts,originalFrames:13,gl506:82,reflow:'PASS bounded',renderer:'FAIL',wholeGoal:'Active'}));
