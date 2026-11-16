import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {openSync,closeSync} from 'node:fs';
import {mkdir,readFile,writeFile,copyFile,symlink,stat,cp,readdir} from 'node:fs/promises';
import {dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import readline from 'node:readline';
const [root,repo,oldPrivate,mode,pageSha,portInput]=process.argv.slice(2);
assert(root&&repo&&oldPrivate&&['baseline','candidate'].includes(mode)&&/^[a-f0-9]{64}$/.test(pageSha));
const port=Number(portInput);assert(Number.isInteger(port)&&port>=19711&&port<=19731);
await mkdir(root);
const sha=b=>createHash('sha256').update(b).digest('hex');
assert.equal(sha(await readFile(join(repo,'apps/web/app/page.tsx'))),pageSha);
const hostSha=sha(await readFile(join(repo,'apps/web/lib/social-operation-budget-host.ts')));
const manifest=JSON.parse(await readFile(join(repo,'apps/web/lib/generated/bevy_runtime_version.json'),'utf8'));assert.equal(manifest.version,'bevy-0cd68510886134e0');
const exe=join(dirname(fileURLToPath(import.meta.url)),'../trade-terminal-r1/fixed-gateway.exe'),exeSha=sha(await readFile(exe));assert.equal(exeSha,'60dbb6b2ad9df5165dc1472dc64c7c0281e6094a1c1208c161dfccf35f5ea974');
const {playableGatewayEnvironment,playableBaseEnvironment}=await import(pathToFileURL(join(repo,'apps/web/scripts/r16-playable-service-config.mjs')));
const state=JSON.parse(await readFile(join(oldPrivate,'realm-state.local.json'),'utf8'));
const priv=join(root,'private'),data=join(priv,'data'),cwd=join(priv,'gateway-cwd');await mkdir(cwd,{recursive:true,mode:0o700});
await mkdir(join(data,'save-recovery'),{recursive:true,mode:0o700});
const sourceAccounts=join(oldPrivate,'accounts.qualified-snapshot.json');
await copyFile(sourceAccounts,join(data,'accounts.json'));
await copyFile(join(data,'accounts.json'),join(priv,'accounts-before.local.json'));
const accountStoreBeforeSha=sha(await readFile(join(data,'accounts.json')));
const origin='http://127.0.0.1:'+(port+1),gatewayHttp='http://127.0.0.1:'+port,gatewayUrl='ws://127.0.0.1:'+port+'/ws';
await writeFile(join(root,'public-config.json'),JSON.stringify({origin,gatewayUrl,realmId:state.world.realmId,mode,pageSha256:pageSha,port}),{flag:'wx'});
const src=join(root,'next-source/mir2-web3'),web=join(src,'apps/web');await mkdir(web,{recursive:true});
for(const dir of ['app','lib'])await cp(join(repo,'apps/web',dir),join(web,dir),{recursive:true});
for(const dir of ['public','scripts'])await symlink(join(repo,'apps/web',dir),join(web,dir),'junction');
const modules=join(web,'node_modules');await mkdir(modules);await cp(join(repo,'apps/web/node_modules/next'),join(modules,'next'),{recursive:true});
for(const name of await readdir(join(repo,'apps/web/node_modules'))){if(name==='next'||name.startsWith('.'))continue;await symlink(join(repo,'apps/web/node_modules',name),join(modules,name),'junction');}
for(const name of ['next.config.ts','package.json','package-lock.json','tsconfig.json','next-env.d.ts'])await copyFile(join(repo,'apps/web',name),join(web,name));
for(const dir of ['packages','config'])await symlink(join(repo,dir),join(src,dir),'junction');
for(const dir of ['game-client','ai-player'])await symlink(join(repo,'apps',dir),join(src,'apps',dir),'junction');
for(const file of ['postcss.config.js','postcss.config.mjs','tailwind.config.js','tailwind.config.ts','proxy.ts']){try{if((await stat(join(repo,'apps/web',file))).isFile())await copyFile(join(repo,'apps/web',file),join(web,file));}catch(e){if(e.code!=='ENOENT')throw e;}}
const ts=JSON.parse(await readFile(join(web,'tsconfig.json'),'utf8'));ts.include=['next-env.d.ts','**/*.ts','**/*.tsx','.next-bidirectional-trade/types/**/*.ts','.next-bidirectional-trade/dev/types/**/*.ts'];await writeFile(join(web,'tsconfig.json'),JSON.stringify(ts,null,2));
function start(command,args,options,file){const fd=openSync(join(priv,file),'wx',0o600);try{const child=spawn(command,args,{...options,windowsHide:true,stdio:['pipe',fd,fd]});child.on('error',()=>{});return child;}finally{closeSync(fd);}}
const {MIR2_PASSKEY_AUTH_SECRET,MIR2_PASSKEY_ALLOWED_ORIGINS,...gatewayEnv}=playableGatewayEnvironment(state,data);
const gateway=start(exe,[],{cwd,env:{...gatewayEnv,MIR2_GATEWAY_WEB_ADDR:'127.0.0.1:'+port,MIR2_GATEWAY_TCP_ADDR:'127.0.0.1:'+(port+2),MIR2_ALLOWED_WEB_ORIGINS:origin,MIR2_SPECTATOR_ENABLED:'false',MIR2_SPECTATOR_RECORDING_ENABLED:'false'}},'gateway.log');
async function ready(url,child,ms){const end=Date.now()+ms;while(Date.now()<end){assert.equal(child.exitCode,null,'OWNED_PROCESS_EXITED');try{const r=await fetch(url,{signal:AbortSignal.timeout(1500)});await r.arrayBuffer();if(r.ok)return;}catch{}await delay(300);}throw Error('OWNED_SERVICE_READINESS_TIMEOUT');}
let webProcess;
try{
 await ready(gatewayHttp+'/health',gateway,45000);
 const revision=execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim();
 webProcess=start(process.execPath,[join(modules,'next/dist/bin/next'),'dev','--webpack','--hostname','127.0.0.1','--port',String(port+1)],{cwd:web,env:{...playableBaseEnvironment(),NODE_ENV:'development',MIR2_NEXT_STANDALONE:'1',MIR2_NEXT_DIST_DIR:'.next-bidirectional-trade',NEXT_TELEMETRY_DISABLED:'1',NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:gatewayUrl,MIR2_GATEWAY_HTTP_URL:gatewayHttp,NEXT_PUBLIC_MIR2_QUEST_GUIDANCE:'newcomer-v2',MIR2_DEPLOY_REVISION:revision}},'next.log');
 await ready(origin,webProcess,150000);
 await writeFile(join(root,'startup.json'),JSON.stringify({ready:true,origin,gatewayUrl,realmId:state.world.realmId,mode,sourceHead:revision,gatewayPid:gateway.pid,webPid:webProcess.pid,pageSha256:pageSha,hostSha256:hostSha,runtimeVersion:manifest.version,gatewaySha256:exeSha,gatewaySourceCommit:'ac6817dbf95ab1e5d1e98f78a9dd88b6885a8731',currentGatewaySourceExact:true,reusedOwnFileStore:false,clonedOwnFileStore:true,fixtureOpeningGoldPreparedBeforeStartup:true,accountStoreBeforeSha256:accountStoreBeforeSha,actorStarted:false,unsafeLocalCommands:false},null,2),{flag:'wx'});
 console.log(JSON.stringify({ready:true,origin,gatewayPid:gateway.pid,webPid:webProcess.pid,runtimeVersion:manifest.version}));
 const input=readline.createInterface({input:process.stdin,terminal:false});await new Promise(resolve=>{input.on('line',line=>{if(line.trim()==='shutdown'){input.close();resolve();}});input.on('close',resolve);});
}catch(e){console.log(JSON.stringify({ready:false,name:e.name,code:e.code??'STARTUP_FAILED',messageHash:sha(e.message)}));process.exitCode=1;}
finally{
 if(webProcess?.exitCode===null){webProcess.kill();await Promise.race([new Promise(r=>webProcess.once('exit',r)),delay(8000)]);}
 if(gateway.exitCode===null){gateway.stdin.end('shutdown\n');await Promise.race([new Promise(r=>gateway.once('exit',r)),delay(8000)]);if(gateway.exitCode===null)gateway.kill();}
 await copyFile(join(data,'accounts.json'),join(priv,'accounts-after.local.json'));
 await writeFile(join(root,'shutdown.json'),JSON.stringify({at:new Date().toISOString(),ownGatewayStopped:gateway.exitCode!==null,ownNextStopped:!webProcess||webProcess.exitCode!==null||webProcess.signalCode!==null,verifiedCopiedPage:sha(await readFile(join(web,'app/page.tsx')))===pageSha,sourceFixtureUnchanged:sha(await readFile(sourceAccounts))===accountStoreBeforeSha,hostSourceUnchanged:sha(await readFile(join(repo,'apps/web/lib/social-operation-budget-host.ts')))===hostSha,accountStoreBeforeSha256:accountStoreBeforeSha,accountStoreAfterSha256:sha(await readFile(join(data,'accounts.json')))}));
}
