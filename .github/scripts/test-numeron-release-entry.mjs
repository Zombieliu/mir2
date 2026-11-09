import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import vm from 'node:vm';

const gateway = readFileSync(new URL('../workflows/gateway-release.yml', import.meta.url), 'utf8').replaceAll('\r\n', '\n');
const promotion = readFileSync(new URL('../workflows/promotion-website-release.yml', import.meta.url), 'utf8').replaceAll('\r\n', '\n');
const SHA = 'a'.repeat(40), OLD_SHA = 'b'.repeat(40);
const legacyJobs = {
  'build-playtest-promotion-backend': '9c368f03c24cc88737174c02aff5db6a696e9dfe141cfa9a7104905c96698108',
  'activities-acceptance': '5aa4bd70027bbda34795257387f67fc13557ed800337b9197798ac1741a42f73',
  'siege-acceptance': 'd383e131a83ff54e3a94dca992da7c060c03be868ae3697d19b3e436933c28f1',
  'build-linux-x64': '4c00dc017b4425af5f1f20a2ffddde655bfbdc373e479c53261bffef9aa19828',
  'build-playtest-wallet-gateway': '9d11bdec2398db9d3b829d7d49e5f3acc6d62a1f23536912706f01bcfda78e2c',
  'build-playtest-wallet-login': '2482fd61f6f227ba01807e833840b763c7087e9b42fb3461b04867b482f8ac9a',
};
const fields = ['unified_source_sha', 'asset_run_id', 'deployment_id', 'main_deployment_id'];
function block(source, name) {
  const marker = `\n  ${name}:\n`, start = source.indexOf(marker);
  assert(start >= 0, `missing job ${name}`);
  return source.slice(start + 1).split(/\n(?=  [a-z][a-z0-9-]*:\n)/)[0];
}
function inputs(values = {}) {
  return { numeron_operation: 'off', unified_source_sha: '', asset_run_id: '', deployment_id: '', main_deployment_id: '',
    numeron_ingress_guard: '', release_tag: '', publish_r2: false, playtest_wallet_no_docker: false, ...values };
}
function admits(name, values) {
  const expression = block(gateway, name).match(/^    if: \$\{\{ (.+) \}\}$/m)?.[1];
  assert(expression, `${name} must have a real input condition`);
  return Boolean(vm.runInNewContext(`(${expression})`, { inputs: inputs(values) }, { timeout: 1000 }));
}
function script(source, name) {
  const position = source.indexOf(`- name: ${name}\n`); assert(position >= 0);
  const section = source.slice(position).split(/\n(?:      - (?:name:|uses:)|  [a-z][a-z0-9-]*:\n)/)[0];
  const match = section.match(/node --input-type=module <<'NODE'\n([\s\S]+?)\n +NODE(?:\n|$)/); assert(match);
  return { section, code: match[1].replace(/^          /gm, '') };
}
const wrapperGuard = script(gateway, 'Validate the isolated Numeron dispatch without credentials');
const reusableGuard = script(promotion, 'Validate isolated Numeron inputs before any credential-bearing job');
function environment(values) {
  const value = inputs(values);
  return { NUMERON_OPERATION: value.numeron_operation, NUMERON_SOURCE_SHA: value.unified_source_sha,
    NUMERON_ASSET_RUN_ID: value.asset_run_id, NUMERON_DEPLOYMENT_ID: value.deployment_id,
    NUMERON_MAIN_DEPLOYMENT_ID: value.main_deployment_id, NUMERON_INGRESS_GUARD: value.numeron_ingress_guard, LEGACY_RELEASE_TAG: value.release_tag,
    LEGACY_PUBLISH_R2: String(value.publish_r2), LEGACY_PLAYTEST_WALLET: String(value.playtest_wallet_no_docker),
    BACKEND_SOURCE_SHA: '', GITHUB_REPOSITORY: 'Zombieliu/mir2', GITHUB_REF: 'refs/heads/codex/windows-player-journey', GITHUB_SHA: SHA };
}
const execute = (guard, env) => vm.runInNewContext(guard.code, { process: { env } }, { timeout: 1000 });
const valid = operation => inputs({ numeron_operation: operation,
  ...(['numeron-stage', 'numeron-promote'].includes(operation) ? { asset_run_id: '42479115894' } : {}),
  ...(['numeron-promote', 'numeron-domain-move'].includes(operation) ? { deployment_id: 'dpl_game123', main_deployment_id: 'dpl_main456', unified_source_sha: OLD_SHA } : {}) });

