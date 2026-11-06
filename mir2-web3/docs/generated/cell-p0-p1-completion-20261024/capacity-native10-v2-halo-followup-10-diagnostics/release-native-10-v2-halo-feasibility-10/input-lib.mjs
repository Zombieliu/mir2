import {EXPECTED_DRIVER_SOURCE_COMMIT,EXPECTED_DRIVER_SOURCE_SHA256} from './driver-report-contract.mjs';
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
export const releaseFile = path.join(base, 'release-socket-diagnostics-v1.json');
export const driverFrozenFile = path.join(base, 'capacity-native-survival-pursuit-witness-driver-frozen-v1.json');
export const driverBaselineRoot = path.join(base, 'release-native-10-feasibility-04');
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
  const backendSourceCommit = '46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b',driverSourceCommit=EXPECTED_DRIVER_SOURCE_COMMIT;
  const candidateFile=path.join(root,'candidate-release-binding.json'),candidate=await read(candidateFile);
  const finalTemplate=await read(path.join(root,'launcher-source-frozen-v1.json'));
  if(finalTemplate.state!=='final-source-bound-not-executed'||finalTemplate.backendSourceCommit!==backendSourceCommit||finalTemplate.driverSourceCommit!==driverSourceCommit||finalTemplate.driverSourceSha256!==EXPECTED_DRIVER_SOURCE_SHA256)throw Error('Final41-input launcher source binding is not ready');
  const {OLD_BINARY_SHA,IMMUTABLE_BINARY,RECEIPT,BUILD_LOG}=await import('./socket-build-source-lib.mjs');
  const {verifyImmutableBackendWithCurrentDriver}=await import('./backend-driver-source-guard.mjs');
  const frozen=await verifyImmutableBackendWithCurrentDriver({repositoryDirectory:repository,driverSourceCommit});
  const releaseBytes=await fs.readFile(releaseFile),release=JSON.parse(releaseBytes),manifest=frozen.manifest,manifestBytes=frozen.manifestBytes;
  if(releaseBytes.length!==1920||sha(releaseBytes)!=='cdcf7ec17ffbdb7ef1a8c529deaf8d8dd957d16a8129315390760e910a4b9c3f'||manifestBytes.length!==14000||sha(manifestBytes)!=='4da9d4dac73773bae041c98b641874a9ad4427217c1b36bb453f1a014ad7b058'||
    manifest.sourceManifestSha256!=='57ea1cde0ab22a03a50d1e9c7e20fdcd8a2060d1cb7922a2ad2cee91e55f5d96'||release.schemaVersion!==1||release.profile!=='release'||release.sourceCommit!==backendSourceCommit||release.baseCommit!==manifest.gitBase||release.backendInputCount!==41||manifest.files.length!==41||
    !samePath(release.backendSourceManifest,path.join(base,'backend-source-socket-diagnostics-v1.json'))||release.sourceManifestFileSha256!==sha(manifestBytes)||release.sourceManifestSha256!==manifest.sourceManifestSha256||
    release.sourceVerifiedBeforeAndAfterBuild!==true||release.ignoredInclusiveUntrackedBackendCount!==0||!samePath(release.binary?.path??'',IMMUTABLE_BINARY)||release.binary.bytes!==87370240||release.binary.sha256!=='a43078b05e70b480f948fee505e23d2757ce1ae20fdc1f22b2348df46ee7125d'||release.binary.sha256===OLD_BINARY_SHA)throw Error('Fresh actual41-input Release/source identity differs');
  await assertPin({file:release.binary.path,...release.binary});
  if(!samePath(release.buildReceipt?.path??'',RECEIPT)||!samePath(release.buildLog?.path??'',BUILD_LOG))throw Error('Actual new build receipt/log paths differ');
  await assertPin({file:RECEIPT,bytes:2095,sha256:'36181ed0755c44bafd676f44bda8a33c72f958777660fb649686f0119478eabd'});
  await assertPin({file:BUILD_LOG,bytes:55560,sha256:'b03df3d3eee471d5dd2070fbd9a59dd2057249e2c3114347e79c7dfac4f56eac'});
  const receipt=await read(RECEIPT),log=await fs.readFile(BUILD_LOG,'utf8');
  if(receipt.success!==true||receipt.exit?.code!==0||receipt.exit?.signal||receipt.sourceUnchangedBeforeAfter!==true||receipt.backendInputCount!==41||receipt.sourceCommit!==backendSourceCommit||receipt.sourceManifestSha256!==release.sourceManifestSha256||receipt.sourceManifestFileSha256!==release.sourceManifestFileSha256||
    receipt.immutableBinary?.sha256!==release.binary.sha256||receipt.immutableBinary?.bytes!==release.binary.bytes||!(/Finished `release` profile/.test(log))||/^error(?:\[|:)/m.test(log))throw Error('Actual successful socket diagnostics build evidence differs');

  for(const [name,bytes,sha256] of [
    ['slow-stage-public.mjs',10186,'c2a137084f2166cbae603f721c6dde7322790c8dc2e05c2b8d59b79d7a58619f'],
    ['slow-stage-public.test.mjs',20681,'3315da0fe78985e95fc5b68f0f61689bee4bfc7c7108ebe6de8d8fcb5a1e0309'],
    ['synthetic-tests-v1.log',6131,'f9a3757f968410a1e60ce48abcc9296c5a4014407e418608974034db1c67449b'],
    ['slow-stage-helper-freeze-v1.json',4235,'875ecbb07a386d339fc2e5f78f62dd9804dde7a5ad93a6114c01c34a3fd2b29d'],
    ['slow-stage-schema-contract.json',7923,'a566d2357976ce4ec0a14c92da2e8f1490e724c527dcb757be46e3c6642ea6f6']
  ])await assertPin({file:path.join(root,name),bytes,sha256});
  const {verifyFull16Gate}=await import('./verify-full16-gate.mjs');
  const full16Gate=await verifyFull16Gate(candidate,release,manifest);
  return{release,manifest,backendSourceCommit,driverSourceCommit,full16Gate,backendSourceUnchangedSinceBuild:true,rebuildPerformed:false,
    releasePin:{file:releaseFile,bytes:releaseBytes.length,sha256:sha(releaseBytes)},manifestPin:{file:release.backendSourceManifest,bytes:manifestBytes.length,sha256:sha(manifestBytes)}};
}

