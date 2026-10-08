#!/usr/bin/env python3
"""Stage a bounded spectator feed without replacing the player Gateway binary.

Run as root on the existing host. Default is preparation only; --apply-proxy
hot-reloads read-only routes. --activate requires an explicit operator action
after players have saved and exited, and refuses every non-zero ingress counter.
No credentials or complete proxy configuration are written to stdout.
"""
import argparse
from contextlib import contextmanager
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import time
import urllib.request

STAGE = Path('/var/lib/mir2-playtest-spectator-stage')
CADDY = Path('/etc/caddy/Caddyfile')
DROPIN = Path('/etc/systemd/system/mir2-playtest.service.d/90-spectator-watch.conf')
MARKER = '# Managed by mir2 playtest spectator stage v1\n'
OLD_ROUTE = '@mir2_playtest_public path /ws /health'
NEW_ROUTE = '@mir2_playtest_public path /ws /health /spectator/matches /spectator/ws /ai-live/status'
FEATURE = MARKER + '''[Service]
Environment="MIR2_SPECTATOR_ENABLED=1"
Environment="MIR2_SPECTATOR_RECORDING_ENABLED=0"
Environment="MIR2_SPECTATOR_PUBLIC=1"
Environment="MIR2_SPECTATOR_PUBLIC_MAPS=0"
Environment="MIR2_SPECTATOR_CAPTURE_INTERVAL_MS=1000"
Environment="MIR2_SPECTATOR_PUBLIC_DELAY_MS=30000"
Environment="MIR2_SPECTATOR_MAX_DELAY_MS=60000"
Environment="MIR2_SPECTATOR_RING_FRAMES=90"
Environment="MIR2_SPECTATOR_MAX_ENTITIES=256"
Environment="MIR2_SPECTATOR_ENTITY_STALE_MS=5000"
'''
FEATURE_ENV = MARKER + ''.join(line[len('Environment="'):-1] + '\n'
    for line in FEATURE.splitlines() if line.startswith('Environment="'))
COUNTERS = ('currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases',
            'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight')

def feature_dropin():
    # EnvironmentFile values override Environment, regardless of drop-in order.
    # Append our bounded file after the existing realm file to take precedence.
    return FEATURE + f'EnvironmentFile={STAGE / "spectator-feature.env"}\n'

def prepare_feature_environment():
    location = STAGE / 'spectator-feature.env'
    content = FEATURE_ENV.encode()
    if location.exists():
        if location.read_bytes() != content:
            raise RuntimeError('Spectator feature environment changed; do not overwrite it')
    else: write_private(location, content)

def require_owned_feature_environment():
    location = STAGE / 'spectator-feature.env'
    if not location.exists() or location.read_bytes() != FEATURE_ENV.encode():
        raise RuntimeError('Spectator feature environment changed; refuse restart with modified values')

def require_drained(observed, revision):
    if not observed.get('ok') or observed.get('revision') != revision:
        raise RuntimeError('Gateway health or revision changed; re-stage against the current release')
    capacity = observed.get('capacity', {})
    if any(capacity.get(key) != 0 for key in COUNTERS):
        raise RuntimeError('Players or ingress still present; save and exit before activation')

def digest(content):
    return hashlib.sha256(content).hexdigest()

def write_private(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=path.name + '.', dir=path.parent)
    temporary = Path(name)
    try:
        os.chmod(temporary, 0o600)
        with os.fdopen(descriptor, 'wb') as output:
            descriptor = None
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if descriptor is not None: os.close(descriptor)
        temporary.unlink(missing_ok=True)

@contextmanager
def operator_lock():
    # Local lock serializes this tool's stages. Other rollout tools are fenced
    # separately by their file/config/binary fingerprints.
    import fcntl
    STAGE.mkdir(parents=True, exist_ok=True)
    os.chmod(STAGE, 0o700)
    with open(STAGE / 'operator.lock', 'a+b') as lock:
        os.chmod(lock.name, 0o600)
        try: fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError('Another spectator operator action is already running')
        try: yield
        finally: fcntl.flock(lock, fcntl.LOCK_UN)

