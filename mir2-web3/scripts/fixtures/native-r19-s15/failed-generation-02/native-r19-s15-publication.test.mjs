// Unit policy fixtures only. Real immutable R19/s15 feed/CMS bytes are read,
// but crypto.DigestStream, FixedLengthStream, R2 and fetch below are Node models.
// No CMS admission is injected. Positive handler tests explicitly skip until a
// genuine new receipt has been issued and reviewed anchors/literal are rebound.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash, webcrypto} from 'node:crypto';
import {readFileSync} from 'node:fs';
import definition from '../src/native-r19-s15-publication-plan.json' with {type: 'json'};
import preparedDefinition from '../../../../scripts/fixtures/native-r17-13/prepared-snapshot-01/infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-publication-plan.json' with {type: 'json'};
import {validateNativePlan as validatePreparedSnapshot, createNativeR17Handler as createPreparedSnapshotHandler} from '../../../../scripts/fixtures/native-r17-13/prepared-snapshot-01/infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-publication.mjs';
import {NATIVE_PLAN_SHA256, OPERATION_POLICY, STAGE_SCHEMA, validatePreparedPlan,
  validateNativePlan, stageEnvelopeTemplate, createNativeR19S15Handler} from '../src/native-r19-s15-publication.mjs';

globalThis.fetch = async () => {throw new Error('outbound network disabled in unit fixtures');};
const FIXTURES = new URL('../../../../scripts/fixtures/native-r19-s15/preview/', import.meta.url);
const FEED_BYTES = new Uint8Array(readFileSync(new URL('artifacts/feed/latest.json', FIXTURES)));
const CMS_BYTES = new Uint8Array(readFileSync(new URL('artifacts/feed/latest.p7s', FIXTURES)));
const FEED = JSON.parse(new TextDecoder().decode(FEED_BYTES));
const NOW = (FEED.createdUnix + 100) * 1000;
const MOCK_SECRET = 'public-fake-unit-secret-only';
const SKIP_PENDING_PROOF = definition.admission !== 'admitted'
  ? 'prepared/unadmitted: actual CMS/root receipt and reviewed anchors required' : false;
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const buffer = hash => Uint8Array.from(Buffer.from(hash, 'hex')).buffer;
const clone = value => JSON.parse(JSON.stringify(value));
function canonical(value) {
  const sort = v => Array.isArray(v) ? v.map(sort) : v && typeof v === 'object'
    ? Object.fromEntries(Object.keys(v).sort().map(k => [k, sort(v[k])])) : v;
  return new TextEncoder().encode(JSON.stringify(sort(value)) + '\n');
}
function request(route, body = {index: 0}, method = 'POST', auth = true) {
  return new Request('https://assets.mir2.obelisk.build/upload/native-r19-s15' + route, {
    method, headers: {authorization: auth ? 'Bearer ' + MOCK_SECRET : 'wrong',
      'content-type': 'application/json'},
    ...(method === 'GET' || method === 'HEAD' ? {} : {body: JSON.stringify(body)}),
  });
}
function metadata(entry, bytes) {
  return {key: entry.r2Key, size: entry.size, httpEtag: '"' + entry.sha256.slice(0, 24) + '"',
    httpMetadata: {contentType: entry.contentType, cacheControl: entry.cacheControl},
    customMetadata: {sha256: entry.sha256}, checksums: {sha256: buffer(entry.sha256)}, bytes};
}
function pointerEntry(value) {
  const bytes = canonical(value);
  return {entry: {r2Key: definition.r2Prefix + 'channels/invited.json',
    size: bytes.length, sha256: sha(bytes), contentType: 'application/json', cacheControl: 'no-store'}, bytes};
}
class BucketModel {
  records = new Map(); calls = []; mutateBeforePointerPut; losePointerAck = false;
  loseObjectAck = false; hangPut = false; consumedBytes = 0; maxChunk = 0;
  seed(entry, bytes) {this.records.set(entry.r2Key, metadata(entry, bytes));}
  seedClosure() {for (const entry of definition.objects) this.seed(entry,
    entry.index === 0 ? FEED_BYTES : entry.index === 1 ? CMS_BYTES : undefined);}
  pointer(value) {const {entry, bytes} = pointerEntry(value); this.seed(entry, bytes);}
  async head(key) {this.calls.push({op: 'head', key}); const r = this.records.get(key); return r ? {...r} : null;}
  async get(key) {
    this.calls.push({op: 'get', key}); const r = this.records.get(key);
    if (!r) return null;
    return {...r, body: new Response(r.bytes ?? new Uint8Array()).body};
  }
  async put(key, body, options) {
    this.calls.push({op: 'put', key, options});
    assert.ok(options.sha256 instanceof ArrayBuffer && options.sha256.byteLength === 32);
    if (key.endsWith('/channels/invited.json')) this.mutateBeforePointerPut?.();
    const existing = this.records.get(key);
    if (options.onlyIf.get('if-none-match') === '*') {if (existing) return null;}
    else {
      assert.ok(key.endsWith('/channels/invited.json'), 'immutable object must remain create-only');
      assert.equal(options.onlyIf.get('if-none-match'), null);
      assert.ok(options.onlyIf.get('if-match'));
      if (!existing || existing.httpEtag !== options.onlyIf.get('if-match')) return null;
    }
    if (this.hangPut) return new Promise(() => {});
    const hash = createHash('sha256'); let size = 0; const kept = [];
    const consume = chunk => {
      assert.ok(chunk instanceof Uint8Array); size += chunk.length;
      this.consumedBytes += chunk.length; this.maxChunk = Math.max(this.maxChunk, chunk.length);
      hash.update(chunk);
      if (size <= 4096) kept.push(chunk.slice());
    };
    if (body instanceof ReadableStream) {
      const reader = body.getReader();
      try {while (true) {const r = await reader.read(); if (r.done) break; consume(r.value);}}
      finally {reader.releaseLock();}
    } else consume(body);
    const actual = hash.digest('hex');
    if (actual !== Buffer.from(options.sha256).toString('hex')) throw new Error('modeled R2 checksum rejection');
    // Model backend conditional check at COMMIT as well as at invocation.
    const latest = this.records.get(key);
    if (options.onlyIf.get('if-none-match') === '*') {if (latest) return null;}
    else if (!latest || latest.httpEtag !== options.onlyIf.get('if-match')) return null;
    const bytes = size <= 4096 ? Buffer.concat(kept) : undefined;
    const record = {key, size, httpEtag: '"' + actual.slice(0, 24) + '"',
      httpMetadata: options.httpMetadata, customMetadata: options.customMetadata,
      checksums: {sha256: buffer(actual)}, bytes};
    this.records.set(key, record);
    if ((this.losePointerAck && key.endsWith('/channels/invited.json'))
      || (this.loseObjectAck && !key.endsWith('/channels/invited.json'))) {
      throw new Error('modeled committed write with lost acknowledgment');
    }
    return {...record};
  }
}
function streamModels(stats) {
  return {
    fixedLengthStream(size) {
      stats.fixedLengths.push(size); let count = 0;
      return new TransformStream({
        transform(chunk, controller) {count += chunk.length; if (count > size) throw new Error('overrun'); controller.enqueue(chunk);},
        flush() {if (count !== size) throw new Error('underrun');},
      });
    },
    digestStream() {
      const hash = createHash('sha256'); let resolve, reject;
      const digest = new Promise((ok, bad) => {resolve = ok; reject = bad;});
      const stream = new WritableStream({
        write(bytes) {hash.update(bytes);},
        close() {resolve(Uint8Array.from(hash.digest()).buffer);}, abort(error) {reject(error);},
      });
      stream.digest = digest; return stream;
    },
  };
}
function fixture({originOverride, policy, now = NOW} = {}) {
  const bucket = new BucketModel(), calls = [], stats = {fixedLengths: [], aborted: 0};
  const fetch = async (url, options) => {
    calls.push(url);
    assert.equal(options.method, 'GET'); assert.equal(options.redirect, 'manual');
    assert.deepEqual(options.headers, {'accept-encoding': 'identity', 'cache-control': 'no-store'});
    options.signal.addEventListener('abort', () => {stats.aborted++;}, {once: true});
    const immutable = definition.objects.find(e => definition.originBase + e.originRelativePath === url);
    const current = url === definition.originBase + 'latest.json' ? definition.objects[0]
      : url === definition.originBase + 'latest.p7s' ? definition.objects[1] : null;
    const entry = immutable ?? current;
    assert.ok(entry, 'fixed source URLs only');
    if (originOverride) return originOverride(entry, options, !!current);
    assert.ok(entry.index === 0 || entry.index === 1, 'no fabricated artifact source bodies');
    const response = new Response(entry.index === 0 ? FEED_BYTES : CMS_BYTES,
      {headers: {'content-length': String(entry.size)}});
    response.arrayBuffer = response.text = response.json = response.clone = () => {
      throw new Error('artifact buffering/cloning forbidden');
    };
    return response;
  };
  const handle = createNativeR19S15Handler({crypto: webcrypto, fetch, now: () => now,
    ...streamModels(stats), ...(policy ? {policy} : {})});
  return {bucket, calls, stats, handle, env: {MIR2_R2_UPLOAD_SECRET: MOCK_SECRET, MIR2_ASSETS: bucket}};
}
function policyTest(name, body) {test(name, {skip: SKIP_PENDING_PROOF}, body);}

