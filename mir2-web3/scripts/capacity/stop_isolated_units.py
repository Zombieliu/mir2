#!/usr/bin/env python3
"""Plan, or explicitly stop, one transient capacity instance's Gateway/Redis.

Default: read-only systemctl show. Add --execute to request SIGINT-based stops.
An absent/GC unit is an idempotent no-op, never proof of graceful game saving.
No shell, kill, restart, reset-failed, environment dump, or arbitrary commands.
"""
import json
import os
import re
import subprocess
import sys
import time


SYSTEMCTL = '/usr/bin/systemctl'
UNIT = re.compile(r'(mir2-capacity-[a-z0-9]+(?:-[a-z0-9]+){0,7})-(gateway|redis)\.service', re.ASCII)
PROPERTIES = ('LoadState', 'ActiveState', 'MainPID', 'InvocationID', 'Transient', 'KillSignal',
              'SendSIGKILL', 'Result', 'ExecMainCode', 'ExecMainStatus')
ENVIRONMENT = {'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LANG': 'C.UTF-8'}


class StopError(Exception):
    def __init__(self, category, return_code=None):
        super().__init__(category)
        self.category = category
        self.return_code = return_code


def require(value, category):
    if not value:
        raise StopError(category)


def validate_units(units):
    require(isinstance(units, (list, tuple)) and 1 <= len(units) <= 2, 'invalid_unit_count')
    require(all(isinstance(u, str) and len(u) <= 160 and UNIT.fullmatch(u) for u in units), 'unit_not_allowed')
    require(len(set(units)) == len(units), 'duplicate_unit')
    require(len({UNIT.fullmatch(u).group(1) for u in units}) == 1, 'mixed_instance_units')
    return list(units)


def invoke(arguments, runner, timeout):
    require(timeout > 0, 'operation_timeout')
    try:
        result = runner([SYSTEMCTL, '--no-ask-password', *arguments],
                        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, check=False,
                        timeout=timeout, env=ENVIRONMENT)
    except subprocess.TimeoutExpired:
        raise StopError('systemctl_timeout') from None
    except PermissionError:
        raise StopError('systemctl_permission_denied') from None
    except OSError:
        raise StopError('systemctl_unavailable') from None
    if result.returncode != 0:
        raise StopError('systemctl_' + arguments[0] + '_failed', return_code=result.returncode)
    require(isinstance(result.stdout, str) and len(result.stdout) <= 16384, 'systemctl_output_invalid')
    return result.stdout


def inspect(unit, runner, timeout=3):
    raw = invoke(['show', unit, '--property=' + ','.join(PROPERTIES)], runner, timeout)
    values = {}
    for line in raw.splitlines():
        key, separator, value = line.partition('=')
        require(separator and key in PROPERTIES and key not in values, 'show_properties_invalid')
        values[key] = value
    require(set(PROPERTIES) - {'InvocationID'} <= set(values), 'show_property_missing')
    for key in ('MainPID', 'ExecMainCode', 'ExecMainStatus'):
        require(re.fullmatch(r'[0-9]{1,10}', values[key]) is not None, 'show_number_invalid')
        values[key] = int(values[key])
    require(values['MainPID'] <= 2147483647, 'show_pid_invalid')
    require(values['LoadState'] in ('loaded', 'not-found', 'masked'), 'unit_load_state_invalid')
    invocation = values.get('InvocationID', '')
    if values['LoadState'] == 'not-found' and invocation in ('', '0' * 32):
        values['InvocationID'] = None
    else:
        require('InvocationID' in values, 'show_property_missing')
        require(re.fullmatch(r'[0-9a-fA-F]{32}', invocation) is not None
                and invocation != '0' * 32, 'show_invocation_id_invalid')
        values['InvocationID'] = invocation.lower()
    require(values['ActiveState'] in ('active', 'inactive', 'failed', 'activating', 'deactivating', 'reloading'), 'unit_active_state_invalid')
    require(values['Transient'] in ('yes', 'no') and values['SendSIGKILL'] in ('yes', 'no'), 'show_boolean_invalid')
    require(re.fullmatch(r'[a-z-]{1,40}', values['Result']) is not None, 'show_result_invalid')
    require(re.fullmatch(r'(?:[0-9]{1,3}|SIG[A-Z0-9]{1,12})', values['KillSignal']) is not None, 'show_signal_invalid')
    if values['LoadState'] == 'not-found':
        require(values['ActiveState'] == 'inactive' and values['MainPID'] == 0, 'absent_unit_inconsistent')
    elif values['ActiveState'] == 'inactive':
        require(values['MainPID'] == 0, 'inactive_unit_has_pid')
    return values


def terminal(snapshot, after_stop=False):
    """Return terminal evidence, or None while still running. Never invent GC exit data."""
    if snapshot['LoadState'] == 'not-found':
        return {'outcome': 'already_absent' if not after_stop else 'absent_after_stop',
                'terminalStateVerified': True, 'normalMainExitVerified': False,
                'exitEvidence': 'not_retained_after_garbage_collection'}
    require(snapshot['ActiveState'] != 'failed', 'unit_failed')
    if snapshot['ActiveState'] != 'inactive':
        return None
    require(snapshot['Result'] == 'success' and snapshot['ExecMainStatus'] == 0, 'abnormal_main_exit')
    normal = snapshot['ExecMainCode'] == 1  # CLD_EXITED, status 0.
    require(snapshot['ExecMainCode'] in (0, 1), 'abnormal_main_exit')
    if after_stop:
        require(normal, 'main_exit_not_retained')
    return {'outcome': 'already_inactive' if not after_stop else 'stopped_normally',
            'terminalStateVerified': True, 'normalMainExitVerified': normal,
            'exitEvidence': 'observed_exit_zero' if normal else 'no_main_exit_record'}


