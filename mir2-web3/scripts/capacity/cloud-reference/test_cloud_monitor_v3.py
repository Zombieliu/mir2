"""Synthetic local failure fixtures only; no real SSH, services or credentials."""
import ast
import copy
import hashlib
import io
import json
import math
import sys
import tempfile
import types
import unittest
from contextlib import ExitStack, redirect_stdout
from pathlib import Path
from unittest.mock import patch

import cloud_monitor_v2 as v2
import cloud_monitor_v3 as monitor
import test_cloud_monitor_v2 as legacy

HERE = Path(__file__).resolve().parent


class V2Regression(legacy.MonitorTests):
    def setUp(self):
        replacement = patch.object(legacy, 'monitor', monitor)
        replacement.start()
        self.addCleanup(replacement.stop)


def run_stream(factory, channel_type=legacy.Channel, atomic_fault=None, transport_type=legacy.Transport):
    clock = legacy.Clock()
    payload = factory(clock)
    channel = channel_type(payload, status=0, stderr=b'PRIVATE stderr fixture')
    transport = transport_type(channel)
    tcp = types.SimpleNamespace(shutdown=lambda unused: None, close=lambda: None)
    pin = 'SHA256:' + legacy.base64.b64encode(hashlib.sha256(legacy.Key().asbytes()).digest()).decode().rstrip('=')
    with tempfile.TemporaryDirectory(dir=legacy.TEST_ROOT) as name, ExitStack() as stack:
        directory = Path(name)
        stack.enter_context(patch.object(monitor, 'REPOSITORY', directory / 'fake-repository'))
        stack.enter_context(patch.object(monitor, 'time', clock))
        stack.enter_context(patch.object(monitor, 'KNOWN_HOST_KEY', pin))
        stack.enter_context(patch.object(monitor.sys.stdin, 'isatty', return_value=True))
        stack.enter_context(patch.object(monitor.getpass, 'getpass', return_value='PRIVATE password fixture'))
        connect = stack.enter_context(patch.object(monitor.socket, 'create_connection', return_value=tcp))
        stack.enter_context(patch.dict(sys.modules, {'paramiko': types.SimpleNamespace(Transport=lambda unused: transport)}))
        if atomic_fault is not None:
            original = monitor.atomic_write
            def write(path, value):
                if atomic_fault(value): raise ValueError('PRIVATE persistence error and coordinates')
                return original(path, value)
            stack.enter_context(patch.object(monitor, 'atomic_write', side_effect=write))
        with redirect_stdout(io.StringIO()):
            code = monitor.main(legacy.cli(directory))
        latest = json.loads((directory / 'latest.json').read_bytes())
        history = [json.loads(row) for row in (directory / 'history.jsonl').read_bytes().splitlines()]
        raw = (directory / 'latest.json').read_bytes() + (directory / 'history.jsonl').read_bytes()
        assert b'PRIVATE' not in raw
        return code, latest, history, transport, channel, connect.call_count


def json_lines(*values):
    return b''.join(json.dumps(value).encode() + b'\n' for value in values)