test('fixed15 closure binds exact new R19 and genuine predecessor14', async () => {
  assert.equal(await validatePreparedPlan(definition, webcrypto), true);
  assert.equal(sha(canonical(definition)), NATIVE_PLAN_SHA256);
  assert.equal(definition.objects.length, 40);
  assert.equal(definition.objects.filter(e => e.path.startsWith('releases/game-')).length, 34);
  assert.equal(definition.candidate.sequence, 15);
  assert.equal(sha(FEED_BYTES), definition.objects[0].sha256);
  assert.equal(sha(CMS_BYTES), definition.objects[1].sha256);
  assert.equal(definition.preparation.requiredGameSourceRevision, '2b04e82c4f42b22f895f5f95b5bd79a8147eacdf');
  assert.equal(definition.objects.reduce((n, e) => n + e.size, 0), 1051831921);
});
test('default admission is closed; no fake passed/CMS or old receipt can admit13', async () => {
  await assert.rejects(validatePreparedSnapshot(preparedDefinition, webcrypto), /prepared_release_unadmitted/);
  for (const alteration of [
    p => {p.admission = 'prepared';},
    p => {p.rootVerificationBase64 = readFileSync(new URL('../../../../scripts/fixtures/native-r19-s15/predecessor14/ROOT-VERIFICATION.json', import.meta.url)).toString('base64');
      p.rootVerificationSha256 = p.predecessorProof.rootVerificationSha256;},
    p => {p.objects.at(-1).sha256 = '0'.repeat(64);},
  ]) {
    const other = clone(definition); alteration(other);
    await assert.rejects(validateNativePlan(other, webcrypto), /native_plan_hash_mismatch/);
  }
});
test('authentication precedes storage/fetch/admission and authorized prepared operation fails closed', async () => {
  const f = fixture();
  assert.equal((await f.handle(request('/import', {index: 0}, 'POST', false), f.env)).status, 401);
  assert.equal(f.calls.length, 0); assert.equal(f.bucket.calls.length, 0);
  {
    const handle = createPreparedSnapshotHandler({crypto: webcrypto,
      fetch: () => {throw new Error('Prepared admission must precede fetch');}});
    const denied = await handle(request('/import'), f.env);
    assert.equal(denied.status, 503); assert.equal((await denied.json()).error, 'prepared_release_unadmitted');
    assert.equal(f.calls.length, 0); assert.equal(f.bucket.calls.length, 0);
  }
});
test('stage template permits only null/exact12/exact13; old and forged stage scope stay distinct', () => {
  for (const value of [null, definition.knownPrevious, definition.candidate]) {
    assert.equal(stageEnvelopeTemplate(value).schema, STAGE_SCHEMA);
    assert.deepEqual(stageEnvelopeTemplate(value).expectedCurrent, value);
  }
  assert.throws(() => stageEnvelopeTemplate({...definition.knownPrevious, feedSha256: '0'.repeat(64)}),
    /unknown_observed_current/);
});
test('fixed111MB deadline policy is finite and transport margin exceeds worker budget', () => {
  const largest = Math.max(...definition.objects.map(e => e.size));
  assert.equal(largest, 111013376);
  assert.ok(largest / OPERATION_POLICY.slowStreamPolicyBytesPerSecond * 1000
    + OPERATION_POLICY.originHeaderMs + OPERATION_POLICY.r2SettleAllowanceMs < OPERATION_POLICY.importMs);
  assert.ok(OPERATION_POLICY.importControlSeconds * 1000 > OPERATION_POLICY.importMs);
  assert.ok(OPERATION_POLICY.promoteControlSeconds * 1000 > OPERATION_POLICY.promoteMs);
  assert.equal(OPERATION_POLICY.measuredSpeed, false);
  assert.equal(definition.preparation.actualDeadlinePolicyAccepted, false);
});

