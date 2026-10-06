import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join,dirname,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const root=dirname(fileURLToPath(import.meta.url));
const repo=execFileSync('git',['rev-parse','--show-toplevel'],{cwd:root,encoding:'utf8'}).trim();
const mode=process.argv[2]??'--head';
assert(['--head','--index','--working'].includes(mode));
const read=p=>readFileSync(join(root,p)),json=p=>JSON.parse(read(p));
const sha=b=>createHash('sha256').update(b).digest('hex');
const git=(rev,path)=>execFileSync('git',['show',rev+':'+path],{cwd:repo,maxBuffer:32*1024*1024});
const manifest=json('manifest.json'),p=json('raw/builds/package-baseline.json'),audit=json('raw/builds/source-and-package-baseline.json'),d=json('diagnosis.json');
assert.equal(p.source,'2161a940ec7a59e966ed2638667f0c7b1a8e32b6');
assert.equal(manifest.source,p.source);assert.equal(d.source,p.source);
assert.equal(p.versionCode,29);assert.equal(p.versionName,'0.1.26-phone-quest-modal');
assert.equal(p.gatewayUrl,'');assert.equal(p.diagnosticProbesRemoved,true);assert.equal(p.rendererFixAccepted,false);
for(const f of manifest.files){
 assert.match(f.path,/^(raw\/[A-Za-z0-9_.\/-]+|diagnosis\.json)$/);
 assert(!f.path.split('/').includes('..'));
 assert(!/\.(apk|so|jks|keystore)$/.test(f.path));
 const bytes=read(f.path);assert.equal(bytes.length,f.bytes);assert.equal(sha(bytes),f.sha256);
 if(mode!=='--working')assert.equal(sha(git(mode==='--head'?'HEAD':'',relative(repo,root)+'/'+f.path)),f.sha256);
}
assert.equal(manifest.files.length,259);assert.equal(p.sourceFiles.length,41);
for(const f of p.sourceFiles)assert.equal(sha(git(p.source,f.path)),f.sha256);
execFileSync('git',['merge-base','--is-ancestor',audit.frozenWindows,p.source],{cwd:repo});
assert.equal(audit.frozenWindows,'3d735745f1117d42a7859e87604a106351dca935');
assert.equal(d.sourceWriteSet.length,5);
const scope=execFileSync('git',['diff-tree','--no-commit-id','--name-only','-r',p.source],{cwd:repo,encoding:'utf8'}).trim().split('\n');
assert.deepEqual(d.sourceWriteSet,scope);
for(const path of ['mir2-web3/third_party/bevy_render/src/view/window/mod.rs','mir2-web3/third_party/bevy_render/src/renderer/mod.rs'])assert(git(p.source,path).equals(git('277ab0a6566b5241c418844e5366865547cd03e0',path)));
const gates=json('raw/baseline-source-3/gate-commands.json');assert.equal(gates.length,6);
for(const g of gates){
 assert.equal(g.exitCode,0);assert.equal(g.sourceUnchanged,true);
 for(const f of p.sourceFiles){assert.equal(g.sourceHashesBefore[f.path],f.sha256);assert.equal(g.sourceHashesAfter[f.path],f.sha256);}
}
for(const [name,passed,ignored] of [['android-normal-final',327,0],['android-preview-final',351,0],['shared-bevy-final',1247,10]])assert.match(read('raw/baseline-source-3/'+name+'.log').toString(),new RegExp('test result: ok\\. '+passed+' passed; 0 failed; '+ignored+' ignored;'));
const shared=read('raw/baseline-source-3/shared-bevy-final.log').toString();
assert.equal(shared.split('\n').filter(l=>l.startsWith('test quest_ui::phone::tests::')&&l.endsWith(' ... ok')).length,17);
for(const variant of ['Debug','UiPreview']){
 const xmls=manifest.files.filter(f=>f.path.startsWith('raw/baseline-source-3/java-xml/'+variant+'/'));assert.equal(xmls.length,5);
 let tests=0;
 for(const f of xmls){
  const head=read(f.path).toString().match(/<testsuite [^>]+>/)?.[0];assert(head);
  tests+=Number(head.match(/ tests="(\d+)"/)[1]);
  for(const name of ['skipped','failures','errors'])assert.match(head,new RegExp(' '+name+'="0"'));
 }
 assert.equal(tests,41);
}
for(const phase of ['debug','preview']){
 assert.equal(read('raw/builds/'+phase+'-source-before.txt').toString().trim(),p.source);
 assert.equal(read('raw/builds/'+phase+'-source-after.txt').toString().trim(),p.source);
 assert.equal(read('raw/builds/'+phase+'-status-before.txt').length,0);
 assert.equal(read('raw/builds/'+phase+'-status-after.txt').length,0);
 assert.match(read('raw/builds/'+phase+'-badging.txt').toString(),/versionCode='29'.*versionName='0.1.26-phone-quest-modal'/);
 assert.match(read('raw/builds/'+phase+'-BuildConfig.java.txt').toString(),/GATEWAY_URL = ""/);
}
assert.equal(audit.artifacts.length,2);assert.equal(audit.selectedResourceCountEach,6647);assert.equal(audit.metadataCountEach,3);
for(const name of ['rendererFixAccepted','fullResourcePackAccepted','wholePhoneUiAccepted','realNetworkAccepted','physicalDeviceAccepted'])assert.equal(audit[name],false);
const install=json('raw/builds/installation.json');
assert.equal(install.source,p.source);assert.equal(install.versionCode,29);assert.equal(install.oldVersionCode,28);
assert.equal(install.dataPreserving,true);assert(install.commands.every(c=>c.exitCode===0));
for(const a of p.artifacts){
 assert.equal(a.source,p.source);
 const audited=audit.artifacts.find(x=>x.variant===a.variant);assert.equal(audited.sha256,a.sha256);assert.equal(audited.bytes,a.bytes);
 assert.equal(read('raw/builds/install-'+a.variant+'-sha-after.txt').toString().trim().split(/\s+/)[0],a.sha256);
 assert.match(read('raw/builds/install-'+a.variant+'-result.txt').toString(),/Success/);
 assert.match(read('raw/builds/install-'+a.variant+'-package.txt').toString(),/versionCode=29\b/);
 const command=install.commands.find(c=>c.name==='install-'+a.variant+'-result.txt');assert.deepEqual(command.args.slice(0,3),['install','-r','-t']);
}
const counts=log=>({gl506:(log.match(/E emuglGLESv2_enc:.*GL error 0x506/g)??[]).length,uninitializedColor:(log.match(/D eglCodecCommon:.*rbo not color renderable\. format: 0x0/g)??[]).length,fatalOrPanic:(log.match(/FATAL EXCEPTION|thread '.*' panicked|Fatal signal/g)??[]).length});
const seen=new Set();let canonical=0;
const baseline=json('raw/baseline-v29/commands.json');
assert.equal(baseline.installedSource,p.source);assert.equal(baseline.versionCode,29);
assert.equal(baseline.actualDevice,'emulator-5554');assert.equal(baseline.offlineOnly,true);
assert.equal(baseline.realNetworkAccepted,false);assert.equal(baseline.physicalDeviceAccepted,false);
assert(baseline.commands.every(c=>c.exitCode===0));
for(const a of p.artifacts)for(const moment of ['before','after']){
 assert.equal(read('raw/baseline-v29/'+a.variant+'-installed-sha-'+moment+'.txt').toString().trim().split(/\s+/)[0],a.sha256);
 assert.match(read('raw/baseline-v29/'+a.variant+'-package-'+moment+'.txt').toString(),/versionCode=29\b/);
}
for(const s of baseline.scenes){
 assert(!seen.has(s.pid));seen.add(s.pid);assert.equal(s.sameProcessResume,true);
 assert.equal(read('raw/baseline-v29/'+s.name+'-pid.txt').toString().trim(),s.pid);
 assert.equal(read('raw/baseline-v29/'+s.name+'-resume-pid.txt').toString().trim(),s.pid);
 assert.deepEqual(counts(read('raw/baseline-v29/'+s.name+'-cold-pid.log').toString()),s.cold);
 assert.deepEqual(s.latestCumulative,s.cold);
}
for(const scene of ['quest-confirmation','quest-alert']){
 const dir='raw/ui-'+scene,c=json(dir+'/commands.json');
 assert.equal(c.source,p.source);assert.equal(c.versionCode,29);
 assert.equal(c.actualDevice,'emulator-5554');assert.equal(c.offlineOnly,true);
 assert.equal(c.realNetworkAccepted,false);assert.equal(c.physicalDeviceAccepted,false);
 assert(c.commands.every(x=>x.exitCode===0));assert(!seen.has(c.pid));seen.add(c.pid);
 for(const x of c.commands){
  if(x.name.endsWith('-installed-sha.txt'))assert.equal(read(dir+'/'+x.name).toString().trim().split(/\s+/)[0],p.artifacts.find(a=>a.variant==='preview').sha256);
  if(/-pid-(before|after)\.txt$/.test(x.name)||x.name==='pid.txt')assert.equal(read(dir+'/'+x.name).toString().trim(),c.pid);
 }
 const cold=read(dir+'/0-cold-pid.log').toString();assert(cold.includes('ANDROID_QUEST_MODAL_MANUAL_UI_ONLY_NOT_LIVE'));
 const event=cold.split('\n').filter(l=>/ I event /.test(l));
 const controls=event.filter(l=>l.includes('ANDROID_PHONE_QUEST_CONTROL')).map(l=>l.match(/size_dp = Vec2\(([^)]+)\); center_dp = Vec2\(([^)]+)\)/)).filter(Boolean).map(m=>({size:m[1].split(',').map(Number),center:m[2].split(',').map(Number)})).filter(n=>n.size[0]>190);
 assert.equal(controls.length,scene==='quest-confirmation'?4:3);assert(controls.every(n=>n.size[1]>=47.99));
 const texts=event.filter(l=>l.includes('ANDROID_PHONE_QUEST_TEXT'));
 assert(texts.every(l=>Number(l.match(/font_dp = ([\d.]+)/)?.[1])>=13.99));
 assert(texts.some(l=>/font_dp = 16.0/.test(l)));
 assert(c.commands.some(x=>x.args.includes('KEYCODE_HOME')));
 assert.equal(c.commands.filter(x=>x.args.includes('swipe')).length,scene==='quest-alert'?1:0);
 const taps=c.commands.filter(x=>x.args.includes('tap')).map(x=>x.args.slice(-2));
 assert.deepEqual(taps,scene==='quest-confirmation'?[['1620','946']]:[['1620','795'],['1085','795'],['1320','946']]);
}
for(const capture of d.actualCaptures){
 const log=read('raw/'+capture.phase+'/'+capture.latestLog).toString();
 assert.deepEqual(counts(log),capture.totals);assert.equal(capture.totals.fatalOrPanic,0);
 assert(!log.includes('MIR2_ANDROID_SURFACE_')&&!log.includes('MIR2_ANDROID_VIEW_OUTPUT_PROBE')&&!log.includes('MIR2_ANDROID_ATTACHMENT_TRANSITION_PROBE'));
 canonical+=capture.totals.gl506;
}
assert.equal(seen.size,5);assert.equal(canonical,17);assert.equal(baseline.canonicalTotal,17);
assert.equal(baseline.zeroErrorAccepted,false);assert.equal(d.canonicalDistinctPidGl506,17);
assert.equal(d.originalFrames.length,16);
for(const frame of d.originalFrames){
 const b=read(frame.path);assert.equal(sha(b),frame.sha256);
 assert.equal(b.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
 assert.equal(b.readUInt32BE(16),2340);assert.equal(b.readUInt32BE(20),1080);
 assert.equal(frame.primaryInspectedOriginal,true);assert.equal(frame.editedOrResized,false);assert(seen.has(frame.pid));
}
for(const f of d.failuresFirst){
 assert.equal(sha(read(f.input)),f.inputSha256);
 assert.match(read(f.red).toString(),new RegExp('test result: FAILED\\. 0 passed; '+f.compiled.failed+' failed;'));
}
assert.match(read('raw/builds/modal-green-1.log').toString(),/test result: ok\. 3 passed; 0 failed;/);
const replay=json('raw/builds/message-replay-red.json');assert.equal(replay.exitCode,101);
assert.equal(replay.phoneSourceSha256,d.failuresFirst[1].inputSha256);
assert.match(shared,/test quest_ui::phone::tests::phone_new_alert_does_not_inherit_the_previous_messages_scroll_position ... ok/);
const helper=json('raw/builds/audit-version-check-failure.json');
assert.equal(helper.productFailure,false);assert.equal(helper.actual,"versionCode='29'");
const old=json('raw/builds/retained-setup-errors.json');assert.match(old.messageSettledRed,/not accepted/);assert.match(old.modalSafety1,/corrected expectation/);
assert.equal(d.rendererFixed,false);assert.equal(d.rendererZeroErrorAccepted,false);
for(const name of ['wholePhoneUiAccepted','fullResourcePackAccepted','realNetworkAccepted','physicalDeviceAccepted'])assert.equal(d[name],false);
assert.equal(d.manualUiSpecimensNotAuthenticatedPackets,true);assert.equal(d.noAbandonYesTap,true);
assert.equal(d.goalStatus,'active');assert.equal(d.graphicsDependencyDecision,'pending user authorization');
const originals=json('raw/builds/original-checkouts.json');assert.equal(originals.fullDirectoryContentAudit,false);
assert(originals.rows.every(r=>r.headBranchStatusMatchesPrior&&r.originalHeadRetained));
console.log(JSON.stringify({mode,integrityFiles:manifest.files.length,source:p.source,committedInputs:41,originalFrames:16,distinctPids:5,canonicalGl506:17,sixSourceGates:'PASS',boundedPhoneModal:'PASS offline only',rendererGate:'FAIL',goal:'active'}));
