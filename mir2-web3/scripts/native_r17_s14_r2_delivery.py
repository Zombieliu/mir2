#!/usr/bin/env python3
"""Fixed R17 CI delivery orchestration. Credentials and HTTP bodies are never logged.

Only the trusted opaque CI invocation may read MIR2_R2_UPLOAD_SECRET. This is
not a CMS verifier, generic uploader, deployment tool or throughput benchmark.
"""
from __future__ import annotations

import base64
from contextlib import contextmanager
import hashlib
import http.client
import json
import os
from pathlib import Path
import queue
import re
import socket
import ssl
import stat
import sys
import threading
import time

sys.dont_write_bytecode = True
HOST = 'assets.mir2.obelisk.build'
PORT = 443
PUBLIC_PREFIX = '/client-updates/'
PUBLIC_BASE = 'https://' + HOST + PUBLIC_PREFIX
PRIVATE_PREFIX = '/upload/native-r17-s14'
IMMUTABLE = 'public, max-age=31536000, immutable'
SOURCE_PLAN_SHA = 'e9daa352802c6c092a7c575b35e6eff7ccf4a4baf139606925a8c348c22925ee'
ROOT_SHA = 'd86637fe188e84a671db378643f865d4dc0ce48d33625dd90cf1f9f87dbef7e5'
NATIVE_SHA = 'a2f209448875cd200ea182b4da880ceba426e9b05e8ef8e9034192f09c360d6a'
LITERAL_SHA = '2bb21fd0a2387de39641e37a3a11663b62610bcc32d8f35e8207e842184a1eb1'
OBJECTS_SHA = 'b6691b9a90d40248934ec93adfc36e7d1e1ec81c3929fadd0c09fd9166c4cc94'
PLAN_FILE = Path(__file__).resolve().parents[1] / 'infra/cloudflare/mir2-r2-bulk-upload/src/native-r17-s14-publication-plan.json'
CHUNK = 65536
CONTROL_LIMIT = 65536
RECEIPT_LIMIT = 262144
CONTROL_SECONDS = 150
IMPORT_CONTROL_SECONDS = 2460
PROMOTE_CONTROL_SECONDS = 360
PUBLIC_SECONDS = 2460
NO_PROGRESS_SECONDS = 90
PUBLIC_HEADER_SECONDS = 30
MAX_CALL_MS = 2460000
SHA_RE = re.compile(r'[0-9a-f]{64}\Z')
ETAG_RE = re.compile(r'"[^"\x00-\x20\x7f]{1,254}"\Z')
SAFE_CODES = frozenset(('ok', 'arguments_invalid', 'plan_invalid', 'secret_missing',
    'secret_invalid', 'receipt_invalid', 'receipt_io_failed', 'stage_receipt_invalid',
    'stage_receipt_hash_mismatch', 'transport_failed', 'request_deadline', 'request_cancelled',
    'tls_policy_rejected', 'endpoint_rejected', 'http_status_rejected', 'redirect_rejected',
    'headers_rejected', 'json_rejected', 'body_length_rejected', 'body_hash_rejected',
    'pointer_rejected', 'import_reply_rejected', 'promote_reply_rejected', 'stage_incomplete',
    'feed_expired_or_invalid', 'prepared_release_unadmitted', 'stage_pointer_changed',
    'object_write_outcome_unknown', 'internal_error'))


class Fault(Exception):
    def __init__(self, code):
        self.code = code if code in SAFE_CODES else 'internal_error'
        super().__init__(self.code)


def require(condition, code):
    if not condition:
        raise Fault(code)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'),
                       ensure_ascii=False, allow_nan=False) + '\n').encode('utf-8')


def strict_json(data, limit=CONTROL_LIMIT, tokens=8192):
    require(isinstance(data, bytes) and len(data) <= limit, 'json_rejected')
    try:
        text = data.decode('utf-8', 'strict')
        require(not text.startswith('\ufeff'), 'json_rejected')
        depth, in_string, escaped = 0, False, False
        for char in text:
            if in_string:
                if escaped:
                    escaped = False
                elif char == '\\':
                    escaped = True
                elif char == '"':
                    in_string = False
            elif char == '"':
                in_string = True
            elif char in '[{':
                depth += 1
                require(depth <= 12, 'json_rejected')
            elif char in ']}':
                depth -= 1
                require(depth >= 0, 'json_rejected')
        def pairs(items):
            result = {}
            for key, value in items:
                require(key not in result, 'json_rejected')
                result[key] = value
            return result
        def no_float(_value):
            raise Fault('json_rejected')
        value = json.loads(text, object_pairs_hook=pairs, parse_constant=no_float,
                           parse_float=no_float)
        count = 0
        def check(item):
            nonlocal count
            count += 1
            require(count <= tokens, 'json_rejected')
            if isinstance(item, str):
                require(not any(0xD800 <= ord(c) <= 0xDFFF for c in item), 'json_rejected')
            elif isinstance(item, dict):
                for key, child in item.items():
                    check(key)
                    check(child)
            elif isinstance(item, list):
                for child in item:
                    check(child)
        check(value)
        return value
    except Fault:
        raise
    except BaseException:
        raise Fault('json_rejected') from None


def closure(plan):
    return [{key: entry[key] for key in ('path', 'size', 'sha256')}
            for entry in plan['objects']]


