import { verifySceneContract } from './scene-contract.mjs';
import fs from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { createSlowStagePublicCapture } from './slow-stage-public.mjs';
import { verifyDriverReportContract } from './driver-report-contract.mjs';
import { root, project, endpoint, healthUrl, currentKeys, limits, read, pin, writeNew, sha, samePath, sleep, verifyRelease, assertPin, listeners, processIdentity, health, spectator } from './input-lib.mjs';

if (process.argv.slice(2).join(' ') !== '--execute-authorized-probe --attack-results=v2') throw new Error('Explicit authorized-execution and --attack-results=v2 arguments required; preparation alone never launches');
const frozen = await read(path.join(root, 'source-pins.json'));
if (!Array.isArray(frozen.files) || sha(JSON.stringify(frozen.files)) !== frozen.sourceConfigSha256) throw new Error('Prepared input manifest is inconsistent');
for (const entry of frozen.files) await assertPin(entry);
const scene=await verifySceneContract(root,'probe');
const binding = await verifyRelease(), build = await read(path.join(root, 'gateway-build-pin.json'));
if (build.sourceCommit !== binding.backendSourceCommit || build.backendSourceCommit !== binding.backendSourceCommit || build.driverSourceCommit !== binding.driverSourceCommit ||
    frozen.backendSourceCommit !== build.backendSourceCommit || frozen.driverSourceCommit !== build.driverSourceCommit || build.sourceManifest.sha256 !== binding.manifestPin.sha256 || frozen.sourceCommit !== build.sourceCommit) throw new Error('Prepared/new release identity differs');
await assertPin({ file: build.ownedExecutable, bytes: build.bytes, sha256: build.sha256 });
if (!samePath(await fs.realpath(root), root) || listeners().length) throw new Error('Probe root redirected or isolated ports already in use');
await fs.writeFile(path.join(root, 'execution.lock'), JSON.stringify({ runnerPid: process.pid, startedAt: new Date().toISOString() }) + '\n', { flag: 'wx' });
const privateRoot = path.join(root, 'private'), secretNames = ['recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt'];
const secrets = await Promise.all(secretNames.map(name => fs.readFile(path.join(privateRoot, name), 'utf8')));
if (secrets.some(value => !/^[a-f0-9]{64}$/.test(value)) || new Set(secrets).size !== 4) throw new Error('Fresh distinct private secrets required');
const inheritedClean = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^MIR2_/i.test(key) && key.toLowerCase() !== 'psmodulepath' && key !== 'CRYSTAL_CLIENT_ROOT'));
const env = { ...inheritedClean };
Object.assign(env, { MIR2_GATEWAY_TCP_ADDR: '127.0.0.1:17500', MIR2_GATEWAY_WEB_ADDR: '127.0.0.1:17510', MIR2_GATEWAY_TOKIO_WORKER_THREADS: '4', MIR2_GATEWAY_STDIN_SHUTDOWN: '1', MIR2_GATEWAY_SLOW_STAGE_MS: '50',
  MIR2_GATEWAY_MAX_WS_CONNECTIONS: '24', MIR2_GATEWAY_MAX_ACTIVE_SESSIONS: '10', MIR2_GATEWAY_MAX_RECONNECT_LEASES: '10', MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT: '4', MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT: '1', MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT: '2',
  MIR2_ACCOUNT_STORE_BACKEND: 'file', MIR2_ACCOUNT_STORE_PATH: path.join(privateRoot, 'accounts.json'), MIR2_SAVE_RECOVERY_DIR: path.join(privateRoot, 'save-recovery'),
  MIR2_SAVE_RECOVERY_MAC_KEY: secrets[0], MIR2_IDENTITY_SESSION_SECRET: secrets[1], MIR2_IDENTITY_RECOVERY_PEPPER: secrets[2], MIR2_PASSKEY_AUTH_SECRET: secrets[3],
  MIR2_GATEWAY_ID: 'p01-release-native-10-v2-halo-feasibility-09', MIR2_ZONE_ROUTING_MODE: 'single', MIR2_CONTENT_PROFILE: 'platinum_176', MIR2_MONTHLY_CARD_REQUIRED: '0', MIR2_RUNTIME_ENV: 'development',
  MIR2_ALLOWED_WEB_ORIGINS: 'http://127.0.0.1:17510', MIR2_SPECTATOR_DATA_DIR: path.join(root, 'recordings'), RUST_LOG: 'warn' });
