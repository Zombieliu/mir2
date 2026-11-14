import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {pathToFileURL,fileURLToPath} from 'node:url';
const BASE='C:/mir2-build/p01-completion-20261024',ROOT=path.join(BASE,'release-native-10-v2-attack-deadline-feasibility-16');
const SELF=fileURLToPath(import.meta.url),NOTE=path.join(BASE,'root-resume-probe16-source-pins-partial-state-v1.json'),RECEIPT=path.join(BASE,'root-resume-probe16-source-pins-execution-v1.json');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const references={
 freeze:['launcher-source-frozen-v1.json',40976,'a22ff77e310680b4f95d0daf5cc630bdc9c899032a265eefa890212ba39e356c'],
 guard:['source-guard-readonly-verification.json',2302,'757ff7b9fd6dcfaa809867b7192df5a24f0b47cb47524f78fd96f92d1e5510de'],
 prepare:['prepare-inputs.mjs',11638,'33f1801a462608e385a64acfa197688cf3c73b29936e957ee3dbdde78abf5dd6'],
 provisioning:['provisioning.json',330,'8e6998c671da917a36c0b8ce511ab62da153b4f79c087543822c4db2a1c171b4'],
 build:['gateway-build-pin.json',1375,'9982a8530ce9e931e9b4f0ea4caf46fe2b0f284cc33cdf3e516df08af3ada6c4'],
 lock:['preparation.lock',230,'58bec2db25faf386e24dcc74ac4eff73c1ba97804586f8fa5d55382d67367e96'],
 failedLog:['root-prepare-actual-v1.log',924,'5f76607cbfb3371018ba1a7df418bf200e6f3697674b437c3309d002d7e9eb84']};
if(process.argv.slice(2).join(' ')!=='--resume-existing-probe16-source-pins')throw Error('Explicit source-pins-only recovery flag required; never rerun account preparation');
const normal=x=>path.resolve(x).toLowerCase();
async function regular(file){const st=await fs.lstat(file);assert.ok(st.isFile()&&!st.isSymbolicLink());assert.equal(normal(await fs.realpath(file)),normal(file));return st;}
async function absent(file){try{await fs.lstat(file);throw Error('Refusing existing execution/source-pin/output state: '+file);}catch(error){if(error.code!=='ENOENT')throw error;}}
async function externalPin(file){await regular(file);const b=await fs.readFile(file);return{file:path.resolve(file),bytes:b.length,sha256:hash(b)}}
for(const file of [path.join(ROOT,'source-pins.json'),path.join(ROOT,'execution.lock'),path.join(ROOT,'realm-execution.json'),path.join(ROOT,'realm-cleanup.json'),path.join(ROOT,'compact-navigation-wire-terminal.json'),NOTE,RECEIPT])await absent(file);
const facts={};for(const [key,[name,bytes,sha256]]of Object.entries(references)){const actual=await externalPin(path.join(ROOT,name));assert.equal(actual.bytes,bytes);assert.equal(actual.sha256,sha256);facts[key]=actual;}
for(const name of ['private','bin','reports','recordings']){const file=path.join(ROOT,name),st=await fs.lstat(file);assert.ok(st.isDirectory()&&!st.isSymbolicLink());assert.equal(normal(await fs.realpath(file)),normal(file));}
await regular(path.join(ROOT,'launcher-syntax-check.log'));
const guard=JSON.parse(await fs.readFile(facts.guard.file,'utf8'));assert.equal(guard.passed,true);assert.equal(guard.readOnly,true);assert.equal(guard.sourceSeal.sha256,facts.freeze.sha256);assert.equal(guard.sourceSeal.bytes,facts.freeze.bytes);
// These imports are pinned by the immutable99 source freeze. Read/recheck that
// entire source list before evaluating any runtime helper, never the private pool.
const frozen=JSON.parse(await fs.readFile(facts.freeze.file,'utf8'));assert.equal(frozen.files.length,99);assert.equal(hash(Buffer.from(JSON.stringify(frozen.files))),frozen.sourceSha256);
for(const entry of frozen.files){const actual=await externalPin(entry.file);assert.equal(actual.bytes,entry.bytes);assert.equal(actual.sha256,entry.sha256);}
const input=await import(pathToFileURL(path.join(ROOT,'input-lib.mjs')));
const {root,base,project,driverBaselineRoot,endpoint,driverFrozenFile,read,pin,writeNew,sha,assertPin,samePath,verifyRelease,collectDriverInputs}=input;
const {verifyWireDiagnosticSource,wireDiagnosticIdentity,CORE_CODE_EVIDENCE_COMMIT:WIRE_HISTORICAL_CORE_CODE_EVIDENCE_COMMIT}=await import(pathToFileURL(path.join(ROOT,'wire-diagnostics-contract.mjs')));
const {verifySceneContract}=await import(pathToFileURL(path.join(ROOT,'scene-contract.mjs')));
const {CORE_CODE_EVIDENCE_COMMIT}=await import(pathToFileURL(path.join(ROOT,'driver-report-contract.mjs')));

