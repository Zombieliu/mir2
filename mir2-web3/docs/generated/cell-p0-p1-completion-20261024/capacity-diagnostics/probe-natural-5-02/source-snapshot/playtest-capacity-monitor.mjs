// External Windows resource monitor for an explicitly isolated capacity realm.
// It never changes affinity, quotas, admission, accounts or gameplay. Thresholds
// are mandatory in a frozen profile; measured resources are not enforced quotas.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';
import { outsideRepository } from './playtest-multiplayer-smoke.mjs';
import { inspectMonitor } from './playtest-load-smoke.mjs';

const CURRENT = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases',
  'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
const LIMITS = ['maxWsConnections', 'maxActiveSessions', 'maxReconnectLeases',
  'maxLoginInFlight', 'maxNewCharacterInFlight', 'maxStartGameInFlight'];
const integer = value => Number.isSafeInteger(value) && value >= 0;

export function validateResourceProfile(value) {
  if (value?.schemaVersion !== 1 || !/^[a-zA-Z0-9][a-zA-Z0-9._-]{2,100}$/.test(value.profileId ?? '') ||
      value.realm !== 'isolated' || value.resourceLimitsEnforcedByMonitor !== false ||
      !['maxPrivateBytes', 'maxWorkingSetBytes', 'maxCpuCores', 'maxIoBytesPerSecond', 'maxIoOperationsPerSecond']
        .every(key => Number.isFinite(value[key]) && value[key] > 0) ||
      !integer(value.minHostFreeBytes) || !integer(value.hostLogicalProcessors) || value.hostLogicalProcessors < 1 ||
      value.maxCpuCores > value.hostLogicalProcessors ||
      !Number.isSafeInteger(value.sustainedSamples) || value.sustainedSamples < 3 || value.sustainedSamples > 30 ||
      !['cpuLimit', 'memoryLimit', 'storage', 'database', 'network', 'tls', 'failureDomain']
        .every(key => typeof value.environment?.[key] === 'string' && value.environment[key].length > 0 && value.environment[key].length <= 512)) {
    throw new Error('Resource profile requires explicit isolated identity, measured thresholds and honest environment/limit metadata');
  }
  if (value.maxHostNetworkBytesPerSecond != null && !(Number.isFinite(value.maxHostNetworkBytesPerSecond) && value.maxHostNetworkBytesPerSecond > 0)) {
    throw new Error('Optional host NIC throughput threshold must be positive');
  }
  return structuredClone(value);
}

export function validateHealthUrl(value) {
  const url = new URL(value);
  if (url.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(url.hostname) || !url.port ||
      url.pathname !== '/health' || url.username || url.password || url.search || url.hash) {
    throw new Error('Monitor requires a literal loopback http://host:port/health endpoint');
  }
  return url.href;
}

export function publicCapacityHealth(body) {
  if (body?.ok !== true || body.http !== 'ready' || body.ws !== 'ready' || body.session_cache?.healthy !== true ||
      !CURRENT.every(key => integer(body.capacity?.[key])) ||
      !LIMITS.every(key => Number.isSafeInteger(body.capacity?.[key]) && body.capacity[key] > 0)) {
    throw new Error('Gateway health is unavailable, unhealthy or lacks finite explicit admission caps');
  }
  const health = Object.fromEntries([...CURRENT, ...LIMITS].map(key => [key, body.capacity[key]]));
  for (let i = 0; i < CURRENT.length; i++) {
    if (health[CURRENT[i]] > health[LIMITS[i]]) throw new Error('Gateway capacity exceeds its declared admission cap');
  }
  return health;
}