test('off/default retains every legacy job body byte-for-byte and the original job selection', () => {
  for (const [name, expected] of Object.entries(legacyJobs)) {
    const normalized = block(gateway, name).trimEnd().split('\n').filter(line => !line.startsWith('    if:')).join('\n') + '\n';
    assert.equal(createHash('sha256').update(normalized).digest('hex'), expected, name);
  }
  for (const operation of ['off', '']) for (const wallet of [false, true]) for (const publish of [false, true]) {
    const values = inputs({ numeron_operation: operation, playtest_wallet_no_docker: wallet, publish_r2: publish, release_tag: 'existing-release-tag' });
    assert.equal(admits('validate-numeron-entry', values), false); assert.equal(admits('numeron-website', values), false);
    for (const name of Object.keys(legacyJobs)) {
      const expected = ['build-playtest-promotion-backend', 'build-playtest-wallet-gateway', 'build-playtest-wallet-login'].includes(name) ? wallet : !wallet;
      assert.equal(admits(name, values), expected, name);
    }
  }
  assert.match(gateway, /numeron_operation:\n[\s\S]*?default: 'off'\n/);
});

test('each explicit Numeron operation selects only validation and the reusable website job', () => {
  for (const operation of ['numeron-context', 'numeron-stage', 'numeron-promote', 'numeron-domain-move']) {
    const values = valid(operation);
    assert.equal(admits('validate-numeron-entry', values), true); assert.equal(admits('numeron-website', values), true);
    for (const name of Object.keys(legacyJobs)) assert.equal(admits(name, values), false, name);
    execute(wrapperGuard, environment(values)); execute(reusableGuard, environment(values));
  }
  assert.match(block(gateway, 'numeron-website'), /needs: validate-numeron-entry/);
  assert.match(block(gateway, 'numeron-website'), /uses: \.\/\.github\/workflows\/promotion-website-release\.yml/);
});

test('orphan Numeron inputs fail closed rather than dispatching legacy jobs', () => {
  for (const field of [...fields, 'numeron_ingress_guard']) for (const operation of ['off', '']) {
    const values = inputs({ numeron_operation: operation, [field]: 'supplied' });
    assert.equal(admits('validate-numeron-entry', values), true);
    for (const name of Object.keys(legacyJobs)) assert.equal(admits(name, values), false);
    assert.throws(() => execute(wrapperGuard, environment(values)), /explicitNumeronOperationRequired/);
  }
  for (const mode of ['numeron-off', 'context', 'numeron-unknown']) {
    const values = inputs({ numeron_operation: mode });
    for (const name of Object.keys(legacyJobs)) assert.equal(admits(name, values), false);
    assert.throws(() => execute(wrapperGuard, environment(values)), /explicitNumeronOperationRequired/);
  }
});

test('ingress operations only select the guarded DNS job and keep deployment inputs separate', () => {
  for (const mode of ['inspect', 'proxy', 'restore']) {
    const values = inputs({ numeron_operation: `numeron-ingress-${mode}`,
      numeron_ingress_guard: mode === 'inspect' ? '' : `${'c'.repeat(64)}:300` });
    assert.equal(admits('validate-numeron-entry', values), true);
    assert.equal(admits('numeron-website', values), false);
    assert.equal(admits('numeron-ingress', values), true);
    for (const name of Object.keys(legacyJobs)) assert.equal(admits(name, values), false, name);
    execute(wrapperGuard, environment(values));
    for (const field of fields) assert.throws(() => execute(wrapperGuard, environment({ ...values, [field]: 'mixed' })), /InputInvalid/);
    assert.throws(() => execute(wrapperGuard, environment({ ...values, numeron_ingress_guard: 'invalid' })), /ingressGuardInvalid/);
  }
  assert.doesNotMatch(block(gateway, 'numeron-ingress'), /VERCEL|PASSKEY|GH_TOKEN|MIR2_R2/);
});

test('legacy publishing and wallet switches cannot be mixed into Numeron dispatch', () => {
  for (const change of [{ release_tag: 'old-tag' }, { publish_r2: true }, { playtest_wallet_no_docker: true }]) {
    const values = { ...valid('numeron-stage'), ...change };
    for (const name of Object.keys(legacyJobs)) assert.equal(admits(name, values), false);
    assert.throws(() => execute(wrapperGuard, environment(values)), /mixedReleaseModes/);
  }
  const env = environment(valid('numeron-stage')); env.BACKEND_SOURCE_SHA = SHA;
  assert.throws(() => execute(reusableGuard, env), /mixedReleaseModes/);
});

