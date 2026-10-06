import {spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
const root='mir2-web3/apps/game-client/platform-android/target/quest-ingress-20261002/';
const out='final-source';
const results=JSON.parse(readFileSync(root+out+'/results.json','utf8'));
for(const gate of ['windows-tooltip','windows-completed','windows-quest','windows-projection',
 'windows-gateway-quest','windows-exact-ack','java','api31-debug','api31-preview']){
 const child=spawnSync('node',[root+'check-gates.mjs',gate,out],{stdio:'inherit',timeout:3600000});
 results.push({gate,status:child.status,error:child.error?.message,finished:new Date().toISOString()});
 writeFileSync(root+out+'/results.json',JSON.stringify(results,null,2)+'\n');
 // Broad Mac Windows filters remain failures, never reclassified as green.
 // Finish independent platform gates and retain every original output.
 if(child.status!==0&&!['windows-tooltip','windows-completed','windows-quest'].includes(gate))process.exit(1);
}
console.log(JSON.stringify({sourceGatesFinished:true,allWindowsFiltersAccepted:false,APKOrOnlineOrDeviceAccepted:false}));
