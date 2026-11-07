import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir,copyFile,constants} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
const base=import.meta.dirname,root='E:/mir2-player-journey/mir2-web3';
const dest=root+'/docs/evidence/r16-native-resume-wait-20261108';
const sha=b=>createHash('sha256').update(b).digest('hex');
const json=async p=>JSON.parse((await readFile(p,'utf8')).replace(/^\uFEFF/,''));
const source=await json(base+'/qualified-before.json');
for(const [file,pin]of Object.entries(source.pins))assert.equal(sha(await readFile(root+'/'+file)),pin,file);
const qualified=await readFile(base+'/tests-qualified.log','utf8');
assert.match(qualified,/test result: ok\. 15 passed; 0 failed; 0 ignored/);
const names=new Set([...qualified.matchAll(/^test (\S+) \.\.\. ok\r?$/gm)].map(m=>m[1]));
assert.equal(names.size,15);
const regression=await json(base+'/regression-result.json');
assert.equal(regression.uniquePassedCount,103);
assert(regression.unchanged&&regression.cases.every(c=>c.exitCode===0&&c.failed===0&&c.ignored===0));
for(const name of regression.uniquePassed)names.add(name);
assert.equal(names.size,118);
const candidate=await json(base+'/candidate-build.json');
assert.equal(candidate.profile,'dev/debug');
assert(candidate.sourceStableBeforeAndAfter&&!candidate.releaseBuild&&!candidate.publicDeploy);
assert.equal(sha(await readFile(candidate.file)),candidate.sha256);
const original=await json(base+'/source-before.json');
for(const f of ['apps/game-client/platform-windows/src/gateway.rs','apps/game-client/platform-windows/src/session_config.rs'])assert.equal(source.pins[f],original.pins[f]);
const preservation=await json(base+'/preservation.json');
assert.equal(preservation.originalWindowsExeSha256,'d930618bc7cc461a4d03507a68fb28c0b15af6200de8f00946c463929f67d638');
assert.equal(preservation.previousOrdinaryReconnectCandidateSha256,'e519dc7a6d0eb3f399fcb20c1145bb260418ec0c13e3d7ad44327610fc987c55');
assert.deepEqual(preservation.originalListeners.map(r=>r.LocalPort).sort(),[19231,19232,19235]);
const red=await readFile(base+'/tests-pre-consume-clock.log','utf8');
assert.match(red,/credential_expiry_inside_final_live_guard_cannot_be_consumed \.\.\. FAILED/);
assert.match(red,/test result: FAILED\. 12 passed; 3 failed/);
await mkdir(dest);
await mkdir(dest+'/tools');
await mkdir(dest+'/fixtures');
const attributes=Buffer.from('# Preserve the exact bytes recorded in SHA256 verification receipts.\n* -text\n');
await writeFile(dest+'/.gitattributes',attributes,{flag:'wx'});
const artifactPins={'.gitattributes':{sha256:sha(attributes),bytes:attributes.length}};
const sourceGitBlobOids={};
for(const file of Object.keys(source.pins)){
  const relative='mir2-web3/'+file;
  const blob=spawnSync('git',['hash-object','--path='+relative,relative],{cwd:'E:/mir2-player-journey',encoding:'utf8',windowsHide:true});
  assert.equal(blob.status,0);
  sourceGitBlobOids[file]=blob.stdout.trim();
}
async function artifact(from,to=from){
  const bytes=await readFile(base+'/'+from);
  // These artifacts contain only ordinary isolated fixtures, not user runtime data.
  const text=bytes.toString('utf8');
  assert(!/\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{32,}|\b(?:github_pat_[A-Za-z0-9_]{30,}|gh[pousr]_[A-Za-z0-9]{30,})|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/.test(text),'credential-shaped content in '+from);
  await copyFile(base+'/'+from,dest+'/'+to,constants.COPYFILE_EXCL);
  artifactPins[to]={sha256:sha(bytes),bytes:bytes.length};
}
for(const f of ['tests-qualified.log','tests-initial.log','tests-ws-second.log','tests-pre-clock-fix.log','tests-pre-consume-clock.log','pre-clock-source.json','pre-consume-clock.json','qualified-before.json','first-attempt-source.json','candidate-build.json','candidate-build.log','candidate-build.d','regression-result.json','preservation.json'])await artifact(f);
for(const row of regression.cases)await artifact('regression-'+row.filter+'.log');
for(const f of ['freeze-source.mjs','regression.mjs','build-candidate.mjs','publish.mjs'])await artifact(f,'tools/'+f);
await artifact('ws-tests-first-attempt.rs','fixtures/ws-tests-first-attempt.rs');
const diff=spawnSync('git',['diff','--no-index','--',base+'/web-pre-clock-fix.rs',root+'/apps/gateway/src/web.rs'],{encoding:'utf8',windowsHide:true});
assert.equal(diff.status,1);
await writeFile(base+'/consume-clock.diff',diff.stdout,{flag:'wx'});
await artifact('consume-clock.diff');
const summary={
  schema:'mir2.r16.native-resume.verification.v1',createdAt:new Date().toISOString(),
  branch:'codex/windows-player-journey',testedHead:source.head,sourcePins:source.pins,
  sourcePinBasis:'Windows worktree bytes; canonical Git blob IDs are recorded separately.',sourceGitBlobOids,artifactBytesPreservedInGit:true,
  testedWorkingChanges:true,sourceStableBeforeTestsAndAfterBuild:true,
  previousGoalTurn:'progress: all staged/unstaged source saved in ce014ec06, integrated and remotely verified at cc0cbf1a5',
  scope:'Same Gateway process, WebSocket, credential-first Native ResumeSession while the old transport has ended but its real File checkpoint has not finished. No desktop control.',
  behavior:{onlyServerBoundEndedTransportWaits:true,waitOutsideWriterAndStoreLocks:true,issuedAuthRevisionAndSharedIdentityChecked:true,exactFullCredentialBindingAndSavedHandoffTokenComparedUnderReservationLock:true,existingRouteZoneMonthlyCardAndPostCommitIdentityChecksRetained:true,credentialConsumedAtMostOnce:true,terminalCheckedAtFinalCommitLock:true,cancelBeforeConsumeRollsBackExactLease:true,wallClockRefreshedImmediatelyBeforeConsume:true,lockedPurgeAlreadyProtectsMutexExpiry:true,readyWithoutWaitRetainsLegacyCredentialPath:true,noColdFallbackOrCommandReplay:true,nativeClientSourcesAnd14SecondBudgetUnchanged:true,noWireOrPersistentSchemaChange:true},
  tests:{new:15,newStore:13,newRealWebSocket:2,adjacentUnique:103,combinedUnique:names.size,failed:0,ignored:0,fullLibraryExecuted:false,alreadyQualifiedNewCasesSkippedDuringAdjacent:true,testExecutableSha256:regression.sha256,testExecutableUnchangedDuringAdjacent:regression.unchanged,names:[...names].sort()},
  realWebSocket:{actualAxumHandlerAndSharedRuntime:true,ordinaryNewAccountLoginCharacterAndStartGame:true,realFileCheckpointBlockedBeforeLeasePublication:true,handlerSubscriptionObservedWhileSaveBlocked:true,subscriptionCountIsNotClaimedAsFuturePoll:true,ackBeforeActiveSnapshot:true,sameCredentialAfterCancelledSocketSucceeds:true,generationIncrementsOnceAndOldReplayRejected:true,protectedPersonalFieldsAndSelfTransformCompared:true,noInheritedQueuedChat:true,logoutAndAllSocketSessionLeasePermitsReturnToZero:true,testListenersAbortedAndAwaited:true,successfulTempCleanupResolvedInsideTempRoot:true,failedFixturesRetained:true,modelCalls:0,adminCommands:false,nativeGui:false},
  candidate,preservation,
  failedPreparations:[{artifact:'tests-initial.log',reason:'First WS fixture used name instead of packet in the actual packet envelope; ten store cases passed.'},{artifact:'tests-ws-second.log',reason:'Fixture selected an older pre-StartGame world snapshot without a self player.'},{artifact:'tests-pre-clock-fix.log',reason:'Fixture used shared wire id instead of the personal snapshot self id; twelve store cases passed, including expiry under mutex contention.'},{artifact:'tests-pre-consume-clock.log',reason:'Precise post-purge/pre-consume expiry regression failed before the final clock refresh; two WS fixtures still used the wrong self id.'}],
  fixturePreparationCorrections:['Use a diverse synthetic recovery MAC accepted by the existing validator.','Read worldSnapshot.payload; select the first active snapshot after StartGame/sessionResumed.','Use playerObjectId within snapshot entities; selfZoneObjectId is a separate wire identity.'],
  openWork:['Actual Windows window and same-candidate Web/Windows save roundtrip remain deferred.','Cross-Gateway/TCP resume is outside the process-local WebSocket store.','Group shared social budget/failure ACK, trade, Alo Web model settings and cross-client memory, release/cold-cache/size gates and final human acceptance remain.'],
  publicArtifactCredentialScan:true,publicDeploy:false,releaseBuild:false,candidate100:false,humanAccepted:false,artifactPins,
};
await writeFile(dest+'/verification.json',JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({evidence:dest,artifacts:Object.keys(artifactPins).length,newTests:15,combinedUnique:names.size,realWebSocketCases:2,sourcePins:Object.keys(source.pins).length,credentialScan:true,candidate100:false}));
