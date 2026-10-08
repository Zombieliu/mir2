import importlib.util
from contextlib import contextmanager
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('stage_spectator', Path(__file__).with_name('stage-spectator.py'))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)

def healthy():
    return {'ok': True, 'revision': 'source-a', 'capacity': {key: 0 for key in stage.COUNTERS}}

def original_proxy():
    return ('watch.example {\n'
            '    @mir2_playtest path /playtest /playtest/*\n'
            '    handle @mir2_playtest {\n'
            '        route {\n'
            '            uri strip_prefix /playtest\n'
            f'            {stage.OLD_ROUTE}\n'
            '            handle @mir2_playtest_public {\n'
            '                reverse_proxy 127.0.0.1:7210\n'
            '            }\n'
            '        }\n'
            '    }\n'
            '    reverse_proxy 127.0.0.1:7110\n'
            '}\n').encode()

def adapted_barrier_routes():
    return [
        {'handle': [{'handler': 'rewrite', 'strip_path_prefix': '/playtest'}]},
        {'match': [{'path': ['/ws', '/spectator/ws']}],
         'handle': [{'handler': 'static_response', 'status_code': 503,
                     'body': 'Brief spectator activation'}]},
        {'handle': [{'handler': 'reverse_proxy', 'upstreams': [{'dial': '127.0.0.1:7210'}]}]},
    ]

def caddy_fixture_command(args):
    if args[0] == 'adapt':
        return json.dumps({'routes': adapted_shared_proxy_routes()}).encode()
    return b''

def adapted_shared_proxy_routes():
    return [
        {'match': [{'path': ['/ws', '/spectator/ws']}],
         'handle': [{'handler': 'subroute', 'routes': [
             {'handle': [{'handler': 'static_response', 'status_code': 503,
                           'body': 'Brief spectator activation (legacy realm)'}]},
         ]}]},
        {'match': [{'path': ['/playtest', '/playtest/*']}],
         'handle': [{'handler': 'subroute', 'routes': adapted_barrier_routes()}]},
        {'handle': [{'handler': 'reverse_proxy', 'upstreams': [{'dial': '127.0.0.1:7110'}]}]},
    ]

def fixture_response(url, **kwargs):
    payload = {'publicDelayMs': 30_000, 'matches': []} if url.endswith('/spectator/matches') else healthy()
    return io.BytesIO(json.dumps(payload).encode())

def fingerprint_fixture(environment_files, file_contents):
    """Exercise the real hash logic using a synthetic systemd readout."""
    binary = '/opt/mir2-playtest/releases/source-a/mir2-gateway'
    unit = '/etc/systemd/system/mir2-playtest.service'
    descriptors = environment_files if isinstance(environment_files, list) else [environment_files]
    properties = '\n'.join([
        f'FragmentPath={unit}',
        'DropInPaths=',
        *(f'EnvironmentFiles={descriptor}' for descriptor in descriptors),
        'User=ubuntu',
        'WorkingDirectory=/var/lib/mir2-playtest',
        f'ExecStart={{ path={binary}; argv[]={binary} web; ignore_errors=no; }}',
    ]).encode()
    files = {binary: b'owned binary fixture', unit: b'owned unit fixture', **file_contents}

    def read_file(path, mode):
        if mode != 'rb' or path not in files:
            raise AssertionError(f'Unexpected host file access in fingerprint test: {path}')
        return io.BytesIO(files[path])

    with patch.object(stage.subprocess, 'check_output', return_value=properties), \
         patch.object(stage.Path, 'glob', return_value=[]), patch('builtins.open', side_effect=read_file):
        return stage.service_fingerprint()

@contextmanager
def prepared_fixture(proxy_applied=True):
    """Use actual stage records and temporary files, never real host services."""
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        live = root / 'Caddyfile'
        dropin = root / '90-spectator-watch.conf'
        original = original_proxy()
        live.write_bytes(original)
        with patch.object(stage, 'STAGE', root / 'stage'), patch.object(stage, 'CADDY', live), \
             patch.object(stage, 'DROPIN', dropin), patch.object(stage, 'health', return_value=healthy()), \
             patch.object(stage, 'service_fingerprint', return_value='service-a'), \
             patch.object(stage.urllib.request, 'urlopen', side_effect=fixture_response), \
             patch.object(stage, 'caddy_command', side_effect=caddy_fixture_command), patch('builtins.print'):
            stage.prepare()
            record_path = stage.STAGE / 'status.json'
            record = json.loads(record_path.read_text())
            candidate = (stage.STAGE / 'Caddyfile.candidate').read_bytes()
            if proxy_applied:
                live.write_bytes(candidate)
                record['proxyApplied'] = True
                record_path.write_text(json.dumps(record))
            yield root, live, dropin, original, candidate

