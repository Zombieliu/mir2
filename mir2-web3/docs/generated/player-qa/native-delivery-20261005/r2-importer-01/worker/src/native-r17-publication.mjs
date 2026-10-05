import definition from './native-r17-publication-plan.json' with {type: 'json'};

// A root-reviewed immutable plan/attestation, not new CMS verification or a
// generic uploader. Changing any field requires a separately reviewed source.
export const NATIVE_PLAN_SHA256 = '1786ca53c2ef03e142419feefa3184070bfacd570c1b3a9495f79e3c50eb3929';
export const STAGE_SCHEMA = 'mir2.windows.r2-native-stage.v1';
const PREFIX = '/upload/native-r17';
const POINTER = 'channels/invited.json';
const MAX_POINTER = 4096;
const MAX_IMPORT_REQUEST = 1024;
const MAX_STAGE_REQUEST = 65536;
const ORIGIN_TIMEOUT_MS = 120000;
const ENCODER = new TextEncoder();
const SHA_PATTERN = /^[0-9a-f]{64}$/;
const ETAG_PATTERN = /^"[^"\x00-\x20\x7f]+"$/;

function freeze(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}
freeze(definition);

class Fault extends Error {
  constructor(code, status = 400) { super(code); this.code = code; this.status = status; }
}
function requireValue(condition, code, status = 400) {
  if (!condition) throw new Fault(code, status);
}
function canonical(value) {
  function sort(item) {
    if (Array.isArray(item)) return item.map(sort);
    if (item && typeof item === 'object') {
      return Object.fromEntries(Object.keys(item).sort().map(key => [key, sort(item[key])]));
    }
    return item;
  }
  return JSON.stringify(sort(value)) + '\n';
}
function same(a, b) { return canonical(a) === canonical(b); }
function exactKeys(value, fields, code) {
  requireValue(value && typeof value === 'object' && !Array.isArray(value)
    && same(Object.keys(value).sort(), [...fields].sort()), code);
}
function hex(buffer) {
  requireValue(buffer instanceof ArrayBuffer || ArrayBuffer.isView(buffer), 'invalid_stored_checksum', 409);
  const bytes = buffer instanceof ArrayBuffer ? new Uint8Array(buffer)
    : new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  return [...bytes].map(byte => byte.toString(16).padStart(2, '0')).join('');
}
async function digest(bytes, cryptoApi) {
  return hex(await cryptoApi.subtle.digest('SHA-256', bytes));
}
function checksumBytes(value) {
  requireValue(SHA_PATTERN.test(value), 'invalid_literal_checksum', 503);
  return Uint8Array.from(value.match(/../g), pair => Number.parseInt(pair, 16)).buffer;
}
function decodeBase64(value) {
  requireValue(typeof value === 'string' && /^[A-Za-z0-9+/]*={0,2}$/.test(value), 'invalid_frozen_proof', 503);
  return Uint8Array.from(atob(value), char => char.charCodeAt(0));
}
function closure(objects = definition.objects) {
  return objects.map(({path, size, sha256}) => ({path, size, sha256}));
}