policyTest('first object import201 is create-only; actual small full-hash resume200 does not refetch', async () => {
  const f = fixture(); let response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 201); assert.equal((await response.json()).resumed, false);
  const writes = f.bucket.calls.filter(c => c.op === 'put');
  assert.equal(writes.length, 1); assert.equal(writes[0].options.onlyIf.get('if-none-match'), '*');
  assert.equal(f.bucket.consumedBytes, FEED_BYTES.length);
  response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 200); assert.equal((await response.json()).resumed, true);
  assert.equal(f.calls.length, 1);
});
policyTest('bad existing body or wrong stored checksum cannot count as immutable resume', async () => {
  const f = fixture(); f.bucket.seed(definition.objects[0], new Uint8Array(FEED_BYTES.length));
  const response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 409); assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 0);
  assert.equal(f.calls.length, 0);
});
policyTest('lost object ACK retains unknown and never retries; independent later stage hashes existing bytes', async () => {
  const f = fixture(); f.bucket.loseObjectAck = true;
  const response = await f.handle(request('/import'), f.env), result = await response.json();
  assert.equal(response.status, 502); assert.equal(result.objectWriteAttempted, true);
  assert.equal(result.objectOutcomeUnknown, true);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
  f.bucket.loseObjectAck = false;
  const reconciled = await f.handle(request('/import'), f.env);
  assert.equal(reconciled.status, 200); assert.equal((await reconciled.json()).resumed, true);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
});
policyTest('source redirect/encoding/length fail before any immutable write', async () => {
  for (const headers of [{status: 302}, {encoding: 'gzip'}, {length: FEED_BYTES.byteLength + 1}]) {
    const f = fixture({originOverride: () => new Response(FEED_BYTES, {
      status: headers.status ?? 200, headers: {'content-length': String(headers.length ?? FEED_BYTES.length),
        ...(headers.encoding ? {'content-encoding': headers.encoding} : {})}})});
    const response = await f.handle(request('/import'), f.env);
    assert.equal(response.status, 502); assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 0);
  }
});
policyTest('scaled header deadline cancels upstream and never changes pointer', async () => {
  const f = fixture({policy: {...OPERATION_POLICY, originHeaderMs: 20, noProgressMs: 80, importMs: 120},
    originOverride: () => new Promise(() => {})});
  const response = await f.handle(request('/import'), f.env), result = await response.json();
  assert.equal(response.status, 504); assert.equal(result.error, 'origin_header_deadline');
  assert.equal(result.objectWriteAttempted, false); assert.ok(f.stats.aborted > 0);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 0);
});
policyTest('scaled total timeout bounds a stalled R2 put; cancellation cannot claim it never committed', async () => {
  const f = fixture({policy: {...OPERATION_POLICY, importMs: 50, noProgressMs: 30,
    originHeaderMs: 20, r2SettleAllowanceMs: 30}});
  f.bucket.hangPut = true;
  const response = await f.handle(request('/import'), f.env), result = await response.json();
  assert.equal(response.status, 504); assert.equal(result.objectWriteAttempted, true);
  assert.equal(result.objectOutcomeUnknown, true); assert.ok(f.stats.aborted > 0);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
});
policyTest('null pointer creation uses only If-None-Match and reads exact13 after commit', async () => {
  const f = fixture(); f.bucket.seedClosure();
  const response = await f.handle(request('/promote', stageEnvelopeTemplate(null)), f.env);
  assert.equal(response.status, 200); const result = await response.json();
  assert.equal(result.pointerChanged, true); assert.equal(result.pointerOutcomeUnknown, false);
  const write = f.bucket.calls.find(c => c.op === 'put');
  assert.equal(write.options.onlyIf.get('if-none-match'), '*');
  assert.equal(write.options.onlyIf.get('if-match'), null);
  assert.deepEqual(f.calls, [definition.originBase + 'latest.json', definition.originBase + 'latest.p7s']);
});
policyTest('observed exact12 upgrade uses that actual ETag If-Match;13 is idempotent', async () => {
  const f = fixture(); f.bucket.seedClosure(); f.bucket.pointer(definition.knownPrevious);
  const {entry} = pointerEntry(definition.knownPrevious);
  const oldEtag = f.bucket.records.get(entry.r2Key).httpEtag;
  let response = await f.handle(request('/promote', stageEnvelopeTemplate(definition.knownPrevious)), f.env);
  assert.equal(response.status, 200);
  const writes = f.bucket.calls.filter(c => c.op === 'put');
  assert.equal(writes[0].options.onlyIf.get('if-match'), oldEtag);
  assert.equal(writes[0].options.onlyIf.get('if-none-match'), null);
  response = await f.handle(request('/promote', stageEnvelopeTemplate(definition.knownPrevious)), f.env);
  assert.equal(response.status, 200); assert.equal((await response.json()).alreadyPromoted, true);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
});
policyTest('unknown/conflicting pointer or changed actual stage state rejects before put', async () => {
  for (const actual of [{...definition.knownPrevious, sequence: 11}, definition.knownPrevious]) {
    const f = fixture(); f.bucket.seedClosure(); f.bucket.pointer(actual);
    const response = await f.handle(request('/promote', stageEnvelopeTemplate(null)), f.env);
    assert.equal(response.status, 409); assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 0);
  }
});
policyTest('pointer race gets412 and preserves raced pointer; no overwrite retry', async () => {
  const f = fixture(); f.bucket.seedClosure(); f.bucket.pointer(definition.knownPrevious);
  f.bucket.mutateBeforePointerPut = () => {
    f.bucket.mutateBeforePointerPut = undefined; f.bucket.pointer({...definition.knownPrevious, sequence: 99});
  };
  const response = await f.handle(request('/promote', stageEnvelopeTemplate(definition.knownPrevious)), f.env);
  assert.equal(response.status, 412); const result = await response.json();
  assert.equal(result.pointerAttempted, true); assert.equal(result.pointerOutcomeUnknown, false);
  const records = [...f.bucket.records.values()].find(r => r.key.endsWith('/channels/invited.json'));
  assert.equal(JSON.parse(new TextDecoder().decode(records.bytes)).sequence, 99);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
});
policyTest('lost pointer ACK retains unknown; separate retry reconciles exact13 without second write', async () => {
  const f = fixture(); f.bucket.seedClosure(); f.bucket.pointer(definition.knownPrevious); f.bucket.losePointerAck = true;
  let response = await f.handle(request('/promote', stageEnvelopeTemplate(definition.knownPrevious)), f.env);
  assert.equal(response.status, 502); const uncertain = await response.json();
  assert.equal(uncertain.pointerAttempted, true); assert.equal(uncertain.pointerOutcomeUnknown, true);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
  f.bucket.losePointerAck = false;
  response = await f.handle(request('/promote', stageEnvelopeTemplate(definition.knownPrevious)), f.env);
  assert.equal(response.status, 200); assert.equal((await response.json()).alreadyPromoted, true);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 1);
});
policyTest('immutable15 preview cannot promote while current origin latest pair remains14', async () => {
  const prior = JSON.parse(readFileSync(new URL('../../../../scripts/fixtures/native-r19-s15/predecessor14/PUBLICATION-PLAN.json', import.meta.url)));
  const oldFeed = readFileSync(new URL('../../../../scripts/fixtures/native-r19-s15/predecessor14/latest.json', import.meta.url));
  assert.equal(sha(oldFeed), definition.knownPrevious.feedSha256);
  const f = fixture({originOverride: (entry, _options, current) => new Response(current
    ? oldFeed : (entry.index === 0 ? FEED_BYTES : CMS_BYTES),
    {headers: {'content-length': String(entry.size)}})});
  f.bucket.seedClosure(); f.bucket.pointer(prior.candidate);
  const response = await f.handle(request('/promote', stageEnvelopeTemplate(definition.knownPrevious)), f.env);
  assert.equal(response.status, 409); assert.equal((await response.json()).error, 'origin_feed_pair_changed');
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 0);
  assert.deepEqual(f.calls, [definition.originBase + 'latest.json']);
});
policyTest('old v1 receipt/changed candidate is rejected before heads or source fetch', async () => {
  for (const change of [e => {e.schema = 'mir2.windows.r2-native-stage.v1';},
    e => {e.candidate.sequence = 14;}, e => {e.rootVerificationSha256 = definition.predecessorProof.rootVerificationSha256;}]) {
    const f = fixture(), envelope = clone(stageEnvelopeTemplate(null)); change(envelope);
    const response = await f.handle(request('/promote', envelope), f.env);
    assert.equal(response.status, 400); assert.equal(f.bucket.calls.length, 0); assert.equal(f.calls.length, 0);
  }
});
policyTest('caller cannot select arbitrary URL/body/hash/key or normalized/query escape', async () => {
  for (const [route, body] of [['/import', {index: 0, url: 'https://other.invalid'}],
    ['/import', {path: 'https://other.invalid'}], ['/import?arbitrary=1', {index: 0}]]) {
    const f = fixture(); const response = await f.handle(request(route, body), f.env);
    assert.equal(response.status, 400); assert.equal(f.bucket.calls.length, 0); assert.equal(f.calls.length, 0);
  }
});

