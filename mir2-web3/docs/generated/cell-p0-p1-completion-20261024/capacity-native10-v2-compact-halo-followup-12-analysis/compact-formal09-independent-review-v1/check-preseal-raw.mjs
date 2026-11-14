import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const base='C:/mir2-build/p01-completion-20261024';
const reviewRoot=path.join(base,'compact-formal09-independent-review-v1');
const target=path.join(base,'formal-native-v2-compact-halo-ladder-09');
const args=process.argv.slice(2);
assert.equal(args.length,2,'Expected --preseal <direct-base-receipt-basename>');
assert.equal(args[0],'--preseal');
assert.match(args[1],/^[a-z0-9-]+preseal-input-check-v[0-9]+\.json$/);
function same(a,b){return path.resolve(a).toLowerCase()===path.resolve(b).toLowerCase();}
function knownFile(file,root){assert(same(path.dirname(file),root),'Only direct explicitly named small metadata files may be read');const real=fs.realpathSync(file);assert(same(real,file),'Redirected file denied');const st=fs.statSync(real);assert(st.isFile()&&st.size<=1048576,'Only <=1MiB source/metadata files allowed');return fs.readFileSync(real);}
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const digest=b=>({bytes:b.length,sha256:sha(b)});
const cfg=JSON.parse(knownFile(path.join(reviewRoot,'review-checklist-v1.json'),reviewRoot));
const presealFile=path.join(base,args[1]),presealBytes=knownFile(presealFile,base),preseal=JSON.parse(presealBytes);
assert(same(fs.realpathSync(target),target),'Target root must be real');
assert.equal(preseal.backendSourceCommit,cfg.immutableIdentities.backendSourceCommit);
assert.equal(preseal.driverSourceCommit,cfg.immutableIdentities.driverSourceCommit);
assert.equal(preseal.driverSourceSha256,cfg.immutableIdentities.driverSourceSha256);
assert.equal(preseal.sceneSourceSha256,cfg.immutableIdentities.sceneSourceSha256);
assert(Array.isArray(preseal.reviewedFiles)&&preseal.reviewedFiles.length>25&&preseal.reviewedFiles.length<=64);
const names=new Set();
for(const row of preseal.reviewedFiles){assert.equal(typeof row.file,'string');const name=path.basename(row.file);assert.match(name,/^[A-Za-z0-9._-]+$/);assert(!names.has(name),'Duplicate file');names.add(name);assert.deepEqual(digest(knownFile(row.file,target)),{bytes:row.bytes,sha256:row.sha256});}
assert.equal(sha(Buffer.from(JSON.stringify(preseal.reviewedFiles))),preseal.reviewedSourceSha256);
for(const name of ['run-owned.mjs','prepare-inputs.mjs','input-lib.mjs','verify-feasibility-gate.mjs','scene-contract.mjs','collect-public.mjs','verify-public.mjs','run-options.json','resource-profile.json','scenario.json'])assert(names.has(name),name+' must be pinned');
const source=name=>knownFile(path.join(target,name),target).toString('utf8');
const opts=JSON.parse(source('run-options.json'));
assert.deepEqual(opts.policy,cfg.baseline.policy);
assert.deepEqual(digest(knownFile(path.join(target,'resource-profile.json'),target)),{bytes:cfg.baseline.resourceProfile.bytes,sha256:cfg.baseline.resourceProfile.sha256});
assert.deepEqual(digest(knownFile(path.join(target,'scenario.json'),target)),cfg.immutableIdentities.formalScene);
const resources=JSON.parse(source('resource-profile.json'));
for(const [key,value]of Object.entries(cfg.formalConfiguration.resourceLimits))assert.equal(resources[key],value);
for(const name of ['collect-public.mjs','verify-public.mjs']){const text=source(name),m=text.match(/const expectedPolicy = (\{[\s\S]*?\r?\n\});/);assert(m,'Explicit policy literal missing');assert.deepEqual(JSON.parse(m[1]),cfg.baseline.policy);}
const absent=['private','bin','accounts.json','source-pins.json','preparation.lock','execution.lock','realm-process.json'];for(const name of absent)assert.equal(fs.existsSync(path.join(target,name)),false,name+' should not exist before review');
const r={schemaVersion:1,checkedAt:new Date().toISOString(),scope:'FS-only bounded metadata/raw-byte verification. No product/template imports, subprocesses, syntax or test runs, sourceguard, geometry/model checks or live raw reads.',preseal:{file:presealFile,...digest(presealBytes)},reviewedFileCount:preseal.reviewedFiles.length,reviewedSourceSha256:preseal.reviewedSourceSha256,allLocalPinsMatched:true,originalFormalPolicyExact:true,originalFormalResourcesRawExact:true,formalScenarioRawExact:true,noProvisioningState:true,semanticReviewPending:true,suitableToSeal:null,authorizesPrepareOrRun:false,capacityAccepted:false};
const output=path.join(reviewRoot,'preseal-raw-check-v1.json');fs.writeFileSync(output,JSON.stringify(r,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({file:output,...digest(fs.readFileSync(output)),reviewedFileCount:r.reviewedFileCount,semanticReviewPending:true}));
