import fs from'node:fs/promises';import path from'node:path';import{fileURLToPath}from'node:url';import assert from'node:assert/strict';
const root=path.dirname(fileURLToPath(import.meta.url));
async function edit(name,fn){const file=path.join(root,name),old=await fs.readFile(file,'utf8'),next=fn(old);assert.notEqual(next,old,name);await fs.writeFile(file,next);}
await edit('prepare-inputs.mjs',s=>{
 s="import {verifyWireDiagnosticSource,wireDiagnosticIdentity,CORE_CODE_EVIDENCE_COMMIT} from './wire-diagnostics-contract.mjs';\n"+s;
 s=s.replace('const releaseBinding = await verifyRelease(), drivers = await collectDriverInputs();','const releaseBinding = await verifyRelease(), drivers = await collectDriverInputs();\nconst wireSource=await verifyWireDiagnosticSource(),wireIdentity=wireDiagnosticIdentity(wireSource);');
 s=s.replace('const files = [...releaseBinding.full16Gate.files, ...drivers.pins, ...scene.files,','const files = [...releaseBinding.full16Gate.files, ...drivers.pins, ...scene.files, ...wireSource.files,');
 s=s.replace('socketDiagnosticsReleaseUsed: true, driverFrozenManifest:','socketDiagnosticsReleaseUsed: true, coreCodeEvidenceCommit:CORE_CODE_EVIDENCE_COMMIT, sceneStaticCodeEvidenceCommit:CORE_CODE_EVIDENCE_COMMIT, wireDiagnostics:wireIdentity, driverFrozenManifest:');
 s=s.replace("...['scenario.json', 'resource-profile.json'","...['copied-baseline-inputs.json','scenario.json', 'resource-profile.json'");return s;
});
await edit('run-owned.mjs',s=>{
 s="import {runCapacityWithWireDiagnostics} from './wire-diagnostics-adapter.mjs';\nimport {verifyWireDiagnosticSource,wireDiagnosticIdentity,CORE_CODE_EVIDENCE_COMMIT,WIRE_TERMINAL} from './wire-diagnostics-contract.mjs';\n"+s;
 s=s.replace("const binding = await verifyRelease(), build = await read(path.join(root, 'gateway-build-pin.json'));","const binding = await verifyRelease(), build = await read(path.join(root, 'gateway-build-pin.json'));\nconst wireSource=await verifyWireDiagnosticSource(),wireIdentity=wireDiagnosticIdentity(wireSource);\nif(JSON.stringify(frozen.wireDiagnostics)!==JSON.stringify(wireIdentity)||frozen.coreCodeEvidenceCommit!==CORE_CODE_EVIDENCE_COMMIT)throw Error('Reviewed external diagnostics/source identity changed');");
 s=s.replace('sceneProfile: scene.profile, sceneSourceSha256:','wireDiagnostics:wireIdentity, coreCodeEvidenceCommit:CORE_CODE_EVIDENCE_COMMIT, sceneProfile: scene.profile, sceneSourceSha256:');
 s=s.replace("const { runCapacity, parseArguments } = await import(","const { CapacityClient, runCapacity, parseArguments } = await import(");
 const old="report = await runCapacity({ ...options, ...explicitMode, accountsFile: path.join(privateRoot, 'capacity-pool.json'), monitor: path.join(root, 'monitor-current.json'), output: path.join(root, 'reports'), onProgress: value => console.log(JSON.stringify(value)) });";
 assert.ok(s.includes(old));s=s.replace(old,"report = await runCapacityWithWireDiagnostics({CapacityClient,runCapacity,options:{ ...options, ...explicitMode, accountsFile: path.join(privateRoot, 'capacity-pool.json'), monitor: path.join(root, 'monitor-current.json'), output: path.join(root, 'reports'), onProgress: value => console.log(JSON.stringify(value)) },writeTerminal:terminal=>writeNew(path.join(root,WIRE_TERMINAL),terminal)});");
 s=s.replace('const reportPin = await pin(report.reportPath);','const reportPin = await pin(report.reportPath),wireTerminalPin=await pin(path.join(root,WIRE_TERMINAL));');
 s=s.replace('capacityAcceptanceAdjudicated: false });','wireTerminal:wireTerminalPin, capacityAcceptanceAdjudicated: false });');
 s=s.replace('  clearInterval(stopRelay);',"  clearInterval(stopRelay);\n  try { const terminal=await read(path.join(root,WIRE_TERMINAL));cleanup.wireDiagnostics={identity:wireIdentity,terminal:await pin(path.join(root,WIRE_TERMINAL)),statusCapturePhase:terminal.statusCapturePhase,uninstallSucceeded:terminal.uninstallSucceeded,uninstallFailed:terminal.uninstallFailed,terminalOutsideRaw:true}; } catch(error) { cleanup.wireDiagnostics={identity:wireIdentity,terminal:null,terminalOutsideRaw:true,terminalUnavailable:true}; }");
 return s;
});
console.log(JSON.stringify({root,integrated:['prepare-inputs.mjs','run-owned.mjs'],originalRunFunctionWrappedOnlyAfterExecutionGuards:true,noWriterCloseHook:true,realmStarted:false}));
