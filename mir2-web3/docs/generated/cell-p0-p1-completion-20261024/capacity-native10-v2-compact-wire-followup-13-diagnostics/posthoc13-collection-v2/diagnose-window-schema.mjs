import fs from 'node:fs/promises';
import {createReadStream} from 'node:fs';
import {createInterface} from 'node:readline';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024',realm='release-native-10-v2-compact-wire-feasibility-13',runId='1792912711217-e7ea81f2';
const root=path.join(base,realm),reportFile=path.join(root,'reports',runId+'.report.json'),rawFile=path.join(root,'reports',runId+'.samples.jsonl');
const report=JSON.parse(await fs.readFile(reportFile,'utf8')),rows=[],hash=crypto.createHash('sha256');let line=0,bytes=0;
const input=createReadStream(rawFile);input.on('data',chunk=>{hash.update(chunk);bytes+=chunk.length;});
for await(const text of createInterface({input,crlfDelay:Infinity})){line++;const row=JSON.parse(text);if(row.type==='window')rows.push({line,phase:row.phase,at:row.at,elapsedSeconds:row.elapsedSeconds,assessmentKeys:Object.keys(row.assessment??{}),passed:row.assessment?.passed,assessmentSeconds:row.assessment?.seconds,assessmentElapsedSeconds:row.assessment?.elapsedSeconds,startedAtMs:row.startedAtMs,endedAtMs:row.endedAtMs,keys:Object.keys(row)});}
const conditions=report.stages.map(stage=>({name:stage.name,completed:stage.completed,passed:stage.passed,controlledActors:stage.controlledActors,configuredSeconds:stage.seconds,measuredSeconds:stage.measuredSeconds,actualSeconds:(stage.endedAtMs-stage.startedAtMs)/1000,reportWindows:stage.rollingWindows.map((w,j)=>({j,passed:w.passed,seconds:w.seconds,elapsedSeconds:w.elapsedSeconds,startedAtMs:w.startedAtMs,endedAtMs:w.endedAtMs,oldConditionFailed:!w.passed||w.seconds<60||w.elapsedSeconds<(j+1)*60})),rawWindows:rows.filter(r=>r.phase===stage.name)}));
const result={schemaVersion:1,recordedAt:new Date().toISOString(),scope:'Post-stop public numeric timing/schema projection only; no input or verdict change.',runId,report:{file:reportFile,bytes:(await fs.stat(reportFile)).size,sha256:crypto.createHash('sha256').update(await fs.readFile(reportFile)).digest('hex')},raw:{file:rawFile,bytes,sha256:hash.digest('hex'),rows:line},conditions,gameVerdictUnchanged:true,capacityAccepted:false};
const out=path.join(base,'posthoc13-collection-v2','window-schema-diagnosis-v1.json');await fs.writeFile(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({output:out,runId,conditions,raw:result.raw}));