def read_prepared_plan_bytes(raw):
    """Literal/table proof only. It does NOT admit this prepared release."""
    try:
        require(len(raw) == 63692 and digest(raw) == LITERAL_SHA, 'plan_invalid')
        plan = strict_json(raw)
        require(digest(canonical(plan)) == NATIVE_SHA
                and plan['schema'] == 'mir2.native.r17.fixed-origin-plan.v1'
                and plan['candidate']['sequence'] == 14
                and plan['candidate']['sourceRevision'] == 'b4390ee4c987bbf000ba1d95761151e39fc0d54f'
                and plan['preparation']['requiredGameSourceRevision'] == '6032ef8b3e27dd97bad0b20c8676ef9f185db64b'
                and plan['objectsSha256'] == OBJECTS_SHA and plan['publicBase'] == PUBLIC_BASE
                and plan['originBase'] == 'https://165.154.65.136.sslip.io/client-updates/'
                and plan['promotionOriginPair'] == {'json': 'latest.json', 'signature': 'latest.p7s'}
                and len(plan['objects']) == 36, 'plan_invalid')
        proof = plan['predecessorProof']
        previous_raw = base64.b64decode(proof['sourcePlanBase64'], validate=True)
        previous_root_raw = base64.b64decode(proof['rootVerificationBase64'], validate=True)
        require(digest(previous_raw) == proof['sourcePlanSha256']
                and digest(previous_root_raw) == proof['rootVerificationSha256'], 'plan_invalid')
        previous = strict_json(previous_raw)
        previous_root = strict_json(previous_root_raw)
        require(previous['schema'] == 'mir2.windows.r2-publication.v1'
                and previous['candidate'] == plan['knownPrevious'] and plan['knownPrevious']['sequence'] == 13
                and previous['verificationReceipt']['sha256'] == proof['rootVerificationSha256']
                and previous_root['schema'] == 'mir2.windows.r2-verification.v1'
                and previous_root['passed'] is True and previous_root['cmsVerified'] is True
                and previous_root['candidateVerified'] is True
                and all(previous_root[k] == plan['knownPrevious'][k] for k in
                        ('sourceRevision', 'sequence', 'feedSha256', 'signatureSha256')), 'plan_invalid')
        old_game = sorted([e for e in previous['objects'] if e['path'].startswith('releases/game-')],
                          key=lambda e: e['path'])
        new_game = [e for e in plan['objects'] if e['path'].startswith('releases/game-')]
        require(len(old_game) == len(new_game) == 30
                and closure({'objects': old_game}) == closure({'objects': new_game})
                and digest(canonical(closure(plan))) == OBJECTS_SHA, 'plan_invalid')
        operation = plan['operationPolicy']
        require(operation['controlSeconds'] == CONTROL_SECONDS
                and operation['importControlSeconds'] == IMPORT_CONTROL_SECONDS
                and operation['promoteControlSeconds'] == PROMOTE_CONTROL_SECONDS
                and operation['publicSeconds'] == PUBLIC_SECONDS
                and operation['noProgressMs'] == NO_PROGRESS_SECONDS * 1000
                and operation['originHeaderMs'] == PUBLIC_HEADER_SECONDS * 1000
                and operation['measuredSpeed'] is False, 'plan_invalid')
        for index, entry in enumerate(plan['objects']):
            require(type(entry['index']) is int and entry['index'] == index
                    and type(entry['size']) is int and 0 < entry['size'] <= 128 * 1024 * 1024
                    and SHA_RE.fullmatch(entry['sha256']) and entry['cacheControl'] == IMMUTABLE
                    and entry['r2Key'] == plan['r2Prefix'] + entry['path']
                    and re.fullmatch(r'[A-Za-z0-9._/-]+', entry['path'])
                    and all(part not in ('', '.', '..') and not part.startswith('.')
                            for part in entry['path'].split('/')), 'plan_invalid')
        return plan
    except BaseException:
        raise Fault('plan_invalid') from None


def load_plan_bytes(raw):
    plan = read_prepared_plan_bytes(raw)
    # Admission binds the root-issued actual CMS/closed36 proof bytes. A test
    # mock, hash-only build receipt or old12 proof cannot admit a different plan.
    require(plan['admission'] == 'admitted' and isinstance(SOURCE_PLAN_SHA, str)
            and isinstance(ROOT_SHA, str) and SHA_RE.fullmatch(SOURCE_PLAN_SHA)
            and SHA_RE.fullmatch(ROOT_SHA), 'prepared_release_unadmitted')
    try:
        original_raw = base64.b64decode(plan['sourcePlanBase64'], validate=True)
        root_raw = base64.b64decode(plan['rootVerificationBase64'], validate=True)
        require(digest(original_raw) == SOURCE_PLAN_SHA and digest(root_raw) == ROOT_SHA, 'plan_invalid')
        original = strict_json(original_raw)
        root = strict_json(root_raw)
        require(plan['sourcePlanSha256'] == SOURCE_PLAN_SHA and plan['rootVerificationSha256'] == ROOT_SHA
                and plan['expectedCurrent'] is None and original['expectedCurrent'] is None
                and original['candidate'] == plan['candidate']
                and original['verificationReceipt']['sha256'] == ROOT_SHA
                and closure({'objects': sorted(original['objects'], key=lambda e: e['path'])}) == closure(plan),
                'plan_invalid')
        require(root['schema'] == 'mir2.windows.r2-verification.v1' and root['passed'] is True
                and root['cmsVerified'] is True and root['candidateVerified'] is True
                and root['objectsSha256'] == OBJECTS_SHA
                and all(root[k] == plan['candidate'][k] for k in
                        ('sourceRevision', 'sequence', 'feedSha256', 'signatureSha256')), 'plan_invalid')
        return plan
    except BaseException:
        raise Fault('plan_invalid') from None


def allowed_current(value, plan):
    return value is None or canonical(value) in (canonical(plan['knownPrevious']), canonical(plan['candidate']))


def expected_envelope(plan, observed_current=None):
    require(allowed_current(observed_current, plan), 'pointer_rejected')
    return {'schema': 'mir2.windows.r2-native-stage.v2', 'mode': 'stage', 'passed': True,
        'planSha256': SOURCE_PLAN_SHA, 'rootVerificationSha256': ROOT_SHA,
        'nativePlanSha256': NATIVE_SHA, 'objectsSha256': OBJECTS_SHA,
        'candidate': plan['candidate'], 'expectedCurrent': observed_current, 'pointerChanged': False,
        'publicBase': PUBLIC_BASE, 'publicVerified': True, 'verifiedObjects': closure(plan)}

