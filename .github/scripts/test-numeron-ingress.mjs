import assert from 'node:assert/strict';
import { test } from 'node:test';
import { runNumeronIngress } from './numeron-ingress.mjs';

const account = 'a'.repeat(32), zoneId = 'b'.repeat(32), recordId = 'c'.repeat(32);
const host = 'numeron.obelisk.build', zonePath = `/zones/${zoneId}`;
const env = { GITHUB_REPOSITORY: 'Zombieliu/mir2', GITHUB_REF: 'refs/heads/codex/windows-player-journey',
  GITHUB_EVENT_NAME: 'workflow_dispatch', GITHUB_SHA: 'f'.repeat(40),
  CLOUDFLARE_ACCOUNT_ID: account, CLOUDFLARE_API_TOKEN: 'fake-ci-secret' };

function harness({ sslMode = 'strict', content = 'f0c0be2daf5700de.vercel-dns-016.com' } = {}) {
  let record = { id: recordId, name: host, type: 'CNAME', content, proxiable: true, proxied: false, ttl: 300, modified_on: 'initial' };
  const calls = [], reports = [];
  const fetchImpl = async (url, init) => {
    const path = new URL(url).pathname.replace('/client/v4', '');
    const body = init.body ? JSON.parse(init.body) : undefined;
    calls.push({ path, method: init.method, body });
    const results = {
      '/zones': [{ id: zoneId, name: 'obelisk.build', account: { id: account }, paused: false, status: 'active', type: 'full' }],
      [`${zonePath}/dns_records`]: [record],
      [`${zonePath}/settings/ssl`]: { id: 'ssl', value: sslMode },
      [`${zonePath}/dns_records/${recordId}`]: record,
    };
    assert(path in results);
    if (init.method === 'PATCH') record = { ...record, ...body, modified_on: 'changed' };
    return Response.json({ success: true, result: results[path] });
  };
  const run = (operation, guard = '', overrides = {}) => runNumeronIngress({
    env: { ...env, NUMERON_OPERATION: `numeron-ingress-${operation}`, NUMERON_INGRESS_GUARD: guard, ...overrides },
    fetchImpl, report: report => reports.push(report),
  });
  return { run, calls, reports };
}

test('read-only inspection, exact record proxy and rollback restore the DNS-only TTL', async () => {
  const h = harness();
  await h.run('inspect');
  assert(h.calls.every(call => call.method === 'GET'));
  const guard = `${h.reports[0].before.revision}:300`;
  await h.run('proxy', guard);
  const proxied = h.reports.at(-1).after;
  assert.equal(proxied.proxied, true);
  assert.equal(proxied.ttl, 1);
  await assert.rejects(h.run('restore', guard), /dnsRecordChangedSinceInspection/);
  await h.run('restore', `${proxied.revision}:300`);
  assert.equal(h.reports.at(-1).after.proxied, false);
  assert.equal(h.reports.at(-1).after.ttl, 300);
  assert.deepEqual(h.calls.filter(call => call.method === 'PATCH'), [
    { path: `${zonePath}/dns_records/${recordId}`, method: 'PATCH', body: { proxied: true, ttl: 1 } },
    { path: `${zonePath}/dns_records/${recordId}`, method: 'PATCH', body: { proxied: false, ttl: 300 } },
  ]);
  assert.doesNotMatch(JSON.stringify(h.reports), /fake-ci-secret/);
});

test('changed origin, wrong checkout, and unvalidated origin TLS fail before any write', async () => {
  const wrong = harness({ content: 'other.vercel-dns.com' });
  await assert.rejects(wrong.run('inspect'), /dnsRecordIdentityChanged/);
  assert(wrong.calls.every(call => call.method === 'GET'));

  const h = harness({ sslMode: 'full' });
  await assert.rejects(h.run('inspect', '', { GITHUB_REF: 'refs/heads/main' }), /sourceIdentityInvalid/);
  assert.equal(h.calls.length, 0);
  await h.run('inspect');
  await assert.rejects(h.run('proxy', `${h.reports[0].before.revision}:300`), /originCertificateValidationRequired/);
  assert(h.calls.every(call => call.method === 'GET'));
});
