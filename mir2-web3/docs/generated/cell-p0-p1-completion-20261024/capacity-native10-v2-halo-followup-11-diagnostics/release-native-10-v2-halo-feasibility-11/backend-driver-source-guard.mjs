import fs from 'node:fs/promises';
import {captureSource,git,sourceInput,MANIFEST,GIT_BASE,BUILD_ARGS} from './socket-build-source-lib.mjs';
const backend='46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b';
export async function verifyImmutableBackendWithCurrentDriver({repositoryDirectory,driverSourceCommit}){
 if(!/^[a-f0-9]{40}$/.test(driverSourceCommit??'')||driverSourceCommit===backend)throw Error('An explicit new committed timing-driver HEAD is required');
 const current=await captureSource({repositoryDirectory,expectedHead:driverSourceCommit});
 git(current.repository,['merge-base','--is-ancestor',backend,driverSourceCommit]);
 const backendChanges=git(current.repository,['diff','--no-ext-diff','--no-textconv','--name-only','-z',backend,driverSourceCommit,'--','mir2-web3']).toString('utf8').split('\0').filter(Boolean).filter(sourceInput);
 if(backendChanges.length)throw Error('Immutable socket backend changed after its accepted468 Release');
 const manifestBytes=await fs.readFile(MANIFEST),manifest=JSON.parse(manifestBytes);
 if(manifest.schemaVersion!==1||manifest.sourceCommit!==backend||manifest.gitBase!==GIT_BASE||manifest.sourceInputCount!==41||manifest.files?.length!==41||manifest.sourceManifestSha256!==current.sourceManifestSha256||JSON.stringify(manifest.files)!==JSON.stringify(current.files)||JSON.stringify(manifest.fixtureProofs)!==JSON.stringify(current.fixtureProofs)||JSON.stringify(manifest.buildArguments)!==JSON.stringify(BUILD_ARGS))throw Error('Immutable468 build manifest differs from the current exact41 raw/Git backend closure');
 return{manifest,manifestBytes,current,backendSourceCommit:backend,driverSourceCommit,backendSourceUnchangedSinceBuild:true,backendChangesAfterBuild:0,rebuildPerformed:false};
}
