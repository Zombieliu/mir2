import test from 'node:test';
import assert from 'node:assert/strict';
import {createPassedProbeWindowCounter} from './passed-probe-window-contract.mjs';
function fixture() {
  let start=100000;
  const stages=[['stage-2',5],['stage-10',5],['soak',6]].map(([name,count])=>{
    const startedAtMs=start,rollingWindows=[];
    for(let i=0;i<count;i++){const wstart=start;start+=60001;rollingWindows.push({elapsedSeconds:(start-startedAtMs)/1000,startedAtMs:wstart,endedAtMs:start,passed:true,seconds:60.001,activity:{moves:80},metrics:{run:{count:40}}});}
    const stage={name,startedAtMs,endedAtMs:start,rollingWindows};start++;return stage;
  });
  const rows=stages.flatMap(stage=>stage.rollingWindows.map(w=>{const{elapsedSeconds,startedAtMs,endedAtMs,...assessment}=w;return{type:'window',phase:stage.name,startedAtMs,endedAtMs,assessment};}));
  return{report:{stages},rows};
}
test('only the supplementary16-window clock/schema projection is validated, never a game capacity verdict',()=>{
 const {report,rows}=fixture(),before=JSON.stringify({report,rows}),counter=createPassedProbeWindowCounter(report);
 rows.forEach(row=>counter.observe(row));counter.assertComplete();assert.deepEqual(['stage-2','stage-10','soak'].map(p=>counter.forPhase(p).length),[5,5,6]);assert.equal(JSON.stringify({report,rows}),before);
});
test('59.999s cannot be relabelled as a complete minute even when the report agrees',()=>{
 const {report,rows}=fixture();rows[0].endedAtMs=rows[0].startedAtMs+59999;rows[0].assessment.seconds=59.999;Object.assign(report.stages[0].rollingWindows[0],{endedAtMs:rows[0].endedAtMs,seconds:59.999,elapsedSeconds:59.999});assert.throws(()=>createPassedProbeWindowCounter(report).observe(rows[0]),/less than60s/);
});
test('actual raw assessment differences are rejected even with the same passing boolean',()=>{
 const {report,rows}=fixture();rows[0].assessment=structuredClone(rows[0].assessment);rows[0].assessment.activity.moves--;assert.throws(()=>createPassedProbeWindowCounter(report).observe(rows[0]),/differs from the original report/);
});
test('missing, extra and reordered window records cannot produce a complete projection',()=>{
 const {report,rows}=fixture(),counter=createPassedProbeWindowCounter(report);rows.slice(0,15).forEach(row=>counter.observe(row));assert.throws(()=>counter.assertComplete());counter.observe(rows[15]);assert.throws(()=>counter.observe(rows[15]),/exceed exact bounded/);assert.throws(()=>createPassedProbeWindowCounter(report).observe(rows[5]),/before all complete windows/);
});
test('phase and elapsed boundary substitutions fail closed',()=>{
 const {report,rows}=fixture();assert.throws(()=>createPassedProbeWindowCounter(report).observe({...rows[0],phase:'other'}),/Unknown or reordered/);assert.throws(()=>createPassedProbeWindowCounter(report).observe({...rows[0],startedAtMs:rows[0].startedAtMs-1}),/contiguous/);report.stages[0].rollingWindows[0].elapsedSeconds=60;assert.throws(()=>createPassedProbeWindowCounter(report).observe(rows[0]),/cumulative elapsed/);
});
test('unrelated records are ignored without recording additional data',()=>{
 const {report,rows}=fixture(),counter=createPassedProbeWindowCounter(report);counter.observe({type:'phaseStarted',name:'stage-2'});rows.forEach(row=>counter.observe(row));counter.assertComplete();assert.equal(counter.forPhase('stage-2').length,5);
});
