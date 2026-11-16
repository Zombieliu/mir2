import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir,copyFile,constants} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const run=import.meta.dirname,repo='E:/mir2-player-journey/mir2-web3';
const out=repo+'/docs/evidence/r16-native-guild-20261108';
const read=async p=>JSON.parse((await readFile(p,'utf8')).replace(/^\uFEFF/,''));
const sha=b=>createHash('sha256').update(b).digest('hex');
const source=await read(run+'/source-final-after.json'),build=await read(run+'/native-build-final-result.json');
assert.equal(build.exitCode,0);assert.equal(source.pins.length,13);
await mkdir(run+'/candidate-final');
const exe=run+'/candidate-final/mir2-platform-windows.exe';
await copyFile('F:/mir2-builds/canonical-companion-native/release/mir2-platform-windows.exe',exe,constants.COPYFILE_EXCL);
const binary=await readFile(exe),binaryHash=sha(binary);assert.notEqual(binaryHash,(await read(run+'/candidate-first.json')).sha256);
const candidate={schema:'mir2.r16.native-guild.candidate.v1',recordedAt:new Date().toISOString(),path:exe,bytes:binary.length,sha256:binaryHash,buildExitCode:0,canonicalManifestsAndLocksUnchanged:true,directSourcePinsStable:13,nativeExecuted:false,installed:false,standaloneInstaller:false,requiresExistingR16Resources:true,publicDeployment:false};
await writeFile(run+'/candidate-final.json',JSON.stringify(candidate,null,2)+'\n',{flag:'wx'});
const specs=[['client-bevy','client-social-v2-final.log',70],['client-bevy','client-guild-v2-final.log',143],['windows','windows-guild-v2-qualified.log',29],['runtime','runtime-guild-v2-final.log',6]];
const tests=new Set(),skipped=[],runs=[];
for(const [crate,file,expected]of specs){
 const text=await readFile(run+'/'+file,'utf8');const found=[...text.matchAll(/^test (\S+) \.\.\. ok\r?$/gm)].map(m=>m[1]);assert.equal(found.length,expected,file);
 assert.match(text,/test result: ok\./);assert(!/test result: FAILED\./.test(text));
 found.forEach(name=>tests.add(crate+':'+name));
 const ignored=[...text.matchAll(/^test (\S+) \.\.\. ignored, (.+)\r?$/gm)].map(m=>({crate,name:m[1],reason:m[2].trim()}));skipped.push(...ignored);
 runs.push({crate,log:file,passed:expected,ignored:ignored.length,tests:found});
}
assert.equal(tests.size,198);assert.equal(skipped.length,2);
await mkdir(out);await mkdir(out+'/tools');await writeFile(out+'/.gitattributes','# Preserve raw verification output.\n* -text whitespace=cr-at-eol\n*.log whitespace=-blank-at-eol,-blank-at-eof,cr-at-eol\n',{flag:'wx'});
const artifacts=[];
async function put(name){const bytes=await readFile(run+'/'+name);assert(!/sk-proj-[A-Za-z0-9_-]{20,}|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{30,}/.test(bytes.toString('utf8')),name);await writeFile(out+'/'+name,bytes,{flag:'wx'});artifacts.push({path:name,bytes:bytes.length,sha256:sha(bytes)});}
for(const file of [
 ...specs.map(row=>row[1]),'client-social-initial.log','client-guild-panel-initial.log','client-guild-v2-initial.log','client-guild-v2-qualified.log',
 'source-final-before.json','source-final-after.json','source-before.json','native-build-result.json','native-build-final.log','native-build-final-result.json','candidate-first.json','candidate-final.json','preservation.json'
])await put(file);
await copyFile(run+'/pin-final-source.mjs',out+'/tools/pin-final-source.mjs',constants.COPYFILE_EXCL);
await copyFile(run+'/publish.mjs',out+'/tools/publish.mjs',constants.COPYFILE_EXCL);
const result={schema:'mir2.r16.native-guild.verification.v1',recordedAt:new Date().toISOString(),parent:source.head,branch:source.branch,
 changes:['Native GuildMemberChange handles real delta statuses without clearing unrelated ranks/members','Typed UserInformation identity reaches the social cursor, using realId rather than AOI/selection IDs','Exact local permission context retains option/value/requester/guild/scope and actual status7 cursor across metadata','Typed roster read retains a fresh255 cursor, exact definitely-unsent cancellation and retry after real bounded-channel rejection','Pending permission/read/rename intents are protected from ordinary queue eviction; original24/8 limits remain'],
 sourcePins:source.pins,locks:source.locks,reference:source.reference,
 tests:{runs,selectedPassExecutions:248,uniquePasses:tests.size,newTests:69,adjacentUniquePasses:129,skipped},candidate,artifacts,
 initialFailures:[
  {log:'client-social-initial.log',passed:63,failed:6,reason:'Legacy tests used non255 full-table fixtures, nonexistent changeType echoes, or omitted real member identity. Fixtures corrected; malformed echoes remain negative tests.'},
  {log:'client-guild-panel-initial.log',passed:32,failed:1,reason:'Legacy nonrename-status fixture treated a populated permission-rank payload as create-rank/status8. Valid create-rank shape and missing-viewer rejection now explicit.'},
  {log:'client-guild-v2-initial.log',passed:141,failed:2,reason:'Existing UI tests expected bare GuildRequestInfo intents after the typed local read conversion.'},
  {log:'client-guild-v2-qualified.log',passed:142,failed:1,reason:'The remaining UI page fixture had no authoritative guild membership. It now seeds an ordinary GuildStatus before requesting its roster.'},
 ],
 reviewFixes:['Preliminary release candidate retained uninstalled: review found definitely-unsent read leakage and ordinary-intent eviction of pending work','Real bounded production try_send Full test proves release before acceptance; fresh accepted read stays pending until a real complete roster','Six reverse-order queue-pressure tests and a real panel refresh test cover the missing paths'],
 scope:{backendChanged:false,serverWireChanged:false,saveSchemaChanged:false,servicesStarted:false,desktopControlled:false,nativeWindowExecuted:false,modelCalled:false,publicDeployment:false,candidate100:false,accepted:false},
 open:['Native window/live-protocol and current candidate cross-client gameplay','Two explicit GPU/GDI real-asset checks','Guild member cap200 versus shared valid1000','Exact recruit/create-rank local context and structured failure acknowledgements','Rank-index renumbering concurrency boundary','Global cross-social budget and Web convergence','First Web owner unexpected-login cold-entry failure','Remaining pixel/language/input and release gates']};
