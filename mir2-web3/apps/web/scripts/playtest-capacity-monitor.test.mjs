import test from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import fs from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { ResourceEnvelope, validateResourceProfile, validateHealthUrl, publicCapacityHealth,
  validateProcessSample, parseArguments, WINDOWS_SAMPLER, windowsSamplerEnvironment } from './playtest-capacity-monitor.mjs';
import { inspectMonitor } from './playtest-load-smoke.mjs';

const profile = () => ({ schemaVersion: 1, profileId: 'fixture-isolated-envelope', realm: 'isolated',
  resourceLimitsEnforcedByMonitor: false, hostLogicalProcessors: 2,
  maxPrivateBytes: 1024 * 1024 * 1024, maxWorkingSetBytes: 2 * 1024 * 1024 * 1024,
  minHostFreeBytes: 512 * 1024 * 1024, maxCpuCores: 1.5, maxIoBytesPerSecond: 1048576,
  maxIoOperationsPerSecond: 1000, sustainedSamples: 3,
  environment: { cpuLimit: 'Fixture; no hard CPU quota', memoryLimit: 'Fixture; no hard memory quota',
    storage: 'Fixture memory data', database: 'File fixture', network: 'loopback fixture', tls: 'disabled fixture', failureDomain: 'single fixture host' } });
const identity = () => ({ pid: 12345, executablePath: 'C:\\fixture\\mir2-gateway.exe', hostLogicalProcessors: 2 });
const raw = (sampledAt = 100000) => ({ pid: 12345, executablePath: 'C:\\fixture\\mir2-gateway.exe', creationTimeMs: 1000,
  sampledAt, cpuTimeMs: 1000, workingSetBytes: 2000000, privateBytes: 1000000,
  readBytes: 500, writeBytes: 600, otherIoBytes: 700, readOperations: 10, writeOperations: 20, otherIoOperations: 30,
  hostFreeBytes: 2 * 1024 * 1024 * 1024, hostTotalBytes: 4 * 1024 * 1024 * 1024, handles: 100, threads: 4,
  cores: [{ id: '0', busyPercent: 5 }, { id: '1', busyPercent: 10 }],
  adapters: [{ name: 'fixture Ethernet', receivedBytes: 10000, sentBytes: 20000 }] });
const health = () => ({ ok: true, http: 'ready', ws: 'ready', session_cache: { healthy: true },
  capacity: { currentWsConnections: 2, currentActiveSessions: 2, currentReconnectLeases: 0,
    currentLoginInFlight: 0, currentNewCharacterInFlight: 0, currentStartGameInFlight: 0,
    maxWsConnections: 110, maxActiveSessions: 100, maxReconnectLeases: 10,
    maxLoginInFlight: 8, maxNewCharacterInFlight: 4, maxStartGameInFlight: 4 } });

test('profile requires explicit measured resource envelope and distinguishes it from enforced quotas', () => {
  assert.deepEqual(validateResourceProfile(profile()), profile());
  for (const key of ['maxPrivateBytes', 'maxWorkingSetBytes', 'maxCpuCores', 'maxIoBytesPerSecond', 'maxIoOperationsPerSecond']) {
    const value = profile(); delete value[key]; assert.throws(() => validateResourceProfile(value));
  }
  const inventedQuota = profile(); inventedQuota.resourceLimitsEnforcedByMonitor = true;
  assert.throws(() => validateResourceProfile(inventedQuota));
  const missingScope = profile(); delete missingScope.environment.database;
  assert.throws(() => validateResourceProfile(missingScope));
});

test('monitor endpoint excludes remote hosts, credentials, redirection inputs and non-health paths', () => {
  assert.equal(validateHealthUrl('http://127.0.0.1:17510/health'), 'http://127.0.0.1:17510/health');
  assert.equal(validateHealthUrl('http://[::1]:17510/health'), 'http://[::1]:17510/health');
  for (const value of ['http://example.com:17510/health', 'http://localhost:17510/health',
    'http://127.0.0.1:17510/health?token=secret', 'http://user:secret@127.0.0.1:17510/health', 'http://127.0.0.1:17510/admin']) {
    assert.throws(() => validateHealthUrl(value));
  }
});

