import fs from 'node:fs';
export const DRIVER_BINDING=Object.freeze(JSON.parse(fs.readFileSync(new URL('./current-driver-binding.json',import.meta.url),'utf8')));
export const DRIVER_FREEZE_FILE=DRIVER_BINDING.driverFrozenFile;
export function assertFinalDriverBinding(){
 const b=DRIVER_BINDING,hex=x=>typeof x==='string'&&/^[a-f0-9]{40}$/.test(x),sha=x=>typeof x==='string'&&/^[a-f0-9]{64}$/.test(x);
 if(b.state!=='FINAL_REUSED_C743_RELEASE_AND_CURRENT_NODE9'||!hex(b.executionSourceCommit)||b.executionSourceCommit!==b.driverSourceCommit||b.coreCodeEvidenceCommit!==b.driverSourceCommit||
 ['c743122af05faca43d9535d8c6c2a4bfb01b924f','d2407fb84055e2088e3ca6ccdb4f8b80d15a9d16'].includes(b.executionSourceCommit)||
 b.backendSourceCommit!=='c743122af05faca43d9535d8c6c2a4bfb01b924f'||b.backendBuildCommit!==b.backendSourceCommit||b.driverSourceSha256!=='d61b7cd057e1a2a55191fe6c407b3058b5ff9205141eaf9290d16da42b028db1'||
 b.backendBinarySha256!=='dae1f873878ee5212e8d84cd57cb9cc737104aaec8a644c4aeb33a49980d82cc'||b.historicalBuildDriverCommit!=='d2407fb84055e2088e3ca6ccdb4f8b80d15a9d16'||b.historicalBuildDriverSourceSha256!=='5ba46ae4a729c4ae8514721284da0f91d796aff247cd7c017b206790be88e2d9'||
 !b.driverFrozenFile||!Number.isSafeInteger(b.driverManifestBytes)||b.driverManifestBytes<1||!sha(b.driverManifestSha256)||!Array.isArray(b.currentEvidence)||b.currentEvidence.length!==1||b.currentEvidence[0].tests!==232||!b.staticGeometryCompatibilityProof)
 throw Error('PENDING_PROBE20_CURRENT_EXECUTION_HEAD_DRIVER232_AND_FRESH_FULL16: Root actual binding, full16 and approved source are required before any mode');
}
