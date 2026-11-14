import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
const DIRECTORY=path.dirname(fileURLToPath(import.meta.url)),BASE=path.dirname(DIRECTORY);
const SOURCE=path.join(BASE,'ordinary-economic-preparation-pilot-v3','episode','private');
const fail=(ok,message)=>{if(!ok)throw Error(message)},same=(a,b)=>path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
function contained(root,file){const r=path.relative(root,path.resolve(file));fail(r&&!path.isAbsolute(r)&&!r.split(/[\\/]/).includes('..'),'Bank path escapes its fixed root');}
function safeExisting(file,kind='file'){
 contained(BASE,file);let cursor=path.resolve(file),base=path.resolve(BASE);
 while(!same(cursor,base)){const s=fs.lstatSync(cursor);fail(!s.isSymbolicLink()&&same(fs.realpathSync(cursor),cursor),'Bank path links or redirects are forbidden');cursor=path.dirname(cursor);}
 const s=fs.lstatSync(file);fail(kind==='directory'?s.isDirectory():s.isFile(),'Bank file type differs');return s;
}
function checked(p){fail(p&&path.isAbsolute(p.file??'')&&Number.isSafeInteger(p.bytes)&&p.bytes>=0&&/^[a-f0-9]{64}$/.test(p.sha256??''),'Exact bank pin required');safeExisting(p.file);const b=fs.readFileSync(p.file);fail(b.length===p.bytes&&sha(b)===p.sha256,'Bank source pin changed: '+path.basename(p.file));return b;}
function pin(file){const b=fs.readFileSync(file);return{file:path.resolve(file),bytes:b.length,sha256:sha(b)}}
function recoveryTree(root){safeExisting(root,'directory');const files=[],directories=[];function walk(d){for(const e of fs.readdirSync(d,{withFileTypes:true})){const f=path.join(d,e.name);safeExisting(f,e.isDirectory()?'directory':'file');if(e.isDirectory()){directories.push(path.relative(root,f).replaceAll('\\','/'));walk(f)}else{fail(e.isFile(),'Recovery entry must be a regular file');files.push(path.relative(root,f).replaceAll('\\','/'));}}}walk(root);return{files:files.sort(),directories:directories.sort()};}
export function validateSourceBank(planPin){
 const plan=JSON.parse(checked(planPin));fail(plan.schemaVersion===1&&plan.packageId===path.basename(DIRECTORY)&&plan.state==='SOURCE_BANK_IDENTIFIED_RUNTIME_BINDING_PENDING','Unexpected bank plan');fail(same(plan.sourcePrivate,SOURCE),'Only the actual V3 source bank is permitted');safeExisting(SOURCE,'directory');
 const r=JSON.parse(checked(plan.sourcePilotReceipt)),c=JSON.parse(checked(plan.sourceCleanup));checked(plan.sourceRootIndependentCleanup);
 fail(r.cleanupComplete===true&&c.checks?.normalOwnedShutdown===true&&c.checks?.sixCountersZero===true&&c.checks?.gatewayProcessAbsent===true&&c.checks?.driverProcessAbsent===true&&c.checks?.monitorProcessAbsent===true&&c.checks?.portsFree===true,'Source bank was not normally closed');
 const expected=['accounts.json','capacity-pool.json','capacity-auth-budget.json','recovery-mac-key.txt','identity-session-secret.txt','identity-recovery-pepper.txt','passkey-auth-secret.txt',...plan.recovery.expectedControlFiles.map(n=>'save-recovery/'+n)].sort();
 fail(plan.files?.length===11&&JSON.stringify(plan.files.map(p=>p.relative).sort())===JSON.stringify(expected),'The exact11 source-bank files are required');
 fail(plan.fileCount===11&&plan.totalBytes===plan.files.reduce((n,p)=>n+p.bytes,0),'Bank source count/bytes differ');
 for(const p of plan.files){fail(same(p.file,path.join(SOURCE,p.relative)),'Source-bank relative mapping differs');checked(p);}
 const tree=recoveryTree(path.join(SOURCE,'save-recovery'));
 fail(JSON.stringify(tree.files)===JSON.stringify([...plan.recovery.expectedControlFiles].sort())&&JSON.stringify(tree.directories)===JSON.stringify(['quarantine']),'Recovery contains journal payloads, extra entries or missing controls; complete recovery is required before reuse');
 fail(plan.recovery.journalPayloadsMustEqualZero===true&&plan.recovery.preservedDestination==='source-save-recovery-preserved'&&plan.recovery.activeDestination==='save-recovery','Recovery separation must remain explicit');
 const pool=JSON.parse(checked(plan.files.find(p=>p.relative==='capacity-pool.json'))),store=JSON.parse(checked(plan.files.find(p=>p.relative==='accounts.json'))),ledger=JSON.parse(checked(plan.files.find(p=>p.relative==='capacity-auth-budget.json')));
 fail(pool.schemaVersion===1&&pool.realm==='isolated'&&pool.endpoint==='ws://127.0.0.1:17510/ws'&&pool.accounts?.length===2,'Actual two-account isolated pool required');
 fail(ledger.schemaVersion===1&&ledger.purpose==='capacity-auth-budget'&&ledger.endpoint===pool.endpoint&&Array.isArray(ledger.attempts),'Original auth ledger is required');
 for(let i=0;i<pool.accounts.length;i++){const a=pool.accounts[i],e=plan.sourceReadyCharacters[i],saved=store.accounts?.[a.accountId];fail(a.state==='ready'&&a.name===e.name&&a.characterIndex===e.characterIndex&&e.state==='ready','Bank ready identity changed');fail(saved?.characters?.some(x=>x.index===a.characterIndex&&x.name===a.name),'Ready pool does not match the ordinarily saved store');}
 for(const name of ['recovery-mac-key.txt','identity-session-secret.txt','identity-recovery-pepper.txt','passkey-auth-secret.txt'])fail(/^[a-f0-9]{64}$/.test(checked(plan.files.find(p=>p.relative===name)).toString('utf8')),'Original secret has unexpected encoding');
 return{plan,pool};
}
export function copySourceBankExclusively(planPin,destinationPrivate){
 const {plan,pool}=validateSourceBank(planPin);fail(same(destinationPrivate,path.join(DIRECTORY,'episode','private')),'Only this new owned private output is permitted');safeExisting(destinationPrivate,'directory');
 const mapped=plan.files.map(p=>({source:p,target:path.join(destinationPrivate,p.relative.startsWith('save-recovery/')?'source-save-recovery-preserved/'+p.relative.slice('save-recovery/'.length):p.relative)}));
 for(const {target}of mapped){contained(destinationPrivate,target);fail(!fs.existsSync(target),'Bank target already exists; no overwrite is permitted');}
 const preserved=path.join(destinationPrivate,'source-save-recovery-preserved'),active=path.join(destinationPrivate,'save-recovery');fail(!fs.existsSync(preserved)&&!fs.existsSync(active),'New recovery directories must be absent');
 // All source pins and the absence of journal payloads were verified before any copy.
 fs.mkdirSync(preserved);fs.mkdirSync(path.join(preserved,'quarantine'));fs.mkdirSync(active);
 const copies=[];for(const {source,target}of mapped){fs.copyFileSync(source.file,target,fs.constants.COPYFILE_EXCL);safeExisting(target);const actual=pin(target);fail(actual.bytes===source.bytes&&actual.sha256===source.sha256,'Copied bank bytes differ');copies.push({source,target:actual});}
 validateSourceBank(planPin);
 fail(fs.readdirSync(active).length===0,'Activity recovery must initially be empty for normal Gateway initialization');
 return{schemaVersion:1,scope:'Ordinary-earned V3 bank reused by exclusive raw copying; no seed/grant/store edits',sourcePlan:planPin,sourceFiles:11,totalBytes:plan.totalBytes,copies,sourceUnchangedAfterCopy:true,recoveryJournalPayloads:0,sourceRecoveryPreserved:true,activityRecoveryInitializedByGateway:false,activityRecoveryDirectory:active,authLedgerCopiedUnchanged:true,sourcePoolManualRewritePerformed:false,sourceAccountStoreRewritePerformed:false,accountWriterLockCopied:false,readyCharacters:pool.accounts.map(a=>({name:a.name,characterIndex:a.characterIndex})),ordinaryReloginReadbackPending:true,freshZeroCharacterPool:false,capacityAccepted:false,p0Accepted:false};
}
if(process.argv[1]&&same(process.argv[1],fileURLToPath(import.meta.url)))throw Error('This draft has no standalone copy mode; only a separately frozen owned runner may invoke it');
