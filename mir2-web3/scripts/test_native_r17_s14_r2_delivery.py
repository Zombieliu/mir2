"""Offline policy fixtures: no sockets, credentials, CMS verification or R2.

The literal14 table and real two small feed bytes are used. Modeled stage receipt
shapes are ONLY validator inputs and never written as actual publication proof.
Actual root/CMS admission is bound; prepared13 negative fixtures stay frozen.
No admission function is patched.
"""
from __future__ import annotations

import copy
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native13_policy_fixture', BASE / 'scripts/native_r17_s14_r2_delivery.py')
P = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = P
SPEC.loader.exec_module(P)
FIXTURE_ROOT = BASE / 'scripts/fixtures/native-r17-s14'
PREPARED_ROOT = BASE / 'scripts/fixtures/native-r17-13/prepared-snapshot-01'
PREPARED_SPEC = importlib.util.spec_from_file_location('immutable_prepared13_fixture',
    PREPARED_ROOT / 'scripts/native_r17_r2_delivery.py')
PREPARED_P = importlib.util.module_from_spec(PREPARED_SPEC)
sys.modules[PREPARED_SPEC.name] = PREPARED_P
PREPARED_SPEC.loader.exec_module(PREPARED_P)
PREPARED_RAW = PREPARED_P.PLAN_FILE.read_bytes()
RAW = P.PLAN_FILE.read_bytes()
PLAN = P.read_prepared_plan_bytes(RAW)  # Structural proof only, NOT load_plan_bytes admission.
FIXTURES = FIXTURE_ROOT / 'preview/artifacts/feed'
FEED = (FIXTURES / 'latest.json').read_bytes()
CMS = (FIXTURES / 'latest.p7s').read_bytes()
FAKE_SECRET = 'public_fake_fixture_secret_only'
NOW = json.loads(FEED)['createdUnix'] + 100


def private_base():
    return {'cache-control': 'no-store', 'x-content-type-options': 'nosniff',
        'x-mir2-native-plan-sha256': P.NATIVE_SHA,
        'x-mir2-source-plan-sha256': P.SOURCE_PLAN_SHA or 'unadmitted',
        'x-mir2-root-verification-sha256': P.ROOT_SHA or 'unadmitted', 'x-mir2-objects-sha256': P.OBJECTS_SHA}


def private_object(entry):
    return {**private_base(), 'content-length': str(entry['size']), 'content-type': entry['contentType'],
        'etag': '"unit-etag-only"', 'x-mir2-stored-sha256': entry['sha256'],
        'x-mir2-custom-sha256': entry['sha256'], 'x-mir2-stored-cache-control': entry['cacheControl'],
        'x-mir2-stored-content-encoding': 'identity'}


def public_object(entry, candidate=None):
    headers = {'content-length': str(entry['size']), 'content-type': P.public_mime(entry['path']),
        'cache-control': 'no-store' if candidate else P.IMMUTABLE,
        'etag': '"unit-public-etag"', 'accept-ranges': 'bytes', 'x-content-type-options': 'nosniff',
        'access-control-allow-origin': '*', 'x-mir2-native-cache': 'BYPASS' if candidate else 'MISS'}
    if candidate:
        directory = 'feeds/s' + str(candidate['sequence']) + '-' + candidate['feedSha256'] + '/'
        headers.update({'x-mir2-sequence': str(candidate['sequence']),
            'x-mir2-feed-sha256': candidate['feedSha256'], 'x-mir2-feed-path': directory + 'latest.json',
            'x-mir2-signature-path': directory + 'latest.p7s'})
    return headers


class Response:
    def __init__(self, status, headers, body=b'', error=None):
        self.status, self.headers, self.body, self.error = status, headers, body, error
        self.offset, self.closed = 0, False
    def getheaders(self):
        return list(self.headers.items())
    def read(self, maximum):
        assert 0 < maximum <= P.CHUNK
        if self.error:
            raise self.error
        raw = self.body[self.offset:self.offset + maximum]
        self.offset += len(raw)
        return raw
    def close(self):
        self.closed = True


class TinyTransport:
    """Only real feed/CMS have body bytes. No whole36 or CMS success is modeled."""
    def __init__(self, current=None):
        self.current, self.calls, self.records = current, [], set()
        self.lose_import_ack, self.lose_promote_ack = False, False
        self.pointer_race, self.promote_error = False, None
    def open(self, method, path, *, token=None, body=None):
        self.calls.append((method, path))
        if path == P.PRIVATE_PREFIX + '/pointer':
            assert method == 'GET' and token == FAKE_SECRET and body is None
            if self.current is None:
                return Response(404, private_base())
            raw = P.canonical(self.current)
            return Response(200, private_object(P.pointer_entry(PLAN, self.current)), raw)
        if path.startswith(P.PRIVATE_PREFIX + '/objects/'):
            index = int(path.rsplit('/', 1)[1])
            assert method == 'HEAD' and token == FAKE_SECRET
            return Response(200 if index in self.records else 404,
                private_object(PLAN['objects'][index]) if index in self.records else private_base())
        if path == P.PRIVATE_PREFIX + '/import':
            index = P.strict_json(body)['index']
            if index > 1:
                raise P.Fault('request_deadline')
            assert token == FAKE_SECRET and method == 'POST'
            was_present = index in self.records
            self.records.add(index)
            if self.lose_import_ack:
                raise P.Fault('request_deadline')
            entry = PLAN['objects'][index]
            reply = {'ok': True, 'mode': 'import', 'index': index, 'path': entry['path'],
                'size': entry['size'], 'sha256': entry['sha256'], 'resumed': was_present, 'pointerChanged': False}
            return Response(200 if was_present else 201,
                {**private_base(), 'content-type': 'application/json'}, P.canonical(reply))
        if path == P.PRIVATE_PREFIX + '/promote':
            assert method == 'POST' and token == FAKE_SECRET
            envelope = P.strict_json(body)
            assert envelope == P.expected_envelope(PLAN, envelope['expectedCurrent'])
            if self.pointer_race or self.promote_error:
                flags = {'pointerAttempted': self.pointer_race, 'pointerChanged': False,
                    'pointerOutcomeUnknown': False, 'objectWriteAttempted': False, 'objectOutcomeUnknown': False}
                return Response(412 if self.pointer_race else 409,
                    {**private_base(), 'content-type': 'application/json'},
                    P.canonical({'ok': False, 'error': self.promote_error or 'pointer_compare_and_swap_failed', **flags}))
            already = self.current == PLAN['candidate']
            self.current = copy.deepcopy(PLAN['candidate'])
            if self.lose_promote_ack:
                raise P.Fault('request_deadline')
            reply = {'ok': True, 'mode': 'promote', 'candidate': PLAN['candidate'],
                'pointerAttempted': not already, 'pointerChanged': not already, 'pointerOutcomeUnknown': False,
                'objectWriteAttempted': False, 'objectOutcomeUnknown': False,
                'alreadyPromoted': already, 'privatePointerVerified': True,
                'aliasesVerified': False, 'publicVerificationRequired': True}
            return Response(200, {**private_base(), 'content-type': 'application/json'}, P.canonical(reply))
        for index, raw in enumerate((FEED, CMS)):
            entry = PLAN['objects'][index]
            if path == P.PUBLIC_PREFIX + entry['path']:
                assert method == 'GET' and token is None
                return Response(200, public_object(entry), raw)
        raise AssertionError('No arbitrary endpoint or fabricated large bytes allowed')


def modeled_stage_input(current=None):
    """Synthetic schema-validator input, never proof of real streams/publication."""
    receipt = P.empty_receipt('stage')
    receipt.update(passed=True, result='ok', candidate=PLAN['candidate'], observedCurrent=current,
        verifiedObjects=P.closure(PLAN), verifiedObjectCount=36,
        publicVerifiedBytes=sum(e['size'] for e in PLAN['objects']), publicVerified=True,
        stageEnvelope=P.expected_envelope(PLAN, current), receiptStatus='written',
        objectWriteAttempted=True, objectOutcomeUnknown=False)
    calls = [{'method': 'GET', 'route': '/pointer', 'index': None,
        'status': 404 if current is None else 200,
        'receivedBytes': 0 if current is None else P.pointer_entry(PLAN, current)['size']}]
    for entry in PLAN['objects']:
        index = entry['index']
        calls.extend([
            {'method': 'HEAD', 'route': '/objects/' + str(index), 'index': index, 'status': 404, 'receivedBytes': 0},
            {'method': 'POST', 'route': '/import', 'index': index, 'status': 201, 'receivedBytes': 200},
            {'method': 'HEAD', 'route': '/objects/' + str(index), 'index': index, 'status': 200, 'receivedBytes': 0},
            {'method': 'GET', 'route': 'public', 'index': index, 'status': 200, 'receivedBytes': entry['size']},
        ])
    receipt['calls'] = [{**c, 'headersVerified': True, 'complete': True,
        'writeAttempted': c['method'] == 'POST', 'elapsedMs': 1} for c in calls]
    return receipt


