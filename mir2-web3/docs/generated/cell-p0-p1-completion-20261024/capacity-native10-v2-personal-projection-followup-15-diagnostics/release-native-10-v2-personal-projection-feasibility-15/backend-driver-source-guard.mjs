import fs from 'node:fs/promises';import path from 'node:path';import crypto from 'node:crypto';
import {DRIVER_BINDING,assertFinalDriverBinding} from './driver-binding-contract.mjs';
const root=path.dirname(import.meta.filename),library=path.resolve(root,'../personal-monster-projection-release-v1/source-lib-v3.mjs');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
export async function verifyFreshBackendWithCurrentDriver({repositoryDirectory,driverSourceCommit}){
 assertFinalDriverBinding();
 if(driverSourceCommit!==DRIVER_BINDING.driverSourceCommit||driverSourceCommit!==DRIVER_BINDING.backendSourceCommit)throw Error('Actual new backend/execution HEAD must be explicitly identical');
 const binding=JSON.parse(await fs.readFile(path.join(root,'backend-library-binding.json'),'utf8')),ref=binding.library;
 if(binding.expectedBackendInputCount!==44||ref?.path!==library||!Number.isSafeInteger(ref.bytes)||ref.bytes<1||!/^[a-f0-9]{64}$/.test(ref.sha256??''))throw Error('Pinned original-location44 source library required');
 const stat=await fs.lstat(library),real=await fs.realpath(library),bytes=await fs.readFile(library);
 if(!stat.isFile()||stat.isSymbolicLink()||path.resolve(real).toLowerCase()!==path.resolve(library).toLowerCase()||bytes.length!==ref.bytes||sha(bytes)!==ref.sha256)throw Error('Original-location source library raw identity differs');
 const checks=await import('./projection-build-source-lib.mjs');
 if(checks.EXPECTED_INPUT_COUNT!==44)throw Error('New source library closure differs');
 const frozen=await checks.requireFrozen({repositoryDirectory,expectedHead:driverSourceCommit});
 return{...frozen,checks,backendSourceCommit:driverSourceCommit,driverSourceCommit,backendSourceUnchangedSinceBuild:true,backendChangesAfterBuild:0,rebuildPerformed:true};
}
