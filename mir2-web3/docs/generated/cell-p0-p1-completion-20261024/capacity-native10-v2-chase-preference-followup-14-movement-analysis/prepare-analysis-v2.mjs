import fs from 'node:fs/promises'; import path from 'node:path'; import crypto from 'node:crypto'; import assert from 'node:assert/strict';
const directory='C:/mir2-build/p01-completion-20261024/probe14-movement-correction-analysis-v1',before=await fs.readFile(path.join(directory,'analyze-closed-raw-v1.mjs'));
const rules=[
 {before:"const digest = crypto.createHash('sha256'), stream = createReadStream(raw), rings = new Map(), cases = [], corrections = [], transforms = [], actionCounts = {}, typeCounts = {};",after:"const digest = crypto.createHash('sha256'), stream = createReadStream(raw), rings = new Map(), cases = [], corrections = [], transforms = [], actionCounts = {}, typeCounts = {}, priorActions = [], lastSnapshots = new Map();"},
 {before:"const projected = wireProjection(row, rows), ring = rings.get(row.connection) ?? [];",after:"const projected = wireProjection(row, rows); if(projected.source==='worldSnapshot')lastSnapshots.set(row.connection,projected); const ring = rings.get(row.connection) ?? [];"},
 {before:"    if (row.status !== 'corrected') continue;",after:"    const recentActions=[...priorActions];priorActions.push(actionProjection(row,rows));if(priorActions.length>12)priorActions.shift();\n    if (row.status !== 'corrected') continue;"},
 {before:"following: [], recentActualSelfTransforms: [...transforms]",after:"following: [], recentActualSelfTransforms: [...transforms], previousPlannedMovements:recentActions, lastFullSnapshot:lastSnapshots.get(1)??null"},
 {before:"assert.equal(cases.length, 4);",after:"assert.equal(cases.length,5); assert.deepEqual(cases.map(item=>item.action.line),[1642,3473,3482,3887,3897]);"},
 {before:"number: item.number, actionLine:",after:"number: item.number, insideFinalFailingWindow:item.number>1, actionLine:"},
 {before:"rawSingleStreamingPass: true, typeCounts",after:"rawSingleStreamingPass: false, analysisRawFullReads:2, firstReadStoppedAtWrongFourTotalCasesAssertion:true, perExecutionRawPasses:1, typeCounts"},
 {before:"selected-corrections-v1.json",after:"selected-corrections-v2.json"},
 {before:"summary-v1.json",after:"summary-v2.json"}
];
let after=before.toString('utf8');for(const rule of rules){assert.equal(after.split(rule.before).length,2);after=after.replace(rule.before,rule.after);}let inverse=after;for(const rule of [...rules].reverse())inverse=inverse.replace(rule.after,rule.before);assert.ok(Buffer.from(inverse).equals(before));
await fs.writeFile(path.join(directory,'analyze-closed-raw-v2.mjs'),after,{flag:'wx'});
await fs.writeFile(path.join(directory,'analysis-tool-revision-v2.json'),JSON.stringify({schemaVersion:1,scope:'A forensic tool correction: whole-stage has five corrected moves, of which the final four are in the failed continuous window. No original input or game decision changed.',before:{bytes:before.length,sha256:crypto.createHash('sha256').update(before).digest('hex')},after:{bytes:Buffer.byteLength(after),sha256:crypto.createHash('sha256').update(after).digest('hex')},rules,exactInverse:true,firstToolExit:1,originalStderrFileCaptured:false,observedFirstFailure:'Expected cases.length4; actual5',firstPassOutputsWritten:false},null,2)+'\n',{flag:'wx'});
