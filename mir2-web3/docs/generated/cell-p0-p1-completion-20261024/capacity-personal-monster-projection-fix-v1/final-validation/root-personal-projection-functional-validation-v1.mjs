import fs from 'node:fs/promises';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const base='C:/mir2-build/p01-completion-20261024',repo='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const decode=b=>b[0]===0xff&&b[1]===0xfe?b.subarray(2).toString('utf16le'):b.toString('utf8');
const read=async p=>JSON.parse(decode(await fs.readFile(p)));
const pin=async p=>{const b=await fs.readFile(p);return{path:p,bytes:b.length,sha256:sha(b)};};
const control=await read(base+'/root-personal-projection-final-source-inputs-v1.json');
const update=p=>p.path==='mir2-web3/apps/gateway/src/routing/tests/live_aoi_order_tests.rs'?{...p,bytes:28187,sha256:'0f21bfd95760a94f189c6b95567b0e65f1a0018cf88e90b2342ff753010a7279'}:{...p};
const sourcePins=control.sourcePins.map(update),expected=control.files.map(update).sort((a,b)=>a.path.localeCompare(b.path));
assert.equal(expected.length,44);assert.equal(sourcePins.length,5);
const sourceFiles=[];
for(const p of expected){const b=await fs.readFile(repo+'/'+p.path);assert.equal(b.length,p.bytes,p.path);assert.equal(sha(b),p.sha256,p.path);sourceFiles.push({path:p.path,bytes:b.length,sha256:sha(b)});}
const nodeFreezeFile=base+'/capacity-native-chase-preference-driver-frozen-v1.json';
assert.deepEqual(await pin(nodeFreezeFile),{path:nodeFreezeFile,bytes:19099,sha256:'20fd347d029d9a912514e400bc74b0d08ec940bd90af20ff2f91bd64dba08bef'});
const nodeFreeze=await read(nodeFreezeFile);assert.equal(nodeFreeze.files.length,9);assert.equal(nodeFreeze.sourceSha256,'db4e22c69b4f1365d8135023e158f4de00ff35d58330cf7eca98801780e674e7');
for(const p of nodeFreeze.files){const b=await fs.readFile(repo+'/mir2-web3/'+p.file);assert.equal(b.length,p.bytes);assert.equal(sha(b),p.sha256);}
const logNames=['probe14-personal-projection-focused-modules-v1.log','probe14-personal-projection-overflow-green-v1.log','probe14-personal-projection-remaining-adjacent-v2.log','probe14-personal-projection-isolated-quest-v1.log','probe14-personal-projection-rustfmt39-v2.log'];
const logs=[];
for(const name of logNames){const b=await fs.readFile(base+'/'+name);logs.push({name,pin:{path:base+'/'+name,bytes:b.length,sha256:sha(b)},text:decode(b)});}
assert.match(logs[0].text,/test result: FAILED\. 26 passed; 1 failed; 0 ignored;/);
assert.match(logs[1].text,/test result: ok\. 1 passed; 0 failed; 0 ignored;/);
assert.match(logs[2].text,/test result: ok\. 174 passed; 0 failed; 4 ignored;/);
assert.match(logs[3].text,/test result: ok\. 2 passed; 0 failed; 0 ignored;/);
assert.equal(logs[4].pin.bytes,0);
const parse=text=>[...text.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,[^\r\n]*)?\r?$/gm)].map(m=>({name:m[1],status:m[2]}));
const focused=parse(logs[0].text),overflow=parse(logs[1].text),remaining=parse(logs[2].text),quest=parse(logs[3].text);
assert.equal(focused.length,27);assert.equal(overflow.length,1);assert.equal(remaining.length,178);assert.equal(quest.length,2);
assert.deepEqual(focused.filter(x=>x.status==='FAILED').map(x=>x.name),overflow.map(x=>x.name));assert.equal(overflow[0].status,'ok');
const finalLive=await fs.readFile(repo+'/mir2-web3/apps/gateway/src/routing/tests/live_aoi_order_tests.rs');
const beforeOverflow=await fs.readFile(base+'/probe14-adjacent-fixture-repair-v1/live_aoi_order_tests.rs.before-overflow-v2.txt');
assert.equal(sha(beforeOverflow),'addfedf29cd0c36b2865df2e40b07b1c91cd10130a981d9d5f55447ab94c85af');
const marker=Buffer.from('#[test]\r\nfn live_aoi_order_wss_overflow_stops_broken_epoch_preserves_personal_state_and_fences_replacement()');
let oldAt=beforeOverflow.indexOf(marker),newAt=finalLive.indexOf(marker);
if(oldAt<0||newAt<0){const lfMarker=Buffer.from(marker.toString('utf8').replaceAll('\r\n','\n'));oldAt=beforeOverflow.indexOf(lfMarker);newAt=finalLive.indexOf(lfMarker);}
assert.ok(oldAt>0&&newAt>0);assert.ok(beforeOverflow.subarray(0,oldAt).equals(finalLive.subarray(0,newAt)),'Only the last overflow test may differ from the26-pass source');
const outcomes=new Map();
for(const x of focused)outcomes.set(x.name,x.status);outcomes.set(overflow[0].name,'ok');
for(const x of remaining){assert.ok(!outcomes.has(x.name),'Focused test repeated in remaining selection');outcomes.set(x.name,x.status);}
for(const x of quest){assert.equal(outcomes.get(x.name),'ignored');assert.equal(x.status,'ok');outcomes.set(x.name,'ok');}
assert.equal(outcomes.size,205);assert.equal([...outcomes.values()].filter(x=>x==='ok').length,203);assert.equal([...outcomes.values()].filter(x=>x==='ignored').length,2);assert.equal([...outcomes.values()].filter(x=>x==='FAILED').length,0);
const adjacent=await read(base+'/probe14-personal-projection-remaining-adjacent-execution-v2.json');
assert.equal(adjacent.exitCode,0);assert.equal(adjacent.sourceUnchangedBeforeAfter,true);assert.equal(adjacent.expectedSummaryMatched,true);
const canonical=a=>a.map(p=>({path:p.path,bytes:p.bytes,sha256:p.sha256})).sort((a,b)=>a.path.localeCompare(b.path));
assert.deepEqual(canonical(adjacent.sourceBefore),canonical(sourceFiles));assert.deepEqual(canonical(adjacent.sourceAfter),canonical(sourceFiles));assert.deepEqual(canonical(adjacent.sourcePins),canonical(sourcePins));
for(const p of sourceFiles){const b=await fs.readFile(repo+'/'+p.path);assert.equal(b.length,p.bytes);assert.equal(sha(b),p.sha256);}
const receipt={schemaVersion:1,verifiedAt:new Date().toISOString(),state:'ROOT_BOUNDED_RUST_VALIDATION_COMPLETE',coordinatorActuallyExecuted:true,sourcePins,sourceInputCount:44,sourceFiles,otherOriginalInputsUnchanged:39,checks:{rustFocusedPassed:true,adjacentPassed:true,rustfmtPassed:true},focused:{selected:27,initialPassed:26,initialFailed:1,repairedOverflowPassed:1,other26FunctionSourceUnchanged:true,scope:'Three affected test modules. The actual26-pass run is retained; only its final overflow test changed afterward and that one was rerun and passed.'},adjacent:{selected:178,passed:174,failed:0,ignored:4,sourceUnchangedBeforeAfter:true},isolatedQuest:{environment:{MIR2_QUEST_CADENCE:'newcomer-v2'},selected:2,passed:2,failed:0,previouslyIgnoredOnly:true},distinctRust:{selected:205,passed:203,failed:0,remainingIgnored:2,remainingIgnoredNames:[...outcomes].filter(([,s])=>s==='ignored').map(([n])=>n),wholeWorkspaceCertified:false},rustfmt:{toolchain:'1.95.0',command:['rustfmt','+1.95.0','--check','--edition','2021','--config','skip_children=true'],sourceCount:39,exitCode:0},logs:logs.map(({name,pin})=>({name,...pin})),adjacentExecution:await pin(base+'/probe14-personal-projection-remaining-adjacent-execution-v2.json'),sourceControlBeforeOverflow:await pin(base+'/root-personal-projection-final-source-inputs-v1.json'),overflowMutation:await pin(base+'/probe14-adjacent-fixture-repair-v1/overflow-mutation-proof-v2.json'),sourceUnchangedDuringThisVerification:true,allHistoricalFailuresPreserved:true,nodeNineSourceNotChangedByThisRustFix:true,newReleaseBuilt:false,freshFull16Passed:false,capacityAccepted:false,fixedCellStarted:false};
const output=base+'/root-personal-projection-functional-validation-v1.json';await fs.writeFile(output,JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({receipt:await pin(output),distinctRust:receipt.distinctRust,rustfmtSourceCount:39,checks:receipt.checks,newReleaseBuilt:false,capacityAccepted:false}));
