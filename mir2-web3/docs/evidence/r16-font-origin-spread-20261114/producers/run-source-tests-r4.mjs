import fs from 'node:fs';
import cp from 'node:child_process';
import crypto from 'node:crypto';
import path from 'node:path';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const root='C:/r16-network-isolation-20261114/tests-r4';
const repo='E:/mir2-player-journey', web=repo+'/mir2-web3/apps/web';
fs.mkdirSync(root);
const beforePath=root+'/worker-6ae.js';
const beforeSource=cp.execFileSync('git',['show','6ae96fa6017d511255ac46216c034594b6fe500f:mir2-web3/apps/web/public/mir2-asset-worker.js'],{cwd:repo});
fs.writeFileSync(beforePath,beforeSource,{flag:'wx'});
const cases=[
 {name:'before',files:['scripts/test-asset-worker-fallback.mjs'],args:['--test-name-pattern=SC origin split'],env:{MIR2_ASSET_WORKER_TEST_SOURCE:beforePath}},
 {name:'after',files:['scripts/test-asset-worker-fallback.mjs','scripts/test-bevy-runtime-font-prewarm.mjs','scripts/test-asset-prewarm-request.mjs','scripts/test-asset-worker-missing-retry.mjs','scripts/test-asset-worker-lifecycle.mjs','scripts/test-bevy-runtime-initializer.mjs'],args:[],env:{}},
 {name:'routes',files:['scripts/test-r16-quest-navigation.mjs','scripts/test-r16-route-correction.mjs','scripts/test-route-authority-wait.mjs'],args:[],env:{}}
];
const results=[];
for(const c of cases){
 const args=['--test','--test-reporter=tap',...c.args,...c.files];
 const run=cp.spawnSync(process.execPath,args,{cwd:web,env:{...process.env,...c.env},encoding:'utf8',timeout:30000});
 const log=(run.stdout??'')+(run.stderr??'');
 fs.writeFileSync(root+'/'+c.name+'.log',log,{flag:'wx'});
 const counts={};
 for(const key of ['tests','pass','fail','cancelled','skipped']){
  const match=log.match(new RegExp('^# '+key+' (\\d+)$','m'));
  counts[key]=match?Number(match[1]):null;
 }
 results.push({name:c.name,args,exitCode:run.status,signal:run.signal,spawnError:run.error?.code??null,counts,logSha256:hash(log),inputs:c.files.map(file=>({path:file,sha256:hash(fs.readFileSync(path.join(web,file)))}))});
}
const receipt={createdAt:new Date().toISOString(),head:cp.execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim(),baseline:'6ae96fa6017d511255ac46216c034594b6fe500f',baselineWorkerSha256:hash(beforeSource),candidateWorkerSha256:hash(fs.readFileSync(web+'/public/mir2-asset-worker.js')),results};
fs.writeFileSync(root+'/receipt.json',JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(results.map(({name,exitCode,counts})=>({name,exitCode,counts}))));
if(results[0].exitCode!==1||results[0].counts.fail!==4||results.slice(1).some(row=>row.exitCode!==0))process.exitCode=1;
