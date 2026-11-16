import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {open,readFile,writeFile} from 'node:fs/promises';
const base=import.meta.dirname,exe='F:/mir2-builds/canonical-companion-gateway/debug/deps/mir2_gateway-e26a7f60d1bf00f1.exe';
const cases=['native_resume','web_transport_close_tests','web_explicit_leave_route_tests','abnormal_teardown_persistence_tests','web_auth_peer_bucket_tests','reconnect_session_store','capacity'];
const results=[],passed=new Set();
for(const filter of cases){const path=base+'/regression-'+filter+'.log',log=await open(path,'wx');let child;
  try{child=spawn(exe,[filter,'--test-threads=1','--color','never'],{windowsHide:true,stdio:['ignore',log.fd,log.fd]});}finally{await log.close();}
  const code=await new Promise(resolve=>{child.once('error',()=>resolve(1));child.once('exit',code=>resolve(code));});const text=await readFile(path,'utf8');
  const match=/test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored/.exec(text);
  const result={filter,exitCode:code,passed:match?Number(match[2]):0,failed:match?Number(match[3]):null,ignored:match?Number(match[4]):null};results.push(result);
  for(const row of text.matchAll(/^test (\S+) \.\.\. ok$/gm))passed.add(row[1]);
  console.log(JSON.stringify(result));if(code!==0||!match||Number(match[2])===0)break;
}
await writeFile(base+'/regression-result.json',JSON.stringify({cases:results,uniquePassed:[...passed].sort(),uniquePassedCount:passed.size},null,2)+'\n',{flag:'wx'});
assert.equal(results.length,cases.length);assert(results.every(row=>row.exitCode===0&&row.failed===0&&row.ignored===0));
