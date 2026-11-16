import fs from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';

const repo = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const taskRoot = "C:\\mir2-build\\p01-completion-20261024\\full-lifecycle-scout-observer-preference-v1";
const privateRoot = path.join(taskRoot, 'private');
const output = "C:\\mir2-build\\p01-completion-20261024\\full-lifecycle-scout-observer-preference-v1\\public";
const stamp = Date.now() + '-' + crypto.randomBytes(4).toString('hex');
const endpoint = 'ws://127.0.0.1:17610/ws';
const reportPath = path.join(output, stamp + '.realm-evidence.json');
// Frozen driver modules are imported only after all six raw pins pass.
const fields = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases',
  'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
const secretNames = ['recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt'];
const { loadPreparedInputs } = await import('./verify-inputs.mjs');
const { profile, releaseInput, backendBytes, backendManifest, backendInputCount, releaseRecordBytes } = await loadPreparedInputs(repo, taskRoot);
const { runPlaytest, redact } = await import(pathToFileURL(path.join(repo, 'apps/web/scripts/playtest-multiplayer-smoke.mjs')));
const aclEnvironment = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toLowerCase() !== 'psmodulepath'));
const privateAcl = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(taskRoot, 'initialize-fresh-private.ps1'), '-PrivateRoot', privateRoot], { encoding: 'utf8', windowsHide: true, timeout: 30000, env: aclEnvironment }).trim());
if (privateAcl.ownerOnlyVerified !== true || privateAcl.allowedPrincipalCount !== 2) throw new Error('Private ACL was not verified');
const secrets = secretNames.map(() => crypto.randomBytes(32).toString('hex'));
await Promise.all(secretNames.map((name, index) => fs.writeFile(path.join(privateRoot, name), secrets[index], { flag: 'wx', mode: 0o600 })));
const hash = async file => crypto.createHash('sha256').update(await fs.readFile(file)).digest('hex');
const psEnvironment = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toLowerCase() !== 'psmodulepath'));
const ps = command => execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from("$ProgressPreference='SilentlyContinue';" + command, 'utf16le').toString('base64')],
  { encoding: 'utf8', windowsHide: true, env: psEnvironment, timeout: 30000 }).trim();
