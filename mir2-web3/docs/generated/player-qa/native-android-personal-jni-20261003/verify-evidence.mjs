import fs from 'node:fs';
import path from 'node:path';
import cp from 'node:child_process';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';

const directory=path.dirname(fileURLToPath(import.meta.url));
const mode=process.argv[2]||'working';
assert(['working','index','HEAD'].includes(mode));
const git=args=>cp.execFileSync('git',args,{cwd:directory,maxBuffer:32*1024*1024});
const repo=git(['rev-parse','--show-toplevel']).toString().trim();
const qa=path.relative(repo,directory).split(path.sep).join('/');
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
function read(relative) {
    assert(!relative.includes('..')&&!path.isAbsolute(relative));
    return mode==='working'?fs.readFileSync(path.join(repo,qa,relative)):
        git(['show',mode==='index'?':'+qa+'/'+relative:'HEAD:'+qa+'/'+relative]);
}
const json=name=>JSON.parse(read(name).toString());
const manifest=json('manifest.json'),diagnosis=json('diagnosis.json');
for(const row of manifest.payloads) {
    const bytes=read(row.path);
    assert.equal(bytes.length,row.bytes,row.path);
    assert.equal(sha(bytes),row.sha256,row.path);
    assert(!/\.(apk|aab|so|jks|keystore|pem|p12)$/i.test(row.path));
}
assert.equal(manifest.totalBytes,manifest.payloads.reduce((sum,row)=>sum+row.bytes,0));
assert.equal(manifest.payloads.length,404);
assert.equal(manifest.source,diagnosis.source);
assert.equal(diagnosis.goalStatus,'Active');
assert.equal(diagnosis.fullNI12Status,'PARTIAL');
assert.equal(diagnosis.fullNI13Status,'PARTIAL');
assert.equal(diagnosis.frozenWindows,'3d735745f1117d42a7859e87604a106351dca935');
for(const flag of ['newNativeApksBuilt','actualOfflineJavaJniVerified',
    'visibleOfflineShopAndStorageVerified','normalIntentIsolationVerified'])
    assert.equal(diagnosis[flag],true,flag);
for(const flag of ['wholePhoneUiAccepted','realAuthenticationAccepted','realNetworkAccepted',
    'purchaseAccepted','storageTransferAccepted','physicalDeviceAccepted','rendererFixAccepted',
    'zeroRendererErrorAccepted','fullResourcePackAccepted','wholeGoalComplete',
    'untrackedRecursivelyHashed','appDataContentHashed']) assert.equal(diagnosis[flag],false,flag);
const overallScope=git(['diff','--name-only',diagnosis.previousPublishedAndroid,diagnosis.source])
    .toString().trim().split('\n').sort();
