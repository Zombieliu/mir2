import fs from 'node:fs/promises';import path from 'node:path';import crypto from 'node:crypto';import {fileURLToPath} from 'node:url';
import {DRIVER_BINDING,assertFinalDriverBinding} from './driver-binding-contract.mjs';
const root=path.dirname(fileURLToPath(import.meta.url)),library=path.resolve(root,'../scout-anti-convergence-runtime-v1/source-lib-v1.mjs'),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
export async function verifyFreshBackendWithCurrentDriver({repositoryDirectory,driverSourceCommit}){
 assertFinalDriverBinding();if(driverSourceCommit!==DRIVER_BINDING.driverSourceCommit)throw Error('Current driver identity differs');
 const binding=JSON.parse(await fs.readFile(path.join(root,'backend-library-binding.json'),'utf8')),ref=binding.library;
 if(binding.expectedBackendInputCount!==50||path.resolve(ref?.file??ref?.path??'')!==library||!Number.isSafeInteger(ref.bytes)||ref.bytes<1||!/^[a-f0-9]{64}$/.test(ref.sha256??''))throw Error('Pinned original-location runtime library required');
 const stat=await fs.lstat(library),real=await fs.realpath(library),bytes=await fs.readFile(library);
 if(!stat.isFile()||stat.isSymbolicLink()||path.resolve(real).toLowerCase()!==library.toLowerCase()||bytes.length!==ref.bytes||sha(bytes)!==ref.sha256)throw Error('Runtime reader raw identity differs');
 const runtimeRef=binding.runtimeBinding,expectedBinding=path.resolve(root,'../scout-anti-convergence-runtime-v1/current-execution-binding-v1.json');
 if(path.resolve(runtimeRef?.file??'')!==expectedBinding||!Number.isSafeInteger(runtimeRef.bytes)||!/^[a-f0-9]{64}$/.test(runtimeRef.sha256??''))throw Error('Actual H runtime binding reference is required');
 const runtimeBytes=await fs.readFile(expectedBinding);if(runtimeBytes.length!==runtimeRef.bytes||sha(runtimeBytes)!==runtimeRef.sha256)throw Error('Actual H runtime binding changed');
 const checks=await import('./projection-build-source-lib.mjs');if(checks.EXPECTED_INPUT_COUNT!==50)throw Error('Backend closure differs');
 const frozen=await checks.requireFrozen({repositoryDirectory,expectedHead:DRIVER_BINDING.executionSourceCommit});
 if(frozen.current?.executionSourceCommit!==DRIVER_BINDING.executionSourceCommit||frozen.backendSourceCommit!==DRIVER_BINDING.backendSourceCommit||frozen.driverSourceCommit!==driverSourceCommit||frozen.rebuildPerformed!==false)throw Error('Reused backend/current execution split differs');
 return {...frozen,checks,backendSourceCommit:DRIVER_BINDING.backendSourceCommit,driverSourceCommit,executionSourceCommit:DRIVER_BINDING.executionSourceCommit,backendSourceUnchangedSinceBuild:true,backendChangesAfterBuild:0,rebuildPerformed:false};
}
