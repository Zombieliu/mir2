import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {openSync,closeSync} from 'node:fs';
import {mkdir,readFile,writeFile,copyFile,symlink,stat,cp,readdir} from 'node:fs/promises';
import {dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import readline from 'node:readline';
const root=dirname(fileURLToPath(import.meta.url)),repo='E:/mir2-player-journey/mir2-web3';
const oldPrivate='D:/mir2-builds/r16-parity-20261109/trade-cancel-ui-r1/private';
const sha=b=>createHash('sha256').update(b).digest('hex');
const pageSha='5fc2fe55f2c5131de3dc6e9956c1ca4a404afeeed31023dba377053d14f4720b';
assert.equal(sha(await readFile(join(repo,'apps/web/app/page.tsx'))),pageSha);
const hostSha=sha(await readFile(join(repo,'apps/web/lib/social-operation-budget-host.ts')));
const manifest=JSON.parse(await readFile(join(repo,'apps/web/lib/generated/bevy_runtime_version.json'),'utf8'));assert.equal(manifest.version,'bevy-0cd68510886134e0');
const exe='D:/mir2-builds/web-newcomer-30-20261107/mir2-gateway-v22-isolated.exe',exeSha=sha(await readFile(exe));assert.equal(exeSha,'1b705387db5c1ce14a12909dec87f8af3268eb0cc7c43957069bdfaba76327d7');
const {playableGatewayEnvironment,playableBaseEnvironment}=await import(pathToFileURL(join(repo,'apps/web/scripts/r16-playable-service-config.mjs')));
const state=JSON.parse(await readFile(join(oldPrivate,'state.local.json'),'utf8'));
const priv=join(root,'private'),data=join(oldPrivate,'data'),cwd=join(priv,'gateway-cwd');await mkdir(cwd,{recursive:true,mode:0o700});
await copyFile(join(data,'accounts.json'),join(priv,'accounts-before.local.json'));
const accountStoreBeforeSha=sha(await readFile(join(data,'accounts.json')));
const origin='http://127.0.0.1:19482',gatewayUrl='ws://127.0.0.1:19481/ws';
await writeFile(join(root,'public-config.json'),JSON.stringify({origin,gatewayUrl,realmId:state.world.realmId}),{flag:'wx'});
const src=join(root,'next-source/mir2-web3'),web=join(src,'apps/web');await mkdir(web,{recursive:true});
for(const dir of ['app','lib'])await cp(join(repo,'apps/web',dir),join(web,dir),{recursive:true});
for(const dir of ['public','scripts'])await symlink(join(repo,'apps/web',dir),join(web,dir),'junction');
const modules=join(web,'node_modules');await mkdir(modules);await cp(join(repo,'apps/web/node_modules/next'),join(modules,'next'),{recursive:true});
for(const name of await readdir(join(repo,'apps/web/node_modules'))){if(name==='next'||name.startsWith('.'))continue;await symlink(join(repo,'apps/web/node_modules',name),join(modules,name),'junction');}
for(const name of ['next.config.ts','package.json','package-lock.json','tsconfig.json','next-env.d.ts'])await copyFile(join(repo,'apps/web',name),join(web,name));
for(const dir of ['packages','config'])await symlink(join(repo,dir),join(src,dir),'junction');
for(const dir of ['game-client','ai-player'])await symlink(join(repo,'apps',dir),join(src,'apps',dir),'junction');
for(const file of ['postcss.config.js','postcss.config.mjs','tailwind.config.js','tailwind.config.ts','proxy.ts']){try{if((await stat(join(repo,'apps/web',file))).isFile())await copyFile(join(repo,'apps/web',file),join(web,file));}catch(e){if(e.code!=='ENOENT')throw e;}}
const ts=JSON.parse(await readFile(join(web,'tsconfig.json'),'utf8'));ts.include=['next-env.d.ts','**/*.ts','**/*.tsx','.next-social-budget/types/**/*.ts','.next-social-budget/dev/types/**/*.ts'];await writeFile(join(web,'tsconfig.json'),JSON.stringify(ts,null,2));
function start(command,args,options,file){const fd=openSync(join(priv,file),'wx',0o600);try{const child=spawn(command,args,{...options,windowsHide:true,stdio:['pipe',fd,fd]});child.on('error',()=>{});return child;}finally{closeSync(fd);}}
const {MIR2_PASSKEY_AUTH_SECRET,MIR2_PASSKEY_ALLOWED_ORIGINS,...gatewayEnv}=playableGatewayEnvironment(state,data);
const gateway=start(exe,[],{cwd,env:{...gatewayEnv,MIR2_GATEWAY_WEB_ADDR:'127.0.0.1:19481',MIR2_GATEWAY_TCP_ADDR:'127.0.0.1:19483',MIR2_ALLOWED_WEB_ORIGINS:origin,MIR2_SPECTATOR_ENABLED:'false',MIR2_SPECTATOR_RECORDING_ENABLED:'false'}},'gateway.log');
async function ready(url,child,ms){const end=Date.now()+ms;while(Date.now()<end){assert.equal(child.exitCode,null,'OWNED_PROCESS_EXITED');try{const r=await fetch(url,{signal:AbortSignal.timeout(1500)});await r.arrayBuffer();if(r.ok)return;}catch{}await delay(300);}throw Error('OWNED_SERVICE_READINESS_TIMEOUT');}
let webProcess;
try{
 await ready('http://127.0.0.1:19481/health',gateway,45000);
 const revision=execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim();
 webProcess=start(process.execPath,[join(modules,'next/dist/bin/next'),'dev','--webpack','--hostname','127.0.0.1','--port','19482'],{cwd:web,env:{...playableBaseEnvironment(),NODE_ENV:'development',MIR2_NEXT_STANDALONE:'1',MIR2_NEXT_DIST_DIR:'.next-social-budget',NEXT_TELEMETRY_DISABLED:'1',NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:gatewayUrl,MIR2_GATEWAY_HTTP_URL:'http://127.0.0.1:19481',NEXT_PUBLIC_MIR2_QUEST_GUIDANCE:'newcomer-v2',MIR2_DEPLOY_REVISION:revision}},'next.log');
 await ready(origin,webProcess,150000);
 await writeFile(join(root,'startup.json'),JSON.stringify({ready:true,origin,gatewayUrl,realmId:state.world.realmId,gatewayPid:gateway.pid,webPid:webProcess.pid,pageSha256:pageSha,hostSha256:hostSha,runtimeVersion:manifest.version,gatewaySha256:exeSha,gatewayBuildBase:'c404bde8a5190534f6bc83ec8a4fe78fffc8cb50',currentGatewaySourceExact:false,reusedOwnFileStore:true,accountStoreBeforeSha256:accountStoreBeforeSha,actorStarted:false,unsafeLocalCommands:false},null,2),{flag:'wx'});
 console.log(JSON.stringify({ready:true,origin,gatewayPid:gateway.pid,webPid:webProcess.pid,runtimeVersion:manifest.version}));
 const input=readline.createInterface({input:process.stdin,terminal:false});await new Promise(resolve=>{input.on('line',line=>{if(line.trim()==='shutdown'){input.close();resolve();}});input.on('close',resolve);});
}catch(e){console.log(JSON.stringify({ready:false,name:e.name,code:e.code??'STARTUP_FAILED',messageHash:sha(e.message)}));process.exitCode=1;}
finally{
 if(webProcess?.exitCode===null){webProcess.kill();await Promise.race([new Promise(r=>webProcess.once('exit',r)),delay(8000)]);}
 if(gateway.exitCode===null){gateway.stdin.end('shutdown\n');await Promise.race([new Promise(r=>gateway.once('exit',r)),delay(8000)]);if(gateway.exitCode===null)gateway.kill();}
 await copyFile(join(data,'accounts.json'),join(priv,'accounts-after.local.json'));
 await writeFile(join(root,'shutdown.json'),JSON.stringify({at:new Date().toISOString(),ownGatewayStopped:gateway.exitCode!==null,ownNextStopped:!webProcess||webProcess.exitCode!==null,pageSourceUnchanged:sha(await readFile(join(repo,'apps/web/app/page.tsx')))===pageSha,hostSourceUnchanged:sha(await readFile(join(repo,'apps/web/lib/social-operation-budget-host.ts')))===hostSha,accountStoreBeforeSha256:accountStoreBeforeSha,accountStoreAfterSha256:sha(await readFile(join(data,'accounts.json')))}));
}
