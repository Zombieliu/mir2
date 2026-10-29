import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { root, base, project, priorRoot, endpoint, driverFrozenFile, read, pin, writeNew, sha, assertPin, samePath, verifyRelease, collectDriverInputs } from './input-lib.mjs';
import { verifyFeasibilityGate } from './verify-feasibility-gate.mjs';

if (process.argv.slice(2).join(' ') !== '--prepare-authorized-inputs') throw new Error('Require the explicit preparation argument; no realm is started by this script');
if (!samePath(await fs.realpath(root), root)) throw new Error('Refusing redirected formal root');
const launcherFreezeFile = path.join(root, 'launcher-source-frozen-v1.json'), launcherFreeze = await read(launcherFreezeFile);
if (!Array.isArray(launcherFreeze.files) || sha(JSON.stringify(launcherFreeze.files)) !== launcherFreeze.sourceSha256) throw new Error('Reviewed formal launcher freeze is inconsistent');
for (const entry of launcherFreeze.files) await assertPin(entry);
const releaseBinding = await verifyRelease(), drivers = await collectDriverInputs();
if (drivers.driverSourceCommit !== releaseBinding.driverSourceCommit) throw new Error('Driver commit changed during preparation');
const feasibilityGate = await verifyFeasibilityGate(releaseBinding, drivers);
const candidateFile = path.join(base, 'partition-ordinary-two-name-safe-home-100-candidate-v3.json');
const candidateBytes = await fs.readFile(candidateFile), candidate = JSON.parse(candidateBytes), scenarioBytes = await fs.readFile(path.join(root, 'scenario.json'));
const prior = await read(path.join(priorRoot, 'scenario.json'));
if (!candidateBytes.equals(scenarioBytes) || sha(candidateBytes) !== '7d0bed4a61c5e62d61b55456d87706da135712161a42987fffbd5d452dfb3402' || candidate.anchors.length !== 100 ||
    JSON.stringify(candidate.anchors.slice(0, 10)) !== JSON.stringify(prior.anchors) || candidate.mapFileName !== prior.mapFileName || candidate.patrolSpan !== prior.patrolSpan ||
    JSON.stringify(candidate.monsterNames) !== JSON.stringify(prior.monsterNames)) throw new Error('Formal original v3/first10 geometry differs');
const combat = candidate.combatClusters.flatMap(cluster => cluster.combatActorIndices), movement = candidate.combatClusters.flatMap(cluster => cluster.movementActorIndices);
if (combat.length !== 10 || movement.length !== 90 || JSON.stringify(combat.filter(index => index < 10)) !== JSON.stringify(prior.combatClusters.flatMap(cluster => cluster.combatActorIndices)) ||
    JSON.stringify(movement.filter(index => index < 10)) !== JSON.stringify(prior.combatClusters.flatMap(cluster => cluster.movementActorIndices))) throw new Error('Formal fighter/observer bindings differ');
const profileBytes = await fs.readFile(path.join(root, 'resource-profile.json'));
if (!profileBytes.equals(await fs.readFile(path.join(base, 'formal-baseline-100/resource-profile-local-host.json')))) throw new Error('Original formal local-host guards changed');
const profile = JSON.parse(profileBytes);
if (profile.maxCpuCores !== 24 || profile.hostLogicalProcessors !== 32 || profile.maxPrivateBytes !== 34359738368 || profile.maxWorkingSetBytes !== 42949672960 || profile.minHostFreeBytes !== 8589934592 ||
    profile.maxIoBytesPerSecond !== 536870912 || profile.maxIoOperationsPerSecond !== 100000 || profile.sustainedSamples !== 3 || profile.resourceLimitsEnforcedByMonitor !== false) throw new Error('Formal guard contract differs');
const options = await read(path.join(root, 'run-options.json'));
if (!samePath(options.scenarioFile ?? '', path.join(root, 'scenario.json')) || !samePath(options.resourceProfile ?? '', path.join(root, 'resource-profile.json')) ||
    options.endpoint !== endpoint || options.profile !== 'standard' || options.activityMode !== 'native' || options.stages !== '2,10,25,50,100' || options.stageSeconds !== 300 || options.soakSeconds !== 7200 ||
    options.combatActors !== 10 || options.resumeSamples !== 5 || options.saveSamples !== 5 || options.wallSeconds !== 18000 || options.movementIntervalMs !== 750 || options.attackResults !== 'v2') throw new Error('Formal frozen workload differs');
const { capacityPolicy, LOGIN_WINDOW_MS } = await import(pathToFileURL(path.join(project, 'apps/web/scripts/playtest-capacity-soak-core.mjs')));
if (JSON.stringify(capacityPolicy(options)) !== JSON.stringify(options.policy) || LOGIN_WINDOW_MS !== 901000) throw new Error('Formal policy or ordinary login window differs');
await fs.writeFile(path.join(root, 'preparation.lock'), JSON.stringify({ preparingAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, feasibilityRunId: feasibilityGate.runId }) + '\n', { flag: 'wx' });
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
const accounts = Array.from({ length: 100 }, (_, index) => { const suffix = index.toString(36).padStart(2, '0'); return { accountId: 'lt' + batch + suffix, name: 'L' + batch + suffix,
  password: crypto.randomBytes(10).toString('hex'), state: 'registered', provisioning: 'admin-provisioned-file-account-only' }; });
