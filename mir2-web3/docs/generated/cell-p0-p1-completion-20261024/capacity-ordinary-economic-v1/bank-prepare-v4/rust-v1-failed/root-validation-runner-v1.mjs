import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn,execFileSync} from 'node:child_process';

const [repository,output,expectedHead]=process.argv.slice(2);
if(!repository||!output||!/^[a-f0-9]{40}$/.test(expectedHead??'')) throw Error('repository, new output and exact HEAD required');
const project=path.join(repository,'mir2-web3');
const git=args=>execFileSync('git',['-c','gc.auto=0','-c','maintenance.auto=false',...args],{cwd:repository,encoding:'utf8',windowsHide:true,env:{...process.env,GIT_NO_LAZY_FETCH:'1'}}).trim();
const clean=()=>{if(git(['rev-parse','HEAD'])!==expectedHead||git(['status','--porcelain','--untracked-files=no']))throw Error('HEAD/tracked worktree must remain frozen');};
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const pin=file=>{const b=fs.readFileSync(file);return {file:path.resolve(file),bytes:b.length,sha256:sha(b)};};
const previous=JSON.parse(fs.readFileSync('C:/mir2-build/p01-completion-20261024/online-presence-reuse-release-v5/backend-source-v5.json','utf8'));
const names=previous.current.files.map(r=>r.path).sort();
if(names.length!==55||new Set(names).size!==55)throw Error('Original complete 55 backend closure required');
const source=()=>names.map(relative=>{const b=fs.readFileSync(path.join(repository,relative));return {path:relative,bytes:b.length,sha256:sha(b)};});
const cargo=(...args)=>['run','1.95.0','cargo','test','--locked','--offline','--jobs','4',...args];
const definitions=[
 {name:'format',args:['run','1.95.0','rustfmt','--edition','2021','--config','skip_children=true','--check','apps/gateway/src/web.rs']},
 {name:'gateway-owner-health-routing',args:cargo('-p','mir2-gateway','--bin','mir2-gateway','owner_health_routing_','--','--nocapture')},
 {name:'gateway-runtime-tick',args:cargo('-p','mir2-gateway','--bin','mir2-gateway','web::tests::runtime_tick_','--','--nocapture')},
 {name:'gateway-use-item-routing',args:cargo('-p','mir2-gateway','--bin','mir2-gateway','use_item_','--','--nocapture')},
 {name:'simulation-dead',args:cargo('-p','mir2-simulation','--lib','dead_player_cannot_move_attack_magic_or_use_normal_potion_until_revived','--','--nocapture')},
 {name:'simulation-potion',args:cargo('-p','mir2-simulation','--lib','use_item_packet_dynamic_crystal_normal_potion_queues_timed_restore','--','--nocapture')},
];
if(path.resolve(process.env.CARGO_TARGET_DIR??'')!==path.resolve('C:/mir2-build/cell-preproduction-p0-p1-20261024/target')||process.env.VSCMD_ARG_TGT_ARCH!=='x64')throw Error('Pinned target and VS amd64 environment required');
clean();fs.mkdirSync(output);const startedAt=new Date().toISOString(),sourceBefore=source(),checks=[];
for(const definition of definitions){
 clean();const stdoutFile=path.join(output,definition.name+'.stdout.log'),stderrFile=path.join(output,definition.name+'.stderr.log');
 const out=fs.openSync(stdoutFile,'wx'),err=fs.openSync(stderrFile,'wx');let executionError=null;
 const start=Date.now();const result=await new Promise(resolve=>{const child=spawn('rustup',definition.args,{cwd:project,windowsHide:true,env:{...process.env,CARGO_BUILD_JOBS:'4'},stdio:['ignore',out,err]});child.on('error',e=>{executionError={name:e.name,message:e.message,code:e.code??null};});child.on('close',(exitCode,signal)=>resolve({exitCode,signal,error:executionError}));});
 fs.closeSync(out);fs.closeSync(err);
 const text=fs.readFileSync(stdoutFile,'utf8')+'\n'+fs.readFileSync(stderrFile,'utf8');
 const totals=[...text.matchAll(/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)].map(m=>({status:m[1].toLowerCase(),passed:+m[2],failed:+m[3],ignored:+m[4],measured:+m[5],filteredOut:+m[6]}));
 checks.push({name:definition.name,command:['rustup',...definition.args],...result,elapsedMs:Date.now()-start,totals,stdout:pin(stdoutFile),stderr:pin(stderrFile)});
 process.stdout.write(JSON.stringify({name:definition.name,...result,totals})+'\n');
 if(result.exitCode!==0||result.signal!==null||result.error!==null)break;
}
clean();const sourceAfter=source();const receipt={schemaVersion:1,scope:'Fresh focused backend qualification; no capacity or gameplay acceptance.',expectedHead,startedAt,finishedAt:new Date().toISOString(),cwd:project,environment:{CARGO_TARGET_DIR:process.env.CARGO_TARGET_DIR,CARGO_BUILD_JOBS:'4',VSCMD_ARG_TGT_ARCH:process.env.VSCMD_ARG_TGT_ARCH,VSCMD_VER:process.env.VSCMD_VER??null},sourceBefore,sourceAfter,sourceUnchanged:JSON.stringify(sourceBefore)===JSON.stringify(sourceAfter),checks,allSelectedTestsPassed:checks.length===definitions.length&&checks.every(c=>c.exitCode===0&&c.signal===null&&c.error===null&&(c.name==='format'?c.totals.length===0:c.totals.length>0&&c.totals.every(t=>t.status==='ok'&&t.passed>0&&t.failed===0&&t.ignored===0))),capacityAccepted:false,p0Accepted:false};
const receiptFile=path.join(output,'root-focused-receipt-v1.json');fs.writeFileSync(receiptFile,JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});process.stdout.write(JSON.stringify({receipt:pin(receiptFile),allSelectedTestsPassed:receipt.allSelectedTestsPassed,selectedRustTestsPassed:checks.flatMap(c=>c.totals).reduce((n,t)=>n+t.passed,0)})+'\n');if(!receipt.allSelectedTestsPassed||!receipt.sourceUnchanged)process.exitCode=1;
