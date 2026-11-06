"""Dispatch only the reviewed read-only branch of an existing registered workflow."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import time

out = Path(__file__).resolve().parent
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
expected_head = '25ecb38f6ca9273304ba0f663e8533de71c51a74'
confirmation = json.loads((out / 'REMOTE-CONFIRMATION-01.json').read_bytes())
assert confirmation['actualRemoteHeadMatches'] and confirmation['targetHead'] == expected_head
git = ['git', '-c', 'gc.auto=0', '-C', str(repo)]
assert subprocess.check_output(git + ['rev-parse', 'HEAD']).decode().strip() == expected_head
assert not subprocess.check_output(git + ['status', '--porcelain']).strip()
data = subprocess.check_output(git + ['show', expected_head + ':.github/workflows/web-assets-r2-release.yml'])
command = ['gh', 'workflow', 'run', 'web-assets-r2-release.yml', '--repo', 'Zombieliu/mir2', '--ref', 'codex/crowded-combat-control-20261003', '-f', 'native_delivery_operation=probe']
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
clock = time.monotonic()
with (out / 'dispatch-01.log').open('xb') as log:
    result = subprocess.run(command, cwd=repo, stdout=log, stderr=subprocess.STDOUT, timeout=90)
receipt = {'schema': 'mir2.native-ci-auth-probe-dispatch.v1', 'command': command, 'startedUtc': started,
           'elapsedSeconds': time.monotonic() - clock, 'exitCode': result.returncode,
           'head': expected_head, 'selectedCommittedWorkflowSha256': hashlib.sha256(data).hexdigest(),
           'onlyReadOnlyProbeRequested': True, 'webFlagsRequested': False,
           'productionDeploymentRequested': False, 'credentialsExported': False}
with (out / 'DISPATCH-01.json').open('xb') as f:
    f.write((json.dumps(receipt, indent=2) + '\n').encode())
print(json.dumps(receipt, separators=(',', ':')))
raise SystemExit(result.returncode)
