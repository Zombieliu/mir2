import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const root='C:/mir2-build/p01-completion-20261024/release-native-10-v2-account-store-wait-feasibility-17';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const pin=async file=>{const b=await fs.readFile(file);return{file,bytes:b.length,sha256:sha(b)}};
const changes=[];
for(const name of ['initialize-private.ps1','recover-drained-owned.ps1']){
 const file=path.join(root,name),before=await fs.readFile(file),text=before.toString('utf8');
 assert.equal(text.split('new44-release-and-full16-bound-not-executed').length,2);
 assert.equal(text.split('PENDING_FRESH44_RELEASE_AND_FULL16').length,2);
 const next=text.replace('new44-release-and-full16-bound-not-executed','new50-release-and-full16-bound-not-executed').replace('PENDING_FRESH44_RELEASE_AND_FULL16','PENDING_FRESH50_RELEASE_AND_FULL16');
 assert.equal(next.replace('new50-release-and-full16-bound-not-executed','new44-release-and-full16-bound-not-executed').replace('PENDING_FRESH50_RELEASE_AND_FULL16','PENDING_FRESH44_RELEASE_AND_FULL16'),text);
 const saved=file+'.before-fresh50-binding-repair-v1.txt';await fs.writeFile(saved,before,{flag:'wx'});await fs.writeFile(file,Buffer.from(next));
 changes.push({before:await pin(saved),after:await pin(file),onlyTwoLiteralReplacements:true});
}
const previousFile=path.join(root,'runtime-prepared-v3.json'),original=JSON.parse(await fs.readFile(previousFile,'utf8')),updated=structuredClone(original);
for(const change of changes){const entry=updated.files.find(p=>path.resolve(p.file)===path.resolve(change.after.file));assert.ok(entry);assert.equal(entry.bytes,change.before.bytes);assert.equal(entry.sha256,change.before.sha256);entry.bytes=change.after.bytes;entry.sha256=change.after.sha256;}
updated.sourceSha256=sha(JSON.stringify(updated.files));
const restored=structuredClone(updated);for(const change of changes){const entry=restored.files.find(p=>path.resolve(p.file)===path.resolve(change.after.file));entry.bytes=change.before.bytes;entry.sha256=change.before.sha256;}restored.sourceSha256=original.sourceSha256;assert.deepEqual(restored,original);
const prepared=path.join(root,'runtime-prepared-v4.json');await fs.writeFile(prepared,JSON.stringify(updated,null,2)+'\n',{flag:'wx'});
const receipt={schemaVersion:1,recordedAt:new Date().toISOString(),previousPrepared:await pin(previousFile),prepared:await pin(prepared),changes,allOtherPreparedFieldsUnchanged:true,nodeJavaScriptUnchanged:true,nodeChecksReexecuted:false,observedRootExecutions:{newSealExitCode:0,readonlyGuardExitCode:0,prepareExitCode:1,prepareError:'initialize-private.ps1 rejected new50 candidate because its guard still required new44; PENDING_FRESH44_RELEASE_AND_FULL16',stdioEvidence:'Observed Root tool output only; no raw stdout/stderr file was created.'},accountsCreated:0,realmStarted:false,powerShellParsingNotYetExecuted:true};
const receiptFile=path.join(root,'root-powershell-binding-repair-v1.json');await fs.writeFile(receiptFile,JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({prepared:await pin(prepared),receipt:await pin(receiptFile),changes}));
