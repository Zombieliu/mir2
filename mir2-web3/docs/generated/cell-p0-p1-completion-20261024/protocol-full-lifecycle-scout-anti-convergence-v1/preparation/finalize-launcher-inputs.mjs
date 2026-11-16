// Explicit binding only; never prepares accounts or starts a process.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { validateCandidate,verifyPreparedReview } from './verify-inputs.mjs';
const taskRoot=path.dirname(import.meta.filename),args=process.argv.slice(2),values={};
for(let i=0;i<args.length;i+=2){if(!['--repository','--expected-head','--review-file','--review-sha256'].includes(args[i])||!args[i+1]||values[args[i]])throw Error('Explicit --repository and --expected-head are required');values[args[i]]=args[i+1];}
const repo=values['--repository'],expectedHead=values['--expected-head'];if(!repo||!/^[a-f0-9]{40}$/.test(expectedHead??''))throw Error('Actual final execution commit and mir2-web3 directory required');
const reviewed=await verifyPreparedReview(taskRoot,values['--review-file'],values['--review-sha256']);
const inputs=await validateCandidate(path.resolve(repo),taskRoot,expectedHead,{requireFinalBinding:false});
const pin=async file=>{const bytes=await fs.readFile(file);return{path:file,bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')}};
const result={schemaVersion:1,state:'final-50-input-release-bound-not-executed',boundAt:new Date().toISOString(),expectedSourceCommit:expectedHead,backendSourceCommit:inputs.releaseInput.sourceCommit,executionSourceCommit:expectedHead,driverSourceCommit:inputs.releaseInput.driverSourceCommit,backendInputCount:50,review:reviewed.review,preparation:reviewed.preparation,
  profile:await pin(path.join(taskRoot,'full-lifecycle-profile.json')),candidateReleaseBinding:await pin(path.join(taskRoot,'candidate-release-binding-v1.json')),
  releaseRecord:await pin(inputs.profile.releaseRecord),backendSourceManifest:await pin(inputs.profile.backendSourceManifest),
  binary:{path:inputs.releaseInput.executable,bytes:inputs.releaseInput.bytes,sha256:inputs.releaseInput.sha256},realmStarted:false,accountsCreated:0,capacityAccepted:false,
  scope:'Explicit reused c743/50 Release and current J Node/execution binding with unchanged six-driver ordinary full16; no account, service or gameplay execution.'};
const out=path.join(taskRoot,'launcher-final-binding-v1.json');await fs.writeFile(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({binding:await pin(out),sourceCommit:expectedHead,backendInputCount:50,realmStarted:false}));
