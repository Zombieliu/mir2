import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { root, base, project, priorRoot, endpoint, driverFrozenFile, read, pin, writeNew, sha, assertPin, samePath, verifyRelease, collectDriverInputs } from './input-lib.mjs';

if (process.argv.slice(2).join(' ') !== '--prepare-authorized-inputs') throw new Error('Require the explicit preparation argument; no realm is started by this script');
if (!samePath(await fs.realpath(root), root)) throw new Error('Refusing redirected formal root');
const launcherFreezeFile = path.join(root, 'launcher-source-frozen-v1.json'), launcherFreeze = await read(launcherFreezeFile);
if (!Array.isArray(launcherFreeze.files) || sha(JSON.stringify(launcherFreeze.files)) !== launcherFreeze.sourceSha256) throw new Error('Reviewed formal launcher freeze is inconsistent');
for (const entry of launcherFreeze.files) await assertPin(entry);
const releaseBinding = await verifyRelease(), drivers = await collectDriverInputs();
const { verifyDriverReportContract, EXPECTED_NATIVE_PATROL_PLANNING, EXPECTED_NAVIGATION_COORDINATION } = await import('./driver-report-contract.mjs');
await verifyDriverReportContract({ navigationCoordination: EXPECTED_NAVIGATION_COORDINATION, nativePatrolPlanning: EXPECTED_NATIVE_PATROL_PLANNING });
if (drivers.driverSourceCommit !== releaseBinding.driverSourceCommit) throw new Error('Driver commit changed during preparation');
const candidateFile = path.join(priorRoot, 'scenario.json');
const candidateBytes = await fs.readFile(candidateFile), scenarioBytes = await fs.readFile(path.join(root, 'scenario.json'));
const candidate = JSON.parse(candidateBytes);
if (!candidateBytes.equals(scenarioBytes) || sha(candidateBytes) !== '701a2a3e30c1b963653af1a767f5af290e15fef207a7359cc90492cf5e496921' ||
    candidate.anchors.length !== 10 || candidate.mapFileName !== '0' || candidate.patrolSpan !== 2 || JSON.stringify(candidate.monsterNames) !== '["Deer","Scarecrow"]' ||
    JSON.stringify(candidate.combatClusters.flatMap(c=>c.combatActorIndices)) !== '[5]' || candidate.combatClusters.flatMap(c=>c.movementActorIndices).length !== 9) throw Error('Original04/06 ten-home scene or role bindings differ');
const profileBytes = await fs.readFile(path.join(root, 'resource-profile.json'));
if (!profileBytes.equals(await fs.readFile(path.join(base,'release-native-10-v2-coordinated-feasibility-06/resource-profile.json')))) throw Error('Original ten-player resource guards changed');
const profile=JSON.parse(profileBytes);
if (profile.maxCpuCores !== 8 || profile.hostLogicalProcessors !== 32 || profile.maxPrivateBytes !== 4294967296 || profile.maxWorkingSetBytes !== 8589934592 || profile.minHostFreeBytes !== 8589934592 ||
    profile.maxIoBytesPerSecond !== 536870912 || profile.maxIoOperationsPerSecond !== 100000 || profile.sustainedSamples !== 3 || profile.resourceLimitsEnforcedByMonitor !== false) throw Error('Original probe soft resource guards differ');
const options = await read(path.join(root, 'run-options.json'));
if (!samePath(options.scenarioFile ?? '', path.join(root, 'scenario.json')) || !samePath(options.resourceProfile ?? '', path.join(root, 'resource-profile.json')) ||
    options.endpoint !== endpoint || options.profile !== 'probe' || options.activityMode !== 'native' || options.stages !== '2,10' || options.stageSeconds !== 300 || options.soakSeconds !== 360 ||
    options.combatActors !== 1 || options.resumeSamples !== 0 || options.saveSamples !== 0 || options.wallSeconds !== 3600 || options.movementIntervalMs !== 750 || options.attackResults !== 'v2') throw new Error('Formal frozen workload differs');
const { capacityPolicy, LOGIN_WINDOW_MS } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-soak-core.mjs')));
if (JSON.stringify(capacityPolicy(options)) !== JSON.stringify(options.policy) || LOGIN_WINDOW_MS !== 901000) throw new Error('Formal policy or ordinary login window differs');
await fs.writeFile(path.join(root, 'preparation.lock'), JSON.stringify({ preparingAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit }) + '\n', { flag: 'wx' });
for (const name of ['private', 'bin', 'recordings', 'reports']) {
  try { await fs.access(path.join(root, name)); throw new Error('Refusing reused formal state directory'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
}
const aclEnv = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toLowerCase() !== 'psmodulepath'));
const acl = {};
for (const name of ['private', 'recordings']) acl[name] = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', path.join(root, 'initialize-private.ps1'), '-PrivateRoot', path.join(root, name)], { encoding: 'utf8', windowsHide: true, env: aclEnv, timeout: 30000 }));
if (Object.values(acl).some(value => value.ownerOnlyVerified !== true || value.allowedPrincipalCount !== 2)) throw new Error('Fresh owner-only ACL verification failed');
await fs.mkdir(path.join(root, 'bin')); await fs.mkdir(path.join(root, 'reports'));
const ownExe = path.join(root, 'bin/mir2-gateway.exe'); await fs.copyFile(releaseBinding.release.binary.path, ownExe, 1);
const copied = await pin(ownExe);
if (copied.bytes !== releaseBinding.release.binary.bytes || copied.sha256 !== releaseBinding.release.binary.sha256) throw new Error('Owned release copy differs');
const batch = crypto.randomBytes(3).toString('hex');
const accounts = Array.from({ length: 10 }, (_, index) => { const suffix = index.toString(36).padStart(2, '0'); return { accountId: 'lt' + batch + suffix, name: 'L' + batch + suffix,
  password: crypto.randomBytes(10).toString('hex'), state: 'registered', provisioning: 'admin-provisioned-file-account-only' }; });