class PreviewPreparationTests(unittest.TestCase):
    def test_exact13_table_real_feed_and_original30_game_hashes(self):
        self.assertEqual(P.digest(RAW), P.LITERAL_SHA)
        self.assertEqual(P.digest(P.canonical(PLAN)), P.NATIVE_SHA)
        self.assertEqual(P.digest(FEED), PLAN['objects'][0]['sha256'])
        self.assertEqual(P.digest(CMS), PLAN['objects'][1]['sha256'])
        self.assertEqual(PLAN['candidate']['sequence'], 14)
        self.assertEqual(len(PLAN['objects']), 36)
        self.assertEqual(sum(e['size'] for e in PLAN['objects']), 817841942)
        self.assertEqual(sum(e['path'].startswith('releases/game-') for e in PLAN['objects']), 30)

    def test_prepared_default_gate_and_old_proof_cannot_admit13(self):
        with self.assertRaisesRegex(PREPARED_P.Fault, 'prepared_release_unadmitted'):
            PREPARED_P.load_plan_bytes(PREPARED_RAW)
        altered = copy.deepcopy(PLAN)
        altered.update(admission='admitted', rootVerificationBase64=PLAN['predecessorProof']['rootVerificationBase64'],
            rootVerificationSha256=PLAN['predecessorProof']['rootVerificationSha256'])
        with self.assertRaisesRegex(P.Fault, 'plan_invalid'):
            P.load_plan_bytes(P.canonical(altered))

    def test_cli_blocks_before_secret_lookup_transport_or_network(self):
        class SecretTripwire(dict):
            def get(self, *_args):
                raise AssertionError('Do not read credentials before real admission')
        class TransportTripwire:
            def open(self, *_args, **_kwargs):
                raise AssertionError('No network call allowed')
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / 'refusal.json'
            status = PREPARED_P.main(['--mode', 'stage', '--receipt', str(target)],
                environ=SecretTripwire(), transport=TransportTripwire(), output=io.StringIO())
            data = P.strict_json(target.read_bytes(), P.RECEIPT_LIMIT)
            self.assertEqual(status, 2)
            self.assertEqual(data['result'], 'prepared_release_unadmitted')
            self.assertFalse(data['passed']); self.assertEqual(data['calls'], [])
            self.assertFalse(data['cmsReverified']); self.assertFalse(data['pointerAttempted'])

    def test_hash_mismatch_and_duplicate_key_fail_closed(self):
        changed = copy.deepcopy(PLAN); changed['objects'][35]['size'] += 1
        for raw in (P.canonical(changed), RAW + b' ', b'{"admission":"prepared","admission":"admitted"}'):
            with self.assertRaises(P.Fault):
                P.read_prepared_plan_bytes(raw)

    def test_new_alias_sequence_headers_require13(self):
        headers = public_object(PLAN['objects'][0], PLAN['candidate'])
        P.public_headers(headers, PLAN['objects'][0], PLAN['candidate'])
        headers['x-mir2-sequence'] = '13'
        with self.assertRaisesRegex(P.Fault, 'headers_rejected'):
            P.public_headers(headers, PLAN['objects'][0], PLAN['candidate'])

    def test_fixed_transport_deadlines_and_no_arbitrary_endpoint(self):
        seen = []
        def response(*args):
            seen.append(args)
            return object()
        transport = P.FixedHttpsTransport(PLAN)
        with patch.object(P, 'NetworkResponse', response):
            transport.open('GET', P.PRIVATE_PREFIX + '/pointer', token=FAKE_SECRET)
            transport.open('POST', P.PRIVATE_PREFIX + '/import', token=FAKE_SECRET, body=P.canonical({'index': 0}))
            transport.open('POST', P.PRIVATE_PREFIX + '/promote', token=FAKE_SECRET,
                body=P.canonical(P.expected_envelope(PLAN, PLAN['knownPrevious'])))
            transport.open('GET', P.PUBLIC_PREFIX + PLAN['objects'][0]['path'])
            self.assertEqual([args[-1] for args in seen], [150, 2460, 360, 2460])
            for method, path, body in [('POST', P.PRIVATE_PREFIX + '/import', b'{"index":0,"url":"https://x"}'),
                ('GET', '/other', None), ('GET', P.PRIVATE_PREFIX + '/objects/00', None),
                ('POST', P.PRIVATE_PREFIX + '/promote', b'{"expectedCurrent":{"sequence":11}}')]:
                with self.assertRaises(P.Fault):
                    transport.open(method, path, token=FAKE_SECRET, body=body)
        self.assertEqual(P.CHUNK, 65536)
        self.assertGreater(P.IMPORT_CONTROL_SECONDS * 1000, PLAN['operationPolicy']['importMs'])
        self.assertFalse(PLAN['operationPolicy']['measuredSpeed'])

    def test_only_actual_null_exact12_exact13_pointer_reads_are_allowed(self):
        for value in (None, PLAN['knownPrevious'], PLAN['candidate']):
            transport = TinyTransport(value)
            receipt = P.empty_receipt('stage')
            self.assertEqual(P.check_pointer(transport, receipt, PLAN, FAKE_SECRET), value)
        for value in ({**PLAN['knownPrevious'], 'sequence': 11},
            {**PLAN['knownPrevious'], 'feedSha256': '0' * 64}):
            with self.assertRaisesRegex(P.Fault, 'pointer_rejected'):
                P.check_pointer(TinyTransport(value), P.empty_receipt('stage'), PLAN, FAKE_SECRET)

    def test_partial_actual_feed_streams_do_not_mint36_object_pass(self):
        transport = TinyTransport(PLAN['knownPrevious'])
        with patch.object(P.time, 'time', return_value=NOW):
            receipt = P.deliver('stage', PLAN, transport, FAKE_SECRET)
        self.assertFalse(receipt['passed'])
        self.assertEqual(receipt['verifiedObjectCount'], 2)
        self.assertEqual(receipt['publicVerifiedBytes'], len(FEED) + len(CMS))
        self.assertEqual(receipt['observedCurrent'], PLAN['knownPrevious'])
        self.assertIsNone(receipt['stageEnvelope'])
        self.assertTrue(receipt['objectOutcomeUnknown'])
        self.assertFalse(receipt['pointerAttempted'])
        self.assertEqual(sum(path == P.PRIVATE_PREFIX + '/import' for _, path in transport.calls), 3)

    def test_lost_import_ack_keeps_unknown_without_blind_retry_or_raw_fallback(self):
        transport = TinyTransport(); transport.lose_import_ack = True
        receipt = P.deliver('stage', PLAN, transport, FAKE_SECRET)
        self.assertFalse(receipt['passed']); self.assertTrue(receipt['objectOutcomeUnknown'])
        self.assertTrue(receipt['objectWriteAttempted'])
        self.assertEqual(transport.records, {0})
        self.assertEqual(sum(path == P.PRIVATE_PREFIX + '/import' for _, path in transport.calls), 1)
        self.assertFalse(any(path.startswith(P.PUBLIC_PREFIX) for _, path in transport.calls))

    def test_modeled_stage_schema_binds_observed12_and_rejects_old_or_forged_receipts(self):
        for current in (None, PLAN['knownPrevious'], PLAN['candidate']):
            P.validate_stage_receipt(modeled_stage_input(current), PLAN)
        base = modeled_stage_input(PLAN['knownPrevious'])
        mutations = [
            lambda r: r.update(schema='mir2.windows.r2-native-ci.v1'),
            lambda r: r['stageEnvelope'].update(schema='mir2.windows.r2-native-stage.v1'),
            lambda r: r.update(observedCurrent=None),
            lambda r: r.update(objectOutcomeUnknown=True),
            lambda r: r['calls'][0].update(status=404, receivedBytes=0),
            lambda r: r['calls'][4].update(receivedBytes=0),
            lambda r: r['bindings'].update(rootVerificationSha256=PLAN['predecessorProof']['rootVerificationSha256']),
        ]
        for change in mutations:
            other = copy.deepcopy(base); change(other)
            with self.assertRaises(P.Fault):
                P.validate_stage_receipt(other, PLAN)

    def test_pointer_race_response_prevents_success_and_no_retry(self):
        transport = TinyTransport(PLAN['knownPrevious']); transport.pointer_race = True
        receipt = P.empty_receipt('promote')
        with self.assertRaises(P.Fault):
            P.promote_pointer(transport, receipt, PLAN, FAKE_SECRET, modeled_stage_input(PLAN['knownPrevious']))
        self.assertTrue(receipt['pointerAttempted']); self.assertFalse(receipt['pointerOutcomeUnknown'])
        self.assertEqual(transport.current, PLAN['knownPrevious'])
        self.assertEqual(sum(path.endswith('/promote') for _, path in transport.calls), 1)

    def test_lost_promote_ack_unknown_then_independent_reconcile_without_second_write(self):
        transport = TinyTransport(PLAN['knownPrevious']); transport.lose_promote_ack = True
        stage = modeled_stage_input(PLAN['knownPrevious']); receipt = P.empty_receipt('promote')
        with self.assertRaisesRegex(P.Fault, 'request_deadline'):
            P.promote_pointer(transport, receipt, PLAN, FAKE_SECRET, stage)
        self.assertTrue(receipt['pointerOutcomeUnknown']); self.assertEqual(transport.current, PLAN['candidate'])
        self.assertEqual(sum(path.endswith('/promote') for _, path in transport.calls), 1)
        # A later separate call verifies an exact already-promoted13 response.
        # This is a transport model, NOT actual R2 authority or replay acceptance.
        transport.lose_promote_ack = False
        other = P.empty_receipt('promote')
        P.promote_pointer(transport, other, PLAN, FAKE_SECRET, stage)
        self.assertFalse(other['pointerAttempted']); self.assertFalse(other['pointerOutcomeUnknown'])
        self.assertTrue(other['privatePointerVerified'])

    def test_current_origin12_error_cannot_pass_promote_even_with_immutable13_stage(self):
        transport = TinyTransport(PLAN['knownPrevious']); transport.promote_error = 'origin_feed_pair_changed'
        receipt = P.empty_receipt('promote')
        with self.assertRaises(P.Fault):
            P.promote_pointer(transport, receipt, PLAN, FAKE_SECRET, modeled_stage_input(PLAN['knownPrevious']))
        self.assertFalse(receipt['pointerAttempted']); self.assertFalse(receipt['pointerOutcomeUnknown'])
        self.assertEqual(transport.current, PLAN['knownPrevious'])

    def test_changed_pointer_since_stage_rejects_before_send(self):
        transport = TinyTransport(PLAN['knownPrevious'])
        with self.assertRaisesRegex(P.Fault, 'stage_pointer_changed'):
            P.promote_pointer(transport, P.empty_receipt('promote'), PLAN, FAKE_SECRET, modeled_stage_input(None))
        self.assertFalse(any(path.endswith('/promote') for _, path in transport.calls))

    def test_feed_expiry_and_signature_pair_binding_stay_separate_from_cms(self):
        with patch.object(P.time, 'time', return_value=NOW):
            P.verify_feed(FEED, PLAN)
        with patch.object(P.time, 'time', return_value=json.loads(FEED)['expiresUnix']):
            with self.assertRaisesRegex(P.Fault, 'feed_expired_or_invalid'):
                P.verify_feed(FEED, PLAN)
        self.assertFalse(P.empty_receipt('stage')['cmsReverified'])



