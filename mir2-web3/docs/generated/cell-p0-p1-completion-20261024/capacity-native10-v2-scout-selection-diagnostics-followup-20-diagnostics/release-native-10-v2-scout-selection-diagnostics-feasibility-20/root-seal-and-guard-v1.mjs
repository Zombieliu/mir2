import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {fileURLToPath,pathToFileURL} from 'node:url';
const ROOT=path.dirname(fileURLToPath(import.meta.url)),BASE=path.dirname(ROOT);
const HEAD='b5374083d257d375437af3de42bead159cd2b917',CORE=HEAD,BUILD='c743122af05faca43d9535d8c6c2a4bfb01b924f',DRIVER='d61b7cd057e1a2a55191fe6c407b3058b5ff9205141eaf9290d16da42b028db1';
const FULL16=path.join(BASE,'full-lifecycle-scout-selection-diagnostics-v1');
const SELF=fileURLToPath(import.meta.url),PREP=path.join(ROOT,'root-controller-preparation-v1.json'),README=path.join(ROOT,'ROOT-CONTROLLER.md');
const FREEZE=path.join(ROOT,'launcher-source-frozen-v1.json'),GUARD=path.join(ROOT,'source-guard-readonly-verification.json'),SYNTAX=path.join(ROOT,'launcher-syntax-check.log');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex'),same=(a,b)=>path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();
const read=async file=>JSON.parse((await fs.readFile(file,'utf8')).replace(/^\uFEFF/,''));
const pin=async input=>{const file=path.resolve(input),b=await fs.readFile(file);return{file,bytes:b.length,sha256:sha(b)}};
const write=(file,value)=>fs.writeFile(file,JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const hex=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const runtime=['private','bin','reports','recordings','preparation.lock','execution.lock','source-pins.json','compact-navigation-wire-terminal.json','realm-cleanup.json','realm-execution.json'];
async function absent(names){for(const name of names){try{await fs.lstat(path.join(ROOT,name));throw Error('Refusing existing runtime/output state: '+name);}catch(error){if(error.code!=='ENOENT')throw error;}}}
async function safeFile(file){
 assert.ok(typeof file==='string'&&path.isAbsolute(file),'Absolute public source path required');
 assert.ok(!file.split(/[\\/]/).some(part=>part==='.'||part==='..'||/^(private|bin|store|recordings|secrets?)$/i.test(part))&&!/\.(exe|dll)$/i.test(file),'Private/binary source path forbidden');
 const stat=await fs.lstat(file),real=await fs.realpath(file),relative=path.relative(await fs.realpath(BASE),real);
 assert.ok(stat.isFile()&&!stat.isSymbolicLink()&&same(real,file),'Regular canonical nonredirected source required');
 assert.ok(relative&&!path.isAbsolute(relative)&&!relative.split(/[\\/]/).some(part=>part==='..'),'Source must remain under external evidence root');return file;
}
async function expectedPin(entry){
 assert.ok(entry&&typeof entry==='object'&&!Array.isArray(entry)&&typeof entry.file==='string'&&Number.isSafeInteger(entry.bytes)&&entry.bytes>=0&&hex(entry.sha256),'Exact file/bytes/sha256 source reference required');
 await safeFile(entry.file);const actual=await pin(entry.file);assert.equal(actual.bytes,entry.bytes,'Source byte count changed: '+entry.file);assert.equal(actual.sha256,entry.sha256,'Source SHA changed: '+entry.file);return actual;
}
function unique(entries){const result=[],seen=new Map();for(const entry of entries){const key=path.resolve(entry.file).toLowerCase(),old=seen.get(key);if(old){assert.equal(old.bytes,entry.bytes);assert.equal(old.sha256,entry.sha256);}else{seen.set(key,entry);result.push(entry);}}assert.ok(result.length>0&&result.length<=128,'Public source closure must contain at most128 unique files');return result;}
async function prepared(file,expectedSha){
 await safeFile(file);assert.ok(same(path.dirname(file),ROOT)&&/^runtime-prepared-v\d+\.json$/.test(path.basename(file)),'Explicit runtime-prepared revision in this root required');
 const ref=await pin(file);assert.equal(ref.sha256,expectedSha);const value=await read(file);
 assert.equal(value.schemaVersion,1);assert.equal(value.state,'ACTUAL_BOUND_UNSEALED_NOT_EXECUTED');
 for(const [list,aggregate] of [[value.files,value.sourceSha256],[value.externalStableInputs,value.externalSourceSha256]]){
  assert.ok(Array.isArray(list)&&list.length>0&&list.length<=120);assert.equal(sha(JSON.stringify(list)),aggregate);for(const entry of list)await expectedPin(entry);
 }
 for(const entry of value.files)assert.ok(same(path.dirname(entry.file),ROOT),'Local source outside runtime root');
 assert.ok(Array.isArray(value.syntax)&&value.syntax.length>0&&value.syntax.every(entry=>entry.exitCode===0));
 assert.equal(value.policyScenarioResourceUnchanged,true);assert.equal(value.runtimeImported,false);assert.equal(value.guardExecuted,false);assert.equal(value.accountsCreated,0);assert.equal(value.realmStarted,false);assert.equal(value.capacityAccepted,false);assert.equal(value.rootExecutionRequired,true);
 const controllerPin=await pin(SELF),controllerPrepPin=await pin(PREP),readmePin=await pin(README);await expectedPin(controllerPin);await expectedPin(controllerPrepPin);await expectedPin(readmePin);
 const controllerPrep=await read(PREP);assert.equal(controllerPrep.schemaVersion,1);assert.equal(controllerPrep.state,'controller-prepared-not-executed');assert.deepEqual(controllerPrep.controller,controllerPin);assert.equal(controllerPrep.syntax.exitCode,0);
 assert.ok(Array.isArray(controllerPrep.supportInputs)&&controllerPrep.supportInputs.length===1);for(const entry of controllerPrep.supportInputs)await expectedPin(entry);
 assert.ok(same(controllerPrep.supportInputs[0].file,path.join(FULL16,'verify-inputs.mjs')),'Only actual new full16 validator may be imported');
 const files=unique([...value.files,...value.externalStableInputs,ref,controllerPin,controllerPrepPin,readmePin,...controllerPrep.supportInputs]);
 return{value,ref,controllerPin,controllerPrepPin,readmePin,files};
}
async function reviewed(pre,file,expectedSha){
 await safeFile(file);const ref=await pin(file);assert.equal(ref.sha256,expectedSha);const value=await read(file);
 assert.equal(value.schemaVersion,1);assert.deepEqual(value.prepared,pre.ref);assert.deepEqual(value.controller,pre.controllerPin);
 assert.equal(value.verdict.preparedContractPassed,true);assert.equal(value.verdict.eligibleForSealNow,true);assert.deepEqual(value.verdict.blockingFindingIds,[]);return ref;
}
async function checkContracts({preSeal}){
 const input=await import(pathToFileURL(path.join(ROOT,'input-lib.mjs'))),candidate=await read(path.join(ROOT,'candidate-release-binding.json'));
 assert.equal(candidate.backendSourceCommit,BUILD);assert.equal(candidate.executionSourceCommit,HEAD);assert.equal(candidate.driverSourceCommit,CORE);assert.equal(candidate.backendInputCount,50);
 let binding;
 if(preSeal){
  // No own freeze exists yet: use the already-finalized new full16 validator,
  // which invokes the original-location new strict50 requireFrozen library.
  const validator=await import(pathToFileURL(path.join(FULL16,'verify-inputs.mjs')));
  const validated=await validator.validateCandidate(input.project,FULL16,HEAD),release=await read(input.releaseFile);
  assert.equal(validated.backendInputCount,50);assert.equal(validated.releaseInput.sourceCommit,BUILD);assert.equal(validated.releaseInput.executionSourceCommit,HEAD);assert.equal(validated.releaseInput.driverSourceCommit,CORE);assert.equal(validated.releaseInput.sha256,release.binary.sha256);
  const {verifyFull16Gate}=await import(pathToFileURL(path.join(ROOT,'verify-full16-gate.mjs')));
  binding={release,manifest:validated.backendManifest,backendSourceCommit:BUILD,backendBuildCommit:BUILD,executionSourceCommit:HEAD,driverSourceCommit:CORE,full16Gate:await verifyFull16Gate(candidate,release,validated.backendManifest)};
 }else binding=await input.verifyRelease();
 assert.equal(binding.manifest.files.length,50);assert.equal(binding.backendSourceCommit,BUILD);assert.equal(binding.executionSourceCommit,HEAD);assert.equal(binding.driverSourceCommit,CORE);assert.equal(binding.release.binary.sha256,candidate.binary.sha256);assert.notEqual(binding.release.binary.sha256,'1980b7bed58499977eb6443aa3f1bf0fabcb9e8262b490f2bc6fb74cf6f7b318');assert.equal(binding.full16Gate.passed,true);
 const drivers=await input.collectDriverInputs();assert.equal(drivers.frozen.sourceSha256,DRIVER);assert.equal(drivers.frozen.files.length,9);assert.equal(drivers.driverSourceCommit,CORE);
 const scene=await(await import(pathToFileURL(path.join(ROOT,'scene-contract.mjs')))).verifySceneContract(ROOT,'probe');
 const wire=await import(pathToFileURL(path.join(ROOT,'wire-diagnostics-contract.mjs'))),wireSource=await wire.verifyWireDiagnosticSource(),wireIdentity=wire.wireDiagnosticIdentity(wireSource);
 const contract=await import(pathToFileURL(path.join(ROOT,'driver-report-contract.mjs')));
 await contract.verifyDriverReportContract({navigationCoordination:contract.EXPECTED_NAVIGATION_COORDINATION,nativePatrolPlanning:contract.EXPECTED_NATIVE_PATROL_PLANNING,measurementClock:contract.EXPECTED_MEASUREMENT_CLOCK,nativeHpMaintenance:contract.EXPECTED_NATIVE_HP_MAINTENANCE,nativeAttackWitness:contract.EXPECTED_NATIVE_ATTACK_WITNESS,nativeChasePreference:contract.EXPECTED_NATIVE_CHASE_PREFERENCE});
 const store=await import(pathToFileURL(path.join(ROOT,'store-diagnostics-contract.mjs'))),storeSource=await store.verifyStoreDiagnosticSource(),storeIdentity=store.storeDiagnosticIdentity(storeSource);
 return{binding,drivers,scene,wireSource,wireIdentity,storeSource,storeIdentity};
}
async function preparationReferences(){
 const files=[];for(const name of ['launcher-source-prepared-v1.json','preparation-plan.json','final-inputs-approved.json','static-preparation-verification.json','copied-baseline-inputs.json','candidate-release-binding.json','current-driver-binding.json','backend-library-binding.json','scenario.json','resource-profile.json','run-options.json','scene-binding.json','scene-driver-compatibility-proof-v1.json','scenario-prefix-readiness.json','slow-stage-diagnostics-config.json','slow-stage-schema-contract.json']){const file=path.join(ROOT,name);await safeFile(file);files.push(await pin(file));}
 const history=path.join(BASE,'release-native-10-v2-account-store-wait-feasibility-17');for(const name of ['slow-stage-public.mjs','slow-stage-public.test.mjs','synthetic-tests-v1.log','slow-stage-helper-freeze-v1.json','slow-stage-schema-contract.json','legacy14-original.mjs.txt','passed-probe-window-contract.test.mjs','wire-diagnostics-adapter.test.mjs','wire-adapter-synthetic-tests.log','wire-adapter-synthetic-tests.json']){const file=path.join(history,name);await safeFile(file);files.push(await pin(file));}
 const file=path.join(BASE,'release-native-10-v2-coordinated-feasibility-06/resource-profile.json');await safeFile(file);files.push(await pin(file));return unique(files);
}
function syntaxChecks(files){
 const checks=[],lines=[],env=Object.fromEntries(Object.entries(process.env).filter(([key])=>key.toLowerCase()!=='psmodulepath'));
 for(const p of files.filter(p=>/\.(mjs|ps1)$/.test(p.file))){const ps=p.file.endsWith('.ps1'),literal=p.file.replaceAll("'","''");
  const script="$ErrorActionPreference='Stop';$taskTokens=$null;$taskErrors=$null;[void][System.Management.Automation.Language.Parser]::ParseFile('"+literal+"',[ref]$taskTokens,[ref]$taskErrors);if($taskErrors.Count){$taskErrors|ForEach-Object {$_.Message};exit 1}";
  const output=execFileSync(ps?'powershell.exe':process.execPath,ps?['-NoProfile','-NonInteractive','-EncodedCommand',Buffer.from(script,'utf16le').toString('base64')]:['--check',p.file],{encoding:'utf8',windowsHide:true,env,timeout:30000,maxBuffer:1048576});
  checks.push({...p,kind:ps?'PowerShell-AST-only':'node--check',exitCode:0});lines.push((ps?'PSAST ':'node --check ')+p.file+' => 0'+(output?'\n'+output:''));
 }
 assert.ok(checks.length>0&&checks.length<=128);return{checks,content:lines.join('\n')+'\n'};
}
const args=process.argv.slice(2),mode=args[0];
if(mode==='--seal-reviewed-inputs'){
 assert.equal(args.length,5);assert.ok(args[1].startsWith('--prepared-file='));assert.match(args[2],/^--prepared-sha256=[a-f0-9]{64}$/);assert.ok(args[3].startsWith('--review-file='));assert.match(args[4],/^--review-sha256=[a-f0-9]{64}$/);
 await absent([...runtime,path.basename(FREEZE),path.basename(GUARD),path.basename(SYNTAX)]);
 const pre=await prepared(args[1].slice('--prepared-file='.length),args[2].slice('--prepared-sha256='.length)),reviewPin=await reviewed(pre,args[3].slice('--review-file='.length),args[4].slice('--review-sha256='.length));
 const references=await preparationReferences(),syntax=syntaxChecks(unique([...pre.files,reviewPin,...references]));
 const checked=await checkContracts({preSeal:true}),{binding,drivers,scene,wireSource,wireIdentity,storeSource,storeIdentity}=checked;
 for(const entry of unique([...pre.files,reviewPin,...references]))await expectedPin(entry);await absent(runtime);
 await fs.writeFile(SYNTAX,syntax.content,{flag:'wx'});const syntaxLog=await pin(SYNTAX);
 const files=unique([...pre.files,reviewPin,...references,syntaxLog]);for(const entry of files)await expectedPin(entry);await absent(runtime);
 const metadata={schemaVersion:1,state:'final-source-bound-not-executed',frozenAt:new Date().toISOString(),root:ROOT,backendSourceCommit:BUILD,backendBuildCommit:BUILD,executionSourceCommit:HEAD,backendInputCount:50,backendRebuilt:false,driverSourceCommit:CORE,coreCodeEvidenceCommit:CORE,driverSourceSha256:DRIVER,binarySha256:binding.release.binary.sha256,backendSourceAggregate:binding.manifest.sourceManifestSha256,sceneProfile:scene.profile,sceneSourceSha256:scene.sourceSha256,patrolSpan:2,storeDiagnostics:storeIdentity,ordinaryAccountPoolSize:100,maximumControlledActors:10,wireDiagnostics:wireIdentity,wireSourceFiles:wireSource.files,coreFunctionalEvidenceCoversWrapper:false,nodeCodeFullTests:232,wrapperUnitTests:28,adapterUnitTests:8,actualAdmissionCaps:[24,10,10,4,1,2],tokioWorkers:4,slowStageThresholdMs:50,full16RunId:binding.full16Gate.runId,preparedReceipt:pre.ref,independentReview:reviewPin,rootSealExecutor:pre.controllerPin,rootControllerPreparation:pre.controllerPrepPin,reviewedRuntimeLocalFiles:pre.value.files.length,reviewedExternalStableInputs:pre.value.externalStableInputs.length,driverClosurePins:drivers.pins.length,syntax:{log:syntaxLog,checks:syntax.checks,targetModulesImported:false,behaviouralTestsExecuted:false},preparationReferencePins:references,sourceConfigNotYet:true,accountsProvisioned:false,realmStarted:false,capacityAccepted:false,files,sourceSha256:sha(JSON.stringify(files)),scripts:files.filter(entry=>/\.(mjs|ps1)$/.test(entry.file))};
 await write(FREEZE,metadata);await absent(runtime);console.log(JSON.stringify({seal:await pin(FREEZE),files:files.length,sourceGuardNotYet:true,accountsProvisioned:false,realmStarted:false,capacityAccepted:false}));
}else if(mode==='--readonly-sourceguard'){
 assert.equal(args.length,1);await absent([...runtime,path.basename(GUARD)]);await safeFile(FREEZE);const frozen=await read(FREEZE);
 assert.equal(frozen.schemaVersion,1);assert.equal(frozen.state,'final-source-bound-not-executed');assert.equal(frozen.backendSourceCommit,BUILD);assert.equal(frozen.executionSourceCommit,HEAD);assert.equal(frozen.backendInputCount,50);assert.equal(frozen.driverSourceCommit,CORE);assert.equal(frozen.driverSourceSha256,DRIVER);assert.ok(Array.isArray(frozen.files));assert.equal(sha(JSON.stringify(frozen.files)),frozen.sourceSha256);
 const pre=await prepared(frozen.preparedReceipt.file,frozen.preparedReceipt.sha256),reviewPin=await reviewed(pre,frozen.independentReview.file,frozen.independentReview.sha256);assert.deepEqual(frozen.preparedReceipt,pre.ref);assert.deepEqual(frozen.independentReview,reviewPin);const references=await preparationReferences();assert.deepEqual(frozen.preparationReferencePins,references);await expectedPin(frozen.syntax.log);assert.ok(same(frozen.syntax.log.file,SYNTAX));assert.ok(Array.isArray(frozen.syntax.checks)&&frozen.syntax.checks.length>0&&frozen.syntax.checks.every(x=>x.exitCode===0));assert.deepEqual(frozen.files,unique([...pre.files,reviewPin,...references,frozen.syntax.log]));for(const entry of frozen.files)await expectedPin(entry);
 const {binding,drivers,scene,wireSource,wireIdentity}=await checkContracts({preSeal:false});assert.deepEqual(wireSource.files,frozen.wireSourceFiles);assert.deepEqual(wireIdentity,frozen.wireDiagnostics);assert.equal(scene.sourceSha256,frozen.sceneSourceSha256);await absent(runtime);
 await write(GUARD,{schemaVersion:1,verifiedAt:new Date().toISOString(),passed:true,readOnly:true,sourceSeal:await pin(FREEZE),preparedReceipt:pre.ref,independentReview:reviewPin,rootSealExecutor:pre.controllerPin,backendSourceCommit:BUILD,backendBuildCommit:BUILD,executionSourceCommit:HEAD,backendInputCount:50,backendSourceUnchangedSinceBuild:true,rebuildPerformed:false,backendSourceAggregate:binding.manifest.sourceManifestSha256,binary:binding.release.binary,driverSourceCommit:CORE,coreCodeEvidenceCommit:CORE,driverSourceSha256:DRIVER,driverClosurePins:drivers.pins.length,full16RunId:binding.full16Gate.runId,full16Passed:true,wireSourcePins:wireSource.files.length,sceneSourceSha256:scene.sourceSha256,sceneStaticEvidenceNotRerun:true,prepareEntryPointInvoked:false,runEntryPointInvoked:false,accountsProvisioned:false,realmStarted:false,capacityAccepted:false});await absent(runtime);console.log(JSON.stringify({sourceGuard:await pin(GUARD),accountsProvisioned:false,realmStarted:false,capacityAccepted:false}));
}else throw Error('Only explicit --seal-reviewed-inputs --prepared-file=<absolute> --prepared-sha256=<sha> --review-file=<absolute> --review-sha256=<sha>, or --readonly-sourceguard is supported. No provisioning, build, test or realm entry exists.');