def plan_bindings():
    return {'literalPlanSha256': LITERAL_SHA, 'nativePlanSha256': NATIVE_SHA,
        'planSha256': SOURCE_PLAN_SHA, 'rootVerificationSha256': ROOT_SHA,
        'objectsSha256': OBJECTS_SHA}


def source_sha():
    return digest(Path(__file__).read_bytes())


def empty_receipt(mode):
    return {'schema': 'mir2.windows.r2-native-ci.v2', 'mode': mode, 'passed': False,
        'result': 'internal_error', 'sourceSha256': source_sha(), 'bindings': plan_bindings(),
        'candidate': None, 'calls': [], 'verifiedObjects': [], 'publicVerified': False,
        'verifiedObjectCount': 0, 'publicVerifiedBytes': 0, 'stageEnvelope': None,
        'observedCurrent': None, 'objectWriteAttempted': False, 'objectOutcomeUnknown': False,
        'pointerAttempted': False, 'pointerChanged': False, 'pointerOutcomeUnknown': False,
        'privatePointerVerified': False, 'aliasesVerified': False, 'stageReceiptSha256': None,
        'receiptStatus': 'not_written', 'cmsReverified': False,
        'networkAuthorityAssumed': False, 'deploymentPerformed': False}


def header_map(response):
    try:
        pairs = response.getheaders()
        require(len(pairs) <= 80, 'headers_rejected')
        result, size = {}, 0
        for key, value in pairs:
            require(isinstance(key, str) and isinstance(value, str), 'headers_rejected')
            key = key.lower()
            size += len(key) + len(value)
            require(size <= 16384 and key not in result and not any(c in value for c in '\r\n\x00'), 'headers_rejected')
            result[key] = value
        require(result.get('content-encoding', 'identity') == 'identity'
                and 'content-range' not in result, 'headers_rejected')
        return result
    except Fault:
        raise
    except BaseException:
        raise Fault('headers_rejected') from None


def private_headers(headers):
    expected = {'cache-control': 'no-store', 'x-content-type-options': 'nosniff',
        'x-mir2-native-plan-sha256': NATIVE_SHA, 'x-mir2-source-plan-sha256': SOURCE_PLAN_SHA or 'unadmitted',
        'x-mir2-root-verification-sha256': ROOT_SHA or 'unadmitted', 'x-mir2-objects-sha256': OBJECTS_SHA}
    require(all(headers.get(k) == v for k, v in expected.items()), 'headers_rejected')


def private_object_headers(headers, entry):
    require(headers.get('content-length') == str(entry['size'])
            and headers.get('content-type') == entry['contentType']
            and ETAG_RE.fullmatch(headers.get('etag', ''))
            and headers.get('x-mir2-stored-sha256') == entry['sha256']
            and headers.get('x-mir2-custom-sha256') == entry['sha256']
            and headers.get('x-mir2-stored-cache-control') == entry['cacheControl']
            and headers.get('x-mir2-stored-content-encoding') == 'identity', 'headers_rejected')


def public_mime(path):
    if path.endswith('.json'):
        return 'application/json'
    if path.endswith('.p7s'):
        return 'application/pkcs7-signature'
    if path.endswith('.gz'):
        return 'application/gzip'
    return 'application/octet-stream'


def public_headers(headers, entry, candidate=None):
    require(headers.get('content-length') == str(entry['size'])
            and headers.get('content-type') == public_mime(entry['path'])
            and headers.get('cache-control') == ('no-store' if candidate else IMMUTABLE)
            and ETAG_RE.fullmatch(headers.get('etag', ''))
            and headers.get('accept-ranges') == 'bytes'
            and headers.get('x-content-type-options') == 'nosniff'
            and headers.get('access-control-allow-origin') == '*'
            and headers.get('x-mir2-native-cache') in (('BYPASS',) if candidate else ('HIT', 'MISS')), 'headers_rejected')
    if candidate:
        directory = 'feeds/s' + str(candidate['sequence']) + '-' + candidate['feedSha256'] + '/'
        require(headers.get('x-mir2-sequence') == str(candidate['sequence'])
                and headers.get('x-mir2-feed-sha256') == candidate['feedSha256']
                and headers.get('x-mir2-feed-path') == directory + 'latest.json'
                and headers.get('x-mir2-signature-path') == directory + 'latest.p7s', 'headers_rejected')


def verified_tls_context():
    # Explicit construction retains verified TLS and system CA trust without
    # create_default_context's implicit SSLKEYLOGFILE environment side effect.
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.load_default_certs(ssl.Purpose.SERVER_AUTH)
    require(context.verify_mode == ssl.CERT_REQUIRED and context.check_hostname is True
            and context.keylog_filename is None, 'tls_policy_rejected')
    return context


