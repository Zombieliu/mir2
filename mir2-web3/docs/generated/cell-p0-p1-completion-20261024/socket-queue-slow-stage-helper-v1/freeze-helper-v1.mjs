import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { LEGACY_STAGES, CONTEXT_STAGES, ALLOWED_STAGES, ALLOWED_SCOPED_STAGES, ALLOWED_QUEUED_PHASES, ALLOWED_INPUT_CLASSES,
  MAX_LINE_BYTES, MAX_PUBLIC_BYTES, MAX_WRITE_BUFFER_BYTES, MAX_FINISH_WAIT_MS } from './slow-stage-public.mjs';
const root=path.dirname(fileURLToPath(import.meta.url));
const project='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const sha=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const pin=file=>{const bytes=fs.readFileSync(file);return{file,bytes:bytes.length,sha256:sha(bytes)};};
const write=(name,value)=>fs.writeFileSync(path.join(root,name),JSON.stringify(value,null,2)+'\n',{flag:'wx'});
const original=fs.readFileSync(path.join(root,'legacy14-original.mjs.txt'),'utf8').replace(/\r\n/g,'\n');
const current=fs.readFileSync(path.join(root,'slow-stage-public.test.mjs'),'utf8').replace(/\r\n/g,'\n');
const second="test('split chunks and CRLF emit only the three declared metadata fields at receive time'";
assert.equal(original.slice(original.indexOf(second)).trimEnd(),current.slice(current.indexOf(second),current.indexOf('const contextWire =')).trimEnd());
const logFile=path.join(root,'synthetic-tests-v1.log'),log=fs.readFileSync(logFile,'utf8');
assert.match(log,/# tests 25\r?\n/);assert.match(log,/# pass 25\r?\n/);assert.match(log,/# fail 0\r?\n/);
const contract={schemaVersion:1,scope:'Strict metadata-only diagnostic projection; no identity, request/action ID, credentials or game payload.',
  producerPrefix:'[gateway-slow-stage]',wireGrammar:{legacy:'[gateway-slow-stage] stage=<label> duration_ms=<canonical safeuint>',
    scoped:'[gateway-slow-stage] stage=<allowed scoped label> duration_ms=<canonical safeuint> connection=<canonical positive safeuint>',
    queue:'[gateway-slow-stage] stage=socket_input.queue_residence duration_ms=<canonical safeuint> connection=<canonical positive safeuint> queued_phase=<phase> phase_elapsed_ms=<canonical safeuint> input_class=<class>'},
  normalizedKeys:{legacy:['observedAtMs','stage','durationMs'],scoped:['observedAtMs','stage','durationMs','connection'],
    queue:['observedAtMs','stage','durationMs','connection','queuedPhase','phaseElapsedMs','inputClass']},
  legacyStages:LEGACY_STAGES,newContextStages:CONTEXT_STAGES,allowedLegacyStages:ALLOWED_STAGES,allowedNonQueueScopedStages:ALLOWED_SCOPED_STAGES,
  allowedQueuedPhases:ALLOWED_QUEUED_PHASES,allowedInputClasses:ALLOWED_INPUT_CLASSES,
  numeric:{canonical:'0 or a nonzero decimal digit followed by zero or more decimal digits; no sign, decimal point, exponent or leading zero',maximum:Number.MAX_SAFE_INTEGER,
    minimumDurationMs:0,minimumObservedAtMs:0,minimumPhaseElapsedMs:0,minimumConnection:1},
  inputClassMeaning:{move:'Walk, Run or Turn',attack_v2:'Typed ordinary attack request; no requestId retained',other:'Any other input including identity/authentication commands; payload never retained'},
  limits:{lineBytes:MAX_LINE_BYTES,totalPublicBytes:MAX_PUBLIC_BYTES,prewriteQueueBytes:MAX_WRITE_BUFFER_BYTES,writerHighWaterMarkBytes:MAX_WRITE_BUFFER_BYTES,finishWaitMs:MAX_FINISH_WAIT_MS},
  parsingRules:['ASCII whole line only, anchored fixed field order, CRLF supported; partial EOF never emitted.',
    'All40 labels may use legacy format, including context-counter exhaustion fallback; other old labels cannot acquire a connection field.',
    'Queue context is all four fields together; a bare queue connection or a partial/extra/reordered metadata set is discarded without legacy downgrade.',
    'Unknown labels/phases/classes, non-ASCII, signed/floating/scientific numeric tokens, extra text and malformed whole lines are filtered without public output, as with prior stderr noise filtering.',
    'Digit-shaped noncanonical/unsafe duration, connection or phase age on an otherwise allowed whole record makes capture incomplete and requests normal stop once.',
    'Existing line/byte/write caps, write backpressure, read/write/finalize error or incomplete EOF make capture incomplete and request normal stop once; original private stderr is independently drained.',
    'observedAtMs is collector receive wall clock. queuedPhase and phaseElapsedMs were sampled at queue construction, not when dequeued; neither field proves an exact span start/end or a cause.',
    'No CRC or new transport protocol; original source/raw SHA evidence binding remains external to this metadata parser.'],
  newFailureReasons:['noncanonical-connection','unsafe-connection','noncanonical-phase-elapsed','unsafe-phase-elapsed'],
  historicalCompatibility:{legacyHelperUntouched:true,legacyEvidenceUntouched:true,originalFourteenScenariosRetained:true,inventoryFixtureExtendedForNewProducerApiAnd40Labels:true,
    remainingThirteenOldTestBodiesExactAfterLfNormalization:true,originalSource:pin(path.join(root,'legacy14-original.mjs.txt'))},
  consumerRequirements:['Future collector/verifier must accept exactly3/4/7 normalized key sets and validate the corresponding fixed label/phase/class sets and numeric ranges.',
    'Future collector status allowlist must include the four new fixed numeric failure reasons.',
    'A new41-input backend, matching Release and functional acceptance must be bound before any new launcher/realm is prepared; the old855b binary is not a producer of these new diagnostics.']};
write('schema-contract.json',contract);
const sources=['slow-stage-public.mjs','slow-stage-public.test.mjs','freeze-helper-v1.mjs'].map(name=>pin(path.join(root,name)));
const files=[...sources,pin(path.join(root,'schema-contract.json')),pin(path.join(root,'legacy14-original.mjs.txt')),pin(logFile)];
const producer=pin(path.join(project,'apps/gateway/src/web.rs'));
assert.equal(producer.bytes,855687);assert.equal(producer.sha256,'769186e87e166a7ba0fbb971463c8e0ae2b4dd119292798badee3b8e28522dd6');
write('freeze-v1.json',{schemaVersion:1,frozenAt:new Date().toISOString(),state:'source-frozen-synthetic-only',sourceSha256:sha(JSON.stringify(sources)),sources,files,
  producerSource:producer,producerSourceCommit:null,newBackendInputCount:41,newBackendReleaseBindingPending:true,
  synthetic:{tests:25,passed:25,failed:0,skipped:0,cancelled:0,durationMs:154.3198,legacyScenariosPassed:14,newContextScenariosPassed:11,log:pin(logFile)},
  normalizedKeys:contract.normalizedKeys,stageCounts:{legacy:27,newContext:13,totalLegacy:40,nonQueueScoped:15,queuedPhases:15,inputClasses:3},limits:contract.limits,
  scope:'New external helper/test/schema sources only. No old03/07 source, production file, launcher, threshold or prior evidence was modified. Synthetic validation only, no game build/service/capacity acceptance.',
  oldFrozenFilesModified:false,productFilesModified:false,helperImportedForSyntheticTestsOnly:true,prepareExecuted:false,realmStarted:false,capacityAccepted:false});
console.log(JSON.stringify({root,helper:pin(path.join(root,'slow-stage-public.mjs')),test:pin(path.join(root,'slow-stage-public.test.mjs')),freeze:pin(path.join(root,'freeze-v1.json')),contract:pin(path.join(root,'schema-contract.json')),
  sourceSha256:sha(JSON.stringify(sources)),tests:25,passed:25,normalizedKeys:contract.normalizedKeys},null,2));