test('health projection emits only public finite capacity fields and fails closed on missing counters', () => {
  const body = health(); body.unexpected = { password: 'must not copy' };
  const projected = publicCapacityHealth(body); assert.equal(projected.currentActiveSessions, 2);
  assert.equal(projected.unexpected, undefined); assert.equal(JSON.stringify(projected).includes('password'), false);
  for (const mutate of [value => { value.ok = false; }, value => { value.session_cache.healthy = false; },
    value => { delete value.capacity.currentNewCharacterInFlight; }, value => { value.capacity.maxActiveSessions = null; },
    value => { value.capacity.currentActiveSessions = 101; }]) {
    const changed = health(); mutate(changed); assert.throws(() => publicCapacityHealth(changed));
  }
});

test('resource sample binds PID, executable, full core inventory and monotonic cumulative counters', () => {
  assert.equal(validateProcessSample(raw(), identity()).pid, 12345);
  for (const mutate of [value => { value.pid++; }, value => { value.executablePath = 'C:\\other.exe'; },
    value => { value.cores.pop(); }, value => { value.privateBytes = null; }, value => { value.cores[1].id = '0'; }]) {
    const changed = raw(); mutate(changed); assert.throws(() => validateProcessSample(changed, identity()));
  }
  const envelope = new ResourceEnvelope(profile(), identity()); envelope.sample(raw(), health(), 100000);
  const changed = raw(101000); changed.cpuTimeMs = 999;
  assert.throws(() => envelope.sample(changed, health(), 101000), /backwards/);
});

test('real cumulative counter deltas produce cores and I/O rates without inventing process network bytes', () => {
  const envelope = new ResourceEnvelope(profile(), identity()); envelope.sample(raw(), health(), 100000);
  const changed = raw(102000); changed.cpuTimeMs += 1000; changed.readBytes += 200; changed.writeBytes += 400;
  changed.readOperations += 4; changed.adapters[0].receivedBytes += 1000;
  const sample = envelope.sample(changed, health(), 102000);
  assert.equal(sample.process.rates.cpuCores, .5); assert.equal(sample.process.rates.readBytesPerSecond, 100);
  assert.equal(sample.process.rates.writeBytesPerSecond, 200); assert.equal(sample.process.rates.ioOperationsPerSecond, 2);
  assert.equal(sample.network.hostBytesPerSecond, 500); assert.equal(sample.network.processBytesPerSecond, null);
  assert.equal(sample.memory.currentBytes, changed.privateBytes); assert.equal(sample.stop, false);
  assert.equal(inspectMonitor(sample, 102000).stop, false);
});

test('private-memory and host safety reserve violations stop immediately and cannot recover to pass', () => {
  for (const mutate of [value => { value.privateBytes = profile().maxPrivateBytes + 1; },
    value => { value.workingSetBytes = profile().maxWorkingSetBytes + 1; }, value => { value.hostFreeBytes = 0; }]) {
    const envelope = new ResourceEnvelope(profile(), identity()), changed = raw(); mutate(changed);
    const stopped = envelope.sample(changed, health(), 100000); assert.equal(stopped.stop, true);
    const later = envelope.sample(raw(101000), health(), 101000); assert.equal(later.stop, true);
    assert.equal(later.reason, stopped.reason); assert.throws(() => inspectMonitor(later, 101000));
  }
});

