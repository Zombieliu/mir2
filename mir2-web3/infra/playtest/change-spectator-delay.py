#!/usr/bin/env python3
"""Prepare a spectator-only delay change; --apply needs drained player ingress."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.request

spec = importlib.util.spec_from_file_location('spectator_stage', Path(__file__).with_name('stage-spectator.py'))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)

def feature_values(delay_ms):
    if type(delay_ms) is not int or not 1000 <= delay_ms <= 30000:
        raise ValueError('Public delay must be 1000..30000 milliseconds')
    old = 'MIR2_SPECTATOR_PUBLIC_DELAY_MS=30000'
    new = f'MIR2_SPECTATOR_PUBLIC_DELAY_MS={delay_ms}'
    return stage.FEATURE_ENV.replace(old, new, 1).encode(), stage.feature_dropin().replace(old, new, 1).encode()

def directory():
    with urllib.request.urlopen('http://127.0.0.1:7210/spectator/matches', timeout=8) as response:
        return json.load(response)

def current_inputs(proxy_digest=None):
    state_bytes = (stage.STAGE / 'status.json').read_bytes()
    record = json.loads(state_bytes)
    if not record.get('proxyApplied') or not record.get('gatewayActivated'):
        raise RuntimeError('Spectator must already be activated')
    stage.require_unchanged_service(record)
    if stage.digest(stage.CADDY.read_bytes()) != (proxy_digest or record['candidateSha256']):
        raise RuntimeError('Proxy changed; do not restart another rollout')
    old_env, old_dropin = feature_values(record['publicDelayMs'])
    if (stage.STAGE / 'spectator-feature.env').read_bytes() != old_env or stage.DROPIN.read_bytes() != old_dropin:
        raise RuntimeError('Owned spectator feature changed; refuse to overwrite it')
    return record, state_bytes, old_env, old_dropin

def require_owned_configuration(record, proxy, environment, dropin):
    stage.require_unchanged_service(record)
    if stage.CADDY.read_bytes() != proxy:
        raise RuntimeError('Proxy changed; refuse to overwrite another operator')
    if ((stage.STAGE / 'spectator-feature.env').read_bytes() != environment
            or stage.DROPIN.read_bytes() != dropin):
        raise RuntimeError('Owned spectator feature changed; refuse to overwrite it')

def require_snapshot(record, state_bytes, proxy, environment, dropin):
    if (stage.STAGE / 'status.json').read_bytes() != state_bytes:
        raise RuntimeError('Stage record changed; prepare again before activation')
    require_owned_configuration(record, proxy, environment, dropin)

def wait_for_delay(record, delay_ms, limits):
    deadline = time.monotonic() + 30
    while True:
        try:
            observed = stage.health()
            if observed.get('ok') and observed.get('revision') == record['revision']:
                if directory().get('publicDelayMs') != delay_ms:
                    raise RuntimeError('Effective delay did not change')
                if observed.get('spectator', {}).get('recordingEnabled') is not False:
                    raise RuntimeError('Recording must remain disabled')
                if any(observed.get('capacity', {}).get(key) != value for key, value in limits.items()):
                    raise RuntimeError('Player limits changed')
                return
        except OSError:
            pass
        if time.monotonic() >= deadline:
            raise RuntimeError('Gateway did not pass delay activation checks')
        time.sleep(1)

def prepare(delay_ms):
    new_env, new_dropin = feature_values(delay_ms)
    record, state_bytes, old_env, old_dropin = current_inputs()
    observed = stage.health()
    if not observed.get('ok') or observed.get('revision') != record['revision']:
        raise RuntimeError('Gateway revision changed')
    if directory().get('publicDelayMs') != record['publicDelayMs']:
        raise RuntimeError('Effective public delay differs from the owned stage')
    plan = {'revision': record['revision'], 'serviceFingerprint': record['serviceFingerprint'],
            'stateSha256': stage.digest(state_bytes), 'oldDelayMs': record['publicDelayMs'],
            'newDelayMs': delay_ms, 'oldEnvironmentSha256': stage.digest(old_env),
            'oldDropinSha256': stage.digest(old_dropin), 'newEnvironmentSha256': stage.digest(new_env),
            'newDropinSha256': stage.digest(new_dropin), 'preparedAtMs': int(time.time() * 1000)}
    stage.write_private(stage.STAGE / 'spectator-feature.delay.env.candidate', new_env)
    stage.write_private(stage.STAGE / 'spectator-feature.delay.conf.candidate', new_dropin)
    stage.write_private(stage.STAGE / 'delay-change-plan.json', json.dumps(plan, indent=2).encode())
    print(json.dumps({'prepared': True, 'applied': False, 'oldDelayMs': plan['oldDelayMs'],
                      'newDelayMs': delay_ms, 'revision': record['revision'],
                      'capacity': observed.get('capacity', {})}))

def apply(delay_ms):
    plan = json.loads((stage.STAGE / 'delay-change-plan.json').read_bytes())
    record, state_bytes, old_env, old_dropin = current_inputs()
    new_env, new_dropin = feature_values(delay_ms)
    if (plan.get('newDelayMs') != delay_ms or plan.get('oldDelayMs') != record['publicDelayMs']
            or plan.get('stateSha256') != stage.digest(state_bytes)
            or plan.get('revision') != record['revision']
            or plan.get('serviceFingerprint') != record['serviceFingerprint']
            or plan.get('oldEnvironmentSha256') != stage.digest(old_env)
            or plan.get('oldDropinSha256') != stage.digest(old_dropin)
            or plan.get('newEnvironmentSha256') != stage.digest(new_env)
            or plan.get('newDropinSha256') != stage.digest(new_dropin)):
        raise RuntimeError('Delay plan changed; prepare again')
    before = stage.health()
    stage.require_drained(before, record['revision'])
    stage.require_legacy_drained()
    if delay_ms == record['publicDelayMs']:
        print(json.dumps({'applied': False, 'unchanged': True, 'publicDelayMs': delay_ms}))
        return
    candidate = (stage.STAGE / 'Caddyfile.candidate').read_bytes()
    if stage.digest(candidate) != record['candidateSha256']:
        raise RuntimeError('Candidate proxy changed')
    barrier = stage.validate_barrier(candidate)
    location = stage.STAGE / 'spectator-feature.env'
    backup = stage.STAGE / f'delay-before-{time.time_ns()}'
    stage.write_private(backup.with_suffix('.env'), old_env)
    stage.write_private(backup.with_suffix('.conf'), old_dropin)
    limits = {key: value for key, value in before.get('capacity', {}).items() if key.startswith('max')}
    # Validation and backups take time; fence again before the first live write.
    stage.require_drained(stage.health(), record['revision'])
    stage.require_legacy_drained()
    require_snapshot(record, state_bytes, candidate, old_env, old_dropin)
    env_written = dropin_written = restarted = proxy_touched = proxy_restored = False
    committed_state = None
    try:
        proxy_touched = True
        stage.replace_caddy(barrier)
        stage.require_drained(stage.health(), record['revision'])
        stage.require_legacy_drained()
        require_snapshot(record, state_bytes, barrier, old_env, old_dropin)
        env_written = True
        stage.write_private(location, new_env)
        dropin_written = True
        stage.write_private(stage.DROPIN, new_dropin)
        subprocess.run(['systemctl', 'daemon-reload'], check=True, capture_output=True)
        stage.require_drained(stage.health(), record['revision'])
        stage.require_legacy_drained()
        require_snapshot(record, state_bytes, barrier, new_env, new_dropin)
        restarted = True
        subprocess.run(['systemctl', 'restart', 'mir2-playtest'], check=True, capture_output=True)
        wait_for_delay(record, delay_ms, limits)
        require_snapshot(record, state_bytes, barrier, new_env, new_dropin)
        # A failed reload belongs to this transaction too. The file alone does
        # not prove that the running proxy has stopped using the admission gate.
        stage.replace_caddy(candidate)
        proxy_restored = True
        require_snapshot(record, state_bytes, candidate, new_env, new_dropin)
        record['publicDelayMs'] = delay_ms
        record['delayChangedAtMs'] = int(time.time() * 1000)
        committed_state = json.dumps(record, indent=2).encode()
        stage.write_private(stage.STAGE / 'status.json', committed_state)
    except Exception as error:
        recovery_error = None
        try:
            if ((env_written and location.read_bytes() not in (old_env, new_env))
                    or (dropin_written and stage.DROPIN.read_bytes() not in (old_dropin, new_dropin))):
                raise RuntimeError('Feature changed by another operator; refuse automatic rollback')
            if env_written or dropin_written:
                stage.require_unchanged_service(record)
            # Recover our desired configuration even if the failed startup has
            # no health endpoint. An actual retry still needs all drain gates.
            if env_written:
                stage.write_private(location, old_env)
            if dropin_written:
                stage.write_private(stage.DROPIN, old_dropin)
            if env_written or dropin_written:
                subprocess.run(['systemctl', 'daemon-reload'], check=True, capture_output=True)
            if restarted:
                stage.require_drained(stage.health(), record['revision'])
                stage.require_legacy_drained()
                current_proxy = stage.CADDY.read_bytes()
                if current_proxy == candidate:
                    # Restore a gate before a second restart. A prior reload may
                    # have failed after replacing the file, or reopened ingress.
                    proxy_restored = False
                    stage.replace_caddy(barrier)
                elif current_proxy != barrier:
                    raise RuntimeError('Proxy changed; refuse rollback during another rollout')
                stage.require_drained(stage.health(), record['revision'])
                stage.require_legacy_drained()
                require_owned_configuration(record, barrier, old_env, old_dropin)
                subprocess.run(['systemctl', 'restart', 'mir2-playtest'], check=True, capture_output=True)
                wait_for_delay(record, plan['oldDelayMs'], limits)
                require_owned_configuration(record, barrier, old_env, old_dropin)
            if committed_state is not None:
                current_state = (stage.STAGE / 'status.json').read_bytes()
                if current_state == committed_state:
                    stage.write_private(stage.STAGE / 'status.json', state_bytes)
                elif current_state != state_bytes:
                    raise RuntimeError('Stage record changed; preserve it for operator reconciliation')
        except Exception as rollback_error:
            recovery_error = rollback_error
        if proxy_touched:
            try:
                current_proxy = stage.CADDY.read_bytes()
                if current_proxy not in (barrier, candidate):
                    raise RuntimeError('Proxy changed; refuse to overwrite another operator')
                if current_proxy == barrier or not proxy_restored:
                    stage.replace_caddy(candidate)
            except Exception as proxy_error:
                recovery_error = proxy_error
        if recovery_error is not None:
            raise RuntimeError(f'Delay change failed; automatic recovery needs attention: {recovery_error}') from error
        raise
    print(json.dumps({'applied': True, 'gatewayRestarted': True, 'revision': record['revision'],
                      'publicDelayMs': delay_ms, 'recordingEnabled': False}))

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--delay-ms', required=True, type=int)
    parser.add_argument('--apply', action='store_true')
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit('Run as root on the player host')
    try:
        with stage.operator_lock():
            if args.apply:
                apply(args.delay_ms)
            else:
                prepare(args.delay_ms)
    except Exception as error:
        raise SystemExit(str(error))