policyTest('fresh wrong SHA body cannot publish even with correct declared length', async () => {
  const f = fixture({originOverride: entry => new Response(new Uint8Array(entry.size),
    {headers: {'content-length': String(entry.size)}})});
  const response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 502); assert.equal(f.bucket.records.has(definition.objects[0].r2Key), false);
  const result = await response.json(); assert.equal(result.pointerChanged, false);
  assert.equal(result.objectOutcomeUnknown, true); // Reconcile required, even for a modeled rejection.
});
policyTest('concurrent same immutable key creates exactly once and never overwrites', async () => {
  const f = fixture();
  const responses = await Promise.all([f.handle(request('/import'), f.env), f.handle(request('/import'), f.env)]);
  assert.deepEqual(responses.map(r => r.status).sort(), [201, 412]);
  assert.equal(f.bucket.records.size, 1);
  const stored = f.bucket.records.get(definition.objects[0].r2Key);
  assert.equal(sha(stored.bytes), definition.objects[0].sha256);
  assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 2);
  for (const call of f.bucket.calls.filter(c => c.op === 'put')) assert.equal(call.options.onlyIf.get('if-none-match'), '*');
});
policyTest('no-progress source cancels partial stream with unknown put and no pointer writes', async () => {
  let cancelled = false;
  const f = fixture({policy: {...OPERATION_POLICY, noProgressMs: 30, importMs: 120,
    originHeaderMs: 20, r2SettleAllowanceMs: 30},
    originOverride: entry => new Response(new ReadableStream({
      start(controller) {controller.enqueue(FEED_BYTES.slice(0, 16));},
      cancel() {cancelled = true;},
    }), {headers: {'content-length': String(entry.size)}})});
  const response = await f.handle(request('/import'), f.env), result = await response.json();
  assert.equal(response.status, 504); assert.equal(result.error, 'operation_no_progress');
  assert.equal(result.objectOutcomeUnknown, true); assert.equal(result.pointerAttempted, false);
  assert.ok(cancelled); assert.equal(f.bucket.records.size, 0);
});