test('transient CPU burst resets but sustained CPU oversubscription requests drain at the original threshold', () => {
  const envelope = new ResourceEnvelope(profile(), identity()); envelope.sample(raw(), health(), 100000);
  const value = raw(); let cpu = value.cpuTimeMs;
  for (let index = 1; index <= 6; index++) {
    value.sampledAt = 100000 + index * 1000; cpu += index === 3 ? 0 : 2000; value.cpuTimeMs = cpu;
    const sample = envelope.sample(value, health(), value.sampledAt);
    assert.equal(sample.stop, index === 6);
  }
});

test('missing process identity, PID reuse, stale samples and NIC counter reset invalidate continuity', () => {
  for (const mutate of [value => { value.creationTimeMs++; }, value => { value.sampledAt = 99999; },
    value => { value.adapters[0].receivedBytes = 0; }]) {
    const envelope = new ResourceEnvelope(profile(), identity()); envelope.sample(raw(), health(), 100000);
    const next = raw(101000); mutate(next); assert.throws(() => envelope.sample(next, health(), 101000));
  }
  const envelope = new ResourceEnvelope(profile(), identity());
  assert.throws(() => envelope.sample(raw(100000), health(), 120001), /stale/);
});

test('NIC removal, addition, rename and duplicate identity cannot silently reduce host bandwidth rate', () => {
  for (const mutate of [value => { value.adapters = []; }, value => { value.adapters[0].name = 'renamed'; },
    value => { value.adapters.push({ name: 'new NIC', receivedBytes: 0, sentBytes: 0 }); },
    value => { value.adapters.push({ ...value.adapters[0] }); }]) {
    const envelope = new ResourceEnvelope(profile(), identity()); envelope.sample(raw(), health(), 100000);
    const next = raw(101000); mutate(next); assert.throws(() => envelope.sample(next, health(), 101000));
  }
});

test('health endpoint provenance requires the pinned PID to own that exact loopback listener', () => {
  const pinned = { ...identity(), healthPort: 17510, healthHost: '127.0.0.1' };
  for (const listeners of [[], [{ address: '127.0.0.1', port: 17510, pid: 999 }],
    [{ address: '127.0.0.1', port: 17511, pid: 12345 }], [{ address: '0.0.0.0', port: 17510, pid: 12345 }]]) {
    assert.throws(() => validateProcessSample({ ...raw(), listeners }, pinned), /listener/);
  }
  assert.equal(validateProcessSample({ ...raw(), listeners: [{ address: '127.0.0.1', port: 17510, pid: 12345 }] }, pinned).pid, 12345);
});

test('CLI rejects duplicate arguments and credentials have no command-line option', () => {
  assert.deepEqual(parseArguments(['--gateway-pid', '123', '--wall-seconds', '100']), { gatewayPid: '123', wallSeconds: '100' });
  assert.throws(() => parseArguments(['--gateway-pid', '123', '--gateway-pid', '123']));
  assert.throws(() => parseArguments(['--password', 'secret']));
});

test('Windows CIM producer samples the owned test process and every host logical core', {
  skip: process.platform !== 'win32', timeout: 30000,
}, async () => {
  const sampler = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', WINDOWS_SAMPLER], {
    windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'], env: windowsSamplerEnvironment(process.pid, 500),
  });
  const lines = createInterface({ input: sampler.stdout, crlfDelay: Infinity });
  const watchdog = setTimeout(() => { sampler.kill(); lines.close(); }, 25000);
  try {
    let first;
    for await (const line of lines) { first = JSON.parse(line); break; }
    assert.ok(first, 'Real OS producer returned a sample');
    const measuredIdentity = { pid: process.pid, executablePath: await fs.realpath(process.execPath), hostLogicalProcessors: os.cpus().length };
    validateProcessSample(first, measuredIdentity);
    assert.equal(first.cores.length, os.cpus().length);
    assert.ok(first.privateBytes > 0); assert.ok(first.workingSetBytes > 0); assert.ok(first.hostTotalBytes > first.hostFreeBytes);
    // This is producer validation of a test process, never Gateway capacity.
  } finally { clearTimeout(watchdog); sampler.kill(); lines.close(); }
});