const poolFile = path.join(root, 'private/capacity-pool.json');
await writeNew(poolFile, { schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint, batch, accounts });
for (const name of ['recovery-mac-key.txt', 'identity-session-secret.txt', 'identity-recovery-pepper.txt', 'passkey-auth-secret.txt']) await fs.writeFile(path.join(root, 'private', name), crypto.randomBytes(32).toString('hex'), { flag: 'wx', mode: 0o600 });
const seeder = path.join(base, '../cell-preproduction-p0-p1-20261024/target/debug/seed_capacity_accounts.exe'), seedPin = await pin(seeder);
const provisioning = JSON.parse(execFileSync(seeder, ['--manifest', poolFile, '--account-store', path.join(root, 'private/accounts.json')], { encoding: 'utf8', windowsHide: true, timeout: 180000 }));
if (provisioning.passed !== true || provisioning.created !== 100 || provisioning.charactersCreated !== 0 || provisioning.gameplayGrants !== 0 || provisioning.networkRequests !== 0 || provisioning.existingStoreModified !== false) throw new Error('Account-only100 provisioning contract differs');
await writeNew(path.join(root, 'provisioning.json'), provisioning);
await writeNew(path.join(root, 'feasibility-gate.json'), feasibilityGate);
await writeNew(path.join(root, 'gateway-build-pin.json'), { schemaVersion: 1, sourceExecutable: releaseBinding.release.binary.path, ownedExecutable: ownExe,
  sha256: copied.sha256, bytes: copied.bytes, sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, baseCommit: releaseBinding.release.baseCommit,
  sourceManifest: { ...releaseBinding.manifestPin, sourceManifestSha256: releaseBinding.manifest.sourceManifestSha256, inputs: releaseBinding.manifest.files.length }, releaseRecord: releaseBinding.releasePin,
  copiedAtUtc: new Date().toISOString(), sourceInputsVerifiedBeforeLaunch: true, capacityAccepted: false, formalRunStarted: false });
const scripts = (await fs.readdir(root)).filter(name => /\.(?:mjs|ps1)$/.test(name)).sort();
const publicFiles = [driverFrozenFile, launcherFreezeFile, path.join(root, 'preparation-plan.json'), releaseBinding.releasePin.file, releaseBinding.manifestPin.file,
  path.join(base, 'release-native-10-v2-feasibility-05/gateway-build-pin.json'), path.join(base, 'capacity-attack-v2-driver-frozen-v1.json'),
  ...scripts.map(name => path.join(root, name)), ...['scenario.json', 'resource-profile.json', 'run-options.json', 'scenario-prefix-readiness.json', 'provisioning.json', 'gateway-build-pin.json', 'feasibility-gate.json'].map(name => path.join(root, name)),
  candidateFile, path.join(priorRoot, 'scenario.json'), path.join(base, 'formal-baseline-100/resource-profile-local-host.json'),
  path.join(base, 'partition-stage-two-name-safe-home-supply-plan-v3.json'), path.join(base, 'partition-stage-two-name-package-v3.json')];
const files = [...drivers.pins, ...await Promise.all(publicFiles.map(pin)), ...feasibilityGate.files, await pin(process.execPath), seedPin];
await writeNew(path.join(root, 'source-pins.json'), { schemaVersion: 1, preparedAt: new Date().toISOString(), sourceCommit: releaseBinding.backendSourceCommit, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit,
  backendSourceManifest: releaseBinding.manifestPin.file, originalBackendReleaseReused: true, driverFrozenManifest: driverFrozenFile, driverSourceSha256: drivers.frozen.sourceSha256, sourceConfigSha256: sha(JSON.stringify(files)), files, feasibilityGate,
  localImportModuleCount: drivers.importModuleCount, nextBundledWsEntry: drivers.bundledWsEntry, acl, scope: 'Exact formal release/driver/import/transport/data/runner and authentic feasibility raw closure. No private store/credential/recording payload in public evidence.',
  ordinaryLoginBudget: { admission: 25, reservedVerification: 5, total: 30, windowMs: LOGIN_WINDOW_MS, persistentLedger: true, resetOrBypass: false },
  realOrdinaryAccounts: 100, charactersCreated: 0, gameplayGrants: 0, realmStarted: false, capacityAccepted: false });
console.log(JSON.stringify({ prepared: true, root, backendSourceCommit: releaseBinding.backendSourceCommit, driverSourceCommit: releaseBinding.driverSourceCommit, backendInputs: releaseBinding.manifest.files.length,
  sha256: copied.sha256, realOrdinaryAccounts: 100, charactersCreated: 0, sourceConfigSha256: sha(JSON.stringify(files)), feasibilityRunId: feasibilityGate.runId, realmStarted: false, capacityAccepted: false }));
