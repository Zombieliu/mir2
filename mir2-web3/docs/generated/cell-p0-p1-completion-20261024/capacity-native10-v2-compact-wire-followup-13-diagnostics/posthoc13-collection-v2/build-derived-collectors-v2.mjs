import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
const base='C:/mir2-build/p01-completion-20261024',realm='release-native-10-v2-compact-wire-feasibility-13',root=path.join(base,'posthoc13-collection-v2'),original=path.join(base,realm);
const sha=b=>crypto.createHash('sha256').update(b).digest('hex'),pin=async file=>{const b=await fs.readFile(file);return{file,bytes:b.length,sha256:sha(b)}};
const freezeFile=path.join(original,'launcher-source-frozen-v1.json'),freeze=JSON.parse(await fs.readFile(freezeFile,'utf8'));
for(const p of freeze.files)assert.deepEqual(await pin(p.file),p,'Original46 source pin changed');
const derived=[],rulesByFile={};
const companions=['collect-public-v2.mjs','verify-public-v2.mjs','passed-probe-window-contract.mjs','passed-probe-window-contract.test.mjs','build-derived-collectors-v2.mjs','source-mutation-proof-v2.json','synthetic-v2.log','synthetic-v2.json','posthoc-source-frozen-v2.json','window-schema-diagnosis-v1.json','diagnose-window-schema.mjs','collect-public-v1.mjs.txt','verify-public-v1.mjs.txt'];
for(const name of ['collect-public','verify-public']){
 const file=path.join(original,name+'.mjs'),bytes=await fs.readFile(file);await fs.writeFile(path.join(root,name+'-v1.mjs.txt'),bytes,{flag:'wx'});
 const text=bytes.toString('utf8'),rules=[];let changed=text;
 const replacements=[];for(const match of text.matchAll(/from '\.\/([^']+)'/g))replacements.push([match[0],`from '../${realm}/${match[1]}'`]);
 for(const [before,after]of replacements){assert.equal(changed.split(before).length-1,1);changed=changed.replace(before,after);rules.push({kind:'resolve-original-readonly-dependency',before,after});}
 const importLine="import {createPassedProbeWindowCounter} from './passed-probe-window-contract.mjs';\n";changed=importLine+changed;rules.push({kind:'supplemental-window-contract-import',before:'',after:importLine,prefix:true});
 const add=(kind,before,after)=>{assert.equal(changed.split(before).length-1,1,kind);changed=changed.replace(before,after);rules.push({kind,before,after});};
 if(name==='collect-public'){
  add('bounded-single-existing-raw-pass-counter','const rawActions = {}, windows = [], typeCounts = {}, windowsByPhase = {}, wireRaw=createWireRawCounter();','const rawActions = {}, windows = [], typeCounts = {}, windowsByPhase = {}, wireRaw=createWireRawCounter(), minuteWindows=report.ok===true?createPassedProbeWindowCounter(report):null;');
  add('observe-only-supplemental-windows-in-existing-pass','  wireRaw.observe(row);','  wireRaw.observe(row); minuteWindows?.observe(row);');
  add('posthoc-source-allowlist',"// No optional arbitrary analysis path: only the explicit public allowlist is copied.",`sourcePaths.push(...${JSON.stringify(companions)}.map(name=>'posthoc13-collection-v2/'+name),'probe13-independent-verification-v1/raw-window-contract-audit-v1.json');\n// No optional arbitrary analysis path: only the explicit public allowlist is copied.`);
  add('posthoc-catalog-identity',"scope: 'Independent explicit public allowlist for authentic non-accepting ordinary native compact span2-halo probe13.","scope: 'Posthoc revision2 collection with an explicit bounded minute-window schema correction; original runtime46 inputs remain unchanged. Independent explicit public allowlist for authentic non-accepting ordinary native compact span2-halo probe13.");
 }else{
  const before='const wireRaw=createWireRawCounter();for await(const line of createInterface({input:createReadStream(expectedRaw),crlfDelay:Infinity})){assert.ok(Buffer.byteLength(line)<=16*1024*1024);if(line)wireRaw.observe(JSON.parse(line));}';
  const after='const wireRaw=createWireRawCounter(),minuteWindows=report.ok===true?createPassedProbeWindowCounter(report):null;for await(const line of createInterface({input:createReadStream(expectedRaw),crlfDelay:Infinity})){assert.ok(Buffer.byteLength(line)<=16*1024*1024);if(line){const row=JSON.parse(line);wireRaw.observe(row);minuteWindows?.observe(row);}}';
  add('bounded-single-existing-raw-pass-counter',before,after);
 }
 add('require-exact-complete16-supplemental-windows',' api.verifyCapacityPhases(report,streamed.selected);',' api.verifyCapacityPhases(report,streamed.selected); minuteWindows.assertComplete();');
 add('use-real-minute-windows-not-generic-phase-projection',"rawWindows=streamed.selected.filter(r=>r.type==='window'&&r.phase===name)","rawWindows=minuteWindows.forPhase(name)");
 let inverse=changed;for(const rule of [...rules].reverse()){if(rule.prefix){assert.ok(inverse.startsWith(rule.after));inverse=inverse.slice(rule.after.length);}else{assert.equal(inverse.split(rule.after).length-1,1);inverse=inverse.replace(rule.after,rule.before);}}
 assert.ok(Buffer.from(inverse).equals(bytes),'Exact inverse must restore old source bytes');
 const output=path.join(root,name+'-v2.mjs');await fs.writeFile(output,changed,{flag:'wx'});
 derived.push({original:await pin(file),preimage:await pin(path.join(root,name+'-v1.mjs.txt')),derived:await pin(output),exactInverseRestoresOriginalBytes:true});rulesByFile[name]=rules;
}
const diagnostic=JSON.parse(await fs.readFile(path.join(root,'window-schema-diagnosis-v1.json'),'utf8'));
const proof={schemaVersion:1,recordedAt:new Date().toISOString(),state:'posthoc-derived-source-not-runtime-frozen',cause:'The generic frozen verifier SELECTED_RAW excludes window records. The original success-only collector/verifier incorrectly filtered that phase/status projection and always counted zero minute windows.',runId:'1792912711217-e7ea81f2',originalRuntimeFreeze:await pin(freezeFile),originalRuntimeFilesCheckedUnchanged:freeze.files.length,originalRuntimeFiles:freeze.files,derived,rulesByFile,rawWindowAudit:await pin(path.join(base,'probe13-independent-verification-v1/raw-window-contract-audit-v1.json')),ownWindowSchemaDiagnosis:await pin(path.join(root,'window-schema-diagnosis-v1.json')),raw:diagnostic.raw,sourceCodeChangesOnly:['Resolve unchanged original dependencies from the new posthoc directory.','Add bounded16-window projection in each already-existing raw pass, with exact report assessment and real>=60s elapsed checks.','Use those actual windows in the original success-only conditions; keep every original game gate.','Add explicit posthoc source provenance to the public allowlist and catalog identity.'],policyChanged:false,durationThresholdChanged:false,rawOrReportChanged:false,oldFailureArtifactsChanged:false,originalGenericApiChanged:false,capacityAccepted:false,collectionExecuted:false};
await fs.writeFile(path.join(root,'source-mutation-proof-v2.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
const testFile=path.join(root,'passed-probe-window-contract.test.mjs'),before=await Promise.all(['passed-probe-window-contract.mjs','passed-probe-window-contract.test.mjs','collect-public-v2.mjs','verify-public-v2.mjs'].map(n=>pin(path.join(root,n))));
const run=spawnSync(process.execPath,['--test',testFile],{windowsHide:true,encoding:null,maxBuffer:1024*1024});
const log=path.join(root,'synthetic-v2.log');await fs.writeFile(log,Buffer.concat([run.stdout??Buffer.alloc(0),run.stderr??Buffer.alloc(0)]),{flag:'wx'});
const after=await Promise.all(before.map(p=>pin(p.file)));assert.deepEqual(before,after);
const tap=(run.stdout??Buffer.alloc(0)).toString('utf8'),count=k=>Number(new RegExp('^# '+k+' (\\d+)$','m').exec(tap)?.[1]);
const synthetic={schemaVersion:1,recordedAt:new Date().toISOString(),scope:'Only minute-clock/schema boundary synthetic assertions; no fake full gameplay acceptance.',tests:count('tests'),passed:count('pass'),failed:count('fail'),exitCode:run.status,log:await pin(log),sourceBefore:before,sourceAfter:after,sourceUnchanged:true,liveRawRead:false,gameModulesEvaluated:false,realmStarted:false};
await fs.writeFile(path.join(root,'synthetic-v2.json'),JSON.stringify(synthetic,null,2)+'\n',{flag:'wx'});assert.equal(run.status,0);assert.equal(synthetic.tests,6);assert.equal(synthetic.passed,6);assert.equal(synthetic.failed,0);
for(const name of ['collect-public-v2.mjs','verify-public-v2.mjs','passed-probe-window-contract.mjs','passed-probe-window-contract.test.mjs']){const check=spawnSync(process.execPath,['--check',path.join(root,name)],{windowsHide:true,encoding:null});assert.equal(check.status,0);}
for(const p of freeze.files)assert.deepEqual(await pin(p.file),p);
const sourceFiles=await Promise.all(companions.filter(n=>n!=='posthoc-source-frozen-v2.json').map(n=>pin(path.join(root,n))));
const sourceFreeze={schemaVersion:1,frozenAt:new Date().toISOString(),state:'posthoc-source-prepared-for-independent-review-only',runId:proof.runId,originalRuntimeFreeze:proof.originalRuntimeFreeze,originalRuntimeFilesUnchanged:freeze.files.length,files:sourceFiles,sourceSha256:sha(JSON.stringify(sourceFiles)),independentAudit:proof.rawWindowAudit,synthetic:await pin(path.join(root,'synthetic-v2.json')),rawOrReportChanged:false,originalGenericApiChanged:false,originalGamePolicyChanged:false,collectionExecuted:false,realmStarted:false,capacityAccepted:false};
await fs.writeFile(path.join(root,'posthoc-source-frozen-v2.json'),JSON.stringify(sourceFreeze,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({proof:await pin(path.join(root,'source-mutation-proof-v2.json')),freeze:await pin(path.join(root,'posthoc-source-frozen-v2.json')),derived,synthetic:await pin(path.join(root,'synthetic-v2.json')),original46Unchanged:true,collectionExecuted:false}));
