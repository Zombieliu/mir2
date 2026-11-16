import {EXPECTED_DRIVER_SOURCE_COMMIT,EXPECTED_DRIVER_SOURCE_SHA256,EXPECTED_DRIVER_MANIFEST_BYTES,EXPECTED_DRIVER_MANIFEST_SHA256} from './driver-report-contract.mjs';
import {DRIVER_BINDING} from './driver-binding-contract.mjs';
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
export const releaseFile = path.join(base, 'release-account-store-wait-diagnostics-v1.json');
export const driverFrozenFile = DRIVER_BINDING.driverFrozenFile ?? path.join(root,'PENDING-NEW-DRIVER-FREEZE.json');
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
  const candidate=await read(path.join(root,'candidate-release-binding.json'));
  const reference=p=>p&&typeof p.file==='string'&&path.isAbsolute(p.file)&&Number.isSafeInteger(p.bytes)&&p.bytes>0&&hex(p.sha256);
  if(candidate.state!=='new50-release-and-full16-bound-not-executed'||candidate.backendInputCount!==50||candidate.backendSourceCommit!==DRIVER_BINDING.backendSourceCommit||
    candidate.driverSourceCommit!==EXPECTED_DRIVER_SOURCE_COMMIT||!['releaseRecord','backendSourceManifest','buildReceipt','buildLog','rootFunctionalValidation'].every(k=>reference(candidate[k]))||
    !candidate.binary||!path.isAbsolute(candidate.binary.path??'')||!Number.isSafeInteger(candidate.binary.bytes)||!hex(candidate.binary.sha256)||candidate.full16?.state!=='passed')throw Error('PENDING_FRESH50_RELEASE_AND_FULL16: actual new backend, successful build/root validation and fresh ordinary full16 are required');
  const backendSourceCommit=candidate.backendSourceCommit,driverSourceCommit=EXPECTED_DRIVER_SOURCE_COMMIT;
  const finalTemplate=await read(path.join(root,'launcher-source-frozen-v1.json'));
  if(finalTemplate.state!=='final-source-bound-not-executed'||finalTemplate.backendSourceCommit!==backendSourceCommit||finalTemplate.driverSourceCommit!==driverSourceCommit||finalTemplate.driverSourceSha256!==EXPECTED_DRIVER_SOURCE_SHA256)throw Error('Reviewed final50-input launcher source binding is not ready');
  const {verifyFreshBackendWithCurrentDriver}=await import('./backend-driver-source-guard.mjs');
  const frozen=await verifyFreshBackendWithCurrentDriver({repositoryDirectory:repository,driverSourceCommit});
  const {checks}=frozen,{IMMUTABLE_BINARY,RECEIPT,BUILD_LOG}=checks;
  const releaseBytes=await fs.readFile(releaseFile),release=JSON.parse(releaseBytes),manifest=frozen.manifest,manifestBytes=frozen.manifestBytes;
  const exact=(ref,file,bytes)=>samePath(ref.file,file)&&ref.bytes===bytes.length&&ref.sha256===sha(bytes);
  const samePin=(a,b)=>a?.bytes===b?.bytes&&a?.sha256===b?.sha256;
  if(!exact(candidate.releaseRecord,releaseFile,releaseBytes)||!exact(candidate.backendSourceManifest,checks.MANIFEST,manifestBytes)||candidate.backendSourceAggregate!==manifest.sourceManifestSha256||
    release.schemaVersion!==1||release.profile!=='release'||release.sourceCommit!==backendSourceCommit||release.driverSourceCommit!==DRIVER_BINDING.coreCodeEvidenceCommit||release.driverSourceSha256!==DRIVER_BINDING.driverSourceSha256||release.baseCommit!==manifest.gitBase||release.backendInputCount!==50||manifest.files.length!==50||
    !samePath(release.backendSourceManifest,checks.MANIFEST)||release.sourceManifestFileSha256!==sha(manifestBytes)||release.sourceManifestSha256!==manifest.sourceManifestSha256||
    release.sourceVerifiedBeforeAndAfterBuild!==true||release.ignoredInclusiveUntrackedBackendCount!==0||!samePath(release.binary?.path??'',IMMUTABLE_BINARY)||checks.OLD_BINARY_SHAS.includes(release.binary?.sha256)||
    release.binary?.sha256!==DRIVER_BINDING.backendBinarySha256||!samePath(candidate.binary.path,release.binary.path)||!samePin(candidate.binary,release.binary)||release.buildJobs!==4||JSON.stringify(release.buildArguments)!==JSON.stringify(checks.BUILD_ARGS))throw Error('Fresh actual50-input Release/source identity differs');
  await assertPin({file:release.binary.path,bytes:release.binary.bytes,sha256:release.binary.sha256});
  for(const [key,file] of [['buildReceipt',RECEIPT],['buildLog',BUILD_LOG]]){if(!samePath(candidate[key].file,file)||!samePath(release[key]?.path??'',file)||!samePin(candidate[key],release[key]))throw Error('New build evidence path or pin differs');await assertPin(candidate[key]);}
  if(!samePath(candidate.rootFunctionalValidation.file,manifest.rootFunctionalValidation.path)||!samePin(candidate.rootFunctionalValidation,manifest.rootFunctionalValidation))throw Error('Actual five-source Rust validation reference differs');await assertPin(candidate.rootFunctionalValidation);
  const receipt=await read(RECEIPT),log=await fs.readFile(BUILD_LOG,'utf8'),quote=String.fromCharCode(96),errorLines=log.split(String.fromCharCode(10)).some(line=>line.startsWith('error:')||line.startsWith('error['));
  const linker=String(receipt.visualStudio?.linkExecutable??'').replaceAll(String.fromCharCode(92),'/');
  if(receipt.kind!=='account-store-wait-diagnostics-v1-owned-cargo-build'||receipt.success!==true||receipt.exit?.code!==0||receipt.exit?.signal||receipt.error||receipt.postSourceFailure||receipt.sourceUnchangedBeforeAfter!==true||receipt.backendInputCount!==50||receipt.sourceCommit!==backendSourceCommit||receipt.sourceManifestSha256!==release.sourceManifestSha256||receipt.sourceManifestFileSha256!==release.sourceManifestFileSha256||
    !samePin(receipt.immutableBinary,release.binary)||!samePin(receipt.buildLog,release.buildLog)||receipt.buildJobs!==4||JSON.stringify(receipt.arguments)!==JSON.stringify(checks.BUILD_ARGS)||
    !receipt.toolchain?.rustc?.startsWith('rustc 1.95.0 ')||!receipt.toolchain?.cargo?.startsWith('cargo 1.95.0 ')||!['x64','amd64'].includes(receipt.visualStudio?.targetArch)||!linker.includes('/VC/Tools/MSVC/')||!log.includes('Finished '+quote+'release'+quote+' profile')||errorLines)throw Error('Actual successful fresh projection build evidence differs');
  for(const [name,bytes,sha256] of [["slow-stage-public.mjs",10318,"8033874548d93323ef6390b07509887e727b5920850d0c3a1c2a0f810c56677c"],["slow-stage-public.test.mjs",22894,"a0f10c9d721404378e8b5bc56070425393fdf82af21588bbf70de2c274af76ae"],["synthetic-tests-v1.log",7579,"451c97bfe087895450890a37e42f21e9f2305da127a73f9ce9857b8955bf9d37"],["slow-stage-helper-freeze-v1.json",2314,"f554d0e280318f3a3624046e3344d8c3becdc7057ca261b28df99c419c283a00"],["slow-stage-schema-contract.json",3573,"b8a66eaeb6dd6518b9d242c6d9d794ff1f0252d75f3c017eb2ef62d01728b512"]])await assertPin({file:path.join(root,name),bytes,sha256});
  const {verifyFull16Gate}=await import('./verify-full16-gate.mjs');
  const full16Gate=await verifyFull16Gate(candidate,release,manifest);
  return{release,manifest,backendSourceCommit,driverSourceCommit,full16Gate,backendSourceUnchangedSinceBuild:true,rebuildPerformed:true,
    releasePin:{file:releaseFile,bytes:releaseBytes.length,sha256:sha(releaseBytes)},manifestPin:{file:release.backendSourceManifest,bytes:manifestBytes.length,sha256:sha(manifestBytes)}};
}