# Retained original46 protected-path/TLS/stream/receipt regressions; modeled remaining34.
"""Offline CI delivery tests. All secrets/transports/stores are synthetic.

The real literal plan and two tiny source-feed objects are bound. Other object
bytes are a toy internal table, never a successful R2/CDN/source36 publication.
"""

import base64
import copy
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import ssl
import sys
import tempfile
import threading
import time
import tracemalloc
import types
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('native_r17_delivery_fixture', BASE / 'scripts/native_r17_s14_r2_delivery.py')
P = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = P
SPEC.loader.exec_module(P)
INPUT_PLAN = P.PLAN_FILE
PLAN_RAW = INPUT_PLAN.read_bytes()
REAL_PLAN = P.load_plan_bytes(PLAN_RAW)
FEED = (FIXTURE_ROOT / 'preview/artifacts/feed/latest.json').read_bytes()
CMS = (FIXTURE_ROOT / 'preview/artifacts/feed/latest.p7s').read_bytes()
MOCK_SECRET = 'MOCK_SECRET_ONLY_c41d60ff813abdec'
BODY_MARKER = 'MOCK_HTTP_BODY_ONLY_b887139fe44fbe0a'
ENV = {'MIR2_R2_UPLOAD_SECRET': MOCK_SECRET}
FIXTURES = BASE / 'evidence/fixtures'
RETAINED = []


def private_base():
    return {'cache-control': 'no-store', 'x-content-type-options': 'nosniff',
        'x-mir2-native-plan-sha256': P.NATIVE_SHA, 'x-mir2-source-plan-sha256': P.SOURCE_PLAN_SHA,
        'x-mir2-root-verification-sha256': P.ROOT_SHA, 'x-mir2-objects-sha256': P.OBJECTS_SHA}


def private_object(entry):
    return {**private_base(), 'content-length': str(entry['size']), 'content-type': entry['contentType'],
        'etag': '"offline-etag"', 'x-mir2-stored-sha256': entry['sha256'],
        'x-mir2-custom-sha256': entry['sha256'], 'x-mir2-stored-cache-control': entry['cacheControl'],
        'x-mir2-stored-content-encoding': 'identity'}


def public_object(entry, candidate=None):
    result = {'cache-control': 'no-store' if candidate else P.IMMUTABLE,
        'content-length': str(entry['size']), 'content-type': P.public_mime(entry['path']),
        'etag': '"offline-etag"', 'accept-ranges': 'bytes', 'x-content-type-options': 'nosniff',
        'access-control-allow-origin': '*', 'x-mir2-native-cache': 'BYPASS' if candidate else 'MISS'}
    if candidate:
        directory = 'feeds/s' + str(candidate['sequence']) + '-' + candidate['feedSha256'] + '/'
        result.update({'x-mir2-sequence': str(candidate['sequence']), 'x-mir2-feed-sha256': candidate['feedSha256'],
            'x-mir2-feed-path': directory + 'latest.json', 'x-mir2-signature-path': directory + 'latest.p7s'})
    return result


class Response:
    def __init__(self, status=200, headers=None, body=b'', *, error=None, close_error=None):
        self.status, self.headers, self.body = status, headers or {}, body
        self.offset, self.reads, self.closed = 0, [], False
        self.error, self.close_error = error, close_error
    def getheaders(self):
        return list(self.headers.items()) if isinstance(self.headers, dict) else self.headers
    def read(self, maximum):
        assert 0 < maximum <= P.CHUNK, 'no unbounded read'
        self.reads.append(maximum)
        if self.error:
            raise self.error
        chunk = self.body[self.offset:self.offset + maximum]
        self.offset += len(chunk)
        return chunk
    def close(self):
        self.closed = True
        if self.close_error:
            raise self.close_error


class StoreTransport:
    """Tiny dependency-injected model; CLI cannot select this table."""
    def __init__(self):
        self.plan = copy.deepcopy(REAL_PLAN)
        self.bodies, self.stored, self.pointer = {}, set(), False
        for entry in self.plan['objects']:
            index = entry['index']
            raw = FEED if index == 0 else CMS if index == 1 else ('toy-only-object-' + str(index)).encode()
            if index >= 2:
                entry['size'], entry['sha256'] = len(raw), P.digest(raw)
            self.bodies[index] = raw
        self.calls, self.responses, self.failures = [], [], {}
    def open(self, method, path, *, token=None, body=None):
        public = path.startswith(P.PUBLIC_PREFIX)
        if public:
            assert token is None, 'never bearer to public route'
        else:
            assert token == MOCK_SECRET, 'only fixed private bearer'
        self.calls.append({'method': method, 'path': path, 'body': body})
        key = (method, path)
        failure = self.failures.get(key)
        if isinstance(failure, BaseException):
            raise failure
        if path == P.PRIVATE_PREFIX + '/pointer':
            if self.pointer:
                response = Response(headers=private_object(P.pointer_entry(self.plan)), body=P.canonical(self.plan['candidate']))
            else:
                response = Response(status=404, headers=private_base())
        elif path.startswith(P.PRIVATE_PREFIX + '/objects/'):
            index = int(path.rsplit('/', 1)[1])
            if index in self.stored:
                response = Response(headers=private_object(self.plan['objects'][index]),
                    body=self.bodies[index] if method == 'GET' else b'')
            else:
                response = Response(status=404, headers=private_base())
        elif path == P.PRIVATE_PREFIX + '/import':
            request = P.strict_json(body)
            assert set(request) == {'index'} and type(request['index']) is int
            index = request['index']; entry = self.plan['objects'][index]
            resumed = index in self.stored
            self.stored.add(index)
            reply = {'ok': True, 'mode': 'import', 'index': index, 'path': entry['path'],
                'size': entry['size'], 'sha256': entry['sha256'], 'resumed': resumed, 'pointerChanged': False}
            response = Response(status=200 if resumed else 201,
                headers={**private_base(), 'content-type': 'application/json; charset=utf-8'}, body=P.canonical(reply))
        elif path == P.PRIVATE_PREFIX + '/promote':
            assert P.strict_json(body) == P.expected_envelope(self.plan, P.strict_json(body)['expectedCurrent']), 'POST direct envelope, not wrapper'
            already = self.pointer
            self.pointer = True
            reply = {'ok': True, 'mode': 'promote', 'candidate': self.plan['candidate'],
                'pointerAttempted': not already, 'pointerChanged': not already, 'pointerOutcomeUnknown': False,
                'objectWriteAttempted': False, 'objectOutcomeUnknown': False,
                'alreadyPromoted': already, 'privatePointerVerified': True,
                'aliasesVerified': False, 'publicVerificationRequired': True}
            response = Response(headers={**private_base(), 'content-type': 'application/json; charset=utf-8'}, body=P.canonical(reply))
        elif path in (P.PUBLIC_PREFIX + 'latest.json', P.PUBLIC_PREFIX + 'latest.p7s'):
            assert self.pointer, 'no aliases before pointer'
            index = 0 if path.endswith('.json') else 1
            response = Response(headers=public_object(self.plan['objects'][index], self.plan['candidate']), body=self.bodies[index])
        elif public:
            entry = next(e for e in self.plan['objects'] if path == P.PUBLIC_PREFIX + e['path'])
            assert entry['index'] in self.stored, 'no public object before import'
            response = Response(headers=public_object(entry), body=self.bodies[entry['index']] if method == 'GET' else b'')
        else:
            raise AssertionError('unexpected nonfixed route')
        if callable(failure):
            response = failure(response)
        self.responses.append(response)
        return response