export function validateProcessSample(value, identity) {
  if (!value || value.error || value.pid !== identity.pid ||
      String(value.executablePath).toLowerCase() !== identity.executablePath.toLowerCase() ||
      !Number.isSafeInteger(value.sampledAt) || !Number.isSafeInteger(value.creationTimeMs) ||
      !['cpuTimeMs', 'workingSetBytes', 'privateBytes', 'readBytes', 'writeBytes', 'otherIoBytes',
        'readOperations', 'writeOperations', 'otherIoOperations', 'hostFreeBytes', 'hostTotalBytes', 'handles', 'threads']
        .every(key => integer(value[key])) || !Array.isArray(value.cores) || value.cores.length !== identity.hostLogicalProcessors ||
      !value.cores.every(core => typeof core.id === 'string' && Number.isFinite(core.busyPercent) && core.busyPercent >= 0 && core.busyPercent <= 100) ||
      new Set(value.cores.map(core => core.id)).size !== value.cores.length || !Array.isArray(value.adapters) ||
      !value.adapters.every(adapter => typeof adapter.name === 'string' && integer(adapter.receivedBytes) && integer(adapter.sentBytes)) ||
      new Set(value.adapters.map(adapter => adapter.name)).size !== value.adapters.length) {
    throw new Error('Process resource sample is incomplete or process/CPU identity changed');
  }
  if (identity.healthPort != null) {
    const matching = value.listeners?.filter(listener => listener.port === identity.healthPort && listener.address === identity.healthHost);
    if (!matching?.length || !matching.every(listener => listener.pid === identity.pid)) {
      throw new Error('Public health listener is not owned by the pinned Gateway PID');
    }
  }
  return value;
}

function nonnegativeDelta(current, prior, key) {
  const result = current[key] - prior[key];
  if (result < 0) throw new Error(`Process cumulative resource counter moved backwards: ${key}`);
  return result;
}

export class ResourceEnvelope {
  constructor(profile, identity) { this.profile = validateResourceProfile(profile); this.identity = identity; this.over = new Map(); this.stopped = false; }
  stop(reason) { this.stopped = true; this.reason ??= reason; }
  sample(raw, body, now = Date.now()) {
    const current = validateProcessSample(raw, this.identity), health = publicCapacityHealth(body);
    if (now - current.sampledAt > 10000 || current.sampledAt - now > 5000) throw new Error('OS sample is stale or from the future');
    if (this.creationTimeMs != null && current.creationTimeMs !== this.creationTimeMs) throw new Error('Gateway PID was reused');
    this.creationTimeMs = current.creationTimeMs;
    const prior = this.previous, elapsedSeconds = prior ? (current.sampledAt - prior.sampledAt) / 1000 : null;
    if (prior && !(elapsedSeconds > 0 && elapsedSeconds <= 15)) throw new Error('OS resource sample clock gap or reversal');
    if (prior && JSON.stringify(current.adapters.map(adapter => adapter.name).sort()) !==
        JSON.stringify(prior.adapters.map(adapter => adapter.name).sort())) {
      throw new Error('Host NIC inventory changed; profile continuity lost');
    }
    const rates = prior ? {
      cpuCores: nonnegativeDelta(current, prior, 'cpuTimeMs') / (elapsedSeconds * 1000),
      readBytesPerSecond: nonnegativeDelta(current, prior, 'readBytes') / elapsedSeconds,
      writeBytesPerSecond: nonnegativeDelta(current, prior, 'writeBytes') / elapsedSeconds,
      otherIoBytesPerSecond: nonnegativeDelta(current, prior, 'otherIoBytes') / elapsedSeconds,
      ioOperationsPerSecond: ['readOperations', 'writeOperations', 'otherIoOperations'].reduce((sum, key) => sum + nonnegativeDelta(current, prior, key), 0) / elapsedSeconds,
    } : null;
    const hostAdapters = current.adapters.map(adapter => {
      const before = prior?.adapters.find(item => item.name === adapter.name);
      if (!before) return { ...adapter, receivedBytesPerSecond: null, sentBytesPerSecond: null };
      if (adapter.receivedBytes < before.receivedBytes || adapter.sentBytes < before.sentBytes) throw new Error('Host NIC counter reset; profile continuity lost');
      return { ...adapter, receivedBytesPerSecond: (adapter.receivedBytes - before.receivedBytes) / elapsedSeconds,
        sentBytesPerSecond: (adapter.sentBytes - before.sentBytes) / elapsedSeconds };
    });
    const hostNetworkRate = prior ? hostAdapters.reduce((sum, adapter) => sum + (adapter.receivedBytesPerSecond ?? 0) + (adapter.sentBytesPerSecond ?? 0), 0) : null;
    if (current.privateBytes > this.profile.maxPrivateBytes) this.stop('Gateway private memory exceeded the frozen resource envelope');
    if (current.workingSetBytes > this.profile.maxWorkingSetBytes) this.stop('Gateway working set exceeded the frozen resource envelope');
    if (current.hostFreeBytes < this.profile.minHostFreeBytes) this.stop('Host available memory fell below the frozen safety reserve');
    const checks = [
      ['cpu', rates?.cpuCores, this.profile.maxCpuCores],
      ['ioBytes', rates && rates.readBytesPerSecond + rates.writeBytesPerSecond + rates.otherIoBytesPerSecond, this.profile.maxIoBytesPerSecond],
      ['ioOperations', rates?.ioOperationsPerSecond, this.profile.maxIoOperationsPerSecond],
      ['hostNetwork', hostNetworkRate, this.profile.maxHostNetworkBytesPerSecond],
    ];
    for (const [kind, measured, limit] of checks) {
      const count = measured != null && limit != null && measured > limit ? (this.over.get(kind) ?? 0) + 1 : 0;
      this.over.set(kind, count);
      if (count >= this.profile.sustainedSamples) this.stop(`${kind} exceeded the frozen resource envelope for ${count} consecutive samples`);
    }
    this.previous = structuredClone(current);
    const sample = { schemaVersion: 1, sampledAt: now, stop: this.stopped, ...(this.reason ? { reason: this.reason } : {}),
      profileId: this.profile.profileId, health, memory: { currentBytes: current.privateBytes,
        definition: 'Win32_Process.PrivatePageCount committed private bytes; working set separately recorded', workingSetBytes: current.workingSetBytes },
      process: { pid: current.pid, creationTimeMs: current.creationTimeMs, cpuTimeMs: current.cpuTimeMs,
        handles: current.handles, threads: current.threads, rates, rawCounters: { readBytes: current.readBytes,
          writeBytes: current.writeBytes, otherIoBytes: current.otherIoBytes, readOperations: current.readOperations,
          writeOperations: current.writeOperations, otherIoOperations: current.otherIoOperations },
        ioScope: 'Windows process I/O counters include file/device I/O and are not pure disk throughput or latency' },
      host: { freeBytes: current.hostFreeBytes, totalBytes: current.hostTotalBytes, cores: current.cores,
        cpuScope: 'Host-wide logical-core utilization, including processes outside this realm' },
      network: { adapters: hostAdapters, hostBytesPerSecond: hostNetworkRate, processBytesPerSecond: null,
        scope: 'Host NIC cumulative counters; loopback traffic may be absent. Realm WebSocket payload bytes are measured separately by the gameplay driver. No TLS/PPS/physical-link capacity inferred.' },
      environment: this.profile.environment, resourceLimitsEnforcedByMonitor: false };
    // inspectMonitor intentionally throws for a real stop request; still return
    // that request so the producer can persist it before the driver drains.
    if (!sample.stop) inspectMonitor(sample, now);
    return sample;
  }
}

