import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const base='C:/mir2-build/p01-completion-20261024', output=path.join(base,'formal03-analysis'), runRoot=path.join(base,'formal-native-v2-ladder-03');
const d=JSON.parse(fs.readFileSync(path.join(output,'tracks-v1.json'),'utf8'));
const report=JSON.parse(fs.readFileSync(path.join(runRoot,'reports',d.runId+'.report.json'),'utf8'));
const peer=d.actor.slice(0,-2)+'01', home=d.reportHome, inside=p=>p.x>=home.x&&p.x<=home.x+2&&p.y>=home.y&&p.y<=home.y+2;
const dist=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y));
const allOwn=d.contextActions.filter(r=>['walk','run'].includes(r.kind)&&r.plannedGameplay);
const movements=d.movementActions.map(row=>{
  const t=row.receiptTransformMatches[0], previous=allOwn.findLast(r=>r.sentAtMs<row.sentAtMs), owner=t?.before.position;
  const observed=d.playerLifecycle.findLast(r=>r.actor===d.actor&&r.monotonicMs<=row.sentAtMs);
  const ownPeer=d.transforms.findLast(r=>r.actor===peer&&r.monotonicMs<=row.sentAtMs);
  const auxiliary=d.contextActions.filter(r=>['chat','turn'].includes(r.kind)&&r.sentAtMs>previous?.sentAtMs&&r.sentAtMs<row.sentAtMs);
  return {at:row.at,phase:row.phase,kind:row.kind,status:row.status,sentAtMs:row.sentAtMs,scheduledAtMs:row.scheduledAtMs,
    latencyMs:row.latencyMs,origin:owner,target:row.plannedTarget,actual:row.actual,movedCells:row.movedCells,
    receiptSequence:t?.sequence,receiptMonoMs:t?.monotonicMs,
    originInsideHome:owner?inside(owner):null,originIsCorner:owner?[0,2].includes(owner.x-home.x)&&[0,2].includes(owner.y-home.y):null,
    lastSuccessfulMovementSentGapMs:previous?row.sentAtMs-previous.sentAtMs:null,
    theoreticalPrimedByLastMovement:previous?row.sentAtMs<previous.sentAtMs+1200:null,
    interveningAuxiliary:auxiliary.map(r=>({kind:r.kind,at:r.at,sentAtMs:r.sentAtMs})),
    ownAoiPlayersLastReceipt:observed?{at:observed.at,monotonicMs:observed.monotonicMs,sequence:observed.sequence,source:observed.source,
      truncated:observed.after.truncated,count:observed.after.count,players:observed.after.players}:null,
    peer01TrueOwnerLastReceipt:ownPeer?{at:ownPeer.at,monotonicMs:ownPeer.monotonicMs,position:ownPeer.after.position,source:ownPeer.source}:null};
});
const counts=rows=>({moves:rows.length,walk:rows.filter(r=>r.kind==='walk').length,run:rows.filter(r=>r.kind==='run').length,
  walkInsideHome:rows.filter(r=>r.kind==='walk'&&r.originInsideHome).length,walkOutsideHome:rows.filter(r=>r.kind==='walk'&&!r.originInsideHome).length,
  walkWithRecentMovePriming:rows.filter(r=>r.kind==='walk'&&r.theoreticalPrimedByLastMovement).length,
  originalBlocked:rows.filter(r=>r.status==='navigation-blocked').length});
const slowFile=path.join(runRoot,'slow-stage-public.jsonl'), slowBytes=fs.readFileSync(slowFile), slow=slowBytes.toString('utf8').trim().split(/\r?\n/).map(line=>JSON.parse(line));
function observedPhase(at){return report.phaseWindows.find(p=>at>=p.startedAtMs&&at<=p.endedAtMs)?.name??(at<report.phaseWindows[0].startedAtMs?'startup-before-driver':'drain-after-bootstrap');}
const slowRows=slow.map((row,index)=>({line:index+1,...row,receiverObservedPhase:observedPhase(row.observedAtMs),estimatedSpanStartIfImmediateLogMs:row.observedAtMs-row.durationMs}));
const groups={};for(const row of slowRows){const key=row.receiverObservedPhase+'|'+row.stage;(groups[key]??=[]).push(row.durationMs);}
const groupSummary=Object.entries(groups).map(([key,values])=>({key,count:values.length,minMs:Math.min(...values),maxMs:Math.max(...values),atLeast500:values.filter(v=>v>=500).length}));
const result={schemaVersion:1,runId:d.runId,actor:d.actor,home,window:d.interval,counts:counts(movements),
  byPhase:[...new Set(movements.map(r=>r.phase))].map(phase=>({phase,...counts(movements.filter(r=>r.phase===phase))})),
  precedingMovements:allOwn.filter(r=>r.at<=d.interval.previousWindowEmittedAtMs).slice(-5),
  auxiliary:d.contextActions.filter(r=>['chat','turn'].includes(r.kind)&&r.at>d.interval.previousWindowEmittedAtMs).map(({kind,at,sentAtMs})=>({kind,at,sentAtMs})),
  observedPlayerOverlap:movements.filter(r=>(r.ownAoiPlayersLastReceipt?.players??[]).some(p=>p.objectId!==1000&&p.x>=252&&p.x<=254&&p.y>=671&&p.y<=673))
    .map(r=>({at:r.at,kind:r.kind,players:r.ownAoiPlayersLastReceipt.players.filter(p=>p.objectId!==1000&&inside(p))})),
  peer01OwnerBounds:movements.reduce((b,r)=>{const p=r.peer01TrueOwnerLastReceipt?.position;if(p){b.minX=Math.min(b.minX,p.x);b.maxX=Math.max(b.maxX,p.x);b.minY=Math.min(b.minY,p.y);b.maxY=Math.max(b.maxY,p.y);}return b;},{minX:Infinity,maxX:-Infinity,minY:Infinity,maxY:-Infinity}),
  movements,slowStageCensus:{file:slowFile,bytes:slowBytes.length,sha256:crypto.createHash('sha256').update(slowBytes).digest('hex'),rows:slow.length,
    timestampScope:'observedAtMs is launcher receive time, not exact source span endpoint. Receiver-phase labels are diagnostic; only>=50ms whitelist records are present, not all runtime calls. Absence of a named spectator span cannot exclude spectator work.',groups:groupSummary,records:slowRows},
  limitations:['Owner AOI player projection covers players, not per-decision monsters/terrain/rejected-cells/full obstacle union.','Theoretical priming is calculated from prior successful action send+1200ms and does not substitute for an actual recorded planner branch.','No public recording payload was read by this extraction.']};
const out=path.join(output,'actor00-track-and-slow-v1.json');fs.writeFileSync(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({out,bytes:fs.statSync(out).size,counts:result.counts,byPhase:result.byPhase,overlaps:result.observedPlayerOverlap,peer01Bounds:result.peer01OwnerBounds,auxiliary:result.auxiliary,slow:result.slowStageCensus.groups}));