const listeners = () => JSON.parse(ps("$ErrorActionPreference='Stop';$rows=@(Get-NetTCPConnection -State Listen -ErrorAction Stop|Where-Object {$_.LocalPort -in @(17600,17610)}|ForEach-Object {@{address=[string]$_.LocalAddress;port=[int]$_.LocalPort;pid=[int]$_.OwningProcess}});ConvertTo-Json -InputObject $rows -Compress"));
if (listeners().length) throw new Error('Owned ports already have listeners; no launch permitted');
try { await fs.access(path.join(privateRoot, 'accounts.json')); throw new Error('Refusing reused account store'); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
const ownExe = path.join(privateRoot, 'mir2-gateway-release.exe');
await fs.copyFile(releaseInput.executable, ownExe, 1);
if (await hash(ownExe) !== releaseInput.sha256.toLowerCase()) throw new Error('Copied release SHA differs from frozen input');
await fs.mkdir(output, { recursive: true });
await fs.copyFile(import.meta.filename, path.join(output, stamp + '.runner-source.mjs'));
await fs.writeFile(path.join(output, stamp + '.release-input.json'), releaseRecordBytes, { flag: 'wx' });
await fs.writeFile(path.join(output, stamp + '.backend-source.json'), backendBytes, { flag: 'wx' });
await fs.writeFile(path.join(output, stamp + '.prepared-profile.json'), JSON.stringify(profile, null, 2) + '\n', { flag: 'wx' });
const sourceFiles = ['apps/web/scripts/playtest-multiplayer-smoke.mjs', 'apps/web/scripts/playtest-lifecycle.mjs',
  'apps/web/scripts/playtest-catalog-transport.mjs', 'apps/web/scripts/quest-agent/protocol-play.mjs',
  'apps/web/scripts/quest-agent/protocol-navigation.mjs', 'apps/web/scripts/quest-agent/protocol-observation.mjs'];
const sourcePins = Object.fromEntries(await Promise.all(sourceFiles.map(async file => [file, await hash(path.join(repo, file))])));
const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^MIR2_/i.test(key) && key !== 'CRYSTAL_CLIENT_ROOT'));
Object.assign(env, {
  MIR2_GATEWAY_TCP_ADDR: '127.0.0.1:17600', MIR2_GATEWAY_WEB_ADDR: '127.0.0.1:17610',
  MIR2_GATEWAY_TOKIO_WORKER_THREADS: '4', MIR2_GATEWAY_STDIN_SHUTDOWN: '1',
  MIR2_GATEWAY_SLOW_STAGE_MS: '50',
  MIR2_ACCOUNT_STORE_BACKEND: 'file', MIR2_ACCOUNT_STORE_PATH: path.join(privateRoot, 'accounts.json'),
  MIR2_SAVE_RECOVERY_DIR: path.join(privateRoot, 'save-recovery'),
  MIR2_SPECTATOR_DATA_DIR: path.join(privateRoot, 'spectator-buffered'), MIR2_SPECTATOR_ENABLED: '1', MIR2_SPECTATOR_RECORDING_ENABLED: '1', MIR2_AI_LIVE_DATA_DIR: path.join(privateRoot, 'ai-live'),
  MIR2_SAVE_RECOVERY_MAC_KEY: secrets[0], MIR2_IDENTITY_SESSION_SECRET: secrets[1],
  MIR2_IDENTITY_RECOVERY_PEPPER: secrets[2], MIR2_PASSKEY_AUTH_SECRET: secrets[3],
  MIR2_GATEWAY_ID: 'cell-p0-p1-scout-observer-preference-full-' + stamp, MIR2_ZONE_ROUTING_MODE: 'single',
  MIR2_CONTENT_PROFILE: 'platinum_176', MIR2_MONTHLY_CARD_REQUIRED: '0', MIR2_RUNTIME_ENV: 'development',
  MIR2_ALLOWED_WEB_ORIGINS: 'http://127.0.0.1:17610',
  MIR2_GATEWAY_MAX_WS_CONNECTIONS: '2048', MIR2_GATEWAY_MAX_ACTIVE_SESSIONS: '512',
  MIR2_GATEWAY_MAX_RECONNECT_LEASES: '512', MIR2_GATEWAY_MAX_LOGIN_IN_FLIGHT: '8',
  MIR2_GATEWAY_MAX_NEW_CHARACTER_IN_FLIGHT: '4', MIR2_GATEWAY_MAX_START_GAME_IN_FLIGHT: '4',
});
const record = { schemaVersion: 1, stamp, endpoint, scope: 'one-reused-c743-release-current-scout-node-h-two-ordinary-account-full-lifecycle-run',
  startedAt: new Date().toISOString(), release: { sourceExecutable: releaseInput.executable,
    executable: ownExe, sha256: releaseInput.sha256.toLowerCase(), profile: 'release',
    backendSourceManifest: releaseInput.backendSourceManifest, backendSourceAggregate: releaseInput.backendSourceAggregate,
    baseCommit: releaseInput.baseCommit, sourceCommit: releaseInput.sourceCommit, executionSourceCommit: releaseInput.executionSourceCommit, driverSourceCommit: releaseInput.driverSourceCommit, backendInputCount, backendSourceManifestFileSha256: releaseInput.backendSourceManifestFileSha256,
    releaseRecord: profile.releaseRecord, releaseRecordFileSha256: releaseInput.releaseRecordFileSha256 }, sourcePins, privateAcl,
  configuration: { routing: 'single', contentProfile: 'platinum_176', accountBackend: 'file', tokioWorkers: 4,
    slowStageThresholdMs: 50,
    bind: ['127.0.0.1:17600', '127.0.0.1:17610'], runtimeEnvironment: 'development', spectatorRecording: 'enabled; fresh independent private directory',
    playerCommandSafety: 'default-enforced; unsafe-local opt-out unset', qaTokenConfigured: false,
    explicitAdmissionCaps: [2048, 512, 512, 8, 4, 4],
    commercialInfrastructureScope: 'local File and memory cache; excludes PostgreSQL/Redis production deployment acceptance' },
  checks: {}, finalAccepted: false, visualAccepted: false, twoComputerAccepted: false, capacityAccepted: false,
  worldGhostScope: 'Public capacity counters and process ownership; no public live-outbound or manager-player census endpoint' };