class NetworkResponse:
    """Bounded two-chunk producer; absolute deadline also fences blocking headers/DNS.

    A cancelled DNS/connect worker is daemon-only and checks cancellation before
    sending any request. An interrupted POST can already have committed: callers
    must retain the unknown outcome rather than retry it.
    """
    def __init__(self, method, path, token, body, seconds):
        started = time.monotonic()
        self.deadline = started + seconds
        self.budget_lock = threading.Lock()
        self.idle_deadline = started + min(NO_PROGRESS_SECONDS, seconds)
        # The Worker completes origin->R2 before import JSON headers. A private
        # import/promote header wait has its route's finite overall allowance;
        # connect, ordinary headers and every body still have progress fences.
        self.header_seconds = seconds if method == 'POST' and path in (
            PRIVATE_PREFIX + '/import', PRIVATE_PREFIX + '/promote') else min(PUBLIC_HEADER_SECONDS, seconds)
        self.cancelled = threading.Event()
        self.messages = queue.Queue(maxsize=2)
        self.connection = None
        self.pending = b''
        self.finished = False
        self.worker = threading.Thread(target=self._pump, args=(method, path, token, body), daemon=True)
        self.worker.start()
        kind, value = self._next()
        require(kind == 'headers', 'transport_failed')
        self.status, self._headers = value

    def _progress_budget(self, seconds):
        with self.budget_lock:
            self.idle_deadline = min(self.deadline, time.monotonic() + seconds)

    def _remaining(self):
        with self.budget_lock:
            remaining = min(self.deadline, self.idle_deadline) - time.monotonic()
        require(not self.cancelled.is_set(), 'request_cancelled')
        require(remaining > 0, 'request_deadline')
        return remaining

    def _send(self, kind, value):
        while not self.cancelled.is_set():
            try:
                self.messages.put((kind, value), timeout=min(self._remaining(), 0.1))
                return
            except queue.Full:
                pass

    def _pump(self, method, path, token, body):
        connection, terminal = None, ('done', None)
        try:
            context = verified_tls_context()
            require(context.verify_mode == ssl.CERT_REQUIRED and context.check_hostname is True, 'tls_policy_rejected')
            connection = http.client.HTTPSConnection(HOST, port=PORT,
                timeout=self._remaining(), context=context)
            self.connection = connection
            connection.set_debuglevel(0)
            connection.connect()
            require(connection.sock is not None, 'transport_failed')
            connection.sock.settimeout(self._remaining())
            headers = {'Accept-Encoding': 'identity', 'Cache-Control': 'no-store',
                       'User-Agent': 'Mir2NativeR17Delivery/1'}
            if token is not None:
                headers['Authorization'] = 'Bearer ' + token
            if body is not None:
                headers['Content-Type'] = 'application/json'
                headers['Content-Length'] = str(len(body))
            self._remaining()
            connection.request(method, path, body=body, headers=headers)
            self._progress_budget(self.header_seconds)
            connection.sock.settimeout(self._remaining())
            response = connection.getresponse()
            self._progress_budget(NO_PROGRESS_SECONDS)
            self._send('headers', (response.status, response.getheaders()))
            if method != 'HEAD' and response.status in (200, 201, 409, 412, 499, 502, 503, 504):
                while True:
                    connection.sock.settimeout(self._remaining())
                    # HTTPResponse.read1 returns currently available progress,
                    # rather than waiting to fill a whole64KiB read.
                    read_progress = getattr(response, 'read1', response.read)
                    chunk = read_progress(CHUNK)
                    self._remaining()
                    require(isinstance(chunk, bytes) and len(chunk) <= CHUNK, 'transport_failed')
                    if not chunk:
                        break
                    self._progress_budget(NO_PROGRESS_SECONDS)
                    self._send('chunk', chunk)
        except Fault as failure:
            terminal = ('error', failure.code)
        except BaseException:
            terminal = ('error', 'transport_failed')
        finally:
            if connection is not None:
                try:
                    connection.close()
                except BaseException:
                    terminal = ('error', 'transport_failed')
            try:
                self._send(*terminal)
            except BaseException:
                pass

    def _next(self):
        try:
            while True:
                try:
                    # Poll only the bounded queue, allowing the connect/header
                    # phase to change its deadline without a stale long wait.
                    message = self.messages.get(timeout=min(self._remaining(), 0.1))
                    break
                except queue.Empty:
                    continue
        except Fault:
            self.close()
            raise
        if message[0] == 'error':
            self.close()
            raise Fault(message[1])
        return message

    def getheaders(self):
        return self._headers

    def read(self, maximum):
        require(type(maximum) is int and 0 < maximum <= CHUNK, 'internal_error')
        if self.finished:
            return b''
        if not self.pending:
            kind, chunk = self._next()
            if kind == 'done':
                self.finished = True
                return b''
            require(kind == 'chunk' and isinstance(chunk, bytes) and len(chunk) <= CHUNK, 'transport_failed')
            self.pending = chunk
        result, self.pending = self.pending[:maximum], self.pending[maximum:]
        return result

    def close(self):
        self.cancelled.set()
        if not self.finished and self.connection is not None:
            try:
                self.connection.sock.shutdown(socket.SHUT_RDWR)
            except BaseException:
                pass
        if threading.current_thread() is not self.worker:
            self.worker.join(timeout=0.1)


class FixedHttpsTransport:
    def __init__(self, plan):
        self.plan = plan
        self.public_paths = frozenset(PUBLIC_PREFIX + e['path'] for e in plan['objects']) | {
            PUBLIC_PREFIX + 'latest.json', PUBLIC_PREFIX + 'latest.p7s'}

    def open(self, method, path, *, token=None, body=None):
        public = path in self.public_paths and method in ('GET', 'HEAD') and body is None and token is None
        private_read = method in ('GET', 'HEAD') and body is None and (path == PRIVATE_PREFIX + '/pointer'
            or re.fullmatch(re.escape(PRIVATE_PREFIX) + r'/objects/(?:[0-9]|[12][0-9]|3[0-5])', path))
        private_post = method == 'POST' and isinstance(body, bytes) and (
            (path == PRIVATE_PREFIX + '/import' and len(body) <= 1024
             and type(strict_json(body).get('index')) is int
             and 0 <= strict_json(body)['index'] < 36
             and body == canonical({'index': strict_json(body)['index']}))
            or (path == PRIVATE_PREFIX + '/promote' and len(body) <= CONTROL_LIMIT
                and isinstance(strict_json(body), dict)
                and allowed_current(strict_json(body).get('expectedCurrent'), self.plan)
                and strict_json(body) == expected_envelope(self.plan, strict_json(body).get('expectedCurrent'))))
        require(public or ((private_read or private_post) and isinstance(token, str) and token), 'endpoint_rejected')
        try:
            return NetworkResponse(method, path, token, body,
                PUBLIC_SECONDS if public else IMPORT_CONTROL_SECONDS if path == PRIVATE_PREFIX + '/import'
                else PROMOTE_CONTROL_SECONDS if path == PRIVATE_PREFIX + '/promote' else CONTROL_SECONDS)
        except Fault:
            raise
        except BaseException:
            raise Fault('transport_failed') from None