class GuardEquivalence(unittest.TestCase):
    def args(self):
        return types.SimpleNamespace(expected_pid=7, expected_start_ticks=9, expected_invocation_id='1' * 32,
                                     expected_revision=legacy.REV, stale_seconds=12)

    def test_v2_immutable_and_remote_bytes_identical(self):
        self.assertEqual(hashlib.sha256((HERE / 'cloud_monitor_v2.py').read_bytes()).hexdigest(),
                         'bf108ff79128799b2b52ec97575e1f30562342eaad30b0c773c3a224d03d2312')
        self.assertEqual(monitor.REMOTE.encode(), v2.REMOTE.encode())
        compile(monitor.REMOTE, '<offline-remote>', 'exec')

    def test_old_guard_helpers_ast_are_identical(self):
        def functions(module):
            return {node.name: node for node in ast.parse(Path(module.__file__).read_bytes()).body
                    if isinstance(node, ast.FunctionDef)}
        old, new = functions(v2), functions(monitor)
        for name in ['repository_root', 'outside_repository', 'atomic_write', 'parse_arguments',
                     'expected_identity', 'identity_matches', 'verify_host_key']:
            self.assertEqual(ast.dump(old[name], include_attributes=False),
                             ast.dump(new[name], include_attributes=False), name)
        class RemoveDiagnosticRaise(ast.NodeTransformer):
            count = 0
            def visit_Raise(self, node):
                if isinstance(node.exc, ast.Call) and isinstance(node.exc.func, ast.Name) and node.exc.func.id == 'sample_error':
                    self.count += 1
                    node.exc = ast.Call(func=ast.Name(id='ValueError', ctx=ast.Load()), args=[node.exc.args[0]], keywords=[])
                return node
        transform = RemoveDiagnosticRaise()
        normalized = transform.visit(copy.deepcopy(new['validate_sample']))
        self.assertEqual(transform.count, 4)
        self.assertEqual(ast.dump(old['validate_sample'], include_attributes=False),
                         ast.dump(normalized, include_attributes=False))

    def test_validation_acceptance_and_exact_boundaries_match_v2(self):
        clock = legacy.Clock()
        with patch.object(v2, 'time', clock), patch.object(monitor, 'time', clock):
            values = [None, [], {}, {'stop': 1}, {'stop': False}, legacy.sample(clock)]
            for age in [12000, 12001, -5000, -5001]:
                value = legacy.sample(clock); value['sampledAt'] -= age; values.append(value)
            for stamp in [None, True, 'PRIVATE time', [], float('nan'), float('inf'), 10 ** 400]:
                value = legacy.sample(clock); value['sampledAt'] = stamp; values.append(value)
            for identity in [None, [], {}, {'sampleBefore': legacy.PINS}]:
                value = legacy.sample(clock); value['monitorIdentity'] = identity; values.append(value)
            for key in ['targetUnit', 'targetProcess', 'health']:
                value = legacy.sample(clock); value[key] = None; values.append(value)
            value = legacy.sample(clock); value['health'] = {'nested': {'revision': legacy.REV}}; values.append(value)
            for stop, reason in [(True, 'remote guard'), (True, 'finite monitor duration completed')]:
                value = {'sampledAt': round(clock.time() * 1000), 'stop': stop, 'reason': reason}; values.append(value)
            # V2 does not reject schemaVersion alone; no new acceptance rule is introduced.
            value = legacy.sample(clock); value['schemaVersion'] = 99; values.append(value)
            for value in values:
                outcomes = []
                for module in [v2, monitor]:
                    try: module.validate_sample(value, self.args()); outcomes.append(None)
                    except Exception as error: outcomes.append(type(error))
                self.assertEqual(outcomes[0], outcomes[1])

    def test_passing_stream_and_terminal_have_no_added_diagnostic_fields(self):
        with patch.object(legacy, 'monitor', v2): old = legacy.run_local()
        with patch.object(legacy, 'monitor', monitor): new = legacy.run_local()
        self.assertEqual(old[:3], new[:3])
        self.assertNotIn('localFailure', new[1])


