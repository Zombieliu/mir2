import fs from 'node:fs';
import path from 'node:path';
import {verifyFrozen,assert,sha,DIRECTORY,same} from './source-lib-v1.mjs';
export async function loadPreparedInputs(repo,taskRoot,frozenSha){
 assert(same(taskRoot,DIRECTORY),'Fixed independent owned package required');const verified=verifyFrozen(frozenSha),f=verified.frozen,b=verified.backend;assert(same(repo,path.join(f.repository,'mir2-web3')),'Current project directory differs');const backendBytes=fs.readFileSync(b.manifestPin.file),manifest=JSON.parse(backendBytes),releaseRecordBytes=fs.readFileSync(b.releasePin.file),release=JSON.parse(releaseRecordBytes);
 return{profile:f.profile,backendBytes,backendManifest:{...manifest.current,files:manifest.current.files},backendInputCount:55,releaseRecordBytes,frozenPin:verified.frozenPin,
 releaseInput:{executable:b.immutableExecutable.file,bytes:b.immutableExecutable.bytes,sha256:b.immutableExecutable.sha256,backendSourceManifest:b.manifestPin.file,backendSourceAggregate:manifest.current.sourceManifestSha256??sha(Buffer.from(JSON.stringify(manifest.current.files))),backendSourceManifestFileSha256:b.manifestPin.sha256,baseCommit:manifest.current.gitBase,sourceCommit:b.backendSourceCommit,executionSourceCommit:f.executionSourceCommit,driverSourceCommit:f.executionSourceCommit,releaseRecordFileSha256:b.releasePin.sha256,backendBuildCommit:b.backendBuildCommit},verified};
}
