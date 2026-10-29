import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
const base='C:/mir2-build/p01-completion-20261024',project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const output=path.join(base,'native-chase-preference-verification-v1');
const main=path.join(project,'apps/web/scripts/playtest-capacity-soak.mjs'),test=path.join(project,'apps/web/scripts/playtest-capacity-soak.test.mjs');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex'),pin=async file=>{const b=await fs.readFile(file);return{file,bytes:b.length,sha256:sha(b)}};
const oldFreeze=JSON.parse(await fs.readFile(path.join(base,'capacity-native-witness-approach-driver-frozen-v1.json'),'utf8'));
async function closure(){const files=[];for(const old of oldFreeze.files){const p=await pin(path.join(project,old.file));files.push({file:old.file,bytes:p.bytes,sha256:p.sha256});if(!/playtest-capacity-soak(?:\.test)?\.mjs$/.test(old.file)&&(p.bytes!==old.bytes||p.sha256!==old.sha256))throw Error('Other seven source inputs changed');}return{files,sourceSha256:sha(JSON.stringify(files))};}
function counts(log){const result={};for(const key of ['tests','pass','fail','cancelled','skipped']){const match=log.match(new RegExp('^# '+key+' (\\d+)$','m'));if(!match)throw Error('Missing TAP '+key);result[key]=Number(match[1]);}return result;}
async function run(name,args,timeout){const startedAt=Date.now(),result=spawnSync(process.execPath,args,{cwd:project,encoding:'utf8',windowsHide:true,timeout,maxBuffer:2*1024*1024});const log=(result.stdout??'')+(result.stderr??'');const file=path.join(output,name+'.log');await fs.writeFile(file,log,{flag:'wx'});return{args,node:process.version,startedAt,elapsedMs:Date.now()-startedAt,exitCode:result.status,signal:result.signal,error:result.error?String(result.error):null,counts:counts(log),log:await pin(file)};}
await fs.mkdir(output,{recursive:true});
const before=await closure();
if((await pin(main)).sha256!=='1cf9da1746db1b23d00b13cf9dd65c14ac72c155bd558621bf7767d4e6376f3f'||(await pin(test)).sha256!=='5c6efc9a49a45550d4c4aed25a5083dc1afaf3e146da33ffd9c372f8902d66db')throw Error('Reviewed final source/test pins differ');
const mode=process.argv.slice(2).join(' ');
if(mode==='--old-red-current-green'){
 const current=await fs.readFile(main),old=await fs.readFile(path.join(base,'native-chase-preference-main-before-v1.mjs.txt'));
 if(old.length!==144416||sha(old)!=='2251b3a9cbdc391ecc0e68f17f7568468d677550a0f7425604a1ec0a7a485a42')throw Error('Original source preimage differs');
 await fs.writeFile(path.join(output,'main-current-before-red-v1.mjs.txt'),current,{flag:'wx'});
 const args=['--test','--test-name-pattern=^native chase preference','apps/web/scripts/playtest-capacity-soak.test.mjs'];
 let red,restored=false;
 try{await fs.writeFile(main,old);red=await run('old-red-v1',args,60000);}finally{await fs.writeFile(main,current);restored=(await fs.readFile(main)).equals(current);if(!restored)throw Error('Current source restore failed');}
 if(red.exitCode===0||red.counts.fail<1||red.counts.tests!==13||red.error||red.signal)throw Error('Original implementation did not yield the expected behavioral failures');
 if(JSON.stringify(await closure())!==JSON.stringify(before))throw Error('Source closure changed after original red');
 const green=await run('current-green-v1',args,60000);
 const after=await closure();
 if(green.exitCode!==0||green.counts.tests!==13||green.counts.pass!==13||green.counts.fail||green.counts.cancelled||green.counts.skipped||green.error||green.signal||JSON.stringify(before)!==JSON.stringify(after))throw Error('Current focused regression or source-preservation check failed');
 const receipt={schemaVersion:1,createdAt:new Date().toISOString(),mode,scope:'Same thirteen focused cases against exact original main, then current main. Current test and all other inputs are unchanged; old main is restored in finally before green.',sourceBefore:before,sourceAfter:after,temporaryOriginal:{bytes:old.length,sha256:sha(old)},currentSourceRestored:restored,redExpectedBehavioralFailure:true,red,green,backendChanged:false,capacityAccepted:false};
 await fs.writeFile(path.join(output,'red-green-receipt-v1.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({mode,red:red.counts,green:green.counts,currentSourceRestored:restored,sourceSha256:after.sourceSha256,receipt:await pin(path.join(output,'red-green-receipt-v1.json'))}));
}else if(mode==='--full-current-source'){
 const args=['--test','apps/web/scripts/playtest-capacity-soak.test.mjs','apps/web/scripts/playtest-capacity-observability.test.mjs','apps/web/scripts/playtest-capacity-monitor.test.mjs','apps/web/scripts/playtest-capacity-telemetry.test.mjs'];
 console.log(JSON.stringify({phase:'full-regression-started',testsExpected:200,sourceSha256:before.sourceSha256}));
 const result=await run('full-current-v1',args,900000),after=await closure(),unchanged=JSON.stringify(before)===JSON.stringify(after);
 const receipt={schemaVersion:1,createdAt:new Date().toISOString(),...result,sourceBefore:before,sourceAfter:after,sourcePinsUnchangedAfterRun:unchanged,backendChanged:false,capacityAccepted:false};
 await fs.writeFile(path.join(output,'full-current-receipt-v1.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({phase:'full-regression-finished',...result.counts,exitCode:result.exitCode,elapsedMs:result.elapsedMs,sourcePinsUnchangedAfterRun:unchanged,sourceSha256:after.sourceSha256,receipt:await pin(path.join(output,'full-current-receipt-v1.json'))}));
 if(result.exitCode!==0||result.counts.tests!==200||result.counts.pass!==200||result.counts.fail||result.counts.cancelled||result.counts.skipped||result.error||result.signal||!unchanged)process.exitCode=1;
}else throw Error('Explicit --old-red-current-green or --full-current-source required');
