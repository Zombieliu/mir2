"""Offline fake-env/transport tests. Never reads real credentials or uses sockets."""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import ssl
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('cf_read_probe_fixture', BASE / 'cloudflare_read_authority_probe.py')
P = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = P
SPEC.loader.exec_module(P)
MOCK_TOKEN = 'MOCK_SECRET_MARKER_ONLY_f931487dbd8040a4'
MOCK_ID = 'MOCK_TOKEN_ID_MARKER_ONLY'
WRONG_ACCOUNT = 'MOCK_WRONG_ACCOUNT_MARKER_ONLY'
BODY_MARKER = 'MOCK_RESPONSE_BODY_MARKER_ONLY'
ENV = {'CLOUDFLARE_API_TOKEN': MOCK_TOKEN, 'CLOUDFLARE_ACCOUNT_ID': P.ACCOUNT}
FIXTURES = BASE / 'fixtures'
RECORDS = []


def encoded(value):
    return json.dumps(value, separators=(',', ':')).encode('utf-8')


ACTIVE = encoded({'success': True, 'result': {'status': 'active', 'id': MOCK_ID},
                  'messages': [BODY_MARKER, MOCK_TOKEN], 'errors': []})
READABLE = encoded({'success': True, 'result': {'name': P.BUCKET, 'creation_date': BODY_MARKER},
                    'errors': [], 'messages': [MOCK_TOKEN]})


class QueueTransport:
    def __init__(self, responses=None):
        self.responses = list(responses if responses is not None else [(200, ACTIVE), (200, READABLE)])
        self.calls = []
    def get(self, path, token):
        self.calls.append((path, token))
        response = self.responses.pop(0)
        if isinstance(response, BaseException):
            raise response
        return response


class FakeResponse:
    def __init__(self, body, status=200, *, kind='application/json', encoding='', read_failure=False):
        self.status, self.body, self.kind, self.encoding = status, body, kind, encoding
        self.read_failure, self.reads, self.headers = read_failure, [], []
    def getheader(self, name, default=None):
        self.headers.append(name)
        if name == 'Content-Type':
            return self.kind
        if name == 'Content-Encoding':
            return self.encoding
        if name == 'Location':
            return 'https://' + BODY_MARKER + '.invalid/' + MOCK_TOKEN
        return default
    def read(self, amount):
        self.reads.append(amount)
        if self.read_failure:
            raise OSError(MOCK_TOKEN + BODY_MARKER)
        return self.body[:amount]


class FakeConnection:
    def __init__(self, response, *, request_failure=False, response_failure=False, close_failure=False):
        self.response = response
        self.request_failure, self.response_failure, self.close_failure = request_failure, response_failure, close_failure
        self.request_calls, self.debug_levels, self.closed = [], [], False
    def set_debuglevel(self, value):
        self.debug_levels.append(value)
    def request(self, method, path, body=None, headers=None):
        self.request_calls.append((method, path, body, headers))
        if self.request_failure:
            raise OSError(MOCK_TOKEN + WRONG_ACCOUNT)
    def getresponse(self):
        if self.response_failure:
            raise ssl.SSLError(MOCK_TOKEN + MOCK_ID)
        return self.response
    def close(self):
        self.closed = True
        if self.close_failure:
            raise OSError(MOCK_TOKEN + BODY_MARKER)