const write = () => fs.writeFile(reportPath, JSON.stringify(redact(record, secrets), null, 2) + '\n');
const stdout = createWriteStream(path.join(privateRoot, stamp + '.gateway.stdout.log'), { flags: 'wx' });
const stderr = createWriteStream(path.join(privateRoot, stamp + '.gateway.stderr.log'), { flags: 'wx' });
const child = spawn(ownExe, [], { cwd: repo, env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
child.stdout.pipe(stdout); child.stderr.pipe(stderr);
const exited = once(child, 'exit');
record.pid = child.pid;
await write();
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const awaitExit = async milliseconds => {
  let timeout;
  try { return await Promise.race([exited.then(([code, signal]) => ({ code, signal })),
    new Promise(resolve => { timeout = setTimeout(() => resolve(null), milliseconds); })]); }
  finally { clearTimeout(timeout); }
};
const health = async () => {
  const response = await fetch('http://127.0.0.1:17610/health', { signal: AbortSignal.timeout(3000), redirect: 'error' });
  if (!response.ok) throw new Error('Owned health rejected');
  const body = await response.json();
  if (body.ok !== true || body.http !== 'ready' || body.ws !== 'ready' || body.session_cache?.healthy !== true ||
      fields.some(key => !Number.isSafeInteger(body.capacity?.[key]) || body.capacity[key] < 0)) throw new Error('Incomplete owned health');
  return body;
};
try {
  const startupDeadline = Date.now() + 90000;
  while (true) {
    if (child.exitCode !== null || child.signalCode !== null) throw new Error('Owned release exited before ready');
    try { record.baselineHealth = await health(); break; }
    catch (error) { if (Date.now() >= startupDeadline) throw error; await sleep(500); }
  }
  const ownedListeners = listeners();
  if (ownedListeners.length !== 2 || ownedListeners.some(row => row.address !== '127.0.0.1' || row.pid !== child.pid) ||
      ![17600, 17610].every(port => ownedListeners.some(row => row.port === port))) throw new Error('Listener ownership differs from spawned PID');
  record.listenerOwnership = ownedListeners;
  const processIdentity = JSON.parse(ps(`$ErrorActionPreference='Stop';$p=Get-CimInstance Win32_Process -Filter 'ProcessId=${child.pid}';if($null -eq $p){throw 'Missing owned process'};@{pid=[int]$p.ProcessId;executable=[string]$p.ExecutablePath;created=[string]$p.CreationDate}|ConvertTo-Json -Compress`));
  if (path.resolve(processIdentity.executable).toLowerCase() !== path.resolve(ownExe).toLowerCase()) throw new Error('PID executable path differs');
  record.processIdentity = processIdentity;
  if (fields.some(key => record.baselineHealth.capacity[key] !== 0)) throw new Error('Fresh realm has nonzero pre-run capacity');
  record.checks.freshRealm = 'passed'; await write();
  console.log(JSON.stringify({ phase: 'owned-release-ready', pid: child.pid, sha256: record.release.sha256, reportPath }));
  const result = await runPlaytest({ endpoint, realm: 'isolated', output, privateDir: path.join(privateRoot, 'registrations'),
    profile: 'full-lifecycle', durationSeconds: 900,
    onProgress: progress => console.log(JSON.stringify(progress)) });
  record.playtest = { reportPath: result.reportPath, runId: result.runId, ok: result.ok, status: result.status,
    registrationMode: result.registrationMode, requiredChecks: result.requiredChecks,
    statuses: Object.fromEntries(Object.entries(result.checks).map(([key, value]) => [key, value.status])) };
  if (result.privateCredentialsPath) {
    const accounts = JSON.parse(await fs.readFile(result.privateCredentialsPath, 'utf8'));
    secrets.push(...accounts.map(account => account.password));
  }
  const cleanupDeadline = Date.now() + 90000; record.cleanupSamples = [];
  while (true) {
    const current = await health();
    record.cleanupSamples.push({ at: new Date().toISOString(), capacity: Object.fromEntries(fields.map(key => [key, current.capacity[key]])) });
    if (fields.every(key => current.capacity[key] === 0)) { record.afterHealth = current; break; }
    if (Date.now() >= cleanupDeadline) throw new Error('Owned capacity did not return to six zeros');
    await sleep(1000);
  }
  record.checks.capacityCleanup = 'passed';
  if (await hash(ownExe) !== record.release.sha256) throw new Error('Owned release identity changed while running');
  for (const file of (await fs.readdir(output)).filter(name => name.startsWith(result.runId))) {
    const content = await fs.readFile(path.join(output, file), 'utf8');
    if (secrets.some(secret => secret && content.includes(secret))) throw new Error('Public run output contains an unredacted configured credential');
  }
  record.checks.publicValueAuditPassed = 'passed';
  for (const pin of profile.driverPins) if (await hash(path.join(repo, pin.file)) !== pin.sha256) throw new Error('Frozen lifecycle driver changed during execution');
  for (const pin of backendManifest.files) if (await hash(path.join(path.dirname(repo), pin.path)) !== pin.sha256) throw new Error('Backend source changed during execution');
  for (const pin of profile.toolPins) if (await hash(path.join(taskRoot, pin.file)) !== pin.sha256) throw new Error('Prepared launcher/audit tool changed during execution');
  if (await hash(profile.releaseRecord) !== releaseInput.releaseRecordFileSha256 || await hash(profile.backendSourceManifest) !== releaseInput.backendSourceManifestFileSha256) throw new Error('Pinned candidate build input changed during execution');
  record.backendManifestVerifiedInputs = backendInputCount;
  record.ok = result.ok === true;
} catch (error) { record.error = error.message; record.ok = false; }
finally {
  await write();
  if (child.exitCode === null && child.signalCode === null) {
    child.stdin.write('shutdown\n');
    const status = await awaitExit(45000);
    if (!status) { record.shutdownError = 'Owned process did not exit within normal shutdown budget; retained for explicit PID-safe cleanup'; record.ok = false; }
    else { record.shutdown = status; record.checks.normalOwnedShutdown = status.code === 0 ? 'passed' : 'failed'; if (status.code !== 0) record.ok = false; }
  } else { record.shutdown = { code: child.exitCode, signal: child.signalCode }; record.ok = false; }
  record.remainingOwnedPortListeners = listeners();
  if (record.remainingOwnedPortListeners.length) { record.ok = false; record.checks.portsAfterStop = 'failed'; }
  else record.checks.portsAfterStop = 'passed';
  record.finishedAt = new Date().toISOString(); await write();
  stdout.end(); stderr.end();
}
console.log(JSON.stringify({ phase: 'finished', ok: record.ok, reportPath, playtest: record.playtest, error: record.error, shutdownError: record.shutdownError }));
if (!record.ok) process.exitCode = 1;
