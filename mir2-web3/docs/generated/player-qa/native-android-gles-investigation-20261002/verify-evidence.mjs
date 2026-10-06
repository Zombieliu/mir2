import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join,dirname,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';

const root=dirname(fileURLToPath(import.meta.url));
const repo=execFileSync('git',['rev-parse','--show-toplevel'],{cwd:root,encoding:'utf8'}).trim();
const read=p=>readFileSync(join(root,p));
const json=p=>JSON.parse(read(p));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=(source,path)=>execFileSync('git',['show',source+':'+path],{cwd:repo,maxBuffer:8*1024*1024});
const manifest=json('manifest.json');
for(const f of manifest.files){
  assert.match(f.path,/^(raw\/[A-Za-z0-9_.\/-]+|diagnosis\.json)$/);
  assert(!f.path.split('/').includes('..'));
  const bytes=read(f.path);
  assert.equal(bytes.length,f.bytes);
  assert.equal(sha(bytes),f.sha256);
  assert.equal(sha(git('HEAD',relative(repo,root)+'/'+f.path)),f.sha256);
}
const versions=[
  ['baseline','package-v24.json','source-v24.json',24,22],
  ['probe','package-probe.json','package-probe.json',25,29],
  ['probe-view','package-view-probe.json','package-view-probe.json',26,25],
  ['probe-transition','package-transition-probe.json','package-transition-probe.json',27,8],
  ['baseline-clean','package-baseline.json','package-baseline.json',28,24]
];
const seenPids=new Set(), frames=[];
for(const [phase,packageFile,sourceFile,version,total] of versions){
  const p=json('raw/builds/'+packageFile),s=json('raw/builds/'+sourceFile);
  assert.equal(p.versionCode,version);
  assert.equal(p.source,s.source);
  assert(s.sourceFiles.length>=39);
  for(const f of s.sourceFiles)assert.equal(sha(git(s.source,f.path)),f.sha256);
  const c=json('raw/'+phase+'/commands.json');
  assert.equal(c.installedSource,p.source); assert.equal(c.versionCode,version);
  assert.equal(c.actualDevice,'emulator-5554');assert.equal(c.offlineOnly,true);
  assert.equal(c.realNetworkAccepted,false);assert.equal(c.physicalDeviceAccepted,false);
  assert(c.commands.every(command=>command.exitCode===0));
  for(const a of p.artifacts)for(const moment of ['before','after']){
    const text=read('raw/'+phase+'/'+a.variant+'-installed-sha-'+moment+'.txt').toString();
    assert.equal(text.trim().split(/\s+/)[0],a.sha256);
    const pkg=read('raw/'+phase+'/'+a.variant+'-package-'+moment+'.txt').toString();
    assert.match(pkg,new RegExp('versionCode='+version+'\\b'));
  }
  let computed=0;
  for(const scene of c.scenes){
    assert(!seenPids.has(scene.pid));seenPids.add(scene.pid);
    assert.equal(scene.sameProcessResume,true);
    assert.equal(read('raw/'+phase+'/'+scene.name+'-pid.txt').toString().trim(),scene.pid);
    assert.equal(read('raw/'+phase+'/'+scene.name+'-resume-pid.txt').toString().trim(),scene.pid);
    for(const suffix of ['cold','latest']){
      const log=read('raw/'+phase+'/'+scene.name+'-'+suffix+'-pid.log').toString();
      const gl=(log.match(/E emuglGLESv2_enc:.*GL error 0x506/g)??[]).length;
      const counts=suffix==='cold'?scene.cold:scene.latestCumulative;
      assert.equal(gl,counts.gl506);
      assert.equal((log.match(/D eglCodecCommon:.*rbo not color renderable\. format: 0x0/g)??[]).length,counts.uninitializedColor);
      assert.equal((log.match(/FATAL EXCEPTION|thread '.*' panicked|Fatal signal/g)??[]).length,counts.fatalOrPanic);
      assert.equal(counts.fatalOrPanic,0);
      if(suffix==='latest')computed+=gl;
      if(version===28)assert(!log.includes('MIR2_ANDROID_SURFACE_')&&!log.includes('MIR2_ANDROID_VIEW_OUTPUT_PROBE')&&!log.includes('MIR2_ANDROID_ATTACHMENT_TRANSITION_PROBE'));
    }
    assert.equal(scene.latestCumulative.gl506,scene.cold.gl506);
    for(const suffix of ['cold','resumed']){
      const path='raw/'+phase+'/'+scene.name+'-'+suffix+'.png',bytes=read(path);
      assert.equal(bytes.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
      assert.equal(bytes.readUInt32BE(16),2340);assert.equal(bytes.readUInt32BE(20),1080);
      frames.push(path);
    }
  }
  assert.equal(computed,total);assert.equal(c.canonicalTotal,total);assert.equal(c.zeroErrorAccepted,false);
}
assert.equal(frames.length,24);assert.equal(seenPids.size,12);
for(const scene of ['world','quests']){
  const surface=read('raw/probe/'+scene+'-latest-pid.log').toString().split('\n').filter(l=>/ I event /.test(l));
  const attachments=surface.filter(l=>l.includes('MIR2_ANDROID_SURFACE_ATTACHMENT_PROBE'));
  assert.equal(attachments.length,129);
  assert.equal(surface.filter(l=>l.includes('MIR2_ANDROID_SURFACE_CONFIG_PROBE')).length,3);
  assert(attachments.every(l=>/live=true internal=0x8058.*samples=0 expected=Rgba8Unorm/.test(l)));
  const views=read('raw/probe-view/'+scene+'-latest-pid.log').toString().split('\n').filter(l=>/ I event /.test(l)&&l.includes('MIR2_ANDROID_VIEW_OUTPUT_PROBE'));
  assert.equal(views.length,130);
  const mismatch=views.filter(l=>l.match(/output=Some.*NativeRenderbuffer\((\d+)\)/)?.[1]!==l.match(/current_surface_raw=(\d+)/)?.[1]);
  assert.equal(mismatch.length,3);
  assert(mismatch.every(l=>l.includes('has_camera=false')&&l.includes('has_view=false')));
  const transitions=read('raw/probe-transition/'+scene+'-latest-pid.log').toString().split('\n').filter(l=>/ I event /.test(l)&&l.includes('MIR2_ANDROID_ATTACHMENT_TRANSITION_PROBE'));
  for(const [stage,status,type] of [['fresh_rbo','0x8cd5','0x8d41'],['replace_with_texture','0x8cd5','0x1702'],['delete_unbound_old_rbo','0x8cd6','0x1702'],['explicit_type_detach','0x8cd5','0x1702']]){
    const found=transitions.filter(l=>l.includes('stage='+stage+' '));
    assert.equal(found.length,1);assert(found[0].includes('status='+status+' actual_type='+type));
  }
  assert.equal(transitions.filter(l=>l.includes('end restored_bindings=true')).length,1);
}
const p=json('raw/builds/package-baseline.json'),audit=json('raw/builds/source-and-package-baseline.json');
assert.equal(p.source,'84fe8e6344bb0c041e01c66a149b5b42b8198565');
assert.equal(p.sourceFiles.length,41);assert.equal(p.gatewayUrl,'');
assert.equal(p.diagnosticProbesRemoved,true);assert.equal(p.rendererFixAccepted,false);
execFileSync('git',['merge-base','--is-ancestor',audit.frozenWindows,p.source],{cwd:repo});
for(const path of ['mir2-web3/third_party/bevy_render/src/view/window/mod.rs','mir2-web3/third_party/bevy_render/src/renderer/mod.rs']){
  assert(git(p.source,path).equals(git('277ab0a6566b5241c418844e5366865547cd03e0',path)));
}
const gates=json('raw/baseline-source/gate-commands.json');
assert.equal(gates.length,6);
for(const g of gates){
  assert.equal(g.exitCode,0);assert.equal(g.sourceUnchanged,true);
  for(const f of p.sourceFiles){
    assert.equal(g.sourceHashesBefore[f.path],f.sha256);assert.equal(g.sourceHashesAfter[f.path],f.sha256);
  }
}
for(const [name,passed,ignored] of [['android-normal-final',327,0],['android-preview-final',350,0],['shared-bevy-final',1238,10]]){
  assert.match(read('raw/baseline-source/'+name+'.log').toString(),new RegExp('test result: ok\\. '+passed+' passed; 0 failed; '+ignored+' ignored;'));
}
for(const variant of ['Debug','UiPreview']){
  const xmls=manifest.files.filter(f=>f.path.startsWith('raw/baseline-source/java-xml/'+variant+'/'));
  assert.equal(xmls.length,5);let tests=0;
  for(const f of xmls){
    const head=read(f.path).toString().match(/<testsuite [^>]+>/)?.[0];assert(head);
    tests+=Number(head.match(/ tests="(\d+)"/)[1]);
    for(const name of ['skipped','failures','errors'])assert.match(head,new RegExp(' '+name+'="0"'));
  }
  assert.equal(tests,41);
}
assert.equal(audit.artifacts.length,2);assert.equal(audit.selectedResourceCountEach,6647);assert.equal(audit.metadataCountEach,3);
for(const name of ['rendererFixAccepted','fullResourcePackAccepted','wholePhoneUiAccepted','realNetworkAccepted','physicalDeviceAccepted'])assert.equal(audit[name],false);
assert.equal(json('raw/builds/view-probe-check-1.json').exitCode,101);
assert.equal(json('raw/builds/view-probe-check-2.json').exitCode,0);
assert.match(read('raw/builds/view-probe-check-1.log').toString(),/E0425/);
assert.match(read('raw/builds/baseline-script-creation-failure.txt').toString(),/parse error near/);
const delta=json('raw/builds/windows-live-delta.json'),frozen=json('raw/builds/windows-ref-comparison.json');
assert.equal(delta.status,'behind');assert.equal(delta.behindBy,33);
assert.equal(frozen.aheadBy,32);assert.equal(frozen.returnedFileCount,300);assert.equal(frozen.fileListMayBeCapped,true);
assert.equal(frozen.didChangeFrozenGoal,false);assert.equal(frozen.didResetOrWriteWindows,false);
const diagnosis=json('diagnosis.json');
assert.equal(diagnosis.rendererFixed,false);assert.equal(diagnosis.goalStatus,'active');
assert.equal(diagnosis.graphicsDependencyDecision,'pending user authorization');
assert.equal(diagnosis.allOriginalFramesInspected,true);
console.log(JSON.stringify({rawIntegrityFiles:manifest.files.length,committedInputs:p.sourceFiles.length,originalFrames:frames.length,distinctPids:seenPids.size,sixSourceGates:'PASS',currentRendererGate:'FAIL: 24 GL0x506',goal:'active'}));
