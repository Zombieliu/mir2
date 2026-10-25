import fs from 'node:fs/promises';import{createReadStream,constants}from'node:fs';import path from'node:path';import assert from'node:assert/strict';
import{root,base,read,pin,writeNew,samePath,verifyRelease,assertPin}from'./input-lib.mjs';
import{assertFinalDriverBinding}from'./driver-binding-contract.mjs';
import{verifyStoreDiagnosticSource,storeDiagnosticIdentity,validateAccountStorePublicDataset}from'./store-diagnostics-contract.mjs';
assertFinalDriverBinding();
if(process.argv.slice(2).join(' ')!=='--collect-store-after-owned-cleanup')throw Error('Explicit ended Store collection argument required');
const source=await verifyStoreDiagnosticSource(),identity=storeDiagnosticIdentity(source),binding=await verifyRelease();
const topic=path.join('C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/docs/generated/cell-p0-p1-completion-20261024');
const child='capacity-native10-v2-scout-observer-followup-18-store-diagnostics',directory=path.join(topic,child),gameDirectory=path.join(topic,'capacity-native10-v2-scout-observer-followup-18-diagnostics');
const cleanup=await read(path.join(root,'realm-cleanup.json'));
if(Object.values(cleanup.checks??{}).length!==7||Object.values(cleanup.checks).some(v=>v!==true)||cleanup.inputsChangedWhileRunning||cleanup.gatewayExit?.code!==0||cleanup.monitorExit?.code!==0)throw Error('Store collection requires actual complete normal owned cleanup');
const processRecord=await read(path.join(root,'realm-process.json'));
assert.deepEqual(processRecord.storeDiagnostics,identity);assert.equal(processRecord.backendSourceCommit,binding.backendSourceCommit);assert.equal(processRecord.sha256,binding.release.binary.sha256);
const baseCatalogFile=path.join(gameDirectory,'collect-release-native-10-v2-scout-observer-feasibility-18-public-artifact-catalog.json'),baseVerificationFile=path.join(gameDirectory,'public-verification.json');
const baseCatalog=await read(baseCatalogFile),baseVerification=await read(baseVerificationFile);
assert.equal(baseCatalog.run?.runId,baseVerification.runId);assert.equal(baseCatalog.run.backendSourceCommit,binding.backendSourceCommit);assert.equal(baseVerification.capacityAccepted,false);assert.equal(baseVerification.originalVerdictPreserved,true);
const statusFile=path.join(root,'store-stage-public-status.json'),dataFile=path.join(root,'store-stage-public.jsonl'),status=await read(statusFile);
assert.equal(status.thresholdMs,50);assert.ok(samePath(status.dataFile??'',dataFile));assert.deepEqual(status.sourceIdentity,identity);
for(const file of[statusFile,dataFile]){const s=await fs.lstat(file);if(!s.isFile()||s.isSymbolicLink()||!samePath(await fs.realpath(file),file))throw Error('Store public data redirected');}
const original=[await pin(statusFile),await pin(dataFile)];if(original[0].bytes>65536||original[1].bytes>33554432)throw Error('Independent Store artifact cap exceeded');
const validation=await validateAccountStorePublicDataset({input:createReadStream(dataFile),status});
for(const p of original)await assertPin(p);
if(!validation.countsMatch||!validation.readEnded||!validation.formatValid||validation.sha256!==original[1].sha256||validation.bytes!==original[1].bytes)throw Error('Store ended file/status/raw counts or exact schema mismatch; original game verdict remains untouched');
await fs.mkdir(directory);const copies=[];
for(const p of original){const file=path.join(directory,path.basename(p.file));await fs.copyFile(p.file,file,constants.COPYFILE_EXCL);const destination=await pin(file);assert.equal(destination.bytes,p.bytes);assert.equal(destination.sha256,p.sha256);copies.push({source:p,destination});}
const catalog={schemaVersion:1,createdAt:new Date().toISOString(),dataset:'account-store',runId:baseVerification.runId,sourceIdentity:identity,backendSourceCommit:binding.backendSourceCommit,driverSourceCommit:binding.driverSourceCommit,backendInputCount:50,gatewaySha256:binding.release.binary.sha256,originalReportOk:baseVerification.reportOk,originalCapacityAccepted:false,gameplayVerdictModified:false,contributesToGameAcceptance:false,diagnosticComplete:validation.complete,validation,copies,baseCatalog:await pin(baseCatalogFile),baseVerification:await pin(baseVerificationFile)};
await writeNew(path.join(directory,'store-artifact-catalog.json'),catalog);
await writeNew(path.join(directory,'store-copy-provenance.json'),{schemaVersion:1,dataset:'account-store',files:copies,sourceIdentity:identity,originalGameDataModified:false});
console.log(JSON.stringify({directory,rows:validation.rows,bytes:validation.bytes,sha256:validation.sha256,diagnosticComplete:validation.complete,capacityAccepted:false}));
