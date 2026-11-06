import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {DRIVER_BINDING,assertFinalDriverBinding} from './driver-binding-contract.mjs';
const expected=path.resolve(fileURLToPath(new URL('../release-native-10-v2-scout-anti-convergence-feasibility-19/driver-evidence-contract.mjs',import.meta.url)));
async function shared(){
 assertFinalDriverBinding();
 const ref=DRIVER_BINDING.sharedDriverEvidence;
 if(!ref||path.resolve(ref.file??'').toLowerCase()!==expected.toLowerCase()||!Number.isSafeInteger(ref.bytes)||ref.bytes<1||!/^[a-f0-9]{64}$/.test(ref.sha256??''))throw Error('PENDING_FORMAL15_SHARED_PROBE19_NODE226_EVIDENCE: original-location module pin required');
 const stat=await fs.lstat(expected),real=await fs.realpath(expected),bytes=await fs.readFile(expected);
 if(!stat.isFile()||stat.isSymbolicLink()||path.resolve(real).toLowerCase()!==expected.toLowerCase()||bytes.length!==ref.bytes||crypto.createHash('sha256').update(bytes).digest('hex')!==ref.sha256)throw Error('Shared Probe19 evidence module differs');
 return{module:await import(pathToFileURL(expected)),pin:{file:expected,bytes:ref.bytes,sha256:ref.sha256}};
}
export async function getDriverDeclarationBasis(frozen){const source=await shared();return source.module.getDriverDeclarationBasis(frozen);}
export async function verifyDriverEvidenceBasis(frozen,assertPin){const source=await shared(),result=await source.module.verifyDriverEvidenceBasis(frozen,assertPin);return{...result,pins:[source.pin,...result.pins]};}