// Retained original107 cases, adapted only fixed release/approved policy fixtures.
{
const {default: worker} = await import('../src/index.ts');
// No test may reach real HTTP or a credential. These are platform behavior
// models; native Workers/R2/CMS/network acceptance is deliberately separate.
globalThis.fetch = async () => {throw new Error('network disabled in offline fixtures');};

const MOCK_SECRET = 'not-a-real-credential-test-fixture';

for (const key of [
  'mir2/native/windows-invited/releases/protected.exe',
  '  ///mir2/native/windows-invited/channels/invited.json  ',
]) {
  test(`legacy PUT rejects reserved native prefix: ${key}`, async () => {
    const writes = [];
    const response = await worker.fetch(new Request(
      `https://assets.mir2.obelisk.build/upload?key=${encodeURIComponent(key)}`, {
        method: 'PUT', headers: {authorization: `Bearer ${MOCK_SECRET}`}, body: 'bad',
      }), {MIR2_R2_UPLOAD_SECRET: MOCK_SECRET,
        MIR2_ASSETS: {put: async (...args) => {writes.push(args); return {};}}});
    assert.equal(response.status, 403);
    assert.deepEqual(writes, []);
  });
}

test('unrelated legacy Web PUT retains its existing behavior', async () => {
  const writes = [];
  const response = await worker.fetch(new Request(
    'https://assets.mir2.obelisk.build/upload?key=mir2/v/web-fixture/a.txt', {
      method: 'PUT', headers: {authorization: `Bearer ${MOCK_SECRET}`, 'content-type': 'text/plain'}, body: 'ok',
    }), {MIR2_R2_UPLOAD_SECRET: MOCK_SECRET,
      MIR2_ASSETS: {put: async (...args) => {writes.push(args); return {};}}});
  assert.equal(response.status, 200);
  assert.equal(writes.length, 1);
  assert.equal(writes[0][0], 'mir2/v/web-fixture/a.txt');
  assert.equal(writes[0][2].onlyIf, undefined);
});

const feedBytes = FEED_BYTES;
const signatureBytes = CMS_BYTES;
const feed = JSON.parse(new TextDecoder().decode(feedBytes));
const NOW = (feed.createdUnix + 100) * 1000;
const bytesFor = entry => entry.index === 0 ? feedBytes : entry.index === 1 ? signatureBytes : undefined;
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const asBuffer = value => Uint8Array.from(Buffer.from(value, 'hex')).buffer;
const clone = value => JSON.parse(JSON.stringify(value));
const jsonHeaders = {authorization: `Bearer ${MOCK_SECRET}`, 'content-type': 'application/json'};
const request = (endpoint, method = 'POST', body = {index: 0}, headers = jsonHeaders) =>
  new Request('https://assets.mir2.obelisk.build/upload/native-r19-s15' + endpoint, {
    method, headers, ...(method === 'GET' || method === 'HEAD' ? {} : {body: JSON.stringify(body)}),
  });

function metadata(entry, bytes = bytesFor(entry)) {
  return {key: entry.r2Key, size: entry.size, httpEtag: '"' + entry.sha256.slice(0, 24) + '"',
    httpMetadata: {contentType: entry.contentType, cacheControl: entry.cacheControl},
    customMetadata: {sha256: entry.sha256}, checksums: {sha256: asBuffer(entry.sha256)}, bytes};
}
class BucketModel {
  records = new Map(); calls = []; forcedConflict = new Set(); bytesRead = 0;
  maxChunk = 0; throwOnPut; throwAfterPointerCommit; throwPointerReadAfterWrite = false;
  seed(entry, bytes) {this.records.set(entry.r2Key, metadata(entry, bytes));}
  seedClosure() {for (const entry of definition.objects) this.seed(entry);}
  async head(key) {
    this.calls.push({operation: 'head', key});
    const record = this.records.get(key); return record ? {...record} : null;
  }
  async get(key) {
    this.calls.push({operation: 'get', key});
    if (this.throwPointerReadAfterWrite && key.endsWith('/channels/invited.json') && this.records.has(key)) {
      throw new Error('injected private storage failure ' + MOCK_SECRET);
    }
    const record = this.records.get(key);
    if (!record) return null;
    const result = {...record, body: new Response(record.bytes ?? new Uint8Array()).body};
    result.text = result.arrayBuffer = () => {throw new Error('artifact buffering forbidden');};
    return result;
  }
  async put(key, body, options) {
    this.calls.push({operation: 'put', key, options});
    assert.equal(options.onlyIf?.get('if-none-match'), '*', 'every native write is create-only');
    assert.ok(options.sha256 instanceof ArrayBuffer && options.sha256.byteLength === 32,
      'native backend checksum is the unambiguous 32-byte SHA256');
    if (this.forcedConflict.has(key) || this.records.has(key)) return null;
    if (this.throwOnPut) throw new Error('injected private put failure ' + MOCK_SECRET);
    const sha = createHash('sha256'); let size = 0; const chunks = [];
    const consume = chunk => {
      size += chunk.byteLength; this.bytesRead += chunk.byteLength;
      this.maxChunk = Math.max(this.maxChunk, chunk.byteLength); sha.update(chunk);
      if (size <= 65536) chunks.push(chunk.slice());
    };
    if (body instanceof ReadableStream) {
      const reader = body.getReader();
      try {while (true) {const result = await reader.read(); if (result.done) break; consume(result.value);}}
      finally {reader.releaseLock();}
    } else consume(body);
    const actual = sha.digest('hex');
    if (actual !== Buffer.from(options.sha256).toString('hex')) throw new Error('backend checksum mismatch');
    const bytes = size <= 65536 ? new Uint8Array(size) : undefined;
    if (bytes) {let offset = 0; for (const chunk of chunks) {bytes.set(chunk, offset); offset += chunk.length;}}
    const record = {key, size, httpEtag: '"' + actual.slice(0, 24) + '"',
      httpMetadata: {...options.httpMetadata}, customMetadata: {...options.customMetadata},
      checksums: {sha256: asBuffer(actual)}, bytes};
    this.records.set(key, record);
    if (this.throwAfterPointerCommit && key.endsWith('/channels/invited.json')) {
      throw new Error('injected uncertain commit ' + MOCK_SECRET);
    }
    return {...record};
  }
}
function platformModels(stats) {
  return {
    fixedLengthStream(size) {
      stats.fixedSizes.push(size); let count = 0;
      return new TransformStream({
        transform(chunk, controller) {
          count += chunk.byteLength;
          if (count > size) throw new Error('FixedLengthStream overrun');
          controller.enqueue(chunk);
        },
        flush() {if (count !== size) throw new Error('FixedLengthStream underrun');},
      });
    },
    digestStream() {
      const sha = createHash('sha256'); let resolve, reject;
      const digest = new Promise((ok, error) => {resolve = ok; reject = error;});
      const stream = new WritableStream({
        write(bytes) {stats.digestBytes += bytes.byteLength; sha.update(bytes);},
        close() {resolve(Uint8Array.from(sha.digest()).buffer);},
        abort(error) {reject(error);},
      });
      stream.digest = digest; return stream;
    },
  };
}
function fixture({originOverride, now = NOW, fixedLengthStream} = {}) {
  const bucket = new BucketModel(); const calls = [];
  const stats = {fixedSizes: [], digestBytes: 0};
  const fetch = async (url, options) => {
    calls.push({url, options});
    const entry = definition.objects.find(item => definition.originBase + item.originRelativePath === url)
      ?? (url === definition.originBase + 'latest.json' ? definition.objects[0]
        : url === definition.originBase + 'latest.p7s' ? definition.objects[1] : undefined);
    assert.ok(entry, 'only a literal fixed-origin object may be fetched');
    assert.equal(options.method, 'GET'); assert.equal(options.redirect, 'manual');
    assert.deepEqual(options.headers, {'accept-encoding': 'identity', 'cache-control': 'no-store'});
    assert.equal(options.headers.authorization, undefined); assert.equal(options.headers.cookie, undefined);
    if (originOverride) return originOverride(entry, options);
    assert.ok(bytesFor(entry), 'this offline fixture has actual small source bytes only');
    const response = new Response(bytesFor(entry), {headers: {'content-length': String(entry.size)}});
    response.arrayBuffer = response.text = response.json = response.clone = () => {
      throw new Error('buffering/cloning origin artifact forbidden');
    };
    return response;
  };
  const models = platformModels(stats);
  const handle = createNativeR19S15Handler({crypto: webcrypto, fetch, now: () => now,
    ...models, ...(fixedLengthStream ? {fixedLengthStream} : {})});
  return {bucket, calls, stats, handle,
    env: {MIR2_R2_UPLOAD_SECRET: MOCK_SECRET, MIR2_ASSETS: bucket}};
}

test('source proof and actual small feed/signature match literal closure', async () => {
  assert.equal(hash(feedBytes), definition.objects[0].sha256);
  assert.equal(hash(signatureBytes), definition.objects[1].sha256);
  assert.equal(await validateNativePlan(definition, webcrypto), true);
  assert.equal(definition.objects.length, 40);
  assert.equal(definition.objects.filter(entry => entry.path.startsWith('installers/'))[0].originRelativePath,
    definition.objects[2].path);
});

for (const change of ['candidate', 'object', 'root', 'origin', 'installer']) {
  test(`changed frozen plan cannot redefine authority: ${change}`, async () => {
    const changed = clone(definition);
    if (change === 'candidate') changed.candidate.sequence++;
    if (change === 'object') changed.objects[0].sha256 = '0'.repeat(64);
    if (change === 'root') changed.rootVerificationBase64 = Buffer.from('{"cmsVerified":true}').toString('base64');
    if (change === 'origin') changed.originBase = 'https://attacker.invalid/';
    if (change === 'installer') changed.objects[2].originRelativePath = 'latest.json';
    await assert.rejects(validateNativePlan(changed, webcrypto), /native_plan_hash_mismatch/);
  });
}

for (const endpoint of ['/import', '/promote', '/objects/0', '/pointer']) {
  test(`unauthorized request precedes every fetch/storage operation: ${endpoint}`, async () => {
    const f = fixture(); const method = endpoint.startsWith('/objects') || endpoint === '/pointer' ? 'GET' : 'POST';
    const response = await f.handle(request(endpoint, method, {}, {'content-type': 'application/json'}), f.env);
    assert.equal(response.status, 401); assert.equal(f.calls.length, 0); assert.equal(f.bucket.calls.length, 0);
  });
}
test('index dispatch reaches native authorization before old PUT-only gate', async () => {
  const response = await worker.fetch(request('/import', 'POST', {}, {'content-type': 'application/json'}), {});
  assert.equal(response.status, 401);
});

for (const input of [
  {index: 0, url: 'https://attacker.invalid/'}, {index: 0, sha256: '0'.repeat(64)},
  {index: 0, key: 'mir2/native/windows-invited/channels/invited.json'}, {index: 0, path: definition.objects[0].path},
  {index: -1}, {index: definition.objects.length}, {index: 0.1}, {index: '0'}, {path: 'latest.json'},
  {path: '../channels/invited.json'}, {path: 'https://attacker.invalid/'}, [], null,
]) {
  test(`import rejects caller authority input: ${JSON.stringify(input)}`, async () => {
    const f = fixture(); const response = await f.handle(request('/import', 'POST', input), f.env);
    assert.equal(response.status, 400); assert.equal(f.calls.length, 0); assert.equal(f.bucket.calls.length, 0);
  });
}
for (const endpoint of ['/import?anything=1', '/import?', '/objects/00', '/objects/' + definition.objects.length,
  '/objects/%30', '/objects/0/', '/arbitrary', '/pointer/']) {
  test(`native path/query is literal only: ${endpoint}`, async () => {
    const f = fixture(); const response = await f.handle(request(endpoint, 'GET'), f.env);
    assert.ok([400, 404, 405].includes(response.status));
    assert.equal(f.calls.length, 0); assert.equal(f.bucket.calls.length, 0);
  });
}
for (const body of ['{"index":0,"index":1}', '{"index":0,"\\u0069ndex":1}',
  '{"index":0} trailing', '\ufeff{"index":0}', '{"index":' + '['.repeat(15) + '0' + ']'.repeat(15) + '}']) {
  test(`strict request JSON rejects ambiguity/complexity: ${body.slice(0, 45)}`, async () => {
    const f = fixture(); const response = await f.handle(new Request(
      'https://assets.mir2.obelisk.build/upload/native-r19-s15/import', {method: 'POST', headers: jsonHeaders, body}), f.env);
    assert.equal(response.status, 400); assert.equal(f.bucket.calls.length, 0); assert.equal(f.calls.length, 0);
  });
}
test('actual request stream and declared length are independently bounded', async () => {
  for (const [body, headers] of [
    [' '.repeat(1025), jsonHeaders], ['{"index":0}', {...jsonHeaders, 'content-length': '1025'}],
    ['{"index":0}', {...jsonHeaders, 'content-length': '1'}],
    ['{"index":0}', {...jsonHeaders, 'content-encoding': 'gzip'}],
    ['{"index":0}', {authorization: `Bearer ${MOCK_SECRET}`, 'content-type': 'text/plain'}],
  ]) {
    const f = fixture(); const response = await f.handle(new Request(
      'https://assets.mir2.obelisk.build/upload/native-r19-s15/import', {method: 'POST', headers, body}), f.env);
    assert.ok([400, 413, 415].includes(response.status)); assert.equal(f.bucket.calls.length, 0);
  }
});

for (const input of [{index: 0}, {path: definition.objects[0].path}, {index: 1}]) {
  test(`actual signed small object streams with checksum/create-only and read-after-write: ${JSON.stringify(input)}`, async () => {
    const f = fixture(); const response = await f.handle(request('/import', 'POST', input), f.env);
    assert.equal(response.status, 201); const body = await response.json();
    assert.equal(body.resumed, false); assert.equal(body.pointerChanged, false);
    const entry = definition.objects[body.index]; const write = f.bucket.calls.find(call => call.operation === 'put');
    assert.equal(write.key, entry.r2Key); assert.equal(Buffer.from(write.options.sha256).toString('hex'), entry.sha256);
    assert.equal(write.options.onlyIf.get('if-none-match'), '*');
    assert.deepEqual(write.options.httpMetadata, {contentType: entry.contentType, cacheControl: entry.cacheControl});
    assert.deepEqual(write.options.customMetadata, {sha256: entry.sha256});
    assert.equal(f.bucket.calls.at(-1).operation, 'head'); assert.equal(f.calls.length, 1);
    assert.deepEqual(f.stats.fixedSizes, [entry.size]); assert.equal(f.bucket.records.size, 1);
  });
}
test('exact existing object resumes only after full incremental hash and metadata', async () => {
  const f = fixture(); f.bucket.seed(definition.objects[0]);
  const response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 200); assert.equal((await response.json()).resumed, true);
  assert.equal(f.stats.digestBytes, feedBytes.length); assert.equal(f.calls.length, 0);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 0);
});
for (const change of ['body', 'short', 'extra', 'size', 'etag', 'weak-etag', 'custom-sha',
  'stored-sha', 'missing-checksum', 'mime', 'cache', 'encoding', 'key']) {
  test(`existing immutable mismatch cannot resume/overwrite: ${change}`, async () => {
    const f = fixture(); const entry = definition.objects[0]; f.bucket.seed(entry);
    const record = f.bucket.records.get(entry.r2Key);
    if (change === 'body') {record.bytes = feedBytes.slice(); record.bytes[0] ^= 1;}
    if (change === 'short') record.bytes = feedBytes.slice(1);
    if (change === 'extra') record.bytes = new Uint8Array([...feedBytes, 0]);
    if (change === 'size') record.size--;
    if (change === 'etag') record.httpEtag = 'unquoted';
    if (change === 'weak-etag') record.httpEtag = 'W/"weak"';
    if (change === 'custom-sha') record.customMetadata.sha256 = '0'.repeat(64);
    if (change === 'stored-sha') record.checksums.sha256 = asBuffer('0'.repeat(64));
    if (change === 'missing-checksum') delete record.checksums.sha256;
    if (change === 'mime') record.httpMetadata.contentType = 'text/html';
    if (change === 'cache') record.httpMetadata.cacheControl = 'no-store';
    if (change === 'encoding') record.httpMetadata.contentEncoding = 'gzip';
    if (change === 'key') record.key = 'mir2/other';
    const response = await f.handle(request('/import'), f.env);
    assert.equal(response.status, 409); assert.equal(f.calls.length, 0);
    assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 0);
  });
}

