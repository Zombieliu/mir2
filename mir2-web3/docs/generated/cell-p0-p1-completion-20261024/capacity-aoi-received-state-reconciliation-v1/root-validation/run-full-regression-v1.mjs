import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawn} from 'node:child_process';
const root = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const dir = 'C:/mir2-build/p01-completion-20261024/root-aoi-received-state-validation-v1';
const baselineFile = 'C:/mir2-build/p01-completion-20261024/capacity-scout-target-waypoint-routing-driver-frozen-v1.json';
const baseline = JSON.parse(fs.readFileSync(baselineFile, 'utf8'));
const expected = new Map([
 ['apps/web/scripts/playtest-capacity-soak-core.mjs', 'c59977540640efe0a2a5f9b05e81d3ce6c526ecc3c498fbb077f886e35605ebf'],
 ['apps/web/scripts/playtest-capacity-soak.mjs', 'fe5c255f45b1010e63f98d6670307b365cc60c9cfb948e1ae6b676604500ddb6'],
 ['apps/web/scripts/playtest-capacity-soak.test.mjs', '6365f8cb988d0b36a3a6b4964b4bb66603bf1e28b13d731b2d842f22d041b1cf'],
]);
const pin = file => {const bytes = fs.readFileSync(file); return {file, bytes:bytes.length, sha256:crypto.createHash('sha256').update(bytes).digest('hex')};};
const sourcePins = () => baseline.files.map(old => {const actual=pin(path.join(root,old.file)); return {...actual,file:old.file};});
const write = (name, value) => fs.writeFileSync(path.join(dir,name), JSON.stringify(value,null,2)+'\n', {flag:'wx'});
const before=sourcePins();
for (const actual of before) { const old=baseline.files.find(f=>f.file===actual.file); if(actual.sha256 !== (expected.get(actual.file) ?? old.sha256)) throw new Error('Unexpected source pin '+actual.file); }
write('source-before-v1.json',before);
const entries=['playtest-capacity-soak.test.mjs','playtest-capacity-observability.test.mjs','playtest-capacity-monitor.test.mjs','playtest-capacity-telemetry.test.mjs'].map(x=>'apps/web/scripts/'+x);
const stdout=path.join(dir,'full-regression-stdout-v1.log'),stderr=path.join(dir,'full-regression-stderr-v1.log');
const out=fs.openSync(stdout,'wx'),err=fs.openSync(stderr,'wx');
const startedAt=new Date().toISOString(),started=performance.now();
const child=spawn(process.execPath,['--test',...entries],{cwd:root,stdio:['ignore',out,err],windowsHide:true});
const result=await new Promise((resolve,reject)=>{child.on('error',reject);child.on('exit',(code,signal)=>resolve({code,signal}));});
fs.closeSync(out);fs.closeSync(err);
const after=sourcePins();write('source-after-v1.json',after);
const output=fs.readFileSync(stdout,'utf8');
const totals=Object.fromEntries([...output.matchAll(/^# (tests|suites|pass|fail|cancelled|skipped|todo|duration_ms) (.+)$/gm)].map(m=>[m[1],Number(m[2])]));
const unchanged=JSON.stringify(before)===JSON.stringify(after);
const receipt={schemaVersion:1,startedAt,endedAt:new Date().toISOString(),durationMs:performance.now()-started,command:[process.execPath,'--test',...entries],cwd:root,actualExitCode:result.code,signal:result.signal,totals,sourcePinsUnchanged:unchanged,baseline:pin(baselineFile),logs:[pin(stdout),pin(stderr)],sourceRecords:[pin(path.join(dir,'source-before-v1.json')),pin(path.join(dir,'source-after-v1.json'))],passed:result.code===0&&totals.tests===242&&totals.pass===242&&totals.fail===0&&unchanged,capacityAcceptance:false};
write('root-full-driver-receipt-v1.json',receipt);
console.log(JSON.stringify(receipt));
process.exitCode=receipt.passed?0:1;