if (!samePath(await fs.realpath(root), root)) throw new Error('Refusing redirected formal root');
const launcherFreezeFile = path.join(root, 'launcher-source-frozen-v1.json'), launcherFreeze = await read(launcherFreezeFile);
if (!Array.isArray(launcherFreeze.files) || sha(JSON.stringify(launcherFreeze.files)) !== launcherFreeze.sourceSha256) throw new Error('Reviewed formal launcher freeze is inconsistent');
for (const entry of launcherFreeze.files) await assertPin(entry);
const releaseBinding = await verifyRelease(), drivers = await collectDriverInputs();
const wireSource=await verifyWireDiagnosticSource(),wireIdentity=wireDiagnosticIdentity(wireSource);
const { verifyDriverReportContract, EXPECTED_NATIVE_PATROL_PLANNING, EXPECTED_NAVIGATION_COORDINATION, EXPECTED_MEASUREMENT_CLOCK, EXPECTED_NATIVE_HP_MAINTENANCE, EXPECTED_NATIVE_ATTACK_WITNESS, EXPECTED_NATIVE_CHASE_PREFERENCE } = await import(pathToFileURL(path.join(ROOT,'driver-report-contract.mjs')));
await verifyDriverReportContract({ navigationCoordination: EXPECTED_NAVIGATION_COORDINATION, nativePatrolPlanning: EXPECTED_NATIVE_PATROL_PLANNING, measurementClock: EXPECTED_MEASUREMENT_CLOCK, nativeHpMaintenance: EXPECTED_NATIVE_HP_MAINTENANCE, nativeAttackWitness: EXPECTED_NATIVE_ATTACK_WITNESS, nativeChasePreference: EXPECTED_NATIVE_CHASE_PREFERENCE });
if (drivers.driverSourceCommit !== releaseBinding.driverSourceCommit) throw new Error('Driver commit changed during preparation');
const scene=await verifySceneContract(root,'probe');
const candidateFile=scene.probeScene.file;
const profileBytes = await fs.readFile(path.join(root, 'resource-profile.json'));
if (!profileBytes.equals(await fs.readFile(path.join(base,'release-native-10-v2-coordinated-feasibility-06/resource-profile.json')))) throw Error('Original ten-player resource guards changed');
const profile=JSON.parse(profileBytes);
if (profile.maxCpuCores !== 8 || profile.hostLogicalProcessors !== 32 || profile.maxPrivateBytes !== 4294967296 || profile.maxWorkingSetBytes !== 8589934592 || profile.minHostFreeBytes !== 8589934592 ||
    profile.maxIoBytesPerSecond !== 536870912 || profile.maxIoOperationsPerSecond !== 100000 || profile.sustainedSamples !== 3 || profile.resourceLimitsEnforcedByMonitor !== false) throw Error('Probe soft resource guards differ');
const options = await read(path.join(root, 'run-options.json'));
if (!samePath(options.scenarioFile ?? '', path.join(root, 'scenario.json')) || !samePath(options.resourceProfile ?? '', path.join(root, 'resource-profile.json')) ||
    options.endpoint !== endpoint || options.profile !== 'probe' || options.activityMode !== 'native' || options.stages !== '2,10' || options.stageSeconds !== 300 || options.soakSeconds !== 360 ||
    options.combatActors !== 1 || options.resumeSamples !== 0 || options.saveSamples !== 0 || options.wallSeconds !== 3600 || options.movementIntervalMs !== 750 || options.attackResults !== 'v2') throw new Error('Frozen halo workload differs');
