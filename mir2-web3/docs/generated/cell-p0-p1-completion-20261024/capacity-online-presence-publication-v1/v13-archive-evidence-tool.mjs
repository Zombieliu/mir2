import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

// Archival only: fixed allowlist, one read/hash of each source in this operation,
// exact byte copies, and exclusive target creation. No raw stream/private store,
// status document, game source, build, load, process control or Git operation.
const base='C:/mir2-build/p01-completion-20261024';
const dest='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/docs/generated/cell-p0-p1-completion-20261024/capacity-online-presence-publication-v1';
const run='native-steady-single-stage-diagnostic-v13';
const analysis='steady-presence-reuse-v13-analysis-v1';
const pairs=[
 ['snapshot-reuse-v13-stage50-root-independent-cleanup-v1.json','v13-root-independent-cleanup.json'],
 [`${run}/episode-50/reports/1793035283410-2a81573b.report.json`,'v13-original-report.json'],
 [`${run}/episode-50/episode-receipt.json`,'v13-original-episode-receipt.json'],
 [`${run}/online-presence-reuse-v13-root-guard-execution-v1.json`,'v13-root-guard-execution.json'],
 [`${run}/online-presence-reuse-v13-stage50-root-prepare-execution-v1.json`,'v13-root-prepare-execution.json'],
 [`${run}/online-presence-reuse-v13-stage50-root-run-execution-v1.json`,'v13-root-run-execution.json'],
 [`${run}/source-pins-v1.json`,'v13-source-pins.json'],
 [`${run}/episode-50/source-pins.json`,'v13-episode-source-pins.json'],
 [`${run}/episode-50/source-pins-admitted-v2.json`,'v13-admitted-source-pins.json'],
 [`${run}/episode-50/report-provenance.json`,'v13-report-provenance.json'],
 [`${run}/episode-50/realm-cleanup.json`,'v13-realm-cleanup.json'],
 [`${run}/episode-50/gateway-build-pin.json`,'v13-gateway-build-pin.json'],
 [`${analysis}/findings-final-v2.json`,'v13-findings-final.json'],
 [`${analysis}/selected-causal-chain-v2.jsonl`,'v13-selected-causal-chain.jsonl'],
 [`${analysis}/final-saved-actor-state-v1.json`,'v13-final-saved-actor-state.json'],
 [`${analysis}/manifest-final-v2.json`,'v13-analysis-manifest.json'],
 ['archive-presence-v13-evidence-v1.mjs','v13-archive-evidence-tool.mjs'],
];
for(const name of ['source-lib-v5.mjs','build-release-v5.mjs','root-final-config-v5.json','backend-source-v5.json','release-v5.json','build-receipt-v5.json','source-before-v5.json','source-after-v5.json','build-stdout-v5.log','build-stderr-v5.log'])pairs.push([`online-presence-reuse-release-v5/${name}`,`release-v5/${name}`]);
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const prepared=[];
for(const [relative,file] of pairs){
 if(relative.includes('/private/')||relative.includes('.samples.jsonl')||relative.includes('all-strict-anonymous')||relative.includes('gateway.stderr'))throw new Error('Non-archival input blocked');
 const target=path.resolve(dest,file);
 if(!target.startsWith(path.resolve(dest)+path.sep))throw new Error('Archive path escapes target');
 try{await fs.access(target);throw new Error('Archive target already exists; refusing overwrite');}catch(e){if(e.code!=='ENOENT')throw e;}
 const source=path.join(base,relative),bytes=await fs.readFile(source);
 prepared.push({source,file,target,bytes,sha256:sha(bytes)});
}
const byName=name=>prepared.find(p=>p.file===name);
const parse=name=>JSON.parse(byName(name).bytes);
const report=parse('v13-original-report.json'),saved=parse('v13-final-saved-actor-state.json'),findings=parse('v13-findings-final.json'),cleanup=parse('v13-root-independent-cleanup.json');
const measurement=report.phaseWindows?.find(p=>p.kind==='measurement');
if(report.ok!==false||report.playableCapacityAccepted!==false||!Array.isArray(report.stages)||report.stages.length!==0)throw new Error('Original verdict/stage contract changed');
if(!measurement||measurement.completed!==false||measurement.endedAtMs-measurement.startedAtMs!==55861)throw new Error('Partial measurement bound changed');
if(saved.finalSavedState?.hp!==0||saved.finalSavedState?.max_hp!==18||saved.finalSavedState?.gold!==0||[658,660].some(i=>saved.finalSavedState?.hpTemplateStock?.[i]?.total!==0))throw new Error('Final scalar state differs from authorised summary');
if(findings.originalVerdicts?.ok!==false||findings.originalVerdicts?.playableCapacityAccepted!==false)throw new Error('Derived verdict differs');
if(cleanup.ownedPidAbsent!==true||cleanup.portsFree!==true||cleanup.cleanupComplete!==true)throw new Error('Independent cleanup is incomplete');
await fs.mkdir(path.join(dest,'release-v5'),{recursive:false});
const copies=[];
for(const p of prepared){await fs.writeFile(p.target,p.bytes,{flag:'wx'});const s=await fs.stat(p.target);if(s.size!==p.bytes.length)throw new Error('Archive size verification failed');copies.push({source:p.source,file:p.file,bytes:p.bytes.length,sha256:p.sha256,sourceReadPassesForThisOperation:1,exclusiveWrite:true});}
const manifest={schemaVersion:1,scope:'V13 bounded offline evidence archive only. Original input bytes and false verdicts preserved; no capacity/frontend/production acceptance claim.',capacityAccepted:false,p0Accepted:false,productionAccepted:false,formalAcceptanceEligible:false,stages:[],measurement:{startedAtMs:measurement.startedAtMs,endedAtMs:measurement.endedAtMs,wallSeconds:55.861,completed:false,onlineSamples:55,minimumOnlineActors:50,maximumOnlineActors:50},finalSavedState:{hp:0,maxHp:18,mp:14,maxMp:14,gold:0,hpTemplate658Total:0,hpTemplate660Total:0,scope:'Post-cleanup durable state, not an exact fatal-instant packet.'},aoiDelivery:{measurementProducedProducerToSentAtLeastOneSecondCount:findings.aoiDelivery.aoiProducerToSentAtLeastOneSecondCount,maximumProducerToSentUs:findings.aoiDelivery.aoiMaximumProducerToSentUs,meaning:'Server producer to send/flush return; not client receive or client membership failure.'},excluded:['Complete original samples/stderr streams','Complete anonymous derived streams','Private accounts store','Release binaries','PENDING build configuration'],copies};
const manifestBytes=Buffer.from(JSON.stringify(manifest,null,2)+'\n');
const manifestName='v13-copied-evidence-manifest-v1.json';
await fs.writeFile(path.join(dest,manifestName),manifestBytes,{flag:'wx'});
console.log(JSON.stringify({destination:dest,copied:copies.length,totalCopiedBytes:copies.reduce((n,p)=>n+p.bytes,0),files:copies.map(p=>p.file),manifest:{file:manifestName,bytes:manifestBytes.length,sha256:sha(manifestBytes)},capacityAccepted:false,formalAcceptanceEligible:false,stages:[],measurementWallSeconds:55.861}));
