import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024';
const topic='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/docs/generated/cell-p0-p1-completion-20261024';
const target=path.join(topic,'capacity-native10-v2-compact-wire-followup-13-analysis');
if(process.argv.slice(2).join(' ')!=='--archive-explicit-public-evidence')throw Error('Explicit archive argument required');
const whitelist=[
 'probe13-root-independent-cleanup-v1.json',
 'probe13-collection-first-refusal-coordinator-record-v1.json',
 'compact-probe13-independent-review-v1/preseal-review-checklist-v1.json',
 'compact-probe13-independent-review-v1/preseal-independent-review-v1.json',
 'compact-probe13-collection-independent-review-v2/review-v2.json',
 'probe13-independent-verification-v1/verify-ended-probe13.mjs',
 'probe13-independent-verification-v1/verification-preparation-v1.json',
 'probe13-independent-verification-v1/verify-ended-probe13-v2.mjs',
 'probe13-independent-verification-v1/verification-preparation-v2.json',
 'probe13-independent-verification-v1/hp-proof-contract-v2.mjs',
 'probe13-independent-verification-v1/hp-proof-v2-selfcheck.mjs',
 'probe13-independent-verification-v1/hp-proof-v2-selfcheck.log',
 'probe13-independent-verification-v1/hp-proof-v2-selfcheck-receipt.json',
 'probe13-independent-verification-v1/audit-closed-raw-windows-v1.mjs',
 'probe13-independent-verification-v1/raw-window-contract-audit-v1.json',
 'probe13-independent-verification-v1/final-verification.json',
 'probe13-independent-verification-v1/preimages-v1/verify-ended-probe13.mjs.txt',
 'probe13-independent-verification-v1/preimages-v1/verification-preparation-v1.json',
 'probe13-independent-verification-v1/preimages-v1/manifest.json',
 'posthoc13-collection-v2/window-schema-diagnosis-v1.json',
 'posthoc13-collection-v2/verify-public-v2.mjs',
 'posthoc13-collection-v2/verify-public-v1.mjs.txt',
 'posthoc13-collection-v2/synthetic-v2.log',
 'posthoc13-collection-v2/synthetic-v2.json',
 'posthoc13-collection-v2/source-mutation-proof-v2.json',
 'posthoc13-collection-v2/posthoc-source-frozen-v2.json',
 'posthoc13-collection-v2/passed-probe-window-contract.test.mjs',
 'posthoc13-collection-v2/passed-probe-window-contract.mjs',
 'posthoc13-collection-v2/diagnose-window-schema.mjs',
 'posthoc13-collection-v2/create-coordinator-bindings-v1.mjs',
 'posthoc13-collection-v2/coordinator-bindings-v1.json',
 'posthoc13-collection-v2/collect-public-v2.mjs',
 'posthoc13-collection-v2/collect-public-v1.mjs.txt',
 'posthoc13-collection-v2/build-derived-collectors-v2.mjs',
 'archive-probe13-independent-evidence-v1.mjs',
];
const digest=b=>crypto.createHash('sha256').update(b).digest('hex');
const receiptFile=path.join(base,'probe13-independent-verification-v1/final-verification.json');
const receiptBytes=await fs.readFile(receiptFile),receipt=JSON.parse(receiptBytes);
if(receiptBytes.length!==359693||digest(receiptBytes)!=='c96b6cc7fbc960592f60c0b81dac3fc4d43238d424a2bfaf968d5528165e946c'||receipt.verifierRevision!==2||receipt.runId!=='1792912711217-e7ea81f2'||receipt.independentVerificationPassed!==true||receipt.probePassed!==true||receipt.capacityAccepted!==false||receipt.rawVerifierAccepted!==false)throw Error('Actual corrected independent receipt identity/verdict differs');
if(receipt.hp.totalRows!==0||receipt.hp.verifiedEventsChecked!==0||receipt.hp.successProofChecked!==false||receipt.hp.noVerifiedDoseObserved!==true)throw Error('Actual zero-event HP interpretation differs');
const inputs=[];
for(const relative of whitelist){
 if(path.isAbsolute(relative)||relative.includes('..')||/(?:^|\/)private(?:\/|$)|\.exe$/i.test(relative))throw Error('Nonpublic whitelist path');
 const file=path.join(base,relative),stat=await fs.lstat(file);
 if(!stat.isFile()||stat.isSymbolicLink())throw Error('Nonregular source');
 const bytes=await fs.readFile(file);inputs.push({relative,file,bytes,pin:{bytes:bytes.length,sha256:digest(bytes)}});
}
await fs.mkdir(target);
const copies=[];
for(const input of inputs){
 const destination=path.join(target,input.relative);
 await fs.mkdir(path.dirname(destination),{recursive:true});
 await fs.writeFile(destination,input.bytes,{flag:'wx'});
 const copied=await fs.readFile(destination),after=await fs.readFile(input.file);
 if(!copied.equals(input.bytes)||!after.equals(input.bytes))throw Error('Raw source/copy mutation');
 copies.push({source:input.file,copy:input.relative,...input.pin});
}
await fs.writeFile(path.join(target,'README.md'),`# Probe 13 independent evidence\n\nRun 1792912711217-e7ea81f2 passed the original 2/10-player 300-second stages and 360-second soak, with 5/5/6 complete windows of at least 60 actual seconds. Corrected independent verifier revision 2 passed. This is a formal-run precondition, not formal capacity acceptance.\n\nThe frozen collector's success-only branch incorrectly expected window rows in the generic selected projection. The separately reviewed posthoc collector captures at most 16 windows during its existing raw pass and preserves original source preimages and exact inverse proofs. Original runtime sources, report, raw data, thresholds and failed collection attempts remain unchanged. The first refusal record is a coordinator-authored account, not an original captured stderr file.\n\nThe original raw/public collection is in [the diagnostics package](../capacity-native10-v2-compact-wire-followup-13-diagnostics/README.md). This package adds independent verification, collection-fix provenance, synthetic evidence, preimages and root process cleanup. All explicit source copies are byte-identical and listed in copy-provenance-v1.json. No private stores, credentials, binaries or raw recordings are copied.\n\nThe full run contains zero HP-maintenance events: no real dose or long-term survival is claimed. Diagnostic trace completeness is separate from gameplay acceptance. The earlier 25-player stall and compact-12 corrections remain unresolved historical observations. No fixed Cell implementation is accepted by this probe.\n`,{flag:'wx'});
const provenance={schemaVersion:1,createdAt:new Date().toISOString(),method:'Explicit whitelist; pre-read, byte-identical wx copies, source-after verification; existing destinations refused.',runId:receipt.runId,independentVerificationPassed:true,probePassed:true,capacityAccepted:false,originalRuntimeSourcesEdited:false,originalRawOrReportEdited:false,copyCount:copies.length,copiedBytes:copies.reduce((s,p)=>s+p.bytes,0),copies};
const output=path.join(target,'copy-provenance-v1.json');
await fs.writeFile(output,JSON.stringify(provenance,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({target,copyCount:copies.length,copiedBytes:provenance.copiedBytes,receipt:output,capacityAccepted:false}));
