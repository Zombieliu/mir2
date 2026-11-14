import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {isDeepStrictEqual} from 'node:util';

// This verifier performs only source/evidence reads and one exclusive proof write.
// It does not invoke any tests, build, service, account, bank or lifecycle runner.
const BASE=path.dirname(fileURLToPath(import.meta.url));
const OLD=path.join(BASE,'ordinary-v6-full-lifecycle-v1');
const ORIGINAL_HEAD='d9ec9d2094ee058bfa909b05bbcf45dcf4a28b39';
const BACKEND_HEAD='ee06c05f3813a1e627c8756d1f5fe8e2e0fbf856';
const BINARY_SHA='ad13d15a6fd294cae0beda04d8f91b7773a9b8390cc9c85e5a84d3e72c2567f6';
const assert=(value,message)=>{if(!value)throw new Error(message);};
const same=(a,b)=>path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
function pin(file){const b=fs.readFileSync(file);return{file:path.resolve(file),bytes:b.length,sha256:sha(b)};}
function checked(p){
  assert(p&&path.isAbsolute(p.file??'')&&Number.isSafeInteger(p.bytes)&&p.bytes>=0&&/^[a-f0-9]{64}$/.test(p.sha256),'Invalid evidence pin');
  const st=fs.lstatSync(p.file),b=fs.readFileSync(p.file);
  assert(st.isFile()&&!st.isSymbolicLink()&&same(fs.realpathSync(p.file),p.file)&&b.length===p.bytes&&sha(b)===p.sha256,'Evidence pin differs: '+p.file);
  return b;
}
const read=p=>JSON.parse(checked(p));
const equal=(a,b,label)=>assert(isDeepStrictEqual(a,b),label);
const pinEqual=(a,b)=>a&&b&&same(a.file,b.file)&&a.bytes===b.bytes&&a.sha256===b.sha256;
const fixed=(name,bytes,sha256)=>({file:path.join(OLD,name),bytes,sha256});
const PINS=Object.freeze({
  frozen:fixed('frozen-inputs-v1.json',66852,'cfb6fd6ed09e72393afe61d089024dd067d7b4ced5a782fa41de2cd329802c90'),
  driver:fixed('final-current-driver-v1.json',18310,'177431db8c72a2bbe19e92a1d1a1158367f5c89a9147e669b979d14903ea1e91'),
  library:fixed('source-lib-v1.mjs',13053,'9758402317d730431fd698a1f121fb3668c455489a4708d29f69a0dc26c8c80a'),
  backendLibrary:fixed('backend-source-lib-v1.mjs',9374,'f75b6eeddbe1246f4941ff1f293786e68b5d17617a15f0938e0c194597ce04ed'),
  realm:fixed('public/1793057632464-9c427e44.realm-evidence.json',36403,'f4a3adec5a5cbda8a3da45c2b7e35a538368adaffea9e59776778d1422f69329'),
  audit:fixed('public/1793057632464-9c427e44.independent-audit.json',1866,'df0043fce0c26b7e08ab3053e6732ca4b71524512ea2bdb65ece72a4643a7c4c'),
  report:fixed('public/1793057646570-c2fb7dc7.report.json',109554,'1879df81f1662f75c9a78e6eef5b7bc79743605e66ceb96c6779ce92289755fd'),
  rootOs:fixed('root-independent-cleanup-v1.json',258,'25357f93e278f41f30d3541176ba304ae460ca7607d80ed71bd94eaf8ef42e02')
});
function flags(){
  const allowed=['--repository','--expected-head','--root-receipt','--root-receipt-sha','--backend-directory','--output'],r={};
  for(let i=2;i<process.argv.length;i+=2){const k=process.argv[i],v=process.argv[i+1];assert(allowed.includes(k)&&typeof v==='string'&&!v.startsWith('--')&&!(k in r),'Unique explicit flag/value required');r[k]=v;}
  for(const k of allowed)assert(r[k],'Missing '+k);
  assert(/^[a-f0-9]{40}$/.test(r['--expected-head']),'Actual new40hex HEAD required');
  assert(/^[a-f0-9]{64}$/.test(r['--root-receipt-sha']),'Actual new Root receipt SHA required');
  for(const k of ['--repository','--root-receipt','--backend-directory','--output'])assert(path.isAbsolute(r[k]),'Absolute path required: '+k);
  return r;
}
function driverContent(d){const {driverSourceCommit,state,...content}=d;return content;}
function backendContent(b){
  // Only declarations which necessarily bind the new JS execution are omitted.
  // Backend/build/source/Rust commits, scope, source rows, all anchors and pins remain exact.
  const {executionSourceCommit,currentDriverCommit,backendReleaseBinding,currentJsReceipt,...content}=b;
  return content;
}
function sixZero(health){const c=health?.capacity;return health?.ok===true&&c&&[
  'currentWsConnections','currentActiveSessions','currentReconnectLeases',
  'currentLoginInFlight','currentNewCharacterInFlight','currentStartGameInFlight'
].every(k=>c[k]===0);}
function git(repo,args){
  return execFileSync('git',['-c','gc.auto=0','-c','maintenance.auto=false',...args],
    {cwd:repo,windowsHide:true,stdio:['pipe','pipe','pipe'],maxBuffer:64*1024*1024,
      env:{...process.env,GIT_OPTIONAL_LOCKS:'0',GIT_NO_LAZY_FETCH:'1'}});
}
async function main(){
  const args=flags(),repo=path.resolve(args['--repository']),head=args['--expected-head'],
    backendDirectory=path.resolve(args['--backend-directory']),output=path.resolve(args['--output']);
  assert(same(path.dirname(backendDirectory),BASE)&&path.basename(backendDirectory)==='ordinary-capacity-bank-prepare-v12','Only ordinary-capacity-bank-prepare-v12 sibling backend package permitted');
  const relative=path.relative(BASE,output);
  assert(relative&&!relative.startsWith('..')&&!path.isAbsolute(relative)&&!same(output,fileURLToPath(import.meta.url)),'New output must stay under external BASE');
  assert(!same(path.dirname(output),OLD)&&!output.toLowerCase().startsWith((OLD+path.sep).toLowerCase()),'Never write into old full16 package');
  assert(!fs.existsSync(output),'Output already exists; no replacement permitted');
  assert(!same(repo,BASE)&&!same(repo,backendDirectory),'Repository root required');
  process.env.GIT_OPTIONAL_LOCKS='0';process.env.GIT_NO_LAZY_FETCH='1';
  for(const p of Object.values(PINS))checked(p);
  const frozen=read(PINS.frozen),oldDriver=read(PINS.driver),realm=read(PINS.realm),
    audit=read(PINS.audit),report=read(PINS.report),os=read(PINS.rootOs);
  assert(frozen.packageId==='ordinary-v6-full-lifecycle-v1'&&frozen.executionSourceCommit===ORIGINAL_HEAD&&frozen.backendSourceCommit===BACKEND_HEAD,'Original frozen actual identities differ');
  assert(frozen.driver.driverSourceCommit===ORIGINAL_HEAD&&oldDriver.driverSourceCommit===ORIGINAL_HEAD&&oldDriver.files.length===14,'Original complete14 closure required');
  equal(frozen.driver,oldDriver,'Frozen/final complete driver snapshots differ');
  assert(same(repo,frozen.repository),'Current repository differs from old actual source root');
  assert(pinEqual(realm.full16InputFrozen,PINS.frozen),'Old actual realm did not execute this freeze');
  const final=read(frozen.finalBinding);
  assert(final.executionSourceCommit===ORIGINAL_HEAD&&pinEqual(final.driverSnapshot,PINS.driver),'Final binding does not bind original actual14');
  equal(final.backend,frozen.backend,'Old final/frozen backend differs');
  equal(final.dataPins,frozen.dataPins,'Old final/frozen data pins differ');
  for(const p of [...frozen.toolPins,...frozen.backend.originalV6ReleaseToolPins,...frozen.dataPins])checked(p);
  assert(frozen.toolPins.some(p=>pinEqual(p,PINS.library))&&frozen.toolPins.some(p=>pinEqual(p,PINS.backendLibrary)),'Old library pins are not frozen tools');
  for(const p of frozen.dataPins)assert(same(p.file,path.join(repo,p.path)),'Data pin is not current repository data');

  const required=frozen.profile.requiredChecks;
  assert(Array.isArray(required)&&required.length===16&&new Set(required).size===16,'Original sixteen named checks required');
  assert(report.profile==='full-lifecycle'&&report.ok===true&&report.fullLifecyclePassed===true&&report.status==='passed','Original full16 report failed');
  equal(report.requiredChecks,required,'Original required check set differs');
  equal(Object.keys(report.checks).sort(),[...required].sort(),'Report check keys differ');
  assert(required.every(k=>report.checks[k]?.status==='passed'),'An original lifecycle check did not pass');
  assert(realm.ok===true&&realm.playtest?.ok===true&&realm.playtest.status==='passed'&&realm.playtest.runId===report.runId&&same(realm.playtest.reportPath,PINS.report.file),'Realm/report link or status differs');
  equal(realm.playtest.requiredChecks,required,'Realm required checks differ');
  assert(required.every(k=>realm.playtest.statuses[k]==='passed')&&Object.values(realm.checks).every(v=>v==='passed'),'Original realm checks failed');
  assert(realm.release.sourceCommit===BACKEND_HEAD&&realm.release.executionSourceCommit===ORIGINAL_HEAD&&realm.release.driverSourceCommit===ORIGINAL_HEAD&&realm.release.backendInputCount===55&&realm.release.sha256===BINARY_SHA,'Original actual runtime identities differ');
  assert(realm.shutdown.code===0&&realm.shutdown.signal===null&&realm.ownedProcessAbsent===true&&realm.ownedProcessAfterStop.absent===true&&realm.remainingOwnedPortListeners.length===0,'Original owned cleanup failed');
  assert(sixZero(realm.baselineHealth)&&sixZero(realm.afterHealth),'Original six counters not zero');
  assert(os.independentRootCheck===true&&os.gatewayPid===realm.pid&&os.gatewayPidAbsent===true&&os.portsFree===true&&os.listeners.length===0,'Original independent Root OS cleanup failed');
  assert(audit.all16Passed===true&&audit.pairedDeathReceiptsMatchRawTrace===true&&audit.pairedReviveAndKillDropPickupReceiptsMatchRawTrace===true&&audit.onlyOrdinarySentCommands===true&&audit.sixCountersZeroBeforeAndAfter===true&&audit.normalExitCode===0,'Original independent raw audit failed');
  assert(audit.runId===report.runId&&audit.realmStamp===realm.stamp&&audit.executionSourceCommit===ORIGINAL_HEAD&&audit.driverSourceCommit===ORIGINAL_HEAD&&audit.backendSourceCommit===BACKEND_HEAD&&audit.backendInputCount===55&&audit.frozenDriverRawPinsMatched===14&&audit.originalEntryRootCount===6&&audit.binarySha256===BINARY_SHA,'Independent audit identity differs');
  assert(Object.keys(realm.sourcePins).length===14,'Actual runtime source pin count differs');
  for(const row of oldDriver.files){const actual=realm.sourcePins[row.path.replace(/^mir2-web3\//,'')];assert(typeof actual==='string'&&actual===row.sha256,'Actual runtime14 source differs: '+row.path);}

  // Import only the already pinned libraries; no runner/freeze/audit entrypoint is imported.
  const old=await import(pathToFileURL(PINS.library.file).href);
  git(repo,['merge-base','--is-ancestor',ORIGINAL_HEAD,head]);
  const driver=old.captureDriver(repo,head);
  assert(driver.files.length===14&&driver.entryRoots.length===6&&driver.dynamicUnexpanded.length===0,'Current complete14/six-root closure differs');
  equal(driverContent(driver),driverContent(oldDriver),'Complete local closure raw/Git identity/edges/external/parser/Node/package metadata differs');
  const receiptPin=pin(args['--root-receipt']);
  assert(receiptPin.sha256===args['--root-receipt-sha'],'New Root receipt SHA differs');
  const root=old.validateRootReceipt(receiptPin,repo,head,driver);
  assert(root.sourceRows.length===42&&root.results.length>0&&root.sourceUnchanged===true&&root.freshForDriverCommit===head,'New complete42 current Root qualification required');
  const backendLibraryPin=pin(path.join(backendDirectory,'source-lib-v1.mjs')),backendLibrary=await import(pathToFileURL(backendLibraryPin.file).href);
  assert(typeof backendLibrary.backendBinding==='function','W12 backend package strict backendBinding API missing');
  const backend=backendLibrary.backendBinding(repo,head);
  assert(backend.executionSourceCommit===head&&backend.currentDriverCommit===head&&pinEqual(backend.currentJsReceipt,receiptPin),'W12 backend package must bind new HEAD and exact actual new Root receipt');
  assert(same(path.dirname(backend.backendReleaseBinding.file),backendDirectory),'W12 backend package backend binding belongs to another package');
  const backendBindingDocument=read(backend.backendReleaseBinding);
  assert(backendBindingDocument.finalized===true&&backendBindingDocument.state==='FINAL_REUSED_V6_RELEASE_AND_CURRENT_JS_BOUND'&&backendBindingDocument.executionSourceCommit===head&&backendBindingDocument.currentJsSourceCommit===head&&pinEqual(backendBindingDocument.newRootJsReceipt,receiptPin),'W12 backend package binding remains pending or mismatched');
  equal(backendContent(backend),backendContent(frozen.backend),'W12 backend package backend55/raw/Git/anchors/Rust32/build/immutable Release differs');
  assert(backend.sourceCount===55&&backend.sourceRows.length===55&&backend.rawAndGitObjectsExact===true&&backend.reusedRustTestsPassed===32&&backend.immutableExecutable.bytes===87456256&&backend.immutableExecutable.sha256===BINARY_SHA,'Strict original V6 qualification differs');
  assert(backend.backendSourceCommit===BACKEND_HEAD&&backend.backendBuildCommit===BACKEND_HEAD&&backend.newRustBuildPerformed===false&&backend.freshRustTestsPerformed===false&&backend.rebuildPerformed===false,'Historical V6 build/Rust was incorrectly relabelled');
  const anchors=backend.sourceRows.filter(p=>!p.path.endsWith('.rs'));
  assert(anchors.length===5,'All five backend Cargo anchors required');
  for(const p of [backend.releasePin,backend.manifestPin,backend.immutableExecutable,backend.reusedBuildReceipt,backend.reusedRustReceipt])checked(p);

  // Close the source/evidence read interval without running any validation commands.
  equal(old.captureDriver(repo,head),driver,'Current driver changed during proof');
  equal(backendLibrary.backendBinding(repo,head),backend,'Current backend changed during proof');
  equal(old.validateRootReceipt(receiptPin,repo,head,driver),root,'New Root qualification changed during proof');
  for(const p of [...Object.values(PINS),...frozen.dataPins,backendLibraryPin])checked(p);
  const proof={
    schemaVersion:1,verifiedAt:new Date().toISOString(),kind:'full16-reuse-after-complete-dependency-equality',
    all16ReusedAfterCompleteDependencyEquality:true,fullLifecycleRerunPerformed:false,
    originalActualExecution:ORIGINAL_HEAD,newHEAD:head,repository:repo,originalEvidence:PINS,
    originalFull16Checks:required.map(name=>({name,status:'passed'})),
    currentCompleteLocalDriver:driver,driverEqualityIgnoredDeclarationFields:['state','driverSourceCommit'],
    currentRootQualification:root,backendLibrary:backendLibraryPin,currentBackend:backend,backendAnchors:anchors,
    backendEqualityIgnoredCurrentBindingFields:['executionSourceCommit','currentDriverCommit','backendReleaseBinding','currentJsReceipt'],
    dataPins:frozen.dataPins,originalBackendBuildCommit:BACKEND_HEAD,reusedOriginalRustTestsPassed:32,
    freshFull16Performed:false,newBackendBuildPerformed:false,freshRustTestsPerformed:false,
    capacityAccepted:false,p0Accepted:false,formalAcceptanceEligible:false,humanFrontendAccepted:false,
    scope:'Historical actual d9ec full16 plus independent raw audit and owned/Root OS cleanup, reused only after complete14 local source/edges/raw/Git/external entry/parser/Node/package metadata and strict original V6 backend55/build/Rust32/immutable equality. Current Root42 is fresh selected JS qualification, not a fresh full lifecycle, full npm/OS dependency closure, capacity or frontend acceptance.'
  };
  fs.writeFileSync(output,JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify({output:pin(output),all16ReusedAfterCompleteDependencyEquality:true,fullLifecycleRerunPerformed:false,originalActualExecution:ORIGINAL_HEAD,newHEAD:head}));
}
main().catch(error=>{console.error(error?.message??String(error));process.exitCode=1;});