// A single long-lived sampler avoids launching PowerShell for every 1 Hz sample.
// The only external input is the numeric PID and finite interval in environment.
export const WINDOWS_SAMPLER = String.raw`
$ErrorActionPreference='Stop'
$wanted=[int]$env:MIR2_CAPACITY_MONITOR_PID
$interval=[int]$env:MIR2_CAPACITY_MONITOR_INTERVAL_MS
$healthPort=[int]$env:MIR2_CAPACITY_MONITOR_HEALTH_PORT
while($true){
  try{
    $p=Get-CimInstance Win32_Process -Filter "ProcessId=$wanted"
    if($null -eq $p){throw 'owned process unavailable'}
    $os=Get-CimInstance Win32_OperatingSystem
    $cores=@(Get-CimInstance Win32_PerfFormattedData_PerfOS_Processor | Where-Object {$_.Name -ne '_Total'} | ForEach-Object {@{id=[string]$_.Name;busyPercent=[double]$_.PercentProcessorTime}})
    $adapters=@(Get-CimInstance Win32_PerfRawData_Tcpip_NetworkInterface | ForEach-Object {@{name=[string]$_.Name;receivedBytes=[double]$_.BytesReceivedPersec;sentBytes=[double]$_.BytesSentPersec}})
    $listeners=@();if($healthPort -gt 0){$listeners=@(Get-NetTCPConnection -State Listen -LocalPort $healthPort -ErrorAction Stop|ForEach-Object {@{address=[string]$_.LocalAddress;port=[int]$_.LocalPort;pid=[int]$_.OwningProcess}})}
    $r=@{sampledAt=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();pid=[int]$p.ProcessId;executablePath=[string]$p.ExecutablePath;creationTimeMs=([DateTimeOffset]$p.CreationDate).ToUnixTimeMilliseconds();cpuTimeMs=[math]::Floor(([double]$p.KernelModeTime+[double]$p.UserModeTime)/10000);workingSetBytes=[double]$p.WorkingSetSize;privateBytes=[double]$p.PrivatePageCount;readBytes=[double]$p.ReadTransferCount;writeBytes=[double]$p.WriteTransferCount;otherIoBytes=[double]$p.OtherTransferCount;readOperations=[double]$p.ReadOperationCount;writeOperations=[double]$p.WriteOperationCount;otherIoOperations=[double]$p.OtherOperationCount;handles=[int]$p.HandleCount;threads=[int]$p.ThreadCount;hostFreeBytes=[double]$os.FreePhysicalMemory*1024;hostTotalBytes=[double]$os.TotalVisibleMemorySize*1024;cores=$cores;adapters=$adapters;listeners=$listeners}
    [Console]::Out.WriteLine(($r|ConvertTo-Json -Compress -Depth 6))
  }catch{[Console]::Out.WriteLine('{"error":"OS resource sampler failed; details suppressed"}');exit 1}
  Start-Sleep -Milliseconds $interval
}`;