def health():
    with urllib.request.urlopen('http://127.0.0.1:7210/health', timeout=8) as response:
        return json.load(response)

def parse_service_properties(raw):
    properties = {}
    for line in raw.decode().splitlines():
        if '=' not in line: continue
        key, value = line.split('=', 1)
        # systemctl emits some array entries as repeated property lines.
        if key in {'EnvironmentFiles', 'DropInPaths'} and key in properties:
            properties[key] += ' ' + value
        else: properties[key] = value
    return properties

def require_legacy_drained():
    with urllib.request.urlopen('http://127.0.0.1:7110/health', timeout=8) as response:
        observed = json.load(response)
    if not observed.get('ok') or any(observed.get('capacity', {}).get(key) != 0 for key in COUNTERS):
        raise RuntimeError('Original Gateway has live ingress; do not reload its shared proxy')

def service_fingerprint():
    """Pin the desired binary, unit/drop-ins and private realm environment.

    Ignore only our own feature override, so the fingerprint stays constant
    across its activation. Secret bytes participate only in a local hash.
    """
    raw = subprocess.check_output(['systemctl', 'show', 'mir2-playtest',
        '-p', 'FragmentPath', '-p', 'DropInPaths', '-p', 'EnvironmentFiles',
        '-p', 'User', '-p', 'WorkingDirectory', '-p', 'ExecStart'])
    properties = parse_service_properties(raw)
    own_environment = re.compile(r'(?<!\S)' + re.escape(str(STAGE / 'spectator-feature.env'))
        + r'(?: \(ignore_errors=(?:yes|no)\))?(?=\s|$)')
    if own_environment.search(properties.get('EnvironmentFiles', '')):
        properties['EnvironmentFiles'] = own_environment.sub('', properties['EnvironmentFiles']).strip()
    command = properties.get('ExecStart', '')
    stable_command = command.split('; ignore_errors=', 1)[0]
    if stable_command == command:
        raise RuntimeError('Cannot pin the effective Gateway start command')
    binaries = set(re.findall(r'/opt/mir2-playtest/releases/[^\s;]+/mir2-gateway', stable_command))
    if len(binaries) != 1: raise RuntimeError('Unexpected Gateway binary layout')
    unit_paths = [properties.get('FragmentPath', ''), *properties.get('DropInPaths', '').split()]
    # Include desired drop-ins not loaded by systemd yet. A concurrent rollout
    # can write a new override before daemon-reload and still expose the old PID.
    for folder in [Path('/etc/systemd/system/mir2-playtest.service.d'),
                   Path('/run/systemd/system/mir2-playtest.service.d'),
                   Path('/usr/lib/systemd/system/mir2-playtest.service.d')]:
        unit_paths.extend(str(path) for path in folder.glob('*.conf'))
    env_paths = re.findall(r'(/[^\s()]+)', properties.get('EnvironmentFiles', ''))
    if '/etc/mir2-playtest/gateway.env' not in env_paths:
        raise RuntimeError('Unexpected realm environment layout')
    hasher = hashlib.sha256()
    for key in ['User', 'WorkingDirectory', 'EnvironmentFiles']:
        hasher.update(f'{key}={properties.get(key, "")}\n'.encode())
    hasher.update(stable_command.encode())
    for item in sorted(set([*unit_paths, *env_paths, *binaries])):
        if not item or item == str(DROPIN): continue
        hasher.update(item.encode() + b'\0')
        with open(item, 'rb') as source:
            for block in iter(lambda: source.read(1024 * 1024), b''):
                hasher.update(block)
    return hasher.hexdigest()

def require_unchanged_service(record):
    if not record.get('serviceFingerprint') or service_fingerprint() != record['serviceFingerprint']:
        raise RuntimeError('Gateway unit, binary or realm environment changed; do not restart another rollout')

def caddy_env():
    pid = subprocess.check_output(['systemctl', 'show', 'caddy', '-p', 'MainPID', '--value'], text=True).strip()
    if not pid.isdigit() or pid == '0': raise RuntimeError('Caddy must be running')
    env = os.environ.copy()
    for item in Path(f'/proc/{pid}/environ').read_bytes().split(b'\0'):
        if b'=' in item:
            key, value = item.split(b'=', 1)
            env[key.decode()] = value.decode()
    return env

