import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
const base = 'C:/mir2-build/p01-completion-20261024', root = path.join(base, 'formal02-analysis');
const input = path.join(root, 'diagnostic-v2.json'), d = JSON.parse(fs.readFileSync(input, 'utf8'));
const pin = file => { const bytes = fs.readFileSync(file); return { file, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') }; };
const summary = {
  schemaVersion: 1, summarizedAt: new Date().toISOString(), runId: d.runId, originalVerdictRetained: true,
  sourceCommits: { backend: d.backendSourceCommit, driver: d.driverSourceCommit },
  originalOutcome: d.originalOutcome,
  fiveFailures: d.allFiveFailedWindows.map(w => {
    const f = w.originalFailure.evidence, fanout = f.metrics['fanout-run'];
    return { actor: f.actor, role: f.role, final: f.final, emittedAtMs: w.originalFailure.at,
      activeDurationSeconds: f.seconds, estimatedWallWindow: w.wallWindow,
      activityPassed: f.activityPassed, latencyPassed: f.latencyPassed, fanoutPassed: f.fanoutPassed,
      timeoutRatio: f.timeoutRatio, correctionRatio: f.correctionRatio, nativeCompletion: f.nativeActivity.completionRatio,
      runRatio: f.nativeActivity.runRatio,
      ...(fanout ? { fanoutRun: { count: fanout.count, statuses: fanout.status, sendP95Ms: fanout.fromSend.p95Ms,
        sendP99Ms: fanout.fromSend.p99Ms, successMaximumMs: fanout.fromSend.maxMs } } : {}),
      failingMetricStatuses: Object.fromEntries(Object.entries(f.metrics).filter(([key, m]) => (m.status.timeout ?? 0) ||
        (['walk','run','turn'].includes(key) && (m.fromSend.p95Ms ?? 0) > 750)).map(([key, m]) => [key, m])) };
  }),
  allTimeouts: d.episode.allTimeouts,
  failureInterpretation: 'One complete60s actor04 window triggered normal drain. Four additional failed actor segments were finalized after drain, but their timeout/latency/correction evidence came from real pre-stop actions; they are not five independent stalls or shutdown-generated false failures.',
  commonStall: {
    observedActionSendWallEstimateRangeMs: [1792872120990,1792872121769], observedDelayedCompletionRangeMs: [1792872121893,1792872123257],
    firstFailureEmittedAtMs: 1792872154828,
    actionClockEstimateRule: d.clockMapping,
    ownerMovementMaximumObservedMs: Math.max(...d.episode.peakActions.filter(row => row.phase === 'stage-25' && ['walk','run'].includes(row.kind)).map(row => row.latencyMs)),
    successfulOwnerMovementMaximumObservedMs: Math.max(...d.episode.peakActions.filter(row => row.phase === 'stage-25' && ['walk','run'].includes(row.kind) && row.status === 'success').map(row => row.latencyMs)),
    timeoutFanout: d.episode.timeoutAssociations,
    diagnosticMetricInterval: d.episode.criticalIntervals.filter(row => row.metrics.ownerTickDeadlineLag.lifetimeMaxAfterUs > row.metrics.ownerTickDeadlineLag.lifetimeMaxBeforeUs ||
      row.metrics.movementAdmissions.duration.lifetimeMaxAfterUs > row.metrics.movementAdmissions.duration.lifetimeMaxBeforeUs ||
      row.metrics.immediateRequests.duration.lifetimeMaxAfterUs > row.metrics.immediateRequests.duration.lifetimeMaxBeforeUs),
    driverFirstWindow: d.allFiveFailedWindows[0].resources.driverEventLoop,
    driverMaximumObservedAllRunMs: Math.max(...d.episode.peakDriverRows.map(row => row.driver.eventLoopLagMaxMs))
  },
  phaseComparisons: d.phaseAnalysis.filter(row => row.kind === 'measurement').map(row => ({ name: row.name, targetActors: row.targetActors,
    completed: row.completed, seconds: row.measuredDurationSeconds, bracket: { from: row.serverTelemetryBracket.beforeSampleAtMs,
      to: row.serverTelemetryBracket.afterSampleAtMs, startMarginMs: row.serverTelemetryBracket.beforeMarginMs, endMarginMs: row.serverTelemetryBracket.afterMarginMs },
    resources: row.resources, serverTelemetry: row.serverTelemetryBracket.metrics })),
  attribution: {
    directlyObserved: [
      'The frozen formal02 report failed at25 actors; stage2 and10 each passed300s,25 ended at220.716s,50/100 and7200s soak were never reached.',
      'Within the same approximately2.253s monitor interval, new server owner-tick deadline lateness reached1793732us, movement handler admission duration reached2018131us, and immediate handler duration reached1641541us.',
      'In that interval,7 measured zoneWork samples were within the10000us bucket with zero over50000us. Movement admissions had30 of38 over50000us, immediate handlers4 of4 over50000us. Pending-movement deadline-lag had zero samples.',
      'The driver first failing-window event-loop maximum was37.847039ms; maximum observed over the entire run was50.003967ms. The original250ms-for3-samples driver stop did not trigger.',
      'Gateway process CPU in the diagnostic interval was0.56984 logical CPU equivalent, total process IO130.34 operations/s, write260034.7B/s, host available memory34246639616B. These sampled values did not approach the frozen aggregate guards.',
      'A host logical-core sample reached100%, but these counters are host-wide and cannot identify the owner process or prove aggregate physical saturation.'
    ],
    supportedInference: 'The measured service-side handler/deadline stall is real and consistent with waiting/scheduling before measured Zone work, rather than a1.8s Zone-work execution or a matching long Node event-loop pause. This is an inference about scope, not a named root cause.',
    rootCauseUnresolved: [ 'Spectator snapshot/merge/serialize/recording contention is a source-level candidate only; no timed lock/flush span or public ring-frame timeline binds it to the incident.',
      'OS scheduling, storage flush latency, shared lock/queue wait and packet/event-retention effects remain unseparated. Low sampled total IO/CPU does not exclude a serialized stall.',
      'Movement/fanout has no request ID or retained complete receive trace. The owner-run to timeout association is unique within a15ms timing tolerance but remains diagnostic correlation.',
      'No ETW, thread-level CPU/wait stack, disk queue/flush latency or complete per-packet stream was captured. Do not claim a unique cause or hardware maximum.' ],
    nextEvidenceRule: 'Any diagnostic optimization or lower-N two-hour follow-up must have a new prospective frozen profile and preserve this failure. Stage10 passing300s is not a two-hour baseline or the original standard100 acceptance.'
  },
  evidence: { detail: pin(input), selected: pin(path.join(root,'selected-events-v2.json')), parser: pin(path.join(root,'analyze-formal02-v2.mjs')),
    ...(fs.existsSync(path.join(base,'formal02-source-stall-review-v1.json')) ? { independentSourceReview: pin(path.join(base,'formal02-source-stall-review-v1.json')) } : {}) },
  limitations: d.limitations,
  operations: d.operations
};
const out = path.join(root,'summary-v2.json');
fs.writeFileSync(out, JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
const md = `formal02 原始失败诊断（${d.runId}）\n\n` +
  `2 人和 10 人阶段各通过 300 秒；25 人阶段运行 220.716 秒后失败。50/100 人和两小时稳态没有执行。原报告和全部门槛保持不变。\n\n` +
  `首次触发是 L7324bd04 的完整 60 秒窗口：活动与 owner 延迟门槛通过，fanout-run 35 成功、1 超时（2.78%）。其余 4 个失败是停止后结算的尾段；其中的异常动作均发生在停止前，不能称为 5 次独立拥塞或清理制造的错误。\n\n` +
  `实际异常集中在墙钟 1792872120990–1792872123257 附近：两次 V2 攻击超过原 900ms、一条移动超过 2s、一条 fanout 超过 2s，多个客户端的 owner 移动延迟达到 1.4–1.925s。约 33 秒后才结算出首个失败窗口。\n\n` +
  `同区间 1792872121229–1792872123482 的服务端指标新增峰值：tick 截止时间滞后 1.793732s、movement admission 2.018131s、immediate handler 1.641541s；7 个 zoneWork 样本均在 10ms 桶内。该差异支持工作执行前的等待或调度停顿，但尚不能定位到特定锁、录像写入或操作系统。\n\n` +
  `驱动首失败窗口最大事件循环延迟为 37.85ms，全程观测最大为 50.00ms。相邻资源样本约 0.57 个逻辑 CPU、130 次进程 IO 操作/秒、260KB 写入/秒，空闲物理内存约 31.9GiB；未触及原总量 guard。单逻辑核 100% 是整机指标，无法归属 Gateway，也不能据此宣布硬件打满。\n\n` +
  `没有读取 private 或录像 payload，没有修改 tracked 源码、测试、Realm 或验收口径。完整原件 SHA、五窗口、邻接监控原始桶差分、逐核样本及归因限制见 summary-v2.json、diagnostic-v2.json 和 selected-events-v2.json。\n`;
fs.writeFileSync(path.join(root,'README-v2.md'),md,{flag:'wx'});
console.log(JSON.stringify({summary:pin(out),readme:pin(path.join(root,'README-v2.md')),parser:pin(path.join(root,'analyze-formal02-v2.mjs'))}));