class FailureDiagnostics(unittest.TestCase):
    def rejected(self, change, expected):
        def factory(clock):
            bad = legacy.sample(clock); change(bad)
            bad['PRIVATE token cookie command address coordinates'] = {'PRIVATE nested': [12, 34]}
            good = legacy.sample(clock, 1); good.update(stop=True, reason='finite monitor duration completed')
            return json_lines(bad, good)
        result = run_stream(factory)
        code, latest, history, transport, channel, connects = result
        self.assertEqual(code, 2); self.assertEqual(latest['sampleCount'], 0)
        self.assertEqual(latest['localFailure']['code'], expected)
        self.assertEqual(latest['localFailure']['phase'], 'validate')
        self.assertEqual(latest['localFailure']['rejectedSampleCount'], 1)
        self.assertEqual(len(history), 2)  # startup + failure terminal; following valid frame is not accepted.
        self.assertNotIn('health', latest)
        self.assertEqual(transport.auth_calls, 1); self.assertEqual(connects, 1)
        self.assertTrue(channel.closed and channel.stdin_closed)
        self.assertEqual(latest['outcome'], 'failure')
        return latest['localFailure']

    def test_stale_and_future_use_exact_original_age(self):
        for age, code in [(12001, 'validate.stale'), (-5001, 'validate.future')]:
            with self.subTest(age=age):
                meta = self.rejected(lambda x: x.update(sampledAt=x['sampledAt'] - age), code)
                self.assertEqual(meta['rejectedSampleProjection']['ageMs'], age)
                self.assertEqual(meta['rejectedSampleProjection']['staleBudgetMs'], 12000)

    def test_schema_and_timestamp_type_are_distinct(self):
        self.rejected(lambda x: x.update(stop=1), 'validate.schema')
        meta = self.rejected(lambda x: x.update(sampledAt='PRIVATE timestamp'), 'validate.timestamp_schema')
        self.assertIsNone(meta['rejectedSampleProjection']['ageMs'])
        self.assertIsNone(meta['rejectedSampleProjection']['sampledAtMs'])

    def test_nondictionary_frames_are_rejected_before_count_or_persistence(self):
        for bad in [None, ['PRIVATE coordinates', 12, 34], 'PRIVATE token', True]:
            with self.subTest(type=type(bad).__name__):
                code, latest, history, transport, unused, connects = run_stream(
                    lambda clock: json_lines(bad, legacy.sample(clock)))
                self.assertEqual(code, 2); self.assertEqual(latest['sampleCount'], 0)
                self.assertEqual(latest['localFailure']['code'], 'validate.schema')
                self.assertFalse(latest['localFailure']['rejectedSampleProjection']['sampleObject'])
                self.assertEqual(len(history), 2); self.assertEqual(connects, 1); self.assertEqual(transport.auth_calls, 1)

    def test_identity_activation_shape_fields_and_flat_health_are_distinct(self):
        identity = self.rejected(lambda x: x['monitorIdentity']['sampleAfter'].update(invocationId='PRIVATE invocation'), 'validate.identity')
        self.assertTrue(identity['rejectedSampleProjection']['sampleBeforeMatches'])
        self.assertFalse(identity['rejectedSampleProjection']['sampleAfterMatches'])
        self.rejected(lambda x: x.update(targetProcess=[]), 'validate.activation_shape')
        self.rejected(lambda x: x['targetUnit'].update(MainPID=8), 'validate.activation_fields')
        meta = self.rejected(lambda x: x.update(health={'nested': {'revision': legacy.REV}}), 'validate.flat_health_revision')
        self.assertFalse(meta['rejectedSampleProjection']['healthRevisionMatches'])

    def test_json_decode_nonfinite_and_encoding_never_store_raw_line_or_continue(self):
        for payload, expected in [(b'{"PRIVATE":"bad"\n', 'decode.json'),
                                  (b'{"PRIVATE":NaN}\n', 'decode.nonfinite'),
                                  (b'{"PRIVATE":Infinity}\n', 'decode.nonfinite'),
                                  (b'{"PRIVATE":-Infinity}\n', 'decode.nonfinite'),
                                  (b'{"PRIVATE":"\xff"}\n', 'decode.encoding')]:
            with self.subTest(expected=expected):
                result = run_stream(lambda clock: payload + json_lines(legacy.sample(clock)))
                code, latest, history, transport, unused, connects = result
                self.assertEqual(code, 2); self.assertEqual(latest['sampleCount'], 0)
                self.assertEqual(latest['localFailure']['code'], expected)
                self.assertEqual(latest['localFailure']['phase'], 'decode')
                self.assertIsNone(latest['localFailure']['rejectedSampleProjection'])
                self.assertEqual(latest['localFailure']['lineBytes'], len(payload) - 1)
                self.assertEqual(len(history), 2); self.assertEqual(transport.auth_calls, 1); self.assertEqual(connects, 1)

    def test_local_channel_exception_has_only_fixed_class_and_code(self):
        class BrokenChannel(legacy.Channel):
            def recv(self, maximum): raise ValueError('PRIVATE raw library error: auth command and coordinates')
        code, latest, history, transport, unused, connects = run_stream(lambda clock: json_lines(legacy.sample(clock)), BrokenChannel)
        self.assertEqual(code, 2)
        self.assertEqual(latest['localFailure']['phase'], 'channelRead')
        self.assertEqual(latest['localFailure']['code'], 'channel.read_exception')
        self.assertEqual(latest['localFailure']['errorClass'], 'ValueError')
        self.assertIsNone(latest['localFailure']['rejectedSampleProjection'])
        self.assertEqual(latest['sampleCount'], 0); self.assertEqual(transport.auth_calls, 1); self.assertEqual(connects, 1)

    def test_channel_open_command_and_status_failures_keep_distinct_phases(self):
        class OpenFailure(legacy.Transport):
            def open_session(self, timeout): raise ValueError('PRIVATE channel open detail')
        class CommandFailure(legacy.Channel):
            def exec_command(self, command): raise ValueError('PRIVATE raw command')
        class StatusFailure(legacy.Channel):
            def exit_status_ready(self): raise ValueError('PRIVATE channel status detail')
        cases = [(legacy.Channel, OpenFailure, 'channelOpen', 'channel.open_exception', 0),
                 (CommandFailure, legacy.Transport, 'remoteCommand', 'channel.command_exception', 0),
                 (StatusFailure, legacy.Transport, 'channelStatus', 'channel.status_exception', 1)]
        for channel_type, transport_type, phase, expected, count in cases:
            with self.subTest(phase=phase):
                code, latest, unused, transport, unused_channel, connects = run_stream(
                    lambda clock: json_lines(legacy.sample(clock)), channel_type, transport_type=transport_type)
                self.assertEqual(code, 2); self.assertEqual(latest['sampleCount'], count)
                self.assertEqual(latest['localFailure']['phase'], phase)
                self.assertEqual(latest['localFailure']['code'], expected)
                self.assertIsNone(latest['localFailure']['rejectedSampleProjection'])
                self.assertEqual(connects, 1); self.assertEqual(transport.auth_calls, 1)
                self.assertFalse(transport.active)

    def test_persist_exception_is_classified_without_raw_error_or_failed_latest(self):
        def factory(clock):
            first = legacy.sample(clock)
            second = legacy.sample(clock, 1); second['remoteMonotonicNs'] += 1
            return json_lines(first, second)
        first_mono = legacy.Clock().monotonic_ns()
        code, latest, history, unused, channel, connects = run_stream(factory, atomic_fault=lambda x: not x.get('monitorTerminal') and x.get('remoteMonotonicNs') == first_mono + 1)
        self.assertEqual(code, 2); self.assertEqual(connects, 1); self.assertTrue(channel.closed)
        self.assertEqual(latest['localFailure']['code'], 'persist.exception')
        self.assertEqual(latest['localFailure']['phase'], 'persist')
        self.assertTrue(latest['localFailure']['candidateCountedBeforeFailure'])
        self.assertFalse(latest['localFailure']['candidatePersisted'])
        self.assertEqual(latest['remoteMonotonicNs'], first_mono)
        self.assertEqual(latest['sampleCount'], 2)  # V2 updates this before persistence; diagnostics expose the failed attempt.

    def test_valid_json_exponent_nonfinite_fails_persist_without_reemitting_frame(self):
        def factory(clock):
            value = json.dumps(legacy.sample(clock))[:-1] + ',"PRIVATE rejected field":1e999}\n'
            return value.encode()
        code, latest, history, unused, unused_channel, connects = run_stream(factory)
        self.assertEqual(code, 2); self.assertEqual(connects, 1)
        self.assertEqual(latest['localFailure']['code'], 'persist.exception')
        self.assertFalse(latest['localFailure']['candidatePersisted'])
        self.assertNotIn('health', latest)
        self.assertEqual(len(history), 2)

    def test_projection_is_strictly_numeric_boolean_or_null_and_bounded(self):
        clock = legacy.Clock(); value = legacy.sample(clock)
        value.update(sampledAt=10 ** 400, remoteMonotonicNs=float('inf'), x=123, y=456,
                     token='PRIVATE', environment={'PRIVATE': 'cookie'}, coordinates=[123, 456])
        a = GuardEquivalence().args()
        projection = monitor.sample_projection(value, a, float('nan'))
        self.assertTrue(all(v is None or type(v) in (int, float, bool) for v in projection.values()))
        self.assertIsNone(projection['sampledAtMs']); self.assertIsNone(projection['remoteMonotonicNs']); self.assertIsNone(projection['ageMs'])
        self.assertFalse(projection['sampledAtWithinProjectionRange'])
        encoded = json.dumps(projection, allow_nan=False)
        for token in ['PRIVATE', 'coordinates', 'environment', 'token']:
            self.assertNotIn(token, encoded)

    def test_unexpected_class_name_and_projection_failure_cannot_leak_or_replace_failure(self):
        secret_error = type('PRIVATEError', (Exception,), {})('PRIVATE error body')
        info = monitor.failure_diagnostics(secret_error, 'channelRead', None, None, None, False, {})
        self.assertEqual(info['errorClass'], 'Other')
        self.assertNotIn('PRIVATE', json.dumps(info))
        with patch.object(monitor, '_failure_diagnostics', side_effect=RuntimeError('PRIVATE observer failure')):
            def factory(clock):
                value = legacy.sample(clock); value['sampledAt'] -= 12001
                return json_lines(value, legacy.sample(clock))
            code, latest, unused, transport, unused_channel, connects = run_stream(factory)
        self.assertEqual(code, 2); self.assertEqual(latest['sampleCount'], 0)
        self.assertEqual(latest['localFailure']['code'], 'diagnostic.unavailable')
        self.assertEqual(connects, 1); self.assertEqual(transport.auth_calls, 1)

    def test_unexpected_library_error_attributes_cannot_supply_controlled_diagnostics(self):
        error = ValueError('PRIVATE library detail')
        error.monitor_phase = 'persist'
        error.monitor_code = 'validate.identity'
        error.monitor_projection = {'PRIVATE token': 'PRIVATE', 'ageMs': 777}
        info = monitor.failure_diagnostics(error, 'channelRead', None, None, None, False, {})
        self.assertEqual(info['phase'], 'channelRead')
        self.assertEqual(info['code'], 'channel.read_exception')
        self.assertIsNone(info['rejectedSampleProjection'])
        self.assertNotIn('PRIVATE', json.dumps(info))


if __name__ == '__main__':
    unittest.main(verbosity=2)
