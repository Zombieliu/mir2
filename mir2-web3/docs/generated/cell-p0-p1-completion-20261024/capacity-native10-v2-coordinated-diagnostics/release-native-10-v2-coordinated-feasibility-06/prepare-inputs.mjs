import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { root, base, project, priorRoot, endpoint, driverFrozenFile, read, pin, writeNew, sha, samePath, verifyRelease, collectDriverInputs } from './input-lib.mjs';

if (process.argv.slice(2).join(' ') !== '--prepare-authorized-inputs') throw new Error('Require the explicit preparation argument; this script never starts a realm');
if (!samePath(await fs.realpath(root), root)) throw new Error('Refusing a redirected probe root');
const releaseBinding = await verifyRelease(), drivers = await collectDriverInputs();
if (drivers.driverSourceCommit !== releaseBinding.driverSourceCommit) throw new Error('Driver source commit changed during preparation');
const originalScenario = await fs.readFile(path.join(priorRoot, 'scenario.json'));
if (sha(originalScenario) !== '701a2a3e30c1b963653af1a767f5af290e15fef207a7359cc90492cf5e496921') throw new Error('Original04 scene bytes changed');
const scenario = JSON.parse(originalScenario);
if (JSON.stringify(scenario.anchors[4]) !== JSON.stringify({ x: 246, y: 646 }) || scenario.patrolSpan !== 2 || JSON.stringify(scenario.monsterNames) !== '["Deer","Scarecrow"]') throw new Error('Original04 scene identity differs');
await fs.writeFile(path.join(root, 'preparation.lock'), JSON.stringify({ preparingAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit,
  backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit }) + '\n', { flag: 'wx' });
for (const name of ['private', 'bin', 'recordings', 'reports']) {
  const target = path.join(root, name); try { await fs.access(target); throw new Error('Refusing reused probe state directory'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
}
const aclEnv = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toLowerCase() !== 'psmodulepath'));
const acl = {};
for (const name of ['private', 'recordings']) acl[name] = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(root, 'initialize-private.ps1'), '-PrivateRoot', path.join(root, name)], { encoding: 'utf8', windowsHide: true, env: aclEnv, timeout: 30000 }));
if (Object.values(acl).some(value => value.ownerOnlyVerified !== true || value.allowedPrincipalCount !== 2)) throw new Error('Fresh directory ACL verification failed');
await fs.mkdir(path.join(root, 'bin')); await fs.mkdir(path.join(root, 'reports'));
const ownExe = path.join(root, 'bin/mir2-gateway.exe');
await fs.copyFile(releaseBinding.release.binary.path, ownExe, 1);
const copied = await pin(ownExe);
if (copied.bytes !== releaseBinding.release.binary.bytes || copied.sha256 !== releaseBinding.release.binary.sha256) throw new Error('Owned executable copy differs');
await fs.writeFile(path.join(root, 'scenario.json'), originalScenario, { flag: 'wx' });
const profile = await read(path.join(priorRoot, 'resource-profile.json')); profile.profileId = 'release-native-10-v2-coordinated-feasibility-06';
await writeNew(path.join(root, 'resource-profile.json'), profile);
const options = await read(path.join(priorRoot, 'run-options.json'));
Object.assign(options, { scenarioFile: path.join(root, 'scenario.json'), attackResults: 'v2',
  scope: 'Fresh ordinary ten-player coordinated-navigation V2 feasibility probe, not formal capacity acceptance. Original04 scene bytes and all30s/360s/native18/12/50%Run/900ms/1%timeout/5%correction/resource guards retained. Current active-observer coverage, bounded packet trace and explicit typed attack results remain; only controlled ordinary navigation hints and coherent owner position references are new. This does not change server occupancy, timing, acceptance or damage proof.' });
if (options.endpoint !== endpoint || options.profile !== 'probe' || options.activityMode !== 'native' || options.stages !== '10' || options.stageSeconds !== 30 || options.soakSeconds !== 360 || options.combatActors !== 1 || options.resumeSamples !== 0 || options.saveSamples !== 0 || options.wallSeconds !== 1800) throw new Error('Original probe policy differs');
await writeNew(path.join(root, 'run-options.json'), options);
const batch = crypto.randomBytes(3).toString('hex');
const accounts = Array.from({ length: 10 }, (_, index) => { const suffix = index.toString(36).padStart(2, '0'); return { accountId: 'lt' + batch + suffix, name: 'L' + batch + suffix,
  password: crypto.randomBytes(10).toString('hex'), state: 'registered', provisioning: 'admin-provisioned-file-account-only' }; });
