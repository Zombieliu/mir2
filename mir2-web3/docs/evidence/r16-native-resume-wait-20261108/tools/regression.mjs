import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {open,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const base=import.meta.dirname;
const exe='F:/mir2-builds/canonical-companion-gateway/debug/deps/mir2_gateway-e26a7f60d1bf00f1.exe';
const sha=b=>createHash('sha256').update(b).digest('hex');
const before=sha(await readFile(exe));
const filters=['reconnect_handoff','native_resume','web_transport_close_tests','web_explicit_leave_route_tests','abnormal_teardown_persistence_tests','web_auth_peer_bucket_tests','reconnect_session_store','capacity'];
const results=[],passed=new Set();
for(const filter of filters){
  const path=base+'/regression-'+filter+'.log',log=await open(path,'wx');
  let child;
  try{child=spawn(exe,[filter,'--skip','web::web_native_resume_handoff','--test-threads=1','--color','never'],{windowsHide:true,stdio:['ignore',log.fd,log.fd]});}finally{await log.close();}
  const code=await new Promise(resolve=>{child.once('error',()=>resolve(1));child.once('exit',resolve);});
  const text=await readFile(path,'utf8');
  const match=/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored/.exec(text);
  const row={filter,exitCode:code,passed:match?Number(match[2]):0,failed:match?Number(match[3]):null,ignored:match?Number(match[4]):null};
  results.push(row);
  for(const test of text.matchAll(/^test (\S+) \.\.\. ok\r?$/gm))passed.add(test[1]);
  console.log(JSON.stringify(row));
  if(code!==0||!match||!row.passed)break;
}
const after=sha(await readFile(exe));
await writeFile(base+'/regression-result.json',JSON.stringify({testExecutable:exe,sha256:before,unchanged:before===after,cases:results,uniquePassed:[...passed].sort(),uniquePassedCount:passed.size,alreadyQualifiedNativeCasesSkipped:true},null,2)+'\n',{flag:'wx'});
assert.equal(before,after);
assert.equal(results.length,filters.length);
assert(results.every(row=>row.exitCode===0&&row.failed===0&&row.ignored===0&&row.passed>0));
