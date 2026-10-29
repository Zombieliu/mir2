"""Publish reviewed account endpoint, then dispatch only the native read probe."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import time

out = Path(__file__).resolve().parent
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
branch = 'codex/crowded-combat-control-20261003'
git = ['git', '-c', 'gc.auto=0', '-C', str(repo)]
assert not subprocess.check_output(git + ['status', '--porcelain']).strip()
head = subprocess.check_output(git + ['rev-parse', 'HEAD']).decode().strip()
for path, expected in {
    'mir2-web3/scripts/cloudflare_read_authority_probe.py': 'a1294fd9a4c6a7978ab9ad8fd2874f7f393697eb1a8746f5d3d4cb8185ec9faa',
    'mir2-web3/scripts/test_cloudflare_read_authority_probe.py': '258495419be6acda64d1c559dcf511e8e6d4aad7739cdc2f5b39e9b0dd88913d',
}.items():
    assert hashlib.sha256(subprocess.check_output(git + ['show', head + ':' + path])).hexdigest() == expected
network_git = git + ['-c', 'core.sshCommand=ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=3']
cmd = network_git + ['push', 'origin', head + ':refs/heads/' + branch]
clock = time.monotonic()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
print(json.dumps({'starting': 'account probe source publication', 'head': head}), flush=True)
with (out / 'push-01.log').open('xb') as log:
    result = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, timeout=300)
receipt = {'schema': 'mir2.native-ci-account-probe-push.v1', 'head': head, 'command': cmd, 'exitCode': result.returncode,
           'startedUtc': started, 'elapsedSeconds': time.monotonic() - clock, 'forcePush': False, 'credentialsExported': False}
with (out / 'PUSH-01.json').open('x') as f:
    json.dump(receipt, f, indent=2)
    f.write('\n')
assert result.returncode == 0
remote = subprocess.run(network_git + ['ls-remote', 'origin', 'refs/heads/' + branch], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
with (out / 'remote-ref-01.log').open('xb') as log:
    log.write(remote.stdout + remote.stderr)
assert remote.returncode == 0 and remote.stdout.decode().split()[0] == head
receipt['actualRemoteHeadMatches'] = True
with (out / 'REMOTE-CONFIRMATION-01.json').open('x') as f:
    json.dump(receipt, f, indent=2)
    f.write('\n')
assert not subprocess.check_output(git + ['status', '--porcelain']).strip()
cmd = ['gh', 'workflow', 'run', 'web-assets-r2-release.yml', '--repo', 'Zombieliu/mir2', '--ref', branch, '-f', 'native_delivery_operation=probe']
clock = time.monotonic()
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
with (out / 'dispatch-01.log').open('xb') as log:
    result = subprocess.run(cmd, cwd=repo, stdout=log, stderr=subprocess.STDOUT, timeout=90)
receipt = {'schema': 'mir2.native-ci-account-probe-dispatch.v1', 'head': head, 'command': cmd, 'exitCode': result.returncode,
           'startedUtc': started, 'elapsedSeconds': time.monotonic() - clock, 'onlyReadOnlyProbeRequested': True,
           'webFlagsRequested': False, 'productionDeploymentRequested': False, 'credentialsExported': False}
with (out / 'DISPATCH-01.json').open('x') as f:
    json.dump(receipt, f, indent=2)
    f.write('\n')
print(json.dumps(receipt), flush=True)
raise SystemExit(result.returncode)