const poolFile = path.join(root, 'private/capacity-pool.json');
await writeNew(poolFile, { schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint, batch, accounts });
for (const name of ['recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt']) await fs.writeFile(path.join(root, 'private', name), crypto.randomBytes(32).toString('hex'), { flag: 'wx', mode: 0o600 });
const seeder = path.join(base, '../cell-preproduction-p0-p1-20261024/target/debug/seed_capacity_accounts.exe');
const seedPin = await pin(seeder);
const provisioning = JSON.parse(execFileSync(seeder, ['--manifest', poolFile, '--account-store', path.join(root, 'private/accounts.json')], { encoding: 'utf8', windowsHide: true, timeout: 90000 }));
if (provisioning.passed !== true || provisioning.created !== 10 || provisioning.charactersCreated !== 0 || provisioning.gameplayGrants !== 0 || provisioning.networkRequests !== 0 || provisioning.existingStoreModified !== false) throw new Error('Account-only provisioning contract differs');
await writeNew(path.join(root, 'provisioning.json'), provisioning);
await writeNew(path.join(root, 'gateway-build-pin.json'), { schemaVersion: 1, sourceExecutable: releaseBinding.release.binary.path, ownedExecutable: ownExe,
  sha256: copied.sha256, bytes: copied.bytes, sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, baseCommit: releaseBinding.release.baseCommit,
  sourceManifest: { ...releaseBinding.manifestPin, sourceManifestSha256: releaseBinding.manifest.sourceManifestSha256, inputs: releaseBinding.manifest.files.length },
  releaseRecord: releaseBinding.releasePin, copiedAtUtc: new Date().toISOString(), sourceInputsVerifiedBeforeLaunch: true, capacityAccepted: false, formalRunStarted: false });
const scripts = (await fs.readdir(root)).filter(name => /\.(?:mjs|ps1)$/.test(name)).sort();
const launcherFiles = await Promise.all(scripts.map(name => pin(path.join(root, name))));
await writeNew(path.join(root, 'launcher-source-frozen-v1.json'), { schemaVersion: 1, frozenAt: new Date().toISOString(), state: 'prepared-not-started', backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, driverFrozenFile, driverSourceSha256: drivers.frozen.sourceSha256, files: launcherFiles, sourceSha256: sha(JSON.stringify(launcherFiles)), scope: 'Independent06 launcher files only; original461f backend Release record is reused unchanged, no duplicate Release binding created' });
const publicFiles = [driverFrozenFile, releaseBinding.releasePin.file, releaseBinding.manifestPin.file,
  ...scripts.map(name => path.join(root, name)), ...['scenario.json', 'resource-profile.json', 'run-options.json', 'provisioning.json', 'gateway-build-pin.json', 'launcher-source-frozen-v1.json', 'launcher-source-prepared-v1.json', 'preparation-plan.json'].map(name => path.join(root, name)),
  path.join(priorRoot, 'scenario.json'), path.join(priorRoot, 'resource-profile.json'), path.join(base, 'partition-native10-actor4-home-review-v1.json'),
  path.join(base, 'release-native-10-v2-feasibility-05/gateway-build-pin.json'), path.join(base, 'capacity-attack-v2-driver-frozen-v1.json')];
const files = [...drivers.pins, ...await Promise.all(publicFiles.map(pin)), await pin(process.execPath), seedPin];
await writeNew(path.join(root, 'source-pins.json'), { schemaVersion: 1, preparedAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit,
  backendSourceManifest: releaseBinding.manifestPin.file, originalBackendReleaseReused: true, driverFrozenManifest: driverFrozenFile, driverSourceSha256: drivers.frozen.sourceSha256, sourceConfigSha256: sha(JSON.stringify(files)), files,
  localImportModuleCount: drivers.importModuleCount, nextBundledWsEntry: drivers.bundledWsEntry, acl,
  scope: 'Exact new release, V2 driver, local import closure, bundled transport, original data and launcher inputs; no credentials/store/recording content is public',
  realOrdinaryAccounts: 10, charactersCreated: 0, gameplayGrants: 0, realmStarted: false, capacityAccepted: false });
console.log(JSON.stringify({ prepared: true, root, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, backendInputs: releaseBinding.manifest.files.length,
  sha256: copied.sha256, realOrdinaryAccounts: 10, charactersCreated: 0, sourceConfigSha256: sha(JSON.stringify(files)), realmStarted: false, capacityAccepted: false }));
