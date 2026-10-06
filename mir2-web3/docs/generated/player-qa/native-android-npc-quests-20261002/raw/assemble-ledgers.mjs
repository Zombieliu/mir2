import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,readdirSync} from 'node:fs';
import {dirname,join,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root=dirname(fileURLToPath(import.meta.url));
const source=JSON.parse(readFileSync(join(root,'source-v30.json'))).source;
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const npc=JSON.parse(readFileSync(join(root,'ui-npc-quests/commands.json')));
const confirmation=JSON.parse(readFileSync(join(root,'ui-quest-confirmation/commands.json')));
const baseline=JSON.parse(readFileSync(join(root,'baseline-v30/commands.json')));
const npcObservations=[
 ['0-cold.png','Eight manual UI-only NPC quests; quest 1 selected; list and message/reward viewports separate; header close and three footer controls visible.'],
 ['8-tap.png','Actual unselected Finish tap opens existing shared controller alert: You must select a reward item. No quest or reward grant is accepted.'],
 ['16-tap.png','Actual OK tap closes the alert; NPC list remains open with quest 1 selected.'],
 ['24-tap.png','Actual quest 2 tap moves the selection marker and right-side title to quest 2; initial message lines visible.'],
 ['32-swipe.png','Actual right-side upward swipe advances quest 2 message to lines 2 through 5; left list and selection remain unchanged.'],
 ['40-tap.png','Actual right Down tap advances message to lines 5 through 8; left list remains unchanged.'],
 ['48-tap.png','Actual right Up tap returns message to lines 2 through 5; quest 2 remains selected.'],
 ['56-tap.png','Actual left Down tap scrolls quest list to quests 3 through 5; right message remains quest 2 at its prior offset.'],
 ['64-swipe.png','Actual left-side upward swipe reaches quests 6 and 7 plus partially clipped quest 8; right quest 2 viewport unchanged.'],
 ['72-tap.png','Actual left Down tap clamps at list end; quests 6, 7 and 8 are fully visible and reachable; right quest 2 unchanged.'],
 ['80-tap.png','Actual quest 8 tap selects quest 8; right title changes and message offset starts at line 0; list end position retained.'],
 ['88-tap.png','Actual right Down tap exposes Gold 10 and both manual reward choices A and B. This is presentation metadata, not a grant.'],
 ['96-tap.png','Actual choice B tap shows the selection marker on UI choice B. Finish is not tapped after this selection; no grant is accepted.'],
 ['104-home.png','Actual HOME and resume retain quest 8, selected reward B, and independent viewport offsets in the same process.'],
 ['113-tap.png','Actual left Up tap exposes quests 4 and 5 plus partial quest 6 while right reward B remains selected; viewport independence retained.'],
 ['121-tap.png','Actual Leave tap closes both NPC dialog and quest-list presentation; ordinary offline phone HUD and touch controls reappear.'],
 ['129-home.png','Actual HOME and resume retain the closed NPC/list state in the same process; offline phone HUD remains visible.'],
];
const confirmationObservations=[
 ['0-cold.png','Manual UI-only confirmation shows Yes and No above the retained one-quest diary/detail; not a server-authorized quest.'],
 ['8-tap.png','Actual No tap closes confirmation; the one-quest diary/detail remains visible with its quest preserved.'],
 ['16-home.png','Actual HOME and resume retain the closed confirmation and one-quest diary/detail in the same process.'],
];
const frames=[];
function add(folder,file,pid,scene,observation){
 const path=folder+'/'+file,bytes=readFileSync(join(root,path));
 assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);
 frames.push({path,source,sha256:sha(bytes),pid:String(pid),scene,offlineOnly:true,
  primaryAgentViewedOriginal:true,observation});
}
for(const [file,observation] of npcObservations)add('ui-npc-quests',file,npc.pid,'npc-quests-manual-ui-only',observation);
for(const [file,observation] of confirmationObservations)add('ui-quest-confirmation',file,confirmation.pid,'quest-confirmation-manual-ui-only',observation);
for(const scene of baseline.scenes){
 const observations={
  login:'Normal native APK login UI visible with Test server not configured; no credentials entered, authentication performed, or network acceptance claimed.',
  world:'Licensed offline Bichon map, entities/effects and existing phone HUD visible; not live players, gameplay, whole-resource, or performance acceptance.',
  quests:'Offline received Daily quest metadata projects into the diary; not the manual NPC fixture and not an authenticated server receipt.',
 };
 assert(observations[scene.name]);
 for(const phase of ['cold','resumed'])add('baseline-v30',scene.name+'-'+phase+'.png',scene.pid,scene.name,
  observations[scene.name]+(phase==='resumed'?' Actual HOME and resume retain visible UI in the same process.':' Cold-start frame.'));
}
assert.equal(frames.length,26);
const pngs=['ui-npc-quests','ui-quest-confirmation','baseline-v30'].flatMap(folder=>readdirSync(join(root,folder)).filter(file=>file.endsWith('.png')).map(file=>folder+'/'+file));
assert.deepEqual(pngs.sort(),frames.map(row=>row.path).sort());
writeFileSync(join(root,'manual-frame-ledger.json'),JSON.stringify(frames,null,2)+'\n',{flag:'wx'});
const geometryLog=readFileSync(join(root,'ui-npc-quests/0-cold-pid.log'),'utf8');
const lines=geometryLog.split('\n').filter(line=>/ I event src\/phone_quests\.rs:/.test(line));
const controls=lines.filter(line=>line.includes('ANDROID_PHONE_QUEST_CONTROL')).map(line=>{
 const values=line.match(/size_dp = Vec2\(([^,]+), ([^)]+)\); center_dp = Vec2\(([^,]+), ([^)]+)\)/);assert(values,line);
 return {widthDp:Number(values[1]),heightDp:Number(values[2]),centerDp:[Number(values[3]),Number(values[4])]};
});
const scrollAreas=lines.filter(line=>line.includes('ANDROID_PHONE_QUEST_SCROLL')).map(line=>{
 const values=line.match(/size_dp = Vec2\(([^,]+), ([^)]+)\); content_dp = Vec2\(([^,]+), ([^)]+)\); offset = Vec2\(([^,]+), ([^)]+)\)/);assert(values,line);
 return {widthDp:Number(values[1]),heightDp:Number(values[2]),contentDp:[Number(values[3]),Number(values[4])],initialOffset:[Number(values[5]),Number(values[6])]};
});
const rawFontDp=[...new Set(lines.filter(line=>line.includes('ANDROID_PHONE_QUEST_TEXT')).map(line=>Number(line.match(/font_dp = ([^;]+);/)[1])))].sort((a,b)=>a-b);
assert.equal(controls.length,18);assert(controls.every(row=>row.widthDp>=48&&row.heightDp>=48));assert.equal(scrollAreas.length,2);
assert(rawFontDp.every(size=>Math.abs(size-Math.round(size))<0.00001));
writeFileSync(join(root,'npc-geometry.json'),JSON.stringify({source,pid:String(npc.pid),frame:'ui-npc-quests/0-cold.png',
  derivation:'Only native I event geometry rows; duplicate RustStdoutStderr rows excluded. Raw floating-point font sizes retained; nominal fontDp rounded only within 0.00001 dp.',
  controls,scrollAreas,rawFontDp,fontDp:rawFontDp.map(Math.round),
  boundary:'Off-screen rows and rewards are clipped until scrolled; not all controls initially visible and not whole-phone-layout acceptance.'},null,2)+'\n',{flag:'wx'});
const before=JSON.parse(readFileSync(join(root,'original-checkouts-before.json')));
const git=(cwd,args)=>execFileSync('git',args,{cwd,encoding:'utf8'});
const records=before.records.map(record=>({root:record.root,head:git(record.root,['rev-parse','HEAD']).trim(),
 branch:git(record.root,['branch','--show-current']).trim(),status:git(record.root,['status','--porcelain'])}));
assert.deepEqual(records,before.records,'Original worktree Git state changed');
writeFileSync(join(root,'original-checkouts-after.json'),JSON.stringify({checkedAt:new Date().toISOString(),scope:before.scope,records},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({source,frames:frames.length,controls:controls.length,scrollAreas:scrollAreas.length,rawFontDp,originalGitStateUnchanged:true}));