const { capacityPolicy, LOGIN_WINDOW_MS } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-soak-core.mjs')));
if (JSON.stringify(capacityPolicy(options)) !== JSON.stringify(options.policy) || LOGIN_WINDOW_MS !== 901000) throw new Error('Formal policy or ordinary login window differs');


// Read-only reconstruction of the exact ACL summary originally returned by
// initialize-private.ps1. No Set-Acl, icacls, account-store or credential read.
const aclEnv=Object.fromEntries(Object.entries(process.env).filter(([key])=>key.toLowerCase()!=='psmodulepath'));
const acl={};
for(const name of ['private','recordings']){
 const target=path.join(ROOT,name),literal=target.replaceAll("'","''");
 const command="$ErrorActionPreference='Stop';$taskOwner=[Security.Principal.WindowsIdentity]::GetCurrent().User;$taskSystem=[Security.Principal.SecurityIdentifier]::new('S-1-5-18');$taskApplied=Get-Acl -LiteralPath '"+literal+"';$taskAllowed=@($taskOwner.Value,$taskSystem.Value);$taskRules=@($taskApplied.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]));if(-not $taskApplied.AreAccessRulesProtected -or $taskRules.Count -ne 2 -or @($taskRules|Where-Object {$_.IsInherited -or $_.IdentityReference.Value -notin $taskAllowed -or $_.AccessControlType -ne [Security.AccessControl.AccessControlType]::Allow}).Count){throw 'Existing private ACL verification failed'};@{ownerOnlyVerified=$true;allowedPrincipalCount=$taskRules.Count}|ConvertTo-Json -Compress";
 acl[name]=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',command],{encoding:'utf8',windowsHide:true,env:aclEnv,timeout:30000}));
 assert.equal(acl[name].ownerOnlyVerified,true);assert.equal(acl[name].allowedPrincipalCount,2);
}
const provisioning=await read(path.join(ROOT,'provisioning.json'));
assert.equal(provisioning.schemaVersion,1);assert.equal(provisioning.passed,true);assert.equal(provisioning.requested,10);assert.equal(provisioning.created,10);assert.equal(provisioning.charactersCreated,0);assert.equal(provisioning.gameplayGrants,0);assert.equal(provisioning.networkRequests,0);assert.equal(provisioning.existingStoreModified,false);assert.equal(provisioning.provisioning,'admin-provisioned-file-account-only');
const build=await read(path.join(ROOT,'gateway-build-pin.json')),ownExe=path.join(ROOT,'bin/mir2-gateway.exe');await regular(ownExe);const copied=await pin(ownExe);
assert.equal(copied.bytes,87371264);assert.equal(copied.sha256,'1980b7bed58499977eb6443aa3f1bf0fabcb9e8262b490f2bc6fb74cf6f7b318');assert.equal(copied.bytes,releaseBinding.release.binary.bytes);assert.equal(copied.sha256,releaseBinding.release.binary.sha256);assert.equal(normal(build.ownedExecutable),normal(ownExe));assert.equal(normal(build.sourceExecutable),normal(releaseBinding.release.binary.path));assert.equal(build.bytes,copied.bytes);assert.equal(build.sha256,copied.sha256);assert.equal(build.backendSourceCommit,releaseBinding.backendSourceCommit);assert.equal(build.driverSourceCommit,releaseBinding.driverSourceCommit);assert.deepEqual(build.releaseRecord,releaseBinding.releasePin);assert.equal(build.sourceManifest.sha256,releaseBinding.manifestPin.sha256);assert.equal(build.sourceManifest.bytes,releaseBinding.manifestPin.bytes);assert.equal(build.sourceManifest.inputs,44);assert.equal(build.sourceInputsVerifiedBeforeLaunch,true);assert.equal(build.capacityAccepted,false);assert.equal(build.formalRunStarted,false);
const fullGate=await read(path.join(ROOT,'full16-gate.json'));assert.equal(fullGate.passed,true);assert.equal(fullGate.runId,releaseBinding.full16Gate.runId);assert.equal(fullGate.backendSourceCommit,releaseBinding.backendSourceCommit);assert.deepEqual(fullGate.files,releaseBinding.full16Gate.files);
const lock=await read(path.join(ROOT,'preparation.lock'));assert.equal(lock.backendSourceCommit,releaseBinding.backendSourceCommit);assert.equal(lock.driverSourceCommit,releaseBinding.driverSourceCommit);
const seeder=path.join(base,'../cell-preproduction-p0-p1-20261024/target/debug/seed_capacity_accounts.exe');await regular(seeder);const seedPin=await pin(seeder);await regular(process.execPath);const nodePin=await pin(process.execPath);
const exeLiteral=ownExe.replaceAll("'","''");
const osCommand="$ErrorActionPreference='Stop';$taskOwned=@(Get-CimInstance -Query 'SELECT ProcessId, ExecutablePath FROM Win32_Process'|Where-Object {$_.ExecutablePath -and [IO.Path]::GetFullPath($_.ExecutablePath) -eq [IO.Path]::GetFullPath('"+exeLiteral+"')}|Select-Object -ExpandProperty ProcessId);$taskListeners=@(Get-NetTCPConnection -State Listen -ErrorAction Stop|Where-Object {$_.LocalPort -in @(17500,17510)}|Select-Object -ExpandProperty LocalPort);if($taskOwned.Count -ne 0 -or $taskListeners.Count -ne 0){throw 'Owned gateway or reserved listeners already active'};@{ownedGatewayProcessCount=$taskOwned.Count;reservedListenPortCount=$taskListeners.Count;ports=@(17500,17510);passed=$true}|ConvertTo-Json -Compress";
const os=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',osCommand],{encoding:'utf8',windowsHide:true,env:aclEnv,timeout:30000}));assert.equal(os.passed,true);assert.equal(os.ownedGatewayProcessCount,0);assert.equal(os.reservedListenPortCount,0);
for(const file of [path.join(ROOT,'source-pins.json'),path.join(ROOT,'execution.lock'),path.join(ROOT,'realm-execution.json'),path.join(ROOT,'realm-cleanup.json')])await absent(file);
const selfPin=await externalPin(SELF),note={schemaVersion:1,observedAt:new Date().toISOString(),kind:'read-only-observation-of-existing-partial-preparation-before-source-pin-reconstruction',firstPreparationCompleted:false,firstPreparationFailure:'ENOENT launcher-syntax-check.log after ordinary account-only provisioning; raw actual log preserved separately',firstPreparationRawLog:facts.failedLog,originalPreparationSource:facts.prepare,frozenSource:facts.freeze,readOnlyGuard:facts.guard,publicProvisioning:facts.provisioning,gatewayBuildPin:facts.build,preparationLock:facts.lock,syntaxCheck:await externalPin(path.join(ROOT,'launcher-syntax-check.log')),existingRealOrdinaryAccounts:10,charactersCreated:0,gameplayGrants:0,existingStoreModified:false,acl,os,ownedExecutable:copied,node:nodePin,seeder:seedPin,executablePinsObservedAtResumeOnly:true,privateValuesRead:false,privateMetadataChanged:false,prepareRerun:false,seederInvoked:false,realmStarted:false,sourcePinsAbsentBefore:true,executionLockAbsent:true,resumeScript:selfPin,capacityAccepted:false};
await fs.writeFile(NOTE,JSON.stringify(note,null,2)+'\n',{flag:'wx'});