// All original source paths remain inert proof strings. No fs/URL argument is
// taken from them. Digest anchors authenticate the already-reviewed attestation.
export async function validateNativePlan(candidate = definition, cryptoApi = globalThis.crypto) {
  requireValue(await digest(ENCODER.encode(canonical(candidate)), cryptoApi) === NATIVE_PLAN_SHA256,
    'native_plan_hash_mismatch', 503);
  const originalBytes = decodeBase64(candidate.sourcePlanBase64);
  const rootBytes = decodeBase64(candidate.rootVerificationBase64);
  requireValue(await digest(originalBytes, cryptoApi) === candidate.sourcePlanSha256
    && await digest(rootBytes, cryptoApi) === candidate.rootVerificationSha256,
  'frozen_proof_hash_mismatch', 503);
  const original = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(originalBytes));
  const root = JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(rootBytes));
  requireValue(original.schema === 'mir2.windows.r2-publication.v1'
    && original.expectedCurrent === null && candidate.expectedCurrent === null
    && same(original.candidate, candidate.candidate)
    && original.verificationReceipt.sha256 === candidate.rootVerificationSha256,
  'frozen_plan_binding_mismatch', 503);
  const originalClosure = closure([...original.objects].sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  requireValue(candidate.objects.length === 36 && same(originalClosure, closure(candidate.objects))
    && await digest(ENCODER.encode(canonical(closure(candidate.objects))), cryptoApi) === candidate.objectsSha256
    && root.schema === 'mir2.windows.r2-verification.v1' && root.passed === true
    && root.candidateVerified === true && root.cmsVerified === true
    && root.objectsSha256 === candidate.objectsSha256
    && ['sourceRevision', 'sequence', 'feedSha256', 'signatureSha256']
      .every(field => root[field] === candidate.candidate[field]),
  'root_attestation_binding_mismatch', 503);
  for (const [index, entry] of candidate.objects.entries()) {
    requireValue(entry.index === index && Number.isSafeInteger(entry.size)
      && entry.size > 0 && entry.size <= 128 * 1024 * 1024 && SHA_PATTERN.test(entry.sha256)
      && entry.r2Key === candidate.r2Prefix + entry.path
      && /^[A-Za-z0-9._/-]+$/.test(entry.path) && !entry.path.includes('..')
      && /^[A-Za-z0-9._/-]+$/.test(entry.originRelativePath)
      && !entry.originRelativePath.includes('..') && !entry.originRelativePath.startsWith('/'),
    'invalid_literal_object', 503);
  }
  return true;
}