def caddy_command(args):
    result = subprocess.run(['/usr/bin/caddy', *args], env=caddy_env(), capture_output=True)
    # The diagnostics can contain imported private configuration; retain locally.
    write_private(STAGE / 'caddy-last-private.log', result.stdout + result.stderr)
    if result.returncode: raise RuntimeError('Caddy command failed; private diagnostics retained on host')
    return result.stdout

def admission_barrier(candidate):
    text = candidate.decode()
    anchor = 'uri strip_prefix /playtest'
    if text.count(anchor) != 1: raise RuntimeError('Playtest admission anchor changed')
    legacy_anchor = '@mir2_playtest path /playtest /playtest/*'
    if text.count(legacy_anchor) != 1: raise RuntimeError('Shared realm admission anchor changed')
    text = text.replace(legacy_anchor,
        '@mir2_watch_legacy_maintenance path /ws /spectator/ws\n'
        '    handle @mir2_watch_legacy_maintenance {\n'
        '        respond "Brief spectator activation (legacy realm)" 503\n'
        '    }\n'
        '    ' + legacy_anchor)
    return text.replace(anchor, anchor + '\n            @mir2_watch_maintenance path /ws /spectator/ws\n'
                        '            respond @mir2_watch_maintenance "Brief spectator activation" 503').encode()

def require_barrier_order(adapted):
    """Prove the adapted route strips /playtest before its 503 admission gate."""
    found = []
    def visit(value):
        if isinstance(value, dict):
            routes = value.get('routes', [])
            stripped = False
            if isinstance(routes, list):
                for route in routes:
                    if not isinstance(route, dict): continue
                    handles = route.get('handle', [])
                    if any(item.get('handler') == 'rewrite' and item.get('strip_path_prefix') == '/playtest'
                           for item in handles if isinstance(item, dict)):
                        stripped = True
                    if any(item.get('handler') == 'static_response' and str(item.get('status_code')) == '503'
                           and item.get('body') == 'Brief spectator activation'
                           for item in handles if isinstance(item, dict)):
                        matches = route.get('match', [])
                        found.append(stripped and any(set(item.get('path', [])) == {'/ws', '/spectator/ws'}
                                                       for item in matches if isinstance(item, dict)))
            for item in value.values(): visit(item)
        elif isinstance(value, list):
            for item in value: visit(item)
    visit(adapted)
    if found != [True]: raise RuntimeError('Adapted proxy does not prove the admission barrier order')

def require_legacy_barrier_order(adapted):
    found = []
    def contains_gate(value):
        if isinstance(value, dict):
            if value.get('handler') == 'static_response' and str(value.get('status_code')) == '503' \
                    and value.get('body') == 'Brief spectator activation (legacy realm)':
                return True
            return any(contains_gate(item) for item in value.values())
        return isinstance(value, list) and any(contains_gate(item) for item in value)
    def contains_proxy(value):
        if isinstance(value, dict):
            if value.get('handler') == 'reverse_proxy' and any(
                upstream.get('dial') in {'127.0.0.1:7110', 'localhost:7110'}
                for upstream in value.get('upstreams', [])):
                return True
            return any(contains_proxy(item) for item in value.values())
        return isinstance(value, list) and any(contains_proxy(item) for item in value)
    def visit(value):
        if isinstance(value, dict):
            routes = value.get('routes', [])
            if isinstance(routes, list):
                for index, route in enumerate(routes):
                    if not isinstance(route, dict): continue
                    correct_paths = any(set(item.get('path', [])) == {'/ws', '/spectator/ws'}
                        for item in route.get('match', []) if isinstance(item, dict))
                    if correct_paths and contains_gate(route.get('handle', [])):
                        found.append(not contains_proxy(routes[:index]) and contains_proxy(routes[index + 1:]))
            for item in value.values(): visit(item)
        elif isinstance(value, list):
            for item in value: visit(item)
    visit(adapted)
    if found != [True]: raise RuntimeError('Adapted proxy does not prove the original realm admission barrier order')