const scripts = (await fs.readdir(root)).filter(name => /\.(?:mjs|ps1)$/.test(name)).sort();
const publicFiles = [driverFrozenFile, launcherFreezeFile, path.join(root, 'launcher-source-prepared-v1.json'), path.join(root, 'preparation-plan.json'), path.join(root,'final-inputs-approved.json'), path.join(root, 'slow-stage-diagnostics-config.json'), path.join(root, 'static-preparation-verification.json'), path.join(root, 'source-guard-readonly-verification.json'),
  path.join(root,'synthetic-tests-v1.log'),path.join(root,'slow-stage-helper-freeze-v1.json'),path.join(root,'slow-stage-schema-contract.json'),path.join(root,'legacy14-original.mjs.txt'),path.join(root,'candidate-release-binding.json'), path.join(root,'backend-library-binding.json'), path.join(base,'attack-deadline-diagnostics-release-v1/source-lib-v1.mjs'), path.join(root, 'launcher-syntax-check.log'), releaseBinding.releasePin.file, releaseBinding.manifestPin.file, path.join(base,'attack-deadline-diagnostics-v1-build-receipt.json'),
  path.join(base, 'capacity-attack-v2-driver-frozen-v1.json'),
  ...scripts.map(name => path.join(root, name)), ...['wire-adapter-synthetic-tests.log','wire-adapter-synthetic-tests.json','copied-baseline-inputs.json','scenario.json', 'resource-profile.json', 'run-options.json', 'provisioning.json', 'gateway-build-pin.json', 'full16-gate.json'].map(name => path.join(root, name)),
  candidateFile, path.join(root,'scene-binding.json'), path.join(root,'scenario-prefix-readiness.json'), path.join(base,'release-native-10-v2-coordinated-feasibility-06/resource-profile.json')];
