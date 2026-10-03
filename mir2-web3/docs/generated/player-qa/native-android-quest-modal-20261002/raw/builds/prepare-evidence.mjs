import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync,readdirSync,mkdirSync,copyFileSync,existsSync} from 'node:fs';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const input=dirname(fileURLToPath(import.meta.url));
const repo='/Users/henryliu/obelisk/numeron-worktrees/android-shared-sync';
const rel='mir2-web3/docs/generated/player-qa/native-android-quest-modal-20261002';
const root=join(repo,rel);
assert(!existsSync(join(root,'manifest.json')),'Never silently overwrite a finished evidence bundle');
const sha=b=>createHash('sha256').update(b).digest('hex');
const json=p=>JSON.parse(readFileSync(join(input,p)));
const save=(p,j)=>{mkdirSync(dirname(join(root,p)),{recursive:true});writeFileSync(join(root,p),JSON.stringify(j,null,2)+'\n',{flag:'wx'});};
const copied=[];
function copy(from,to){
 const bytes=readFileSync(from);mkdirSync(dirname(join(root,to)),{recursive:true});
 assert(!existsSync(join(root,to)),to);copyFileSync(from,join(root,to));
 assert(readFileSync(join(root,to)).equals(bytes));copied.push(to);
}
function tree(dir,to){
 for(const e of readdirSync(join(input,dir),{withFileTypes:true})){
  const a=join(dir,e.name),b=join(to,e.name);
  assert(!e.isSymbolicLink());
  if(e.isDirectory())tree(a,b);else{assert(e.isFile());copy(join(input,a),b);}
 }
}
for(const e of readdirSync(input,{withFileTypes:true})){
 if(!e.isFile()||e.name==='prepare-evidence.mjs')continue;
 assert(/\.(mjs|json|log|txt|rs)$/.test(e.name),e.name);
 copy(join(input,e.name),'raw/builds/'+e.name);
}
for(const p of ['baseline-source','baseline-source-2','baseline-source-3','baseline-v29','ui-quest-confirmation','ui-quest-alert'])tree(p,'raw/'+p);
copy(join(input,'prepare-evidence.mjs'),'raw/builds/prepare-evidence.mjs');
const priorRel='mir2-web3/docs/generated/player-qa/native-android-gles-investigation-20261002/raw/builds/original-checkouts.json';
const prior=JSON.parse(readFileSync(join(repo,priorRel)));
const rows=prior.rows.map(old=>{
 const git=args=>execFileSync('git',args,{cwd:old.dir,encoding:'utf8'});
 const row={name:old.name,dir:old.dir,head:git(['rev-parse','HEAD']).trim(),branch:git(['branch','--show-current']).trim(),status:git(['status','--short'])};
 return {...row,originalHeadRetained:row.head===old.head,headBranchStatusMatchesPrior:['head','branch','status'].every(k=>row[k]===old[k])};
});
save('raw/builds/original-checkouts.json',{stamp:new Date().toISOString(),comparedTo:priorRel,rows,noWriteToOriginalCheckouts:true,fullDirectoryContentAudit:false});
const p=json('package-baseline.json'),audit=json('source-and-package-baseline.json'),base=json('baseline-v29/commands.json');
assert.equal(p.source,'2161a940ec7a59e966ed2638667f0c7b1a8e32b6');
const captures=[];
for(const scene of base.scenes)captures.push({phase:'baseline-v29',scene:scene.name,pid:scene.pid,latestLog:scene.name+'-latest-pid.log',totals:scene.latestCumulative,frames:[scene.name+'-cold.png',scene.name+'-resumed.png']});
for(const scene of ['quest-confirmation','quest-alert']){
 const c=json('ui-'+scene+'/commands.json');
 captures.push({phase:'ui-'+scene,scene,pid:c.pid,latestLog:c.latestLog,totals:c.latestTotals,frames:c.commands.filter(x=>x.name.endsWith('.png')).map(x=>x.name)});
}
assert.equal(new Set(captures.map(x=>x.pid)).size,5);
const total=captures.reduce((n,c)=>n+c.totals.gl506,0);assert.equal(total,17);
const notes={
 'baseline-v29/login-cold.png':'Native normal login: empty credential fields, test server not configured. Not real authentication.',
 'baseline-v29/login-resumed.png':'Same PID normal login after HOME/resume; not authenticated.',
 'baseline-v29/world-cold.png':'Offline map/entities and shared phone HUD/belt/chat/thumb areas visible. Partial proof resources only.',
 'baseline-v29/world-resumed.png':'Same PID offline map after HOME/resume; not live Zone or a resource-completeness gate.',
 'baseline-v29/quests-cold.png':'Offline wire-shape quest-ingress diary: Daily row, readable shared list. Not real JNI or server receipt.',
 'baseline-v29/quests-resumed.png':'Same PID offline wire-shape diary restored.',
 'ui-quest-confirmation/0-cold.png':'Manual shared phone confirmation: visible question and Yes/No footer. No action to server.',
 'ui-quest-confirmation/8-tap.png':'Actual No tap removed confirmation; one task remains; diary/detail/HUD/thumb controls restored.',
 'ui-quest-confirmation/16-home.png':'Same PID after HOME/resume: confirmation remains closed; underlying task and shared controls retained.',
 'ui-quest-alert/0-cold.png':'Manual 30-line message: lines 0-10 visible, scroll and OK footer are not clipped.',
 'ui-quest-alert/8-swipe.png':'Actual body swipe advances visible text to around lines 7-15; partial boundary lines are clipped by the scroll viewport.',
 'ui-quest-alert/16-tap.png':'Actual Down tap advances visible text to around lines 13-19; same footer remains accessible.',
 'ui-quest-alert/24-tap.png':'Actual Up tap returns to around lines 7-15.',
 'ui-quest-alert/32-home.png':'Same PID after HOME/resume: message still open and visible scroll position retained.',
 'ui-quest-alert/41-tap.png':'Actual OK tap removed message; one task, diary/detail/HUD/thumb controls restored.',
 'ui-quest-alert/49-home.png':'Same PID after second HOME/resume: message stays closed; task and shared controls remain.'
};
const frames=captures.flatMap(c=>c.frames.map(f=>{
 const path=c.phase+'/'+f;assert(notes[path],path);
 return {path:'raw/'+path,pid:c.pid,sha256:sha(readFileSync(join(input,path))),primaryInspectedOriginal:true,editedOrResized:false,observation:notes[path]};
}));
assert.equal(frames.length,16);assert.equal(Object.keys(notes).length,16);
const scope=execFileSync('git',['diff-tree','--no-commit-id','--name-only','-r',p.source],{cwd:repo,encoding:'utf8'}).trim().split('\n');
assert.equal(scope.length,5);
save('diagnosis.json',{
 source:p.source,sourceParent:execFileSync('git',['rev-parse',p.source+'^'],{cwd:repo,encoding:'utf8'}).trim(),
 frozenWindows:audit.frozenWindows,versionCode:p.versionCode,sourceWriteSet:scope,
 goalStatus:'active',scope:'Optional shared phone quest confirmation and alert presentation/input only',
 failuresFirst:[
  {input:'raw/builds/red-source-quest_phone.rs',inputSha256:json('modal-red.json').phoneSourceSha256,red:'raw/builds/modal-red.log',compiled:{passed:0,failed:3},green:'raw/builds/modal-green-1.log',greenPassed:3},
  {input:'raw/builds/message-replay-red-source-quest_phone.rs',inputSha256:json('message-replay-red.json').phoneSourceSha256,red:'raw/builds/message-replay-red.log',compiled:{passed:0,failed:1},green:'raw/baseline-source-3/shared-bevy-final.log',sameSettledFixture:true}
 ],
 actualCaptures:captures,canonicalDistinctPidGl506:total,rendererZeroErrorAccepted:false,rendererFixed:false,
 actualModalAcceptedScope:['No closes confirmation','body swipe','Down and Up scroll','OK closes message','same-PID background restoration'],
 manualUiSpecimensNotAuthenticatedPackets:true,noAbandonYesTap:true,allOriginalFramesInspected:true,originalFrames:frames,
 freshFinalGates:'raw/baseline-source-3',sharedPhoneModuleTests:17,
 graphicsDependencyDecision:'pending user authorization',
 wholePhoneUiAccepted:false,fullResourcePackAccepted:false,realNetworkAccepted:false,physicalDeviceAccepted:false,
 noGameplayOrAuthOrServerRulesChanged:true,
 remaining:['phone NPC quest list','compact screens and IME','nine languages','multi-touch runtime acceptance','complete NI-11 through NI-20 ingress','real authentication/JNI/HTTPS/WSS','shared Zone and saved state','complete resources/audio/update','physical device and human acceptance'],
 setupFailuresClassifiedAt:'raw/builds/retained-setup-errors.json',versionAuditHelperFailure:'raw/builds/audit-version-check-failure.json'
});
const files=[...copied,'raw/builds/original-checkouts.json','diagnosis.json'].sort().map(path=>{
 const bytes=readFileSync(join(root,path));return {path,bytes:bytes.length,sha256:sha(bytes)};
});
assert.equal(new Set(files.map(f=>f.path)).size,files.length);
save('manifest.json',{source:p.source,versionCode:29,rawBytePreserved:true,files});
console.log(JSON.stringify({bundle:root,integrityFiles:files.length,originalFrames:frames.length,distinctPids:captures.length,canonicalGl506:total,originalCheckouts:rows.map(r=>({name:r.name,same:r.headBranchStatusMatchesPrior}))}));
