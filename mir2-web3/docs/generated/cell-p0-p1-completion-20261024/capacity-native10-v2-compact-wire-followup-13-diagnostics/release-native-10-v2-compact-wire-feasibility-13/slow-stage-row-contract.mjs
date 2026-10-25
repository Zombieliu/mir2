import { ALLOWED_STAGES, ALLOWED_SCOPED_STAGES, ALLOWED_QUEUED_PHASES, ALLOWED_INPUT_CLASSES } from './slow-stage-public.mjs';
const stages=new Set(ALLOWED_STAGES),scoped=new Set(ALLOWED_SCOPED_STAGES),phases=new Set(ALLOWED_QUEUED_PHASES),classes=new Set(ALLOWED_INPUT_CLASSES);
const integer=value=>Number.isSafeInteger(value)&&value>=0&&!Object.is(value,-0);
const shapes={3:['observedAtMs','stage','durationMs'],4:['observedAtMs','stage','durationMs','connection'],7:['observedAtMs','stage','durationMs','connection','queuedPhase','phaseElapsedMs','inputClass']};
export function validateSlowStagePublicRow(row,line) {
  const keys=row&&typeof row==='object'&&!Array.isArray(row)?Object.keys(row):[];
  const expected=shapes[keys.length];
  if(!expected||JSON.stringify(keys)!==JSON.stringify(expected)||!stages.has(row.stage)||!integer(row.observedAtMs)||!integer(row.durationMs))throw Error('Invalid diagnostic public row shape or primitive');
  if(keys.length>3&&(!integer(row.connection)||row.connection<1))throw Error('Invalid diagnostic connection counter');
  if(keys.length===4&&!scoped.has(row.stage))throw Error('Only approved nonqueue labels may carry four-key context');
  if(keys.length===7&&(row.stage!=='socket_input.queue_residence'||!phases.has(row.queuedPhase)||!classes.has(row.inputClass)||!integer(row.phaseElapsedMs)))throw Error('Invalid complete queue diagnostic context');
  if(typeof line==='string'&&line!==JSON.stringify(row))throw Error('Diagnostic JSONL must retain canonical exact-key serialization');
  return row;
}