@contextmanager
def request(transport, receipt, method, path, token=None, body=None, index=None):
    started = time.monotonic()
    call = {'method': method, 'route': 'public' if path.startswith(PUBLIC_PREFIX) else path[len(PRIVATE_PREFIX):],
        'index': index, 'status': None, 'receivedBytes': 0, 'headersVerified': False,
        'writeAttempted': method == 'POST', 'complete': False, 'elapsedMs': 0}
    receipt['calls'].append(call)
    response = None
    try:
        response = transport.open(method, path, token=token, body=body)
        require(type(response.status) is int and 100 <= response.status <= 599, 'http_status_rejected')
        call['status'] = response.status
        require(not 300 <= response.status <= 399, 'redirect_rejected')
        headers = header_map(response)
        if not path.startswith(PUBLIC_PREFIX):
            private_headers(headers)
        yield response, headers, call
        call['complete'] = True
    except Fault:
        raise
    except BaseException:
        raise Fault('transport_failed') from None
    finally:
        call['elapsedMs'] = max(0, min(MAX_CALL_MS, int((time.monotonic() - started) * 1000)))
        if response is not None:
            try:
                response.close()
            except BaseException:
                raise Fault('transport_failed') from None


def read_small(response, call, limit=CONTROL_LIMIT, expected=None):
    chunks, size = [], 0
    while True:
        chunk = response.read(CHUNK)
        require(isinstance(chunk, bytes) and len(chunk) <= CHUNK, 'body_length_rejected')
        if not chunk:
            break
        size += len(chunk)
        call['receivedBytes'] = size
        require(size <= limit, 'body_length_rejected')
        chunks.append(chunk)
    require(expected is None or size == expected, 'body_length_rejected')
    return b''.join(chunks)


def read_full(response, call, entry, capture=False):
    hasher, size, kept = hashlib.sha256(), 0, []
    require(not capture or entry['size'] <= 4096, 'internal_error')
    while True:
        chunk = response.read(CHUNK)
        require(isinstance(chunk, bytes) and len(chunk) <= CHUNK, 'body_length_rejected')
        if not chunk:
            break
        size += len(chunk)
        call['receivedBytes'] = size
        require(size <= entry['size'], 'body_length_rejected')
        hasher.update(chunk)
        if capture:
            kept.append(chunk)
    require(size == entry['size'], 'body_length_rejected')
    require(hasher.hexdigest() == entry['sha256'], 'body_hash_rejected')
    return b''.join(kept) if capture else None


def pointer_entry(plan, value=None):
    raw = canonical(plan['candidate'] if value is None else value)
    return {'path': 'channels/invited.json', 'size': len(raw), 'sha256': digest(raw),
            'contentType': 'application/json', 'cacheControl': 'no-store'}


def check_pointer(transport, receipt, plan, token):
    # Return the actual read value, never an assumed predecessor. ETag/CAS are
    # reread by the Worker immediately before put; the client cannot supply one.
    with request(transport, receipt, 'GET', PRIVATE_PREFIX + '/pointer', token) as (response, headers, call):
        require(response.status in (200, 404), 'http_status_rejected')
        if response.status == 404:
            call['headersVerified'] = True
            return None
        raw = read_small(response, call, 4096)
        value = strict_json(raw, 4096)
        require(value is not None and allowed_current(value, plan) and raw == canonical(value), 'pointer_rejected')
        entry = pointer_entry(plan, value)
        private_object_headers(headers, entry)
        require(len(raw) == entry['size'] and digest(raw) == entry['sha256'], 'pointer_rejected')
        call['headersVerified'] = True
        return value

def private_object_head(transport, receipt, entry, token, *, allow_absent):
    with request(transport, receipt, 'HEAD', PRIVATE_PREFIX + '/objects/' + str(entry['index']),
                 token, index=entry['index']) as (response, headers, call):
        require(response.status == 200 or (allow_absent and response.status == 404), 'http_status_rejected')
        if response.status == 200:
            private_object_headers(headers, entry)
        call['headersVerified'] = True


def control_json(response, headers, call):
    require(headers.get('content-type', '').split(';')[0].strip() == 'application/json', 'headers_rejected')
    length = headers.get('content-length')
    expected = None
    if length is not None:
        require(re.fullmatch(r'(?:0|[1-9][0-9]*)', length) and int(length) <= CONTROL_LIMIT
                and 'transfer-encoding' not in headers, 'headers_rejected')
        expected = int(length)
    elif 'transfer-encoding' in headers:
        require(headers['transfer-encoding'].lower() == 'chunked', 'headers_rejected')
    call['headersVerified'] = True
    return strict_json(read_small(response, call, expected=expected))


def import_object(transport, receipt, entry, token):
    # Sending a POST can commit even if its reply is lost or the caller cancels.
    # No automatic retry/reconcile grants a pass. A separate later stage first
    # asks the Worker's actual full-hash immutable resume path.
    receipt.update(objectWriteAttempted=True, objectOutcomeUnknown=True)
    with request(transport, receipt, 'POST', PRIVATE_PREFIX + '/import', token,
                 canonical({'index': entry['index']}), entry['index']) as (response, headers, call):
        require(response.status in (200, 201), 'http_status_rejected')
        reply = control_json(response, headers, call)
        resumed = response.status == 200
        expected = {'ok': True, 'mode': 'import', 'index': entry['index'], 'path': entry['path'],
            'size': entry['size'], 'sha256': entry['sha256'], 'resumed': resumed, 'pointerChanged': False}
        require(canonical(reply) == canonical(expected), 'import_reply_rejected')
    receipt['objectOutcomeUnknown'] = False

