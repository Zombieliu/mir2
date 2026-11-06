import fs from 'node:fs/promises';import path from 'node:path';import {pathToFileURL} from 'node:url';
const [kind,optionsFile]=process.argv.slice(2);if(!process.connected||!['monitor','driver'].includes(kind)||!optionsFile||process.argv.length!==4)throw Error('Only the owned IPC runner may start a pilot child');
const envelope=JSON.parse(await fs.readFile(optionsFile,'utf8'));let requested=false;
process.on('message',value=>{if(value?.type==='stop-owned-'+kind&&Object.keys(value).length===1)requested=true});
const relay=setInterval(()=>{if(requested&&process.listenerCount('SIGTERM')){requested=false;process.emit('SIGTERM')}},100);
try{const modulePath=path.join(envelope.project,'apps/web/scripts',kind==='monitor'?'playtest-capacity-monitor.mjs':'playtest-capacity-soak.mjs'),api=await import(pathToFileURL(modulePath));
 if(kind==='monitor'){const result=await api.runMonitor(envelope.options);console.log(JSON.stringify(result));if(process.connected)process.send({type:'owned-monitor-finished',result})}
 else{const report=await api.runCapacity(envelope.options),summary={type:'owned-driver-finished',reportPath:report.reportPath??null,ok:report.ok??false,error:report.error??null};console.log(JSON.stringify(summary));if(process.connected)process.send(summary);process.exitCode=report.ok?0:1}
}catch(error){console.error(JSON.stringify({kind,error:error.message}));process.exitCode=1;if(process.connected)process.send({type:'owned-'+kind+'-error',error:error.message})}
finally{clearInterval(relay);if(process.connected)process.disconnect()}