for (const failure of ['301', '302', '307', '206', '404', 'gzip', 'missing-length',
  'wrong-length', 'noncanonical-length', 'short-stream', 'extra-stream', 'wrong-sha']) {
  test(`origin contract fails closed: ${failure}`, async () => {
    const f = fixture({originOverride(entry) {
      let bytes = feedBytes.slice(); const headers = {'content-length': String(entry.size)};
      let status = 200;
      if (/^\d+$/.test(failure)) {status = Number(failure); headers.location = 'https://attacker.invalid/';}
      if (failure === 'gzip') headers['content-encoding'] = 'gzip';
      if (failure === 'missing-length') delete headers['content-length'];
      if (failure === 'wrong-length') headers['content-length'] = String(entry.size + 1);
      if (failure === 'noncanonical-length') headers['content-length'] = '0' + entry.size;
      if (failure === 'short-stream') bytes = bytes.slice(1);
      if (failure === 'extra-stream') bytes = new Uint8Array([...bytes, 0]);
      if (failure === 'wrong-sha') bytes[0] ^= 1;
      return new Response(bytes, {status, headers});
    }});
    const response = await f.handle(request('/import'), f.env);
    assert.equal(response.status, 502); assert.equal(f.bucket.records.size, 0);
    assert.equal((await response.json()).pointerChanged, false);
    assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length,
      ['short-stream', 'extra-stream', 'wrong-sha'].includes(failure) ? 1 : 0);
  });
}
test('early create race cancels blocked source and never retries unconditionally', {timeout: 3000}, async () => {
  let cancelled = false;
  let observedCancel;
  const cancellation = new Promise(resolve => {observedCancel = resolve;});
  const f = fixture({originOverride(entry) {
    let emitted = false;
    return new Response(new ReadableStream({
      pull(controller) {if (!emitted) {emitted = true; controller.enqueue(new Uint8Array([0]));}},
      cancel() {cancelled = true; observedCancel();},
    }), {headers: {'content-length': String(entry.size)}});
  }});
  f.bucket.forcedConflict.add(definition.objects[0].r2Key);
  const response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 412);
  // Bounded cleanup must not delay the response on an unsettled storage call.
  // Observe the source cancellation independently, with a finite test deadline.
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('source cancellation deadline')), 250);
    cancellation.then(() => {clearTimeout(timeout); resolve();}, reject);
  });
  assert.ok(cancelled);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 1);
  assert.equal(f.bucket.records.size, 0);
});
test('runtime without FixedLengthStream refuses before origin request', async () => {
  const f = fixture({fixedLengthStream() {throw new Error('not present');}});
  const response = await f.handle(request('/import'), f.env);
  assert.equal(response.status, 503); assert.equal(f.calls.length, 0);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 0);
});
test('actual game executable artifact is consumed in 64KiB chunks, checksum rejects synthetic bytes without commit', async () => {
  const entry = definition.objects.find(item => item.path.endsWith('/mir2-platform-windows.exe'));
  let largestSourceChunk = 0;
  const f = fixture({originOverride(actual) {
    assert.equal(actual.index, entry.index); let left = actual.size;
    const stream = new ReadableStream({pull(controller) {
      if (!left) return controller.close();
      const size = Math.min(65536, left); largestSourceChunk = Math.max(largestSourceChunk, size);
      left -= size; controller.enqueue(new Uint8Array(size));
    }});
    return new Response(stream, {headers: {'content-length': String(actual.size)}});
  }});
  const response = await f.handle(request('/import', 'POST', {index: entry.index}), f.env);
  assert.equal(response.status, 502); assert.equal(f.bucket.bytesRead, entry.size);
  assert.equal(f.bucket.maxChunk, 65536); assert.equal(largestSourceChunk, 65536);
  assert.deepEqual(f.stats.fixedSizes, [entry.size]); assert.equal(f.bucket.records.size, 0);
});