def validate_barrier(candidate):
    barrier = admission_barrier(candidate)
    location = STAGE / 'Caddyfile.admission-barrier'
    write_private(location, barrier)
    caddy_command(['validate', '--config', str(location), '--adapter', 'caddyfile'])
    adapted = caddy_command(['adapt', '--config', str(location), '--adapter', 'caddyfile'])
    configuration = json.loads(adapted)
    require_barrier_order(configuration)
    require_legacy_barrier_order(configuration)
    return barrier

def replace_caddy(content):
    stat = CADDY.stat()
    temporary = CADDY.with_name('Caddyfile.spectator-watch.tmp')
    write_private(temporary, content)
    os.chown(temporary, stat.st_uid, stat.st_gid)
    os.chmod(temporary, stat.st_mode & 0o777)
    os.replace(temporary, CADDY)
    caddy_command(['reload', '--config', str(CADDY), '--adapter', 'caddyfile'])

def prepare():
    STAGE.mkdir(parents=True, exist_ok=True)
    os.chmod(STAGE, 0o700)
    original = CADDY.read_bytes()
    text = original.decode()
    original_route = re.compile(r'(?m)^([ \t]*)' + re.escape(OLD_ROUTE) + r'[ \t]*$')
    if len(original_route.findall(text)) != 1:
        raise RuntimeError('Expected one original playtest route; do not overwrite another rollout')
    candidate = original_route.sub(lambda match: match.group(1) + NEW_ROUTE, text).encode()
    write_private(STAGE / 'Caddyfile.before', original)
    write_private(STAGE / 'Caddyfile.candidate', candidate)
    prepare_feature_environment()
    write_private(STAGE / '90-spectator-watch.conf', feature_dropin().encode())
    caddy_command(['validate', '--config', str(STAGE / 'Caddyfile.candidate'), '--adapter', 'caddyfile'])
    validate_barrier(candidate)
    observed = health()
    if not observed.get('ok') or not observed.get('revision'):
        raise RuntimeError('Player Gateway health or revision is unavailable')
    record = {'revision': observed['revision'], 'serviceFingerprint': service_fingerprint(), 'beforeSha256': digest(original),
              'candidateSha256': digest(candidate), 'proxyApplied': False, 'gatewayActivated': False,
              'preparedAtMs': int(time.time() * 1000), 'admissionBarrierOrderVerified': True, 'recordingEnabled': False,
              'publicMaps': ['0'], 'publicDelayMs': 30_000, 'captureIntervalMs': 1_000}
    write_private(STAGE / 'status.json', json.dumps(record, indent=2).encode())
    print(json.dumps(record))

def apply_proxy():
    record = json.loads((STAGE / 'status.json').read_text())
    # Reloading Caddy can close existing WebSockets, even for other routes.
    # Check both player realms before any live proxy mutation.
    require_drained(health(), record['revision'])
    require_unchanged_service(record)
    require_legacy_drained()
    if digest(CADDY.read_bytes()) != record['beforeSha256']:
        raise RuntimeError('Proxy changed since staging; prepare again before applying')
    candidate = (STAGE / 'Caddyfile.candidate').read_bytes()
    if digest(candidate) != record['candidateSha256']:
        raise RuntimeError('Staged proxy digest does not match')
    try: replace_caddy(candidate)
    except Exception:
        if digest(CADDY.read_bytes()) != record['candidateSha256']:
            raise RuntimeError('Proxy changed after apply failed; do not overwrite another operator')
        before = (STAGE / 'Caddyfile.before').read_bytes()
        if digest(before) != record['beforeSha256']:
            raise RuntimeError('Staged proxy backup changed; refuse automatic restoration')
        replace_caddy(before)
        raise
    record['proxyApplied'] = True
    write_private(STAGE / 'status.json', json.dumps(record, indent=2).encode())
    print(json.dumps({'proxyApplied': True, 'gatewayRestarted': False, 'revision': health().get('revision')}))

