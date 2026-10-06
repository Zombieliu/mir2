import fs from 'node:fs';
import path from 'node:path';
import cp from 'node:child_process';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
const directory=path.dirname(fileURLToPath(import.meta.url)),mode=process.argv[2]||'working';
assert(['working','index','HEAD'].includes(mode));
const git=args=>cp.execFileSync('git',args,{cwd:directory,maxBuffer:32*1024*1024});
const repo=git(['rev-parse','--show-toplevel']).toString().trim();
const qa=path.relative(repo,directory).split(path.sep).join('/');
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const read=relative=>{assert(!relative.includes('..')&&!path.isAbsolute(relative));return mode==='working'?fs.readFileSync(path.join(repo,qa,relative)):git(['show',mode==='index'?':'+qa+'/'+relative:'HEAD:'+qa+'/'+relative]);};
const json=name=>JSON.parse(read(name).toString()),raw=name=>json('raw/'+name);
const d=json('diagnosis.json'),m=json('manifest.json'),b=raw('source-commit-binding.json');
assert.equal(d.source,b.source);assert.equal(d.source,m.source);
assert.equal(m.payloadCount,278);assert.equal(m.payloads.length,278);
assert.equal(m.totalBytes,16957314);assert.equal(m.totalBytes,m.payloads.reduce((n,v)=>n+v.bytes,0));
for(const row of m.payloads){const bytes=read(row.path);assert.equal(bytes.length,row.bytes,row.path);assert.equal(sha(bytes),row.sha256,row.path);assert(!/\.(apk|aab|so|jks|keystore|pem|p12)$/i.test(row.path));}
assert.equal(d.goalStatus,'Active');assert.equal(d.fullNI12Status,'PARTIAL');assert.equal(d.fullNI13Status,'PARTIAL');
assert.equal(d.version,34);assert.equal(d.versionName,'0.1.31-gameshop-page-cache');
assert.equal(d.frozenWindows,'3d735745f1117d42a7859e87604a106351dca935');
git(['merge-base','--is-ancestor',d.frozenWindows,d.source]);
assert.deepEqual(git(['diff-tree','--no-commit-id','--name-only','-r',d.source]).toString().trim().split('\n').sort(),[...d.scope].sort());
assert.deepEqual(b.sourceScope,d.scope);assert.equal(b.parent,d.previousPublishedAndroid);
assert.equal(b.inputs.length,49);assert.equal(d.inputCount,49);
for(const row of b.inputs){const bytes=git(['show',d.source+':'+row.path]);assert.equal(bytes.length,row.bytes);assert.equal(sha(bytes),row.sha256);}
for(const file of b.protectedPathsUnchanged)assert.equal(sha(git(['show',d.source+':'+file])),sha(git(['show',b.parent+':'+file])),file);
for(const file of ['mir2-web3/apps/game-client/client-bevy/src/crystal_ui/game_shop_dialog.rs','mir2-web3/apps/game-client/client-bevy/src/crystal_ui/overlays.rs']){
 const before=git(['show',b.parent+':'+file]).toString(),after=git(['show',d.source+':'+file]).toString();
 assert.equal(before.split('\nmod tests {')[1],after.split('\nmod tests {')[1],'existing tests unchanged');
 if(file.endsWith('game_shop_dialog.rs'))assert.equal(before.split('\n#[cfg(test)]\n')[0],after.split('\n#[cfg(test)]\n')[0],'dialog production unchanged');
}
const red=raw('working-game-shop-page-cache-corrected-red-v34-result.json'),green=raw('working-game-shop-page-cache-corrected-green-v34-result.json');
assert.equal(red.code,101);assert.equal(red.sourceUnchanged,true);assert.equal(green.code,0);assert.equal(green.sourceUnchanged,true);
assert(read('raw/working-game-shop-page-cache-corrected-red-v34.log').toString().includes('5 passed; 2 failed;'));
assert(read('raw/working-game-shop-page-cache-corrected-green-v34.log').toString().includes('7 passed; 0 failed;'));
for(const row of green.sourceBefore){assert.equal(row.sha256,b.inputs.find(r=>r.path===row.path).sha256);}
const rejected=raw('working-game-shop-hit-compiled-red-v34-result.json');
assert.equal(rejected.code,0);assert.equal(d.hypotheses[0].status,'REJECTED');assert.equal(d.hypotheses[1].status,'CONFIRMED');
assert(read('raw/working-game-shop-hit-red-v34.log').toString().includes('ui_stack_system'));
assert(read('raw/working-game-shop-page-cache-green-v34.log').toString().includes('[2000, 2001, 2010, 2100, 2101, 2102, 2103, 2104]'));
assert.equal(d.gates.length,7);
for(const label of d.gates){
 const r=raw(label+'-result.json'),log=read('raw/'+label+'.log').toString();
 assert.equal(r.source,d.source);assert.equal(r.code,0);assert.equal(r.sourceUnchanged,true);
 assert.deepEqual(r.sourceBefore,b.inputs);assert.deepEqual(r.sourceAfter,b.inputs);
 let counts;
 if(label.startsWith('bound-rust-normal-'))counts=[359,0];
 if(label.startsWith('bound-rust-preview-'))counts=[387,0];
 if(label.startsWith('bound-shared-native-player-'))counts=[1267,10];
 if(label.startsWith('bound-runtime-'))counts=[292,1];
 if(counts)assert(log.includes('test result: ok. '+counts[0]+' passed; 0 failed; '+counts[1]+' ignored;'));
 if(label.startsWith('bound-api31-')){assert(r.cwd.endsWith('/platform-android'));assert.deepEqual(r.args.slice(0,7),['+1.95.0','ndk','-t','arm64-v8a','--platform','31','check']);assert(log.includes('Finished'));}
 if(label.startsWith('bound-java-')){assert(r.args.includes('--no-build-cache')&&r.args.includes('--rerun-tasks'));assert(log.includes('BUILD SUCCESSFUL'));assert.equal(r.xml.length,12);}
}
const java=raw('java-results.json');assert.equal(java.length,2);
for(let variant=0;variant<2;variant++){const j=java[variant];assert.equal(j.tests,52);assert.equal(j.failures+j.errors+j.ignored,0);assert.equal(j.suites.length,6);
 for(const suite of j.suites){const xml=read('raw/bound-java-full-v34-xml-'+variant+'/'+suite.name).toString(),head=xml.match(/<testsuite\b[^>]*>/)[0];
  const value=key=>Number(head.match(new RegExp(key+'="(\\d+)"'))?.[1]??0);
  assert.equal(value('tests'),suite.tests);assert.equal(value('failures')+value('errors')+value('skipped'),0);
 }}
