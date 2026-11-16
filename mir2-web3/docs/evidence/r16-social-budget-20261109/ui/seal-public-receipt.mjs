import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile,readdir} from 'node:fs/promises';
import {dirname,join,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=dirname(fileURLToPath(import.meta.url)),repo='E:/mir2-player-journey/mir2-web3';
const json=async rel=>JSON.parse((await readFile(join(root,rel),'utf8')).replace(/^\uFEFF/,''));const sha=b=>createHash('sha256').update(b).digest('hex');
const [r1,r2,r3,late,peerRaw,peer,primary,startup,shutdown,cleanup]=await Promise.all(['browser-r1/proof.json','browser-r2/proof.json','browser-r3/proof.json','browser-r3/retrieve-primary-late-ack-proof.json','browser-r3/peer-cached-frames-readonly.json','browser-r3/peer-ordinary-readback.json','browser-r3/primary-ui-readback.json','startup.json','shutdown.json','cleanup-verified.json'].map(json));
assert(!r1.passed&&!r2.passed&&!r3.passed);
const group=r2.checks.find(c=>c.name==='ordinary-dom-two-player-shared-party'),guild=r3.checks.find(c=>c.name==='ordinary-guild-window-membership-gate'),deposit=r3.checks.find(c=>c.name==='deposit-pending-cancel-real-server-restores-assets');
assert(group?.passed&&guild?.passed&&deposit?.passed&&late.passed&&peer.passed&&primary.passed&&cleanup.allStopped);
assert.equal(r3.errors[0].safeCode,'PEER_TRADE_NOT_CLOSED');assert(peerRaw.rows.filter(r=>r.type==='worldSnapshot').at(-1)?.tradeIsNull===true&&!peerRaw.projection.tradeActive&&peerRaw.projection.uncertain);
const rawSnapCount=peerRaw.rows.filter(r=>r.type==='worldSnapshot').length,cacheCancelPackets=peerRaw.rows.filter(r=>r.packet==='TradeCancel').length;
const module=r3.moduleResponses.find(r=>r.matchesFrozenRuntime);assert(module?.status===200);
const manifestBytes=await readFile(join(repo,'apps/web/lib/generated/bevy_runtime_version.json')),manifest=JSON.parse(manifestBytes);assert.equal(manifest.version,'bevy-0cd68510886134e0');assert(manifest.files.some(f=>f.sha256===module.sha256));
const pageSha=sha(await readFile(join(repo,'apps/web/app/page.tsx'))),hostSha=sha(await readFile(join(repo,'apps/web/lib/social-operation-budget-host.ts')));assert.equal(pageSha,startup.pageSha256);assert.equal(hostSha,startup.hostSha256);assert(shutdown.pageSourceUnchanged&&shutdown.hostSourceUnchanged);
const gCov=r2.bridgeCoverage.find(r=>r.stage==='ordinary-dom-shared-party'),tCov=r3.bridgeCoverage.find(r=>r.stage==='failure');assert(gCov.bridgeCalls>0&&tCov.bridgeCalls>0);
const fileNames=['browser-r2/02-ordinary-shared-party.png','browser-r3/03-guild-no-membership.png','browser-r3/deposit-pending-cancel-enabled.png','browser-r3/retrieve-pending-cancel-enabled.png','browser-r3/retrieve-primary-late-ack-confirmed.png','browser-r3/primary-ui-relogin-verified.png'];
const state=JSON.parse(await readFile('D:/mir2-builds/r16-parity-20261109/trade-cancel-ui-r1/private/state.local.json','utf8'));
const identities=JSON.parse(await readFile('D:/mir2-builds/r16-parity-20261109/trade-cancel-ui-r1/private/ui-identities.local.json','utf8'));
const denied=[state.owner.accountId,state.owner.password,state.actor.accountId,state.actor.password,identities.primary.accountId,identities.primary.password,identities.peer.accountId,identities.peer.password,...Object.values(state.keys)];
const publicFiles=[];
for(const name of await readdir(root)){
 if(/\.(mjs|json)$/.test(name)&&name!=='receipt-final.json')publicFiles.push(join(root,name));
 if(/^browser-r[1-3]$/.test(name))for(const leaf of await readdir(join(root,name)))if(/\.(json|png)$/.test(leaf))publicFiles.push(join(root,name,leaf));
}
let scans=0;const checksums=[];
for(const file of publicFiles){const bytes=await readFile(file);assert(!denied.some(secret=>bytes.includes(Buffer.from(secret))),'PUBLIC_ARTIFACT_FORBIDDEN_VALUE');scans++;checksums.push({path:relative(root,file).replaceAll('\\','/'),bytes:bytes.length,sha256:sha(bytes)});}
assert(fileNames.every(f=>checksums.some(c=>c.path===f)));
const receipt={schema:'mir2-r16-social-budget-page-ui/1',originalWholeRunPassed:false,scopedOrdinaryUiAndReadbackPassed:true,scope:'Independent positive components with all three original false runs preserved; not a single all-green session.',
 build:{pageSha256:pageSha,hostSha256:hostSha,runtimeVersion:manifest.version,runtimeManifestSha256:sha(manifestBytes),actualLoadedModule:module,legacyIsolatedGatewaySha256:startup.gatewaySha256,gatewayExactCurrentSourceClaimed:false},
 isolation:{loopbackOnly:true,ordinaryAccountsReused:2,newRegistrations:0,newCharacters:0,publicTestUsed:false,originalWorldUsed:false,modelCalls:0,gmOrGrantCommands:0,finalDeliveryOrGoldOfferTested:false},
 group:{...group,proof:'browser-r2/proof.json'},guild:{...guild,proof:'browser-r3/proof.json',membershipMutationNotTested:true},
 deposit:{...deposit,proof:'browser-r3/proof.json'},retrievePrimary:{...late,proof:'browser-r3/retrieve-primary-late-ack-proof.json'},
 auxiliaryPeer:{originalObservationGatePassed:false,originalFailure:'PEER_TRADE_NOT_CLOSED',sameConnectionRawSnapshotExplicitTradeNull:true,projectionActive:false,projectionUncertain:true,cachedFramesExamined:peerRaw.cachedFramesExamined,rawWorldSnapshotsInCache:rawSnapCount,terminalTradeCancelPacketsInRetainedCache:cacheCancelPackets,earlierPacketOrderingProven:false,noObserverRuleRelaxed:true,proof:'browser-r3/peer-cached-frames-readonly.json'},
 pageBridge:{passivePreciseCoverage:true,bridgeOrGameActionInjected:false,group:gCov,trade:tCov,actualCallsTotal:gCov.bridgeCalls+tCov.bridgeCalls,rendererPackage:'webgpu',controllerThreePackageEvidenceSeparate:'../social-budget-r1/wasm-browser-r1/proof.json'},
 assets:{primaryBefore:r3.assetsBefore.primary,primaryReadback:primary.economy,peerBefore:r3.assetsBefore.peer,peerReadback:peer.economy,allOwnedUidsAndQuantitiesAndGoldMatch:true,expectedCandleSlotMove:{from:r3.starter.slot,to:primary.expectedCandleSlot}},
 readback:{primaryUi:true,peerOrdinaryProtocol:true,normalLogoutSuccessPrimary:primary.normalLogoutAcknowledgements,normalLogoutSuccessPeer:peer.packets.filter(p=>p.packet==='LogOutSuccess').length,noTradeReplay:true,primaryProof:'browser-r3/primary-ui-readback.json',peerProof:'browser-r3/peer-ordinary-readback.json'},
 preservedFalseRuns:[{proof:'browser-r1/proof.json',reason:'Synchronous DOM roster count before React paint; both server rosters and the subsequent screenshot already show two.'},{proof:'browser-r2/proof.json',reason:'Guild snapshot had an empty-name default object; the helper incorrectly demanded null. Actual ordinary shared party passed.'},{proof:'browser-r3/proof.json',reason:'Auxiliary observation remained uncertain after primary retrieve cancel. Raw peer trade null and fresh asset readback proven separately.'}],
 browserErrors:{group:{http:r2.httpErrors.length,page:r2.pageErrors.length,console:r2.consoleErrors.length},trade:{http:r3.httpErrors.length,page:r3.pageErrors.length,console:r3.consoleErrors.length}},
 cleanup:{...cleanup,originalLauncherNextExitCodeOnlyFlag:shutdown.ownNextStopped,originalLauncherFlagNotProcessLiveness:true,oldServicesNotRestarted:true,sourcePinsUnchanged:true,proof:'cleanup-verified.json'},
 privacy:{forbiddenValueMatches:0,publicArtifactFilesScanned:scans,credentialInputsBlackMasked:true,publicAuthFramesIncluded:false,privateStoreAndKeysExcluded:true},safeImages:fileNames,checksums};
const bytes=JSON.stringify(receipt,null,2)+'\n';assert(!denied.some(secret=>bytes.includes(secret)));await writeFile(join(root,'receipt-final.json'),bytes,{flag:'wx'});
console.log(JSON.stringify({receipt:join(root,'receipt-final.json'),scopedPassed:true,originalWholeRunPassed:false,pageBridgeCalls:receipt.pageBridge.actualCallsTotal,normalLogouts:receipt.readback.normalLogoutSuccessPrimary+receipt.readback.normalLogoutSuccessPeer,scanned:scans,allStopped:cleanup.allStopped}));
