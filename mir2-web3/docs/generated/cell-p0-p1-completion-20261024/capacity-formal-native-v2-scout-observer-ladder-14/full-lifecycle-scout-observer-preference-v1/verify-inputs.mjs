// Prepared full16 preflight. A null/PENDING candidate refuses before private
// provisioning, driver import, process launch, or backend source capture.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
const base='C:/mir2-build/p01-completion-20261024';
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const read=async file=>JSON.parse(await fs.readFile(file,'utf8'));
const pin=async file=>{const bytes=await fs.readFile(file);return{path:file,bytes:bytes.length,sha256:sha(bytes)}};
const match=(a,b)=>a?.bytes===b?.bytes&&a?.sha256===b?.sha256;
const normal=file=>path.resolve(file).replaceAll('\\','/').toLowerCase();
const hex=text=>typeof text==='string'&&/^[a-f0-9]{64}$/.test(text);
const safe=name=>typeof name==='string'&&!path.isAbsolute(name)&&!name.includes('\\')&&!name.split('/').some(p=>!p||p==='.'||p==='..')&&!name.includes(':');
const reference=value=>value&&typeof value==='object'&&!Array.isArray(value)&&JSON.stringify(Object.keys(value).sort())===JSON.stringify(['bytes','path','sha256'])&&typeof value.path==='string'&&path.isAbsolute(value.path)&&Number.isSafeInteger(value.bytes)&&value.bytes>0&&hex(value.sha256);
async function candidateBinding(taskRoot){
  const candidate=await read(path.join(taskRoot,'candidate-release-binding-v1.json'));
  if(candidate.schemaVersion!==1||candidate.state!=='PENDING_ROOT_REVIEWED_RUNTIME_REUSE'||candidate.sourceCommit!=='c743122af05faca43d9535d8c6c2a4bfb01b924f'||candidate.backendSourceCommit!=='c743122af05faca43d9535d8c6c2a4bfb01b924f'||candidate.executionSourceCommit!=='3a6d8c862d03ded3133605472a8f38bc6258af52'||candidate.driverSourceCommit!=='3a6d8c862d03ded3133605472a8f38bc6258af52'||candidate.backendInputCount!==50||
    !['backendSourceManifest','releaseRecord','binary','buildReceipt','buildLog','rootFunctionalValidation','runtimeBinding','readonlyRuntimeGuard','currentDriverFrozen','currentNodeRegression'].every(key=>reference(candidate[key])))throw Error('PENDING_FRESH50_SOURCE_AND_RELEASE: root must first supply the actual new commit, manifest, Release, binary, build and Rust-validation receipt pins; no finalization/provisioning/runtime is permitted');
  return candidate;
}
export async function validateCandidate(repo,taskRoot,expectedHead,{requireFinalBinding=true}={}) {
  const candidate=await candidateBinding(taskRoot);
  if(!/^[a-f0-9]{40}$/.test(expectedHead??'')||candidate.executionSourceCommit!==expectedHead)throw Error('Final explicit committed50-input backend HEAD is required');
  const profileFile=path.join(taskRoot,'full-lifecycle-profile.json'),profileBytes=await fs.readFile(profileFile),profile=JSON.parse(profileBytes);
  const releaseFile=path.join(base,'release-account-store-wait-diagnostics-v1.json'),manifestFile=path.join(base,'backend-source-account-store-wait-diagnostics-v1.json');
  const libraryFile=path.join(base,'scout-observer-preference-runtime-v1/source-lib-v1.mjs');
  if(normal(profile.repository)!==normal(repo)||normal(profile.releaseRecord)!==normal(releaseFile)||normal(profile.backendSourceManifest)!==normal(manifestFile)||
    profile.expectedBackendInputCount!==50||profile.dynamicBackendInputCount!==true||profile.driverPins?.length!==6||profile.toolPins?.length!==7||
    profile.profile!=='full-lifecycle'||profile.durationSeconds!==900||profile.ordinaryRegistrationCount!==2||profile.qaAdminEnabled!==false||profile.normalPlayerGates!==true||
    profile.socketDiagnosticsEnabled!==true||profile.slowStageThresholdMs!==50||profile.intendedBackendSourceCommit!==candidate.backendSourceCommit||profile.intendedExecutionSourceCommit!==expectedHead||profile.intendedDriverSourceCommit!==candidate.driverSourceCommit||!hex(profile.intendedSourceManifestSha256)||!hex(profile.intendedSourceManifestFileSha256)||
    !reference(profile.externalSourceLibrary)||normal(profile.externalSourceLibrary.path)!==normal(libraryFile)||!hex(profile.intendedBinarySha256))throw Error('Prepared full16 candidate profile identity differs');
  for(const entry of [...profile.driverPins.map(p=>({...p,root:repo})),...profile.toolPins.map(p=>({...p,root:taskRoot}))]){
    if(!safe(entry.file)||!hex(entry.sha256)||!Number.isSafeInteger(entry.bytes)||entry.bytes<=0)throw Error('Unsafe prepared source pin');
    if(!match(await pin(path.join(entry.root,entry.file)),entry))throw Error('Original lifecycle driver or prepared launcher changed');
  }
  if(!match(await pin(libraryFile),profile.externalSourceLibrary))throw Error('Actual original-location source library raw pin differs');
  const runtimeBindingFile=path.join(base,'scout-observer-preference-runtime-v1/current-execution-binding-v1.json'),runtimeGuardFile=path.join(base,'scout-observer-runtime-readonly-guard-stdout-v1.log');
  for(const [key,file] of [['runtimeBinding',runtimeBindingFile],['readonlyRuntimeGuard',runtimeGuardFile],['currentDriverFrozen',path.join(base,'capacity-scout-observer-preference-driver-frozen-v1.json')],['currentNodeRegression',path.join(base,'scout-observer-preference-root-full-driver-receipt-v1.json')]])if(normal(candidate[key].path)!==normal(file)||!match(await pin(file),candidate[key]))throw Error('Actual current runtime/Node proof pin differs');
  const guard=await read(runtimeGuardFile);if(guard.readonlySourceGuardPassed!==true||guard.backendBuildCommit!==candidate.backendSourceCommit||guard.currentExecutionHead!==expectedHead||guard.driverSourceCommit!==candidate.driverSourceCommit||guard.backendInputs!==50||guard.allFiftyBackendInputsUnchanged!==true||guard.rebuildPerformed!==false||guard.backendSourceSha256!==profile.intendedSourceManifestSha256||guard.driverSourceSha256!==candidate.driverSourceSha256)throw Error('Root actual readonly runtime guard proof differs');
  // Only now import the pinned original-location runtime adapter; no build library guard is called.
  const checks=await import(pathToFileURL(path.join(taskRoot,'projection-build-source-lib.mjs')));
  if(checks.EXPECTED_INPUT_COUNT!==50)throw Error('Source library expected closure differs');
  const frozen=await checks.requireFrozen({repositoryDirectory:repo,expectedHead});
  if(frozen.backendSourceCommit!==candidate.backendSourceCommit||frozen.driverSourceCommit!==candidate.driverSourceCommit||frozen.executionSourceCommit!==expectedHead||frozen.rebuildPerformed!==false||frozen.current.allFiftyBackendInputsUnchanged!==true||frozen.current.driverSourceSha256!==candidate.driverSourceSha256||frozen.manifest.files?.length!==50||frozen.manifest.sourceManifestSha256!==profile.intendedSourceManifestSha256||sha(frozen.manifestBytes)!==profile.intendedSourceManifestFileSha256)throw Error('Intended committed50-input source proof differs');
  for(const [key,file] of [['backendSourceManifest',manifestFile],['releaseRecord',releaseFile],['buildReceipt',checks.RECEIPT],['buildLog',checks.BUILD_LOG],['binary',checks.IMMUTABLE_BINARY]]){
    if(normal(candidate[key].path)!==normal(file)||!match(await pin(file),candidate[key]))throw Error('Root-confirmed candidate '+key+' pin differs');
  }
  if(JSON.stringify(candidate.rootFunctionalValidation)!==JSON.stringify(frozen.manifest.rootFunctionalValidation)||!match(await pin(candidate.rootFunctionalValidation.path),candidate.rootFunctionalValidation))throw Error('Actual prebuild Rust-validation reference differs');
  const releaseRecordBytes=await fs.readFile(releaseFile),release=JSON.parse(releaseRecordBytes);
  if(release.schemaVersion!==1||release.profile!=='release'||release.sourceCommit!==candidate.backendSourceCommit||release.baseCommit!==checks.GIT_BASE||normal(release.backendSourceManifest)!==normal(manifestFile)||
    release.backendInputCount!==50||release.sourceManifestSha256!==frozen.manifest.sourceManifestSha256||release.sourceManifestFileSha256!==sha(frozen.manifestBytes)||
    release.sourceVerifiedBeforeAndAfterBuild!==true||release.ignoredInclusiveUntrackedBackendCount!==0||checks.OLD_BINARY_SHAS.includes(release.binary?.sha256)||
    normal(release.binary?.path??'')!==normal(checks.IMMUTABLE_BINARY)||release.binary?.sha256!==profile.intendedBinarySha256||!hex(release.binary.sha256)||!match(release.binary,candidate.binary)||
    JSON.stringify(release.buildArguments)!==JSON.stringify(checks.BUILD_ARGS)||release.buildJobs!==4||!['x64','amd64'].includes(release.visualStudio?.targetArch)||!/[\\/]VC[\\/]Tools[\\/]MSVC[\\/]/i.test(release.visualStudio?.linkExecutable??''))throw Error('Fresh50-input account-store-wait-diagnostics Release binding differs');
  if(normal(release.buildReceipt?.path??'')!==normal(checks.RECEIPT)||!match(candidate.buildReceipt,release.buildReceipt)||normal(release.buildLog?.path??'')!==normal(checks.BUILD_LOG)||!match(candidate.buildLog,release.buildLog))throw Error('Actual complete build receipt/log pins are missing');
  const receipt=await read(checks.RECEIPT),log=await fs.readFile(checks.BUILD_LOG,'utf8');
  if(receipt.kind!=='account-store-wait-diagnostics-v1-owned-cargo-build'||receipt.success!==true||receipt.exit?.code!==0||receipt.exit?.signal||receipt.error||receipt.postSourceFailure||receipt.sourceUnchangedBeforeAfter!==true||receipt.backendInputCount!==50||receipt.sourceCommit!==candidate.backendSourceCommit||
    receipt.sourceManifestFileSha256!==release.sourceManifestFileSha256||receipt.sourceManifestSha256!==release.sourceManifestSha256||JSON.stringify(receipt.arguments)!==JSON.stringify(checks.BUILD_ARGS)||receipt.buildJobs!==4||
    !receipt.toolchain?.rustc?.startsWith('rustc 1.95.0 ')||!receipt.toolchain?.cargo?.startsWith('cargo 1.95.0 ')||JSON.stringify(receipt.visualStudio)!==JSON.stringify(release.visualStudio)||
    !match(receipt.immutableBinary,release.binary)||!match(receipt.buildLog,release.buildLog)||!(/Finished [\x60]release[\x60] profile/.test(log))||/^error(?:\[|:)/m.test(log))throw Error('Actual successful Release build evidence differs');
  if(requireFinalBinding){
    const binding=await read(path.join(taskRoot,'launcher-final-binding-v1.json'));
    await verifyPreparedReview(taskRoot,binding.review.path,binding.review.sha256);
    if(binding.state!=='final-50-input-release-bound-not-executed'||binding.expectedSourceCommit!==expectedHead||binding.backendSourceCommit!==candidate.backendSourceCommit||binding.executionSourceCommit!==expectedHead||binding.driverSourceCommit!==candidate.driverSourceCommit||binding.backendInputCount!==50||
      !match(binding.profile,await pin(profileFile))||!match(binding.candidateReleaseBinding,await pin(path.join(taskRoot,'candidate-release-binding-v1.json')))||!match(binding.releaseRecord,candidate.releaseRecord)||!match(binding.backendSourceManifest,candidate.backendSourceManifest)||
      !match(binding.binary,release.binary)||binding.realmStarted!==false||binding.accountsCreated!==0)throw Error('Final launcher binding is absent or differs');
  }
  return{profile,backendBytes:frozen.manifestBytes,backendManifest:frozen.manifest,backendInputCount:50,releaseRecordBytes,
    releaseInput:{executable:release.binary.path,bytes:release.binary.bytes,sha256:release.binary.sha256,sourceCommit:release.sourceCommit,backendSourceCommit:release.sourceCommit,executionSourceCommit:expectedHead,driverSourceCommit:candidate.driverSourceCommit,baseCommit:release.baseCommit,
      backendSourceManifest:manifestFile,backendInputCount:50,backendSourceAggregate:release.sourceManifestSha256,backendSourceManifestFileSha256:release.sourceManifestFileSha256,releaseRecordFileSha256:sha(releaseRecordBytes)}};
}
export async function verifyPreparedReview(taskRoot,reviewFile,reviewSha){
  if(typeof reviewFile!=='string'||!path.isAbsolute(reviewFile)||!hex(reviewSha))throw Error('Explicit actual review file and SHA are required');
  const stat=await fs.lstat(reviewFile),real=await fs.realpath(reviewFile),relative=path.relative(base,real);if(!stat.isFile()||stat.isSymbolicLink()||!relative||path.isAbsolute(relative)||relative==='..'||relative.startsWith('..'+path.sep)||relative.split(path.sep).some(p=>/^(private|store|recordings|bin)$/i.test(p)))throw Error('Review must be public regular BASE evidence');
  const bytes=await fs.readFile(real);if(sha(bytes)!==reviewSha)throw Error('Independent prepared review SHA differs');const review=JSON.parse(bytes),preparedFile=path.join(taskRoot,'template-prepared-v2.json'),preparedBytes=await fs.readFile(preparedFile);
  if(review.schemaVersion!==1||normal(review.preparation?.file??'')!==normal(preparedFile)||review.preparation.bytes!==preparedBytes.length||review.preparation.sha256!==sha(preparedBytes)||review.verdict?.preparedSourceContractPassed!==true||review.verdict.eligibleForRootFinalization!==true||!Array.isArray(review.verdict.blockingFindingIds)||review.verdict.blockingFindingIds.length)throw Error('Exact prepared source review must approve Root finalization without blockers');
  const prepared=JSON.parse(preparedBytes);if(prepared.state!=='PENDING_ROOT_FINALIZATION_NOT_EXECUTED'||!Array.isArray(prepared.files)||prepared.files.length>32||sha(JSON.stringify(prepared.files))!==prepared.sourceSha256||prepared.syntax?.passed!==true||prepared.actualFull16Passed!==null)throw Error('Prepared source closure differs');
  for(const entry of prepared.files){if(!reference({path:entry.file,bytes:entry.bytes,sha256:entry.sha256})||!normal(entry.file).startsWith(normal(taskRoot)+'/')||!match(await pin(entry.file),entry))throw Error('Prepared current raw source changed');}
  return{review:await pin(real),preparation:await pin(preparedFile)};
}
export async function loadPreparedInputs(repo,taskRoot){
  await candidateBinding(taskRoot);
  let binding;try{binding=await read(path.join(taskRoot,'launcher-final-binding-v1.json'));}catch(error){if(error.code==='ENOENT')throw Error('PENDING_ROOT_FINALIZATION: reviewed final launcher binding is required before private initialization, driver import or realm');throw error;}
  return validateCandidate(repo,taskRoot,binding.expectedSourceCommit);
}
