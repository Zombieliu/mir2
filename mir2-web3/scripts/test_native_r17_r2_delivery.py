"""Offline CI delivery tests. All secrets/transports/stores are synthetic.

The real literal plan and two tiny source-feed objects are bound. Other object
bytes are a toy internal table, never a successful R2/CDN/source36 publication.
"""
from __future__ import annotations

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
SPEC = importlib.util.spec_from_file_location('native_r17_delivery_fixture', BASE / 'scripts/native_r17_r2_delivery.py')
P = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = P
SPEC.loader.exec_module(P)
INPUT_PLAN = P.PLAN_FILE if P.PLAN_FILE.exists() else Path(
    'C:/mir2-playtest-releases/20261004-native-r18/r17-native-r2-importer-worker-01/src/native-r17-publication-plan.json')
PLAN_RAW = INPUT_PLAN.read_bytes()
REAL_PLAN = P.load_plan_bytes(PLAN_RAW)
FEED = base64.b64decode('eyJzY2hlbWEiOiJtaXIyLndpbmRvd3MudXBkYXRlLWZlZWQudjEiLCJjaGFubmVsIjoiaW52aXRlZCIsInBsYXRmb3JtIjoid2luZG93cy14NjQiLCJzZXF1ZW5jZSI6MTIsImNyZWF0ZWRVbml4IjoxNzkxMDgwMzA1LCJleHBpcmVzVW5peCI6MTc5MzY3MjMwNSwibWluQm9vdHN0cmFwIjoxLCJwcm90b2NvbCI6ImNyeXN0YWwtbWlyMi12MSIsImNvbnRlbnQiOiJtaXIyLndpbmRvd3MucGFja2FnZS1tYW5pZmVzdC52NCIsImdhbWUiOnsiZGlyZWN0b3J5IjoicmVsZWFzZXMvZ2FtZS1XTi1DQU5ESURBVEUtMjAyNjEwMDQtaW52aXRlZC0xNyIsImlkZW50aXR5IjoiV04tQ0FORElEQVRFLTIwMjYxMDA0LWludml0ZWQtMTciLCJtZXRhZGF0YSI6W3sicGF0aCI6IlBBQ0tBR0UtTUFOSUZFU1QuanNvbiIsInNpemUiOjI0MzYwMTc0LCJzaGEyNTYiOiJDRUFFMzY2QkVCQ0ExQzE5QzNGNTI5RUIyMkYwMTZDQ0M4MERGRjVCODY1NTY5NEU3NjY4MkUzMUFCMTJDQjE4In0seyJwYXRoIjoiVkVSU0lPTi5qc29uIiwic2l6ZSI6MTIyMSwic2hhMjU2IjoiMTQwOUUyMUM5RjFBRTdBMDRCNzE2MDNDRTEyMjBGRTM4QzM3RjIyMkQ1NDg1OUFGMjU5MkNERDcyQ0U5OUIxNiJ9LHsicGF0aCI6IlJFTEVBU0UtU1RBVEVNRU5ULmpzb24iLCJzaXplIjo3MDcsInNoYTI1NiI6IkU2NEE4RUY0RkM2MkU2RTlGNkZFMTY2MjUzMjFGQzhBODhBMEM4QzI5QTY2MUFFREE0NjAyNUIwODAxNURBODIifSx7InBhdGgiOiJSRUxFQVNFLVNUQVRFTUVOVC5wN3MiLCJzaXplIjoxNjE0LCJzaGEyNTYiOiJCRUU4NEI1NzRGNDNGNjQxMUVCMUJDM0FCMjNBM0NFMEMzODlCODUzRkIwRjhCNjNGNkFEOUJEQjEyODc4NTZFIn1dfSwiZW5naW5lIjp7ImRpcmVjdG9yeSI6InJlbGVhc2VzL3VwZGF0ZXItRkIzNTA1RDZEODREOTk3Qy1zMTIiLCJpZGVudGl0eSI6IjEiLCJtZXRhZGF0YSI6W3sicGF0aCI6IkVOR0lORS5qc29uIiwic2l6ZSI6NDIxLCJzaGEyNTYiOiIzMTQ2MTY5RkM2MTc2QTVFQUVBNDJCQUFBMDQzMEUzNDVGNUI3RTYxNDcwOTk3MUEwMEIwRDdEMEM0NzMyQkNGIn0seyJwYXRoIjoiRU5HSU5FLnA3cyIsInNpemUiOjE2MTQsInNoYTI1NiI6IjcwREMyREMxMUMzMTA1RkQ0REQyQjgwQkQ3RjlGQjMwRjdBN0Y0QjNCQkJGRTJBQTEzOTdDMDM4QzlCM0MxNEEifV19fQ==')
CMS = base64.b64decode('MIIGSgYJKoZIhvcNAQcCoIIGOzCCBjcCAQExDTALBglghkgBZQMEAgEwCwYJKoZIhvcNAQcBoIIEKjCCBCYwggKOoAMCAQICEEGdQHONjWm2Su1HznxL72AwDQYJKoZIhvcNAQELBQAwKzEpMCcGA1UEAwwgTWlyMiBJbnZpdGVkIFBsYXl0ZXN0IDIwMjYtMDktMjkwHhcNMjYwOTI4MTYxOTQzWhcNMjYxMjI3MTYyOTQzWjArMSkwJwYDVQQDDCBNaXIyIEludml0ZWQgUGxheXRlc3QgMjAyNi0wOS0yOTCCAaIwDQYJKoZIhvcNAQEBBQADggGPADCCAYoCggGBANwYe0Ie9mrIOpKGSqGZoxI+X4c1qpsASY8vWPETnCnhT6NWsu1jd2j14PCMX5XeXlK876K25JPEjkkR0xo18lSYfsdtQyGHzTUkdMsg4eNtq8Hrf7RxXummGbufBAGRnPtzZ+cOq1K/ykjqFAG5TxYYc08CWePiPz/7tEEsir/qZ84XLsAUjy/ff3QuoGy7rOhbziFLRIMnJ/DTrG97csxXzHZP8RHK9WS+LrpRWyjr96+6EEfDbQo3KlmrD0mjPoKVqhVHbVP2HjVfy5LLW8wREMLc2KjJNKxlLM/288z3//yU7W50SSbQW/Rb3dqzls0LcgTLuoooXfxqvy3+0HjV5Ceix+dt+RLboHCJJCBDzPXZ3QKSleUs0xUsMAFxPUjGBT0EwSSt5h9Hc9vWGa2UavLvNB8Dg6zOZI8nbf6w7Z2ksRYX/CoQPGjlECmmkopuuUPTkH0nsHVjJrQnd2AArkBpjyHKqGpFZOytFlTPBkZRVNsaxZ2rgIcB6R/E+QIDAQABo0YwRDAOBgNVHQ8BAf8EBAMCB4AwEwYDVR0lBAwwCgYIKwYBBQUHAwMwHQYDVR0OBBYEFP6W1p1O/6LNZvtKwPn+2cWW+ifMMA0GCSqGSIb3DQEBCwUAA4IBgQDHfTrA24HutGrDOo9pWeA6lX4g8vIccSaXz1ZFF9Xt99yQfDAzDYVB+vMd8Xxs/J59H+eGW4iWUt6IDgRf3/WkB7ShTrMFOQgeWCgZ0p6nUjR9BQE2EN/pdPhm0INK80Bf0kbBBocg6clnSOXrhAZZHWeXx9vXbGdtmAGLhrHV6fivlcX3a19qrOvcnX5vwAFkygOmBew0rM2BkY+1JeDKs3aIBpARlZcxzTtX6H+mbtXEh3vM+wOaWZA0nBb8PoMNNQFVjJlzyDcMfslglNm6af2uCY8DBSj1COxW7pPLopvR66VE5/FNK+REBEnai3EPNnqo23pAuH+5VZCqDtKqDKg518ebZg63otIkINlG4eLEoQ5vP+Ld4/D3lQaqTPIdWu7JHNoGQIjdveqtpYnc/k3nJnPsJMXV9CIz7Ur14/OpiOIicw1QWkYIC87jONq3oJUuW8gdpKou4nr63Bl0RGEVGJoz3aOf0pKfVXZ0StV55giDWuCGGVoF9nLYTWExggHmMIIB4gIBATA/MCsxKTAnBgNVBAMMIE1pcjIgSW52aXRlZCBQbGF5dGVzdCAyMDI2LTA5LTI5AhBBnUBzjY1ptkrtR858S+9gMAsGCWCGSAFlAwQCATALBgkqhkiG9w0BAQEEggGAOyPhjm/jCulXqCLdKZydmEDJLTKLw2rwlgM2l+YqtOStXfHgRJ1Ra6b3kzQU2b9nZCNfqJD+FP73T2hH9o3Z0vM26KliY4G33LSdX3iX4FnkhMqqIP2tOmKwHFj3gfG6aNogW1l2VOHSCU//r8jCBTJLeGd/ll5+m/F5qgdrZ/EPgKsmYuNnP4Expsc8m8E4wEBAULoD6swaj1JnHB1x0y3RHatSEYrxD6tDmgOup24pQ3vEnOKZK0asYnRmZYg1Pf8u66H2a/ga/qdYsdqccfWT7IIetiOCjy7bS2x5N8sPeikxX2nu3en/J5MGC6QCyLa+1kjJGIx8QRbsv/ryVWCRK3TnU3Q4J2LVZEg58hSAbzIufFi3ldj9tDh6iM4qjX8gqbq95E86XJ1G25s9q4xPa+yPGnBxjg+41XFHkbEjo4CsUmdGQadG7GPD05N6rJyrONHWypLY9LI6oK6IjlNsaUfdmKzAfQGhkf+uWfez+LZs4QkqrrMoLzmfAcLi')
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
        directory = 'feeds/s12-' + candidate['feedSha256'] + '/'
        result.update({'x-mir2-sequence': '12', 'x-mir2-feed-sha256': candidate['feedSha256'],
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
            assert P.strict_json(body) == P.expected_envelope(self.plan), 'POST direct envelope, not wrapper'
            already = self.pointer
            self.pointer = True
            reply = {'ok': True, 'mode': 'promote', 'candidate': self.plan['candidate'],
                'pointerAttempted': not already, 'pointerChanged': not already, 'pointerOutcomeUnknown': False,
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
        self.assertEqual(P.digest(PLAN_RAW), '4a7eda51fd375b64a250faa8d1709f8b611af8d60172aabf7c1032b864e490fc')
        self.assertEqual(P.digest(P.canonical(REAL_PLAN)), P.NATIVE_SHA)
        self.assertEqual(len(REAL_PLAN['objects']), 36)
        self.assertEqual(sum(e['size'] for e in REAL_PLAN['objects']), 817657896)
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
                     'pointerChanged': False, 'pointerOutcomeUnknown': unknown}
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
        for raw in (PLAN_RAW.replace(b'"sequence": 12', b'"sequence": 13', 1), PLAN_RAW + b' ',
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
             patch.object(P, 'CONTROL_SECONDS', 0.025):
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


if __name__ == '__main__':
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(DeliveryTests)
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