const files = [...releaseBinding.full16Gate.files, ...drivers.pins, ...scene.files, ...wireSource.files, ...await Promise.all(publicFiles.map(pin)), await pin(process.execPath), seedPin];
await writeNew(path.join(root, 'source-pins.json'), { schemaVersion: 1, preparedAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, sceneSourceSha256: scene.sourceSha256, sceneProfile: scene.profile,
  backendSourceManifest: releaseBinding.manifestPin.file, socketDiagnosticsReleaseUsed: true, coreCodeEvidenceCommit:CORE_CODE_EVIDENCE_COMMIT, sceneStaticCodeEvidenceCommit:WIRE_HISTORICAL_CORE_CODE_EVIDENCE_COMMIT, wireDiagnostics:wireIdentity, driverFrozenManifest: driverFrozenFile, driverSourceSha256: drivers.frozen.sourceSha256, sourceConfigSha256: sha(JSON.stringify(files)), files,
  localImportModuleCount: drivers.importModuleCount, nextBundledWsEntry: drivers.bundledWsEntry, acl, scope: 'Exact probe release/driver/import/transport/data/runner and new compact span2 halo scene/static source closure. Resource profile and gameplay gates are preserved. No private store/credential/recording payload in public evidence.',
  ordinaryLoginBudget: { admission: 25, reservedVerification: 5, total: 30, windowMs: LOGIN_WINDOW_MS, persistentLedger: true, resetOrBypass: false },
  realOrdinaryAccounts: 10, charactersCreated: 0, gameplayGrants: 0, realmStarted: false, capacityAccepted: false });
console.log(JSON.stringify({ prepared: true, root, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, backendInputs: releaseBinding.manifest.files.length,
  sha256: copied.sha256, realOrdinaryAccounts: 10, charactersCreated: 0, sourceConfigSha256: sha(JSON.stringify(files)), realmStarted: false, capacityAccepted: false }));


const receipt={schemaVersion:1,completedAt:new Date().toISOString(),kind:'source-pins-only-resume-after-existing-account-provisioning',passed:true,partialStateNote:await externalPin(NOTE),resumeScript:selfPin,originalPreparationSource:facts.prepare,sourcePins:await externalPin(path.join(ROOT,'source-pins.json')),sourceConfigSha256:sha(JSON.stringify(files)),node:nodePin,seeder:seedPin,accountPreparationRepeated:false,seederInvoked:false,privateValuesRead:false,privateMetadataChanged:false,accountsAddedHere:0,charactersCreatedHere:0,gameplayGrantsHere:0,realmStarted:false,capacityAccepted:false};
await fs.writeFile(RECEIPT,JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({resumedSourcePinsOnly:true,receipt:await externalPin(RECEIPT),sourceConfigSha256:receipt.sourceConfigSha256,existingRealOrdinaryAccounts:10,accountsAddedHere:0,realmStarted:false,capacityAccepted:false}));
