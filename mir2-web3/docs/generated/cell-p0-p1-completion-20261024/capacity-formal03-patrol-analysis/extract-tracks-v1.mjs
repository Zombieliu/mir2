import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import readline from 'node:readline';
import crypto from 'node:crypto';
const base = 'C:/mir2-build/p01-completion-20261024', root = path.join(base,'formal-native-v2-ladder-03'), output = path.join(base,'formal03-analysis');
const runId = '1792873511986-56c2dbe6', actor = 'Lc7785700', file = path.join(root,'reports',runId+'.samples.jsonl');
const report = JSON.parse(fs.readFileSync(path.join(root,'reports',runId+'.report.json'),'utf8'));
const actions=[], transforms=[], lifecycle=[], windows=[], phases=[], coverage=[], admitted=[], otherActions=[];
const hash=crypto.createHash('sha256'); let bytes=0, rows=0;
const input=fs.createReadStream(file); input.on('data',chunk=>{bytes+=chunk.length;hash.update(chunk)});
for await(const line of readline.createInterface({input,crlfDelay:Infinity})) {
  const row=JSON.parse(line); row.line=++rows;
  if(row.type==='action') { if(row.actor===actor) actions.push(row); else if(row.plannedGameplay&&['walk','run'].includes(row.kind))otherActions.push(row); }
  if(row.type==='selfTransform')transforms.push(row);
  if(row.type==='playerLifecycle')lifecycle.push(row);
  if(row.type==='continuousWindow')windows.push(row);
  if(['phaseStarted','phaseEnded','safetyStop','continuousFailure','finished'].includes(row.type))phases.push(row);
  if(row.type==='coverage')coverage.push(row);
  if(row.type==='admitted')admitted.push(row);
}
const ownWindows=windows.filter(row=>row.actor===actor), failed=ownWindows.find(row=>!row.passed);
if(!failed||failed.activity.moves!==77||failed.activity.runs!==38)throw new Error('Unexpected03 failure');
const previous=ownWindows[ownWindows.indexOf(failed)-1], estimatedStart=failed.at-failed.seconds*1000;
const low=previous?.at??estimatedStart, high=failed.at;
const ownTransforms=transforms.filter(row=>row.actor===actor);
const moves=actions.filter(row=>row.plannedGameplay&&['walk','run'].includes(row.kind)&&row.at>low&&row.at<=high);
for(const row of moves) {
  const candidates=ownTransforms.filter(t=>t.sequence!=null&&Math.abs(t.monotonicMs-(row.sentAtMs+row.latencyMs))<.01);
  row.receiptTransformMatches=candidates.map(t=>({line:t.line,sequence:t.sequence,monotonicMs:t.monotonicMs,at:t.at,source:t.source,
    before:t.before,after:t.after,wire:t.wire}));
}
const result={schemaVersion:1,runId,actor,reportHome:report.scenario.anchors[0],raw:{file,bytes,rows,sha256:hash.digest('hex')},
  failure:failed,precedingWindow:previous,interval:{previousWindowEmittedAtMs:previous?.at,estimatedActiveStartAtMs:estimatedStart,failureEmittedAtMs:high,
    rule:'Use prior same-actor window emission as a diagnostic wall boundary; true continuous accumulation uses monotonic time. Full actions/ACKs retained for independent reconciliation.'},
  movementCounts:{walk:moves.filter(row=>row.kind==='walk').length,run:moves.filter(row=>row.kind==='run').length,total:moves.length},
  movementActions:moves,contextActions:actions.filter(row=>row.at>=low-5000&&row.at<=high),
  allOwnWindows:ownWindows,
  transforms:transforms.filter(row=>row.at>=low-5000&&row.at<=high),playerLifecycle:lifecycle.filter(row=>row.at>=low-5000&&row.at<=high),
  controlledMovement:otherActions.filter(row=>row.at>=low-5000&&row.at<=high),phaseEvents:phases,admitted,
  coverage:coverage.filter(row=>row.at>=low-5000&&row.at<=high),limitations:['No full receive trace or ordinary monster lifecycle/positions were logged for navigation decisions. Do not infer an unobserved monster blocker.','Same-tick controlled hints and full planning obstacles were not individually traced; observed peer positions can establish presence but not every send-time planner set.','No recording payload was read for this extraction.']};
await fsp.mkdir(output,{recursive:true});const out=path.join(output,'tracks-v1.json');await fsp.writeFile(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({out,bytes:fs.statSync(out).size,movementCounts:result.movementCounts,interval:result.interval,home:result.reportHome,
  allOwnWindows:ownWindows.map(row=>({at:row.at,seconds:row.seconds,phases:row.phases,passed:row.passed,moves:row.activity.moves,runs:row.activity.runs})),
  movements:moves.map(row=>({at:row.at,phase:row.phase,kind:row.kind,target:row.plannedTarget,actual:row.actual,movedCells:row.movedCells,sentAtMs:row.sentAtMs,
    receipt:row.receiptTransformMatches.map(t=>({before:t.before.position,after:t.after.position,sequence:t.sequence}))}))}));