const poolFile = path.join(root, 'private/capacity-pool.json');
await writeNew(poolFile, { schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint, batch, accounts });
for (const name of ['recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt']) await fs.writeFile(path.join(root, 'private', name), crypto.randomBytes(32).toString('hex'), { flag: 'wx', mode: 0o600 });
const seeder = path.join(base, '../cell-preproduction-p0-p1-20261024/target/debug/seed_capacity_accounts.exe'), seedPin = await pin(seeder);
const provisioning = JSON.parse(execFileSync(seeder, ['--manifest', poolFile, '--account-store', path.join(root, 'private/accounts.json')], { encoding: 'utf8', windowsHide: true, timeout: 90000 }));
if (provisioning.passed !== true || provisioning.created !== 10 || provisioning.charactersCreated !== 0 || provisioning.gameplayGrants !== 0 || provisioning.networkRequests !== 0 || provisioning.existingStoreModified !== false) throw new Error('Account-only10 provisioning contract differs');
await writeNew(path.join(root, 'provisioning.json'), provisioning);
await writeNew(path.join(root,'full16-gate.json'),releaseBinding.full16Gate);
await writeNew(path.join(root, 'gateway-build-pin.json'), { schemaVersion: 1, sourceExecutable: releaseBinding.release.binary.path, ownedExecutable: ownExe,
  sha256: copied.sha256, bytes: copied.bytes, sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, baseCommit: releaseBinding.release.baseCommit,
  sourceManifest: { ...releaseBinding.manifestPin, sourceManifestSha256: releaseBinding.manifest.sourceManifestSha256, inputs: releaseBinding.manifest.files.length }, releaseRecord: releaseBinding.releasePin,
  copiedAtUtc: new Date().toISOString(), sourceInputsVerifiedBeforeLaunch: true, capacityAccepted: false, formalRunStarted: false });
const scripts = (await fs.readdir(root)).filter(name => /\.(?:mjs|ps1)$/.test(name)).sort();
const publicFiles = [driverFrozenFile, launcherFreezeFile, path.join(root, 'launcher-source-prepared-v1.json'), path.join(root, 'preparation-plan.json'), path.join(root, 'slow-stage-diagnostics-config.json'), path.join(root, 'static-preparation-verification.json'), path.join(root, 'source-guard-readonly-verification.json'),
  path.join(root,'synthetic-tests-v1.log'),path.join(root,'slow-stage-helper-freeze-v1.json'),path.join(root,'slow-stage-schema-contract.json'),path.join(root,'legacy14-original.mjs.txt'),path.join(root,'candidate-release-binding.json'), path.join(root, 'launcher-syntax-check.log'), releaseBinding.releasePin.file, releaseBinding.manifestPin.file, path.join(base,'socket-diagnostics-v1-build-receipt.json'),
  path.join(base, 'capacity-attack-v2-driver-frozen-v1.json'),
  ...scripts.map(name => path.join(root, name)), ...['scenario.json', 'resource-profile.json', 'run-options.json', 'provisioning.json', 'gateway-build-pin.json', 'full16-gate.json'].map(name => path.join(root, name)),
  candidateFile, path.join(priorRoot, 'scenario.json'), path.join(base,'release-native-10-v2-coordinated-feasibility-06/resource-profile.json')];
const files = [...releaseBinding.full16Gate.files, ...drivers.pins, ...await Promise.all(publicFiles.map(pin)), await pin(process.execPath), seedPin];
await writeNew(path.join(root, 'source-pins.json'), { schemaVersion: 1, preparedAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit,
  backendSourceManifest: releaseBinding.manifestPin.file, socketDiagnosticsReleaseUsed: true, driverFrozenManifest: driverFrozenFile, driverSourceSha256: drivers.frozen.sourceSha256, sourceConfigSha256: sha(JSON.stringify(files)), files,
  localImportModuleCount: drivers.importModuleCount, nextBundledWsEntry: drivers.bundledWsEntry, acl, scope: 'Exact probe release/driver/import/transport/data/runner and unchanged original ten-player geometry and resource profile. No private store/credential/recording payload in public evidence.',
  ordinaryLoginBudget: { admission: 25, reservedVerification: 5, total: 30, windowMs: LOGIN_WINDOW_MS, persistentLedger: true, resetOrBypass: false },
  realOrdinaryAccounts: 10, charactersCreated: 0, gameplayGrants: 0, realmStarted: false, capacityAccepted: false });
console.log(JSON.stringify({ prepared: true, root, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, backendInputs: releaseBinding.manifest.files.length,
  sha256: copied.sha256, realOrdinaryAccounts: 10, charactersCreated: 0, sourceConfigSha256: sha(JSON.stringify(files)), realmStarted: false, capacityAccepted: false }));