def verify_feed(raw, plan):
    value = strict_json(raw, 4096)
    now = int(time.time())
    require(isinstance(value, dict) and value.get('schema') == 'mir2.windows.update-feed.v1'
            and value.get('channel') == 'invited' and value.get('platform') == 'windows-x64'
            and type(value.get('sequence')) is int and value['sequence'] == plan['candidate']['sequence']
            and type(value.get('createdUnix')) is int and type(value.get('expiresUnix')) is int
            and value['createdUnix'] <= now < value['expiresUnix'], 'feed_expired_or_invalid')


def verify_public_object(transport, receipt, entry, plan, *, alias=False):
    relative = ('latest.json' if entry['index'] == 0 else 'latest.p7s') if alias else entry['path']
    with request(transport, receipt, 'GET', PUBLIC_PREFIX + relative,
                 index=entry['index']) as (response, headers, call):
        require(response.status == 200, 'http_status_rejected')
        public_headers(headers, entry, plan['candidate'] if alias else None)
        call['headersVerified'] = True
        raw = read_full(response, call, entry, capture=entry['index'] == 0)
        if entry['index'] == 0:
            verify_feed(raw, plan)


def validate_stage_receipt(stage, plan):
    require(isinstance(stage, dict) and set(stage) == set(empty_receipt('stage'))
            and stage['schema'] == 'mir2.windows.r2-native-ci.v2' and stage['mode'] == 'stage'
            and stage['passed'] is True and stage['result'] == 'ok'
            and stage['sourceSha256'] == source_sha() and stage['bindings'] == plan_bindings()
            and stage['candidate'] == plan['candidate'] and stage['publicVerified'] is True
            and stage['verifiedObjectCount'] == 36
            and stage['publicVerifiedBytes'] == sum(e['size'] for e in plan['objects'])
            and stage['verifiedObjects'] == closure(plan)
            and allowed_current(stage['observedCurrent'], plan)
            and canonical(stage['stageEnvelope']) == canonical(expected_envelope(plan, stage['observedCurrent']))
            and stage['pointerAttempted'] is False and stage['pointerChanged'] is False
            and stage['pointerOutcomeUnknown'] is False and stage['aliasesVerified'] is False
            and stage['objectWriteAttempted'] is True and stage['objectOutcomeUnknown'] is False
            and stage['privatePointerVerified'] is False
            and stage['stageReceiptSha256'] is None and stage['receiptStatus'] == 'written'
            and stage['cmsReverified'] is False and stage['networkAuthorityAssumed'] is False
            and stage['deploymentPerformed'] is False and isinstance(stage['calls'], list), 'stage_receipt_invalid')
    order = [('GET', '/pointer', None)]
    for entry in plan['objects']:
        index = entry['index']
        order.extend([('HEAD', '/objects/' + str(index), index), ('POST', '/import', index),
                      ('HEAD', '/objects/' + str(index), index), ('GET', 'public', index)])
    require(len(stage['calls']) == len(order), 'stage_receipt_invalid')
    call_keys = {'method', 'route', 'index', 'status', 'receivedBytes', 'headersVerified',
                 'writeAttempted', 'complete', 'elapsedMs'}
    for call, expected in zip(stage['calls'], order):
        require(isinstance(call, dict) and set(call) == call_keys
                and (call['method'], call['route'], call['index']) == expected
                and (call['index'] is None or type(call['index']) is int)
                and type(call['status']) is int and call['status'] in (200, 201, 404)
                and type(call['receivedBytes']) is int and 0 <= call['receivedBytes'] <= 128 * 1024 * 1024
                and type(call['elapsedMs']) is int and 0 <= call['elapsedMs'] <= MAX_CALL_MS
                and call['headersVerified'] is True and call['complete'] is True
                and call['writeAttempted'] is (call['method'] == 'POST'), 'stage_receipt_invalid')
        if call['route'] == 'public':
            require(call['status'] == 200 and call['receivedBytes'] == plan['objects'][call['index']]['size'], 'stage_receipt_invalid')
        elif call['method'] == 'HEAD':
            require(call['receivedBytes'] == 0, 'stage_receipt_invalid')
        elif call['route'] == '/import':
            require(call['status'] in (200, 201) and 0 < call['receivedBytes'] <= CONTROL_LIMIT, 'stage_receipt_invalid')
    require((stage['calls'][0]['status'] == 404) is (stage['observedCurrent'] is None)
            and stage['calls'][0]['status'] in (200, 404)
            and stage['calls'][0]['receivedBytes'] == (pointer_entry(plan, stage['observedCurrent'])['size']
                if stage['calls'][0]['status'] == 200 else 0), 'stage_receipt_invalid')
    for index in range(36):
        require(stage['calls'][1 + index * 4]['status'] in (200, 404)
                and stage['calls'][3 + index * 4]['status'] == 200, 'stage_receipt_invalid')


def promote_pointer(transport, receipt, plan, token, stage):
    current = check_pointer(transport, receipt, plan, token)
    receipt['observedCurrent'] = current
    require(canonical(current) == canonical(stage['observedCurrent'])
            or canonical(current) == canonical(plan['candidate']), 'stage_pointer_changed')
    # The Worker must still recheck all immutable metadata, current-origin13
    # signed pair, actual pointer and observed ETag; this is not client authority.
    receipt.update(pointerAttempted=True, pointerOutcomeUnknown=True)
    with request(transport, receipt, 'POST', PRIVATE_PREFIX + '/promote', token,
                 canonical(stage['stageEnvelope'])) as (response, headers, call):
        require(response.status in (200, 409, 412, 499, 502, 503, 504), 'http_status_rejected')
        reply = control_json(response, headers, call)
        flags = ('pointerAttempted', 'pointerChanged', 'pointerOutcomeUnknown',
                 'objectWriteAttempted', 'objectOutcomeUnknown')
        if response.status != 200:
            require(isinstance(reply, dict) and set(reply) == {'ok', 'error', *flags}
                    and reply['ok'] is False and isinstance(reply['error'], str)
                    and all(type(reply[k]) is bool for k in flags)
                    and reply['objectWriteAttempted'] is False and reply['objectOutcomeUnknown'] is False
                    and (not reply['pointerChanged'] or reply['pointerAttempted']), 'promote_reply_rejected')
            if response.status == 412:
                require(reply['pointerAttempted'] is True and reply['pointerChanged'] is False
                        and reply['pointerOutcomeUnknown'] is False, 'promote_reply_rejected')
            receipt.update({k: reply[k] for k in flags})
            raise Fault('http_status_rejected')
        require(isinstance(reply, dict) and type(reply.get('alreadyPromoted')) is bool, 'promote_reply_rejected')
        already = reply['alreadyPromoted']
        expected = {'ok': True, 'mode': 'promote', 'candidate': plan['candidate'],
            'pointerAttempted': not already, 'pointerChanged': not already, 'pointerOutcomeUnknown': False,
            'objectWriteAttempted': False, 'objectOutcomeUnknown': False,
            'alreadyPromoted': already, 'privatePointerVerified': True,
            'aliasesVerified': False, 'publicVerificationRequired': True}
        require(canonical(reply) == canonical(expected), 'promote_reply_rejected')
        receipt.update(pointerAttempted=not already, pointerChanged=not already)
    require(canonical(check_pointer(transport, receipt, plan, token)) == canonical(plan['candidate']), 'pointer_rejected')
    receipt.update(privatePointerVerified=True, pointerOutcomeUnknown=False)