def safe_active(snapshot):
    require(snapshot['LoadState'] == 'loaded' and snapshot['ActiveState'] == 'active'
            and snapshot['MainPID'] > 1, 'unit_not_safely_active')
    require(snapshot['Transient'] == 'yes', 'persistent_unit_refused')
    require(snapshot['KillSignal'] in ('2', 'SIGINT') and snapshot['SendSIGKILL'] == 'no', 'unsafe_stop_policy')


def same_activation(initial, snapshot):
    # Missing GC state is not exit evidence. Any retained unit must still be the
    # original activation, even if a replacement has already exited with PID 0.
    if snapshot['LoadState'] != 'not-found':
        require(snapshot['InvocationID'] == initial['InvocationID'], 'unit_activation_changed')


def operate(units, execute=False, runner=subprocess.run, clock=time.monotonic,
            sleep=time.sleep, timeout_seconds=30):
    units = validate_units(units)
    require(type(execute) is bool and 0 < timeout_seconds <= 120, 'invalid_operation_options')
    report = {'schemaVersion': 1, 'mode': 'execute' if execute else 'plan',
              'scope': 'isolated-capacity-unit-operations-only', 'passed': False,
              'stopCommandsRequested': 0, 'applicationSaveVerified': False, 'units': []}
    deadline = clock() + timeout_seconds

    def probe(unit):
        return inspect(unit, runner, min(3, deadline - clock()))

    try:
        # Check every requested unit before performing any stop.
        for unit in units:
            snapshot = probe(unit)
            row = {'unit': unit, 'before': snapshot, 'action': 'not_requested'}
            report['units'].append(row)
            evidence = terminal(snapshot)
            if evidence is None:
                safe_active(snapshot)
            row.update(action='no_op' if evidence else 'stop_with_configured_sigint', **(evidence or {}))
        if execute:
            for row in report['units']:
                if row['action'] == 'no_op':
                    continue
                fresh = probe(row['unit'])
                row['preStop'] = fresh
                same_activation(row['before'], fresh)
                evidence = terminal(fresh)
                if evidence is not None:
                    row.update(after=fresh, **evidence)
                    row['action'] = 'no_op_became_terminal'
                    continue
                safe_active(fresh)
                require(fresh['MainPID'] == row['before']['MainPID'], 'unit_process_replaced')
                report['stopCommandsRequested'] += 1
                try:
                    invoke(['stop', row['unit']], runner, min(20, deadline - clock()))
                    row['stopReturnCode'] = 0
                except StopError as error:
                    if error.return_code is not None:
                        row['stopReturnCode'] = error.return_code
                    if error.category != 'systemctl_stop_failed' or error.return_code != 5:
                        raise
                    gone = probe(row['unit'])
                    row['after'] = gone
                    same_activation(row['before'], gone)
                    # Only exit 5 plus independently observed GC absence is a
                    # benign race. Permission/timeout/other errors stay errors.
                    if gone['LoadState'] != 'not-found' or gone['MainPID'] != 0:
                        raise
                    row.update(terminal(gone, after_stop=True))
                    row['outcome'] = 'absent_after_stop_exit5_race'
                    continue
                while True:
                    snapshot = probe(row['unit'])
                    row['after'] = snapshot
                    same_activation(row['before'], snapshot)
                    evidence = terminal(snapshot, after_stop=True)
                    if evidence is not None:
                        row.update(evidence)
                        break
                    require(snapshot['MainPID'] in (0, fresh['MainPID']), 'unit_process_replaced')
                    require(snapshot['ActiveState'] in ('active', 'deactivating'), 'unexpected_post_stop_state')
                    require(clock() < deadline, 'operation_timeout')
                    sleep(min(.2, max(0, deadline - clock())))
        report['passed'] = True
    except StopError as error:
        report['error'] = error.category
    return report


def main(argv=None, runner=subprocess.run, output=None):
    """Small CLI: [--execute] UNIT [UNIT]. No arbitrary executable/path input."""
    argv = list(sys.argv[1:] if argv is None else argv)
    output = sys.stdout if output is None else output
    try:
        execute = bool(argv and argv[0] == '--execute')
        units = validate_units(argv[1:] if execute else argv)
        require(sys.platform == 'linux', 'linux_required')
        if execute:
            require(os.geteuid() == 0, 'root_required_for_execute')
        report = operate(units, execute=execute, runner=runner)
    except StopError as error:
        report = {'schemaVersion': 1, 'passed': False, 'error': error.category,
                  'stopCommandsRequested': 0}
    except Exception:
        # Never echo subprocess output, exceptions, argv, ExecStart, or environment.
        report = {'schemaVersion': 1, 'passed': False, 'error': 'unexpected_internal_error'}
    print(json.dumps(report, sort_keys=True), file=output)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
