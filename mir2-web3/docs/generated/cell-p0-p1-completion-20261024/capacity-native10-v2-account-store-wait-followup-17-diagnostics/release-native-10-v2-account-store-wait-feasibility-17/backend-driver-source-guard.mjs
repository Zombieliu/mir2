import fs from 'node:fs/promises';import path from 'node:path';import crypto from 'node:crypto';
import {DRIVER_BINDING,assertFinalDriverBinding} from './driver-binding-contract.mjs';
const root=path.dirname(import.meta.filename),library=path.resolve(root,'../account-store-wait-diagnostics-release-v1/source-lib-v1.mjs');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
export async function verifyFreshBackendWithCurrentDriver({repositoryDirectory,driverSourceCommit}){
 assertFinalDriverBinding();
 if(driverSourceCommit!==DRIVER_BINDING.driverSourceCommit||!/^[a-f0-9]{40}$/.test(DRIVER_BINDING.backendSourceCommit??''))throw Error('Historical Node driver and actual current backend identities must be separately bound');
 const binding=JSON.parse(await fs.readFile(path.join(root,'backend-library-binding.json'),'utf8')),ref=binding.library;
 if(binding.expectedBackendInputCount!==50||ref?.path!==library||!Number.isSafeInteger(ref.bytes)||ref.bytes<1||!/^[a-f0-9]{64}$/.test(ref.sha256??''))throw Error('Pinned original-location50 source library required');
 const stat=await fs.lstat(library),real=await fs.realpath(library),bytes=await fs.readFile(library);
 if(!stat.isFile()||stat.isSymbolicLink()||path.resolve(real).toLowerCase()!==path.resolve(library).toLowerCase()||bytes.length!==ref.bytes||sha(bytes)!==ref.sha256)throw Error('Original-location source library raw identity differs');
 const checks=await import('./projection-build-source-lib.mjs');
 if(checks.EXPECTED_INPUT_COUNT!==50)throw Error('New source library closure differs');
 const frozen=await checks.requireFrozen({repositoryDirectory,expectedHead:DRIVER_BINDING.backendSourceCommit});
 return{...frozen,checks,backendSourceCommit:DRIVER_BINDING.backendSourceCommit,driverSourceCommit,backendSourceUnchangedSinceBuild:true,backendChangesAfterBuild:0,rebuildPerformed:true};
}
