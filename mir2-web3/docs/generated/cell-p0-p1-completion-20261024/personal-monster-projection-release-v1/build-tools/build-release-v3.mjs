import fs from 'node:fs/promises';
import { createWriteStream, constants } from 'node:fs';
import path from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import { options,requireFrozen,pin,sha,absent,samePin,MANIFEST,RECEIPT,BUILD_LOG,TARGET,BINARY,IMMUTABLE_BINARY,OLD_BINARY_SHAS,BUILD_ARGS } from './source-lib-v3.mjs';
const selected=options(),before=await requireFrozen(selected);
await absent([RECEIPT,BUILD_LOG,IMMUTABLE_BINARY]);
const env={...process.env,CARGO_TARGET_DIR:TARGET,CARGO_BUILD_JOBS:'4',GIT_NO_LAZY_FETCH:'1'};
// rustup run fails if the already-installed toolchain is absent; this step does
// not download a replacement and the actual Cargo build remains --offline.
const toolchain={rustc:execFileSync('rustup',['run','1.95.0','rustc','--version'],{cwd:before.current.repository,windowsHide:true,env,encoding:'utf8'}).trim(),
  cargo:execFileSync('rustup',['run','1.95.0','cargo','--version'],{cwd:before.current.repository,windowsHide:true,env,encoding:'utf8'}).trim()};
if(!toolchain.rustc.startsWith('rustc 1.95.0 ')||!toolchain.cargo.startsWith('cargo 1.95.0 ')) throw Error('Actual installed toolchain does not match the original pinned1.95.0 build');
const targetArch=String(env.VSCMD_ARG_TGT_ARCH??'').toLowerCase();if(!['x64','amd64'].includes(targetArch))throw Error('Load the existing Visual Studio amd64 development shell before the authorized build');
const linkExecutable=execFileSync('where.exe',['link.exe'],{cwd:before.current.repository,windowsHide:true,env,encoding:'utf8'}).split(/\r?\n/).find(Boolean);if(!/[\\/]VC[\\/]Tools[\\/]MSVC[\\/]/i.test(linkExecutable??''))throw Error('The active linker must be the Visual Studio MSVC linker');
const visualStudio={targetArch,linkExecutable};
const cwd=path.join(before.current.repository,'mir2-web3'),startAtMs=Date.now(),log=createWriteStream(BUILD_LOG,{flags:'wx'});
let child,exit,error;
console.log(JSON.stringify({type:'buildStarted',startedAtMs:startAtMs,sourceCommit:selected.expectedHead,log:BUILD_LOG,command:'cargo',arguments:BUILD_ARGS,cwd}));
try {
  child=spawn('cargo',BUILD_ARGS,{cwd,windowsHide:true,env,stdio:['ignore','pipe','pipe']});
  const append=source=>source.on('data',chunk=>{if(!log.write(chunk)){source.pause();log.once('drain',()=>source.resume());}});
  append(child.stdout);append(child.stderr);
  exit=await new Promise((resolve,reject)=>{child.once('error',reject);child.once('close',(code,signal)=>resolve({code,signal}));log.once('error',reject);});
} catch(caught){error=String(caught.message);if(child&&!child.killed)child.kill();}
finally{await new Promise((resolve,reject)=>{log.once('error',reject);log.end(resolve);});}
const endAtMs=Date.now(),buildLog=await pin(BUILD_LOG),logText=await fs.readFile(BUILD_LOG,'utf8');
let after,binary,immutableBinary,postSourceFailure;
try{after=await requireFrozen(selected);if(!samePin({bytes:before.manifestBytes.length,sha256:sha(before.manifestBytes)},{bytes:after.manifestBytes.length,sha256:sha(after.manifestBytes)}))throw Error('Source manifest changed during build');}
catch(caught){postSourceFailure=String(caught.message);}
let success=!error&&exit?.code===0&&!exit?.signal&&!postSourceFailure&&/Finished `release` profile/.test(logText)&&!/^error(?:\[|:)/m.test(logText);
if(success){try{binary=await pin(BINARY);const stat=await fs.stat(BINARY);if(OLD_BINARY_SHAS.includes(binary.sha256)||stat.mtimeMs<startAtMs-2000||stat.mtimeMs>endAtMs+2000)throw Error('Build did not produce a fresh non-855b/non-a430 Release executable');
  await fs.mkdir(path.dirname(IMMUTABLE_BINARY),{recursive:true});await fs.copyFile(BINARY,IMMUTABLE_BINARY,constants.COPYFILE_EXCL);immutableBinary=await pin(IMMUTABLE_BINARY);if(!samePin(binary,immutableBinary))throw Error('Immutable Release copy differs');}
  catch(caught){error=String(caught.message);success=false;}}
const receipt={schemaVersion:1,kind:'personal-monster-projection-v1-owned-cargo-build',startedAtMs:startAtMs,finishedAtMs:endAtMs,durationMs:endAtMs-startAtMs,
  sourceCommit:selected.expectedHead,backendSourceManifest:MANIFEST,sourceManifestFileSha256:sha(before.manifestBytes),sourceManifestSha256:before.manifest.sourceManifestSha256,
  backendInputCount:before.manifest.files.length,command:'cargo',arguments:BUILD_ARGS,cwd,targetDirectory:TARGET,toolchain,visualStudio,buildJobs:4,
  exit:exit??null,error:error??null,postSourceFailure:postSourceFailure??null,sourceUnchangedBeforeAfter:!postSourceFailure,buildLog,binary,immutableBinary,success,
  scope:'Complete merged stdout/stderr log and actual Cargo exit tied to exact before/after committed44-input source and fresh immutable executable; no lifecycle/capacity acceptance.'};
await fs.writeFile(RECEIPT,JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({receipt:await pin(RECEIPT),success,exit,binary:immutableBinary}));if(!success)process.exitCode=1;