class FakeSocket:
    def __init__(self):
        self.timeouts, self.shutdowns = [], []
        self.cancelled = threading.Event()
    def settimeout(self, value):
        self.timeouts.append(value)
    def shutdown(self, how):
        self.shutdowns.append(how)
        self.cancelled.set()


class FakeConnection:
    def __init__(self, response, *, wait_headers=False, failure=None, close_failure=False):
        self.response, self.wait_headers, self.failure = response, wait_headers, failure
        self.close_failure, self.sock, self.requests = close_failure, FakeSocket(), []
        self.closed, self.debug, self.connected = False, None, False
    def set_debuglevel(self, value):
        self.debug = value
    def connect(self):
        self.connected = True
    def request(self, method, path, *, body=None, headers=None):
        self.requests.append((method, path, body, headers))
        if self.failure:
            raise self.failure
    def getresponse(self):
        if self.wait_headers:
            self.sock.cancelled.wait(2)
            raise OSError(MOCK_SECRET)
        return self.response
    def close(self):
        self.closed = True
        if self.close_failure:
            raise OSError(MOCK_SECRET + BODY_MARKER)


class DeliveryTests(unittest.TestCase):
    def fixture(self, label):
        FIXTURES.mkdir(parents=True, exist_ok=True)
        path = Path(tempfile.mkdtemp(prefix=label + '-', dir=FIXTURES))
        RETAINED.append({'label': label, 'directory': str(path)})
        return path
    def safe(self, receipt):
        text = P.canonical(receipt).decode()
        for marker in (MOCK_SECRET, BODY_MARKER, 'Bearer ', 'Authorization', 'Traceback'):
            self.assertNotIn(marker, text)
        self.assertIn(receipt['result'], P.SAFE_CODES)
        self.assertIs(receipt['cmsReverified'], False)
        self.assertIs(receipt['networkAuthorityAssumed'], False)
        self.assertIs(receipt['deploymentPerformed'], False)
    def stage(self, transport=None):
        transport = StoreTransport() if transport is None else transport
        receipt = P.deliver('stage', transport.plan, transport, MOCK_SECRET)
        self.safe(receipt)
        return receipt, transport
    def passed_stage(self, transport):
        stage, _ = self.stage(transport)
        self.assertTrue(stage['passed'])
        stage['receiptStatus'] = 'written'
        return stage
    def promote(self, transport=None):
        transport = StoreTransport() if transport is None else transport
        stage = self.passed_stage(transport)
        transport.calls.clear()
        receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET,
            stage=stage, stage_sha=P.digest(P.canonical(stage)))
        self.safe(receipt)
        return receipt, transport
    def run_cli(self, root, *, mode='stage', env=ENV, transport=None, args=None, plan_loader=None):
        transport = StoreTransport() if transport is None else transport
        output = io.StringIO()
        if args is None:
            args = ['--mode', mode, '--receipt', str(root / 'receipt.json')]
        with patch.object(P, 'PLAN_FILE', INPUT_PLAN), patch.object(P, 'load_plan_bytes',
            side_effect=plan_loader or (lambda raw: transport.plan if P.digest(raw) == P.LITERAL_SHA else P.load_plan_bytes(raw))):
            code = P.main(args, environ=env, transport=transport, output=output)
        summary = json.loads(output.getvalue())
        self.assertEqual(len(output.getvalue().splitlines()), 1)
        self.assertNotIn(MOCK_SECRET, output.getvalue())
        return code, summary, transport

    def test_literal_plan_and_two_real_tiny_objects_bound(self):
        self.assertEqual(P.digest(PLAN_RAW), P.LITERAL_SHA)
        self.assertEqual(P.digest(P.canonical(REAL_PLAN)), P.NATIVE_SHA)
        self.assertEqual(len(REAL_PLAN['objects']), 36)
        self.assertEqual(sum(e['size'] for e in REAL_PLAN['objects']), 817841942)
        self.assertEqual(P.digest(FEED), REAL_PLAN['objects'][0]['sha256'])
        self.assertEqual(P.digest(CMS), REAL_PLAN['objects'][1]['sha256'])

    def test_stage_consumes_full_public_every_object_before_envelope(self):
        receipt, transport = self.stage()
        self.assertTrue(receipt['passed'])
        gets = [c for c in transport.calls if c['method'] == 'GET' and c['path'].startswith(P.PUBLIC_PREFIX)]
        self.assertEqual(len(gets), 36)
        self.assertEqual([c['path'] for c in gets], [P.PUBLIC_PREFIX + e['path'] for e in transport.plan['objects']])
        self.assertEqual(receipt['publicVerifiedBytes'], sum(e['size'] for e in transport.plan['objects']))
        self.assertEqual(receipt['stageEnvelope'], P.expected_envelope(transport.plan))
        self.assertFalse(any(c['path'].endswith('/promote') for c in transport.calls))
        self.assertEqual(receipt['verifiedObjects'], P.closure(transport.plan))

    def test_stage_corrupt_public_bytes_cannot_template_pass(self):
        transport = StoreTransport()
        entry = transport.plan['objects'][0]
        transport.failures[('GET', P.PUBLIC_PREFIX + entry['path'])] = lambda r: Response(headers=r.headers, body=b'X' + r.body[1:])
        receipt, _ = self.stage(transport)
        self.assertFalse(receipt['passed'])
        self.assertEqual(receipt['result'], 'body_hash_rejected')
        self.assertIsNone(receipt['stageEnvelope'])
        self.assertEqual(receipt['verifiedObjectCount'], 0)

    def test_stage_partial_length_failure_retains_actual_bytes_and_no_envelope(self):
        transport = StoreTransport(); entry = transport.plan['objects'][1]
        transport.failures[('GET', P.PUBLIC_PREFIX + entry['path'])] = lambda r: Response(headers=r.headers, body=r.body[:-1])
        receipt, _ = self.stage(transport)
        self.assertFalse(receipt['passed'])
        self.assertEqual(receipt['verifiedObjectCount'], 1)
        self.assertEqual(receipt['calls'][-1]['receivedBytes'], entry['size'] - 1)
        self.assertIsNone(receipt['stageEnvelope'])
        self.assertFalse(receipt['pointerAttempted'])

    def test_stage_resume_still_checks_private_and_full_public(self):
        transport = StoreTransport(); transport.stored.update(range(36))
        receipt, _ = self.stage(transport)
        self.assertTrue(receipt['passed'])
        heads = [c for c in transport.calls if c['method'] == 'HEAD' and '/objects/' in c['path']]
        self.assertGreaterEqual(len(heads), 36)
        self.assertEqual(len([c for c in transport.calls if c['method'] == 'GET' and c['path'].startswith(P.PUBLIC_PREFIX)]), 36)
        self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 36)

    def test_private_plan_header_change_stops_before_any_write(self):
        transport = StoreTransport()
        def changed(response):
            response.headers['x-mir2-native-plan-sha256'] = '0' * 64
            return response
        transport.failures[('GET', P.PRIVATE_PREFIX + '/pointer')] = changed
        receipt, _ = self.stage(transport)
        self.assertFalse(receipt['passed'])
        self.assertEqual(receipt['result'], 'headers_rejected')
        self.assertEqual(len(transport.calls), 1)
        self.assertEqual(transport.stored, set())

    def test_pointer_exact_candidate_permits_stage_but_other_pointer_refuses(self):
        transport = StoreTransport(); transport.pointer = True
        receipt, _ = self.stage(transport)
        self.assertTrue(receipt['passed'])
        transport = StoreTransport(); transport.pointer = True
        transport.failures[('GET', P.PRIVATE_PREFIX + '/pointer')] = lambda r: Response(headers=r.headers, body=b'X' + r.body[1:])
        receipt, _ = self.stage(transport)
        self.assertFalse(receipt['passed'])
        self.assertEqual(transport.stored, set())

    def test_private_resume_metadata_corruption_stops_before_post(self):
        for key in ('x-mir2-stored-sha256', 'x-mir2-custom-sha256', 'x-mir2-stored-cache-control',
                    'x-mir2-stored-content-encoding', 'content-length', 'content-type', 'etag'):
            with self.subTest(key=key):
                transport = StoreTransport(); transport.stored.add(0)
                def changed(response, key=key):
                    response.headers[key] = BODY_MARKER
                    return response
                transport.failures[('HEAD', P.PRIVATE_PREFIX + '/objects/0')] = changed
                receipt, _ = self.stage(transport)
                self.assertFalse(receipt['passed'])
                self.assertFalse(any(c['method'] == 'POST' for c in transport.calls))

    def test_import_forged_or_malformed_reply_not_accepted(self):
        for body in (b'{"ok":true}', b'{"ok":true,"ok":true}', b'\xef\xbb\xbf{}',
                     b'{}' + BODY_MARKER.encode(), P.canonical({'error': MOCK_SECRET})):
            with self.subTest(body=len(body)):
                transport = StoreTransport()
                transport.failures[('POST', P.PRIVATE_PREFIX + '/import')] = lambda r, body=body: Response(status=201, headers=r.headers, body=body)
                receipt, _ = self.stage(transport)
                self.assertFalse(receipt['passed'])
                self.assertIsNone(receipt['stageEnvelope'])
                self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)

    def test_stage_401_and_412_stop_without_retry_or_pointer_write(self):
        for status in (401, 412):
            transport = StoreTransport()
            transport.failures[('POST', P.PRIVATE_PREFIX + '/import')] = lambda r, status=status: Response(status, r.headers, MOCK_SECRET.encode())
            receipt, _ = self.stage(transport)
            self.assertFalse(receipt['passed'])
            self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)
            self.assertFalse(receipt['pointerAttempted'])
            self.assertFalse(transport.pointer)

    def test_public_headers_reject_partial_encoding_cache_and_length(self):
        for key, value in (('content-length', '01'), ('cache-control', 'no-store'),
                           ('content-encoding', 'gzip'), ('content-range', 'bytes 0-1/1159'),
                           ('etag', 'W/"weak"'), ('content-type', 'text/html'),
                           ('accept-ranges', 'none'), ('x-mir2-native-cache', 'BYPASS')):
            transport = StoreTransport(); entry = transport.plan['objects'][0]
            def changed(response, key=key, value=value):
                response.headers[key] = value
                return response
            transport.failures[('GET', P.PUBLIC_PREFIX + entry['path'])] = changed
            receipt, _ = self.stage(transport)
            self.assertFalse(receipt['passed'])
            self.assertEqual(receipt['result'], 'headers_rejected')

    def test_public_206_and_redirect_refuse_without_following(self):
        for status in (206, 301, 302, 307, 308):
            transport = StoreTransport(); entry = transport.plan['objects'][0]
            transport.failures[('GET', P.PUBLIC_PREFIX + entry['path'])] = lambda r, status=status: Response(status, {**r.headers, 'location': 'https://attacker.invalid/' + MOCK_SECRET}, BODY_MARKER.encode())
            receipt, _ = self.stage(transport)
            self.assertFalse(receipt['passed'])
            self.assertFalse(any('attacker' in c['path'] for c in transport.calls))
            self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)

    def test_cancel_and_exception_body_never_printed_partial_retained(self):
        for error in (KeyboardInterrupt(MOCK_SECRET), OSError(MOCK_SECRET + BODY_MARKER)):
            transport = StoreTransport(); entry = transport.plan['objects'][1]
            transport.failures[('GET', P.PUBLIC_PREFIX + entry['path'])] = lambda r, error=error: Response(headers=r.headers, body=r.body, error=error)
            receipt, _ = self.stage(transport)
            self.assertFalse(receipt['passed'])
            self.assertEqual(receipt['verifiedObjectCount'], 1)
            self.assertIsNone(receipt['stageEnvelope'])

    def test_promote_direct_envelope_then_private_and_full_alias_gets(self):
        receipt, transport = self.promote()
        self.assertTrue(receipt['passed'])
        self.assertTrue(receipt['privatePointerVerified'])
        self.assertTrue(receipt['aliasesVerified'])
        self.assertFalse(receipt['pointerOutcomeUnknown'])
        self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)
        self.assertTrue(any(c['method'] == 'GET' and c['path'] == P.PRIVATE_PREFIX + '/pointer' for c in transport.calls))
        self.assertEqual([c['path'] for c in transport.calls if c['method'] == 'GET' and c['path'].startswith(P.PUBLIC_PREFIX)],
                         [P.PUBLIC_PREFIX + 'latest.json', P.PUBLIC_PREFIX + 'latest.p7s'])
        self.assertEqual(transport.calls[0]['path'], P.PRIVATE_PREFIX + '/pointer')

    def test_promote_response_loss_after_commit_keeps_unknown_no_retry(self):
        transport = StoreTransport(); stage = self.passed_stage(transport); transport.calls.clear()
        def lost(_response):
            raise OSError(MOCK_SECRET + BODY_MARKER)
        transport.failures[('POST', P.PRIVATE_PREFIX + '/promote')] = lost
        receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET, stage=stage, stage_sha=P.digest(P.canonical(stage)))
        self.safe(receipt)
        self.assertFalse(receipt['passed'])
        self.assertTrue(transport.pointer)
        self.assertTrue(receipt['pointerOutcomeUnknown'])
        self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)
        self.assertFalse(any(c['method'] in ('DELETE', 'PUT') for c in transport.calls))

    def test_promote_worker_unknown_and_cas_denial_retained(self):
        for status, unknown in ((502, True), (412, False)):
            transport = StoreTransport(); stage = self.passed_stage(transport); transport.calls.clear()
            reply = {'ok': False, 'error': BODY_MARKER + MOCK_SECRET, 'pointerAttempted': True,
                     'pointerChanged': False, 'pointerOutcomeUnknown': unknown,
                     'objectWriteAttempted': False, 'objectOutcomeUnknown': False}
            transport.failures[('POST', P.PRIVATE_PREFIX + '/promote')] = lambda r, status=status: Response(status, r.headers, P.canonical(reply))
            receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET, stage=stage, stage_sha=P.digest(P.canonical(stage)))
            self.safe(receipt)
            self.assertFalse(receipt['passed'])
            self.assertEqual(receipt['pointerOutcomeUnknown'], unknown)
            self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)

    def test_alias_wrong_headers_or_bytes_never_overall_pass(self):
        for change in ('bytes', 'cache', 'sequence', 'feedsha', 'feedpath', 'signaturepath'):
            transport = StoreTransport(); stage = self.passed_stage(transport); transport.calls.clear()
            def changed(response, change=change):
                if change == 'bytes':
                    response.body = b'X' + response.body[1:]
                else:
                    key = {'cache': 'cache-control', 'sequence': 'x-mir2-sequence',
                        'feedsha': 'x-mir2-feed-sha256', 'feedpath': 'x-mir2-feed-path',
                        'signaturepath': 'x-mir2-signature-path'}[change]
                    response.headers[key] = BODY_MARKER
                return response
            transport.failures[('GET', P.PUBLIC_PREFIX + 'latest.p7s')] = changed
            receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET, stage=stage, stage_sha=P.digest(P.canonical(stage)))
            self.safe(receipt)
            self.assertFalse(receipt['passed'])
            self.assertFalse(receipt['aliasesVerified'])
            self.assertTrue(receipt['privatePointerVerified'])

    def test_forged_incomplete_or_changed_stage_never_promotes(self):
        transport = StoreTransport(); base = self.passed_stage(transport)
        for key, value in (('passed', False), ('sourceSha256', '0' * 64), ('verifiedObjectCount', 35),
                           ('publicVerifiedBytes', 0), ('publicVerified', False), ('receiptStatus', 'not_written'),
                           ('stageEnvelope', P.expected_envelope(REAL_PLAN)), ('verifiedObjects', [])):
            stage = copy.deepcopy(base); stage[key] = value; transport.calls.clear()
            receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET, stage=stage, stage_sha='1' * 64)
            self.safe(receipt)
            self.assertFalse(receipt['passed'])
            self.assertEqual(transport.calls, [])

    def test_strict_json_utf8_duplicate_depth_tokens_and_float(self):
        for body in (b'\xff', b'\xef\xbb\xbf{}', b'{"x":1,"\\u0078":2}', b'{"x":NaN}',
                     b'{"x":1.0}', b'{"x":"\\ud800"}', b'[' * 13 + b'0' + b']' * 13,
                     b'[' + b'0,' * 8192 + b'0]', b'{}' + BODY_MARKER.encode()):
            with self.subTest(size=len(body)):
                with self.assertRaises(P.Fault):
                    P.strict_json(body)
        self.assertEqual(P.strict_json(b'{"x":[1,true,null]}'), {'x': [1, True, None]})

    def test_changed_plan_or_attestation_is_rejected_before_http(self):
        for raw in (PLAN_RAW.replace(b'"sequence": 14', b'"sequence": 15', 1), PLAN_RAW + b' ',
                    PLAN_RAW.replace(b'assets.mir2.obelisk.build', b'attacker.mir2.obelisk.build', 1)):
            with self.assertRaises(P.Fault) as failure:
                P.load_plan_bytes(raw)
            self.assertEqual(failure.exception.code, 'plan_invalid')

    def test_fixed_transport_no_host_query_key_secret_or_method_override(self):
        transport = P.FixedHttpsTransport(REAL_PLAN)
        with patch.object(P.http.client, 'HTTPSConnection') as factory:
            for method, path, token, body in (
                ('GET', 'https://attacker.invalid/' + MOCK_SECRET, None, None),
                ('GET', P.PUBLIC_PREFIX + 'latest.json?x=1', None, None),
                ('GET', P.PUBLIC_PREFIX + 'latest.json', MOCK_SECRET, None),
                ('GET', P.PRIVATE_PREFIX + '/objects/036', MOCK_SECRET, None),
                ('DELETE', P.PRIVATE_PREFIX + '/pointer', MOCK_SECRET, None),
                ('POST', P.PRIVATE_PREFIX + '/import', MOCK_SECRET, b'{"url":"bad"}'),
                ('POST', P.PRIVATE_PREFIX + '/promote', MOCK_SECRET, b'{"passed":true}')):
                with self.assertRaises(P.Fault):
                    transport.open(method, path, token=token, body=body)
            factory.assert_not_called()

    def test_real_https_transport_fixed_tls_no_proxy_and_public_no_auth(self):
        raw = FEED; connection = FakeConnection(Response(headers=public_object(REAL_PLAN['objects'][0]), body=raw))
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        with patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection) as factory, \
             patch.object(P.os, 'environ', {'https_proxy': 'https://' + MOCK_SECRET + '.invalid'}):
            response = P.FixedHttpsTransport(REAL_PLAN).open('GET', P.PUBLIC_PREFIX + REAL_PLAN['objects'][0]['path'])
            result = b''
            while chunk := response.read(P.CHUNK):
                result += chunk
            response.close()
        self.assertEqual(result, raw)
        args, kwargs = factory.call_args
        self.assertEqual(args, (P.HOST,))
        self.assertEqual(kwargs['port'], 443)
        self.assertIs(kwargs['context'], context)
        self.assertTrue(0 < kwargs['timeout'] <= P.PUBLIC_SECONDS)
        self.assertEqual(connection.debug, 0)
        self.assertEqual(connection.requests[0][0:3], ('GET', P.PUBLIC_PREFIX + REAL_PLAN['objects'][0]['path'], None))
        self.assertNotIn('Authorization', connection.requests[0][3])
        self.assertEqual(connection.requests[0][3]['Accept-Encoding'], 'identity')
        self.assertTrue(connection.closed)

    def test_tls_unverified_context_rejects_before_connection(self):
        for verify, hostname in ((ssl.CERT_NONE, True), (ssl.CERT_REQUIRED, False)):
            with patch.object(P, 'verified_tls_context', return_value=types.SimpleNamespace(verify_mode=verify, check_hostname=hostname)), \
                 patch.object(P.http.client, 'HTTPSConnection') as factory:
                with self.assertRaises(P.Fault) as failure:
                    P.FixedHttpsTransport(REAL_PLAN).open('GET', P.PUBLIC_PREFIX + 'latest.json')
                self.assertEqual(failure.exception.code, 'tls_policy_rejected')
                factory.assert_not_called()

    def test_absolute_deadline_cancels_stalled_headers_not_throughput_proof(self):
        connection = FakeConnection(Response(), wait_headers=True)
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        started = time.monotonic()
        with patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection), \
             patch.object(P, 'PUBLIC_SECONDS', 0.025):
            with self.assertRaises(P.Fault) as failure:
                P.FixedHttpsTransport(REAL_PLAN).open('GET', P.PUBLIC_PREFIX + 'latest.json')
        self.assertEqual(failure.exception.code, 'request_deadline')
        self.assertLess(time.monotonic() - started, 0.4)
        self.assertTrue(connection.sock.cancelled.is_set())

    def test_connection_close_error_cannot_be_success_and_is_redacted(self):
        connection = FakeConnection(Response(body=FEED), close_failure=True)
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        with patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection):
            response = P.FixedHttpsTransport(REAL_PLAN).open('GET', P.PUBLIC_PREFIX + 'latest.json')
            try:
                with self.assertRaises(P.Fault) as failure:
                    while response.read(P.CHUNK):
                        pass
                self.assertEqual(failure.exception.code, 'transport_failed')
                self.assertNotIn(MOCK_SECRET, str(failure.exception))
            finally:
                response.close()

    def test_111mb_synthetic_stream_is_bounded_and_fails_real_exe_checksum(self):
        entry = REAL_PLAN['objects'][32]
        class Synthetic:
            def __init__(self):
                self.remaining, self.maximum = entry['size'], 0
            def read(self, amount):
                self.maximum = max(self.maximum, amount)
                size = min(amount, self.remaining)
                self.remaining -= size
                return b'\0' * size
        response = Synthetic(); call = {'receivedBytes': 0}
        tracemalloc.start()
        try:
            with self.assertRaises(P.Fault) as failure:
                P.read_full(response, call, entry)
            _, peak = tracemalloc.get_traced_memory()
        finally:
            tracemalloc.stop()
        self.assertEqual(failure.exception.code, 'body_hash_rejected')
        self.assertEqual(call['receivedBytes'], 111013376)
        self.assertEqual(response.maximum, 65536)
        self.assertLess(peak, 1024 * 1024)

    def test_cli_stage_success_fresh_receipt_exit_zero_and_exact_summary(self):
        root = self.fixture('cli-stage')
        code, summary, transport = self.run_cli(root)
        self.assertEqual(code, 0)
        self.assertTrue(summary['passed'])
        receipt = P.strict_json((root / 'receipt.json').read_bytes(), P.RECEIPT_LIMIT, 20000)
        self.safe(receipt)
        self.assertEqual(receipt['receiptStatus'], 'written')
        P.validate_stage_receipt(receipt, transport.plan)
        if os.name == 'posix':
            self.assertEqual((root / 'receipt.json').stat().st_mode & 0o777, 0o600)

    def test_missing_invalid_secret_and_poisoned_environment_never_changes_paths(self):
        for env in ({}, {'MIR2_R2_UPLOAD_SECRET': ''}, {'MIR2_R2_UPLOAD_SECRET': MOCK_SECRET + '\r\nHost: bad'}):
            root = self.fixture('bad-env'); code, summary, transport = self.run_cli(root, env=env)
            self.assertEqual(code, 2)
            self.assertFalse(summary['passed'])
            self.assertEqual(transport.calls, [])
        class PoisonEnv:
            def get(self, key):
                if key != 'MIR2_R2_UPLOAD_SECRET':
                    raise AssertionError(MOCK_SECRET)
                return MOCK_SECRET
        root = self.fixture('poison-env'); code, _, transport = self.run_cli(root, env=PoisonEnv())
        self.assertEqual(code, 0)
        self.assertTrue(all(c['path'].startswith((P.PUBLIC_PREFIX, P.PRIVATE_PREFIX)) for c in transport.calls))

    def test_cli_no_plan_url_key_override_or_duplicate_arguments(self):
        for args in (['--mode', 'stage', '--receipt', MOCK_SECRET, '--plan', 'bad'],
                     ['--mode', 'stage', '--receipt', 'x', '--url', BODY_MARKER],
                     ['--mode', 'stage', '--mode', 'promote'],
                     ['--mode', 'bad', '--receipt', MOCK_SECRET]):
            root = self.fixture('bad-args'); code, summary, transport = self.run_cli(root, args=args)
            self.assertEqual(code, 2)
            self.assertEqual(summary['result'], 'arguments_invalid')
            self.assertEqual(transport.calls, [])

    def test_receipt_existing_symlink_hardlink_parent_link_refuse_before_http(self):
        for kind in ('existing', 'symlink', 'hardlink', 'parent-link', 'directory'):
            root = self.fixture('receipt-' + kind); original = root / 'original.json'
            original.write_bytes(b'preserve original')
            destination = root / 'receipt.json'
            if kind == 'existing':
                destination.write_bytes(b'preserve destination')
            elif kind == 'symlink':
                destination.symlink_to(original)
            elif kind == 'hardlink':
                os.link(original, destination)
            elif kind == 'directory':
                destination.mkdir()
            else:
                actual = root / 'actual'; actual.mkdir()
                link = root / 'parent'; link.symlink_to(actual, target_is_directory=True)
                destination = link / 'receipt.json'
            code, summary, transport = self.run_cli(root, args=['--mode', 'stage', '--receipt', str(destination)])
            self.assertEqual(code, 2)
            self.assertEqual(summary['result'], 'receipt_invalid')
            self.assertEqual(transport.calls, [])
            self.assertEqual(original.read_bytes(), b'preserve original')

    def test_promote_retained_stage_hash_and_regular_file_gate_before_http(self):
        transport = StoreTransport(); stage = self.passed_stage(transport)
        raw = P.canonical(stage)
        for kind in ('good', 'wrong-sha', 'symlink', 'hardlink', 'truncated'):
            root = self.fixture('stage-input-' + kind)
            source = root / 'stage.json'
            with source.open('xb') as out:
                out.write(raw[:-1] if kind == 'truncated' else raw)
            source.chmod(0o600)
            input_file = source
            if kind == 'symlink':
                input_file = root / 'linked.json'; input_file.symlink_to(source)
            elif kind == 'hardlink':
                input_file = root / 'linked.json'; os.link(source, input_file)
            transport.calls.clear()
            code, summary, _ = self.run_cli(root, transport=transport, args=['--mode', 'promote', '--receipt',
                str(root / 'receipt.json'), '--stage-receipt', str(input_file), '--stage-sha256',
                '0' * 64 if kind == 'wrong-sha' else P.digest(raw)])
            if kind == 'good':
                self.assertEqual(code, 0)
            else:
                self.assertEqual(code, 2)
                self.assertFalse(summary['passed'])
                self.assertEqual(transport.calls, [])

    def test_receipt_write_failure_exit2_even_if_fake_core_passed(self):
        root = self.fixture('write-failure')
        actual = P.reserve_receipt(root / 'receipt.json')
        writes = []
        class Broken:
            _mir2_path = actual._mir2_path
            _mir2_identity = actual._mir2_identity
            def fileno(self):
                return actual.fileno()
            def write(self, _raw):
                writes.append(True)
                raise OSError(MOCK_SECRET + BODY_MARKER)
            def close(self):
                actual.close()
                raise OSError(MOCK_SECRET)
        with patch.object(P, 'reserve_receipt', return_value=Broken()):
            code, summary, _ = self.run_cli(root)
        self.assertEqual(code, 2)
        self.assertFalse(summary['passed'])
        self.assertEqual(summary['result'], 'receipt_io_failed')
        self.assertEqual(summary['receiptStatus'], 'not_written')
        self.assertEqual(writes, [True])

    def test_stage_envelope_numeric_boolean_and_call_index_bool_refuse(self):
        transport = StoreTransport(); original = self.passed_stage(transport)
        for change in ('envelope-bool', 'envelope-false', 'index-bool', 'private-pointer', 'post-head-404'):
            stage = copy.deepcopy(original)
            if change == 'envelope-bool':
                stage['stageEnvelope']['passed'] = 1
            elif change == 'envelope-false':
                stage['stageEnvelope']['pointerChanged'] = 0
            elif change == 'index-bool':
                stage['calls'][5]['index'] = True
            elif change == 'private-pointer':
                stage['privatePointerVerified'] = True
            else:
                stage['calls'][3]['status'] = 404
            transport.calls.clear()
            receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET,
                stage=stage, stage_sha=P.digest(P.canonical(stage)))
            self.safe(receipt)
            self.assertFalse(receipt['passed'], change)
            self.assertEqual(transport.calls, [], change)

    def test_stage_fabricated_receipt_calls_or_wrong_byte_hash_refuse(self):
        transport = StoreTransport(); original = self.passed_stage(transport)
        for change in ('calls-empty', 'public-bytes', 'unknown-call-key', 'stage-sha'):
            stage = copy.deepcopy(original)
            if change == 'calls-empty':
                stage['calls'] = []
            elif change == 'public-bytes':
                stage['calls'][-1]['receivedBytes'] -= 1
            elif change == 'unknown-call-key':
                stage['calls'][0]['body'] = BODY_MARKER
            transport.calls.clear()
            receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET,
                stage=stage, stage_sha='0' * 64 if change == 'stage-sha' else P.digest(P.canonical(stage)))
            self.safe(receipt)
            self.assertFalse(receipt['passed'], change)
            self.assertEqual(transport.calls, [], change)

    def test_private_pointer_readback_failure_after_ack_retains_unknown(self):
        transport = StoreTransport(); stage = self.passed_stage(transport); transport.calls.clear()
        def changed(response):
            if transport.pointer:
                response.headers['x-mir2-stored-sha256'] = '0' * 64
            return response
        transport.failures[('GET', P.PRIVATE_PREFIX + '/pointer')] = changed
        receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET,
            stage=stage, stage_sha=P.digest(P.canonical(stage)))
        self.safe(receipt)
        self.assertFalse(receipt['passed'])
        self.assertTrue(receipt['pointerChanged'])
        self.assertTrue(receipt['pointerOutcomeUnknown'])
        self.assertFalse(receipt['privatePointerVerified'])
        self.assertFalse(receipt['aliasesVerified'])
        self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)

    def test_exact_already_promoted_candidate_does_not_claim_new_pointer_change(self):
        transport = StoreTransport(); transport.pointer = True
        receipt, _ = self.promote(transport)
        self.assertTrue(receipt['passed'])
        self.assertFalse(receipt['pointerAttempted'])
        self.assertFalse(receipt['pointerChanged'])
        self.assertFalse(receipt['pointerOutcomeUnknown'])

    def test_promote_401_malformed_ack_and_oversized_ack_stop_no_retry(self):
        for kind in ('401', 'shape', 'duplicate', 'oversized'):
            transport = StoreTransport(); stage = self.passed_stage(transport); transport.calls.clear()
            if kind == '401':
                action = lambda r: Response(401, r.headers, MOCK_SECRET.encode())
            elif kind == 'shape':
                action = lambda r: Response(headers=r.headers, body=P.canonical({'ok': True, 'error': MOCK_SECRET}))
            elif kind == 'duplicate':
                action = lambda r: Response(headers=r.headers, body=b'{"ok":true,"ok":true}')
            else:
                action = lambda r: Response(headers=r.headers, body=b' ' * (P.CONTROL_LIMIT + 1))
            transport.failures[('POST', P.PRIVATE_PREFIX + '/promote')] = action
            receipt = P.deliver('promote', transport.plan, transport, MOCK_SECRET,
                stage=stage, stage_sha=P.digest(P.canonical(stage)))
            self.safe(receipt)
            self.assertFalse(receipt['passed'])
            self.assertTrue(receipt['pointerOutcomeUnknown'])
            self.assertEqual(len([c for c in transport.calls if c['method'] == 'POST']), 1)

    def test_source_feed_exact_expiration_boundary_refuses_stage(self):
        feed = P.strict_json(FEED)
        for now in (feed['createdUnix'] - 1, feed['expiresUnix']):
            with patch.object(P.time, 'time', return_value=now):
                receipt, transport = self.stage()
            self.assertFalse(receipt['passed'])
            self.assertEqual(receipt['result'], 'feed_expired_or_invalid')
            self.assertIsNone(receipt['stageEnvelope'])
            self.assertEqual(len(transport.stored), 1)

    def test_control_json_declared_length_duplicate_headers_and_corrupt_type(self):
        for kind in ('length', 'duplicate', 'mime'):
            transport = StoreTransport()
            def changed(response, kind=kind):
                if kind == 'length':
                    response.headers['content-length'] = str(len(response.body) + 1)
                elif kind == 'duplicate':
                    response.headers = [*response.headers.items(), ('cache-control', 'no-store')]
                else:
                    response.headers['content-type'] = 'text/html'
                return response
            transport.failures[('POST', P.PRIVATE_PREFIX + '/import')] = changed
            receipt, _ = self.stage(transport)
            self.assertFalse(receipt['passed'])
            self.assertIsNone(receipt['stageEnvelope'])
            self.assertEqual(receipt['verifiedObjectCount'], 0)

    def test_real_cdn_cache_timing_lines_preserve_object_validation(self):
        entry = PLAN['objects'][19]
        response = Response()
        response.headers = [*public_object(entry).items(),
            ('Server-Timing', 'cfCacheStatus;desc="HIT"'),
            ('Server-Timing', 'cfEdge;dur=38,cfOrigin;dur=0,cfWorker;dur=32')]
        headers = P.header_map(response)
        P.public_headers(headers, entry)
        self.assertEqual(headers['content-length'], str(entry['size']))
        self.assertIn('cfCacheStatus;desc="HIT"', headers['server-timing'])
        self.assertIn('cfWorker;dur=32', headers['server-timing'])

    def test_cdn_timing_exception_keeps_critical_duplicates_and_bounds_rejected(self):
        entry = PLAN['objects'][19]
        for name, value in [('Content-Length', str(entry['size'])),
                            ('Content-Type', P.public_mime(entry['path'])),
                            ('Cache-Control', P.IMMUTABLE), ('ETag', '"unit-public-etag"'),
                            ('X-Mir2-Native-Cache', 'HIT')]:
            with self.subTest(name=name):
                response = Response()
                response.headers = [*public_object(entry).items(), (name, value)]
                with self.assertRaisesRegex(P.Fault, 'headers_rejected'):
                    P.header_map(response)
        for timing in [[('Server-Timing', 'bad\r\nContent-Length: 0')],
                       [('Server-Timing', 'x' * 16384)], [('Server-Timing', 'x')] * 81]:
            response = Response()
            response.headers = [*public_object(entry).items(), *timing]
            with self.assertRaisesRegex(P.Fault, 'headers_rejected'):
                P.header_map(response)

    def test_reflected_secret_in_permitted_etag_is_never_persisted(self):
        transport = StoreTransport(); transport.stored.add(0)
        def changed(response):
            response.headers['etag'] = '"' + MOCK_SECRET + '"'
            return response
        transport.failures[('HEAD', P.PRIVATE_PREFIX + '/objects/0')] = changed
        receipt, _ = self.stage(transport)
        self.assertTrue(receipt['passed'])
        self.safe(receipt)

    def test_cancelled_connect_never_sends_auth_after_deadline(self):
        gate, closed = threading.Event(), threading.Event()
        class SlowConnect(FakeConnection):
            def connect(self):
                gate.wait(2)
                super().connect()
            def close(self):
                super().close()
                closed.set()
        connection = SlowConnect(Response())
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        with patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection), \
             patch.object(P, 'IMPORT_CONTROL_SECONDS', 0.025):
            with self.assertRaises(P.Fault) as failure:
                P.FixedHttpsTransport(REAL_PLAN).open('POST', P.PRIVATE_PREFIX + '/import',
                    token=MOCK_SECRET, body=P.canonical({'index': 0}))
            self.assertEqual(failure.exception.code, 'request_deadline')
            gate.set()
            self.assertTrue(closed.wait(0.5))
        self.assertEqual(connection.requests, [])

    def test_each_actual_transport_request_gets_its_own_deadline(self):
        class SlowRead(Response):
            def read(self, maximum):
                if self.offset == 0:
                    time.sleep(0.035)
                return super().read(maximum)
        connections = [FakeConnection(SlowRead(body=FEED)), FakeConnection(SlowRead(body=CMS))]
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        with patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', side_effect=connections), \
             patch.object(P, 'PUBLIC_SECONDS', 0.065):
            transport = P.FixedHttpsTransport(REAL_PLAN)
            for index in (0, 1):
                response = transport.open('GET', P.PUBLIC_PREFIX + REAL_PLAN['objects'][index]['path'])
                call = {'receivedBytes': 0}
                try:
                    P.read_full(response, call, REAL_PLAN['objects'][index])
                finally:
                    response.close()
        self.assertTrue(all(c.closed for c in connections))

    def test_actual_private_post_bearer_body_only_fixed_verified_origin(self):
        connection = FakeConnection(Response(status=201, body=b'{}'))
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        body = P.canonical({'index': 0})
        with patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection) as factory:
            response = P.FixedHttpsTransport(REAL_PLAN).open('POST', P.PRIVATE_PREFIX + '/import', token=MOCK_SECRET, body=body)
            try:
                while response.read(P.CHUNK):
                    pass
            finally:
                response.close()
        self.assertEqual(factory.call_args.args, (P.HOST,))
        request = connection.requests[0]
        self.assertEqual(request[:3], ('POST', P.PRIVATE_PREFIX + '/import', body))
        self.assertEqual(request[3]['Authorization'], 'Bearer ' + MOCK_SECRET)
        self.assertEqual(request[3]['Content-Length'], str(len(body)))
        self.assertEqual(connection.debug, 0)

    def test_read_regular_rejects_new_hardlink_during_read(self):
        root = self.fixture('read-race'); source = root / 'stage.json'
        source.write_bytes(b'public fixture')
        real_fdopen = P.os.fdopen
        class LinkedRead:
            def __init__(self, stream):
                self.stream = stream
            def __enter__(self):
                return self
            def __exit__(self, *_args):
                self.stream.close()
            def fileno(self):
                return self.stream.fileno()
            def read(self, maximum):
                os.link(source, root / 'new-link.json')
                return self.stream.read(maximum)
        def opener(fd, mode):
            return LinkedRead(real_fdopen(fd, mode))
        with patch.object(P.os, 'fdopen', side_effect=opener):
            with self.assertRaises(P.Fault) as failure:
                P.read_regular(source, 100)
        self.assertEqual(failure.exception.code, 'receipt_invalid')
        self.assertEqual(source.read_bytes(), b'public fixture')

    def test_output_hardlink_added_during_stage_cannot_pass_cli(self):
        root = self.fixture('output-race'); transport = StoreTransport()
        entry = transport.plan['objects'][35]
        def linked(response):
            os.link(root / 'receipt.json', root / 'new-link.json')
            return response
        transport.failures[('GET', P.PUBLIC_PREFIX + entry['path'])] = linked
        code, summary, _ = self.run_cli(root, transport=transport)
        self.assertEqual(code, 2)
        self.assertFalse(summary['passed'])
        self.assertEqual(summary['result'], 'receipt_io_failed')

    def test_poisoned_sslkeylogfile_is_not_created_or_written(self):
        root = self.fixture('keylog-env')
        destination = root / 'ssl-session-keylog.txt'
        connection = FakeConnection(Response(body=FEED))
        # Real SSLContext construction, mocked CA load and HTTPS socket only.
        # No credential environment traversal, TLS handshake or auth request.
        with patch.object(P.os, 'environ', {'SSLKEYLOGFILE': str(destination)}), \
             patch.object(P.ssl.SSLContext, 'load_default_certs', autospec=True) as load_certs, \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection) as factory:
            response = P.FixedHttpsTransport(REAL_PLAN).open('GET', P.PUBLIC_PREFIX + 'latest.json')
            try:
                P.read_full(response, {'receivedBytes': 0}, REAL_PLAN['objects'][0])
            finally:
                response.close()
            context = factory.call_args.kwargs['context']
            self.assertEqual(context.verify_mode, ssl.CERT_REQUIRED)
            self.assertIs(context.check_hostname, True)
            self.assertIsNone(context.keylog_filename)
            load_certs.assert_called_once_with(context, ssl.Purpose.SERVER_AUTH)
        self.assertFalse(destination.exists())