test('authenticated native GET/HEAD are limited, byte exact and no-store', async () => {
  const f = fixture(); f.bucket.seed(definition.objects[0]);
  for (const method of ['GET', 'HEAD']) {
    const response = await f.handle(request('/objects/0', method), f.env);
    assert.equal(response.status, 200); assert.equal(response.headers.get('cache-control'), 'no-store');
    assert.equal(response.headers.get('x-mir2-native-plan-sha256'), NATIVE_PLAN_SHA256);
    assert.equal(response.headers.get('x-mir2-stored-sha256'), hash(feedBytes));
    if (method === 'GET') assert.equal(hash(new Uint8Array(await response.arrayBuffer())), hash(feedBytes));
    else assert.equal(response.body, null);
  }
  const missing = await f.handle(request('/objects/1', 'HEAD'), f.env);
  assert.equal(missing.status, 404); assert.equal(missing.headers.get('cache-control'), 'no-store');
});
test('GET never serves an object whose stored checksum was changed', async () => {
  const f = fixture(); f.bucket.seed(definition.objects[0]);
  f.bucket.records.get(definition.objects[0].r2Key).checksums.sha256 = asBuffer('0'.repeat(64));
  const response = await f.handle(request('/objects/0', 'GET'), f.env);
  assert.equal(response.status, 409); assert.equal(f.calls.length, 0);
});

