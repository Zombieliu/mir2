import {spawn} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import readline from 'node:readline';
const base=import.meta.dirname;
const {playableBaseEnvironment}=await import(pathToFileURL('E:/mir2-player-journey/mir2-web3/apps/web/scripts/r16-playable-service-config.mjs'));
const env={...playableBaseEnvironment(),NODE_ENV:'development',MIR2_NEXT_STANDALONE:'1',NEXT_TELEMETRY_DISABLED:'1',MIR2_NEXT_DIST_DIR:'.next-reconnect-proof',NEXT_PUBLIC_MIR2_QUEST_GUIDANCE:'newcomer-v2',NEXT_PUBLIC_MIR2_GATEWAY_WS_URL:'ws://127.0.0.1:19252/ws',MIR2_GATEWAY_HTTP_URL:'http://127.0.0.1:19252',MIR2_DISABLE_REQUEST_FILE_WRITES:'1'};
const fd=openSync(base+'/next.private.log','wx',0o600);let child;try{child=spawn(process.execPath,[base+'/apps/web/node_modules/next/dist/bin/next','dev','--webpack','--hostname','127.0.0.1','--port','19352'],{cwd:base+'/apps/web',env,windowsHide:true,stdio:['pipe',fd,fd]});}finally{closeSync(fd);}
let exited=false;const input=readline.createInterface({input:process.stdin,terminal:false});const stop=()=>{if(!exited)child.kill('SIGTERM');};input.on('line',line=>{if(line.trim()==='shutdown')stop();});process.once('SIGINT',stop);
const code=await new Promise(resolve=>{child.once('error',()=>resolve(1));child.once('exit',code=>{exited=true;resolve(code);});});input.close();console.log(JSON.stringify({stopped:true,code}));
