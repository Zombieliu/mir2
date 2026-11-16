import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const ZONE = 'obelisk.build';
const HOST = 'numeron.obelisk.build';
const TARGET = 'f0c0be2daf5700de.vercel-dns-016.com';
const ID = /^[a-f0-9]{32}$/;
const OPERATIONS = ['numeron-ingress-inspect', 'numeron-ingress-proxy', 'numeron-ingress-restore'];

function requireState(condition, code) {
  if (!condition) throw new Error(code);
}

function recordSummary(record) {
  requireState(ID.test(record?.id) && record.name === HOST && record.type === 'CNAME'
    && typeof record.content === 'string' && record.content.replace(/\.$/, '') === TARGET && record.proxiable === true
    && typeof record.proxied === 'boolean' && Number.isInteger(record.ttl)
    && typeof record.modified_on === 'string', 'dnsRecordIdentityChanged');
  const revision = createHash('sha256').update(JSON.stringify([
    record.id, record.name, record.type, record.content, record.proxied, record.ttl, record.modified_on,
  ])).digest('hex');
  return { host: HOST, target: TARGET, proxied: record.proxied, ttl: record.ttl, revision };
}

export async function runNumeronIngress({ env = process.env, fetchImpl = fetch, report = console.log } = {}) {
  const operation = env.NUMERON_OPERATION;
  requireState(env.GITHUB_REPOSITORY === 'Zombieliu/mir2'
    && env.GITHUB_REF === 'refs/heads/codex/windows-player-journey'
    && env.GITHUB_EVENT_NAME === 'workflow_dispatch' && /^[a-f0-9]{40}$/.test(env.GITHUB_SHA || ''), 'sourceIdentityInvalid');
  requireState(OPERATIONS.includes(operation), 'ingressOperationInvalid');
  const inspect = operation === 'numeron-ingress-inspect';
  const guard = /^([a-f0-9]{64}):([1-9][0-9]{0,4})$/.exec(env.NUMERON_INGRESS_GUARD || '');
  requireState(inspect ? !env.NUMERON_INGRESS_GUARD : guard !== null, 'ingressGuardInvalid');
  requireState(ID.test(env.CLOUDFLARE_ACCOUNT_ID) && Boolean(env.CLOUDFLARE_API_TOKEN), 'cloudflareCiIdentityMissing');

  async function cloudflare(label, pathname, body) {
    let response;
    try {
      response = await fetchImpl(`https://api.cloudflare.com/client/v4${pathname}`, {
        method: body === undefined ? 'GET' : 'PATCH',
        headers: { authorization: `Bearer ${env.CLOUDFLARE_API_TOKEN}`, 'content-type': 'application/json' },
        ...(body === undefined ? {} : { body: JSON.stringify(body) }),
        redirect: 'error', signal: AbortSignal.timeout(15_000),
      });
    } catch { throw new Error(`${label}TransportFailed`); }
    const envelope = await response.json().catch(() => null);
    if (!response.ok || envelope?.success !== true) {
      const codes = Array.isArray(envelope?.errors) ? envelope.errors.map(error => error.code).filter(Number.isSafeInteger) : [];
      throw new Error(`${label}Http${response.status}${codes.length ? `_${codes.join('_')}` : ''}`);
    }
    return envelope.result;
  }

  const zoneParams = new URLSearchParams({ name: ZONE, 'account.id': env.CLOUDFLARE_ACCOUNT_ID });
  const zones = await cloudflare('listZone', `/zones?${zoneParams}`);
  const zone = Array.isArray(zones) && zones.length === 1 ? zones[0] : null;
  requireState(ID.test(zone?.id) && zone.name === ZONE && zone.account?.id === env.CLOUDFLARE_ACCOUNT_ID
    && zone.status === 'active' && zone.paused === false && zone.type === 'full', 'zoneIdentityChanged');
  const zonePath = `/zones/${zone.id}`;
  const records = await cloudflare('listRecords', `${zonePath}/dns_records?name.exact=${HOST}&type=CNAME`);
  requireState(Array.isArray(records) && records.length === 1, 'expectedOneIngressRecord');
  const record = records[0];
  const before = recordSummary(record);
  report({ operation, before });

  const proxied = operation === 'numeron-ingress-proxy';
  if (inspect || proxied) {
    const ssl = await cloudflare('readTlsMode', `${zonePath}/settings/ssl`);
    const mode = ['off', 'flexible', 'full', 'strict', 'origin_pull'].includes(ssl?.value) ? ssl.value : 'unknown';
    report({ tlsMode: mode });
    if (inspect) return;
    requireState(ssl?.id === 'ssl' && (mode === 'strict' || mode === 'origin_pull'), 'originCertificateValidationRequired');
  }

  const originalTtl = Number(guard[2]);
  requireState(originalTtl === 1 || (originalTtl >= 30 && originalTtl <= 86400), 'originalTtlInvalid');
  const ttl = proxied ? 1 : originalTtl;
  requireState(before.revision === guard[1] && before.proxied !== proxied
    && before.ttl === (proxied ? originalTtl : 1), 'dnsRecordChangedSinceInspection');
  const recordPath = `${zonePath}/dns_records/${record.id}`;
  // If a write or its verification is interrupted, inspect again before retrying.
  await cloudflare('updateRecord', recordPath, { proxied, ttl });
  const saved = await cloudflare('readRecord', recordPath);
  const after = recordSummary(saved);
  requireState(saved.id === record.id && after.proxied === proxied && after.ttl === ttl, 'dnsUpdateDidNotConverge');
  report({ operation, after });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runNumeronIngress({ report: receipt => console.log(JSON.stringify(receipt)) })
    .catch(error => { console.error(error.message); process.exitCode = 1; });
}
