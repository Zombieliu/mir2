import fs from 'node:fs';
import path from 'node:path';
import cp from 'node:child_process';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';

const directory=path.dirname(fileURLToPath(import.meta.url));
const mode=process.argv[2]||'working';
assert(['working','index','HEAD'].includes(mode),'mode: working, index or HEAD');
const git=(args)=>cp.execFileSync('git',args,{cwd:directory});
const repo=git(['rev-parse','--show-toplevel']).toString().trim();
const qa=path.relative(repo,directory).split(path.sep).join('/');
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
function read(relative) {
    assert(!relative.includes('..') && !path.isAbsolute(relative));
    const file=qa+'/'+relative;
    return mode==='working'?fs.readFileSync(path.join(repo,file)):
        git(['show',mode==='index'?':'+file:'HEAD:'+file]);
}
const json=file=>JSON.parse(read(file).toString());
const manifest=json('manifest.json');
for(const row of manifest.payloads) {
    const bytes=read(row.path);
    assert.equal(bytes.length,row.bytes,row.path+' byte count');
    assert.equal(sha(bytes),row.sha256,row.path+' hash');
    assert(!/\.(apk|aab|jks|keystore|p12|pem)$/i.test(row.path),'no binary/key artifacts');
}
assert.equal(manifest.payloads.reduce((sum,row)=>sum+row.bytes,0),manifest.totalBytes);
const diagnosis=json('diagnosis.json'),binding=json('raw/source-commit-binding.json');
const source=binding.source,base=json('raw/baseline.json'),audit=json('raw/source-boundary-audit.json');
assert.equal(diagnosis.source,source);
assert.equal(diagnosis.goalStatus,'Active');
assert.equal(diagnosis.fullNI13Status,'PARTIAL');
assert.equal(diagnosis.sourceLeafPassed,true);
for(const flag of ['newApkBuilt','actualJniVerified','visibleUiVerified','onlineVerified',
    'physicalDeviceVerified','rendererFixed','wholeGoalComplete']) assert.equal(diagnosis[flag],false,flag);
assert.equal(audit.wholeGoalComplete,false);
assert.equal(audit.newApkBuilt,false);
assert.equal(audit.originals.length,2);
assert.deepEqual(audit.originals,base.originals);
assert.equal(audit.untrackedRecursivelyHashed,false);
const sourceBlob=file=>git(['show',source+':'+file]);
const paths=binding.inputs.map(row=>row.path);
assert.equal(paths.length,6);
assert.equal(new Set(paths).size,6);
const changed=git(['diff-tree','--no-commit-id','--name-only','-r',source])
    .toString().trim().split('\n').sort();