assert.deepEqual(overallScope,[...diagnosis.scope].sort());
const originals=json('raw/v33/original-worktrees-after.json');
assert.equal(originals.length,2);
for(const version of diagnosis.versions) {
    const prefix='raw/v'+version.version+'/';
    const binding=json(prefix+'source-commit-binding.json');
    const audit=json(prefix+'source-and-package-baseline.json');
    const packages=json(prefix+'package-baseline.json');
    const installed=json(prefix+'install-baseline.json');
    assert.equal(binding.source,version.source);
    assert.equal(audit.source,version.source);
    assert.equal(packages.source,version.source);
    assert.equal(installed.source,version.source);
    assert.equal(packages.versionCode,version.version);
    assert.equal(installed.versionCode,version.version);
    assert.equal(packages.gatewayUrl,'');
    assert.equal(audit.inputs.length,47);
    assert.deepEqual(audit.inputs,binding.inputs);
    assert.deepEqual(binding.sourceScope,version.sourceScope);
    assert.deepEqual(git(['diff-tree','--no-commit-id','--name-only','-r',version.source])
        .toString().trim().split('\n').sort(),[...binding.sourceScope].sort());
    git(['merge-base','--is-ancestor',diagnosis.frozenWindows,version.source]);
    for(const row of binding.inputs) {
        const bytes=git(['show',version.source+':'+row.path]);
        assert.equal(bytes.length,row.bytes,row.path);
        assert.equal(sha(bytes),row.sha256,row.path);
    }
    for(const file of binding.protectedPathsUnchanged) {
        assert.equal(sha(git(['show',version.source+':'+file])),
            sha(git(['show',diagnosis.previousPublishedAndroid+':'+file])),file);
    }
    assert.deepEqual(originals,json(prefix+'original-worktrees-before.json'));
    assert.equal(version.gates.length,7);
    for(const label of version.gates) {
        const report=json(prefix+label+'-result.json'),log=read(prefix+label+'.log').toString();
        assert.equal(report.source,version.source,label);
        assert.equal(report.code,0,label);
        assert.equal(report.sourceUnchanged,true,label);
        assert.deepEqual(report.sourceBefore,binding.inputs,label);
        assert.deepEqual(report.sourceAfter,binding.inputs,label);
        let counts;
        if(label.startsWith('bound-rust-normal-')) counts=[version.rustPassed.normal,0];
        if(label.startsWith('bound-rust-preview-')) counts=[version.rustPassed.preview,0];
        if(label.startsWith('bound-shared-native-player-')) counts=[version.rustPassed.shared,10];
        if(label.startsWith('bound-runtime-')) counts=[version.rustPassed.runtime,1];
        if(counts) assert(log.includes('test result: ok. '+counts[0]+' passed; 0 failed; '+counts[1]+' ignored;'),label);
        if(label.startsWith('bound-api31-')) {
            assert(report.cwd.endsWith('/platform-android'));
            assert.deepEqual(report.args.slice(0,7),['+1.95.0','ndk','-t','arm64-v8a','--platform','31','check']);
            assert(report.args.includes('--lib')&&report.args.includes('--locked')&&report.args.includes('--offline'));
            assert(log.includes('Finished'));
        }
        if(label.startsWith('bound-java-full-')) {
            assert(report.args.includes('--rerun-tasks')&&report.args.includes('--no-build-cache'));
            assert(log.includes('compileDebugUnitTestJavaWithJavac'));
            assert(log.includes('compileUiPreviewUnitTestJavaWithJavac'));
            for(let variant=0;variant<2;variant++) {
                const rows=manifest.payloads.filter(row=>row.path.startsWith(prefix+label+'-xml-'+variant+'/')&&row.path.endsWith('.xml'));
                assert.equal(rows.length,6);
                let total=0;
                for(const row of rows) {
                    const xml=read(row.path).toString();
                    const n=key=>Number(xml.match(new RegExp('\\b'+key+'="(\\d+)"'))?.[1]||0);
                    total+=n('tests');
                    assert.equal(n('failures')+n('errors')+n('skipped'),0,row.path);
                }
                assert.equal(total,version.javaPassedEach);
            }
        }
    }
    assert.equal(audit.selectedResourceCountEach,6647);
    assert.equal(audit.metadataCountEach,3);
    assert.equal(audit.artifacts.length,2);
    assert.equal(installed.artifacts.length,2);
    for(const artifact of audit.artifacts) {
        const packageRecord=packages.artifacts.find(v=>v.variant===artifact.variant);
        const install=installed.artifacts.find(v=>v.variant===artifact.variant);
        assert.equal(artifact.source,version.source);
        assert.equal(artifact.sha256,packageRecord.sha256);
        assert.equal(artifact.bytes,packageRecord.bytes);
        assert.equal(install.installedSha256,artifact.sha256);
        assert.equal(install.upgradePreservedDataFlag,true);
        const config=read(prefix+artifact.variant+'-BuildConfig.java.txt').toString();
        assert(config.includes('VERSION_CODE = '+version.version));
        assert(config.includes('GATEWAY_URL = ""'));
        assert(config.includes('UI_PREVIEW = '+(artifact.variant==='preview')));
        assert.equal(artifact.personalJniDiagnosticCompiled,artifact.variant==='preview');
        assert.equal(artifact.nativeElfMatchesThisVariantStrippedOutput,true);
        assert.equal(artifact.libraries.reduce((sum,v)=>sum+v.pngCount,0),6647);
        assert(artifact.libraries.every(v=>v.allSelectedBytesMatch));
        const before=read(prefix+artifact.variant+'-package-before.txt').toString();
        const after=read(prefix+artifact.variant+'-package-after.txt').toString();
        const first=text=>text.match(/firstInstallTime=(.*)/)?.[1];
        assert.equal(first(before),first(after));
        assert.equal(first(after),install.preservedFirstInstallTime);
        assert.equal(read(prefix+artifact.variant+'-source-before.txt').toString().trim(),version.source);
        assert.equal(read(prefix+artifact.variant+'-source-after.txt').toString().trim(),version.source);
        assert.equal(read(prefix+artifact.variant+'-status-before.txt').length,0);
        assert.equal(read(prefix+artifact.variant+'-status-after.txt').length,0);
    }
}
for(const version of [31,32]) {
    const prefix='raw/v'+version+'/ui-gameshop-jni/';
    const report=json(prefix+'commands.json');
    assert.equal(report.javaProducerSent,true);
    assert.equal(report.sharedConsumerObserved,false);
    assert(read(prefix+report.latestLog).toString().includes('PERSONAL_JNI_OFFLINE_SENT'));
}
assert(read('raw/v32/ui-gameshop-jni/0-cold-pid.log').toString()
    .includes('render receipt does not match the authenticated player'));
