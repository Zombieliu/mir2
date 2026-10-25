import { options,requireFrozen } from './source-lib-v1.mjs';
const args=process.argv.slice(2);if(args.shift()!=='--readonly-sourceguard')throw Error('Only --readonly-sourceguard is permitted; no bind/capture/build/provision/run mode');
const result=await requireFrozen(options(args));
console.log(JSON.stringify({readonlySourceGuardPassed:true,backendBuildCommit:result.backendSourceCommit,currentExecutionHead:result.executionSourceCommit,driverSourceCommit:result.driverSourceCommit,backendInputs:result.manifest.files.length,backendSourceSha256:result.manifest.sourceManifestSha256,driverSourceSha256:result.current.driverSourceSha256,allFiftyBackendInputsUnchanged:true,rebuildPerformed:false,capacityAccepted:false}));
