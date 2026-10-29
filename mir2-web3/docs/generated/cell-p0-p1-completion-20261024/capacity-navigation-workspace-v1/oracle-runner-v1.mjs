import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import {performance} from 'node:perf_hooks';
const directory='C:/mir2-build/p01-completion-20261024/root-navigation-workspace-oracle-v1';
await fs.mkdir(directory,{recursive:true});
const originalFile='C:/mir2-build/p01-completion-20261024/protocol-navigation-workspace-v1/protocol-navigation.mjs.before-workspace-v1.txt';
const candidateFile='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/apps/web/scripts/quest-agent/protocol-navigation.mjs';
const oldCopy=path.join(directory,'original-navigation.mjs');
await fs.writeFile(oldCopy,await fs.readFile(originalFile),{flag:'wx'});
const old=await import(pathToFileURL(oldCopy));
const candidate=await import(pathToFileURL(candidateFile));
let seed=0x364a91fd;
const random=()=>{seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return(seed>>>0)/4294967296};
let compared=0,errorsCompared=0;
const held=[];
for(let i=0;i<3000;i++){
 const width=2+Math.floor(random()*27),height=2+Math.floor(random()*23);
 const blocked=new Uint8Array(width*height);
 for(let k=0;k<blocked.length;k++)blocked[k]=random()<0.23?1:0;
 const point=()=>({x:Math.floor(random()*width),y:Math.floor(random()*height)});
 const start=point(),target=point();
 const options={map:{mapFileName:'oracle',type:100,width,height,blocked},start,target,desiredDistance:Math.floor(random()*3),dynamicObstacles:Array.from({length:Math.floor(random()*12)},point),staticWalkableOverrides:Array.from({length:Math.floor(random()*5)},point),maxExpanded:[1,4,30,width*height][i%4]};
 const expected=old.findProtocolWalkPath(options),actual=candidate.findProtocolWalkPath(options);
 assert.deepEqual(actual,expected,'exact original path on case '+i);
 if(i%41===0)held.push({value:actual,encoded:JSON.stringify(actual)});
 compared++;
}
for(const maxExpanded of [NaN,0,-3,Infinity,'4']){
 const options={map:{mapFileName:'edge',width:4,height:5,blocked:new Uint8Array(20)},start:{x:0,y:0},target:{x:3,y:4},maxExpanded};
 assert.deepEqual(candidate.findProtocolWalkPath(options),old.findProtocolWalkPath(options));compared++;
}
for(const start of [{x:-1,y:0},{x:5,y:3},{x:NaN,y:0}]){
 const options={map:{mapFileName:'edge',width:4,height:5,blocked:new Uint8Array(20)},start,target:{x:3,y:4}};
 const capture=fn=>{try{return{value:fn(options)}}catch(e){return{error:{name:e.name,message:e.message}}}};
 assert.deepEqual(capture(candidate.findProtocolWalkPath),capture(old.findProtocolWalkPath));errorsCompared++;
}
for(const record of held)assert.equal(JSON.stringify(record.value),record.encoded,'previously returned path not mutated');
const map=await candidate.loadProtocolCollisionMap('0');
const queries=Array.from({length:200},(_,i)=>({map,start:{x:232+(i%7),y:609+(i%5)},target:{x:240+(i%9),y:616+(i%11)},maxExpanded:4000}));
const benchmark=fn=>{let totalPoints=0;const started=performance.now();for(const query of queries){const value=fn(query);totalPoints+=value?.length??0}return{durationMs:performance.now()-started,totalPoints}};
for(const q of queries.slice(0,8))assert.deepEqual(candidate.findProtocolWalkPath(q),old.findProtocolWalkPath(q));
const originalBenchmark=benchmark(old.findProtocolWalkPath),candidateBenchmark=benchmark(candidate.findProtocolWalkPath);
assert.equal(candidateBenchmark.totalPoints,originalBenchmark.totalPoints);
const pin=async file=>{const b=await fs.readFile(file);return{file,bytes:b.length,sha256:crypto.createHash('sha256').update(b).digest('hex')}};
const result={schemaVersion:1,finishedAt:new Date().toISOString(),original:await pin(originalFile),candidate:await pin(candidateFile),randomExactPathCases:compared,errorEdgeCases:errorsCompared,priorReturnedPathsChecked:held.length,allComparisonsPassed:true,realMap:{width:map.width,height:map.height,mapFileName:map.mapFileName},benchmark:{queries:queries.length,maxExpanded:4000,original:originalBenchmark,candidate:candidateBenchmark,informationalOnly:true,isolatedProcesses:false,capacityClaim:false},p0Accepted:false};
await fs.writeFile(path.join(directory,'result-v1.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify(result,null,2));