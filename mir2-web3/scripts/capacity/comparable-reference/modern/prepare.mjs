// Creates new local-only fixture inputs and copies the pinned active map.
// Does not run a Gateway, seed binary, socket, Docker or workload.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { MAP_SHA, hash, requireThat as check, validatePlan, decodeNavigation } from './contract.mjs';

export async function securePrivate(directory) {
  await fs.chmod(directory, 0o700);
  if (process.platform === 'win32') {
    const source = "$ErrorActionPreference='Stop';$p=$env:MIR2_COMPARABLE_PRIVATE_PATH;$sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User;$acl=New-Object System.Security.AccessControl.DirectorySecurity;$acl.SetOwner($sid);$acl.SetAccessRuleProtection($true,$false);$acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule($sid,'FullControl','ContainerInherit,ObjectInherit','None','Allow')));Set-Acl -LiteralPath $p -AclObject $acl;if(-not (Get-Acl -LiteralPath $p).AreAccessRulesProtected){throw 'unprotected'}";
    const childEnv = { ...process.env, MIR2_COMPARABLE_PRIVATE_PATH: directory };
    for (const key of Object.keys(childEnv)) if (key.toUpperCase() === 'PSMODULEPATH') delete childEnv[key];
    execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand', Buffer.from(source, 'utf16le').toString('base64')],
      { env: childEnv, windowsHide: true, stdio: 'pipe' });
  }
}
const writeNew = (file, value) => fs.writeFile(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });

export async function prepareInputs({ planFile, navigationFile, originalMap, output, url, sourceRevision, combatContractFile = null }) {
  for (const p of [planFile, navigationFile, originalMap, output]) check(path.isAbsolute(p), 'Explicit absolute paths required');
  if (process.platform === 'win32') check(/^c:[\\/]/i.test(output), 'All new artifacts must be on C drive');
  const endpoint = new URL(url);
  check(endpoint.protocol === 'ws:' && ['127.0.0.1', '[::1]'].includes(endpoint.hostname) && endpoint.port && ['/ws', '/playtest/ws'].includes(endpoint.pathname) &&
    !endpoint.username && !endpoint.password && !endpoint.search && !endpoint.hash, 'Literal loopback endpoint required');
  check(/^[0-9a-f]{40}$/.test(sourceRevision), 'Exact prospective build source required');
  const [planBytes, navBytes, rawMap] = await Promise.all([fs.readFile(planFile), fs.readFile(navigationFile), fs.readFile(originalMap)]);
  const plan = JSON.parse(planBytes), nav = JSON.parse(navBytes); let combatBytes;
  if (plan.scenario === 'dispersed-melee') {
    check(combatContractFile && path.isAbsolute(combatContractFile), 'Explicit absolute --combat-contract required for melee');
    combatBytes = await fs.readFile(combatContractFile);
    (await import('./combat.mjs')).validateCombatPlan(plan, combatBytes);
  } else { check(combatContractFile === null, 'Dense preparation must not enable a combat contract'); validatePlan(plan); }
  check(hash(rawMap) === MAP_SHA && rawMap.length === 9600054, 'Active original map bytes required');
  check(plan.navigationSha256 === hash(navBytes), 'Original-library raw navigation SHA mismatch');
  const eligible = decodeNavigation(nav, plan.map);
  for (const a of plan.actors) for (const p of a.lane) check(eligible(p.x, p.y), 'Plan lane is not ordinary walkable navigation');
  const parent = path.dirname(output), realParent = await fs.realpath(parent);
  check(path.resolve(parent).toLowerCase() === realParent.toLowerCase(), 'Linked output parent rejected');
  await fs.mkdir(output); // Never reuse an old fixture, ledger or run directory.
  const privateDir = path.join(output, 'private'), clientRoot = path.join(output, 'source-client');
  await fs.mkdir(privateDir, { mode: 0o700 }); await securePrivate(privateDir);
  await fs.mkdir(path.join(clientRoot, 'Map'), { recursive: true }); await fs.mkdir(path.join(output, 'recovery'), { mode: 0o700 });
  await fs.writeFile(path.join(clientRoot, 'Map', 'DogYoHyun.map'), rawMap, { flag: 'wx' });
  await fs.writeFile(path.join(output, 'plan.json'), planBytes, { flag: 'wx' });
  await fs.writeFile(path.join(output, 'navigation.json'), navBytes, { flag: 'wx' });
  if (combatBytes) await fs.writeFile(path.join(output, 'combat-contract.json'), combatBytes, { flag: 'wx' });
  const batch = crypto.randomBytes(4).toString('hex');
  const actors = plan.actors.map(a => ({ ordinal: a.ordinal, accountId: `mc${batch}${a.ordinal.toString(36).padStart(2, '0')}`,
    password: crypto.randomBytes(9).toString('hex'), name: `Mc${batch}${a.ordinal.toString(36).padStart(2, '0')}` }));
  const manifest = { schema: 'mir2.comparable.modern-actors.v1', qualificationLayer: 'game-core-local',
    planSha256: hash(planBytes), navigationSha256: hash(navBytes), mapSha256: MAP_SHA, actors };
  const accountStore = path.join(privateDir, 'accounts.json');
  await writeNew(path.join(privateDir, 'actors.json'), manifest);
  await writeNew(path.join(privateDir, 'login-ledger.json'), { schema: 'mir2.comparable.local-auth-ledger.v1', purpose: 'comparable-local-only',
    endpoint: endpoint.toString(), planSha256: hash(planBytes), attempts: [] });
  await writeNew(path.join(output, 'modern-offline.json'), { schema: 'mir2.comparable.modern-offline.v1', gatewayState: 'never-started',
    gatewayPid: null, actualExitCode: null, liveMarker: path.join(output, 'modern-live.json') });
  const env = { MIR2_ENV: 'local', MIR2_RUNTIME_ENV: 'development', MIR2_IDENTITY_POLICY: 'commercial', MIR2_ACCOUNT_STORE_BACKEND: 'file',
    MIR2_ACCOUNT_STORE_PATH: accountStore, MIR2_GATEWAY_REDIS_CACHE_URL: null, MIR2_GATEWAY_REQUIRE_REDIS_CACHE: '0',
    MIR2_CHANNEL_IDENTITY_DATABASE_URL: null, MIR2_REQUIRE_CHANNEL_IDENTITY_POSTGRES: '0', MIR2_REQUIRE_CHANNEL_IDENTITY_STORE: '0',
    MIR2_CONTENT_PROFILE: 'crystal_full', CRYSTAL_CLIENT_ROOT: clientRoot, MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: '0',
    MIR2_GATEWAY_STDIN_SHUTDOWN: '1',
    MIR2_GATEWAY_WEB_ADDR: endpoint.host, MIR2_GATEWAY_TCP_ADDR: null, MIR2_ALLOWED_WEB_ORIGINS: `${endpoint.protocol === 'ws:' ? 'http:' : 'https:'}//${endpoint.host}`,
    MIR2_GATEWAY_ID: `comparable-local-${batch}`, MIR2_REALM_ID: `comparable-local-${batch}`, MIR2_SAVE_RECOVERY_DIR: path.join(output, 'recovery'),
    MIR2_GATEWAY_TOKIO_WORKER_THREADS: '4', MIR2_PROCUREMENT_ENABLED: '0', MIR2_SUI_USDC_ENABLED: '0', MIR2_STRIPE_ENABLED: '0',
    MIR2_PASSKEY_AUTH_SECRET: crypto.randomBytes(32).toString('hex'), MIR2_IDENTITY_SESSION_SECRET: crypto.randomBytes(32).toString('hex'),
    MIR2_IDENTITY_RECOVERY_PEPPER: crypto.randomBytes(32).toString('hex'), MIR2_SAVE_RECOVERY_MAC_KEY: crypto.randomBytes(32).toString('hex') };
  await writeNew(path.join(privateDir, 'environment.json'), env);
  const publicReceipt = { schema: 'mir2.comparable.modern-inputs.v1', passed: true, preparedOnly: true, servicesStarted: false, networkRequests: 0,
    qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', sourceRevisionIntended: sourceRevision,
    sourceBuildVerified: false, planSha256: hash(planBytes), navigationSha256: hash(navBytes), mapSha256: MAP_SHA, count: actors.length,
    endpoint: endpoint.toString(), environmentKeys: Object.keys(env), scrubAllInheritedMir2AndCrystalEnvironment: true,
    requiredControllerValues: ['MIR2_GATEWAY_TCP_ADDR: different explicit loopback port', 'MIR2_CRYSTAL_MAP_PACK: pinned C-only full map pack'],
    seedArguments: ['--manifest', path.join(privateDir, 'actors.json'), '--plan', path.join(output, 'plan.json'), '--navigation', path.join(output, 'navigation.json'),
      '--account-store', accountStore, '--client-root', clientRoot, '--offline-receipt', path.join(output, 'modern-offline.json')],
    runtimeReceiptContract: { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory',
      sourceRevision, binarySha256: '<actual-source-pinned-binary-SHA256>', endpoint: endpoint.toString(), accountStore, planSha256: hash(planBytes),
      processAndQuotaProof: '<owned controller receipt, full4CPU/8GiB game-core boundary>' },
    capacityClaimAllowed: false, fullStackQualified: false };
  if (combatBytes) { publicReceipt.combatContractSha256 = hash(combatBytes);
    publicReceipt.seedArguments.push('--combat-contract', path.join(output, 'combat-contract.json')); }
  await writeNew(path.join(output, 'inputs-receipt.json'), publicReceipt); return publicReceipt;
}

async function main() {
  const args = process.argv.slice(2), opts = {};
  check([12, 14].includes(args.length), 'Use --plan --navigation --original-map --output --endpoint --source-revision and explicit --combat-contract for melee');
  for (let i = 0; i < args.length; i += 2) { check(['--plan', '--navigation', '--original-map', '--output', '--endpoint', '--source-revision', '--combat-contract'].includes(args[i]) && !opts[args[i]], 'Unknown/duplicate option'); opts[args[i]] = args[i + 1]; }
  const receipt = await prepareInputs({ planFile: opts['--plan'], navigationFile: opts['--navigation'], originalMap: opts['--original-map'],
    output: opts['--output'], url: opts['--endpoint'], sourceRevision: opts['--source-revision'], combatContractFile: opts['--combat-contract'] ?? null });
  console.log(JSON.stringify({ passed: receipt.passed, preparedOnly: true, count: receipt.count, planSha256: receipt.planSha256, qualificationLayer: 'game-core-local' }));
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) main().catch(e => { console.error(JSON.stringify({ passed: false, error: e.message })); process.exitCode = 1; });