await writeFile(out+'/verification.json',JSON.stringify(result,null,2)+'\n',{flag:'wx'});
await writeFile(out+'/README.md',[
 '# Native 公会数据与请求生命周期', '',
 '本轮仅修改 Windows R16 代码基线的客户端消费、队列和发送桥；服务器规则、协议与存档 schema 均未变。', '',
 '- 70 项社交、143 项公会限定、29 项 Windows 与6项实际入包队列测试通过；248 次选定执行去重为198项（69新增、129相邻）。',
 '- 两项真实素材 GPU/GDI 图像测试明确未执行，具体名字和原因见 [verification.json](verification.json)。',
 '- 完整名册仅255；其它状态按成员／职级增量处理。权限只由真实status7的actor、嵌套index、目标bit及新鲜cursor确认，元数据不会抹掉已到达回执。',
 '- 普通真实UserInformation经typed分支进入SocialModel，使用realId与name绑定；不拿AOI对象或选角槽替代身份。',
 '- 读请求确定未发即精确释放；真实8槽bounded通道满载后可再次发一次，已接收请求保持等待。24槽普通队列不再淘汰已占8槽额度的受保护公会请求。',
 '- 原失败及首个未资格候选保留；最后候选通过原manifest/lock的Rust1.95 release/offline/locked构建，13直接源码及6个manifest/lock前后稳定，不声称全工作区输入闭包。',
 '- [候选](candidate-final.json)是单EXE，需要既有R16资源；没有启动、安装或替换原游戏。原三服和Windows EXE保持，未调用模型或部署。',
 '- 成员上限、招募／建职阶的精确失败回执、索引重编号并发、Web共用预算、像素／多语言及最终真人验收仍开放。', '',
 'tools 是实际本机历史脚本，含当时工作树与构建路径；不含密钥或私人测试账号。', '',
].join('\n'),{flag:'wx'});
console.log(JSON.stringify({published:true,uniqueTests:tests.size,skipped:skipped.length,sourcePins:source.pins.length,artifacts:artifacts.length,candidateSha256:binaryHash,candidateBytes:binary.length}));