// Bounded strict JSON rejects duplicate keys, including escaped aliases; depth
// and token limits avoid attacker-controlled recursion before schema checks.
function parseJson(bytes) {
  let text;
  try { text = new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(bytes); }
  catch { throw new Fault('invalid_json'); }
  let offset = 0, tokens = 0;
  const whitespace = () => { while (/^[\t\r\n ]$/.test(text[offset] ?? '')) offset++; };
  function string() {
    const match = /^"(?:[^"\\\x00-\x1f]|\\(?:["\\/bfnrt]|u[0-9a-fA-F]{4}))*"/.exec(text.slice(offset));
    requireValue(match, 'invalid_json'); offset += match[0].length; return JSON.parse(match[0]);
  }
  function value(depth = 0) {
    requireValue(depth <= 12 && ++tokens <= 2048, 'json_complexity_limit'); whitespace();
    const character = text[offset];
    if (character === '"') return string();
    if (character === '{') {
      offset++; whitespace(); const result = {}, keys = new Set();
      if (text[offset] === '}') {offset++; return result;}
      while (true) {
        whitespace(); const key = string();
        requireValue(!keys.has(key), 'duplicate_json_key'); keys.add(key); whitespace();
        requireValue(text[offset++] === ':', 'invalid_json');
        Object.defineProperty(result, key, {value: value(depth + 1), enumerable: true});
        whitespace(); const delimiter = text[offset++];
        if (delimiter === '}') return result;
        requireValue(delimiter === ',', 'invalid_json');
      }
    }
    if (character === '[') {
      offset++; whitespace(); const result = [];
      if (text[offset] === ']') {offset++; return result;}
      while (true) {
        result.push(value(depth + 1)); whitespace(); const delimiter = text[offset++];
        if (delimiter === ']') return result;
        requireValue(delimiter === ',', 'invalid_json');
      }
    }
    const match = /^(?:true|false|null|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(text.slice(offset));
    requireValue(match, 'invalid_json'); offset += match[0].length; return JSON.parse(match[0]);
  }
  const result = value(); whitespace(); requireValue(offset === text.length, 'invalid_json'); return result;
}

async function cancelBody(body) {
  if (body) { try { await body.cancel(); } catch {} }
}
async function boundedBytes(body, limit, expectedSize = undefined) {
  requireValue(body && typeof body.getReader === 'function', 'missing_body');
  const reader = body.getReader(); const chunks = []; let size = 0;
  try {
    while (true) {
      const {done, value} = await reader.read(); if (done) break;
      requireValue(value instanceof Uint8Array, 'invalid_byte_stream');
      size += value.byteLength; requireValue(size <= limit, 'body_size_limit', 413); chunks.push(value);
    }
    requireValue(expectedSize === undefined || size === expectedSize, 'body_size_mismatch');
    const bytes = new Uint8Array(size); let offset = 0;
    for (const chunk of chunks) {bytes.set(chunk, offset); offset += chunk.byteLength;}
    return bytes;
  } catch (error) { try { await reader.cancel(); } catch {} throw error; }
  finally { reader.releaseLock(); }
}
async function requestJson(request, limit) {
  requireValue(request.headers.get('content-type')?.split(';')[0].trim().toLowerCase() === 'application/json',
    'json_content_type_required', 415);
  requireValue(!request.headers.has('content-encoding')
    || request.headers.get('content-encoding') === 'identity', 'request_encoding_rejected', 415);
  const declared = request.headers.get('content-length');
  if (declared !== null) requireValue(/^(?:0|[1-9][0-9]*)$/.test(declared)
    && Number(declared) <= limit, 'request_size_limit', 413);
  return parseJson(await boundedBytes(request.body, limit,
    declared === null ? undefined : Number(declared)));
}
function verifyMetadata(record, entry, cacheControl = entry.cacheControl) {
  requireValue(record && record.key === entry.r2Key && record.size === entry.size
    && typeof record.httpEtag === 'string' && ETAG_PATTERN.test(record.httpEtag)
    && record.httpMetadata?.contentType === entry.contentType
    && record.httpMetadata?.cacheControl === cacheControl
    && (!record.httpMetadata.contentEncoding || record.httpMetadata.contentEncoding === 'identity')
    && record.customMetadata?.sha256 === entry.sha256
    && hex(record.checksums?.sha256) === entry.sha256, 'stored_object_mismatch', 409);
}
function selectedHeaders(record) {
  return {'content-length': String(record.size), 'content-type': record.httpMetadata.contentType,
    'etag': record.httpEtag, 'cache-control': 'no-store',
    'x-mir2-stored-sha256': hex(record.checksums.sha256),
    'x-mir2-custom-sha256': record.customMetadata.sha256,
    'x-mir2-stored-cache-control': record.httpMetadata.cacheControl,
    'x-mir2-stored-content-encoding': record.httpMetadata.contentEncoding ?? 'identity'};
}
function responseHeaders() {
  return {'cache-control': 'no-store', 'x-content-type-options': 'nosniff',
    'x-mir2-native-plan-sha256': NATIVE_PLAN_SHA256,
    'x-mir2-source-plan-sha256': definition.sourcePlanSha256,
    'x-mir2-root-verification-sha256': definition.rootVerificationSha256,
    'x-mir2-objects-sha256': definition.objectsSha256};
}
function json(payload, status = 200) {
  return new Response(JSON.stringify(payload), {status,
    headers: {...responseHeaders(), 'content-type': 'application/json; charset=utf-8'}});
}

function pointerEntry(sha256, size) {
  return {r2Key: definition.r2Prefix + POINTER, size, sha256,
    contentType: 'application/json', cacheControl: 'no-store'};
}

export function stageEnvelopeTemplate() {
  return {schema: STAGE_SCHEMA, mode: 'stage', passed: true,
    planSha256: definition.sourcePlanSha256,
    rootVerificationSha256: definition.rootVerificationSha256,
    nativePlanSha256: NATIVE_PLAN_SHA256, objectsSha256: definition.objectsSha256,
    candidate: JSON.parse(JSON.stringify(definition.candidate)), expectedCurrent: null,
    pointerChanged: false, publicBase: definition.publicBase, publicVerified: true,
    verifiedObjects: closure()};
}
function validateStageEnvelope(envelope) {
  exactKeys(envelope, Object.keys(stageEnvelopeTemplate()), 'invalid_stage_envelope');
  requireValue(same(envelope, stageEnvelopeTemplate()), 'passed_bound_stage_envelope_required');
}

// Dependency injection is internal test scaffolding only. Production dispatch
// uses the immutable module defaults; no env/client option can choose a URL,
// object table, crypto implementation, runtime stream or clock.
export function createNativeR17Handler(dependencies = {}) {
  const cryptoApi = dependencies.crypto ?? globalThis.crypto;
  const fetchOrigin = dependencies.fetch ?? ((...args) => globalThis.fetch(...args));
  const makeFixedLength = dependencies.fixedLengthStream ?? (size => new globalThis.FixedLengthStream(size));
  const makeDigest = dependencies.digestStream ?? (() => new cryptoApi.DigestStream('SHA-256'));
  const now = dependencies.now ?? Date.now;
  let validated;
  const validate = () => validated ??= validateNativePlan(definition, cryptoApi);

  async function fetchSource(entry) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), ORIGIN_TIMEOUT_MS);
    let response;
    try {
      response = await fetchOrigin(definition.originBase + entry.originRelativePath, {
        method: 'GET', redirect: 'manual', signal: controller.signal,
        headers: {'accept-encoding': 'identity', 'cache-control': 'no-store'},
      });
      const length = response.headers.get('content-length');
      requireValue(response.status === 200 && !response.redirected, 'origin_status_rejected', 502);
      requireValue(!response.headers.get('content-encoding')
        || response.headers.get('content-encoding') === 'identity', 'origin_encoding_rejected', 502);
      requireValue(length !== null && /^[1-9][0-9]*$/.test(length)
        && Number(length) === entry.size && response.body, 'origin_size_rejected', 502);
      return {response, clear: () => clearTimeout(timeout), abort: () => controller.abort()};
    } catch (error) {
      controller.abort(); clearTimeout(timeout); await cancelBody(response?.body);
      if (error instanceof Fault) throw error;
      throw new Fault('origin_fetch_failed', 502);
    }
  }
  async function hashExisting(record, entry) {
    try {
      verifyMetadata(record, entry);
      requireValue(record.body && typeof makeDigest === 'function', 'digest_stream_unavailable', 503);
      const digestStream = makeDigest(); let size = 0;
      const digestResult = digestStream.digest; digestResult.catch(() => {});
      const limiter = new TransformStream({
        transform(chunk, controller) {
          requireValue(chunk instanceof Uint8Array, 'invalid_byte_stream');
          size += chunk.byteLength; requireValue(size <= entry.size, 'existing_size_mismatch', 409);
          controller.enqueue(chunk);
        },
        flush() {requireValue(size === entry.size, 'existing_size_mismatch', 409);},
      });
      await record.body.pipeThrough(limiter).pipeTo(digestStream);
      requireValue(hex(await digestResult) === entry.sha256, 'existing_hash_mismatch', 409);
    } catch (error) { await cancelBody(record.body); throw error; }
  }
  async function importObject(bucket, entry) {
    const existing = await bucket.get(entry.r2Key);
    if (existing !== null) {
      await hashExisting(existing, entry);
      return json({ok: true, mode: 'import', index: entry.index, path: entry.path,
        size: entry.size, sha256: entry.sha256, resumed: true, pointerChanged: false});
    }
    let fixed;
    try { fixed = makeFixedLength(entry.size); }
    catch { throw new Fault('fixed_length_stream_unavailable', 503); }
    requireValue(fixed?.readable && fixed?.writable, 'fixed_length_stream_unavailable', 503);
    const origin = await fetchSource(entry); const controller = new AbortController();
    const pipe = origin.response.body.pipeTo(fixed.writable, {signal: controller.signal});
    const put = Promise.resolve().then(() => bucket.put(entry.r2Key, fixed.readable, {
      onlyIf: new Headers({'if-none-match': '*'}), sha256: checksumBytes(entry.sha256),
      httpMetadata: {contentType: entry.contentType, cacheControl: entry.cacheControl},
      customMetadata: {sha256: entry.sha256},
    })).then(record => {
      requireValue(record !== null, 'immutable_create_conflict', 412); return record;
    });
    try {
      const [, stored] = await Promise.all([pipe, put]); verifyMetadata(stored, entry);
      verifyMetadata(await bucket.head(entry.r2Key), entry);
      return json({ok: true, mode: 'import', index: entry.index, path: entry.path,
        size: entry.size, sha256: entry.sha256, resumed: false, pointerChanged: false}, 201);
    } catch (error) {
      controller.abort(); origin.abort(); await cancelBody(fixed.readable);
      await Promise.allSettled([pipe, put]);
      await cancelBody(origin.response.body);
      if (error instanceof Fault) throw error;
      throw new Fault('immutable_stream_or_checksum_failed', 502);
    } finally {origin.clear();}
  }
  async function readPointer(bucket) {
    const record = await bucket.get(definition.r2Prefix + POINTER);
    if (record === null) return null;
    try {
      const expectedBytes = ENCODER.encode(canonical(definition.candidate));
      const expectedHash = await digest(expectedBytes, cryptoApi);
      verifyMetadata(record, pointerEntry(expectedHash, expectedBytes.byteLength));
      const bytes = await boundedBytes(record.body, MAX_POINTER, expectedBytes.byteLength);
      requireValue(await digest(bytes, cryptoApi) === expectedHash
        && same(parseJson(bytes), definition.candidate), 'current_pointer_mismatch', 409);
      return {record, bytes};
    } catch (error) {await cancelBody(record.body); throw error;}
  }
  async function verifyOriginPair() {
    const pair = definition.objects.filter(entry => entry.path.startsWith('feeds/'));
    let parsedFeed;
    for (const entry of pair) {
      const origin = await fetchSource(entry);
      try {
        const bytes = await boundedBytes(origin.response.body, MAX_POINTER, entry.size);
        requireValue(await digest(bytes, cryptoApi) === entry.sha256, 'origin_feed_pair_changed', 409);
        if (entry.path.endsWith('/latest.json')) parsedFeed = parseJson(bytes);
      } finally {origin.clear();}
    }
    const seconds = Math.floor(now() / 1000);
    requireValue(parsedFeed?.schema === 'mir2.windows.update-feed.v1'
      && parsedFeed.channel === 'invited' && parsedFeed.platform === 'windows-x64'
      && parsedFeed.sequence === 12 && Number.isSafeInteger(parsedFeed.createdUnix)
      && Number.isSafeInteger(parsedFeed.expiresUnix) && seconds >= parsedFeed.createdUnix
      && seconds < parsedFeed.expiresUnix, 'origin_feed_expired_or_invalid', 409);
  }
  async function promote(bucket, envelope, progress) {
    validateStageEnvelope(envelope);
    for (const entry of definition.objects) verifyMetadata(await bucket.head(entry.r2Key), entry);
    // Reread exactly the bound signed pair last, after all 36 metadata checks.
    await verifyOriginPair();
    const current = await readPointer(bucket);
    if (current) {
      return json({ok: true, mode: 'promote', candidate: definition.candidate,
        ...progress, alreadyPromoted: true, privatePointerVerified: true,
        aliasesVerified: false, publicVerificationRequired: true});
    }
    const bytes = ENCODER.encode(canonical(definition.candidate));
    const sha256 = await digest(bytes, cryptoApi);
    progress.pointerAttempted = true; progress.pointerOutcomeUnknown = true;
    let record;
    try {
      record = await bucket.put(definition.r2Prefix + POINTER, bytes, {
        onlyIf: new Headers({'if-none-match': '*'}), sha256: checksumBytes(sha256),
        httpMetadata: {contentType: 'application/json', cacheControl: 'no-store'},
        customMetadata: {sha256},
      });
    } catch {throw new Fault('pointer_write_outcome_unknown', 502);}
    if (record === null) {
      progress.pointerOutcomeUnknown = false; throw new Fault('pointer_compare_and_swap_failed', 412);
    }
    progress.pointerChanged = true;
    verifyMetadata(record, pointerEntry(sha256, bytes.byteLength));
    requireValue(await readPointer(bucket), 'pointer_read_after_write_failed', 502);
    progress.pointerOutcomeUnknown = false;
    return json({ok: true, mode: 'promote', candidate: definition.candidate,
      ...progress, alreadyPromoted: false, privatePointerVerified: true,
      aliasesVerified: false, publicVerificationRequired: true});
  }
  return async function handle(request, env) {
    const progress = {pointerAttempted: false, pointerChanged: false, pointerOutcomeUnknown: false};
    try {
      requireValue(typeof env.MIR2_R2_UPLOAD_SECRET === 'string' && env.MIR2_R2_UPLOAD_SECRET.length > 0
        && request.headers.get('authorization') === `Bearer ${env.MIR2_R2_UPLOAD_SECRET}`, 'unauthorized', 401);
      const url = new URL(request.url);
      requireValue(url.protocol === 'https:' && !url.username && !url.password && !url.search && !url.hash
        && !request.url.includes('?') && !/%|\\|\/\.\.?\//.test(url.pathname), 'invalid_native_path');
      await validate();
      requireValue(env.MIR2_ASSETS && typeof env.MIR2_ASSETS.get === 'function'
        && typeof env.MIR2_ASSETS.head === 'function' && typeof env.MIR2_ASSETS.put === 'function',
      'r2_binding_unavailable', 503);
      if (url.pathname === PREFIX + '/import') {
        requireValue(request.method === 'POST', 'method_not_allowed', 405);
        const input = await requestJson(request, MAX_IMPORT_REQUEST);
        const keys = Object.keys(input ?? {});
        requireValue(keys.length === 1 && (keys[0] === 'path' || keys[0] === 'index'), 'invalid_import_request');
        const entry = keys[0] === 'index'
          ? (Number.isSafeInteger(input.index) && input.index >= 0 ? definition.objects[input.index] : null)
          : definition.objects.find(object => object.path === input.path);
        requireValue(entry, 'literal_object_required');
        return await importObject(env.MIR2_ASSETS, entry);
      }
      if (url.pathname === PREFIX + '/promote') {
        requireValue(request.method === 'POST', 'method_not_allowed', 405);
        return await promote(env.MIR2_ASSETS, await requestJson(request, MAX_STAGE_REQUEST), progress);
      }
      const match = new RegExp('^' + PREFIX + '/objects/(0|[1-9][0-9]?)$').exec(url.pathname);
      if (match) {
        requireValue(request.method === 'GET' || request.method === 'HEAD', 'method_not_allowed', 405);
        const entry = definition.objects[Number(match[1])]; requireValue(entry, 'literal_object_required');
        const record = await env.MIR2_ASSETS[request.method === 'HEAD' ? 'head' : 'get'](entry.r2Key);
        if (record === null) return new Response(null, {status: 404, headers: responseHeaders()});
        try {verifyMetadata(record, entry);} catch (error) {await cancelBody(record.body); throw error;}
        return new Response(request.method === 'HEAD' ? null : record.body, {
          headers: {...selectedHeaders(record), ...responseHeaders()},
        });
      }
      if (url.pathname === PREFIX + '/pointer') {
        requireValue(request.method === 'GET' || request.method === 'HEAD', 'method_not_allowed', 405);
        const result = await readPointer(env.MIR2_ASSETS);
        if (!result) return new Response(null, {status: 404, headers: responseHeaders()});
        return new Response(request.method === 'HEAD' ? null : result.bytes, {
          headers: {...selectedHeaders(result.record), ...responseHeaders()},
        });
      }
      throw new Fault('not_found', 404);
    } catch (error) {
      return json({ok: false, error: error instanceof Fault ? error.code : 'native_operation_failed',
        ...progress}, error instanceof Fault ? error.status : 502);
    }
  };
}

export const nativeR17Fetch = createNativeR17Handler();
