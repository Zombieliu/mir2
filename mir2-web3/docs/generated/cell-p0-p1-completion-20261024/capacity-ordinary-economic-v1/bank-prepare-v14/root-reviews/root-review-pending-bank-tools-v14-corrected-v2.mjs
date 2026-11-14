import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const directory=process.argv[2];
if(!directory||path.basename(directory)!=='ordinary-capacity-bank-prepare-v14')throw Error('exact W14 directory required');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const read=n=>JSON.parse(fs.readFileSync(path.join(directory,n),'utf8'));
const assert=(v,m)=>{if(!v)throw Error(m);};
const pin=p=>{const b=fs.readFileSync(p.file);assert(b.length===p.bytes&&sha(b)===p.sha256,'pin mismatch: '+path.basename(p.file));return b;};
const d=read('derivation-v1.json'),plan=read('source-bank-plan-pending-v1.json');
assert(d.currentSourceQualification===null&&d.actualCurrentBackendVerification===null&&plan.newCurrentSourceQualification===null,'pending qualification required');
assert(d.toolDiffs.length===5,'five tools required');
const counts=[0,3,2,4,0],tools=[];
for(let i=0;i<5;i++){
  const row=d.toolDiffs[i],before=pin(row.source),after=pin(row.current);
  assert(row.hunks.length===counts[i],'unexpected hunk count');
  let forward=before.toString('utf8');
  for(const [from,to,count] of row.literalRules){assert(forward.split(from).length-1===count,'literal count');forward=forward.split(from).join(to);}
  assert(Buffer.from(forward).equals(after),'forward literal reconstruction differs');
  let reverse=after.toString('utf8');
  for(const h of [...row.hunks].reverse()){
    assert(reverse.slice(h.newOffsetUtf16,h.newOffsetUtf16+h.after.length)===h.after,'inverse hunk anchor');
    reverse=reverse.slice(0,h.newOffsetUtf16)+h.before+reverse.slice(h.newOffsetUtf16+h.after.length);
  }
  assert(Buffer.from(reverse).equals(before),'reverse reconstruction differs');
  tools.push({file:row.name,hunks:counts[i],forwardAndReverseExact:true});
}
assert(d.sourceBank.fileCount===11&&plan.files.length===11&&plan.totalBytes===441799,'complete bank required');
for(const p of plan.files)pin(p);
for(const p of Object.values(d.sourcePublicEvidence))pin(p);
assert(plan.priorRecoveryArchive.files.length===4,'four prior controls required');
for(const p of plan.priorRecoveryArchive.files)pin(p);
assert(plan.sourceReadyCharacters===30&&plan.sourceRegisteredAccounts===70&&plan.sourcePreparedCharacters.length===29&&plan.sourcePreparedEligibilityInherited===false,'pool/readiness distinction differs');
assert(d.sourceBank.journalPayloads===0&&plan.copyPolicy.authLedgerCopiedUnchanged===true&&plan.copyPolicy.loginNotBeforeOverrideAllowed===false&&plan.copyPolicy.seederInvoked===false,'copy/auth policy differs');
const receipt={schemaVersion:1,packageId:d.packageId,pendingToolsReviewed:true,approved:true,tools,source:plan.files,sourceTotalBytes:plan.totalBytes,sourceReadyCharacters:30,sourcePreparedCount:29,sourceQualificationInherited:false,sourcePublicEvidencePinsExact:true,priorControlsExact:true,ledgerUnchanged:true,currentQualificationPending:true,noModeExecuted:true,capacityAccepted:false,p0Accepted:false};
const file=path.join(directory,'root-pending-tools-review-v1.json');
fs.writeFileSync(file,JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
const b=fs.readFileSync(file);process.stdout.write(JSON.stringify({file,bytes:b.length,sha256:sha(b),approved:true})+'\n');
