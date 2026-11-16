import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';
const project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const repo=path.dirname(project),dir='C:/mir2-build/p01-completion-20261024/root-shared-experience-parse-cache-validation-v1',base='C:/mir2-build/p01-completion-20261024';
const rustup='C:/Users/Administrator/.cargo/bin/rustup.exe';
const baseline=JSON.parse(fs.readFileSync(base+'/backend-source-account-store-wait-diagnostics-v1.json','utf8'));
const nodeBaseline=JSON.parse(fs.readFileSync(base+'/capacity-aoi-received-state-driver-frozen-v1.json','utf8'));
const changed='mir2-web3/apps/simulation/src/runtime/shared_experience_profile.rs';
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const pin=file=>{const b=fs.readFileSync(file);return{file,bytes:b.length,sha256:hash(b)};};
const snapshot=()=>({backend:baseline.files.map(x=>({...pin(path.join(repo,x.path)),file:x.path})),driver:nodeBaseline.files.map(x=>({...pin(path.join(project,x.file)),file:x.file}))});
const write=(name,value)=>fs.writeFileSync(path.join(dir,name),JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const before=snapshot();
for(const a of before.backend){const b=baseline.files.find(x=>x.path===a.file);if(a.file!==changed&&(a.sha256!==b.sha256||a.bytes!==b.bytes))throw Error('Unrelated backend delta '+a.file);}
if(JSON.stringify(before.driver)!==JSON.stringify(nodeBaseline.files))throw Error('Node driver changed');
write('source-before-v1.json',before);
const checks=[
 {name:'format',args:['run','1.95.0','rustfmt','--check','--edition','2021','apps/simulation/src/runtime/shared_experience_profile.rs']},
 {name:'experience-profile',crate:'mir2-simulation',filter:'runtime::shared_experience_profile::tests::',min:13},
 {name:'store-durability',crate:'mir2-simulation',filter:'config::config_account_store_durability_tests::',min:2},
 {name:'guild-clock-store',crate:'mir2-simulation',filter:'guild_clock_file_configs_share_store_mutex_and_frozen_state',min:1},
 {name:'admission-lock-order',crate:'mir2-gateway',filter:'owner_health_routing_repository_admission_can_acquire_zone_while_personal_tick_waits',min:1},
 {name:'kill-award-commit',crate:'mir2-gateway',filter:'shared_in_process_runtime_emits_gain_experience_after_kill_award_commit',min:1},
 {name:'guild-kill-durability',crate:'mir2-gateway',filter:'guild_kill_source_',min:2},
];
const results=[],startedAt=new Date().toISOString();
for(const check of checks){
 const args=check.args??['run','1.95.0','cargo','test','--locked','--offline','--jobs','4','-p',check.crate,'--lib',check.filter,'--','--nocapture','--test-threads=1'];
 const stdout=path.join(dir,check.name+'-stdout-v1.log'),stderr=path.join(dir,check.name+'-stderr-v1.log');
 const out=fs.openSync(stdout,'wx'),err=fs.openSync(stderr,'wx'),t=performance.now();
 const child=spawn(rustup,args,{cwd:project,windowsHide:true,stdio:['ignore',out,err]});
 const result=await new Promise(resolve=>{let spawnError=null;child.on('error',e=>{spawnError=String(e);resolve({exitCode:null,signal:null,error:spawnError});});child.on('exit',(code,signal)=>resolve({exitCode:code,signal,error:spawnError}));});
 fs.closeSync(out);fs.closeSync(err);
 const output=fs.readFileSync(stdout,'utf8');
 const totals=[...output.matchAll(/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)].map(m=>({status:m[1],passed:Number(m[2]),failed:Number(m[3]),ignored:Number(m[4]),measured:Number(m[5]),filteredOut:Number(m[6])}));
 const passed=result.exitCode===0&&result.error===null&&(!check.filter||(totals.length===1&&totals[0].status==='ok'&&totals[0].passed>=check.min&&totals[0].failed===0&&totals[0].ignored===0));
 const record={name:check.name,command:[rustup,...args],...result,durationMs:performance.now()-t,totals,passed,logs:[pin(stdout),pin(stderr)]};results.push(record);write(check.name+'-execution-v1.json',record);console.log(JSON.stringify({name:check.name,exitCode:result.exitCode,totals,passed}));if(!passed)break;
}
const after=snapshot();write('source-after-v1.json',after);
const receipt={schemaVersion:1,state:'WORKING_TREE_CANDIDATE',startedAt,endedAt:new Date().toISOString(),changedBackendInput:changed,unchangedBackendInputs:49,driverInputs:9,sourceUnchanged:JSON.stringify(before)===JSON.stringify(after),checks:results,passed:results.length===checks.length&&results.every(x=>x.passed)&&JSON.stringify(before)===JSON.stringify(after),capacityAcceptance:false};write('root-focused-receipt-v1.json',receipt);console.log(JSON.stringify({passed:receipt.passed,sourceUnchanged:receipt.sourceUnchanged,checks:results.length,capacityAcceptance:false}));process.exitCode=receipt.passed?0:1;