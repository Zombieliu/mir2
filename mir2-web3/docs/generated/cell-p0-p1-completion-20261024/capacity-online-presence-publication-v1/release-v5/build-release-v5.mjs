import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { DIRECTORY,TARGET,BUILD_ARGS,BASELINE_PIN,BASELINE_RELEASE_PIN,QUALIFICATION,assert,equal,pin,verifyPin,writeJson,loadConfig,capture,validateRustReceipt,requireFrozen } from './source-lib-v5.mjs';

// Provisional tool only. A fully pinned ROOT_FINAL_V5_SOURCE_CONFIG is mandatory.
// Root supplies a new committed HEAD, final exact backend whitelist/source rows,
// and a newly executed focused Rust receipt. Historical Rust57 or V4 Rust111 cannot qualify.
function options(args) {
 const keys=['repository','expected-head','config','config-sha256'],o={};
 assert(args.length===keys.length*2,'Usage: node build-release-v5.mjs --repository ROOT --expected-head NEW40HEX --config FINALJSON --config-sha256 SHA256');
 for(let i=0;i<args.length;i+=2) {const k=args[i].replace(/^--/,''); assert(args[i]===`--${k}`&&keys.includes(k)&&!(k in o),'Unknown/duplicate option: '+args[i]); o[k]=args[i+1];}
 for(const k of keys) assert(typeof o[k]==='string'&&o[k].length>0,'Missing '+k);
 assert(/^[a-f0-9]{64}$/.test(o['config-sha256']),'Expected lowercase config SHA256'); return o;
}
const o=options(process.argv.slice(2)),repositoryDirectory=path.resolve(o.repository),expectedHead=o['expected-head'];
assert(path.resolve(process.env.CARGO_TARGET_DIR??'')===path.resolve(TARGET),'Set CARGO_TARGET_DIR='+TARGET+' in VS2022 amd64 environment before execution');
const inputConfigPin=pin(path.resolve(o.config)); equal(inputConfigPin.sha256,o['config-sha256'],'CLI final config SHA mismatch');
const config=loadConfig(inputConfigPin,expectedHead),captureOptions={repositoryDirectory,expectedHead,configPin:inputConfigPin};
const before=capture(captureOptions),rustValidation=validateRustReceipt(config,before); before.selectedRustTestsPassed=rustValidation.selectedRustTestsPassed;
const names=['root-final-config-v5.json','source-before-v5.json','source-after-v5.json','build-stdout-v5.log','build-stderr-v5.log','build-receipt-v5.json','backend-source-v5.json','release-v5.json','bin'];
for(const n of names) assert(!fs.existsSync(path.join(DIRECTORY,n)),'One-shot V5 output exists; preserve evidence and prepare a new versioned directory: '+n);
// No metadata is written until all final config/source/fresh Rust checks pass.
const sealedConfig=writeJson('root-final-config-v5.json',config),sourceBefore=writeJson('source-before-v5.json',before);
const stdoutFile=path.join(DIRECTORY,'build-stdout-v5.log'),stderrFile=path.join(DIRECTORY,'build-stderr-v5.log');
const out=fs.openSync(stdoutFile,'wx'),err=fs.openSync(stderrFile,'wx'),startedAt=new Date().toISOString();
const result=await new Promise(resolve=>{let executionError=null; const child=spawn('rustup',BUILD_ARGS,{cwd:path.join(repositoryDirectory,'mir2-web3'),env:{...process.env,CARGO_BUILD_JOBS:'4'},stdio:['ignore',out,err],windowsHide:true}); child.on('error',e=>{executionError={name:e.name,message:e.message,code:e.code??null};}); child.on('close',(exitCode,signal)=>resolve({exitCode,signal,error:executionError}));});
fs.closeSync(out); fs.closeSync(err); const finishedAt=new Date().toISOString();
let after=null,sourceCaptureError=null;
try { verifyPin(inputConfigPin); after=capture({repositoryDirectory,expectedHead,configPin:sealedConfig}); after.selectedRustTestsPassed=validateRustReceipt(config,after).selectedRustTestsPassed; equal(after,before,'Sources or validation changed during actual V5 build'); }
catch(e) {sourceCaptureError={name:e.name,message:e.message};}
const sourceAfter=writeJson('source-after-v5.json',after??{sourceCaptureError});
const build={schemaVersion:1,startedAt,finishedAt,command:['rustup',...BUILD_ARGS],cwd:path.join(repositoryDirectory,'mir2-web3'),targetDirectory:path.resolve(TARGET),environment:{CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR,CARGO_BUILD_JOBS:'4',VSCMD_ARG_TGT_ARCH:process.env.VSCMD_ARG_TGT_ARCH??null,VSCMD_VER:process.env.VSCMD_VER??null},...result,sourceBefore,sourceAfter,sourceUnchanged:sourceCaptureError===null,sourceCaptureError,stdout:pin(stdoutFile),stderr:pin(stderrFile),immutableRelease:null};
let sealError=null;
if(result.exitCode===0&&result.signal===null&&result.error===null&&sourceCaptureError===null) {
 try {
  equal(validateRustReceipt(config,after),rustValidation,'New Rust receipt changed during build');
  const actual=path.join(TARGET,'release/mir2-gateway.exe'); assert(fs.statSync(actual).isFile()&&fs.statSync(actual).size>0,'Successful cargo build executable missing'); const actualBuildOutput=pin(actual);
  fs.mkdirSync(path.join(DIRECTORY,'bin')); const immutable=path.join(DIRECTORY,'bin/mir2-gateway.exe'); fs.copyFileSync(actual,immutable,fs.constants.COPYFILE_EXCL); fs.chmodSync(immutable,0o444);
  build.immutableRelease=pin(immutable); assert(actualBuildOutput.bytes===build.immutableRelease.bytes&&actualBuildOutput.sha256===build.immutableRelease.sha256,'Exclusive immutable copy differs from actual build executable'); build.actualBuildOutput=actualBuildOutput;
 } catch(e) {sealError={name:e.name,message:e.message};}
}
build.sealError=sealError;
const buildReceipt=writeJson('build-receipt-v5.json',build);
assert(result.exitCode===0&&result.signal===null&&result.error===null&&sourceCaptureError===null&&sealError===null&&build.immutableRelease,'Actual V5 build/seal failed; evidence saved at '+buildReceipt.file);
const backendSourceManifest=writeJson('backend-source-v5.json',{schemaVersion:1,capturedAt:new Date().toISOString(),config:sealedConfig,rootConfigInput:inputConfigPin,current:after,baselineManifest:BASELINE_PIN,baselineRelease:BASELINE_RELEASE_PIN,rustValidation,selectedRustTestsPassed:rustValidation.selectedRustTestsPassed,toolPins:['source-lib-v5.mjs','build-release-v5.mjs'].map(n=>pin(path.join(DIRECTORY,n))),...QUALIFICATION});
const identities=Object.fromEntries(['sourceCommit','backendSourceCommit','executionSourceCommit','driverSourceCommit','navigationDependencyCommit','observationDependencyCommit','frontendSourceCommit'].map(k=>[k,after[k]]));
writeJson('release-v5.json',{schemaVersion:1,capturedAt:new Date().toISOString(),...identities,config:sealedConfig,backendSourceManifest,buildReceipt,rustValidation:config.freshRustReceipt,immutableRelease:build.immutableRelease,selectedRustTestsPassed:rustValidation.selectedRustTestsPassed,rustTestsReexecuted:true,freshNodeTestsPassed:0,reusedNodeTestsPassed:242,reusedFocusedTestsPassed:39,typecheckReexecuted:false,...QUALIFICATION});
const frozen=requireFrozen({repositoryDirectory,expectedHead});
console.log(JSON.stringify({manifestPin:frozen.manifestPin,releasePin:frozen.releasePin,immutableRelease:frozen.release.immutableRelease,...identities,selectedRustTestsPassed:rustValidation.selectedRustTestsPassed,rustTestsReexecuted:true,freshNodeTestsPassed:0,reusedNodeTestsPassed:242,reusedFocusedTestsPassed:39,typecheckReexecuted:false,...QUALIFICATION},null,2));
