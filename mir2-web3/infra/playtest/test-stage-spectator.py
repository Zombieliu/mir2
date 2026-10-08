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
            self.assertEqual(dropin.read_bytes(), stage.FEATURE.encode())
            record = json.loads((stage.STAGE / 'status.json').read_text())
            self.assertTrue(record['gatewayActivated'])
            self.assertEqual(record['revision'], 'source-a')
            self.assertEqual(record['serviceFingerprint'], 'service-a')

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
