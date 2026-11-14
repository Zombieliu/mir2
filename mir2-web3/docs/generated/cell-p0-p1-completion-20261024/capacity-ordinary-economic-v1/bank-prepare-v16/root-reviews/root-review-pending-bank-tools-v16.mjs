import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const D='C:/mir2-build/p01-completion-20261024/ordinary-capacity-bank-prepare-v16';
const read=n=>JSON.parse(fs.readFileSync(path.join(D,n),'utf8'));
const assert=(v,m)=>{if(!v)throw Error(m)};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const pin=f=>{const b=fs.readFileSync(f);return{file:f,bytes:b.length,sha256:sha(b)}};
const checked=p=>{const b=fs.readFileSync(p.file);assert(b.length===p.bytes&&sha(b)===p.sha256,'pin differs '+p.file);return b};
const p=read('preparation-v2.json'),d=read('derivation-v1.json'),c=read('prepare-config-pending-v1.json'),plan=read('source-bank-plan-pending-v1.json'),cor=read('ledger-reference-correction-v2.json');
let metadataPins=0;for(const v of Object.values(p)){if(v?.file&&v?.sha256){checked(v);metadataPins++}}
assert(metadataPins===9,'top-level metadata pin count');
const rules=[];
for(let i=0;i<5;i++){
 const old=checked(d.parents[i]),current=checked(d.current[i]),name=path.basename(d.current[i].file),rs=d.rules[name];
 assert(p.tools[i].sha256===d.current[i].sha256,'tool binding');
 let forward=old.toString('utf8');
 for(const r of rs){assert(r.before&&r.after&&r.before!==r.after,'invalid rule');assert(forward.split(r.before).length===2,'forward anchor not unique '+r.label);forward=forward.replace(r.before,r.after)}
 assert(Buffer.from(forward).equals(current),'forward raw '+name);
 let inverse=current.toString('utf8');
 for(const r of [...rs].reverse()){assert(inverse.split(r.after).length===2,'inverse anchor not unique '+r.label);inverse=inverse.replace(r.after,r.before)}
 assert(Buffer.from(inverse).equals(old),'inverse raw '+name);
 rules.push({name,count:rs.length,forwardAndInverseRawExact:true,parent:d.parents[i],current:d.current[i]});
}
assert(JSON.stringify(rules.map(x=>x.count))==='[0,8,6,10,1]','rule counts');
assert(checked(d.parents[0]).equals(checked(d.current[0])),'source lib changed');
const before=checked(cor.before),after=checked(cor.after),old=cor.oldValue,next=cor.newValue;
assert(before.toString('utf8').split(old).length===2&&after.toString('utf8').split(next).length===2,'correction anchor');
assert(Buffer.from(before.toString('utf8').replace(old,next)).equals(after)&&Buffer.from(after.toString('utf8').replace(next,old)).equals(before),'correction raw');
assert(d.config.sha256===cor.before.sha256&&d.config.bytes===before.length&&p.config.sha256===cor.after.sha256,'preserved derivation and corrected config');
checked(d.sourcePlan);checked(d.binding);
assert(plan.files.length===11&&plan.fileCount===11&&plan.files.reduce((s,x)=>s+checked(x).length,0)===596910&&plan.totalBytes===596910,'whole11');
assert(plan.priorRecoveryArchive.files.length===4,'prior4');for(const q of plan.priorRecoveryArchive.files)checked(q);
const sourcePublic=['sourceReceipt','sourceReport','sourceCleanup','sourcePreparedIndex','sourceOwnedProcess','sourceFrozenManifest','sourceIndependentCleanup'];
for(const k of sourcePublic)checked(plan[k]);
assert(c.actors===100&&c.maxPrepared===100&&c.options.maxPrepared===100&&c.workerCount===1&&c.options.finiteWallSeconds===18000&&c.options.targetHp===12&&c.options.attackResults==='v2','unchanged preparation limits');
assert(c.historicalPreparedCount===42&&c.expectedThisRun===58&&c.continuationMode==='historical-prepared-index-v1'&&plan.sourcePreparedCharacters.length===42&&plan.sourceReadyCharacters===43&&plan.sourceRegisteredAccounts===57,'42/58 contract');
assert(c.options.continuation.evidence.length===1,'single actual history');
const g=c.options.continuation.evidence[0];for(const q of Object.values(g))checked(q);
for(const [a,b]of [['report','sourceReport'],['preparedIndex','sourcePreparedIndex'],['cleanup','sourceCleanup'],['receipt','sourceReceipt']])assert(JSON.stringify(g[a])===JSON.stringify(plan[b]),'history cross pin '+a);
assert(c.authOrigin.ledgerSource==='Latest actual W15 traffic ledger exact bytes'&&c.authOrigin.loginNotBeforeAllowed===false&&!Object.hasOwn(c.options,'loginNotBefore'),'ledger origin unchanged');
assert(c.sourceQualification.driverSourceCommit===null&&c.sourceQualification.executionSourceCommit===null&&c.sourceQualification.newRootJsReceipt===null&&c.sourceQualification.freshNodeTestsPassed===null&&plan.newCurrentSourceQualification===null,'pending qualification');
assert(plan.sourcePreparedEligibilityInherited===false&&plan.sourceOriginalVerdict.ok===false&&c.all100FreshPreparedThisRun===false&&c.capacityAccepted===false&&c.p0Accepted===false,'no inherited acceptance');
const out={schemaVersion:1,reviewedAt:new Date().toISOString(),scope:'Root independent pending W16 raw pins, exact forward/inverse rules, immutable source-lib/backend contract and explicit42 historical notFresh plus58 original-index continuation. No runtime, bank copy, auth or Store mutation.',preparation:pin(path.join(D,'preparation-v2.json')),metadataPins,rules,source11Files:11,source11Bytes:596910,priorControlFiles:4,sourcePublicPins:7,continuationPins:4,correction:pin(path.join(D,'ledger-reference-correction-v2.json')),historicalPreparedCount:42,expectedThisRun:58,sourcePreparedEligibilityInherited:false,rootReviewPassed:true,runtimeExecuted:false,capacityAccepted:false,p0Accepted:false};
const file=path.join(D,'root-pending-tools-review-v1.json');fs.writeFileSync(file,JSON.stringify(out,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(pin(file)));