const audited=raw('source-and-package-baseline.json'),packages=raw('package-baseline.json'),installed=raw('install-baseline.json');
for(const r of [audited,packages,installed])assert.equal(r.source,d.source);
assert.deepEqual(audited.inputs,b.inputs);assert.equal(audited.selectedResourceCountEach,6647);assert.equal(audited.metadataCountEach,3);
assert.equal(packages.versionCode,34);assert.equal(installed.versionCode,34);assert.equal(packages.gatewayUrl,'');assert.equal(packages.artifacts.length,2);
assert.deepEqual(installed.artifacts,d.artifacts);
for(const artifact of packages.artifacts){const a=audited.artifacts.find(x=>x.variant===artifact.variant),i=installed.artifacts.find(x=>x.variant===artifact.variant);assert.equal(a.sha256,artifact.sha256);assert.equal(i.installedSha256,artifact.sha256);assert.equal(a.nativeElfMatchesThisVariantStrippedOutput,true);assert.equal(a.personalJniDiagnosticCompiled,artifact.variant==='preview');
 const config=read('raw/'+artifact.variant+'-BuildConfig.java.txt').toString();assert(config.includes('VERSION_CODE = 34'));assert(config.includes('VERSION_NAME = "0.1.31-gameshop-page-cache"'));assert(config.includes('MIR2_GATEWAY_URL = ""'));assert(config.includes('UI_PREVIEW = '+(artifact.variant==='preview')));
 assert.match(read('raw/'+artifact.variant+'-badging.txt').toString(),/versionCode='34'/);
 const before=read('raw/'+artifact.variant+'-package-before.txt').toString(),after=read('raw/'+artifact.variant+'-package-after.txt').toString();
 assert.match(before,/versionCode=33\b/);assert.match(after,/versionCode=34\b/);
 assert.equal(before.match(/firstInstallTime=(.*)/)[1],after.match(/firstInstallTime=(.*)/)[1]);assert.equal(i.upgradePreservedDataFlag,true);
}
assert.deepEqual(raw('original-worktrees-before.json'),raw('original-worktrees-after.json'));
const published=raw('source-publication-verified.json');assert.equal(published.localAndRemoteExact,true);assert.equal(published.source,d.source);assert.equal(published.pr.headSha,d.source);assert.equal(published.pr.draft,true);assert.equal(published.windowsRemote,'6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4');
assert.equal(d.screenshots.length,21);
for(const shot of d.screenshots){assert.equal(shot.inspectedOriginal,true);const png=read(shot.path);assert.equal(png.subarray(0,8).toString('hex'),'89504e470d0a1a0a');assert.equal(png.readUInt32BE(16),2340);assert.equal(png.readUInt32BE(20),1080);}
assert(read('raw/device-list.txt').toString().includes('emulator-5554'));assert.equal(read('raw/device-api.txt').toString().trim(),'31');
const pidCounts=[];
for(const scene of d.scenes){const c=raw('ui-'+scene.scene+'/commands.json'),log=read('raw/ui-'+scene.scene+'/'+c.latestLog).toString();
 assert.equal(c.source,d.source);assert.equal(c.versionCode,34);assert.equal(c.pid,scene.pid);
 assert.equal(c.latestTotals.gl506,scene.totals.gl506);assert.equal(c.latestTotals.fatalOrPanic,0);
 const javaStarts=(log.match(/I Mir2UiPreview: PERSONAL_JNI_OFFLINE_START/g)||[]).length,sends=(log.match(/I Mir2UiPreview: PERSONAL_JNI_OFFLINE_SENT/g)||[]).length;
 assert.equal(javaStarts,scene.scene==='normal-intent-isolation'?0:1);assert.equal(sends,javaStarts);
 if(scene.scene==='normal-intent-isolation'){assert(!log.includes('ANDROID_UI_PREVIEW_READY'));assert(!log.includes('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE'));assert.equal(c.javaProducerSent,false);}
 else{assert(c.commands.some(x=>x.args?.includes('KEYCODE_HOME')));assert.equal(c.sharedConsumerObserved,true);assert(log.includes('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE'));}
 for(const command of c.commands){assert.equal(command.exitCode,0);if(command.args?.[0]==='shell'&&command.args?.[1]==='input')assert(['tap','keyevent'].includes(command.args[2]));}
 pidCounts.push(scene.pid);
}
assert.equal(new Set(pidCounts).size,3);assert.equal(d.totalDistinctPidGl506,17);assert.equal(d.observedFatalOrPanic,0);
for(const flag of ['newNativeApksBuilt','actualOfflineJavaJniVerified','actualShopPagingVerified','shopFinalPageBoundaryVerified','shopPreviousAndSamePidResumeVerified','storagePageAndSamePidResumeRegressionVerified','normalIntentIsolationVerified'])assert.equal(d[flag],true,flag);
for(const flag of ['wholePhoneUiAccepted','phoneShop48dpTargetsAccepted','storageInventoryLayoutAccepted','realAuthenticationAccepted','realNetworkAccepted','purchaseAccepted','storageTransferAccepted','physicalDeviceAccepted','rendererFixAccepted','zeroRendererErrorAccepted','fullResourcePackAccepted','windowsHostBuildAccepted','appDataContentHashed','untrackedRecursivelyHashed','wholeGoalComplete'])assert.equal(d[flag],false,flag);
console.log(JSON.stringify({mode,source:d.source,payloads:m.payloads.length,bytes:m.totalBytes,inputs:49,screenshots:21,gates:7,goalStatus:'Active',wholeGoalComplete:false}));