export function windowsSamplerEnvironment(pid, intervalMs, healthPort = 0) {
  // Native Windows PowerShell must load its own CIM/network modules even when
  // a caller just built Rust from a VS shell or another PowerShell edition.
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toLowerCase() !== 'psmodulepath'));
  return { ...env, MIR2_CAPACITY_MONITOR_PID: String(pid), MIR2_CAPACITY_MONITOR_INTERVAL_MS: String(intervalMs),
    MIR2_CAPACITY_MONITOR_HEALTH_PORT: String(healthPort) };
}

export function parseArguments(args) {
  const options = {}, allowed = new Set(['gatewayPid', 'gatewayExe', 'expectedSha256', 'healthUrl', 'output', 'history', 'resourceProfile', 'intervalMs', 'wallSeconds']);
  for (let i = 0; i < args.length; i += 2) {
    if (!args[i].startsWith('--') || args[i + 1] == null) throw new Error('Use --name value arguments');
    const key = args[i].slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    if (!allowed.has(key) || Object.hasOwn(options, key)) throw new Error('Unknown or duplicate monitor option');
    options[key] = args[i + 1];
  }
  return options;
}

async function atomicOwnedSample(file, sample) {
  const temporary = `${file}.next`;
  await fs.writeFile(temporary, JSON.stringify(sample) + '\n', { mode: 0o600 });
  for (let attempt = 0; ; attempt++) {
    try { await fs.rename(temporary, file); return; }
    catch (error) {
      if (!['EPERM', 'EACCES'].includes(error.code) || attempt >= 10) throw new Error('Owned monitor sample replace failed');
      await new Promise(resolve => setTimeout(resolve, 10));
    }
  }
}

