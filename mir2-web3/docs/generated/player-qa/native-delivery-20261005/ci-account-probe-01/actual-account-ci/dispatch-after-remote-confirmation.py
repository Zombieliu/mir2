"""Retain ambiguous SSH push failure, verify actual remote, dispatch read-only."""
import hashlib
import json
import subprocess
import time
from pathlib import Path

out = Path(__file__).resolve().parent
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
head = '0250695126be552009d81a8e0b951ba85746a70d'
git = ['git', '-c', 'gc.auto=0', '-C', str(repo)]
assert subprocess.check_output(git + ['rev-parse', 'HEAD']).decode().strip() == head
assert not subprocess.check_output(git + ['status', '--porcelain']).strip()
result = subprocess.run(git + ['-c', 'core.sshCommand=ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=3', 'ls-remote', 'origin', 'refs/heads/codex/crowded-combat-control-20261003'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=90)
with (out / 'remote-ref-02.log').open('xb') as f:
    f.write(result.stdout + result.stderr)
assert result.returncode == 0 and result.stdout.decode().split()[0] == head
receipt = {'head': head, 'actualRemoteHeadMatches': True, 'firstPushExitCode': 128, 'firstPushFailureRetained': True, 'forcePush': False, 'retryPushPerformed': False}
with (out / 'REMOTE-CONFIRMATION-02.json').open('x') as f:
    json.dump(receipt, f, indent=2)
    f.write('\n')
workflow = subprocess.check_output(git + ['show', head + ':.github/workflows/web-assets-r2-release.yml'])
cmd = ['gh', 'workflow', 'run', 'web-assets-r2-release.yml', '--repo', 'Zombieliu/mir2', '--ref', 'codex/crowded-combat-control-20261003', '-f', 'native_delivery_operation=probe']
clock = time.monotonic()
with (out / 'dispatch-01.log').open('xb') as f:
    result = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, timeout=90)
receipt = {'head': head, 'command': cmd, 'exitCode': result.returncode, 'elapsedSeconds': time.monotonic() - clock,
           'selectedWorkflowSha256': hashlib.sha256(workflow).hexdigest(), 'onlyReadOnlyProbeRequested': True, 'oldWebFlagsRequested': False,
           'productionDeploymentRequested': False, 'credentialsExported': False}
with (out / 'DISPATCH-01.json').open('x') as f:
    json.dump(receipt, f, indent=2)
    f.write('\n')
print(json.dumps(receipt), flush=True)
raise SystemExit(result.returncode)
