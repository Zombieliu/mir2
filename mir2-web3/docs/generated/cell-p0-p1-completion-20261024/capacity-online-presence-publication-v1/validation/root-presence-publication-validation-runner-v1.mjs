import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';
const repository='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey',project=path.join(repository,'mir2-web3');
const [configFile,directory]=process.argv.slice(2);
if(!configFile||!directory)throw Error('config and new output directory required');
const config=JSON.parse(await fs.readFile(configFile,'utf8'));
if(!Array.isArray(config.sources)||!Array.isArray(config.checks)||config.sources.length!==new Set(config.sources).size||config.checks.length!==new Set(config.checks.map(x=>x.name)).size)throw Error('Invalid config');
await fs.mkdir(directory,{recursive:false});
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const pin=async file=>{const b=await fs.readFile(file);return{file,bytes:b.length,sha256:sha(b)}};
const source=async()=>Promise.all([...config.sources].sort().map(async relative=>{const b=await fs.readFile(path.join(repository,relative));return{path:relative,bytes:b.length,sha256:sha(b)}}));
const sourceBefore=await source(),startedAt=new Date().toISOString(),results=[];
for(const check of config.checks){
 const args=check.args??['run','1.95.0','cargo','test','--locked','--offline','--jobs','4','-p',check.crate,'--lib',check.filter,'--','--nocapture','--test-threads=1'];
 const stdoutFile=path.join(directory,check.name+'-stdout-v1.log'),stderrFile=path.join(directory,check.name+'-stderr-v1.log');
 const out=await fs.open(stdoutFile,'wx'),err=await fs.open(stderrFile,'wx'),began=new Date().toISOString();let error=null;
 const child=spawn('rustup',args,{cwd:project,windowsHide:true,stdio:['ignore',out.fd,err.fd]});
 const result=await new Promise(resolve=>{child.on('error',e=>{error=String(e);resolve({exitCode:null,signal:null})});child.on('exit',(exitCode,signal)=>resolve({exitCode,signal}))});await out.close();await err.close();
 const stdoutText=await fs.readFile(stdoutFile,'utf8');
 const totals=[...stdoutText.matchAll(/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)].map(m=>({status:m[1],passed:Number(m[2]),failed:Number(m[3]),ignored:Number(m[4]),measured:Number(m[5]),filteredOut:Number(m[6])}));
 const passed=result.exitCode===0&&result.signal===null&&error===null&&(check.name==='format'||(totals.length===1&&totals[0].passed>=check.minimum&&totals[0].failed===0&&totals[0].ignored===0));
 const record={name:check.name,startedAt:began,endedAt:new Date().toISOString(),command:['rustup',...args],...result,error,stdout:await pin(stdoutFile),stderr:await pin(stderrFile),totals,passed};results.push(record);console.log(JSON.stringify({name:check.name,...result,error,totals,passed}));if(!passed)break;
}
const sourceAfter=await source(),sourceUnchanged=JSON.stringify(sourceBefore)===JSON.stringify(sourceAfter),allSelectedTestsPassed=results.length===config.checks.length&&results.every(x=>x.passed)&&sourceUnchanged;
const selectedRustTestsPassed=results.reduce((n,c)=>n+c.totals.reduce((s,t)=>s+t.passed,0),0);
const record={schemaVersion:1,scope:'Fresh selected Rust/source qualification for certified online presence publication; no capacity or whole workspace acceptance',startedAt,endedAt:new Date().toISOString(),sourceBefore,sourceAfter,sourceUnchanged,sourceInputCount:sourceBefore.length,config:await pin(configFile),runner:await pin(new URL(import.meta.url)),checks:results,selectedRustTestsPassed,allSelectedTestsPassed,capacityAccepted:false,p0Accepted:false,formalAcceptanceEligible:false};
const file=path.join(directory,'root-focused-receipt-v1.json');await fs.writeFile(file,JSON.stringify(record,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({receipt:await pin(file),allSelectedTestsPassed,selectedRustTestsPassed,checks:results.length}));process.exitCode=allSelectedTestsPassed?0:1;
