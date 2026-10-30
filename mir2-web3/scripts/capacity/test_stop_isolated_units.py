import importlib.util
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest import mock


spec = importlib.util.spec_from_file_location('stop_isolated_units', Path(__file__).with_name('stop_isolated_units.py'))
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)
GW = 'mir2-capacity-5000-20261030-gateway.service'
RD = 'mir2-capacity-5000-20261030-redis.service'


def state(**changes):
    result = {'LoadState': 'loaded', 'ActiveState': 'active', 'MainPID': '123',
              'InvocationID': '1' * 32,
              'Transient': 'yes', 'KillSignal': '2', 'SendSIGKILL': 'no',
              'Result': 'success', 'ExecMainCode': '0', 'ExecMainStatus': '0'}
    result.update(changes)
    return result


def inactive(**changes):
    return state(**({'ActiveState': 'inactive', 'MainPID': '0', 'ExecMainCode': '1'} | changes))


def absent():
    return state(LoadState='not-found', ActiveState='inactive', MainPID='0',
                 InvocationID='', Transient='no', KillSignal='15', SendSIGKILL='yes')


class FakeRunner:
    def __init__(self, states, stop_error=0):
        self.states = {unit: list(values) for unit, values in states.items()}
        self.stop_error = stop_error
        self.calls = []

    def __call__(self, argv, **kwargs):
        self.calls.append((argv, kwargs))
        action, unit = argv[2:4]
        if action == 'stop':
            return subprocess.CompletedProcess(argv, self.stop_error, '', 'PRIVATE-DO-NOT-PRINT')
        values = self.states[unit]
        value = values.pop(0) if len(values) > 1 else values[0]
        if isinstance(value, Exception):
            raise value
        if isinstance(value, tuple):
            return subprocess.CompletedProcess(argv, value[0], value[1], 'PRIVATE-DO-NOT-PRINT')
        return subprocess.CompletedProcess(argv, 0, ''.join(k + '=' + str(v) + '\n' for k, v in value.items()), '')

    @property
    def stops(self):
        return [call[0] for call in self.calls if call[0][2] == 'stop']