export async function runMonitor(options) {
  if (process.platform !== 'win32') throw new Error('This resource producer requires Windows CIM; no synthetic resource fallback');
  const pid = Number(options.gatewayPid), intervalMs = Number(options.intervalMs ?? 1000), wallSeconds = Number(options.wallSeconds);
  if (!Number.isSafeInteger(pid) || pid <= 0 || !Number.isInteger(intervalMs) || intervalMs < 500 || intervalMs > 3000 ||
      !Number.isInteger(wallSeconds) || wallSeconds < 30 || wallSeconds > 43200 || !/^[a-fA-F0-9]{64}$/.test(options.expectedSha256 ?? '')) {
    throw new Error('Require a pinned process SHA256, positive PID, 500..3000 ms interval and explicit 30..43200 second wall budget');
  }
  const healthUrl = validateHealthUrl(options.healthUrl), executablePath = await fs.realpath(options.gatewayExe);
  if (path.basename(executablePath).toLowerCase() !== 'mir2-gateway.exe') throw new Error('Only a pinned mir2-gateway.exe process may be monitored');
  const sha256 = crypto.createHash('sha256').update(await fs.readFile(executablePath)).digest('hex');
  if (sha256 !== options.expectedSha256.toLowerCase()) throw new Error('Gateway build hash differs from the frozen profile');
  const profileFile = outsideRepository(await fs.realpath(outsideRepository(options.resourceProfile)));
  const profileBytes = await fs.readFile(profileFile), profile = validateResourceProfile(JSON.parse(profileBytes.toString('utf8')));
  const profileSha256 = crypto.createHash('sha256').update(profileBytes).digest('hex');
  const resolveOutput = async value => {
    const requested = outsideRepository(value);
    await fs.mkdir(path.dirname(requested), { recursive: true });
    return outsideRepository(path.join(await fs.realpath(path.dirname(requested)), path.basename(requested)));
  };
  const output = await resolveOutput(options.output), historyFile = await resolveOutput(options.history);
  if (output === historyFile) throw new Error('Monitor current sample and append-only history must differ');
  await fs.mkdir(path.dirname(output), { recursive: true }); await fs.mkdir(path.dirname(historyFile), { recursive: true });
  // The current file and history must both be fresh. No previous realm record is replaced.
  const current = await fs.open(output, 'wx', 0o600); await current.close();
  const history = await fs.open(historyFile, 'wx', 0o600);
  const healthAddress = new URL(healthUrl);
  const envelope = new ResourceEnvelope(profile, { pid, executablePath, hostLogicalProcessors: profile.hostLogicalProcessors,
    healthPort: Number(healthAddress.port), healthHost: healthAddress.hostname.replace(/^\[|\]$/g, '') });
  const sampler = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', WINDOWS_SAMPLER], {
    windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'], env: windowsSamplerEnvironment(pid, intervalMs, Number(healthAddress.port)),
  });
  const lines = createInterface({ input: sampler.stdout, crlfDelay: Infinity });
  let lastSample, samples = 0, timer, forced, samplerError;
  sampler.on('error', error => { samplerError = error; lines.close(); });
  const stop = reason => { forced ??= reason; sampler.kill(); lines.close(); };
  const signal = () => stop('Operator requested load drain');
  process.on('SIGINT', signal); process.on('SIGTERM', signal);
  timer = setTimeout(() => stop('Finite resource-monitor wall budget exhausted'), wallSeconds * 1000);
  const staleTimer = setInterval(() => { if (lastSample && Date.now() - lastSample.sampledAt > 12000) stop('OS resource producer became stale'); }, 1000);
  try {
    await history.writeFile(JSON.stringify({ type: 'monitorStarted', schemaVersion: 1, startedAt: Date.now(),
      pid, gatewaySha256: sha256, resourceProfileSha256: profileSha256, profile, intervalMs, wallSeconds }) + '\n');
    for await (const line of lines) {
      let sample;
      try {
        if (line.length > 256 * 1024) throw new Error('OS resource sample exceeds its finite bound');
        const raw = JSON.parse(line), response = await fetch(healthUrl, { signal: AbortSignal.timeout(3000), redirect: 'error' });
        if (!response.ok) throw new Error('Public Gateway health request failed');
        sample = envelope.sample(raw, await response.json());
      } catch (error) {
        envelope.stop(error.message); sample = { ...(lastSample ?? {}), schemaVersion: 1, sampledAt: Date.now(), stop: true, reason: envelope.reason };
      }
      lastSample = sample; samples++;
      await history.appendFile(JSON.stringify({ type: 'monitor', sample }) + '\n'); await history.sync();
      await atomicOwnedSample(output, sample);
      if (sample.stop) break;
    }
    if (!envelope.stopped) envelope.stop(forced ?? (samplerError ? 'OS resource sampler could not start' : 'OS resource producer exited unexpectedly'));
    const terminal = { ...(lastSample ?? {}), schemaVersion: 1, sampledAt: Date.now(), stop: true, reason: envelope.reason };
    await history.appendFile(JSON.stringify({ type: 'monitorStopped', sample: terminal, samples }) + '\n'); await history.sync();
    await atomicOwnedSample(output, terminal);
    return { samples, stopped: true, reason: envelope.reason, output, history: historyFile };
  } finally {
    clearTimeout(timer); clearInterval(staleTimer); sampler.kill(); lines.close(); await history.close();
    process.off('SIGINT', signal); process.off('SIGTERM', signal);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  runMonitor(parseArguments(process.argv.slice(2))).then(result => console.log(JSON.stringify(result)))
    .catch(error => { console.error(JSON.stringify({ passed: false, error: error.message })); process.exitCode = 1; });
}
