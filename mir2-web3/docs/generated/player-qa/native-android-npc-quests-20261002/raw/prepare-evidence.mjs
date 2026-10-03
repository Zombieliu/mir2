import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,readdirSync,copyFileSync,statSync,existsSync} from 'node:fs';
import {dirname,join,relative,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const root=dirname(fileURLToPath(import.meta.url));
const qa=resolve('mir2-web3/docs/generated/player-qa/native-android-npc-quests-20261002');
assert(!existsSync(qa),'Never overwrite previous evidence');
mkdirSync(join(qa,'raw'),{recursive:true});
function copyTree(from,to){
 mkdirSync(to,{recursive:true});
 for(const item of readdirSync(from,{withFileTypes:true})){
  const src=join(from,item.name),dst=join(to,item.name);
  assert(!item.isSymbolicLink());
  if(item.isDirectory())copyTree(src,dst);
  else copyFileSync(src,dst);
 }
}
for(const item of readdirSync(root,{withFileTypes:true})){
 if(item.isFile() && /\.(mjs|json|log|txt|rs)$/.test(item.name))copyFileSync(join(root,item.name),join(qa,'raw',item.name));
}
for(const name of ['baseline-source','baseline-source-2','baseline-v30','ui-npc-quests','ui-quest-confirmation']){
 assert(existsSync(join(root,name)),name);copyTree(join(root,name),join(qa,'raw',name));
}
const source=JSON.parse(readFileSync(join(root,'source-v30.json')));
const pkg=JSON.parse(readFileSync(join(root,'package-baseline.json')));
const baseline=JSON.parse(readFileSync(join(root,'baseline-v30/commands.json')));
const npc=JSON.parse(readFileSync(join(root,'ui-npc-quests/commands.json')));
const confirmation=JSON.parse(readFileSync(join(root,'ui-quest-confirmation/commands.json')));
const frames=JSON.parse(readFileSync(join(root,'manual-frame-ledger.json')));
const canonical=[...baseline.scenes.map(scene=>({pid:scene.pid,name:scene.name,gl506:scene.latestCumulative.gl506,fatalOrPanic:scene.latestCumulative.fatalOrPanic})),
 {pid:npc.pid,name:'npc-quests',...npc.latestTotals},{pid:confirmation.pid,name:'quest-confirmation',...confirmation.latestTotals}];
assert.equal(new Set(canonical.map(row=>row.pid)).size,canonical.length);
const diagnosis={source:source.source,frozenWindows:source.frozenWindows,latestObservedWindows:source.latestObservedWindows,
 sourceScope:source.sourceFiles.map(row=>row.path),versionCode:30,versionName:'0.1.27-phone-npc-quests',
 gates:{normal:327,preview:352,shared:1254,sharedExistingIgnored:10,phone:24,javaEach:41,actualApi31Both:true},
 manualFrameCount:frames.length,canonicalProcesses:canonical,canonicalGl506:canonical.reduce((n,row)=>n+row.gl506,0),
 previewManualUIOnly:true,worldGameplayAccepted:false,realNetworkAccepted:false,fullResourcePackAccepted:false,
 zeroErrorRendererAccepted:false,physicalDeviceAccepted:false,wholeWindowsParityAccepted:false,goalStatus:'active'};
assert.equal(pkg.source,source.source);
writeFileSync(join(qa,'diagnosis.json'),JSON.stringify(diagnosis,null,2)+'\n',{flag:'wx'});
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const payloads=[];
function walk(folder){for(const item of readdirSync(folder,{withFileTypes:true})){const path=join(folder,item.name);if(item.isDirectory())walk(path);else payloads.push({path:relative(qa,path),bytes:statSync(path).size,sha256:sha(readFileSync(path))});}}
walk(join(qa,'raw'));payloads.push({path:'diagnosis.json',bytes:statSync(join(qa,'diagnosis.json')).size,sha256:sha(readFileSync(join(qa,'diagnosis.json')))});
writeFileSync(join(qa,'manifest.json'),JSON.stringify({source:source.source,payloads:payloads.toSorted((a,b)=>a.path.localeCompare(b.path))},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({qa,payloads:payloads.length,frames:frames.length,canonicalGl506:diagnosis.canonicalGl506}));

