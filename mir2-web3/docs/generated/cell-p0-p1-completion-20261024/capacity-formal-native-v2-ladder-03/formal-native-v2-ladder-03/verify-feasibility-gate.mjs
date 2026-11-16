import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { base, project, currentKeys, driverFrozenFile, read, pin, sha, assertPin, samePath, collectDriverInputs, git } from './input-lib.mjs';

export async function verifyFeasibilityGate(binding, preparedDrivers) {
  const { release, backendSourceCommit, driverSourceCommit } = binding;
  const probeRoot = path.join(base, 'release-native-10-v2-coordinated-feasibility-06');
  if (!samePath(await fs.realpath(probeRoot), probeRoot)) throw new Error('Probe06 root must be a real owned directory');
  const owned = async name => { const file = path.join(probeRoot, name); if (!samePath(await fs.realpath(file), file)) throw new Error('Redirected probe06 evidence is forbidden'); return file; };
  const cleanupFile = await owned('realm-cleanup.json'), buildFile = await owned('gateway-build-pin.json');
  const cleanup = await read(cleanupFile), build = await read(buildFile);
  if (build.sourceCommit !== backendSourceCommit || build.backendSourceCommit !== backendSourceCommit || !/^[a-f0-9]{40}$/.test(build.driverSourceCommit ?? '') ||
      build.sha256 !== release.binary.sha256 || build.bytes !== release.binary.bytes || build.sourceManifest?.sha256 !== release.sourceManifestFileSha256 ||
      JSON.stringify(build.releaseRecord) !== JSON.stringify(binding.releasePin) || build.sourceManifest?.inputs !== 39) throw new Error('Formal admission requires the original same461f/855b/39 backend from real probe06');
  if (cleanup.sourceCommit !== backendSourceCommit || cleanup.backendSourceCommit !== backendSourceCommit || cleanup.driverSourceCommit !== build.driverSourceCommit ||
      cleanup.sha256 !== build.sha256 || cleanup.formalStarted !== false || cleanup.capacityAccepted !== false) throw new Error('Probe06 cleanup provenance differs');
  const sourcePinFile = await owned('source-pins-admitted-v2.json'), sources = await read(sourcePinFile);
  const preparedSourceFile = await owned('source-pins.json'), preparedSources = await read(preparedSourceFile);
  const drivers = preparedDrivers ?? await collectDriverInputs(), driver = drivers.frozen;
  if (drivers.driverSourceCommit !== driverSourceCommit || sources.sourceCommit !== backendSourceCommit || sources.backendSourceCommit !== backendSourceCommit ||
      sources.driverSourceCommit !== build.driverSourceCommit || preparedSources.backendSourceCommit !== backendSourceCommit ||
      preparedSources.driverSourceCommit !== build.driverSourceCommit || !samePath(preparedSources.driverFrozenManifest ?? '', driverFrozenFile) ||
      preparedSources.driverSourceSha256 !== driver.sourceSha256 || preparedSources.originalBackendReleaseReused !== true ||
      !Array.isArray(sources.files) || sha(JSON.stringify(sources.files)) !== sources.sourceConfigSha256) throw new Error('Probe06 exact coordinated-driver input binding is incomplete');
  const preparedSourcePin = await pin(preparedSourceFile), boundPrepared = sources.files.find(row => samePath(row.file, preparedSourceFile));
  if (!boundPrepared || boundPrepared.bytes !== preparedSourcePin.bytes || boundPrepared.sha256 !== preparedSourcePin.sha256 ||
      !Array.isArray(preparedSources.files) || sha(JSON.stringify(preparedSources.files)) !== preparedSources.sourceConfigSha256) throw new Error('Probe06 prepared source closure is not bound into admitted evidence');
  for (const input of sources.files) await assertPin(input);
  for (const input of drivers.pins) {
    const matched = sources.files.find(row => samePath(row.file, input.file));
    if (!matched || matched.bytes !== input.bytes || matched.sha256 !== input.sha256) throw new Error('Probe06 driver/import/data/transport differs from the current exact frozen closure');
  }
  for (const input of driver.files) {
    const bytes = await fs.readFile(path.join(project, input.file));
    if (sha(Buffer.from(bytes.toString('utf8').replace(/\r\n/g, '\n'))) !== sha(git(['show', build.driverSourceCommit + ':mir2-web3/' + input.file]))) throw new Error('Probe06 driver commit does not contain the exact same frozen nine inputs');
  }
  const coordinationSource = await fs.readFile(path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs'), 'utf8');
  const declaration = coordinationSource.match(/Object\.defineProperty\(report, 'navigationCoordination', \{ enumerable: true,\s*value: Object\.freeze\(\{ scope: '([^'\r\n]+)',\s*maximumHintCellsPerConnection: NAVIGATION_HINT_MAX_CELLS, blockedOpportunityRule: '([^'\r\n]+)' \}\) \}\)/);
  const maximumCells = coordinationSource.match(/export const NAVIGATION_HINT_MAX_CELLS = (\d+);/);
  if (!declaration || maximumCells?.[1] !== '7') throw new Error('Current pinned native navigation declaration is missing or incompatible');
  const expectedCoordination = { scope: declaration[1], maximumHintCellsPerConnection: 7, blockedOpportunityRule: declaration[2] };
  const { capacityPolicy } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-soak-core.mjs')));
  if (currentKeys.some(key => cleanup.preStopCapacity?.[key] !== 0) || cleanup.checks?.sixCountersZero !== true ||
      cleanup.checks?.normalOwnedShutdown !== true || cleanup.checks?.processAbsent !== true || cleanup.checks?.portsFree !== true ||
      cleanup.checks?.monitorStoppedBeforeGateway !== true || cleanup.checks?.monitorExitZero !== true || cleanup.checks?.monitorTerminalPreserved !== true ||
      cleanup.gatewayExit?.code !== 0 || cleanup.monitorExit?.code !== 0 || cleanup.inputsChangedWhileRunning || cleanup.error) throw new Error('Probe06 owned cleanup did not pass');
  const reportFile = cleanup.report?.reportPath;
  if (typeof reportFile !== 'string' || !samePath(path.dirname(reportFile), path.join(probeRoot, 'reports')) || !reportFile.endsWith('.report.json') ||
      !samePath(await fs.realpath(reportFile), reportFile)) throw new Error('Probe06 report must be an explicit owned real file');
  const provenanceFile = await owned('report-provenance.json'), provenance = await read(provenanceFile), reportPin = await pin(reportFile);
  if (provenance.backendSourceCommit !== backendSourceCommit || provenance.driverSourceCommit !== build.driverSourceCommit ||
      provenance.backendBinarySha256 !== build.sha256 || provenance.driverSourceSha256 !== driver.sourceSha256 ||
      !samePath(provenance.driverFrozenManifest ?? '', driverFrozenFile) || provenance.originalBackendReleaseReused !== true || provenance.capacityAccepted !== false ||
      JSON.stringify(provenance.report) !== JSON.stringify(reportPin)) throw new Error('Probe06 original report bytes are not bound to the backend/driver identity sidecar');
  const report = await read(reportFile), p = report.policy;
  if (provenance.runId !== report.runId || cleanup.report?.runId !== report.runId || cleanup.report?.backendSourceCommit !== backendSourceCommit ||
      cleanup.report?.driverSourceCommit !== build.driverSourceCommit || JSON.stringify(report.navigationCoordination) !== JSON.stringify(expectedCoordination)) throw new Error('Probe06 report navigation declaration/provenance differs from exact pinned source');
  const expectedPolicy = capacityPolicy({ profile: 'probe', activityMode: 'native', stages: '10', stageSeconds: 30, soakSeconds: 360,
    combatActors: 1, resumeSamples: 0, saveSamples: 0, wallSeconds: 1800, movementIntervalMs: 750 });
  if (JSON.stringify(p) !== JSON.stringify(expectedPolicy)) throw new Error('Probe06 must retain the entire original probe policy');
  if (report.ok !== true || report.ordinaryAccountsOnly !== true || report.attackResults?.mode !== 'v2' || report.attackResults.capability !== 'nativeAttackOutcomeV1' || p?.profile !== 'probe' || p.activityMode !== 'native' ||
      JSON.stringify(p.stages) !== '[10]' || p.stageSeconds !== 30 || p.soakSeconds !== 360 || p.combatActors !== 1 || p.resumeSamples !== 0 || p.saveSamples !== 0 ||
      p.fullAcceptanceEligible !== false || report.playableCapacityAccepted !== false || report.acceptedControlledActors !== null ||
      report.peakControlledActors !== 10 || report.highestPassedStage !== 10 || report.safetyStopped !== false || report.error || report.cleanupErrors?.length ||
      report.continuousAcceptance?.passed !== true || report.continuousAcceptance.failureCount !== 0) throw new Error('Real coordinated probe06 has not passed; a probe remains ineligible for capacity acceptance');
  const strict = { minimumCombatAttacksPerMinute: 18, minimumCombatHitsPerMinute: 12, minimumNativeRunRatio: .5, minimumNativeCellsPerMinute: 80,
    maximumCorrectionRatio: .05, maximumMissedPlanRatio: .05, timeoutExclusive: .01, movementP95Ms: 750, maximumMemorySlopeMiBPerHour: 32 };
  if (Object.entries(strict).some(([key, value]) => p[key] !== value)) throw new Error('Probe06 policy differs from fixed ordinary thresholds');
  const stage = report.stages?.find(value => value.name === 'stage-10'), soak = report.stages?.find(value => value.name === 'soak');
  if (report.stages?.length !== 2 || !stage?.completed || !stage.passed || stage.controlledActors !== 10 || stage.seconds !== 30 ||
      !soak?.completed || !soak.passed || soak.seconds !== 360 || soak.controlledActors !== 10 || soak.combatCoverage !== 'shared-positive-damage' ||
      soak.rollingWindows?.length !== 6 || soak.rollingWindows.some((value, index) => !value.passed || !Number.isFinite(value.seconds) || value.seconds <= 0 ||
        !Number.isFinite(value.elapsedSeconds) || value.elapsedSeconds < (index + 1) * 60) ||
      soak.endedAtMs - soak.startedAtMs < 360000) throw new Error('Probe06 needs six actual complete passing60s soak windows');
  const rawFile = reportFile.replace(/\.report\.json$/, '.samples.jsonl');
  if (!samePath(await fs.realpath(rawFile), rawFile)) throw new Error('Redirected raw probe06 evidence is forbidden');
  const before = await fs.stat(rawFile), digest = crypto.createHash('sha256'); let bytes = 0;
  for await (const chunk of createReadStream(rawFile)) { bytes += chunk.length; digest.update(chunk); }
  const rawPin = { file: rawFile, bytes, sha256: digest.digest('hex') };
  const verifierFile = path.join(project, 'docs/generated/cell-p0-p1-completion-20261024/evidence-tools/verify-evidence.mjs');
  const { streamCapacitySamples, STREAM_LIMITS, verifyCapacityStatus, verifyCapacityPhases } = await import(pathToFileURL(verifierFile));
  const streamed = await streamCapacitySamples(rawFile, rawPin, STREAM_LIMITS, report);
  verifyCapacityPhases(report, streamed.selected);
  if (verifyCapacityStatus(report, streamed.selected, streamed.monitorSummary) !== false) throw new Error('Probe06 was incorrectly promoted to formal capacity acceptance');
  const after = await fs.stat(rawFile), monitor = streamed.monitorSummary;
  if (before.size !== bytes || after.size !== bytes || before.mtimeMs !== after.mtimeMs || !monitor.complete || monitor.count < 2 || monitor.invalidCount || monitor.stoppedCount || monitor.maxGapMs > 15000 ||
      streamed.selected.some(row => ['continuousFailure', 'safetyStop'].includes(row.type))) throw new Error('Probe06 raw/monitor continuity is incomplete');
  for (const phase of [stage, soak]) {
    const bracket = monitor.phaseBrackets.find(value => value.name === phase.name);
    if (!bracket?.beforeStart || !bracket.afterStart || !bracket.beforeEnd || [bracket.startBeforeGapMs, bracket.startAfterGapMs, bracket.endBeforeGapMs].some(gap => gap < 0 || gap > 15000)) throw new Error('Probe06 raw monitor does not cover its true phase interval');
  }
  const files = await Promise.all([cleanupFile, buildFile, reportFile, sourcePinFile, preparedSourceFile, provenanceFile, await owned('launcher-source-frozen-v1.json'), verifierFile].map(pin));
  files.push(rawPin);
  return { schemaVersion: 1, verifiedAt: new Date().toISOString(), sourceCommit: backendSourceCommit, backendSourceCommit, driverSourceCommit,
    feasibilityDriverSourceCommit: build.driverSourceCommit, exactSameFrozenDriverInputs: true, driverFrozenManifest: driverFrozenFile,
    driverSourceSha256: driver.sourceSha256, releaseSha256: release.binary.sha256, runId: report.runId, navigationCoordination: expectedCoordination,
    feasibilityPassed: true, capacityAccepted: false, profile: 'probe', native10SoakSeconds: 360, actualPassedSoakWindows: 6, rawRows: streamed.rows,
    rawMonitor: { count: monitor.count, complete: monitor.complete, maxGapMs: monitor.maxGapMs }, files };
}
