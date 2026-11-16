import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024', previous=path.join(base,'socket-diagnostics-v1'), root=path.join(base,'socket-diagnostics-v2');
const pin=async file=>{const bytes=await fs.readFile(file);return{file,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')}};
const expected=new Map([
  ['build-socket-diagnostics-v1.mjs','7eb341979e8ae14771ff80ba1ca001de0cbbdcfac639a003fee175cf39b23272'],
  ['capture-socket-diagnostics-release-v1.mjs','ea6b02e379d0faa7d7ebf72b888b7b287a871736e9edf0b2a7a9d846d2b4bea3'],
  ['pin-socket-diagnostics-build-v1.mjs','a9c3bb551b36d29436e55243bf59ee8e9c452654a1beb33f98bf6c0753403b7b'],
  ['README-prepared-v1.md','4777032eccd362c14aacfdd4d1c3ab04b30bb9884c6b642e892c5b0797534ce2'],
  ['source-lib.mjs','5988e4190276c2498580dc9664c4ef35daf99b365892b8ce4d3964b4d3064ffe'],
  ['source-review-v1.json','687dec7769fc8b2a83fc16bed532cb37c16005020b614fb77bfb3783b207e6c7']
]);
const preserved=[];for(const[name,sha256]of expected){const row=await pin(path.join(previous,name));if(row.sha256!==sha256)throw Error('Prepared-v1 preimage changed');preserved.push(row);}
const failure=await pin('C:/mir2-build/cell-preproduction-p0-p1-20261024/p0-socket-diagnostics-focused-v1.log');
if(failure.bytes!==62567||failure.sha256!=='d9191d6a310a2e6c7e008046fea14aee62afd08149c38d0f293051051b15972c')throw Error('Original compile-failure log changed');
const log=await fs.readFile(failure.file,'utf8');if((log.match(/^error\[E0061\]:/gm)??[]).length!==4||!/could not compile `mir2-gateway` \(lib test\) due to 4 previous errors/.test(log))throw Error('Original four compile-time arity errors missing');
const files=[];for(const name of ['source-lib.mjs','pin-socket-diagnostics-build-v2.mjs','build-socket-diagnostics-v2.mjs','capture-socket-diagnostics-release-v2.mjs','README-prepared-v2.md','prepare-catalog-v2.mjs'])files.push(await pin(path.join(root,name)));
const result={schemaVersion:2,state:'prepared-only-not-source-or-build-captured',preparedAt:new Date().toISOString(),expectedNewSourceCommit:null,
  reason:'Original39-input assumption was invalidated by the focused compile-v1. Four existing fixtures need the newly required diagnostics argument. The real committed dependency closure must include those two files rather than hiding them behind a compatibility wrapper.',
  oldPreparedV1RawPreserved:preserved,originalCompileFailure:{...failure,classification:'Compile failure E0061 at four old fixture calls; not a runtime test failure',e0061Count:4,diagnosticTestsExecutedInThatAttempt:false},
  expectedBackendClosure:{total:41,rust:36,cargoAndDotCargo:5,preservedOriginalInputs:39,original38Unchanged:true,originalMutableInput:'mir2-web3/apps/gateway/src/web.rs',
    addedFixtures:[{path:'mir2-web3/apps/gateway/src/web_live_overflow_tests.rs',newNoneArguments:3},{path:'mir2-web3/apps/gateway/src/web_transport_close_tests.rs',newNoneArguments:1}],
    fixtureBaselineCommit:'0998105c30f35502b8b569b82fb7942a5d356c89',fixtureRule:'Remove only the four approved None constructor-argument lines; restored final committed Git-LF bytes must exactly equal the099 blobs. No assertion or other byte may change.'},
  files,outputs:{backendSourceManifest:path.join(base,'backend-source-socket-diagnostics-v1.json'),release:path.join(base,'release-socket-diagnostics-v1.json'),reasonForV1OutputNames:'First actual socket-diagnostics source/build candidate has not been captured; prepared-v2 corrects the tools, not an executed build.'},
  guards:['Explicit final new full40 commit and currentHEAD equality','Tracked tree clean','Ignored-inclusive untracked backend rejection','Exact base-to-HEAD41 path closure','Current raw and committedGitLF binding','Old38 raw/Git hashes unchanged','Two fixture inverse transforms exact099','Source unchanged before and after owned build','Complete unique Cargo log plus actual exit0 and installed1.95.0 toolchain','Fresh immutable executable must not equal preserved855b','Exclusive creation: old evidence cannot be overwritten'],
  execution:{sourcePinRun:false,buildRun:false,releaseCaptureRun:false,realmStarted:false,trackedSourceWritten:false,syntaxChecksOnly:true,capacityAccepted:false}};
const output=path.join(root,'prepared-v2.json');await fs.writeFile(output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({manifest:await pin(output),files:files.length,preserved:preserved.length,expectedInputs:41,compiledOrCaptured:false}));
