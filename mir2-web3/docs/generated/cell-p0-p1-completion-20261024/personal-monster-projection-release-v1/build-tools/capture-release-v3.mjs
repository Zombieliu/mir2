import fs from 'node:fs/promises';
import path from 'node:path';
import { options,requireFrozen,pin,sha,read,absent,samePin,MANIFEST,RECEIPT,RELEASE,BUILD_LOG,TARGET,BINARY,IMMUTABLE_BINARY,OLD_BINARY_SHAS,BUILD_ARGS,GIT_BASE } from './source-lib-v3.mjs';
const selected=options();await absent([RELEASE]);
const {manifest,manifestBytes,current}=await requireFrozen(selected),receipt=await read(RECEIPT);
if(receipt.schemaVersion!==1||receipt.kind!=='personal-monster-projection-v1-owned-cargo-build'||receipt.success!==true||receipt.exit?.code!==0||receipt.exit?.signal||receipt.error||receipt.postSourceFailure||receipt.sourceUnchangedBeforeAfter!==true||
  receipt.sourceCommit!==current.sourceCommit||receipt.backendSourceManifest!==MANIFEST||receipt.backendInputCount!==44||receipt.sourceManifestFileSha256!==sha(manifestBytes)||receipt.sourceManifestSha256!==manifest.sourceManifestSha256||JSON.stringify(receipt.arguments)!==JSON.stringify(BUILD_ARGS)||receipt.command!=='cargo'||receipt.buildJobs!==4||!['x64','amd64'].includes(receipt.visualStudio?.targetArch)||!/[\\/]VC[\\/]Tools[\\/]MSVC[\\/]/i.test(receipt.visualStudio?.linkExecutable??'')||
  receipt.cwd!==path.join(current.repository,'mir2-web3')||receipt.targetDirectory!==TARGET||receipt.binary?.path!==BINARY||
  !Number.isSafeInteger(receipt.startedAtMs)||!Number.isSafeInteger(receipt.finishedAtMs)||receipt.finishedAtMs<=receipt.startedAtMs||receipt.durationMs!==receipt.finishedAtMs-receipt.startedAtMs||!receipt.toolchain?.rustc?.startsWith('rustc 1.95.0 ')||!receipt.toolchain?.cargo?.startsWith('cargo 1.95.0 '))throw Error('Successful complete owned build/source receipt is required');
const buildLog=await pin(BUILD_LOG),binary=await pin(IMMUTABLE_BINARY),log=await fs.readFile(BUILD_LOG,'utf8');
if(receipt.buildLog.path!==BUILD_LOG||!samePin(buildLog,receipt.buildLog)||receipt.immutableBinary.path!==IMMUTABLE_BINARY||!samePin(binary,receipt.immutableBinary)||!samePin(binary,receipt.binary)||OLD_BINARY_SHAS.includes(binary.sha256)||!/Finished `release` profile/.test(log)||/^error(?:\[|:)/m.test(log))throw Error('Fresh executable or complete Release build log binding differs');
const release={schemaVersion:1,capturedAt:new Date().toISOString(),profile:'release',sourceCommit:current.sourceCommit,baseCommit:GIT_BASE,
  backendSourceManifest:MANIFEST,backendInputCount:manifest.files.length,sourceManifestSha256:manifest.sourceManifestSha256,sourceManifestFileSha256:sha(manifestBytes),
  binary,buildLog,buildReceipt:await pin(RECEIPT),buildArguments:BUILD_ARGS,toolchain:receipt.toolchain,visualStudio:receipt.visualStudio,buildJobs:4,
  sourceVerifiedBeforeAndAfterBuild:true,ignoredInclusiveUntrackedBackendCount:0,
  scope:'Fresh personal/shared-monster projection Release candidate; readonly Zone ownership API and focused projection tests are explicitly source-bound. Fresh immutable executable, complete Cargo log/actual exit, explicit committed44-input source before/after binding including the three additional existing projection/AOI test files and preserved original four-None fixture proof; preserves468/a430 and older461f/855b evidence. Build success is not lifecycle or capacity acceptance.'};
await fs.writeFile(RELEASE,JSON.stringify(release,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({release:await pin(RELEASE),sourceCommit:current.sourceCommit,binary,buildLog}));