def stage_objects(transport, receipt, plan, token):
    receipt['observedCurrent'] = check_pointer(transport, receipt, plan, token)
    for entry in plan['objects']:
        private_object_head(transport, receipt, entry, token, allow_absent=True)
        import_object(transport, receipt, entry, token)
        private_object_head(transport, receipt, entry, token, allow_absent=False)
        verify_public_object(transport, receipt, entry, plan)
        receipt['verifiedObjects'].append({k: entry[k] for k in ('path', 'size', 'sha256')})
        receipt['verifiedObjectCount'] += 1
        receipt['publicVerifiedBytes'] += entry['size']
    require(receipt['verifiedObjectCount'] == 36 and receipt['verifiedObjects'] == closure(plan)
            and receipt['publicVerifiedBytes'] == sum(e['size'] for e in plan['objects']), 'stage_incomplete')
    # An envelope is created only after the last real stream/headers/close gate.
    receipt.update(publicVerified=True, stageEnvelope=expected_envelope(plan, receipt['observedCurrent']), passed=True, result='ok')


def deliver(mode, plan, transport, token, *, stage=None, stage_sha=None):
    """Internal dependency-injected core; CLI never accepts a plan/endpoint override."""
    receipt = empty_receipt(mode)
    receipt['candidate'] = plan['candidate']
    try:
        if mode == 'stage':
            stage_objects(transport, receipt, plan, token)
        else:
            validate_stage_receipt(stage, plan)
            require(isinstance(stage_sha, str) and SHA_RE.fullmatch(stage_sha)
                    and digest(canonical(stage)) == stage_sha, 'stage_receipt_hash_mismatch')
            receipt['stageReceiptSha256'] = stage_sha
            promote_pointer(transport, receipt, plan, token, stage)
            for entry in plan['objects'][:2]:
                verify_public_object(transport, receipt, entry, plan, alias=True)
            receipt.update(aliasesVerified=True, passed=True, result='ok')
    except Fault as failure:
        receipt['result'] = failure.code
    except BaseException:
        receipt['result'] = 'internal_error'
    return receipt


def checked_path(value):
    require(isinstance(value, (str, Path)) and str(value) and '\x00' not in str(value), 'receipt_invalid')
    path = Path(value)
    require('..' not in path.parts, 'receipt_invalid')
    path = Path(os.path.abspath(path))
    for parent in [*reversed(path.parent.parents), path.parent]:
        info = parent.lstat()
        require(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode)
                and not getattr(info, 'st_file_attributes', 0) & 0x400, 'receipt_invalid')
    return path


@contextmanager
def parent_anchor(path):
    """POSIX openat walks reject directory swaps/symlinks at each component.

    Windows uses lstat/reparse and opened-file identity fences, exercised locally;
    the POSIX branch still requires the separate Linux CI run.
    """
    descriptor = None
    try:
        if os.name == 'posix':
            flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | getattr(os, 'O_CLOEXEC', 0)
            descriptor = os.open(path.anchor, flags)
            for part in path.parts[1:-1]:
                child = os.open(part, flags, dir_fd=descriptor)
                try:
                    require(stat.S_ISDIR(os.fstat(child).st_mode), 'receipt_invalid')
                except BaseException:
                    os.close(child)
                    raise
                os.close(descriptor)
                descriptor = child
        yield descriptor
    finally:
        if descriptor is not None:
            os.close(descriptor)


def anchored_stat(path, parent):
    return path.lstat() if parent is None else os.stat(path.name, dir_fd=parent, follow_symlinks=False)


def anchored_open(path, flags, parent, mode=0o600):
    return os.open(path if parent is None else path.name, flags, mode,
                   **({} if parent is None else {'dir_fd': parent}))


def read_regular(value, limit, private=False):
    fd = None
    try:
        path = checked_path(value)
        with parent_anchor(path) as parent:
            before = anchored_stat(path, parent)
            require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
                    and not getattr(before, 'st_file_attributes', 0) & 0x400, 'receipt_invalid')
            if private and os.name == 'posix':
                require(before.st_uid == os.geteuid() and stat.S_IMODE(before.st_mode) == 0o600, 'receipt_invalid')
            fd = anchored_open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_CLOEXEC', 0), parent)
            opened = os.fstat(fd)
            require((before.st_dev, before.st_ino) == (opened.st_dev, opened.st_ino)
                    and stat.S_ISREG(opened.st_mode) and opened.st_nlink == 1 and opened.st_size <= limit, 'receipt_invalid')
            with os.fdopen(fd, 'rb') as stream:
                fd = None
                raw = stream.read(limit + 1)
                after = os.fstat(stream.fileno())
            current = anchored_stat(path, parent)
            checked_path(path)
            require(len(raw) <= limit and after.st_nlink == 1 and current.st_nlink == 1
                    and stat.S_ISREG(current.st_mode) and not getattr(current, 'st_file_attributes', 0) & 0x400
                    and (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns, opened.st_mode)
                    == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode)
                    and (opened.st_dev, opened.st_ino) == (current.st_dev, current.st_ino), 'receipt_invalid')
        return raw
    except BaseException:
        raise Fault('receipt_invalid') from None
    finally:
        if fd is not None:
            os.close(fd)