const attachLogs = (child, prefix) => { for (const [name, stream] of [['stdout', child.stdout], ['stderr', child.stderr]]) { const file = createWriteStream(path.join(privateRoot, prefix + '.' + name + '.log'), { flags: 'wx', mode: 0o600 }); stream.pipe(file); } };
const track = child => { let status; const promise = new Promise(resolve => { child.once('exit', (code, signal) => { status = { code, signal }; resolve(status); }); child.once('error', () => { status = { code: null, signal: null, spawnError: true }; resolve(status); }); }); return { child, promise, get status() { return status; } }; };
const within = async (promise, milliseconds) => { let timer; try { return await Promise.race([promise, new Promise(resolve => { timer = setTimeout(() => resolve(null), milliseconds); })]); } finally { clearTimeout(timer); } };
let gateway, monitor, report, slowStageCapture, stopRequested = false, stopRelay, reportError;
const cleanup = { schemaVersion: 1, scope: 'Owned non-accepting probe process cleanup only. Driver report and complete raw verifier adjudicate capacity separately; no forced normal-exit claim.', sourceCommit: build.sourceCommit, backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit, sha256: build.sha256, capacityAccepted: false, capacityAcceptanceAdjudicated: false, formalStarted: false, checks: {} };
const requestStop = () => { stopRequested = true; };
process.on('SIGINT', requestStop); process.on('SIGTERM', requestStop);
try {
  const startedAtUtc = new Date().toISOString();
  gateway = track(spawn(build.ownedExecutable, [], { cwd: project, env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }));
  attachLogs(gateway.child, 'gateway');
  slowStageCapture = createSlowStagePublicCapture({ input: gateway.child.stderr, outputFile: path.join(root, 'slow-stage-public.jsonl'), requestStop });
  cleanup.pid = gateway.child.pid; cleanup.ownedExe = build.ownedExecutable;
  await writeNew(path.join(root, 'realm-process.json'), { schemaVersion: 1, pid: gateway.child.pid, runnerPid: process.pid, exe: build.ownedExecutable, sha256: build.sha256, sourceCommit: build.sourceCommit, backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit,
    endpoint, startedAtUtc, tokioWorkers: 4, content: 'platinum_176', routing: 'single', accountBackend: 'file', explicitAdmissionCaps: limits,
    sceneProfile: scene.profile, sceneSourceSha256: scene.sourceSha256, scene10Sha256: scene.probeScene.sha256, scene100Sha256: scene.formalScene.sha256, sceneAnalysisSha256: scene.analysis.sha256, patrolSpan: 4, attackResults: 'v2', slowStageThresholdMs: 50, slowStagePublicSchema: 'socket-context-v1', slowStageHelperSha256: 'c2a137084f2166cbae603f721c6dde7322790c8dc2e05c2b8d59b79d7a58619f', full16RunId: binding.full16Gate.runId, slowStagePublicFile: path.join(root, 'slow-stage-public.jsonl'), stdinShutdown: true, freshPrivateSecrets: 4, parallelCompilation: false, capacityAcceptanceEligible: false, capacityAcceptanceActuallyPassed: false,
    spectatorDataDir: path.join(root, 'recordings'), spectatorEnabledFromFrozenDefault: true, spectatorRecordingFromFrozenDefault: true, spectatorPublicFromFrozenDefault: true });
  let initial;
  const startupDeadline = Date.now() + 90000;
  while (!initial) {
    if (stopRequested || gateway.status) throw new Error('Owned gateway stopped or drain requested before ready');
    try { initial = await health(gateway.child.pid); } catch (error) { if (Date.now() >= startupDeadline) throw error; await sleep(500); }
  }
  const identity = processIdentity(gateway.child.pid);
  if (!identity || !samePath(identity.exe, build.ownedExecutable) || currentKeys.some(key => initial.capacity[key] !== 0)) throw new Error('Fresh owned process does not have six zero counters');
  cleanup.processIdentityBefore = identity;
  await writeNew(path.join(root, 'spectator-health-before.json'), spectator(initial, gateway.child.pid, 'before', build.sourceCommit));
  const options = await read(path.join(root, 'run-options.json'));
  Object.assign(options, { notStarted: false, formalRunStarted: false, viable10GatePending: false, loginNotBefore: startedAtUtc, repositoryCommit: build.driverSourceCommit, backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit, admissionLedgerScope: 'Fresh File realm; zero prior Login; persistent original auth ledger and serialized ordinary NewCharacter; no limiter changes', spectatorDataDir: path.join(root, 'recordings') });
  await writeNew(path.join(root, 'run-options-admitted-v2.json'), options);
  const monitorOptions = { gatewayPid: gateway.child.pid, gatewayExe: build.ownedExecutable, expectedSha256: build.sha256, healthUrl, output: path.join(root, 'monitor-current.json'), history: path.join(root, 'monitor-history.jsonl'), resourceProfile: path.join(root, 'resource-profile.json'), intervalMs: 1000, wallSeconds: 3600 };
  await writeNew(path.join(root, 'monitor-options.json'), monitorOptions);
  const extraFiles = await Promise.all(['realm-process.json', 'spectator-health-before.json', 'run-options-admitted-v2.json', 'monitor-options.json', 'source-pins.json'].map(name => pin(path.join(root, name))));
  const finalPins = [...frozen.files, ...extraFiles];
  await writeNew(path.join(root, 'source-pins-admitted-v2.json'), { schemaVersion: 1, sourceCommit: build.sourceCommit, backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit,
    socketDiagnosticsReleaseUsed: true, driverFrozenManifest: frozen.driverFrozenManifest, driverSourceSha256: frozen.driverSourceSha256, backendSourceManifest: binding.manifestPin.file,
    primaryBuildPin: path.join(root, 'gateway-build-pin.json'), preparedAt: new Date().toISOString(), sourceConfigSha256: sha(JSON.stringify(finalPins)), files: finalPins, capacityAccepted: false });
  monitor = track(spawn(process.execPath, [path.join(root, 'monitor-owned.mjs'), '--owned-monitor'], { cwd: project, env: inheritedClean, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe', 'ipc'] }));
  attachLogs(monitor.child, 'monitor');
  await writeNew(path.join(root, 'monitor-process.json'), { schemaVersion: 1, pid: monitor.child.pid, runnerPid: process.pid, exe: process.execPath, gatewayPid: gateway.child.pid, control: 'Private parent-child IPC stop-owned-monitor; no network control listener' });
  const monitorDeadline = Date.now() + 15000;
  while (true) {
    if (monitor.status || gateway.status) throw new Error('Owned monitor/Gateway exited before load');
    try { const sample = await read(path.join(root, 'monitor-current.json')); if (sample.stop === false && sample.process?.pid === gateway.child.pid && sample.zoneMetrics?.complete === true) break; if (sample.stop) throw new Error('Resource monitor refused the frozen profile'); }
    catch (error) { if (error.code !== 'ENOENT' && !(error instanceof SyntaxError)) throw error; }
    if (Date.now() >= monitorDeadline) throw new Error('No complete valid resource sample before load'); await sleep(200);
  }
  console.log(JSON.stringify({ phase: 'owned-native-halo-probe09-ready', pid: gateway.child.pid, monitorPid: monitor.child.pid, sha256: build.sha256, sourceCommit: build.sourceCommit, backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit, explicitAttackResults: 'v2', endpoint }));
  const { runCapacity, parseArguments } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs')));
  const explicitMode = parseArguments(process.argv.slice(3));
  if (explicitMode.attackResults !== 'v2' || options.attackResults !== 'v2') throw new Error('Explicit V2 CLI and frozen profile must agree');
  stopRelay = setInterval(async () => { try { const stop = await read(path.join(root, 'stop-request.json')); if (stop.gatewayPid === gateway.child.pid) stopRequested = true; } catch (error) { if (error.code !== 'ENOENT') stopRequested = true; }
    if (stopRequested) process.emit('SIGTERM'); }, 500);
  report = await runCapacity({ ...options, ...explicitMode, accountsFile: path.join(privateRoot, 'capacity-pool.json'), monitor: path.join(root, 'monitor-current.json'), output: path.join(root, 'reports'), onProgress: value => console.log(JSON.stringify(value)) });
  await verifyDriverReportContract(report);
  const reportPin = await pin(report.reportPath);
  await writeNew(path.join(root, 'report-provenance.json'), { schemaVersion: 1, runId: report.runId, report: reportPin,
    backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit, backendBinarySha256: build.sha256,
    driverFrozenManifest: frozen.driverFrozenManifest, driverSourceSha256: frozen.driverSourceSha256, socketDiagnosticsReleaseUsed: true,
    capacityAccepted: false, sceneProfile: scene.profile, sceneSourceSha256: scene.sourceSha256, navigationCoordination: report.navigationCoordination, nativePatrolPlanning: report.nativePatrolPlanning, measurementClock: report.measurementClock, full16RunId: binding.full16Gate.runId,
    scope: 'Identity sidecar only. Original probe driver report bytes and verdict are unchanged; independent complete raw verifier adjudicates acceptance.',
    capacityAcceptanceAdjudicated: false });
  cleanup.driverPlayableCapacityAccepted = report.playableCapacityAccepted === true;
  cleanup.report = { reportPath: report.reportPath, runId: report.runId, backendSourceCommit: build.backendSourceCommit, driverSourceCommit: build.driverSourceCommit, ok: report.ok, playableCapacityAccepted: report.playableCapacityAccepted, error: report.error };
} catch (error) { reportError = error.message; cleanup.error = reportError; }
finally {
  clearInterval(stopRelay);
  if (gateway?.child.pid && !gateway.status) {
    const drainDeadline = Date.now() + 90000;
    while (true) {
      try { const last = await health(gateway.child.pid); cleanup.preStopCapacity = Object.fromEntries(currentKeys.map(key => [key, last.capacity[key]]));
        if (currentKeys.every(key => last.capacity[key] === 0)) { cleanup.checks.sixCountersZero = true; await writeNew(path.join(root, 'spectator-health-after.json'), spectator(last, gateway.child.pid, 'after', build.sourceCommit)); break; } }
      catch (error) { cleanup.healthDrainError = error.message; if (gateway.status) break; }
      if (Date.now() >= drainDeadline) { cleanup.checks.sixCountersZero = false; break; } await sleep(1000);
    }
  }
  // Stop the owned producer before the owned gateway, keeping its terminal marker.
  if (monitor) {
    if (!monitor.status && monitor.child.connected) monitor.child.send({ type: 'stop-owned-monitor' });
    cleanup.monitorExit = monitor.status ?? await within(monitor.promise, 15000);
    try { const terminal = await read(path.join(root, 'monitor-current.json')); cleanup.monitorTerminal = { sampledAt: terminal.sampledAt, stop: terminal.stop, reason: terminal.reason };
      cleanup.checks.monitorTerminalPreserved = terminal.stop === true && Number.isSafeInteger(terminal.sampledAt); }
    catch { cleanup.monitorTerminalMissing = true; }
    cleanup.checks.monitorStoppedBeforeGateway = cleanup.monitorExit !== null;
    cleanup.checks.monitorExitZero = cleanup.monitorExit?.code === 0;
  }
  if (gateway && !gateway.status && cleanup.checks.sixCountersZero === true && (!monitor || cleanup.checks.monitorStoppedBeforeGateway)) {
    const identity = processIdentity(gateway.child.pid);
    if (!identity || !samePath(identity.exe, build.ownedExecutable) || (await pin(build.ownedExecutable)).sha256 !== build.sha256) cleanup.identityError = 'Owned identity changed; no shutdown sent';
    else { gateway.child.stdin.end('shutdown\n'); cleanup.gatewayExit = await within(gateway.promise, 45000); /* Normal cleanup is recorded after the metadata stream is finished. */ }
  } else if (gateway) cleanup.gatewayExit = gateway.status ?? null;
  if (slowStageCapture) {
    cleanup.slowStageDiagnostics = await slowStageCapture.finish({ gatewayExited: Boolean(gateway?.status) });
    await writeNew(path.join(root, 'slow-stage-public-status.json'), { ...cleanup.slowStageDiagnostics, thresholdMs: 50, dataFile: path.join(root, 'slow-stage-public.jsonl') });
  } else cleanup.slowStageDiagnostics = { complete: false, reasons: ['capture-not-created'] };
  cleanup.checks.normalOwnedShutdown = cleanup.gatewayExit?.code === 0;
  cleanup.checks.processAbsent = !gateway?.child.pid || processIdentity(gateway.child.pid) === null;
  cleanup.remainingOwnedPortListeners = listeners(); cleanup.checks.portsFree = cleanup.remainingOwnedPortListeners.length === 0;
  for (const entry of frozen.files) { try { await assertPin(entry); } catch { cleanup.inputsChangedWhileRunning = true; break; } }
  try { const ending = await verifyRelease(); if (ending.backendSourceCommit !== build.backendSourceCommit || ending.driverSourceCommit !== build.driverSourceCommit) cleanup.inputsChangedWhileRunning = true; }
  catch (error) { cleanup.inputsChangedWhileRunning = true; cleanup.finalSourceGuardError = error.message; }
  cleanup.stoppedAtUtc = new Date().toISOString();
  await writeNew(path.join(root, 'realm-cleanup.json'), cleanup);
  process.off('SIGINT', requestStop); process.off('SIGTERM', requestStop);
}
const cleanupComplete = cleanup.checks.sixCountersZero === true && cleanup.checks.normalOwnedShutdown === true && cleanup.checks.processAbsent && cleanup.checks.portsFree &&
  (!monitor || (cleanup.checks.monitorStoppedBeforeGateway && cleanup.checks.monitorExitZero && cleanup.checks.monitorTerminalPreserved)) && !cleanup.inputsChangedWhileRunning && cleanup.slowStageDiagnostics?.complete === true;
console.log(JSON.stringify({ phase: 'finished', root, report: cleanup.report, error: reportError, cleanupComplete, checks: cleanup.checks, capacityAccepted: false, capacityAcceptanceAdjudicated: false, formalStarted: false }));
process.exitCode = report?.ok === true && cleanupComplete ? 0 : 1;
