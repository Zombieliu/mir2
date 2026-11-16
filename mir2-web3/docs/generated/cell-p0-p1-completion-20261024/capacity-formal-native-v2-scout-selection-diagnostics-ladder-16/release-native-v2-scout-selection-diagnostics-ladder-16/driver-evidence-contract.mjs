import fs from 'node:fs/promises';
import crypto from 'node:crypto';
const target=new URL('../release-native-10-v2-scout-selection-diagnostics-feasibility-20/driver-evidence-contract.mjs',import.meta.url);
const expected={bytes:10516,sha256:'3f94f86cd7c9ba3f7faa667911f36cd18b322c39e17be79adafbdc896b2a10eb'};
async function load(){const bytes=await fs.readFile(target);if(bytes.length!==expected.bytes||crypto.createHash('sha256').update(bytes).digest('hex')!==expected.sha256)throw Error('Original-location current K driver-evidence source changed');return import(target.href);}
export async function verifyDriverEvidenceBasis(frozen,assertPin){return(await load()).verifyDriverEvidenceBasis(frozen,assertPin);}
export async function getDriverDeclarationBasis(frozen){return(await load()).getDriverDeclarationBasis(frozen);}