export async function collectDriverInputs() {
  const driverFreezeBytes = await fs.readFile(driverFrozenFile);
  if (driverFreezeBytes.length !== EXPECTED_DRIVER_MANIFEST_BYTES || sha(driverFreezeBytes) !== EXPECTED_DRIVER_MANIFEST_SHA256) throw Error('Exact reviewed patrol driver freeze manifest raw bytes changed');
  const frozen = await read(driverFrozenFile);
  const baseline = await read(path.join(base, 'capacity-attack-v2-driver-frozen-v1.json'));
  if (baseline.files?.length !== 9 || sha(JSON.stringify(baseline.files)) !== baseline.sourceSha256) throw new Error('Original V2 baseline manifest differs');
  if (frozen.schemaVersion !== 1 || frozen.files?.length !== 9 || !hex(frozen.sourceSha256) || sha(JSON.stringify(frozen.files)) !== frozen.sourceSha256 ||
      JSON.stringify(frozen.files.map(entry => entry.file).sort()) !== JSON.stringify(baseline.files.map(entry => entry.file).sort())) throw new Error('Final patrol-preference nine-input driver freeze missing');
  const currentBackendHead = git(['rev-parse', 'HEAD']).toString('utf8').trim();
  if(currentBackendHead!==DRIVER_BINDING.backendSourceCommit)throw Error('Current committed backend/execution identity differs');
  for (const entry of frozen.files) {
    if (!safeRelative(entry.file) || !entry.file.startsWith('apps/web/') || !hex(entry.sha256) || !Number.isSafeInteger(entry.bytes)) throw new Error('Invalid patrol driver raw pin');
    const file = path.join(project, entry.file); await assertPin({ ...entry, file });
    const bytes = await fs.readFile(file), committed = git(['show', currentBackendHead + ':mir2-web3/' + entry.file]);
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
  return { frozen, pins:[...pins,...evidence.pins], driverEvidenceBasis:evidence, driverSourceCommit:EXPECTED_DRIVER_SOURCE_COMMIT, importModuleCount: seen.size, bundledWsEntry: wsEntry };
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