assert.deepEqual(changed,[...paths].sort());
for(const row of binding.inputs) {
    const bytes=sourceBlob(row.path);
    assert.equal(bytes.length,row.bytes,row.path);
    assert.equal(sha(bytes),row.sha256,row.path);
}
git(['merge-base','--is-ancestor',base.frozenWindows,source]);
assert.equal(base.frozenWindows,'3d735745f1117d42a7859e87604a106351dca935');
for(const row of base.protectedFiles) assert.equal(sha(sourceBlob(row.path)),row.sha256,row.path);
const sharedPath='mir2-web3/apps/game-client/client-bevy/src/storage.rs';
const shared=sourceBlob(sharedPath).toString();
const old=git(['show',binding.parent+':'+sharedPath]).toString();
assert(shared.startsWith(old),'original storage rules must remain byte-identical');
function extract(text,name) {
    const start=text.indexOf('fn '+name+'(');
    assert(start>=0,name);
    const open=text.indexOf('{',start);
    let depth=0,quoted=false,escaped=false;
    for(let at=open;at<text.length;at++) {
        const char=text[at];
        if(quoted) {
            if(escaped) escaped=false;
            else if(char==='\\') escaped=true;
            else if(char==='"') quoted=false;
        } else if(char==='"') quoted=true;
        else if(char==='{') depth++;
        else if(char==='}' && --depth===0) return text.slice(start,at+1);
    }
    throw Error('Unclosed function '+name);
}
function normalized(text) {
    let out='',quoted=false,escaped=false;
    for(const char of text) {
        if(quoted) {
            out+=char;
            if(escaped) escaped=false;
            else if(char==='\\') escaped=true;
            else if(char==='"') quoted=false;
        } else if(char==='"') {out+=char;quoted=true;}
        else if(!/\s/.test(char)) out+=char;
    }
    return out;
}
const frozen=git(['show',base.frozenWindows+':mir2-web3/apps/game-client/platform-windows/src/gateway.rs']).toString();
assert.equal(audit.functions.length,10);
for(const row of audit.functions) {
    const a=normalized(extract(frozen,row.name)),b=normalized(extract(shared,row.name));
    assert.equal(a,b,row.name+' frozen tokens');
    assert.equal(sha(a),row.frozenHash);
    assert.equal(sha(b),row.sharedHash);
    assert.equal(row.exactFrozenTokens,true);
}
const rustCounts={
    'bound-rust-normal':[359,0],
    'bound-rust-preview':[384,0],
    'bound-shared-native-player':[1260,10],
    'bound-runtime':[292,1],
};
assert.equal(diagnosis.boundGateLabels.length,7);
for(const label of diagnosis.boundGateLabels) {
    const report=json('raw/'+label+'-result.json');
    assert.equal(report.code,0,label);
    assert.equal(report.source,source,label);
    assert.equal(report.sourceUnchanged,true,label);
    assert.deepEqual(report.sourceBefore,binding.inputs,label+' before');
    assert.deepEqual(report.sourceAfter,binding.inputs,label+' after');
    const log=read('raw/'+label+'.log').toString();
    if(rustCounts[label]) {
        const [passed,ignored]=rustCounts[label];
        assert(log.includes('test result: ok. '+passed+' passed; 0 failed; '+ignored+' ignored;'),label);
    }
    if(label.startsWith('bound-api31-')) {
        assert(report.cwd.endsWith('/platform-android'),label+' cargo-ndk metadata cwd');
        assert.deepEqual(report.args.slice(0,7),
            ['+1.95.0','ndk','-t','arm64-v8a','--platform','31','check']);
        assert(log.includes('Finished'),label);
    }
}
const normalLog=read('raw/bound-rust-normal.log').toString();
assert.equal((normalLog.match(/^test storage_ingress::tests::.* \.\.\. ok$/gm)||[]).length,11);
assert.equal((normalLog.match(/^test shared_shell::tests::storage_host_.* \.\.\. ok$/gm)||[]).length,5);
const sharedLog=read('raw/bound-shared-native-player.log').toString();
assert.equal((sharedLog.match(/^test storage::native_storage_projection_tests::.* \.\.\. ok$/gm)||[]).length,3);
for(let variant=0;variant<2;variant++) {
    const rows=manifest.payloads.filter(row=>row.path.startsWith('raw/bound-java-full-xml-'+variant+'/')&&row.path.endsWith('.xml'));
    assert.equal(rows.length,5);
    let count=0;
    for(const row of rows) {
        const xml=read(row.path).toString();
        const number=name=>Number(xml.match(new RegExp('\\b'+name+'="(\\d+)"'))?.[1]||0);
        count+=number('tests');
        assert.equal(number('failures')+number('errors')+number('skipped'),0,row.path);
    }
    assert.equal(count,47);
}
const javaLog=read('raw/bound-java-full.log').toString();
assert(javaLog.includes('compileDebugUnitTestJavaWithJavac'));
assert(javaLog.includes('compileUiPreviewUnitTestJavaWithJavac'));
const red=read('raw/java-red.xml').toString(),redInput=json('raw/red-input.json');
assert(red.includes('tests="2"')&&red.includes('failures="2"'));
assert.equal(json('raw/java-red-result.json').code,1);
assert.equal(sha(read('raw/GatewaySessionTest-red.java')),redInput.testSha256);
assert(read('raw/java-red.log').toString().includes('compileDebugUnitTestJavaWithJavac'));
const oldGateway=git(['show',binding.parent+':mir2-web3/apps/game-client/platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java']);
assert.equal(sha(read('raw/GatewaySession-before.java')),sha(oldGateway));
const preparation=json('raw/failure-classification.json');
assert.equal(preparation.productRed.tests,2);
assert.equal(preparation.productRed.failures,2);
assert(read('raw/java-green-full.log').toString().includes("Task 'testUiPreviewDebugUnitTest' not found"));
assert.equal(json('raw/java-green-full-result.json').code,1);
assert.equal(audit.existingStorageRulesByteIdentical,true);
assert.equal(audit.protectedFilesUnchanged,true);
console.log(JSON.stringify({mode,source,payloads:manifest.payloads.length,result:'PASS',
    newApkBuilt:false,fullNI13Status:'PARTIAL',wholeGoalComplete:false}));
