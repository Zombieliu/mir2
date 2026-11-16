import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';

const [repository, output, expectedHead] = process.argv.slice(2);
if (!repository || !output || !/^[a-f0-9]{40}$/.test(expectedHead ?? '')) throw Error('repository, new output directory and exact HEAD required');
const project = path.join(repository, 'mir2-web3');
const git = args => execFileSync('git', ['-c','gc.auto=0','-c','maintenance.auto=false', ...args], {cwd:repository, encoding:'utf8', windowsHide:true, env:{...process.env,GIT_NO_LAZY_FETCH:'1'}}).trim();
const clean = () => { if (git(['rev-parse','HEAD']) !== expectedHead || git(['status','--porcelain','--untracked-files=no'])) throw Error('HEAD/tracked inputs must remain frozen'); };
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = async file => { const bytes = await fs.readFile(file); return {file,bytes:bytes.length,sha256:sha(bytes)}; };
const names = [
  'playtest-capacity-soak.mjs','playtest-capacity-soak.test.mjs','playtest-capacity-soak-core.mjs',
  'playtest-capacity-policy.test.mjs','playtest-capacity-observability.mjs','playtest-capacity-observability.test.mjs',
  'playtest-capacity-monitor.mjs','playtest-capacity-monitor.test.mjs','playtest-capacity-telemetry.mjs','playtest-capacity-telemetry.test.mjs',
  'capacity-ordinary-supplies.mjs','capacity-ordinary-supplies.test.mjs','capacity-ordinary-protection.mjs','capacity-ordinary-protection.test.mjs','playtest-capacity-prepare.mjs','playtest-capacity-prepare.test.mjs','playtest-multiplayer-smoke.mjs',
  'playtest-native-resume-smoke.mjs','playtest-load-smoke.mjs','playtest-catalog-transport.mjs',
  'quest-agent/protocol-funding.mjs','quest-agent/test-protocol-funding.mjs',
  'quest-agent/protocol-play.mjs','quest-agent/test-protocol-play.mjs',
  'quest-agent/protocol-combat.mjs','quest-agent/test-protocol-combat.mjs',
  'quest-agent/protocol-supplies.mjs','quest-agent/test-protocol-supplies.mjs',
  'quest-agent/protocol-navigation.mjs','quest-agent/protocol-observation.mjs','quest-agent/test-protocol-observation.mjs','quest-agent/protocol-loadout.mjs',
];
const source = async () => Promise.all(names.map(async name => {
  const relative = 'mir2-web3/apps/web/scripts/' + name, bytes = await fs.readFile(path.join(repository,relative));
  return {path:relative,bytes:bytes.length,sha256:sha(bytes)};
}));
const definitions = [
  {name:'complete-preparation-and-protected-navigation',args:['--test',...['capacity-ordinary-supplies.test.mjs','capacity-ordinary-protection.test.mjs','playtest-capacity-prepare.test.mjs'].map(name=>'apps/web/scripts/'+name)]},
  {name:'ordinary-economy-and-navigation-dependencies',args:['--test',...['test-protocol-funding.mjs','test-protocol-play.mjs','test-protocol-combat.mjs','test-protocol-supplies.mjs','test-protocol-observation.mjs'].map(name=>'apps/web/scripts/quest-agent/'+name)]},
];
await fs.mkdir(output); clean();
const startedAt = new Date().toISOString(), sourceBefore = await source(), checks=[];
for (const check of definitions) {
  clean();
  const stdoutFile=path.join(output,check.name+'.stdout.log'), stderrFile=path.join(output,check.name+'.stderr.log');
  const stdout=await fs.open(stdoutFile,'wx'), stderr=await fs.open(stderrFile,'wx');
  const started=Date.now(); let error=null;
  const result=await new Promise(resolve=>{
    const child=spawn(process.execPath,check.args,{cwd:project,windowsHide:true,stdio:['ignore',stdout.fd,stderr.fd]});
    child.once('error',e=>{error=e.message;resolve({exitCode:null,signal:null});});
    child.once('exit',(exitCode,signal)=>resolve({exitCode,signal}));
  });
  await stdout.close();await stderr.close();
  const text=await fs.readFile(stdoutFile,'utf8'), summary={};
  for(const name of ['tests','pass','fail','cancelled','skipped','todo']) {
    const match=text.match(new RegExp('^# '+name+' ([0-9]+)$','m'));summary[name]=match?Number(match[1]):null;
  }
  checks.push({name:check.name,command:[process.execPath,...check.args],...result,error,elapsedMs:Date.now()-started,summary,stdout:await pin(stdoutFile),stderr:await pin(stderrFile)});
  process.stdout.write(JSON.stringify({check:check.name,...result,summary})+'\n');
}
clean(); const sourceAfter=await source();
const receipt={schemaVersion:1,scope:'Fresh selected complete preparation/protection suites and adjacent ordinary economy/navigation suites and protocol observations on32 named inputs. The previous828 full-tool check on ee06 is historical; unchanged main suites are not freshly reexecuted here. Static runtime imports separately frozen; no real gameplay/capacity/P0 acceptance.',
  expectedHead,startedAt,endedAt:new Date().toISOString(),sourceBefore,sourceAfter,
  sourceUnchanged:JSON.stringify(sourceBefore)===JSON.stringify(sourceAfter),checks,
  allSelectedTestsPassed:checks.every(check=>check.exitCode===0&&check.signal===null&&check.error===null&&check.summary.fail===0&&check.summary.cancelled===0&&check.summary.skipped===0&&check.summary.todo===0),
  capacityAccepted:false,p0Accepted:false,realCapacityRunPerformed:false};
await fs.writeFile(path.join(output,'root-focused-receipt-v1.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
process.stdout.write(JSON.stringify({receipt:await pin(path.join(output,'root-focused-receipt-v1.json')),allSelectedTestsPassed:receipt.allSelectedTestsPassed})+'\n');
if(!receipt.allSelectedTestsPassed||!receipt.sourceUnchanged)process.exitCode=1;