class StageSafetyTests(unittest.TestCase):
    def test_all_ingress_and_lease_counters_must_be_known_zero(self):
        stage.require_drained(healthy(), 'source-a')
        for key in stage.COUNTERS:
            with self.subTest(counter=key):
                active = healthy()
                active['capacity'][key] = 1
                with self.assertRaises(RuntimeError): stage.require_drained(active, 'source-a')
                del active['capacity'][key]
                with self.assertRaises(RuntimeError): stage.require_drained(active, 'source-a')

    def test_changed_or_unknown_revision_never_permits_restart(self):
        for observed in [{}, {**healthy(), 'ok': False}, {**healthy(), 'revision': 'source-b'}]:
            with self.assertRaises(RuntimeError): stage.require_drained(observed, 'source-a')

    def test_preparation_does_not_touch_live_proxy_or_gateway(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            live = root / 'Caddyfile'
            original = original_proxy()
            live.write_bytes(original)
            with patch.object(stage, 'STAGE', root / 'stage'), patch.object(stage, 'CADDY', live), \
                 patch.object(stage, 'health', return_value=healthy()), \
                 patch.object(stage, 'caddy_command', side_effect=caddy_fixture_command) as command, \
                 patch.object(stage, 'service_fingerprint', return_value='service-a'), \
                 patch('builtins.print'):
                stage.prepare()
                self.assertEqual(live.read_bytes(), original)
                self.assertGreaterEqual(command.call_count, 2)
                self.assertTrue(all(call.args[0][0] in {'validate', 'adapt'} for call in command.call_args_list))
                self.assertTrue(any(call.args[0][0] == 'validate' for call in command.call_args_list))
                self.assertTrue(any(call.args[0][0] == 'adapt' for call in command.call_args_list))
                candidate = (root / 'stage' / 'Caddyfile.candidate').read_text()
                self.assertIn('/spectator/ws', candidate)
                self.assertNotIn('/ai-live/control', candidate)
                self.assertNotIn('/distribution/heartbeat', candidate)
                self.assertNotIn('/admin', candidate)
                self.assertFalse(json.loads((root / 'stage' / 'status.json').read_text())['gatewayActivated'])

    def test_preparation_rejects_unexpected_proxy_instead_of_guessing(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            live = root / 'Caddyfile'
            live.write_text('some other operator configuration')
            with patch.object(stage, 'STAGE', root / 'stage'), patch.object(stage, 'CADDY', live), \
                 patch.object(stage, 'caddy_command') as command:
                with self.assertRaises(RuntimeError): stage.prepare()
                command.assert_not_called()
                self.assertEqual(live.read_text(), 'some other operator configuration')

    def test_preparation_refuses_already_applied_route_without_resetting_stage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            live = root / 'Caddyfile'
            applied = original_proxy().replace(stage.OLD_ROUTE.encode(), stage.NEW_ROUTE.encode())
            live.write_bytes(applied)
            stage_dir = root / 'stage'
            stage_dir.mkdir()
            existing_status = b'{"proxyApplied": true, "gatewayActivated": true}\n'
            (stage_dir / 'status.json').write_bytes(existing_status)
            with patch.object(stage, 'STAGE', stage_dir), patch.object(stage, 'CADDY', live), \
                 patch.object(stage, 'caddy_command') as command:
                with self.assertRaisesRegex(RuntimeError, 'Expected one original playtest route'):
                    stage.prepare()
                command.assert_not_called()
            self.assertEqual(live.read_bytes(), applied)
            self.assertEqual((stage_dir / 'status.json').read_bytes(), existing_status)
            self.assertFalse((stage_dir / 'Caddyfile.candidate').exists())

    def test_occupied_player_connection_blocks_proxy_reload_before_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            live = root / 'Caddyfile'
            live.write_bytes(b'untouched')
            (root / 'status.json').write_text(json.dumps({'revision': 'source-a'}))
            occupied = healthy()
            occupied['capacity']['currentWsConnections'] = 1
            with patch.object(stage, 'STAGE', root), patch.object(stage, 'CADDY', live), \
                 patch.object(stage, 'health', return_value=occupied), patch.object(stage, 'replace_caddy') as reload:
                with self.assertRaises(RuntimeError): stage.apply_proxy()
                reload.assert_not_called()
                self.assertEqual(live.read_bytes(), b'untouched')

    def test_failed_barrier_reload_restores_proxy_without_gateway_mutation(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            attempts = []

            def replace_then_fail_once(content):
                attempts.append(content)
                live.write_bytes(content)
                if len(attempts) == 1:
                    raise RuntimeError('simulated reload failure after disk replacement')

            with patch.object(stage, 'replace_caddy', side_effect=replace_then_fail_once), \
                 patch.object(stage.subprocess, 'run') as service_command:
                with self.assertRaisesRegex(RuntimeError, 'simulated reload failure'):
                    stage.activate()
                service_command.assert_not_called()
            self.assertEqual(len(attempts), 2)
            self.assertIn(b'@mir2_watch_maintenance', attempts[0])
            self.assertEqual(attempts[1], candidate)
            self.assertEqual(live.read_bytes(), candidate)
            self.assertFalse(dropin.exists())
            self.assertFalse(json.loads((stage.STAGE / 'status.json').read_text())['gatewayActivated'])

    def test_changed_service_refuses_activation_before_live_mutations(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            with patch.object(stage, 'service_fingerprint', return_value='another-release'), \
                 patch.object(stage, 'replace_caddy') as proxy_command, \
                 patch.object(stage.subprocess, 'run') as service_command:
                with self.assertRaisesRegex(RuntimeError, 'changed'):
                    stage.activate()
                proxy_command.assert_not_called()
                service_command.assert_not_called()
            self.assertEqual(live.read_bytes(), candidate)
            self.assertFalse(dropin.exists())

    def test_service_changed_during_barrier_is_restored_without_restart(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            attempts = []

            def replace(content):
                attempts.append(content)
                live.write_bytes(content)

            def fingerprint():
                return 'another-release' if attempts else 'service-a'

            with patch.object(stage, 'service_fingerprint', side_effect=fingerprint), \
                 patch.object(stage, 'replace_caddy', side_effect=replace), \
                 patch.object(stage.subprocess, 'run') as service_command:
                with self.assertRaisesRegex(RuntimeError, 'changed'):
                    stage.activate()
                service_command.assert_not_called()
            self.assertEqual(len(attempts), 2)
            self.assertEqual(attempts[-1], candidate)
            self.assertEqual(live.read_bytes(), candidate)
            self.assertFalse(dropin.exists())

    def test_activation_does_not_overwrite_concurrent_proxy_on_reload_failure(self):
        with prepared_fixture() as (_, live, dropin, _, _):
            other_operator = b'other operator owns this configuration\n'

            def concurrent_change(content):
                self.assertIn(b'@mir2_watch_maintenance', content)
                live.write_bytes(other_operator)
                raise RuntimeError('reload failed concurrently')

            with patch.object(stage, 'replace_caddy', side_effect=concurrent_change) as proxy_command, \
                 patch.object(stage.subprocess, 'run') as service_command:
                with self.assertRaisesRegex(RuntimeError, 'Proxy changed'):
                    stage.activate()
                self.assertEqual(proxy_command.call_count, 1)
                service_command.assert_not_called()
            self.assertEqual(live.read_bytes(), other_operator)
            self.assertFalse(dropin.exists())

    def test_apply_does_not_overwrite_concurrent_proxy_on_reload_failure(self):
        with prepared_fixture(proxy_applied=False) as (_, live, _, _, _):
            other_operator = b'concurrent proxy rollout\n'

            def concurrent_change(content):
                live.write_bytes(other_operator)
                raise RuntimeError('reload failed concurrently')

            with patch.object(stage.urllib.request, 'urlopen',
                              return_value=io.BytesIO(json.dumps(healthy()).encode())), \
                 patch.object(stage, 'replace_caddy', side_effect=concurrent_change) as proxy_command:
                with self.assertRaisesRegex(RuntimeError, 'Proxy changed'):
                    stage.apply_proxy()
                self.assertEqual(proxy_command.call_count, 1)
            self.assertEqual(live.read_bytes(), other_operator)
            self.assertFalse(json.loads((stage.STAGE / 'status.json').read_text())['proxyApplied'])

    def test_changed_service_refuses_proxy_apply_before_reload(self):
        with prepared_fixture(proxy_applied=False) as (_, live, _, original, _):
            with patch.object(stage, 'service_fingerprint', return_value='another-release'), \
                 patch.object(stage, 'replace_caddy') as proxy_command:
                with self.assertRaisesRegex(RuntimeError, 'changed'):
                    stage.apply_proxy()
                proxy_command.assert_not_called()
            self.assertEqual(live.read_bytes(), original)

    def test_occupied_original_realm_refuses_proxy_apply_before_reload(self):
        for counter in stage.COUNTERS:
            with self.subTest(counter=counter), prepared_fixture(proxy_applied=False) as (_, live, _, original, _):
                legacy = healthy()
                legacy['capacity'][counter] = 1
                with patch.object(stage.urllib.request, 'urlopen',
                                  return_value=io.BytesIO(json.dumps(legacy).encode())), \
                     patch.object(stage, 'replace_caddy') as proxy_command:
                    with self.assertRaisesRegex(RuntimeError, 'Original Gateway'):
                        stage.apply_proxy()
                    proxy_command.assert_not_called()
                self.assertEqual(live.read_bytes(), original)

    def test_original_realm_rejoining_after_apply_blocks_activation_reload(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            legacy = healthy()
            legacy['capacity']['currentWsConnections'] = 1
            with patch.object(stage.urllib.request, 'urlopen',
                              return_value=io.BytesIO(json.dumps(legacy).encode())), \
                 patch.object(stage, 'replace_caddy',
                              side_effect=AssertionError('Shared proxy reload must be refused')) as proxy_command, \
                 patch.object(stage.subprocess, 'run') as service_command:
                with self.assertRaisesRegex(RuntimeError, 'Original Gateway'):
                    stage.activate()
                proxy_command.assert_not_called()
                service_command.assert_not_called()
            self.assertEqual(live.read_bytes(), candidate)
            self.assertFalse(dropin.exists())

    def test_successful_activation_restores_admission_and_only_adds_owned_feature(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            activated_health = {**healthy(), 'spectator': {'recordingEnabled': False}}
            attempts = []

            def replace(content):
                attempts.append(content)
                live.write_bytes(content)

            with patch.object(stage, 'health', return_value=activated_health), \
                 patch.object(stage, 'replace_caddy', side_effect=replace), \
                 patch.object(stage.subprocess, 'run') as service_command:
                stage.activate()
            self.assertEqual([call.args[0] for call in service_command.call_args_list],
                             [['systemctl', 'daemon-reload'], ['systemctl', 'restart', 'mir2-playtest']])
            self.assertEqual(len(attempts), 2)
            self.assertIn(b'@mir2_watch_maintenance', attempts[0])
            self.assertEqual(attempts[1], candidate)
            self.assertEqual(live.read_bytes(), candidate)
            self.assertEqual(dropin.read_bytes(), stage.feature_dropin().encode())
            record = json.loads((stage.STAGE / 'status.json').read_text())
            self.assertTrue(record['gatewayActivated'])
            self.assertEqual(record['revision'], 'source-a')
            self.assertEqual(record['serviceFingerprint'], 'service-a')

    def test_feature_dropin_ends_with_owned_environment_file_and_intended_values(self):
        with prepared_fixture() as (_, _, _, _, _):
            feature_file = stage.STAGE / 'spectator-feature.env'
            staged_dropin = (stage.STAGE / '90-spectator-watch.conf').read_text()
            self.assertEqual(staged_dropin.splitlines()[-1], f'EnvironmentFile={feature_file}')
            self.assertEqual(staged_dropin, stage.feature_dropin())
            self.assertEqual(feature_file.read_bytes(), stage.FEATURE_ENV.encode())
            assignments = dict(line.split('=', 1) for line in feature_file.read_text().splitlines()
                               if line and not line.startswith('#'))
            self.assertEqual(assignments['MIR2_SPECTATOR_ENABLED'], '1')
            self.assertEqual(assignments['MIR2_SPECTATOR_PUBLIC'], '1')
            self.assertEqual(assignments['MIR2_SPECTATOR_RECORDING_ENABLED'], '0')
            self.assertEqual(assignments['MIR2_SPECTATOR_PUBLIC_MAPS'], '0')
            self.assertEqual(assignments['MIR2_SPECTATOR_CAPTURE_INTERVAL_MS'], '1000')
            self.assertTrue(all(name.startswith('MIR2_SPECTATOR_') for name in assignments))

    def test_identical_feature_environment_is_accepted_without_rewriting(self):
        with prepared_fixture() as (_, _, _, _, _):
            feature_file = stage.STAGE / 'spectator-feature.env'
            with patch.object(stage, 'write_private') as write:
                stage.prepare_feature_environment()
                write.assert_not_called()
            self.assertEqual(feature_file.read_bytes(), stage.FEATURE_ENV.encode())

    def test_modified_feature_environment_is_never_overwritten(self):
        with prepared_fixture() as (_, _, _, _, _):
            feature_file = stage.STAGE / 'spectator-feature.env'
            concurrent = b'MIR2_SPECTATOR_PUBLIC=0\nMIR2_DATABASE_URL=other-operator\n'
            feature_file.write_bytes(concurrent)
            with patch.object(stage, 'write_private') as write:
                with self.assertRaisesRegex(RuntimeError, 'feature environment changed'):
                    stage.prepare_feature_environment()
                write.assert_not_called()
            self.assertEqual(feature_file.read_bytes(), concurrent)

    def test_feature_environment_changed_during_barrier_prevents_restart(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            feature_file = stage.STAGE / 'spectator-feature.env'
            concurrent = b'MIR2_SPECTATOR_PUBLIC=0\nMIR2_DATABASE_URL=other-operator\n'
            attempts = []

            def replace(content):
                attempts.append(content)
                live.write_bytes(content)
                if len(attempts) == 1:
                    feature_file.write_bytes(concurrent)

            with patch.object(stage, 'replace_caddy', side_effect=replace), \
                 patch.object(stage.subprocess, 'run') as service_command:
                with self.assertRaisesRegex(RuntimeError, 'feature environment changed'):
                    stage.activate()
                service_command.assert_not_called()
            self.assertEqual(feature_file.read_bytes(), concurrent)
            self.assertFalse(dropin.exists())
            self.assertEqual(live.read_bytes(), candidate)

    def test_feature_environment_changed_during_daemon_reload_prevents_restart(self):
        with prepared_fixture() as (_, live, dropin, _, candidate):
            feature_file = stage.STAGE / 'spectator-feature.env'
            concurrent = b'MIR2_SPECTATOR_ENABLED=1\nMIR2_DATABASE_URL=other-operator\n'
            commands = []

            def service_command(command, **kwargs):
                commands.append(command)
                if command == ['systemctl', 'daemon-reload']:
                    feature_file.write_bytes(concurrent)
                else:
                    raise AssertionError('Changed feature environment must never reach Gateway restart')

            with patch.object(stage, 'replace_caddy', side_effect=live.write_bytes), \
                 patch.object(stage.subprocess, 'run', side_effect=service_command):
                with self.assertRaisesRegex(RuntimeError, 'feature environment changed'):
                    stage.activate()
            self.assertTrue(commands)
            self.assertTrue(all(command == ['systemctl', 'daemon-reload'] for command in commands))
            self.assertEqual(feature_file.read_bytes(), concurrent)
            self.assertFalse(dropin.exists())
            self.assertEqual(live.read_bytes(), candidate)

    def test_fingerprint_ignores_only_appended_owned_feature_environment(self):
        base_file = '/etc/mir2-playtest/gateway.env'
        base_descriptor = f'{base_file} (ignore_errors=no)'
        files = {base_file: b'MIR2_REALM_ID=owned-realm\n'}
        baseline = fingerprint_fixture(base_descriptor, files)
        for flag in ['yes', 'no']:
            with self.subTest(ignore_errors=flag):
                descriptor = base_descriptor + f' {stage.STAGE / "spectator-feature.env"} (ignore_errors={flag})'
                self.assertEqual(fingerprint_fixture(descriptor, files), baseline)

    def test_repeated_environment_file_properties_retain_realm_before_owned_feature(self):
        base_file = '/etc/mir2-playtest/gateway.env'
        base_descriptor = f'{base_file} (ignore_errors=no)'
        owned_descriptor = f'{stage.STAGE / "spectator-feature.env"} (ignore_errors=no)'
        files = {base_file: b'MIR2_REALM_ID=owned-realm\n'}
        baseline = fingerprint_fixture(base_descriptor, files)
        repeated = [base_descriptor, owned_descriptor]
        self.assertEqual(fingerprint_fixture(repeated, files), baseline)
        changed = {base_file: b'MIR2_REALM_ID=changed-realm\n'}
        self.assertNotEqual(fingerprint_fixture(repeated, changed), baseline)
        other_file = '/etc/mir2-playtest/other.env'
        self.assertNotEqual(
            fingerprint_fixture([*repeated, f'{other_file} (ignore_errors=no)'],
                                {**files, other_file: b'MIR2_GATEWAY_ID=other-unit\n'}),
            baseline)

    def test_fingerprint_keeps_other_environment_paths_contents_and_flags_pinned(self):
        base_file = '/etc/mir2-playtest/gateway.env'
        other_file = '/etc/mir2-playtest/other.env'
        descriptor = f'{base_file} (ignore_errors=no)'
        files = {base_file: b'MIR2_REALM_ID=owned-realm\n', other_file: b'MIR2_GATEWAY_ID=another-unit\n'}
        baseline = fingerprint_fixture(descriptor, files)
        self.assertNotEqual(fingerprint_fixture(descriptor + f' {other_file} (ignore_errors=no)', files), baseline)
        changed = {**files, base_file: b'MIR2_REALM_ID=changed-realm\n'}
        self.assertNotEqual(fingerprint_fixture(descriptor, changed), baseline)
        self.assertNotEqual(fingerprint_fixture(f'{base_file} (ignore_errors=yes)', files), baseline)

    def test_adapted_barrier_requires_prefix_strip_before_both_ws_routes(self):
        routes = adapted_barrier_routes()
        stage.require_barrier_order({'routes': routes})
        with self.assertRaisesRegex(RuntimeError, 'barrier order'):
            stage.require_barrier_order({'routes': [routes[1], routes[0], routes[2]]})
        routes[1]['match'][0]['path'] = ['/ws']
        with self.assertRaisesRegex(RuntimeError, 'barrier order'):
            stage.require_barrier_order({'routes': routes})

    def test_adapted_barrier_rejects_missing_and_ambiguous_gates(self):
        routes = adapted_barrier_routes()
        with self.assertRaisesRegex(RuntimeError, 'barrier order'):
            stage.require_barrier_order({'routes': [routes[0], routes[2]]})
        with self.assertRaisesRegex(RuntimeError, 'barrier order'):
            stage.require_barrier_order({'routes': [routes[0], routes[1], routes[1], routes[2]]})

    def test_legacy_gate_precedes_shared_proxy_and_preserves_playtest_order(self):
        routes = adapted_shared_proxy_routes()
        stage.require_legacy_barrier_order({'routes': routes})
        stage.require_barrier_order({'routes': routes})
        with self.assertRaisesRegex(RuntimeError, 'original realm admission barrier order'):
            stage.require_legacy_barrier_order({'routes': [routes[2], routes[0], routes[1]]})
        routes[0]['match'][0]['path'] = ['/spectator/ws']
        with self.assertRaisesRegex(RuntimeError, 'original realm admission barrier order'):
            stage.require_legacy_barrier_order({'routes': routes})

    def test_legacy_gate_rejects_absent_proxy_and_duplicate_gates(self):
        routes = adapted_shared_proxy_routes()
        with self.assertRaisesRegex(RuntimeError, 'original realm admission barrier order'):
            stage.require_legacy_barrier_order({'routes': [routes[0], routes[1]]})
        with self.assertRaisesRegex(RuntimeError, 'original realm admission barrier order'):
            stage.require_legacy_barrier_order({'routes': [routes[0], routes[0], routes[1], routes[2]]})

if __name__ == '__main__': unittest.main()