class ProbeTests(unittest.TestCase):
    def fixture(self, label):
        FIXTURES.mkdir(exist_ok=True)
        root = Path(tempfile.mkdtemp(prefix=label + '-', dir=FIXTURES))
        RECORDS.append({'label': label, 'root': str(root), 'fixtureOnly': True})
        return root

    def safe(self, result):
        raw = P.canonical(result).decode('ascii')
        for forbidden in (MOCK_TOKEN, MOCK_ID, WRONG_ACCOUNT, BODY_MARKER, 'Authorization', 'Bearer', 'Traceback'):
            self.assertTrue(forbidden not in raw, 'sensitive fixture marker or raw header must not be emitted')
        self.assertIn(result['result'], [code.value for code in P.Code])
        self.assertIs(type(result['passed']), bool)
        self.assertIs(type(result['token_active']), bool)
        self.assertIs(type(result['bucket_readable']), bool)
        self.assertEqual(result['r2_write_authority'], 'not_probed')
        self.assertEqual(result['worker_edit_authority'], 'not_probed')
        self.assertEqual(result['deployment_authority'], 'not_probed')
        self.assertEqual(result['scope'], P.empty_result()['scope'])
        return raw

    def run_cli(self, root, transport=None, env=None, args=None):
        transport = transport if transport is not None else QueueTransport()
        output = io.StringIO()
        receipt = root / 'REPORT.json'
        args = ['--receipt', str(receipt)] if args is None else args
        # Explicit dictionaries only: no os.environ get, actual secret or network.
        code = P.main(args, environ=ENV if env is None else env, transport=transport, output=output)
        self.assertEqual(len(output.getvalue().splitlines()), 1)
        result = json.loads(output.getvalue())
        self.safe(result)
        if receipt.exists() and receipt.is_file() and result['receipt_status'] == 'written':
            self.assertEqual(P.canonical(result), receipt.read_bytes())
        return code, result, transport

    def test_active_and_known_bucket_two_gets_only_success_never_claims_write(self):
        transport = QueueTransport()
        result = P.probe(ENV, transport)
        self.assertTrue(result['passed'])
        self.assertTrue(result['token_active'])
        self.assertTrue(result['bucket_readable'])
        self.assertEqual([call[0] for call in transport.calls], list(P.ALLOWED_PATHS))
        self.assertTrue(all(call[1] == MOCK_TOKEN for call in transport.calls), 'opaque token passed only to transport')
        self.safe(result)

    def test_success_cli_zero_and_fresh_regular_safe_receipt(self):
        root = self.fixture('success')
        code, result, _ = self.run_cli(root)
        self.assertEqual(code, 0)
        self.assertEqual(result['result'], 'ok')
        self.assertEqual(result['receipt_status'], 'written')
        self.assertEqual((root / 'REPORT.json').stat().st_nlink, 1)

    def test_missing_token_or_account_and_wrong_account_before_http(self):
        for label, env, expected in (
            ('missing-token', {'CLOUDFLARE_ACCOUNT_ID': P.ACCOUNT}, 'token_missing'),
            ('empty-token', {'CLOUDFLARE_API_TOKEN': '', 'CLOUDFLARE_ACCOUNT_ID': P.ACCOUNT}, 'token_missing'),
            ('missing-account', {'CLOUDFLARE_API_TOKEN': MOCK_TOKEN}, 'account_missing'),
            ('empty-account', {'CLOUDFLARE_API_TOKEN': MOCK_TOKEN, 'CLOUDFLARE_ACCOUNT_ID': ''}, 'account_missing'),
            ('wrong-account', {**ENV, 'CLOUDFLARE_ACCOUNT_ID': WRONG_ACCOUNT}, 'account_mismatch'),
            ('account-case', {**ENV, 'CLOUDFLARE_ACCOUNT_ID': P.ACCOUNT.upper()}, 'account_mismatch')):
            with self.subTest(label=label):
                code, result, transport = self.run_cli(self.fixture(label), env=env)
                self.assertEqual(code, 2)
                self.assertEqual(result['result'], expected)
                self.assertEqual(transport.calls, [])
                self.assertFalse(result['token_active'])

    def test_token_header_injection_or_invalid_token_before_http(self):
        for token in (MOCK_TOKEN + '\r\nInjected:bad', MOCK_TOKEN + ' ', '\x00', 12, 'x' * 4097):
            transport = QueueTransport()
            result = P.probe({**ENV, 'CLOUDFLARE_API_TOKEN': token}, transport)
            self.assertEqual(result['result'], 'token_invalid')
            self.assertEqual(transport.calls, [])
            self.safe(result)

    def test_disabled_expired_and_unknown_status_never_bucket_request(self):
        for status in ('disabled', 'expired', MOCK_TOKEN, None, []):
            body = encoded({'success': True, 'result': {'status': status, 'id': MOCK_ID}})
            transport = QueueTransport([(200, body), (200, READABLE)])
            result = P.probe(ENV, transport)
            self.assertFalse(result['passed'])
            self.assertEqual(len(transport.calls), 1)
            self.assertFalse(result['token_active'])
            self.safe(result)

    def test_first_step_http_failure_does_not_fetch_second(self):
        for status in (401, 403, 429, 500):
            with self.subTest(status=status):
                transport = QueueTransport([(status, encoded({'errors': [MOCK_TOKEN]}))])
                result = P.probe(ENV, transport)
                self.assertEqual(result['result'], 'http_error')
                self.assertEqual(result['verify_http_status'], status)
                self.assertEqual(len(transport.calls), 1)
                self.assertIsNone(result['bucket_http_status'])
                self.safe(result)

    def test_second_step_denied_preserves_active_but_unreadable(self):
        for status in (401, 403, 404, 503):
            transport = QueueTransport([(200, ACTIVE), (status, b'private server body ' + MOCK_TOKEN.encode())])
            result = P.probe(ENV, transport)
            self.assertEqual(result['bucket_http_status'], status)
            self.assertTrue(result['token_active'])
            self.assertFalse(result['bucket_readable'])
            self.assertFalse(result['passed'])
            self.assertEqual(len(transport.calls), 2)
            self.safe(result)

    def test_wrong_or_missing_bucket_name_refuses_exact_scope(self):
        for name in ('different-bucket-' + MOCK_TOKEN, None, [P.BUCKET], P.BUCKET.upper()):
            body = encoded({'success': True, 'result': {'name': name}})
            result = P.probe(ENV, QueueTransport([(200, ACTIVE), (200, body)]))
            self.assertEqual(result['result'], 'bucket_scope_mismatch')
            self.assertTrue(result['token_active'])
            self.assertFalse(result['bucket_readable'])
            self.safe(result)

    def test_redirect_never_followed_on_either_step(self):
        for step in (1, 2):
            for status in (301, 302, 303, 307, 308):
                replies = [(status, b'redirect ' + MOCK_TOKEN.encode())]
                if step == 2:
                    replies.insert(0, (200, ACTIVE))
                transport = QueueTransport(replies)
                result = P.probe(ENV, transport)
                self.assertEqual(result['result'], 'redirect_denied')
                self.assertEqual(len(transport.calls), step)
                self.safe(result)

    def test_malformed_non_json_utf8_duplicate_and_nonfinite_refuse(self):
        bodies = [b'<html>' + MOCK_TOKEN.encode() + b'</html>', b'\xff', b'{broken',
            b'{"success":true,"success":true,"result":{"status":"active"}}',
            b'{"success":true,"result":{"status":"active"},"metadata":NaN}']
        for body in bodies:
            transport = QueueTransport([(200, body)])
            result = P.probe(ENV, transport)
            self.assertEqual(result['result'], 'json_invalid')
            self.assertEqual(len(transport.calls), 1)
            self.safe(result)

    def test_schema_and_api_success_must_be_real_boolean(self):
        for value in ([], None, {'success': True, 'result': None}, {'success': True, 'result': []},
                      {'success': 1, 'result': {'status': 'active'}},
                      {'success': False, 'result': {'status': 'active'}, 'errors': [MOCK_TOKEN]}):
            transport = QueueTransport([(200, encoded(value))])
            result = P.probe(ENV, transport)
            self.assertFalse(result['passed'])
            self.assertEqual(len(transport.calls), 1)
            self.safe(result)

    def test_oversized_body_stops_before_second_request(self):
        transport = QueueTransport([(200, ACTIVE + b' ' * P.BODY_LIMIT)])
        result = P.probe(ENV, transport)
        self.assertEqual(result['result'], 'response_oversized')
        self.assertEqual(len(transport.calls), 1)
        self.safe(result)

    def test_arbitrary_transport_exception_strings_never_emitted(self):
        for error in (OSError(MOCK_TOKEN + BODY_MARKER), ssl.SSLError(MOCK_TOKEN),
                      RuntimeError(MOCK_TOKEN + WRONG_ACCOUNT), KeyboardInterrupt(MOCK_TOKEN)):
            transport = QueueTransport([error])
            result = P.probe(ENV, transport)
            self.assertFalse(result['passed'])
            self.assertEqual(len(transport.calls), 1)
            self.safe(result)

    def test_status_not_boolean_or_outside_real_http_range(self):
        for status in (True, '200-' + MOCK_TOKEN, 0, 999):
            result = P.probe(ENV, QueueTransport([(status, ACTIVE)]))
            self.assertEqual(result['result'], 'response_shape_invalid')
            self.assertIsNone(result['verify_http_status'])
            self.safe(result)

    def test_real_transport_construction_is_fixed_verified_no_proxy_get_no_body(self):
        context = types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        connections = [FakeConnection(FakeResponse(ACTIVE)), FakeConnection(FakeResponse(READABLE))]
        constructors = []
        def factory(host, *, port, timeout, context):
            constructors.append((host, port, timeout, context))
            return connections[len(constructors) - 1]
        # Only proxy markers are set; no actual Cloudflare environment values are read.
        with patch.object(P.ssl, 'create_default_context', return_value=context) as tls, \
             patch.object(P.http.client, 'HTTPSConnection', side_effect=factory), \
             patch.dict(os.environ, {'HTTPS_PROXY': 'http://proxy-' + MOCK_TOKEN + '.invalid',
                                     'HTTP_PROXY': 'http://another-' + MOCK_TOKEN + '.invalid'}, clear=True):
            result = P.probe(ENV)
        self.assertTrue(result['passed'])
        self.assertEqual(tls.call_count, 2)
        self.assertEqual([(c[0], c[1], c[2]) for c in constructors], [(P.HOST, 443, 10)] * 2)
        for index, connection in enumerate(connections):
            self.assertEqual(connection.debug_levels, [0])
            self.assertTrue(connection.closed)
            method, path, body, headers = connection.request_calls[0]
            self.assertEqual((method, path, body), ('GET', P.ALLOWED_PATHS[index], None))
            self.assertTrue(headers['Authorization'] == 'Bearer ' + MOCK_TOKEN, 'opaque header must be exact')
            self.assertEqual(set(headers), {'Authorization', 'Accept', 'Accept-Encoding', 'User-Agent'})
            self.assertEqual(headers['Accept-Encoding'], 'identity')
            self.assertEqual(connection.response.reads, [65 * 1024])
            self.assertTrue('Location' not in connection.response.headers)
        self.safe(result)

    def fake_https(self, connections, context=None):
        context = context or types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=True)
        cursor = iter(connections)
        with patch.object(P.ssl, 'create_default_context', return_value=context), \
             patch.object(P.http.client, 'HTTPSConnection', side_effect=lambda *_args, **_kwargs: next(cursor)) as factory:
            result = P.probe(ENV)
            return result, factory.call_count

    def test_tls_policy_must_keep_required_certificate_and_hostname(self):
        for context in (types.SimpleNamespace(verify_mode=ssl.CERT_NONE, check_hostname=True),
                        types.SimpleNamespace(verify_mode=ssl.CERT_REQUIRED, check_hostname=False)):
            result, calls = self.fake_https([], context)
            self.assertEqual(result['result'], 'tls_policy_error')
            self.assertEqual(calls, 0)
            self.safe(result)

    def test_actual_transport_http_redirect_or_auth_failure_does_not_read_body(self):
        for status in (302, 307, 401, 403, 500):
            connection = FakeConnection(FakeResponse(MOCK_TOKEN.encode(), status))
            result, calls = self.fake_https([connection])
            self.assertEqual(calls, 1)
            self.assertEqual(connection.response.reads, [])
            self.assertEqual(result['verify_http_status'], status)
            self.assertTrue(connection.closed)
            self.safe(result)

    def test_actual_transport_non_json_or_encoded_body_refuses_before_read(self):
        for response in (FakeResponse(ACTIVE, kind='text/html'), FakeResponse(ACTIVE, kind=MOCK_TOKEN),
                         FakeResponse(ACTIVE, encoding='gzip'), FakeResponse(ACTIVE, encoding=MOCK_TOKEN)):
            connection = FakeConnection(response)
            result, calls = self.fake_https([connection])
            self.assertEqual(calls, 1)
            self.assertEqual(response.reads, [])
            self.assertFalse(result['passed'])
            self.safe(result)

    def test_actual_transport_read_bound_handles_64k_and_denies_over_64k(self):
        for size, allowed in ((P.BODY_LIMIT, True), (P.BODY_LIMIT + 1, False), (P.READ_LIMIT + 1000, False)):
            first = FakeConnection(FakeResponse(ACTIVE + b' ' * (size - len(ACTIVE))))
            second = FakeConnection(FakeResponse(READABLE))
            result, calls = self.fake_https([first, second])
            self.assertEqual(first.response.reads, [P.READ_LIMIT])
            self.assertEqual(result['passed'], allowed)
            self.assertEqual(calls, 2 if allowed else 1)
            if not allowed:
                self.assertEqual(result['result'], 'response_oversized')
            self.safe(result)

    def test_actual_transport_request_response_read_failures_sanitized_and_closed(self):
        for connection in (FakeConnection(FakeResponse(ACTIVE), request_failure=True),
                           FakeConnection(FakeResponse(ACTIVE), response_failure=True),
                           FakeConnection(FakeResponse(ACTIVE, read_failure=True))):
            result, calls = self.fake_https([connection])
            self.assertEqual(result['result'], 'network_error')
            self.assertEqual(calls, 1)
            self.assertTrue(connection.closed)
            self.safe(result)

    def test_real_transport_refuses_any_other_path_before_connection(self):
        with patch.object(P.http.client, 'HTTPSConnection') as factory:
            with self.assertRaises(P.SafeFailure):
                P.FixedHttpsTransport().get('/client/v4/accounts/' + WRONG_ACCOUNT, MOCK_TOKEN)
            self.assertEqual(factory.call_count, 0)

    def test_cli_auth_failures_exit2_safe_summary_and_receipt(self):
        for status in (401, 403):
            code, result, transport = self.run_cli(self.fixture('cli-http-' + str(status)),
                transport=QueueTransport([(status, MOCK_TOKEN.encode())]))
            self.assertEqual(code, 2)
            self.assertEqual(result['result'], 'http_error')
            self.assertEqual(len(transport.calls), 1)

    def test_existing_file_no_overwrite_and_no_http(self):
        root = self.fixture('existing-file')
        receipt = root / 'REPORT.json'
        receipt.write_bytes(b'preserve existing proof')
        code, result, transport = self.run_cli(root)
        self.assertEqual(code, 2)
        self.assertEqual(result['result'], 'receipt_error')
        self.assertEqual(transport.calls, [])
        self.assertEqual(receipt.read_bytes(), b'preserve existing proof')

    def test_receipt_symlink_hardlink_or_directory_no_overwrite_no_http(self):
        for kind in ('symlink', 'hardlink', 'directory'):
            root = self.fixture('receipt-' + kind)
            target = root / 'preserved-original.json'
            target.write_bytes(b'unchanged linked proof')
            receipt = root / 'REPORT.json'
            if kind == 'symlink':
                receipt.symlink_to(target)
            elif kind == 'hardlink':
                os.link(target, receipt)
            else:
                receipt.mkdir()
            code, result, transport = self.run_cli(root)
            self.assertEqual(code, 2)
            self.assertEqual(result['result'], 'receipt_error')
            self.assertEqual(transport.calls, [])
            self.assertEqual(target.read_bytes(), b'unchanged linked proof')

    def test_receipt_parent_symlink_refuses_without_creating_target(self):
        root = self.fixture('receipt-parent-link')
        actual = root / 'real-parent'
        actual.mkdir()
        link = root / 'linked-parent'
        link.symlink_to(actual, target_is_directory=True)
        code, result, transport = self.run_cli(root, args=['--receipt', str(link / 'REPORT.json')])
        self.assertEqual(code, 2)
        self.assertEqual(result['result'], 'receipt_error')
        self.assertEqual(transport.calls, [])
        self.assertFalse((actual / 'REPORT.json').exists())

    def test_bad_argument_with_mock_secret_does_not_echo(self):
        root = self.fixture('arguments')
        for args in ([], ['--receipt'], ['--other', MOCK_TOKEN], ['--receipt', 'x', MOCK_TOKEN]):
            code, result, transport = self.run_cli(root, args=args)
            self.assertEqual(code, 2)
            self.assertEqual(result['result'], 'arguments_invalid')
            self.assertEqual(transport.calls, [])

    def test_missing_parent_or_path_traversal_receipt_error_no_http(self):
        root = self.fixture('bad-receipt-path')
        for name in (str(root / 'absent/REPORT.json'), str(root / 'nested/../REPORT.json')):
            code, result, transport = self.run_cli(root, args=['--receipt', name])
            self.assertEqual(code, 2)
            self.assertEqual(result['result'], 'receipt_error')
            self.assertEqual(transport.calls, [])

    def test_receipt_write_error_exception_text_redacted_exit2(self):
        root = self.fixture('write-error')
        class BrokenStream:
            def write(self, _data):
                raise OSError(MOCK_TOKEN + WRONG_ACCOUNT)
            def close(self):
                raise OSError(MOCK_TOKEN + BODY_MARKER)
        with patch.object(P, 'reserve_receipt', return_value=BrokenStream()):
            code, result, transport = self.run_cli(root)
        self.assertEqual(code, 2)
        self.assertEqual(result['result'], 'receipt_error')
        self.assertEqual(len(transport.calls), 2)
        self.assertEqual(result['receipt_status'], 'not_written')

    def test_environment_lookup_exception_never_emits_exception_or_input(self):
        class BrokenEnvironment:
            def get(self, _name):
                raise RuntimeError(MOCK_TOKEN + WRONG_ACCOUNT)
        transport = QueueTransport()
        result = P.probe(BrokenEnvironment(), transport)
        self.assertEqual(result['result'], 'internal_error')
        self.assertEqual(transport.calls, [])
        self.safe(result)


if __name__ == '__main__':
    started = int(time.time())
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ProbeTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    receipt = {'schema': 'mir2.cloudflare.read-probe-local-tests.v1', 'createdUnix': int(time.time()),
        'startedUnix': started, 'tests': result.testsRun, 'passed': result.wasSuccessful(),
        'failures': len(result.failures), 'errors': len(result.errors), 'skipped': len(result.skipped),
        'python': sys.version, 'platform': sys.platform,
        'sourceSha256': hashlib.sha256((BASE / 'cloudflare_read_authority_probe.py').read_bytes()).hexdigest(),
        'testSha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'actualCredentialsRead': False, 'authenticatedNetworkRequest': False,
        'fakeTransportOnly': True, 'fixturesRetained': RECORDS}
    path = BASE / ('TEST-RECEIPT-' + str(time.time_ns()) + '.json')
    with path.open('xb') as stream:
        stream.write(encoded(receipt) + b'\n')
    print('RAW_RECEIPT=' + str(path))
    raise SystemExit(0 if result.wasSuccessful() else 1)
