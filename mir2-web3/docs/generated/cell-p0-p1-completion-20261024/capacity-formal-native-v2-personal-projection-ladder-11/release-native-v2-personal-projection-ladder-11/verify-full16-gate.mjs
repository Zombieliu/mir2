import fs from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024',directory=path.join(base,'full-lifecycle-personal-monster-projection-v1/public');
const same=(a,b)=>path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();
const hex=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
async function verified(entry){
  if(!entry||!same(path.dirname(entry.file??''),directory)||!/^\d{13}-[a-f0-9]{8}[a-zA-Z0-9_.-]*$/.test(path.basename(entry.file))||!Number.isSafeInteger(entry.bytes)||entry.bytes<1||!hex(entry.sha256))throw Error('Only exact public fresh44 full16 artifact pins are permitted');
  if(!same(await fs.realpath(entry.file),entry.file)||(await fs.lstat(entry.file)).isSymbolicLink())throw Error('Redirected full16 artifacts are forbidden');
  const before=await fs.stat(entry.file),hash=crypto.createHash('sha256');let bytes=0;
  for await(const chunk of createReadStream(entry.file)){bytes+=chunk.length;hash.update(chunk);}
  const after=await fs.stat(entry.file);
  if(bytes!==entry.bytes||hash.digest('hex')!==entry.sha256||before.size!==bytes||after.size!==bytes||before.mtimeMs!==after.mtimeMs)throw Error('Full16 public artifact changed');
  return entry;
}
export async function verifyFull16Gate(binding,release,manifest){
  const full=binding.full16;
  if(binding.state!=='new44-release-and-full16-bound-not-executed'||full?.state!=='passed'||!Array.isArray(full.files)||full.files.length<6||full.files.length>64)throw Error('Fresh enabled50 fresh44 full16 acceptance is still pending');
  if(!same(await fs.realpath(directory),directory))throw Error('Full16 public directory redirected');
  for(const entry of full.files)await verified(entry);
  const auditPin=full.files.find(e=>same(e.file,full.audit?.file??''));
  if(!auditPin||auditPin.bytes!==full.audit.bytes||auditPin.sha256!==full.audit.sha256)throw Error('Full16 audit is not in the exact pinned public artifact set');
  const audit=JSON.parse(await fs.readFile(auditPin.file,'utf8'));
  if(!/^\d{13}-[a-f0-9]{8}$/.test(audit.runId??'')||!/^\d{13}-[a-f0-9]{8}$/.test(audit.realmStamp??'')||audit.all16Passed!==true||audit.onlyOrdinarySentCommands!==true||audit.backendInputCount!==44||
    audit.backendSourceCommit!==release.sourceCommit||audit.backendSourceAggregate!==manifest.sourceManifestSha256||audit.binarySha256!==release.binary.sha256||audit.sixCountersZeroBeforeAndAfter!==true||audit.normalExitCode!==0||
    audit.credentialValuesAbsent!==true||audit.pairedDeathReceiptsMatchRawTrace!==true||audit.pairedReviveAndKillDropPickupReceiptsMatchRawTrace!==true||audit.frozenDriverRawPinsMatched!==6)throw Error('Fresh matching full16 has not passed all original functional/raw/safety gates');
  const required=[`${audit.runId}.report.json`,`${audit.realmStamp}.realm-evidence.json`,`${audit.realmStamp}.source-manifest-verification.json`,`${audit.runId}-a.trace.jsonl`,`${audit.runId}-b.trace.jsonl`];
  for(const name of required)if(!full.files.some(e=>same(e.file,path.join(directory,name))))throw Error('Full16 report/realm/source proof/paired raw trace binding is incomplete');
  const report=JSON.parse(await fs.readFile(path.join(directory,required[0]),'utf8')),realm=JSON.parse(await fs.readFile(path.join(directory,required[1]),'utf8')),source=JSON.parse(await fs.readFile(path.join(directory,required[2]),'utf8'));
  if(report.ok!==true||report.fullLifecyclePassed!==true||report.requiredChecks?.length!==16||report.requiredChecks.some(k=>report.checks?.[k]?.status!=='passed')||report.registrationMode!=='two-fresh-ordinary-registrations'||
    realm.ok!==true||realm.shutdown?.code!==0||realm.remainingOwnedPortListeners?.length!==0||realm.configuration?.slowStageThresholdMs!==50||realm.release?.sha256!==release.binary.sha256||realm.release?.sourceCommit!==release.sourceCommit||realm.release?.backendInputCount!==44||
    source.matchedRawInputs!==44||source.matchedRawDriverPins!==6||source.sourceCommit!==release.sourceCommit||source.sourceManifestSha256!==manifest.sourceManifestSha256||source.sourceManifestFileSha256!==release.sourceManifestFileSha256||source.releaseSha256!==release.binary.sha256)throw Error('Full16 original report/realm/source proof contradicts its independent audit');
  return{schemaVersion:1,verifiedAt:new Date().toISOString(),passed:true,runId:audit.runId,realmStamp:audit.realmStamp,backendSourceCommit:release.sourceCommit,backendInputCount:44,backendSourceAggregate:manifest.sourceManifestSha256,binarySha256:release.binary.sha256,slowStageThresholdMs:50,files:full.files,capacityAccepted:false};
}
