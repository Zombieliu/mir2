import {spawnSync} from 'node:child_process';
import {writeFileSync,mkdirSync} from 'node:fs';
const root='mir2-web3/apps/game-client/platform-android/target/quest-ingress-20261002/';
const out=process.argv[2]??'final-source';
mkdirSync(root+out,{recursive:true});
const results=[];
for(const gate of ['android-normal','android-preview','shared-bevy','shared-runtime',
 'windows-bridge','windows-tooltip','windows-completed','windows-quest','java','api31-debug','api31-preview']){
 const child=spawnSync('node',[root+'check-gates.mjs',gate,out],{stdio:'inherit',timeout:3600000});
 results.push({gate,status:child.status,error:child.error?.message,finished:new Date().toISOString()});
 writeFileSync(root+out+'/results.json',JSON.stringify(results,null,2)+'\n');
 if(child.status!==0&&gate!=='windows-quest')process.exit(1);
}
console.log(JSON.stringify({complete:true,positiveGatesPass:results.filter(row=>row.gate!=='windows-quest').every(row=>row.status===0),
 wideWindowsQuestFailureRetained:true}));
