import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// V6 imports no previous runtime module. The pinned actual V5 manifest supplies
// original55 backend rows and historical JS origins; current JS is qualified separately.
export const DIRECTORY = path.dirname(fileURLToPath(import.meta.url)), BASE = path.dirname(DIRECTORY);
export const BACKEND_BASELINE_COMMIT = '7b43caa41ee12f2e6b41d0a5df7e9ab45a32580d';
export const OBSERVATION_COMMIT = 'c6f46b633e087dac86b06c8872e5d2cba0665cd8';
export const DRIVER_COMMIT = '657b7528f03f3c8f824fcadcabcdcc64e73b4745';
export const NAV_COMMIT = '4ecd237e25858a2080456a2e52d64b9ba0c167cd';
export const TARGET = 'C:/mir2-build/cell-preproduction-p0-p1-20261024/target';
export const BUILD_ARGS = ['run','1.95.0','cargo','build','--locked','--offline','--release','--jobs','4','-p','mir2-gateway','--bin','mir2-gateway'];
export const QUALIFICATION = { allSelectedTestsPassed: true, diagnosticOnly: true, diagnosticCandidateOnly: true, P0Eligible: false, p0Eligible: false, formalAcceptanceEligible: false, capacityAccepted: false, p0Accepted: false, freshOrdinaryLifecyclePerformed: false, realCapacityRunPerformed: false };
export const BASELINE_PIN = { file: path.join(BASE,'online-presence-reuse-release-v5/backend-source-v5.json'), bytes: 66046, sha256: 'cf2c8e39096e5133dc8cf95045e957b8f81a4e04f626444b5d25c1a015d3f0f7' };
export const BASELINE_RELEASE_PIN = { file: path.join(BASE,'online-presence-reuse-release-v5/release-v5.json'), bytes: 2275, sha256: '8272b00152df3bd96902fa7fbaa6a0bb365ca50abd0b9b25ef5fa0d570a1d65c' };
const REUSED = {
 node: { file: path.join(BASE,'root-owner-action-pose-validation-v1/full-driver-receipt-v1.json'), bytes: 6449, sha256: '714a32dad7c2db920db7ecb938b0d0f438d4ffea32ec09b6157c5f5866381eb8' },
 focused: { file: path.join(BASE,'root-owner-action-pose-validation-v2/focused-observation-web-receipt-v1.json'), bytes: 7372, sha256: '552c91e2bd67232bf3ba17e06d18698d6ae98bb57197ff6da4cf08c9e2455169' },
 typecheck: { file: path.join(BASE,'root-owner-action-pose-validation-v2/typecheck-receipt-v1.json'), bytes: 7108, sha256: '666662841a185b690908750cd73f216ab91469d5e4f9209deaafc87f80329cb1' },
};
const NODE_ENTRIES = ['apps/web/scripts/playtest-capacity-soak.test.mjs','apps/web/scripts/playtest-capacity-observability.test.mjs','apps/web/scripts/playtest-capacity-monitor.test.mjs','apps/web/scripts/playtest-capacity-telemetry.test.mjs'];
const FOCUSED_ENTRIES = ['apps/web/scripts/quest-agent/test-protocol-observation.mjs','apps/web/scripts/test-owner-action-pose.mjs'];
export function assert(v,message) { if (!v) throw new Error(message); }
export function equal(a,b,message) { assert(JSON.stringify(a) === JSON.stringify(b),message); }
export function sha(b) { return crypto.createHash('sha256').update(b).digest('hex'); }
export function pin(file) { const b=fs.readFileSync(file); return {file:path.resolve(file),bytes:b.length,sha256:sha(b)}; }
export function verifyPin(p) { assert(p && path.isAbsolute(p.file ?? '') && Number.isInteger(p.bytes) && /^[a-f0-9]{64}$/.test(p.sha256),'Invalid file pin'); const b=fs.readFileSync(p.file); assert(b.length===p.bytes && sha(b)===p.sha256,'File pin mismatch: '+p.file); return b; }
export function readJsonPin(p) { return JSON.parse(verifyPin(p)); }
export function writeJson(name,data) { assert(path.basename(name)===name,'Output must be local basename'); const f=path.join(DIRECTORY,name); fs.writeFileSync(f,JSON.stringify(data,null,2)+'\n',{flag:'wx'}); return pin(f); }
function git(repo,args,input) { return execFileSync('git',args,{cwd:repo,input,maxBuffer:32*1024*1024,stdio:['pipe','pipe','pipe']}); }
function gitText(repo,args,input) { return git(repo,args,input).toString('utf8').trim(); }
function paths(b) { return b.toString('utf8').split('\0').filter(Boolean).sort(); }
function backend(p) { return p.startsWith('mir2-web3/') && (/\.rs$/.test(p) || /(^|\/)Cargo\.(toml|lock)$/.test(p) || p.includes('/.cargo/')); }
function cleanPath(p) { return typeof p==='string' && p.startsWith('mir2-web3/') && !p.includes('\\') && !p.split('/').some(s=>s==='..'||s==='.'||s===''); }
function rawRows(rows) { assert(Array.isArray(rows),'Missing source rows'); return rows.map(r=>({path:(r.path??r.file).replaceAll('\\','/').replace(/^(?!mir2-web3\/)/,'mir2-web3/'),bytes:r.bytes,sha256:r.sha256})).sort((a,b)=>a.path<b.path?-1:a.path>b.path?1:0); }
function exactRows(actual,expected,label) { const a=rawRows(actual),e=rawRows(expected); assert(new Set(a.map(r=>r.path)).size===a.length,label+' duplicate source paths'); equal(a,e,label+' exact raw source mismatch'); }
function binding(repo,commit,p) { const raw=fs.readFileSync(path.join(repo,p)),lf=git(repo,['show',`${commit}:${p}`]); const object=gitText(repo,['rev-parse',`${commit}:${p}`]),filtered=gitText(repo,['hash-object',`--path=${p}`,'--stdin'],raw); equal(object,filtered,'Raw source is not current committed object: '+p); return {path:p,bytes:raw.length,sha256:sha(raw),gitBlobLfBytes:lf.length,gitBlobLfSha256:sha(lf),gitAttributeFilteredObject:filtered,committedGitObject:object}; }
export function loadConfig(configPin,expectedHead) {
 const c=readJsonPin(configPin);
 assert(c.schemaVersion===1 && c.finalized===true && c.state==='ROOT_FINAL_V6_SOURCE_CONFIG','V6 requires Root-final config; PENDING template cannot capture/build/seal');
 assert(/^[a-f0-9]{40}$/.test(expectedHead??'') && expectedHead!==BACKEND_BASELINE_COMMIT,'Expected new actual 40-hex HEAD');
 equal(c.expectedHead,expectedHead,'Root config/CLI HEAD mismatch'); equal(c.baselineCommit,BACKEND_BASELINE_COMMIT,'Root config baseline mismatch');
 assert(Array.isArray(c.allowedBackendDelta) && c.allowedBackendDelta.length>0,'Root final backend whitelist missing');
 equal(c.allowedBackendDelta,[...new Set(c.allowedBackendDelta)].sort(),'Backend whitelist must be unique and default-sorted');
 equal(c.allowedBackendDelta,['mir2-web3/apps/gateway/src/web.rs'],'Only the reviewed UseItem bootstrap wake Rust module may differ from V5');
 equal(c.backendSourceInputCount,55,'Original55 closure must remain complete; web.rs is already included');
 assert(Array.isArray(c.backendSourceFiles)&&c.backendSourceFiles.length===c.backendSourceInputCount,'Root final complete source rows missing');
 assert(c.backendSourceFiles.every(r=>cleanPath(r.path)&&backend(r.path)&&Number.isInteger(r.bytes)&&r.bytes>0&&/^[a-f0-9]{64}$/.test(r.sha256)),'Invalid Root final raw source row');
 assert(new Set(c.backendSourceFiles.map(r=>r.path)).size===c.backendSourceInputCount,'Duplicate Root source paths');
 equal(c.rustCheckNames,['format','gateway-owner-health-routing','gateway-runtime-tick','gateway-use-item-routing','simulation-dead','simulation-potion'],'Exact Root V6 six-check Rust qualification required');
 equal(c.rustCheckNames,[...new Set(c.rustCheckNames)].sort(),'Rust check names must be unique and default-sorted');
 assert(Number.isInteger(c.expectedRustTestsPassed)&&c.expectedRustTestsPassed>0,'Root actual selected Rust total required');
 assert(c.freshRustReceipt && c.freshRustReceipt.sha256!=='a83a9eb82c4bd442912d97b6ccd1f1be4fd393fc8832c7a97e8e7222189696f3' && !Object.values(REUSED).some(p=>p.sha256===c.freshRustReceipt.sha256),'New V6 Rust receipt required; V5 Rust116 cannot qualify');
 assert(!['930da95bf003a0e7ff76101cf56231a3beaf7fe4d0453697ccb706c237138e45','5ab9ec343004c176868c98d906d7d96fa91149680ace4e1f9c3636aa7c6a57b5'].includes(c.freshRustReceipt.sha256),'Historical Rust57 or V4 Rust111 receipt cannot qualify V6');
 equal(path.resolve(c.freshRustReceipt.file),path.join(BASE,'root-ordinary-hp-wake-validation-v1/root-focused-receipt-v1.json'),'New Root V6 Rust receipt path differs');
 verifyPin(c.freshRustReceipt);
 return c;
}
function reusedReceipt(kind,current,baseline) {
 const receipt=REUSED[kind],r=readJsonPin(receipt);
 const rows=[...current.driverFiles,...current.navigationFiles,...current.observationFiles,...(kind==='node'?[]:current.sourceFrontendFiles)];
 assert(rows.length===(kind==='node'?13:16),'Reused receipt layer input count differs');
 assert(r.schemaVersion===1&&r.exitCode===0&&r.signal===null&&r.error===null&&r.sourceUnchanged===true,'Reused receipt was unsuccessful: '+kind);
 equal(r.sourceInputCount,rows.length,'Reused sourceInputCount: '+kind); exactRows(r.sourceBefore??r.before,rows,kind+' before'); exactRows(r.sourceAfter??r.after,rows,kind+' after');
 equal(path.resolve(r.cwd),path.join(baseline.repository,kind==='typecheck'?'mir2-web3/apps/web':'mir2-web3'),'Reused historical cwd mismatch');
 assert(Date.parse(r.finishedAt)>=Date.parse(r.startedAt),'Reused execution times invalid');
 const stdout=verifyPin(r.stdout).toString('utf8'); verifyPin(r.stderr);
 let counts=null;
 if(kind==='typecheck') { assert(Array.isArray(r.command)&&/^cmd(?:\.exe)?$/i.test(path.basename(r.command[0])),'Reused typecheck command missing'); equal(r.command.slice(1),['/d','/s','/c','npm.cmd run typecheck'],'Reused typecheck command differs'); }
 else {
  assert(Array.isArray(r.command)&&/^(?:node|node\.exe)$/i.test(path.basename(r.command[0])),'Reused Node executable missing'); equal(r.command.slice(1),['--test',...(kind==='node'?NODE_ENTRIES:FOCUSED_ENTRIES)],'Reused test entry command mismatch');
  const summary=stdout.split(/\r?\n/).filter(l=>/^# (tests|pass|fail|cancelled|skipped|todo|duration_ms) /.test(l)); equal(r.summary,summary,'Reused raw summary mismatch'); counts={};
  for(const line of summary) { const m=/^# (tests|pass|fail|cancelled|skipped|todo) (\d+)$/.exec(line); if(m) { assert(!(m[1] in counts),'Duplicate raw Node total'); counts[m[1]]=+m[2]; } }
  equal(counts,{tests:kind==='node'?242:39,pass:kind==='node'?242:39,fail:0,cancelled:0,skipped:0,todo:0},'Reused Node raw totals mismatch');
 }
 return {receipt,stdout:r.stdout,stderr:r.stderr,command:r.command,sourceInputCount:rows.length,actualPassed:counts?.pass??null,actualCounts:counts,sourceUnchanged:true,fresh:false,reused:true,reexecutedForV6:false,reusedFromObservationFrontendCommit:OBSERVATION_COMMIT,currentSourceQualification:false,scope:'Historical V5 validation on its exact then-current JS/TS inputs only. These inputs are NOT asserted unchanged in the new preparation driver and do not qualify current JS. No fresh Node run, lifecycle or capacity claim.'};
}
export function capture({repositoryDirectory,expectedHead,configPin}) {
 const c=loadConfig(configPin,expectedHead),repo=path.resolve(repositoryDirectory),baseline=readJsonPin(BASELINE_PIN).current,previousRelease=readJsonPin(BASELINE_RELEASE_PIN);
 equal(previousRelease.backendSourceManifest,BASELINE_PIN,'Pinned V5 release/manifest binding differs');
 for(const k of ['sourceCommit','backendSourceCommit','executionSourceCommit']) equal(previousRelease[k],BACKEND_BASELINE_COMMIT,'Pinned V5 backend baseline differs: '+k);
 equal(previousRelease.driverSourceCommit,DRIVER_COMMIT,'Pinned V5 driver identity differs'); equal(previousRelease.navigationDependencyCommit,NAV_COMMIT,'Pinned V5 navigation identity differs');
 for(const k of ['observationDependencyCommit','frontendSourceCommit']) equal(previousRelease[k],OBSERVATION_COMMIT,'Pinned V5 JS/TS origin differs: '+k);
 equal(baseline.sourceCommit,BACKEND_BASELINE_COMMIT,'Pinned V5 source baseline differs'); assert(baseline.files.length===55,'Pinned V5 original closure differs');
 equal(gitText(repo,['rev-parse','--show-toplevel']).replaceAll('\\','/').toLowerCase(),repo.replaceAll('\\','/').toLowerCase(),'Repository root required'); equal(gitText(repo,['rev-parse','HEAD']),expectedHead,'Actual HEAD mismatch'); git(repo,['merge-base','--is-ancestor',BACKEND_BASELINE_COMMIT,expectedHead]);
 assert(git(repo,['diff','--name-only','-z','HEAD','--']).length===0,'Tracked worktree must be clean');
 const untracked=paths(git(repo,['ls-files','--others','-z','--','mir2-web3'])).filter(backend); assert(untracked.length===0,'Ignored-inclusive untracked backend files: '+untracked.join(','));
 const changed=paths(git(repo,['diff','--name-only','--no-renames','-z',BACKEND_BASELINE_COMMIT,expectedHead,'--','mir2-web3'])).filter(backend);
 equal(changed,c.allowedBackendDelta,'Actual non-documentation changes differ from Root final Rust whitelist');
 const anchors=baseline.files.filter(f=>!f.path.endsWith('.rs')).map(f=>f.path).sort(); assert(anchors.length===5,'Expected original five Cargo anchors');
 const wanted=[...new Set([...baseline.files.map(f=>f.path),...c.allowedBackendDelta])].sort();
 equal(wanted,rawRows(c.backendSourceFiles).map(r=>r.path),'Root source rows must cover original55 union final whitelist'); equal(wanted.length,c.backendSourceInputCount,'Actual backend input count differs');
 const closure=[...new Set([...paths(git(repo,['diff','--name-only','--no-renames','-z',baseline.gitBase,expectedHead,'--','mir2-web3'])).filter(backend),...anchors])].sort(); equal(closure,wanted,'gitBase-to-new Rust/Cargo closure differs');
 const files=wanted.map(p=>binding(repo,expectedHead,p)); exactRows(files,c.backendSourceFiles,'Root final source snapshot');
 for(const old of baseline.files) if(!c.allowedBackendDelta.includes(old.path)) equal(files.find(f=>f.path===old.path),old,'Unchanged baseline raw/GitLF/object changed: '+old.path);
 const layer=(name,count,origin)=> { const old=baseline[name]; assert(old?.length===count,'Pinned historical layer count differs: '+name); return old.map(r=>{ const blob=git(repo,['show',`${origin}:${r.path}`]); equal(r.committedGitObject,gitText(repo,['rev-parse',`${origin}:${r.path}`]),'Historical canonical object differs: '+r.path); equal(blob.length,r.gitBlobLfBytes,'Historical GitLF bytes differ: '+r.path); equal(sha(blob),r.gitBlobLfSha256,'Historical GitLF SHA differs: '+r.path); return r; }); };
 const current={schemaVersion:1,repository:repo,sourceCommit:expectedHead,backendSourceCommit:expectedHead,executionSourceCommit:expectedHead,driverSourceCommit:DRIVER_COMMIT,navigationDependencyCommit:NAV_COMMIT,observationDependencyCommit:OBSERVATION_COMMIT,frontendSourceCommit:OBSERVATION_COMMIT,gitBase:baseline.gitBase,backendBaselineCommit:BACKEND_BASELINE_COMMIT,sourceInputCount:files.length,files,sourceManifestSha256:sha(Buffer.from(JSON.stringify(files))),allowedBackendDelta:c.allowedBackendDelta,baselineOriginalInputCount:55,unchangedBaselineInputCount:baseline.files.filter(f=>!c.allowedBackendDelta.includes(f.path)).length,cargoAnchorCount:5,driverFiles:layer('driverFiles',9,DRIVER_COMMIT),navigationFiles:layer('navigationFiles',2,NAV_COMMIT),observationFiles:layer('observationFiles',2,OBSERVATION_COMMIT),sourceFrontendFiles:layer('sourceFrontendFiles',3,OBSERVATION_COMMIT),trackedTreeClean:true,ignoredInclusiveUntrackedBackendCount:0,rustSourceReuse:false,driverIdentityScope:'Historical V5 JS origins only. Current preparation JS is qualified separately by its new Root receipt.',currentNodeSourceQualificationPerformed:false};
 current.historicalNodeValidation=reusedReceipt('node',current,baseline); current.historicalFocusedObservationFrontendValidation=reusedReceipt('focused',current,baseline); current.historicalTypecheckValidation=reusedReceipt('typecheck',current,baseline);
 return current;
}
function rustTotals(text) { return [...text.matchAll(/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/g)].map(m=>({status:m[1],passed:+m[2],failed:+m[3],ignored:+m[4],measured:+m[5],filteredOut:+m[6]})); }
export function validateRustReceipt(config,current) {
 const r=readJsonPin(config.freshRustReceipt); assert(r.schemaVersion===1&&r.sourceUnchanged===true&&r.allSelectedTestsPassed===true,'New Root Rust checks did not all pass unchanged');
 exactRows(r.sourceBefore,current.files,'Fresh Rust sourceBefore'); exactRows(r.sourceAfter,current.files,'Fresh Rust sourceAfter');
 assert(Array.isArray(r.checks),'Missing fresh Root checks'); equal(r.checks.map(c=>c.name).sort(),config.rustCheckNames,'Fresh Rust exact check set differs');
 let selectedRustTestsPassed=0;
 const results=r.checks.map(c=>{ assert(c.exitCode===0&&c.signal===null&&c.error===null,'Fresh Rust check failed: '+c.name); assert(Array.isArray(c.command)&&c.command.length>0,'Fresh Rust actual command missing: '+c.name); const stdout=verifyPin(c.stdout).toString('utf8'),stderr=verifyPin(c.stderr).toString('utf8'),totals=rustTotals(stdout+'\n'+stderr);
  if(c.name==='format') assert(totals.length===0&&(c.totals==null||Array.isArray(c.totals)&&c.totals.length===0),'Format must not claim Rust tests');
  else { assert(c.command.includes('cargo')&&c.command.includes('test')&&c.command.includes('--locked')&&c.command.includes('--offline')&&!c.command.includes('--no-run'),'Expected actual locked offline cargo test command: '+c.name); assert(totals.length>0&&Array.isArray(c.totals)&&c.totals.length===totals.length,'Fresh Rust summaries missing: '+c.name); let passed=0;
   totals.forEach((t,i)=>{assert(t.status==='ok'&&t.failed===0&&t.ignored===0,'Fresh raw Rust summary failed: '+c.name); for(const k of ['passed','failed','ignored']) equal(c.totals[i][k],t[k],'Fresh declared Rust total differs: '+c.name+'/'+k); for(const k of ['measured','filteredOut']) if(k in c.totals[i]) equal(c.totals[i][k],t[k],'Fresh declared Rust total differs: '+c.name+'/'+k); if('status' in c.totals[i]) equal(String(c.totals[i].status).toLowerCase(),t.status,'Fresh declared Rust status differs'); passed+=t.passed;}); assert(passed>0,'Fresh check ran zero selected tests: '+c.name); selectedRustTestsPassed+=passed; }
  return {name:c.name,command:c.command,exitCode:0,stdout:c.stdout,stderr:c.stderr,actualTotals:totals}; });
 equal(selectedRustTestsPassed,config.expectedRustTestsPassed,'Fresh actual Rust total differs from Root final expected count');
 return {receipt:config.freshRustReceipt,selectedRustTestsPassed,sourceUnchanged:true,allSelectedTestsPassed:true,fresh:true,reused:false,rustReexecuted:true,executionSourceCommit:current.sourceCommit,sourceInputCount:current.sourceInputCount,results,scope:'Only the newly executed Root selected Rust checks on the complete new backend closure; historical Rust57 and V4 Rust111 are not reused.'};
}
export function requireFrozen({repositoryDirectory,expectedHead}) {
 const releasePin=pin(path.join(DIRECTORY,'release-v6.json')),release=readJsonPin(releasePin); assert(release.schemaVersion===1,'Unsupported release schema');
 equal(release.config.file,path.join(DIRECTORY,'root-final-config-v6.json'),'Unexpected sealed Root config copy'); const config=loadConfig(release.config,expectedHead);
 equal(release.backendSourceManifest.file,path.join(DIRECTORY,'backend-source-v6.json'),'Unexpected source manifest path'); const manifest=readJsonPin(release.backendSourceManifest); assert(manifest.schemaVersion===1,'Unsupported manifest schema'); equal(manifest.config,release.config,'Config binding differs'); equal(manifest.baselineManifest,BASELINE_PIN,'V5 source baseline pin differs'); equal(manifest.baselineRelease,BASELINE_RELEASE_PIN,'V5 release baseline pin differs');
 const current=capture({repositoryDirectory,expectedHead,configPin:release.config}),rustValidation=validateRustReceipt(config,current); current.selectedRustTestsPassed=rustValidation.selectedRustTestsPassed;
 equal(manifest.current,current,'Frozen current sources drift'); equal(manifest.rustValidation,rustValidation,'Fresh Rust validation drift'); equal(release.rustValidation,config.freshRustReceipt,'Release Rust pin differs');
 equal(manifest.toolPins.map(p=>p.file).sort(),['source-lib-v6.mjs','build-release-v6.mjs'].map(n=>path.join(DIRECTORY,n)).sort(),'V6 tool source set differs'); for(const p of manifest.toolPins) verifyPin(p);
 equal(release.buildReceipt.file,path.join(DIRECTORY,'build-receipt-v6.json'),'Unexpected build receipt path'); const b=readJsonPin(release.buildReceipt); assert(b.schemaVersion===1&&b.exitCode===0&&b.signal===null&&b.error===null&&b.sourceUnchanged===true&&b.sourceCaptureError===null&&b.sealError===null,'Actual V6 build/seal failed'); equal(b.command,['rustup',...BUILD_ARGS],'Build command differs'); equal(path.resolve(b.cwd),path.join(path.resolve(repositoryDirectory),'mir2-web3'),'Build cwd differs'); equal(path.resolve(b.targetDirectory),path.resolve(TARGET),'Build target differs'); assert(Date.parse(b.finishedAt)>=Date.parse(b.startedAt),'Build times invalid'); verifyPin(b.stdout); verifyPin(b.stderr); equal(readJsonPin(b.sourceBefore),current,'Build before source differs'); equal(readJsonPin(b.sourceAfter),current,'Build after source differs');
 equal(b.immutableRelease,release.immutableRelease,'Immutable/build binding differs'); equal(release.immutableRelease.file,path.join(DIRECTORY,'bin/mir2-gateway.exe'),'Unexpected immutable path'); assert(b.actualBuildOutput?.bytes===release.immutableRelease.bytes&&b.actualBuildOutput?.sha256===release.immutableRelease.sha256&&release.immutableRelease.bytes>0,'Actual build/copy hash mismatch'); verifyPin(release.immutableRelease);
 for(const [k,v] of Object.entries(QUALIFICATION)) {equal(release[k],v,'Release qualification differs: '+k); equal(manifest[k],v,'Manifest qualification differs: '+k);}
 for(const k of ['sourceCommit','backendSourceCommit','executionSourceCommit','driverSourceCommit','navigationDependencyCommit','observationDependencyCommit','frontendSourceCommit']) equal(release[k],current[k],'Layer identity differs: '+k);
 equal(release.selectedRustTestsPassed,rustValidation.selectedRustTestsPassed,'Release fresh Rust total differs'); equal(manifest.selectedRustTestsPassed,rustValidation.selectedRustTestsPassed,'Manifest fresh Rust total differs');
 for(const [k,v] of Object.entries({rustTestsReexecuted:true,freshNodeTestsPassed:0,reusedNodeTestsPassed:0,reusedFocusedTestsPassed:0,typecheckReexecuted:false})) equal(release[k],v,'Fresh/reused validation identity differs: '+k);
 return {manifest,release,current,manifestPin:release.backendSourceManifest,releasePin,backendSourceCommit:expectedHead,executionSourceCommit:expectedHead,driverSourceCommit:DRIVER_COMMIT,navigationDependencyCommit:NAV_COMMIT,observationDependencyCommit:OBSERVATION_COMMIT,frontendSourceCommit:OBSERVATION_COMMIT};
}