assert.equal(read('raw/v31/red-jni-preview.log').toString().includes('0 passed; 2 failed;'),true);
assert.equal(read('raw/v31/working-preview-full.log').toString().includes('385 passed; 1 failed;'),true);
assert.equal(json('raw/v31/working-java-result.json').code,1);
assert(read('raw/v31/working-java.log').toString().includes('39 个错误'));
assert.equal(json('raw/v31/bound-api31-preview-v31-result.json').code,1);
assert.equal(json('raw/v33/red-jni-identity-v33-result.json').code,101);
assert(read('raw/v33/red-jni-identity-v33.log').toString().includes('0 passed; 1 failed;'));
assert.equal(json('raw/v33/red-jni-resize-v33-result.json').code,1);
assert(read('raw/v33/red-jni-resize-v33.log').toString().includes('1 test completed, 1 failed'));

const observations=diagnosis.observations;
assert.equal(observations.length,4);
assert.equal(new Set(observations.map(v=>v.pid)).size,4);
for(const row of observations) {
    const log=read(row.log).toString(),normal=row.scene==='normal-intent-isolation';
    assert.equal(log.includes('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE'),!normal);
    assert.equal((log.match(/I Mir2UiPreview: PERSONAL_JNI_OFFLINE_SENT/g)||[]).length,normal?0:1);
    assert.equal((log.match(/I Mir2UiPreview: PERSONAL_JNI_OFFLINE_START/g)||[]).length,normal?0:1);
    assert.equal((log.match(/E emuglGLESv2_enc:.*GL error 0x506/g)||[]).length,row.gl506);
    assert.equal((log.match(/D eglCodecCommon:.*rbo not color renderable\. format: 0x0/g)||[]).length,row.uninitializedColor);
    assert.equal(row.fatalOrPanic,0);
    assert(!/FATAL EXCEPTION|thread '.*' panicked|Fatal signal/.test(log));
    if(normal) assert(!log.includes('ANDROID_UI_PREVIEW_READY'));
    else {
        const receipt=log.split('\n').find(line=>line.includes('I event src/ui_preview.rs:154:')&&line.includes('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE'));
        assert(receipt);
        for(const text of ['catalogue = 105','last_g_index = 2104','last_stock = 3',
            'storage_items = 160','storage_size = 160','last_slot = 159','has_password = true',
            'icon_width = 16','icon_height = 35']) assert(receipt.includes(text),text);
        assert(receipt.includes('unlocked = '+(row.scene!=='storage-locked-jni')));
    }
}
assert.equal(observations.reduce((s,row)=>s+row.gl506,0),52);
assert.equal(diagnosis.gl506UniquePidTotal,52);
const secure=read('raw/v33/ui-storage-locked-jni/capture-diagnostic-window.txt').toString();
assert(secure.includes('FULLSCREEN SECURE'));
assert(secure.includes('ITYPE_IME: visible'));
assert.equal(manifest.images.filter(v=>v.valid).length,11);
assert.equal(manifest.images.filter(v=>!v.valid&&v.bytes===0).length,3);
for(const row of manifest.images) {
    const b=read(row.path);
    if(row.valid) {
        assert.equal(b.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
        assert.equal(b.readUInt32BE(16),2340);
        assert.equal(b.readUInt32BE(20),1080);
    } else assert.equal(b.length,0);
}
assert.equal(diagnosis.ui.gameshopNextTapPassed,false);
assert.equal(diagnosis.ui.gameshopNextLongPressPassed,false);
assert.equal(diagnosis.ui.storageSecondTabPassed,true);
assert.equal(diagnosis.ui.storageSecondTabRetainedAfterResume,true);
assert.equal(diagnosis.ui.lockedScreenshotAccepted,false);
assert.equal(diagnosis.ui.lockedModelUnlocked,false);
const remote=read('raw/v33/remote-before.txt').toString();
assert(remote.includes(diagnosis.previousPublishedAndroid+'\trefs/heads/codex/android-shared-sync'));
assert(remote.includes(diagnosis.observedRemoteWindows+'\trefs/heads/codex/windows-player-journey'));
const pr=json('raw/v33/pr-before.json');
assert.equal(pr.number,253);
assert.equal(pr.state,'open');
assert.equal(pr.draft,true);
assert.equal(pr.headRef,'codex/android-shared-sync');
assert.equal(pr.headSha,diagnosis.previousPublishedAndroid);
assert.equal(pr.baseRef,'codex/playtest-registration');
console.log(JSON.stringify({mode,source:diagnosis.source,payloads:manifest.payloads.length,
    bytes:manifest.totalBytes,validOriginalImages:11,emptyProtectedCaptures:3,
    actualOfflineJni:true,fullNI12:'PARTIAL',fullNI13:'PARTIAL',goal:'Active',gl506:52}));
