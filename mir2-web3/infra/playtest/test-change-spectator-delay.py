import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch, Mock

spec = importlib.util.spec_from_file_location('delay_change', Path(__file__).with_name('change-spectator-delay.py'))
change = importlib.util.module_from_spec(spec)
spec.loader.exec_module(change)

class DelayChangeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='mir2-owned-delay-qa-')
        self.root = Path(self.temporary.name).resolve()
        self.assertEqual(self.root.parent, Path(tempfile.gettempdir()).resolve())
        self.assertTrue(self.root.name.startswith('mir2-owned-delay-qa-'))
        self.folder = self.root / 'stage'
        self.folder.mkdir()
        self.caddy = self.root / 'Caddyfile'
        self.dropin = self.root / 'watch.conf'
        self.environment = self.folder / 'spectator-feature.env'
        self.caddy.write_bytes(b'candidate')
        (self.folder / 'Caddyfile.candidate').write_bytes(b'candidate')
        self.old_env, self.old_dropin = change.feature_values(30000)
        self.environment.write_bytes(self.old_env)
        self.dropin.write_bytes(self.old_dropin)
        self.record = {'revision': 'owned-revision', 'serviceFingerprint': 'pinned',
                       'proxyApplied': True, 'gatewayActivated': True, 'publicDelayMs': 30000,
                       'candidateSha256': change.stage.digest(b'candidate')}
        self.state = self.folder / 'status.json'
        self.state.write_text(json.dumps(self.record))
        self.health = {'ok': True, 'revision': 'owned-revision',
                       'capacity': {**{key: 0 for key in change.stage.COUNTERS},
                                    'maxWsConnections': 66, 'maxActiveSessions': 51},
                       'spectator': {'recordingEnabled': False}}
        self.patches = [
            patch.object(change.stage, 'STAGE', self.folder),
            patch.object(change.stage, 'CADDY', self.caddy),
            patch.object(change.stage, 'DROPIN', self.dropin),
            patch.object(change.stage, 'health', return_value=self.health),
            patch.object(change.stage, 'require_unchanged_service'),
            patch.object(change.stage, 'require_legacy_drained'),
            patch.object(change.stage, 'validate_barrier', return_value=b'barrier'),
            patch.object(change.stage, 'replace_caddy', side_effect=self.caddy.write_bytes),
            patch.object(change, 'directory', return_value={'publicDelayMs': 30000}),
            patch.object(change.subprocess, 'run'),
        ]
        self.mocks = [item.start() for item in self.patches]
        # feature_values depends on DROPIN only via the fixed EnvironmentFile stage path.
        self.old_env, self.old_dropin = change.feature_values(30000)
        self.environment.write_bytes(self.old_env)
        self.dropin.write_bytes(self.old_dropin)

    def tearDown(self):
        for item in reversed(self.patches):
            item.stop()
        self.temporary.cleanup()

    def prepare(self):
        with contextlib.redirect_stdout(io.StringIO()):
            change.prepare(3000)

    def test_only_delay_changes(self):
        new_env, new_dropin = change.feature_values(3000)
        old = b'MIR2_SPECTATOR_PUBLIC_DELAY_MS=30000'
        new = b'MIR2_SPECTATOR_PUBLIC_DELAY_MS=3000'
        self.assertEqual(new_env.replace(new, old), self.old_env)
        self.assertEqual(new_dropin.replace(new, old), self.old_dropin)
        for invalid in [0, 999, 30001, True, '3000']:
            with self.assertRaises(ValueError):
                change.feature_values(invalid)

    def test_prepare_with_player_does_not_change_live_files(self):
        self.health['capacity']['currentActiveSessions'] = 1
        self.prepare()
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertEqual(self.dropin.read_bytes(), self.old_dropin)
        self.mocks[-1].assert_not_called()
        self.assertEqual(self.caddy.read_bytes(), b'candidate')

    def test_busy_ingress_refuses_restart(self):
        self.prepare()
        self.health['capacity']['currentWsConnections'] = 1
        with self.assertRaisesRegex(RuntimeError, 'save and exit'):
            change.apply(3000)
        self.mocks[-1].assert_not_called()
        self.mocks[7].assert_not_called()
        self.assertEqual(self.environment.read_bytes(), self.old_env)

    def test_success_preserves_proxy_and_other_features(self):
        self.prepare()
        self.mocks[-2].return_value = {'publicDelayMs': 3000}
        with contextlib.redirect_stdout(io.StringIO()):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), change.feature_values(3000)[0])
        self.assertEqual(self.dropin.read_bytes(), change.feature_values(3000)[1])
        self.assertEqual(json.loads(self.state.read_bytes())['publicDelayMs'], 3000)
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        restarts = [call for call in self.mocks[-1].call_args_list if call.args[0][1] == 'restart']
        self.assertEqual(len(restarts), 1)

    def test_wrong_effective_delay_restores_both_files_and_proxy(self):
        self.prepare()
        with self.assertRaisesRegex(RuntimeError, 'Effective delay'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertEqual(self.dropin.read_bytes(), self.old_dropin)
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        self.assertEqual(json.loads(self.state.read_bytes())['publicDelayMs'], 30000)
        restarts = [call for call in self.mocks[-1].call_args_list if call.args[0][1] == 'restart']
        self.assertEqual(len(restarts), 2)

    def test_tampered_plan_refuses_mutation(self):
        self.prepare()
        plan_file = self.folder / 'delay-change-plan.json'
        plan = json.loads(plan_file.read_bytes())
        plan['newEnvironmentSha256'] = 'not-the-planned-bytes'
        plan_file.write_text(json.dumps(plan))
        with self.assertRaisesRegex(RuntimeError, 'plan changed'):
            change.apply(3000)
        self.mocks[-1].assert_not_called()
        self.assertEqual(self.environment.read_bytes(), self.old_env)

    def test_modified_owned_feature_is_not_overwritten(self):
        self.prepare()
        self.environment.write_bytes(b'another operator')
        with self.assertRaisesRegex(RuntimeError, 'feature changed'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), b'another operator')
        self.mocks[-1].assert_not_called()

    def test_concurrent_feature_change_prevents_rollback_overwrite(self):
        self.prepare()
        def run(command, **kwargs):
            if command[1] == 'restart':
                self.environment.write_bytes(b'another operator')
                raise RuntimeError('restart failed')
        self.mocks[-1].side_effect = run
        with self.assertRaisesRegex(RuntimeError, 'another operator'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), b'another operator')
        self.assertEqual(self.caddy.read_bytes(), b'candidate')

    def test_proxy_changed_during_validation_is_preserved_without_restart(self):
        self.prepare()
        def validate(candidate):
            self.caddy.write_bytes(b'another proxy rollout')
            return b'barrier'
        self.mocks[6].side_effect = validate
        with self.assertRaisesRegex(RuntimeError, 'Proxy changed'):
            change.apply(3000)
        self.assertEqual(self.caddy.read_bytes(), b'another proxy rollout')
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.mocks[7].assert_not_called()
        self.mocks[-1].assert_not_called()

    def test_state_changed_during_validation_is_preserved_without_restart(self):
        self.prepare()
        altered = json.dumps({**self.record, 'operatorNote': 'concurrent update'}).encode()
        def validate(candidate):
            self.state.write_bytes(altered)
            return b'barrier'
        self.mocks[6].side_effect = validate
        with self.assertRaisesRegex(RuntimeError, 'Stage record changed'):
            change.apply(3000)
        self.assertEqual(self.state.read_bytes(), altered)
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.mocks[7].assert_not_called()
        self.mocks[-1].assert_not_called()

    def test_state_changed_after_environment_reload_is_fenced_before_restart(self):
        self.prepare()
        altered = json.dumps({**self.record, 'operatorNote': 'changed during reload'}).encode()
        def run(command, **kwargs):
            if command[1] == 'daemon-reload':
                self.state.write_bytes(altered)
        self.mocks[-1].side_effect = run
        with self.assertRaisesRegex(RuntimeError, 'Stage record changed'):
            change.apply(3000)
        self.assertEqual(self.state.read_bytes(), altered)
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertEqual(self.dropin.read_bytes(), self.old_dropin)
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        self.assertFalse(any(call.args[0][1] == 'restart' for call in self.mocks[-1].call_args_list))

    def test_final_proxy_reload_failure_rolls_back_delay_and_reloads_original_proxy(self):
        self.prepare()
        failed = False
        active_proxy = b'candidate'
        def replace(content):
            nonlocal failed, active_proxy
            self.caddy.write_bytes(content)
            if content == b'candidate' and not failed:
                failed = True
                raise RuntimeError('final proxy reload failed after file replacement')
            active_proxy = content
        self.mocks[7].side_effect = replace
        self.mocks[-2].side_effect = lambda: {
            'publicDelayMs': 30000 if self.environment.read_bytes() == self.old_env else 3000}
        with self.assertRaisesRegex(RuntimeError, 'final proxy reload failed'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertEqual(self.dropin.read_bytes(), self.old_dropin)
        self.assertEqual(json.loads(self.state.read_bytes())['publicDelayMs'], 30000)
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        self.assertEqual(active_proxy, b'candidate')
        restarts = [call for call in self.mocks[-1].call_args_list if call.args[0][1] == 'restart']
        self.assertEqual(len(restarts), 2)

    def test_permanent_proxy_reload_failure_reports_incomplete_recovery(self):
        self.prepare()
        def replace(content):
            self.caddy.write_bytes(content)
            if content == b'candidate':
                raise RuntimeError('proxy reload still unavailable')
        self.mocks[7].side_effect = replace
        self.mocks[-2].side_effect = lambda: {
            'publicDelayMs': 30000 if self.environment.read_bytes() == self.old_env else 3000}
        with self.assertRaisesRegex(RuntimeError, 'automatic recovery needs attention'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertEqual(self.dropin.read_bytes(), self.old_dropin)
        self.assertEqual(json.loads(self.state.read_bytes())['publicDelayMs'], 30000)

    def test_foreign_proxy_change_before_restart_is_preserved(self):
        self.prepare()
        def run(command, **kwargs):
            if command[1] == 'daemon-reload':
                self.caddy.write_bytes(b'foreign proxy rollout')
        self.mocks[-1].side_effect = run
        with self.assertRaisesRegex(RuntimeError, 'Proxy changed'):
            change.apply(3000)
        self.assertEqual(self.caddy.read_bytes(), b'foreign proxy rollout')
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertFalse(any(call.args[0][1] == 'restart' for call in self.mocks[-1].call_args_list))

    def test_offline_startup_failure_restores_owned_files_without_unsafe_retry(self):
        self.prepare()
        def run(command, **kwargs):
            if command[1] == 'restart':
                self.mocks[3].side_effect = ConnectionRefusedError('gateway offline')
                raise RuntimeError('gateway startup failed')
        self.mocks[-1].side_effect = run
        with self.assertRaisesRegex(RuntimeError, 'automatic recovery needs attention'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), self.old_env)
        self.assertEqual(self.dropin.read_bytes(), self.old_dropin)
        self.assertEqual(json.loads(self.state.read_bytes())['publicDelayMs'], 30000)
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        restarts = [call for call in self.mocks[-1].call_args_list if call.args[0][1] == 'restart']
        self.assertEqual(len(restarts), 1)

    def test_foreign_feature_change_during_rollback_reload_prevents_second_restart(self):
        self.prepare()
        reloads = 0
        def run(command, **kwargs):
            nonlocal reloads
            if command[1] == 'daemon-reload':
                reloads += 1
                if reloads == 2:
                    self.environment.write_bytes(b'foreign feature during rollback')
        self.mocks[-1].side_effect = run
        with self.assertRaisesRegex(RuntimeError, 'automatic recovery needs attention.*feature changed'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), b'foreign feature during rollback')
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        restarts = [call for call in self.mocks[-1].call_args_list if call.args[0][1] == 'restart']
        self.assertEqual(len(restarts), 1)

    def test_foreign_feature_after_rollback_health_is_detected_and_preserved(self):
        self.prepare()
        calls = 0
        def directory():
            nonlocal calls
            calls += 1
            if calls == 2:
                self.environment.write_bytes(b'foreign feature after health')
            return {'publicDelayMs': 30000}
        self.mocks[-2].side_effect = directory
        with self.assertRaisesRegex(RuntimeError, 'automatic recovery needs attention.*feature changed'):
            change.apply(3000)
        self.assertEqual(self.environment.read_bytes(), b'foreign feature after health')
        self.assertEqual(self.caddy.read_bytes(), b'candidate')
        restarts = [call for call in self.mocks[-1].call_args_list if call.args[0][1] == 'restart']
        self.assertEqual(len(restarts), 2)

if __name__ == '__main__':
    unittest.main()
