import fs from 'node:fs/promises'; import path from 'node:path'; import crypto from 'node:crypto'; import assert from 'node:assert/strict';
const base='C:/mir2-build/p01-completion-20261024',directory=path.join(base,'probe14-movement-correction-analysis-v1'),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const pin=async file=>{const bytes=await fs.readFile(file);return{file,bytes:bytes.length,sha256:sha(bytes)};};
const selected=JSON.parse(await fs.readFile(path.join(directory,'selected-corrections-v2.json'))),causal=JSON.parse(await fs.readFile(path.join(directory,'bounded-causality-v1.json')));
const independentFile=path.join(base,'probe14-independent-verification-v1/final-verification.json'),independent=JSON.parse(await fs.readFile(independentFile));
assert.equal(independent.independentVerificationPassed,true);assert.equal(independent.probePassed,false);assert.equal(independent.capacityAccepted,false);assert.equal(independent.rawVerifierAccepted,false);
assert.equal(independent.raw.sha256,selected.raw.sha256);assert.equal(independent.raw.bytes,selected.raw.bytes);assert.equal(independent.raw.rows,selected.raw.rows);assert.equal(independent.raw.correctedActionsTruncated,false);
const actual=selected.cases.map(item=>Object.fromEntries(['line','at','actor','phase','kind','plannedTarget','actual','latencyMs'].map(key=>[key,item.action[key]])));
assert.deepEqual(actual,independent.raw.correctedActions);assert.equal(actual.length,5);assert.equal(causal.cases.filter(item=>item.insideFailingWindow).length,4);
assert.ok(causal.cases.every(item=>item.previousOwnerReceiptAlreadyBeforeCurrentSend));
const verification={schemaVersion:1,verifiedAt:new Date().toISOString(),runId:selected.runId,independentEndedReceipt:await pin(independentFile),sourceRaw:selected.raw,
 fullStageCorrections:5,failedContinuousWindowCorrections:4,allCorrectedRowsExactAgainstIndependentReplay:true,allFivePriorOwnerReceiptsAlreadyArrived:true,
 boundedConnection1WireEvents:causal.cases.reduce((sum,item)=>sum+item.events.length,0),spectatorPayloadRead:false,privateDataRead:false,totalAnalysisRawFullReads:2,
 interpretation:'Exact client command/cursor/packet/cache correspondence. Authority rejection reasons and simultaneous occupancy are absent; no client-filter or server-authority bug is declared.',
 originalReportOk:false,capacityAccepted:false,sourceOrSceneOrThresholdChanged:false};
await fs.writeFile(path.join(directory,'verification-v1.json'),JSON.stringify(verification,null,2)+'\n',{flag:'wx'});
const names=['analyze-closed-raw-v1.mjs','analyze-closed-raw-v2.mjs','prepare-analysis-v2.mjs','analysis-tool-revision-v2.json','selected-corrections-v2.json','summary-v2.json','derive-bounded-causality-v1.mjs','bounded-causality-v1.json','verification-v1.json','README.md','finalize-public-whitelist-v1.mjs'];
const files=await Promise.all(names.map(name=>pin(path.join(directory,name))));
const whitelist={schemaVersion:1,createdAt:new Date().toISOString(),scope:'Explicit final public-safe bounded14 movement correction analysis. Original failed report and all existing public evidence remain unchanged. Fullraw, private data and spectator payload are excluded.',runId:selected.runId,files,fileCount:files.length,totalBytes:files.reduce((sum,file)=>sum+file.bytes,0),fullRawIncluded:false,capacityAccepted:false};
const file=path.join(directory,'public-whitelist-v1.json');await fs.writeFile(file,JSON.stringify(whitelist,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({whitelist:await pin(file),files:files.length,bytes:whitelist.totalBytes,verification:await pin(path.join(directory,'verification-v1.json')),boundedCausality:await pin(path.join(directory,'bounded-causality-v1.json')),selected:await pin(path.join(directory,'selected-corrections-v2.json'))}));