export async function collectDriverInputs() {
  const driverFreezeBytes = await fs.readFile(driverFrozenFile);
  if (driverFreezeBytes.length !== 28714 || sha(driverFreezeBytes) !== '4bac2cca1a8cb6eca3432ebc22add873456ce38a7f33b81f39998d88befe3063') throw Error('Exact reviewed patrol driver freeze manifest raw bytes changed');
  const frozen = await read(driverFrozenFile);
  const baseline = await read(path.join(base, 'capacity-attack-v2-driver-frozen-v1.json'));
  if (baseline.files?.length !== 9 || sha(JSON.stringify(baseline.files)) !== baseline.sourceSha256) throw new Error('Original V2 baseline manifest differs');
  if (frozen.schemaVersion !== 1 || frozen.files?.length !== 9 || !hex(frozen.sourceSha256) || sha(JSON.stringify(frozen.files)) !== frozen.sourceSha256 ||
      JSON.stringify(frozen.files.map(entry => entry.file).sort()) !== JSON.stringify(baseline.files.map(entry => entry.file).sort())) throw new Error('Final patrol-preference nine-input driver freeze missing');
  const driverSourceCommit = git(['rev-parse', 'HEAD']).toString('utf8').trim();
  if(driverSourceCommit!==EXPECTED_DRIVER_SOURCE_COMMIT)throw Error('Current committed timing-driver identity differs');
  for (const entry of frozen.files) {
    if (!safeRelative(entry.file) || !entry.file.startsWith('apps/web/') || !hex(entry.sha256) || !Number.isSafeInteger(entry.bytes)) throw new Error('Invalid patrol driver raw pin');
    const file = path.join(project, entry.file); await assertPin({ ...entry, file });
    const bytes = await fs.readFile(file), committed = git(['show', driverSourceCommit + ':mir2-web3/' + entry.file]);
    if (sha(Buffer.from(bytes.toString('utf8').replace(/\r\n/g, '\n'))) !== sha(committed)) throw new Error('Frozen driver bytes are not in the current committed driver tree');
  }
  const {verifyDriverEvidenceBasis}=await import('./driver-evidence-contract.mjs');
  const evidence=await verifyDriverEvidenceBasis(frozen,assertPin);
  const prior = await read(path.join(driverBaselineRoot, 'source-pins-checkpoint-v2.json'));
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
    for (const match of source.matchAll(/new URL\(['"]([^'"]+)['"],\s*import.meta.url\)/g)) {
      if (!/\.(?:json|gz)$/.test(match[1])) continue;
      const dataFile = path.resolve(path.dirname(file), match[1]);
      if (!dataFile.startsWith(path.resolve(project) + path.sep)) throw new Error('Driver literal data resolved outside the frozen project');
      files.add(dataFile);
    }
  }
  const require = createRequire(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-multiplayer-smoke.mjs')));
  const wsEntry = require.resolve('next/dist/compiled/ws');
  const wsRoot = path.dirname(wsEntry), wsNames = await fs.readdir(wsRoot);
  if (wsNames.length > 16) throw new Error('Bundled WS directory unexpectedly expanded');
  for (const name of wsNames) { const file = path.join(wsRoot, name); if (!(await fs.stat(file)).isFile()) throw new Error('Bundled WS has an unpinned subdirectory'); files.add(file); }
  files.add(require.resolve('next/package.json'));
  files.add(path.join(project, 'apps/web/package.json'));
  const pins = await Promise.all([...files].sort().map(pin));
  return { frozen, pins:[...pins,...evidence.pins], driverEvidenceBasis:evidence, driverSourceCommit, importModuleCount: seen.size, bundledWsEntry: wsEntry };
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