def activate():
    record = json.loads((STAGE / 'status.json').read_text())
    if not record.get('proxyApplied') or digest(CADDY.read_bytes()) != record['candidateSha256']:
        raise RuntimeError('Verified proxy stage is missing or changed')
    if DROPIN.exists() and not DROPIN.read_text().startswith(MARKER):
        raise RuntimeError('Feature drop-in belongs to another operator')
    previous = DROPIN.read_bytes() if DROPIN.exists() else None
    require_drained(health(), record['revision'])
    require_legacy_drained()
    require_unchanged_service(record)
    # Close the admission race: block new player/spectator handshakes while the
    # existing connections remain subject to the explicit drain gate above.
    candidate = (STAGE / 'Caddyfile.candidate').read_bytes()
    if digest(candidate) != record['candidateSha256']:
        raise RuntimeError('Staged proxy digest does not match')
    barrier = validate_barrier(candidate)
    restart_attempted = False
    dropin_written = False
    verified_after = None
    try:
        replace_caddy(barrier)
        require_drained(health(), record['revision'])
        require_legacy_drained()
        require_unchanged_service(record)
        current_dropin = DROPIN.read_bytes() if DROPIN.exists() else None
        if current_dropin != previous:
            raise RuntimeError('Feature drop-in changed since staging; do not overwrite it')
        prepare_feature_environment()
        write_private(DROPIN, feature_dropin().encode())
        dropin_written = True
        subprocess.run(['systemctl', 'daemon-reload'], check=True, capture_output=True)
        require_unchanged_service(record)
        require_owned_feature_environment()
        restart_attempted = True
        subprocess.run(['systemctl', 'restart', 'mir2-playtest'], check=True, capture_output=True)
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            try:
                after = health()
                if after.get('ok') and after.get('revision') == record['revision']:
                    require_owned_feature_environment()
                    if after.get('spectator', {}).get('recordingEnabled') is not False:
                        raise RuntimeError('Recording must remain disabled')
                    with urllib.request.urlopen('http://127.0.0.1:7210/spectator/matches', timeout=8) as response:
                        directory = json.load(response)
                    if directory.get('publicDelayMs') != 30_000:
                        raise RuntimeError('Unexpected public spectator delay')
                    verified_after = after
                    break
            except urllib.error.HTTPError:
                raise RuntimeError('Spectator directory rejected the feature; verify effective process environment')
            except (OSError, urllib.error.URLError): pass
            time.sleep(1)
        if verified_after is None: raise RuntimeError('Gateway did not pass activation health check')
    except Exception:
        # Restore only this owned feature override; never switch the binary/DB.
        if dropin_written:
            if not DROPIN.exists() or DROPIN.read_bytes() != feature_dropin().encode():
                raise RuntimeError('Feature override changed during activation; do not overwrite another operator')
            if previous is None: DROPIN.unlink(missing_ok=True)
            else: write_private(DROPIN, previous)
            subprocess.run(['systemctl', 'daemon-reload'], check=True, capture_output=True)
        if restart_attempted:
            require_unchanged_service(record)
            subprocess.run(['systemctl', 'restart', 'mir2-playtest'], check=True, capture_output=True)
        raise
    finally:
        current_digest = digest(CADDY.read_bytes())
        if current_digest == digest(candidate): pass
        elif current_digest == digest(barrier): replace_caddy(candidate)
        else:
            raise RuntimeError('Proxy changed during activation; do not overwrite the other operator')
    record['gatewayActivated'] = True
    write_private(STAGE / 'status.json', json.dumps(record, indent=2).encode())
    print(json.dumps({'gatewayActivated': True, 'revision': verified_after['revision'],
                      'recordingEnabled': False, 'matches': len(directory.get('matches', []))}))

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    action = parser.add_mutually_exclusive_group()
    action.add_argument('--apply-proxy', action='store_true')
    action.add_argument('--activate', action='store_true')
    args = parser.parse_args()
    if os.geteuid() != 0: raise SystemExit('Run as root on the player host')
    try:
        with operator_lock():
            if args.apply_proxy: apply_proxy()
            elif args.activate: activate()
            else: prepare()
    except Exception as error:
        raise SystemExit(str(error))
