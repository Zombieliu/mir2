import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
export { createSlowStagePublicCapture as createStorePublicCapture } from '../account-store-slow-stage-diagnostics-v1/slow-stage-public.mjs';
export { validateAccountStorePublicDataset } from '../account-store-slow-stage-diagnostics-v1/account-store-public-validator.mjs';
const base=path.dirname(path.dirname(import.meta.filename)),directory=path.join(base,'account-store-slow-stage-diagnostics-v1');
const manifest={file:path.join(directory,'source-freeze-v1.json'),bytes:4628,sha256:'632e875bd2d56299009411d6d22ee8912eed3f15c3d26f8b95466be2bd22f9cb'};
const review={file:path.join(base,'account-store-slow-stage-independent-review-v1/review-v1.json'),bytes:6459,sha256:'54a9c1a27336537d0232b6f8b76ac18f92ef76f8064899b2a253f78d68eefb68'};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const same=(a,b)=>path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();
async function checked(p){const stat=await fs.lstat(p.file),real=await fs.realpath(p.file),b=await fs.readFile(p.file);if(!stat.isFile()||stat.isSymbolicLink()||!same(real,p.file)||b.length!==p.bytes||sha(b)!==p.sha256)throw Error('Store diagnostic public source raw identity differs');return b;}
export async function verifyStoreDiagnosticSource(){
 const bytes=await checked(manifest),value=JSON.parse(bytes);await checked(review);
 if(value.schemaVersion!==1||value.files?.length!==17||sha(JSON.stringify(value.files))!==value.sourceSha256||value.sourceSha256!=='8db992f5b7246f585d0158a914e616bbf9397421b66d92f27b350444b1939c8c'||value.actualTests?.passed!==51||value.actualTests.failed!==0)throw Error('Reviewed Store helper source contract differs');
 const activeNames=['slow-stage-public.mjs','account-store-public-validator.mjs','protocol-contract-v1.json'],active=[];
 for(const name of activeNames){const p=value.files.find(p=>same(p.file,path.join(directory,name)));if(!p||!same(path.dirname(p.file??''),directory)||!Number.isSafeInteger(p.bytes)||p.bytes<1||!/^[a-f0-9]{64}$/.test(p.sha256??''))throw Error('Store active source is outside the explicit public package');await checked(p);active.push(p);}
 return{files:[...active,manifest,review],manifest,review,sourceSha256:value.sourceSha256};
}
export function storeDiagnosticIdentity(source){return{schemaVersion:1,dataset:'account-store',storeOnly:true,manifest:source.manifest,review:source.review,sourceSha256:source.sourceSha256,helperSha256:'6cde67571184692d7b89da32adb21fb2f8a228db5e0865cb9cc96bf3a34925c7',validatorSha256:'d71add02703a84acc2d870d5c7daf96dd17e0533b2d49af0480e55dea7aea262',separateFromBaseThreeFourSeven:true,contributesToGameAcceptance:false};}