def reserve_receipt(value):
    fd = None
    try:
        path = checked_path(value)
        with parent_anchor(path) as parent:
            fd = anchored_open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL
                | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_CLOEXEC', 0), parent)
            opened, current = os.fstat(fd), anchored_stat(path, parent)
            require(stat.S_ISREG(opened.st_mode) and opened.st_nlink == 1
                    and stat.S_ISREG(current.st_mode) and not getattr(current, 'st_file_attributes', 0) & 0x400
                    and (opened.st_dev, opened.st_ino) == (current.st_dev, current.st_ino), 'receipt_invalid')
            if parent is not None:
                os.fsync(parent)
        stream = os.fdopen(fd, 'wb')
        fd = None
        stream._mir2_path = path
        stream._mir2_identity = (opened.st_dev, opened.st_ino)
        return stream
    except BaseException:
        raise Fault('receipt_invalid') from None
    finally:
        if fd is not None:
            os.close(fd)


def validate_output_handle(stream):
    try:
        path = checked_path(stream._mir2_path)
        opened, current = os.fstat(stream.fileno()), path.lstat()
        require(stat.S_ISREG(opened.st_mode) and opened.st_nlink == 1
                and stat.S_ISREG(current.st_mode) and current.st_nlink == 1
                and not getattr(current, 'st_file_attributes', 0) & 0x400
                and (opened.st_dev, opened.st_ino) == stream._mir2_identity
                and (current.st_dev, current.st_ino) == stream._mir2_identity, 'receipt_io_failed')
        if os.name == 'posix':
            require(opened.st_uid == os.geteuid() and stat.S_IMODE(opened.st_mode) == 0o600, 'receipt_io_failed')
    except BaseException:
        raise Fault('receipt_io_failed') from None


def parse_args(args):
    require(isinstance(args, list) and len(args) in (4, 8), 'arguments_invalid')
    pairs = {}
    for index in range(0, len(args), 2):
        key, value = args[index:index + 2]
        require(isinstance(key, str) and isinstance(value, str) and key not in pairs
                and key in ('--mode', '--receipt', '--stage-receipt', '--stage-sha256'), 'arguments_invalid')
        pairs[key] = value
    mode = pairs.get('--mode')
    require(mode in ('stage', 'promote') and '--receipt' in pairs, 'arguments_invalid')
    require(set(pairs) == ({'--mode', '--receipt'} if mode == 'stage' else
        {'--mode', '--receipt', '--stage-receipt', '--stage-sha256'}), 'arguments_invalid')
    if mode == 'promote':
        require(SHA_RE.fullmatch(pairs['--stage-sha256']), 'arguments_invalid')
    return pairs


def main(argv=None, *, environ=None, transport=None, output=None):
    receipt, stream = empty_receipt('invalid'), None
    try:
        args = parse_args(sys.argv[1:] if argv is None else argv)
        mode = args['--mode']
        receipt = empty_receipt(mode)
        stream = reserve_receipt(args['--receipt'])
        plan = load_plan_bytes(read_regular(PLAN_FILE, CONTROL_LIMIT))
        stage, stage_sha = None, None
        if mode == 'promote':
            stage_sha = args['--stage-sha256']
            raw = read_regular(args['--stage-receipt'], RECEIPT_LIMIT, private=True)
            require(digest(raw) == stage_sha, 'stage_receipt_hash_mismatch')
            stage = strict_json(raw, RECEIPT_LIMIT, 20000)
            require(raw == canonical(stage), 'stage_receipt_invalid')
            validate_stage_receipt(stage, plan)
        secret = (os.environ if environ is None else environ).get('MIR2_R2_UPLOAD_SECRET')
        require(secret is not None and secret != '', 'secret_missing')
        require(isinstance(secret, str) and 1 <= len(secret) <= 4096
                and re.fullmatch(r'[A-Za-z0-9._~+/-]+=*', secret), 'secret_invalid')
        receipt = deliver(mode, plan, FixedHttpsTransport(plan) if transport is None else transport,
                          secret, stage=stage, stage_sha=stage_sha)
    except Fault as failure:
        receipt['result'] = failure.code
    except BaseException:
        receipt['result'] = 'internal_error'
    if stream is not None:
        try:
            validate_output_handle(stream)
            receipt['receiptStatus'] = 'written'
            raw = canonical(receipt)
            require(len(raw) <= RECEIPT_LIMIT, 'receipt_io_failed')
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
            validate_output_handle(stream)
            stream.close()
            stream = None
        except BaseException:
            receipt.update(result='receipt_io_failed', passed=False, receiptStatus='not_written')
        finally:
            if stream is not None:
                try:
                    stream.close()
                except BaseException:
                    pass
    try:
        target = sys.stdout if output is None else output
        summary = {'schema': receipt['schema'], 'mode': receipt['mode'], 'passed': receipt['passed'],
            'result': receipt['result'], 'receiptStatus': receipt['receiptStatus'],
            'verifiedObjectCount': receipt['verifiedObjectCount'],
            'pointerOutcomeUnknown': receipt['pointerOutcomeUnknown'],
            'objectOutcomeUnknown': receipt['objectOutcomeUnknown']}
        target.write(canonical(summary).decode('utf-8'))
        target.flush()
    except BaseException:
        return 2
    return 0 if receipt['passed'] is True and receipt['receiptStatus'] == 'written' else 2


if __name__ == '__main__':
    raise SystemExit(main())
