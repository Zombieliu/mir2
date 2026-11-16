import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const root = fileURLToPath(new URL('.', import.meta.url));
export const base = path.dirname(path.resolve(root));
export const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
export const repository = path.dirname(project);
export const releaseFile = path.join(base, 'release-native-attack-v2.json');
export const driverFrozenFile = path.join(base, 'capacity-attack-v2-driver-frozen-v1.json');
export const priorRoot = path.join(base, 'release-native-10-feasibility-04');
export const endpoint = 'ws://127.0.0.1:17510/ws';
export const healthUrl = 'http://127.0.0.1:17510/health';
export const currentKeys = ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight'];
export const limitKeys = ['maxWsConnections', 'maxActiveSessions', 'maxReconnectLeases', 'maxLoginInFlight', 'maxNewCharacterInFlight', 'maxStartGameInFlight'];
export const limits = [24, 10, 10, 4, 1, 2];
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export const read = async file => JSON.parse((await fs.readFile(file, 'utf8')).replace(/^\uFEFF/, ''));
export const pin = async file => { const bytes = await fs.readFile(file); return { file, bytes: bytes.length, sha256: sha(bytes) }; };
export const writeNew = (file, value) => fs.writeFile(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
export const samePath = (a, b) => path.resolve(a).toLowerCase() === path.resolve(b).toLowerCase();
export const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const safeRelative = value => typeof value === 'string' && value.length > 0 && !path.isAbsolute(value) && !value.split(/[\\/]/).some(part => part === '..' || !part);
const hex = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export const git = args => execFileSync('git.exe', args, { cwd: repository, windowsHide: true, env: { ...process.env, GIT_NO_LAZY_FETCH: '1' }, maxBuffer: 20 * 1024 * 1024 });
export async function assertPin(entry) {
  const now = await pin(entry.file);
  if (now.bytes !== entry.bytes || now.sha256 !== entry.sha256) throw new Error('Frozen public input changed: ' + entry.file);
}

export async function verifyRelease() {
  const releaseBytes = await fs.readFile(releaseFile), release = JSON.parse(releaseBytes);
  if (release.schemaVersion !== 1 || release.profile !== 'release' || !/^[a-f0-9]{40}$/.test(release.sourceCommit ?? '') ||
      !/^[a-f0-9]{40}$/.test(release.baseCommit ?? '') || !hex(release.sourceManifestSha256) || !hex(release.sourceManifestFileSha256) ||
      !path.isAbsolute(release.backendSourceManifest ?? '') || !path.isAbsolute(release.binary?.path ?? '') ||
      !hex(release.binary?.sha256) || !Number.isSafeInteger(release.binary?.bytes) || release.binary.bytes < 1) throw new Error('New release record lacks strict source/binary identity');
  const manifestBytes = await fs.readFile(release.backendSourceManifest), manifest = JSON.parse(manifestBytes);
  if (sha(manifestBytes) !== release.sourceManifestFileSha256 || manifest.sourceCommit !== release.sourceCommit ||
      manifest.gitBase !== release.baseCommit || manifest.sourceManifestSha256 !== release.sourceManifestSha256 ||
      !Array.isArray(manifest.files) || !manifest.files.length || sha(JSON.stringify(manifest.files)) !== manifest.sourceManifestSha256) throw new Error('New backend manifest/release binding differs');
  if (git(['rev-parse', 'HEAD']).toString('utf8').trim() !== release.sourceCommit || git(['status', '--porcelain', '--untracked-files=no']).toString('utf8').trim()) throw new Error('Run requires the exact clean committed release tree');
  const isSource = file => /^mir2-web3\/(?:.*\.rs|(?:.*\/)?Cargo\.(?:toml|lock)|\.cargo\/.*)$/.test(file);
  const changed = git(['diff', '--name-only', '-z', release.baseCommit, release.sourceCommit]).toString('utf8').split('\0').filter(isSource);
  const fixed = ['mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock', 'mir2-web3/apps/gateway/Cargo.toml', 'mir2-web3/apps/simulation/Cargo.toml', 'mir2-web3/.cargo/config.toml'];
  const expected = [...new Set([...changed, ...fixed])].sort();
  if (JSON.stringify(manifest.files.map(entry => entry.path)) !== JSON.stringify(expected)) throw new Error('Backend manifest must close the explicit base-to-commit Rust/Cargo input set');
  for (const entry of manifest.files) {
    if (!safeRelative(entry.path) || !entry.path.startsWith('mir2-web3/')) throw new Error('Unsafe backend source path');
    await assertPin({ file: path.join(repository, entry.path), bytes: entry.bytes, sha256: entry.sha256 });
    if (!hex(entry.gitBlobLfSha256) || sha(git(['show', `${release.sourceCommit}:${entry.path}`])) !== entry.gitBlobLfSha256) throw new Error('Backend committed blob identity differs');
  }
  const binary = await pin(release.binary.path);
  if (binary.bytes !== release.binary.bytes || binary.sha256 !== release.binary.sha256) throw new Error('New release executable does not match its record');
  return { release, manifest, releasePin: { file: releaseFile, bytes: releaseBytes.length, sha256: sha(releaseBytes) }, manifestPin: { file: release.backendSourceManifest, bytes: manifestBytes.length, sha256: sha(manifestBytes) } };
}

export async function collectDriverInputs() {
  const frozen = await read(driverFrozenFile);
  if (frozen.files?.length !== 9 || sha(JSON.stringify(frozen.files)) !== frozen.sourceSha256 || frozen.fullRegression?.passed !== 129 || frozen.finalBoundary?.finalFocused?.passed !== 4) throw new Error('Final V2 driver freeze missing');
  for (const entry of frozen.files) await assertPin({ ...entry, file: path.join(project, entry.file) });
  const prior = await read(path.join(priorRoot, 'source-pins-checkpoint-v2.json'));
  const newDriverFiles = new Set(frozen.files.map(entry => entry.file));
  const files = new Set(frozen.files.map(entry => path.join(project, entry.file)));
  for (const entry of prior.files.filter(entry => !path.isAbsolute(entry.file))) {
    if (!newDriverFiles.has(entry.file)) await assertPin({ ...entry, file: path.join(project, entry.file) });
    files.add(path.join(project, entry.file));
  }
  // Follow the actual local static import closure without evaluating game modules.
  const queue = [path.join(project, 'apps/web/scripts/playtest-capacity-soak.mjs'), path.join(project, 'apps/web/scripts/playtest-capacity-monitor.mjs')], seen = new Set();
  while (queue.length) {
    const file = queue.shift(); if (seen.has(file)) continue; seen.add(file); files.add(file);
    if (seen.size > 80 || !path.resolve(file).startsWith(path.resolve(project) + path.sep) || !file.endsWith('.mjs')) throw new Error('Driver local import closure exceeded its explicit scope');
    const source = await fs.readFile(file, 'utf8');
    const imports = [...source.matchAll(/(?:\bfrom\s*|\bimport\s*)['"]([^'"]+)['"]/g)].map(match => match[1]);
    for (const specifier of imports) if (specifier.startsWith('.')) queue.push(path.resolve(path.dirname(file), specifier));
  }
  const require = createRequire(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-multiplayer-smoke.mjs')));
  const wsEntry = require.resolve('next/dist/compiled/ws');
  const wsRoot = path.dirname(wsEntry), wsNames = await fs.readdir(wsRoot);
  if (wsNames.length > 16) throw new Error('Bundled WS directory unexpectedly expanded');
  for (const name of wsNames) { const file = path.join(wsRoot, name); if (!(await fs.stat(file)).isFile()) throw new Error('Bundled WS has an unpinned subdirectory'); files.add(file); }
  files.add(require.resolve('next/package.json'));
  files.add(path.join(project, 'apps/web/package.json'));
  const pins = await Promise.all([...files].sort().map(pin));
  return { frozen, pins, importModuleCount: seen.size, bundledWsEntry: wsEntry };
}

export function ps(script, timeout = 30000) {
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toLowerCase() !== 'psmodulepath'));
  return execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from("$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';" + script, 'utf16le').toString('base64')], { encoding: 'utf8', windowsHide: true, env, timeout }).trim();
}
export const listeners = () => JSON.parse(ps("$rows=@(Get-NetTCPConnection -State Listen -ErrorAction Stop|Where-Object {$_.LocalPort -in @(17500,17510)}|ForEach-Object {@{address=[string]$_.LocalAddress;port=[int]$_.LocalPort;pid=[int]$_.OwningProcess}});ConvertTo-Json -InputObject $rows -Compress"));
export const processIdentity = pid => {
  if (!Number.isSafeInteger(pid) || pid < 1) throw new Error('Invalid owned process ID');
  return JSON.parse(ps(`$p=Get-CimInstance Win32_Process -Filter 'ProcessId=${pid}';if($null -eq $p){'null'}else{@{pid=[int]$p.ProcessId;exe=[string]$p.ExecutablePath;created=[string]$p.CreationDate}|ConvertTo-Json -Compress}`));
};
export async function health(pid) {
  const response = await fetch(healthUrl, { signal: AbortSignal.timeout(3000), redirect: 'error' });
  if (!response.ok) throw new Error('Owned health HTTP rejected');
  const body = await response.json(), owned = listeners();
  if (body.ok !== true || body.http !== 'ready' || body.ws !== 'ready' || !body.session_cache?.healthy ||
      currentKeys.some(key => !Number.isSafeInteger(body.capacity?.[key]) || body.capacity[key] < 0) ||
      limitKeys.some((key, index) => body.capacity?.[key] !== limits[index]) || owned.length !== 2 ||
      owned.some(row => row.address !== '127.0.0.1' || row.pid !== pid) || ![17500, 17510].every(port => owned.some(row => row.port === port))) throw new Error('Owned health/capacity/listener identity differs');
  return body;
}
export function spectator(body, pid, phase, sourceCommit) {
  const source = body.spectator;
  if (source?.recordingEnabled !== true || !samePath(source.dataDir ?? '', path.join(root, 'recordings'))) throw new Error('Independent default spectator recording differs');
  const metrics = { recordingEnabled: true };
  for (const key of ['activeViewers', 'activeMaps', 'bufferedFrames', 'publishedFramesTotal', 'persistedFramesTotal', 'droppedFramesTotal', 'recordingErrorsTotal']) {
    if (!Number.isSafeInteger(source[key]) || source[key] < 0) throw new Error('Invalid public spectator counter'); metrics[key] = source[key];
  }
  return { schemaVersion: 1, phase, observedAtMs: Date.now(), gatewayPid: pid, sourceCommit, dataDir: path.join(root, 'recordings'), dataDirMatches: true, metrics,
    scope: 'Whitelisted owned public health counts only; no recording/entity payload and no capacity acceptance' };
}