for (const change of ['missing', 'not-passed', 'mode', 'plan', 'root', 'table', 'closure',
  'candidate', 'expected-current', 'pointer-changed', 'public-false', 'public-host',
  'missing-object', 'extra-object', 'altered-size', 'reordered', 'extra-field']) {
  test(`promote rejects incomplete/forged bound stage: ${change}`, async () => {
    const f = fixture(); f.bucket.seedClosure(); const stage = stageEnvelopeTemplate();
    if (change === 'missing') delete stage.verifiedObjects;
    if (change === 'not-passed') stage.passed = false;
    if (change === 'mode') stage.mode = 'promote';
    if (change === 'plan') stage.planSha256 = '0'.repeat(64);
    if (change === 'root') stage.rootVerificationSha256 = '0'.repeat(64);
    if (change === 'table') stage.nativePlanSha256 = '0'.repeat(64);
    if (change === 'closure') stage.objectsSha256 = '0'.repeat(64);
    if (change === 'candidate') stage.candidate.sequence++;
    if (change === 'expected-current') stage.expectedCurrent = {...clone(stage.candidate), feedSha256: '0'.repeat(64)};
    if (change === 'pointer-changed') stage.pointerChanged = true;
    if (change === 'public-false') stage.publicVerified = false;
    if (change === 'public-host') stage.publicBase = 'https://attacker.invalid/';
    if (change === 'missing-object') stage.verifiedObjects.pop();
    if (change === 'extra-object') stage.verifiedObjects.push(stage.verifiedObjects[0]);
    if (change === 'altered-size') stage.verifiedObjects[0].size--;
    if (change === 'reordered') stage.verifiedObjects.reverse();
    if (change === 'extra-field') stage.secret = 'must not be reflected';
    const response = await f.handle(request('/promote', 'POST', stage), f.env);
    assert.equal(response.status, 400); assert.equal(f.bucket.calls.length, 0); assert.equal(f.calls.length, 0);
  });
}
for (const change of ['installer-missing', 'ordinary-missing', 'stored-sha', 'cache']) {
  test(`complete stage cannot bypass actual R2 metadata: ${change}`, async () => {
    const f = fixture(); f.bucket.seedClosure();
    const entry = change === 'installer-missing' ? definition.objects[2] : definition.objects.at(-1);
    if (change.endsWith('missing')) f.bucket.records.delete(entry.r2Key);
    if (change === 'stored-sha') f.bucket.records.get(entry.r2Key).checksums.sha256 = asBuffer('0'.repeat(64));
    if (change === 'cache') f.bucket.records.get(entry.r2Key).httpMetadata.cacheControl = 'no-store';
    const response = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
    assert.equal(response.status, 409); assert.equal(f.calls.length, 0);
    assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 0);
  });
}
for (const change of ['json', 'signature', 'expired', 'not-created']) {
  test(`promotion rereads exact unexpired origin pair: ${change}`, async () => {
    const f = fixture({now: change === 'expired' ? feed.expiresUnix * 1000
      : change === 'not-created' ? (feed.createdUnix - 1) * 1000 : NOW,
    originOverride(entry) {
      const bytes = bytesFor(entry).slice();
      if ((change === 'json' && entry.index === 0) || (change === 'signature' && entry.index === 1)) bytes[0] ^= 1;
      return new Response(bytes, {headers: {'content-length': String(entry.size)}});
    }});
    f.bucket.seedClosure();
    const response = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
    assert.equal(response.status, 409); assert.equal(f.bucket.calls.filter(call => call.operation === 'head').length, definition.objects.length);
    assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 0);
  });
}
test('only after all fixed object checks and source pair does pointer create; exact retry never rewrites', async () => {
  const f = fixture(); f.bucket.seedClosure();
  const first = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
  assert.equal(first.status, 200); const result = await first.json();
  assert.equal(result.pointerAttempted, true); assert.equal(result.pointerChanged, true);
  assert.equal(result.pointerOutcomeUnknown, false); assert.equal(result.aliasesVerified, false);
  assert.equal(result.publicVerificationRequired, true);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'head').length, definition.objects.length);
  assert.equal(f.calls.length, 2);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 1);
  const second = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
  assert.equal(second.status, 200); const repeated = await second.json();
  assert.equal(repeated.alreadyPromoted, true); assert.equal(repeated.pointerChanged, false);
  assert.equal(repeated.pointerAttempted, false);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 1);
  const pointer = await f.handle(request('/pointer', 'GET'), f.env);
  assert.equal(pointer.status, 200); assert.deepEqual(await pointer.json(), definition.candidate);
  const head = await f.handle(request('/pointer', 'HEAD'), f.env);
  assert.equal(head.status, 200); assert.equal(head.body, null);
  assert.equal(head.headers.get('cache-control'), 'no-store');
});
test('current unknown pointer is refused, never overwritten', async () => {
  const f = fixture(); f.bucket.seedClosure();
  const unknown = new TextEncoder().encode('{"unexpected":true}\n');
  const key = definition.r2Prefix + 'channels/invited.json';
  f.bucket.records.set(key, metadata({r2Key: key, size: unknown.length, sha256: hash(unknown),
    contentType: 'application/json', cacheControl: 'no-store'}, unknown));
  const response = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
  assert.equal(response.status, 409); assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 0);
});
test('pointer CAS race stops with 412 and a single conditional attempt', async () => {
  const f = fixture(); f.bucket.seedClosure();
  f.bucket.forcedConflict.add(definition.r2Prefix + 'channels/invited.json');
  const response = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
  assert.equal(response.status, 412); const body = await response.json();
  assert.equal(body.pointerAttempted, true); assert.equal(body.pointerChanged, false);
  assert.equal(body.pointerOutcomeUnknown, false);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 1);
});
for (const change of ['throw-before-put', 'throw-after-commit', 'read-after-write']) {
  test(`uncertain pointer outcome survives sanitized failure: ${change}`, async () => {
    const f = fixture(); f.bucket.seedClosure();
    f.bucket.throwOnPut = change === 'throw-before-put';
    f.bucket.throwAfterPointerCommit = change === 'throw-after-commit';
    f.bucket.throwPointerReadAfterWrite = change === 'read-after-write';
    const response = await f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env);
    assert.equal(response.status, 502); const text = await response.text(); assert.ok(!text.includes(MOCK_SECRET));
    const result = JSON.parse(text); assert.equal(result.pointerAttempted, true);
    assert.equal(result.pointerOutcomeUnknown, true);
    assert.equal(result.pointerChanged, change === 'read-after-write');
    assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 1);
    assert.equal(f.bucket.records.has(definition.r2Prefix + 'channels/invited.json'), change !== 'throw-before-put');
  });
}
test('concurrent prepared promotions have one winner and no unconditional overwrite', async () => {
  const f = fixture(); f.bucket.seedClosure();
  const responses = await Promise.all([f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env),
    f.handle(request('/promote', 'POST', stageEnvelopeTemplate()), f.env)]);
  assert.deepEqual(responses.map(response => response.status).sort(), [200, 412]);
  assert.equal(f.bucket.records.size, 37);
  assert.equal(f.bucket.calls.filter(call => call.operation === 'put').length, 2);
});
}


for (const authority of ['wrong.invalid', 'assets.mir2.obelisk.build:444']) {
  for (const route of ['/import', '/promote', '/objects/0', '/pointer']) {
    test('exact authority rejects authenticated wronghost/port before side effects: ' + authority + route, async () => {
      const f = fixture(); f.bucket.seedClosure();
      const read = route === '/objects/0' || route === '/pointer';
      const req = new Request('https://' + authority + '/upload/native-r19-s15' + route, {
        method: read ? 'GET' : 'POST', headers: {'authorization': 'Bearer ' + MOCK_SECRET,
          'content-type': 'application/json'},
        ...(read ? {} : {body: JSON.stringify(route === '/promote' ? stageEnvelopeTemplate(null) : {index: 0})}),
      });
      const response = await f.handle(req, f.env);
      if (response.body) await response.body.cancel(); // Release an unexpectedly accepted GET before assertion.
      assert.equal(response.status, 400); assert.equal(f.calls.length, 0); assert.equal(f.bucket.calls.length, 0);
    });
  }
}
test('late read-only R2 get resolution cancels orphaned body after operation deadline', async () => {
  let cancelled = false;
  const f = fixture({policy: {...OPERATION_POLICY, importMs: 20, noProgressMs: 100}});
  f.bucket.get = () => new Promise(resolve => setTimeout(() => resolve({...metadata(definition.objects[0], FEED_BYTES),
    body: new ReadableStream({cancel() {cancelled = true;}})}), 40));
  const response = await f.handle(request('/objects/0', null, 'GET'), f.env);
  assert.equal(response.status, 504);
  await new Promise(resolve => setTimeout(resolve, 50));
  assert.equal(cancelled, true); assert.equal(f.bucket.calls.filter(c => c.op === 'put').length, 0);
});