class NetworkIdlePolicyTests(unittest.TestCase):
    def test_after_header_body_idle_uses_progress_read_and_cancels_before_absolute_deadline(self):
        class ProgressThenStall(Response):
            progress_read_called = False
            def read(self, maximum):
                if not self.offset:
                    self.offset = 16
                    return FEED[:16]
                self.socket.cancelled.wait(2)
                raise OSError('public fake stalled body')
            def read1(self, maximum):
                self.progress_read_called = True
                return self.read(maximum)
        body = ProgressThenStall(headers=public_object(REAL_PLAN['objects'][0]))
        connection = FakeConnection(body); body.socket = connection.sock
        context = P.verified_tls_context()
        with patch.object(P, 'NO_PROGRESS_SECONDS', 0.04, create=True), \
             patch.object(P, 'PUBLIC_HEADER_SECONDS', 0.04, create=True), \
             patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection):
            response = P.NetworkResponse('GET', P.PUBLIC_PREFIX + REAL_PLAN['objects'][0]['path'], None, None, 0.4)
            started = time.monotonic()
            try:
                self.assertEqual(response.read(P.CHUNK), FEED[:16])
                with self.assertRaisesRegex(P.Fault, 'request_deadline'):
                    response.read(P.CHUNK)
            finally:
                response.close()
            self.assertLess(time.monotonic() - started, 0.2)
            self.assertTrue(body.progress_read_called)
            self.assertTrue(connection.sock.shutdowns)
    def test_public_stalled_headers_obey_fixed_header_budget(self):
        connection = FakeConnection(Response(body=FEED), wait_headers=True)
        context = P.verified_tls_context()
        with patch.object(P, 'NO_PROGRESS_SECONDS', 0.04, create=True), \
             patch.object(P, 'PUBLIC_HEADER_SECONDS', 0.04, create=True), \
             patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection):
            started = time.monotonic()
            with self.assertRaisesRegex(P.Fault, 'request_deadline'):
                P.NetworkResponse('GET', P.PUBLIC_PREFIX + REAL_PLAN['objects'][0]['path'], None, None, 0.4)
            self.assertLess(time.monotonic() - started, 0.2)
            self.assertTrue(connection.sock.shutdowns)
    def test_private_import_can_wait_for_worker_headers_beyond_body_idle_budget(self):
        class SlowHeaderConnection(FakeConnection):
            def getresponse(self):
                time.sleep(0.08)
                return self.response
        connection = SlowHeaderConnection(Response(headers=private_base(), body=b'{}'))
        context = P.verified_tls_context()
        with patch.object(P, 'NO_PROGRESS_SECONDS', 0.02, create=True), \
             patch.object(P, 'PUBLIC_HEADER_SECONDS', 0.02, create=True), \
             patch.object(P, 'verified_tls_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', return_value=connection):
            response = P.NetworkResponse('POST', P.PRIVATE_PREFIX + '/import',
                MOCK_SECRET, P.canonical({'index': 0}), 0.4)
            try:
                self.assertEqual(response.read(P.CHUNK), b'{}')
            finally:
                response.close()
            self.assertEqual(len(connection.requests), 1)


