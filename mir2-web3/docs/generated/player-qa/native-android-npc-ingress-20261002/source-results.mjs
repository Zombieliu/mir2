// Reproduce this source-only checkpoint from the repository root.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const qa = path.dirname(fileURLToPath(import.meta.url));
const raw = 'mir2-web3/apps/game-client/platform-android/target/npc-ingress-v17-20261002';
const selected = process.argv[2] ?? null;
const windowsChecks = [
  ['gateway-tests-actual_npc_response_retires_service_children', 1],
  ['gateway-tests-pearl_goods_currency_survives_snapshot', 1],
  ['gateway-tests-npc_goods_then_npc_sell', 1],
  ['gateway-tests-crystal_packet_tooltip_projection_preserves_instance', 1],
  ['gameplay_bridge-tests-npc_service_', 2],
];
const hash = value => createHash('sha256').update(value).digest('hex');
const git = (...args) => execFileSync('git', args, {encoding: 'utf8', maxBuffer: 8 * 1024 * 1024}).trim();
const proofNames = [
  'rust-failure-first.log', 'java-failure-first.log', 'Java-failure-first.xml',
  'rust-focused-first.log', 'rust-focused-final.log', 'java-focused.log',
  'android-all.log', 'android-preview-all.log', 'shared-all.log', 'runtime-all.log', 'java-all.log',
  'shared-frame-failure-first.log', 'shared-frame-first-fix.log', 'shared-final-all.log',
  'shared-repaired-all.log', 'shared-close-pending-failure-first.log', 'shared-close-pending-red.log',
  'shared-final-fixed-all.log', 'android-final-fixed-all.log', 'android-preview-final-fixed-all.log',
  'runtime-final-all.log', 'java-final-all.log', 'java-fresh-all.log',
  'api31-debug-final.log', 'api31-preview-final.log', 'windows-npc-mac-source.log',
  'equivalence-audit-first.log', 'equivalence-audit-final.log',
  ...windowsChecks.map(([name]) => `windows-${name}.log`),
];
const originals = proofNames.map(name => {
  const target = path.join(qa, name);
  if (fs.existsSync(path.join(raw, name))) {
    const bytes = fs.readFileSync(path.join(raw, name));
    if (fs.existsSync(target)) assert(bytes.equals(fs.readFileSync(target)), `would overwrite original ${name}`);
    else fs.writeFileSync(target, bytes);
  }
  return {name, sha256: hash(fs.readFileSync(target))};
});
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const resultFor = (name, status, expected) => {
  const match = read(name).match(new RegExp(`test result: ${status}\\. (\\d+) passed; (\\d+) failed; (\\d+) ignored`));
  assert(match, name);
  const counts = match.slice(1).map(Number);
  if (expected) assert.deepEqual(counts, expected, name);
  return {name, passed: counts[0], failed: counts[1], ignored: counts[2]};
};
const failureFirst = [
  resultFor('rust-failure-first.log', 'FAILED', [1, 2, 0]),
  resultFor('shared-frame-failure-first.log', 'FAILED', [1, 1, 0]),
  resultFor('shared-close-pending-red.log', 'FAILED', [0, 3, 0]),
];
assert.match(read('java-failure-first.log'), /2 tests completed, 2 failed/);
assert.match(read('Java-failure-first.xml'), /tests="2"/);
assert.match(read('Java-failure-first.xml'), /failures="2"/);
const rust = [
  resultFor('shared-final-fixed-all.log', 'ok', [1230, 0, 10]),
  resultFor('android-final-fixed-all.log', 'ok', [290, 0, 0]),
  resultFor('android-preview-final-fixed-all.log', 'ok', [304, 0, 0]),
  resultFor('runtime-final-all.log', 'ok', [292, 0, 1]),
  ...windowsChecks.map(([name, count]) => resultFor(`windows-${name}.log`, 'ok', [count, 0, 0])),
];
const windowsBroadFilter = resultFor('windows-npc-mac-source.log', 'FAILED', [19, 6, 0]);
assert.match(read('java-fresh-all.log'), /BUILD SUCCESSFUL/);
assert.match(read('java-fresh-all.log'), /42 actionable tasks: 42 executed/);
const java = ['Debug', 'UiPreview'].map(variant => {
  const directory = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
  const files = fs.existsSync(directory) ? fs.readdirSync(directory).filter(name => name.endsWith('.xml')) :
    fs.readdirSync(qa).filter(name => name.startsWith(`${variant}-TEST-`) && name.endsWith('.xml'))
      .map(name => name.slice(variant.length + 1));
  assert.equal(files.length, 5);
  const totals = {tests: 0, failures: 0, errors: 0, skipped: 0};
  for (const name of files) {
    const archived = path.join(qa, `${variant}-${name}`);
    const bytes = fs.existsSync(directory) ? fs.readFileSync(path.join(directory, name)) : fs.readFileSync(archived);
    if (fs.existsSync(archived)) assert(bytes.equals(fs.readFileSync(archived)), `would overwrite Java evidence ${name}`);
    else fs.writeFileSync(archived, bytes);
    const suite = bytes.toString().match(/<testsuite[^>]+>/)?.[0];
    assert(suite, name);
    for (const field of Object.keys(totals)) totals[field] += Number(suite.match(new RegExp(`${field}="(\\d+)"`))[1]);
  }
  assert.deepEqual(totals, {tests: 39, failures: 0, errors: 0, skipped: 0});
  return {variant, ...totals, environment: 'local synthetic TLS/MockWebServer, not real login'};
});
for (const [name, variant] of [['debug', 'debug'], ['preview', 'uiPreview']]) {
  assert(read(`api31-${name}-final.log`).includes(`check ${variant} aarch64-linux-android`));
  assert(read(`api31-${name}-final.log`).includes('target check passed'));
}
const files = [
  'client-bevy/src/lib.rs', 'client-bevy/src/native_inventory_ingress.rs',
  'client-bevy/src/native_npc_ingress.rs', 'client-bevy/src/read_model.rs',
  'client-bevy/src/crystal_ui/overlays.rs', 'client-bevy/src/crystal_ui/npc_shop_layout_tests.rs',
  'runtime/src/lib.rs', 'platform-windows/src/gateway.rs',
  'platform-android/src/lib.rs', 'platform-android/src/player_ingress.rs',
  'platform-android/src/shared_shell.rs', 'platform-android/src/npc_ingress.rs',
  'platform-android/src/npc_host_tests.rs',
  'platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java',
  'platform-android/android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java',
].map(relative => {
  const file = `mir2-web3/apps/game-client/${relative}`, bytes = fs.readFileSync(file);
  if (selected) {
    const committed = execFileSync('git', ['show', `${selected}:${file}`], {maxBuffer: 4 * 1024 * 1024});
    assert(bytes.equals(committed), `source differs from tested checkpoint: ${file}`);
  }
  return {path: file, sha256: hash(bytes)};
});
const result = {
  scope: 'NI-11 bounded NPC service/catalogue source ingress; NI-10 remains open',
  source: selected ? git('rev-parse', selected) : null,
  sourceParent: selected ? git('rev-parse', `${selected}^1`) : git('rev-parse', 'HEAD'),
  frozenWindows: '3d735745f1117d42a7859e87604a106351dca935',
  sourceFiles: files, originals, failureFirst,
  javaFailureFirst: {tests: 2, failures: 2, environment: 'local synthetic TLS'},
  retainedSetupAndIntermediateFailures: ['rust-focused-first.log', 'shared-final-all.log',
    'shared-close-pending-failure-first.log', 'equivalence-audit-first.log'],
  rust, java, extraction: JSON.parse(read('source-equivalence.json')),
  windowsBroadFilter: {...windowsBroadFilter,
    scope: 'Mac source run; six geometry/marker/map-fixture failures retained, no broad Windows gate pass'},
  arm64Api31: {debug: true, uiPreview: true},
  hostPairEnqueueAtomic: true, hostFailedPushRetriesOnlySuffix: true,
  sharedRuntimeEndToEndLosslessAccepted: false, completeNpcReceiptsAccepted: false,
  fullQuestDialogAccepted: false, newApkBuilt: false, emulatorNpcFlowAccepted: false,
  authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  existingV16GlErrorsRetained: true, fullWindowsParityAccepted: false, goalStatus: 'active',
};
fs.writeFileSync(path.join(qa, 'source-results.json'), `${JSON.stringify(result, null, 2)}\n`);
console.log(JSON.stringify({source: result.source, rust, java, originals: originals.length,
  sourceFiles: files.length, fullWindowsParityAccepted: false}));
