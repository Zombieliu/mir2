import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const root='mir2-web3/apps/game-client/';
const proof=root+'platform-android/target/quest-ingress-20261002/';
const source='ccdd51a90542d866a3b08096469c2d2819656ce4';
const frozen='3d735745f1117d42a7859e87604a106351dca935';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=(args,cwd=process.cwd())=>execFileSync('git',args,{cwd,encoding:'utf8',maxBuffer:4*1024*1024}).trim();
assert.equal(git(['rev-parse','HEAD']),source);
execFileSync('git',['merge-base','--is-ancestor',frozen,source]);
assert.equal(git(['status','--short','--',root]),'');
const rows=JSON.parse(readFileSync(proof+'final-source/results.json'));
const expected={'windows-bridge':101,'windows-quest':101};
let sources;
const gates=rows.map(({gate})=>{
 const record=JSON.parse(readFileSync(proof+'final-source/'+gate+'.json'));
 assert.equal(record.status,expected[gate]??0,gate);
 assert.equal(record.sourceUnchanged,true,gate);
 assert.deepEqual(record.sourceHashesBefore,record.sourceHashesAfter,gate);
 for(const [file,hash] of Object.entries(record.sourceHashesAfter)){
  assert.equal(sha(readFileSync(root+file)),hash,gate+' '+file);
  assert.equal(sha(execFileSync('git',['show',source+':'+root+file],{maxBuffer:4*1024*1024})),hash,gate+' git '+file);
 }
 sources=record.sourceHashesAfter;
 const raw=readFileSync(proof+'final-source/'+gate+'.log','utf8');
 const result=raw.match(/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/);
 return {gate,status:record.status,sourceUnchanged:true,
  rustResult:result?{result:result[1],passed:+result[2],failed:+result[3],ignored:+result[4],measured:+result[5],filtered:+result[6]}:null,
  rawSha256:sha(Buffer.from(raw))};
});
const projection=JSON.parse(readFileSync(proof+'projection-parity.json'));
assert.equal(projection.allBodiesIdentical,true);
assert.equal(projection.rows.length,25);
const windows=JSON.parse(readFileSync(proof+'windows-bridge-baseline-binding.json'));
assert.equal(windows.identicalResultSet,true);
assert.equal(windows.finalQuestIdenticalResultSet,true);
assert.equal(windows.currentOwnedFilesRestored,true);
const java=JSON.parse(readFileSync(proof+'final-source/java-results.json'));
assert.equal(java.length,10);
const javaTotals=['Debug','UiPreview'].map(variant=>{
 const rows=java.filter(row=>row.variant===variant);
 const total=rows.reduce((n,row)=>n+row.tests,0);
 for(const row of rows){assert.equal(row.failures+row.errors+row.skipped,0);}
 assert.equal(total,41);
 return {variant,tests:total,failed:0,errors:0,skipped:0};
});
const original='/Users/henryliu/obelisk/numeron';
const oldAndroid='/Users/henryliu/obelisk/numeron-worktrees/android-player-journey';
const originalStatus=git(['status','--short'],original);
assert.equal(git(['rev-parse','HEAD'],original),'31b2b396057d82aa80567a33a668907d5d41a432');
assert.equal(originalStatus,[' M .gitignore',' M mir2-web3/apps/web/app/globals.css',' M mir2-web3/apps/web/app/page.tsx',
 '?? apps/','?? docs/generated/','?? mir2-web3/apps/web/node_modules','?? mir2-web3/output/','?? output/','?? scripts/'].join('\n').trim());
assert.equal(git(['status','--short'],oldAndroid),'');
assert.equal(git(['rev-parse','HEAD'],oldAndroid),'5d417ca75a5fe845a7d6f6d9e16c720d55e19b36');
const record={sourceCommit:source,publishedParent:'2c2e5e214693859d7d56e414fb3ff9233e58409c',
 frozenWindows:frozen,frozenIsAncestor:true,ownedSourceFiles:sources,gates,javaTotals,
 pureProjectionEntries:25,pureProjectionIdentical:true,
 windowsBroaderFiltersAccepted:false,windowsFailuresEqualRetainedParentFilters:true,
 originalCheckoutsPreserved:{originalHead:'31b2b396057d82aa80567a33a668907d5d41a432',originalStatus,
  oldAndroidHead:'5d417ca75a5fe845a7d6f6d9e16c720d55e19b36',oldAndroidClean:true},
 acceptance:{sourceCheckpoint:true,actualJni:false,newApk:false,renderedQuest:false,realHttpsWss:false,
  actualQuestAction:false,zoneAndSave:false,physicalDevice:false,globalParity:false,goal:'Active'},
 lastInstalledDiagnosticPackage:{version:19,source:'78e7d2309f6a6b92cea1737c76c5eedfd863e46e',containsThisSource:false},
 rendererGate:'FAIL: prior v19 135 GL0x506 across nine PID captures; no new renderer fix or capture'};
writeFileSync(proof+'source-binding.json',JSON.stringify(record,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({source,ownedFiles:Object.keys(sources).length,gates:gates.length,javaTotals,
 originalsPreserved:true,newApk:false,globalParity:false}));