test('both actual guards reject bad source identity, stale stage source and missing or confused inputs', () => {
  for (const guard of [wrapperGuard, reusableGuard]) {
    for (const change of [{ GITHUB_REF: 'refs/heads/main' }, { GITHUB_REPOSITORY: 'other/repo' }, { GITHUB_SHA: 'not-a-sha' }])
      assert.throws(() => execute(guard, { ...environment(valid('numeron-stage')), ...change }), /sourceIdentityInvalid/);
    assert.throws(() => execute(guard, environment({ ...valid('numeron-stage'), unified_source_sha: OLD_SHA })), /sourceInputInvalid/);
    assert.throws(() => execute(guard, environment({ ...valid('numeron-promote'), unified_source_sha: '../other' })), /sourceInputInvalid/);
    for (const operation of ['numeron-stage', 'numeron-promote']) for (const id of ['', '0', '-1', '1;curl bad', '4\n2'])
      assert.throws(() => execute(guard, environment({ ...valid(operation), asset_run_id: id })), /assetRunInputInvalid/);
    for (const operation of ['numeron-context', 'numeron-domain-move'])
      assert.throws(() => execute(guard, environment({ ...valid(operation), asset_run_id: '123' })), /assetRunInputInvalid/);
    for (const operation of ['numeron-context', 'numeron-stage'])
      assert.throws(() => execute(guard, environment({ ...valid(operation), deployment_id: 'dpl_unused' })), /candidateInputInvalid/);
    for (const operation of ['numeron-promote', 'numeron-domain-move']) {
      for (const change of [{ deployment_id: '' }, { main_deployment_id: '' }, { deployment_id: 'https://other.example' }, { main_deployment_id: 'dpl_game123' }])
        assert.throws(() => execute(guard, environment({ ...valid(operation), ...change })), /candidateInputInvalid/);
    }
  }
});

test('the reusable workflow declares all four optional inputs and forwards only the necessary secrets', () => {
  const call = promotion.split('  workflow_call:\n')[1].split('  workflow_dispatch:\n')[0];
  for (const name of fields) assert.match(call, new RegExp(`      ${name}:\\n        required: false\\n        type: string`));
  assert.match(call, /backend_source_sha:\n        required: false\n/);
  const job = block(gateway, 'numeron-website');
  for (const name of fields) assert(job.includes(`${name}: \${{ inputs.${name} }}`));
  assert.doesNotMatch(job, /secrets: inherit/);
  assert.match(job, /MIR2_R16_PASSKEY_AUTH_SECRET: \$\{\{ inputs\.numeron_operation == 'numeron-stage' && secrets\.MIR2_R16_PASSKEY_AUTH_SECRET \|\| '' \}\}/);
  for (const name of ['VERCEL_TOKEN', 'VERCEL_ORG_ID', 'VERCEL_PROJECT_ID', 'MIR2_R16_PASSKEY_AUTH_SECRET']) assert(call.includes(`      ${name}:\n        required: false`));
  assert.match(gateway.split('\njobs:\n')[0], /permissions:\n  contents: read\n  actions: read/);
  assert.match(block(promotion, 'website'), /permissions:\n      contents: read\n      actions: read/);
});

test('validation and context steps have no Gateway secret access and cannot interpolate input as code', () => {
  for (const guard of [wrapperGuard, reusableGuard]) {
    assert.doesNotMatch(guard.section, /secrets\.|PASSKEY|VERCEL_TOKEN|GH_TOKEN/);
    assert.doesNotMatch(guard.code, /\$\{\{|fetch\(|exec|readFile|console\.log/);
  }
  const context = promotion.split('- name: Inspect the three Numeron project identities without Gateway credentials\n')[1].split('\n      - name:')[0];
  assert.doesNotMatch(context, /PASSKEY|GH_TOKEN|numeron-r16-domain/);
  assert.match(context, /if: inputs.operation == 'numeron-context'/);
});

test('domain move invokes only its separate helper and receipt; stage and promote never move the domain', () => {
  const domain = promotion.split('- name: Move the Numeron custom domain only after independent paired-candidate checks\n')[1].split('\n      - name:')[0];
  assert.match(domain, /if: inputs.operation == 'numeron-domain-move'/);
  assert.match(domain, /run: node \.github\/scripts\/numeron-r16-domain\.mjs/);
  assert.doesNotMatch(domain, /PASSKEY|GH_TOKEN|asset_run_id|numeron-r16-release\.mjs/);
  assert.match(promotion, /if: always\(\) && inputs.operation == 'numeron-domain-move'[\s\S]*?path: numeron-r16-domain.json/);
  const paired = promotion.split('- name: Stage or promote the isolated Numeron R16 pair without moving the custom domain\n')[1].split('\n      - name:')[0];
  assert.match(paired, /if: inputs.operation == 'numeron-stage' \|\| inputs.operation == 'numeron-promote'/);
  assert.doesNotMatch(paired, /numeron-r16-domain/);
  assert(promotion.includes("&& !startsWith(inputs.operation, 'numeron-')"));
  assert.match(promotion, /cancel-in-progress: false/);
});
