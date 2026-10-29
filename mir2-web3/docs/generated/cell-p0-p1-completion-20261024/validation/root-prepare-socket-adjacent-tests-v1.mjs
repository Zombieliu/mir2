import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
const project=process.cwd(), root='C:/mir2-build/p01-completion-20261024';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const previousFile=path.join(project,'docs/generated/cell-p0-p1-completion-20261024/validation/p01-typed-attack-and-adjacent-final-04.log');
const previous=await fs.readFile(previousFile);
const oldNames=[...new Set([...previous.toString('utf8').matchAll(/^test (\S+) \.\.\./gm)].map(row=>row[1]))].sort();
if(oldNames.length!==159)throw Error('Original adjacent set must contain159 unique tests');
const tests=[...new Set([...oldNames,'web::tests::socket_serial_guard_blocks_zone_bypass_until_action_finishes','web::tests::slow_stage_threshold_accepts_only_positive_milliseconds'])].sort();
if(tests.length!==161||tests.some(name=>!/^\w+(?:::\w+)+$/.test(name)))throw Error('Invalid exact selection');
const files=[];
for(const file of ['apps/gateway/src/web.rs','apps/gateway/src/web_live_overflow_tests.rs','apps/gateway/src/web_transport_close_tests.rs']){const b=await fs.readFile(path.join(project,file));files.push({file,bytes:b.length,sha256:sha(b)});}
const manifest={schemaVersion:1,preparedAt:new Date().toISOString(),scope:'Exact previous159-test typed-attack/adjacent regression plus the existing serial-guard and slow-stage threshold tests. New7diagnostics have a separate focused run; this selection does not replace them.',previousLog:{file:previousFile,bytes:previous.length,sha256:sha(previous)},sourceFiles:files,expectedTotal:161,expectedPassed:159,expectedIgnored:2,tests};
await fs.writeFile(path.join(root,'socket-diagnostics-adjacent-selection-v1.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({tests:tests.length,sourceFiles:files.length,expectedIgnored:2}));
