import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const base='C:/mir2-build/p01-completion-20261024';
const root=path.join(base,'formal-native-v2-compact-wire-ladder-10');
const file=path.join(root,'verify-probe13-collected-evidence.mjs');
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const pin=async file=>{const bytes=await fs.readFile(file);return{file,bytes:bytes.length,sha256:sha(bytes)}};
const oldReceiptFile=path.join(base,'compact-wire-formal10-preseal-input-check-v1.json');
const oldReceipt=JSON.parse(await fs.readFile(oldReceiptFile,'utf8'));
const before=await fs.readFile(file),oldPin=oldReceipt.reviewedFiles.find(p=>p.file===file);
assert.deepEqual(await pin(file),oldPin);
const preimage=path.join(base,'compact-wire-formal10-preseal-helper-v1.mjs.txt');
await fs.writeFile(preimage,before,{flag:'wx'});
const rules=[
 {label:'Actual independent receipt uses lowerCamel hp',before:'receipt.HP',after:'receipt.hp',expectedMatches:3},
 {label:'Actual independent terminal pin uses fixed relative path plus exact bytes/SHA',before:'JSON.stringify(w.terminal)!==JSON.stringify(terminalPin)',after:"w.terminal?.path!==`${realm}/${WIRE_TERMINAL}`||w.terminal.bytes!==terminalPin.bytes||w.terminal.sha256!==terminalPin.sha256",expectedMatches:1},
];
let changed=before.toString('utf8');
for(const rule of rules){assert.equal(changed.split(rule.before).length-1,rule.expectedMatches);changed=changed.split(rule.before).join(rule.after);}
let inverse=changed;for(const rule of [...rules].reverse())inverse=inverse.split(rule.after).join(rule.before);
assert.ok(Buffer.from(inverse).equals(before));
await fs.writeFile(file,changed);
const preserved=[];for(const p of oldReceipt.reviewedFiles){if(p.file!==file){assert.deepEqual(await pin(p.file),p);preserved.push(p);}}
const producer=path.join(base,'probe13-independent-verification-v1/verify-ended-probe13.mjs');
const producerText=await fs.readFile(producer,'utf8');
assert.ok(producerText.includes('hp:{observedStatuses:hpStatuses'));
assert.ok(producerText.includes('terminal:wireTerminalPin'));
const result={schemaVersion:1,recordedAt:new Date().toISOString(),scope:'Unsealed formal10 success-only schema normalization; no gate or live data was evaluated.',oldPreseal:await pin(oldReceiptFile),originalSource:oldPin,preservedPreimage:await pin(preimage),newSource:await pin(file),logicalChanges:rules,exactInverseRestoresOriginalBytes:true,otherLocalInputsUnchanged:preserved.length,producerSchemaSource:await pin(producer),originalPolicyUnchanged:true,originalGameplayThresholdsUnchanged:true,diagnosticCompletenessNotGameplayGate:true,sourceGuardExecuted:false,gateInvoked:false,liveProbeRawRead:false,realmStarted:false,sealed:false};
const deltaFile=path.join(base,'compact-wire-formal10-schema-normalization-v2.json');
await fs.writeFile(deltaFile,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
const checkerV1=path.join(base,'check-compact-wire-formal10-preseal-v1.mjs');
let checker=await fs.readFile(checkerV1,'utf8');
assert.equal(checker.split('compact-wire-formal10-preseal-input-check-v1.json').length-1,1);
checker=checker.replace('compact-wire-formal10-preseal-input-check-v1.json','compact-wire-formal10-preseal-input-check-v2.json');
const outputAnchor="const file=path.join(base,'compact-wire-formal10-preseal-input-check-v2.json');";
assert.ok(checker.includes(outputAnchor));
checker=checker.replace(outputAnchor,"result.schemaNormalization=await pin(path.join(base,'compact-wire-formal10-schema-normalization-v2.json'));"+outputAnchor);
const checkerV2=path.join(base,'check-compact-wire-formal10-preseal-v2.mjs');
await fs.writeFile(checkerV2,checker,{flag:'wx'});
console.log(JSON.stringify({delta:await pin(deltaFile),checker:await pin(checkerV2),source:await pin(file),otherLocalInputsUnchanged:preserved.length,exactInverse:true,liveRawRead:false}));