if __name__ == '__main__':
    suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    failed_methods = {getattr(test, 'test_case', test).id()
                      for test, _trace in [*result.failures, *result.errors]}
    receipt = {'schema': 'mir2.native.r17.ci-delivery-local-tests.v1', 'createdUnix': int(time.time()),
        'python': sys.version, 'platform': sys.platform, 'run': result.testsRun,
        'pass': result.testsRun - len(failed_methods) - len(result.skipped),
        'failedTestMethods': len(failed_methods), 'failureAssertions': len(result.failures),
        'fail': len(result.failures), 'errors': len(result.errors), 'skipped': len(result.skipped),
        'passed': result.wasSuccessful(), 'sourceSha256': P.source_sha(),
        'testSha256': P.digest(Path(__file__).read_bytes()), 'literalPlanSha256': P.digest(PLAN_RAW),
        'actualEnvironmentRead': False, 'credentialsUsed': False, 'actualNetworkRequests': 0,
        'source36Accepted': False, 'public36Accepted': False, 'cmsReverified': False,
        'modelBoundary': 'real tiny feed pair; toy remaining34; synthetic111MB deliberately fails real EXE hash',
        'fixturesRetained': RETAINED}
    path = BASE / ('evidence/TEST-RECEIPT-' + str(time.time_ns()) + '.json')
    with path.open('xb') as out:
        out.write(P.canonical(receipt))
    print('RAW_RECEIPT=' + str(path))
    raise SystemExit(0 if result.wasSuccessful() else 1)