class StopTests(unittest.TestCase):
    def test_garbage_collected_is_noop_not_graceful_exit_proof(self):
        runner = FakeRunner({GW: [absent()]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertTrue(result['passed'])
        self.assertFalse(result['units'][0]['normalMainExitVerified'])
        self.assertEqual(result['units'][0]['outcome'], 'already_absent')
        self.assertEqual(runner.stops, [])

    def test_already_inactive_is_idempotent(self):
        runner = FakeRunner({GW: [inactive()]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertTrue(result['passed'])
        self.assertTrue(result['units'][0]['normalMainExitVerified'])
        self.assertEqual(runner.stops, [])

    def test_active_normal_stop_is_verified(self):
        runner = FakeRunner({GW: [state(), state(), inactive()]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertTrue(result['passed'])
        self.assertEqual(len(runner.stops), 1)
        self.assertEqual(result['units'][0]['outcome'], 'stopped_normally')
        self.assertTrue(result['units'][0]['normalMainExitVerified'])
        self.assertFalse(result['applicationSaveVerified'])

    def test_gc_after_stop_is_terminal_without_inventing_exit_record(self):
        runner = FakeRunner({GW: [state(), state(), absent()]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertTrue(result['passed'])
        self.assertEqual(result['units'][0]['outcome'], 'absent_after_stop')
        self.assertFalse(result['units'][0]['normalMainExitVerified'])

    def test_gc_race_before_stop_does_not_call_stop(self):
        runner = FakeRunner({GW: [state(), absent()]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertTrue(result['passed'])
        self.assertEqual(runner.stops, [])

    def test_unsafe_signal_and_kill_policy_rejected_before_any_stop(self):
        for changes in ({'KillSignal': '15'}, {'SendSIGKILL': 'yes'}):
            runner = FakeRunner({GW: [state()], RD: [state(**changes)]})
            result = tool.operate([GW, RD], execute=True, runner=runner)
            self.assertFalse(result['passed'])
            self.assertEqual(result['error'], 'unsafe_stop_policy')
            self.assertEqual(runner.stops, [])

    def test_persistent_lookalike_is_never_stopped(self):
        runner = FakeRunner({GW: [state(Transient='no')]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'persistent_unit_refused')
        self.assertEqual(runner.stops, [])

    def test_missing_property_rejects(self):
        value = state(); value.pop('MainPID')
        result = tool.operate([GW], runner=FakeRunner({GW: [value]}))
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'show_property_missing')

    def test_show_error_does_not_trust_even_absent_output(self):
        for error in (1, 4, 5):
            runner = FakeRunner({GW: [(error, 'LoadState=not-found\nActiveState=inactive\nMainPID=0\n')]})
            result = tool.operate([GW], execute=True, runner=runner)
            self.assertFalse(result['passed'])
            self.assertEqual(result['error'], 'systemctl_show_failed')
            self.assertEqual(runner.stops, [])

    def test_show_permission_and_timeout_fail_closed(self):
        cases = [(PermissionError('private'), 'systemctl_permission_denied'),
                 (subprocess.TimeoutExpired('private', 3), 'systemctl_timeout')]
        for error, category in cases:
            result = tool.operate([GW], runner=FakeRunner({GW: [error]}))
            self.assertFalse(result['passed'])
            self.assertEqual(result['error'], category)
            self.assertNotIn('private', json.dumps(result))

    def test_nonzero_main_exit_is_not_laundered_as_inactive(self):
        for value in (inactive(Result='exit-code', ExecMainStatus='7'),
                      inactive(Result='success', ExecMainStatus='7')):
            runner = FakeRunner({GW: [value]})
            result = tool.operate([GW], execute=True, runner=runner)
            self.assertFalse(result['passed'])
            self.assertEqual(result['error'], 'abnormal_main_exit')
            self.assertEqual(result['units'][0]['before']['ExecMainStatus'], 7)
            self.assertEqual(runner.stops, [])

    def test_loaded_inactive_without_exit_record_is_not_verified_after_stop(self):
        runner = FakeRunner({GW: [state(), state(), inactive(ExecMainCode='0')]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'main_exit_not_retained')
        self.assertEqual(result['units'][0]['after']['ExecMainCode'], 0)

    def test_post_stop_abnormal_record_is_retained(self):
        runner = FakeRunner({GW: [state(), state(), inactive(Result='exit-code', ExecMainStatus='7')]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['units'][0]['after']['Result'], 'exit-code')
        self.assertEqual(result['units'][0]['after']['ExecMainStatus'], 7)

    def test_failed_main_after_stop_and_signal_exit_reject(self):
        for value in (state(ActiveState='failed', MainPID='0', Result='signal', ExecMainCode='2', ExecMainStatus='2'),
                      inactive(ExecMainCode='2')):
            result = tool.operate([GW], execute=True, runner=FakeRunner({GW: [state(), state(), value]}))
            self.assertFalse(result['passed'])

    def test_stop_exit5_gc_race_preserves_return_code_without_normal_exit_claim(self):
        runner = FakeRunner({GW: [state(), state(), absent()]}, stop_error=5)
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertTrue(result['passed'])
        self.assertEqual(result['units'][0]['stopReturnCode'], 5)
        self.assertEqual(result['units'][0]['outcome'], 'absent_after_stop_exit5_race')
        self.assertFalse(result['units'][0]['normalMainExitVerified'])

    def test_stop_exit5_without_gc_and_other_stop_errors_fail_closed(self):
        for code, after in [(5, inactive()), (1, absent()), (4, absent()), (7, absent())]:
            runner = FakeRunner({GW: [state(), state(), after]}, stop_error=code)
            result = tool.operate([GW], execute=True, runner=runner)
            self.assertFalse(result['passed'])
            self.assertEqual(result['error'], 'systemctl_stop_failed')
            self.assertEqual(result['units'][0]['stopReturnCode'], code)

    def test_stop_exit5_with_failed_followup_probe_is_not_success(self):
        runner = FakeRunner({GW: [state(), state(), (1, '')]}, stop_error=5)
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'systemctl_show_failed')
        self.assertEqual(result['units'][0]['stopReturnCode'], 5)

    def test_new_activation_terminal_before_stop_is_not_the_old_instance(self):
        runner = FakeRunner({GW: [state(), inactive(InvocationID='2' * 32)]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'unit_activation_changed')
        self.assertEqual(runner.stops, [])

    def test_new_activation_terminal_after_stop_cannot_claim_normal_exit(self):
        runner = FakeRunner({GW: [state(), state(), inactive(InvocationID='2' * 32)]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'unit_activation_changed')
        self.assertNotIn('normalMainExitVerified', result['units'][0])

    def test_reused_pid_with_new_activation_rejects(self):
        runner = FakeRunner({GW: [state(), state(InvocationID='2' * 32)]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'unit_activation_changed')
        self.assertEqual(runner.stops, [])

    def test_missing_and_malformed_loaded_invocation_ids_reject(self):
        value = state(); value.pop('InvocationID')
        result = tool.operate([GW], runner=FakeRunner({GW: [value]}))
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'show_property_missing')
        for invocation in ('', '1' * 31, 'g' * 32, '0' * 32, '1' * 32 + ' '):
            result = tool.operate([GW], runner=FakeRunner({GW: [state(InvocationID=invocation)]}))
            self.assertFalse(result['passed'])
            self.assertEqual(result['error'], 'show_invocation_id_invalid')

    def test_gc_may_omit_invocation_id(self):
        value = absent(); value.pop('InvocationID')
        result = tool.operate([GW], execute=True, runner=FakeRunner({GW: [value]}))
        self.assertTrue(result['passed'])
        self.assertIsNone(result['units'][0]['before']['InvocationID'])
        self.assertFalse(result['units'][0]['normalMainExitVerified'])

    def test_process_replacement_before_stop_rejects(self):
        runner = FakeRunner({GW: [state(), state(MainPID='456')]})
        result = tool.operate([GW], execute=True, runner=runner)
        self.assertFalse(result['passed'])
        self.assertEqual(result['error'], 'unit_process_replaced')
        self.assertEqual(runner.stops, [])

    def test_plan_queries_only_show_and_never_stops(self):
        runner = FakeRunner({GW: [state()], RD: [inactive()]})
        output = io.StringIO()
        with mock.patch.object(tool.sys, 'platform', 'linux'):
            code = tool.main([GW, RD], runner=runner, output=output)
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(output.getvalue())['mode'], 'plan')
        self.assertEqual(runner.stops, [])

    def test_explicit_execute_cli_requires_root_and_targets_only_exact_units(self):
        runner = FakeRunner({GW: [state(), state(), inactive()]})
        output = io.StringIO()
        with mock.patch.object(tool.sys, 'platform', 'linux'), mock.patch.object(tool.os, 'geteuid', return_value=0, create=True):
            self.assertEqual(tool.main(['--execute', GW], runner=runner, output=output), 0)
        self.assertEqual(runner.stops, [[tool.SYSTEMCTL, '--no-ask-password', 'stop', GW]])
        self.assertEqual(runner.calls[0][1]['stdin'], subprocess.DEVNULL)
        self.assertFalse(runner.calls[0][1]['check'])
        self.assertEqual(runner.calls[0][1]['env'], tool.ENVIRONMENT)

    def test_execute_nonroot_refuses_before_probing(self):
        runner = FakeRunner({GW: [state()]})
        with mock.patch.object(tool.sys, 'platform', 'linux'), mock.patch.object(tool.os, 'geteuid', return_value=1000, create=True):
            code = tool.main(['--execute', GW], runner=runner, output=io.StringIO())
        self.assertEqual(code, 1)
        self.assertEqual(runner.calls, [])

    def test_malformed_units_duplicates_mixed_instances_and_flags_reject(self):
        cases = [['mir2-gateway.service'], ['mir2-playtest.service'], ['caddy.service'],
                 [GW, GW], [GW, 'mir2-capacity-other-redis.service'],
                 ['--execute'], ['--execute', '--execute', GW], [GW + ';echo'],
                 ['../' + GW], [GW + '\n'], ['--root=/tmp', GW], []]
        for args in cases:
            runner = FakeRunner({GW: [state()]})
            with mock.patch.object(tool.sys, 'platform', 'linux'):
                self.assertEqual(tool.main(args, runner=runner, output=io.StringIO()), 1, args)
            self.assertEqual(runner.calls, [], args)

    def test_timeout_waiting_for_inactive_does_not_force_kill(self):
        now = [0.0]
        runner = FakeRunner({GW: [state(), state(), state(ActiveState='deactivating')]})
        result = tool.operate([GW], execute=True, runner=runner, clock=lambda: now[0],
                              sleep=lambda seconds: now.__setitem__(0, now[0] + seconds), timeout_seconds=.4)
        self.assertFalse(result['passed'])
        self.assertIn(result['error'], ('operation_timeout', 'systemctl_timeout'))
        self.assertEqual(len(runner.stops), 1)

    def test_unexpected_or_duplicate_property_never_leaks_payload(self):
        for text in ('Environment=TOP-SECRET\n', 'MainPID=123\nMainPID=456\n'):
            result = tool.operate([GW], runner=FakeRunner({GW: [(0, text)]}))
            self.assertFalse(result['passed'])
            self.assertNotIn('TOP-SECRET', json.dumps(result))


if __name__ == '__main__':
    unittest.main()
