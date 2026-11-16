// Read-only bounded16/146 supplemental window projection: the frozen generic API intentionally
// retains phase/status events, not minute windows. No gameplay gate is evaluated here.
import assert from 'node:assert/strict';
const PROBE13 = Object.freeze([['stage-2',5],['stage-10',5],['soak',6]]);
const FORMAL = Object.freeze([['stage-2',5],['stage-10',5],['stage-25',5],['stage-50',5],['stage-100',5],['soak',120],['post-resume-observation',1]]);
const clock = value => Number.isSafeInteger(value) && value >= 0 && !Object.is(value,-0);
export function createPhaseWindowCounter(report,scope) {
  assert.ok(scope==='probe13'||scope==='formal','Only the frozen probe13 or full formal phase profile is supported');
  const EXPECTED=scope==='probe13'?PROBE13:FORMAL,maximum=scope==='probe13'?16:146;
  const states = new Map(EXPECTED.map(([name,required]) => {
    const stage = report.stages.find(value => value.name === name);
    assert.ok(stage && stage.rollingWindows?.length === required, 'Passing probe report must contain exactlyfixed phase counts minute windows');
    return [name,{stage,required,rows:[]}];
  }));
  let total=0,lastPhase=0;
  return {
    observe(row) {
      if (row.type !== 'window') return;
      const phaseIndex=EXPECTED.findIndex(([name])=>name===row.phase),state=states.get(row.phase);
      assert.ok(state && phaseIndex>=lastPhase, 'Unknown or reordered passing probe minute phase');
      if(phaseIndex>lastPhase)assert.equal(states.get(EXPECTED[lastPhase][0]).rows.length,EXPECTED[lastPhase][1], 'Minute phase advanced before all complete windows');
      lastPhase=phaseIndex;
      assert.ok(total<maximum && state.rows.length<state.required, 'Passing probe minute windows exceed exact boundedfixed phase counts');
      const index=state.rows.length,expected=state.stage.rollingWindows[index];
      assert.ok(clock(row.startedAtMs)&&clock(row.endedAtMs)&&row.endedAtMs-row.startedAtMs>=60000, 'Actual minute elapsed time is less than60s or invalid');
      assert.equal(row.startedAtMs,index===0?state.stage.startedAtMs:state.rows[index-1].endedAtMs, 'Minute boundaries must be contiguous actual clocks');
      assert.ok(row.endedAtMs<=state.stage.endedAtMs, 'Minute extends past original measured phase');
      assert.equal(expected.startedAtMs,row.startedAtMs);assert.equal(expected.endedAtMs,row.endedAtMs);
      const seconds=(row.endedAtMs-row.startedAtMs)/1000;
      assert.equal(row.assessment?.seconds,seconds, 'Raw minute seconds must equal actual wall elapsed');
      assert.equal(expected.seconds,seconds, 'Report minute seconds must equal actual wall elapsed');
      assert.equal(expected.elapsedSeconds,(row.endedAtMs-state.stage.startedAtMs)/1000, 'Report cumulative elapsed must equal actual phase wall time');
      assert.equal(row.assessment.passed,true, 'Original minute assessment must have passed');
      const {elapsedSeconds,startedAtMs,endedAtMs,...assessment}=expected;
      assert.deepEqual(row.assessment,assessment, 'Actual raw minute assessment differs from the original report');
      state.rows.push({phase:row.phase,startedAtMs:row.startedAtMs,endedAtMs:row.endedAtMs,assessment:{seconds,passed:row.assessment.passed}});total++;
    },
    forPhase(name) {assert.ok(states.has(name));return states.get(name).rows;},
    assertComplete() {assert.equal(total,maximum);for(const state of states.values())assert.equal(state.rows.length,state.required);},
  };
}
