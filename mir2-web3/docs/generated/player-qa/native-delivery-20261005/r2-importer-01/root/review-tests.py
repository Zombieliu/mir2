"""Independent root checks of the integrated fixed-closure importer."""
import hashlib
import json
import re
import subprocess
import time
from pathlib import Path

out = Path(__file__).resolve().parent
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
worker = out.parent / 'r17-native-r2-importer-worker-01'
expected = json.loads((out / 'INTEGRATION-01.json').read_bytes())
for row in expected['files']:
    assert hashlib.sha256((repo / row['repositoryPath']).read_bytes()).hexdigest() == row['sha256']
command = ['node', '--experimental-strip-types', '--test', '--test-reporter=tap', 'test/native-r17-publication.test.mjs']
clock = time.monotonic()
with (out / 'tests-01.stdout.log').open('xb') as stdout, (out / 'tests-01.stderr.log').open('xb') as stderr:
    result = subprocess.run(command, cwd=repo / 'mir2-web3/infra/cloudflare/mir2-r2-bulk-upload', stdout=stdout, stderr=stderr, timeout=120)
raw = (out / 'tests-01.stdout.log').read_bytes()
values = {name: int(re.search(rb'^# ' + name.encode() + rb' (\d+)$', raw, re.M)[1]) for name in ['tests', 'pass', 'fail', 'cancelled', 'skipped']}
passed = result.returncode == 0 and values == {'tests':107, 'pass':107, 'fail':0, 'cancelled':0, 'skipped':0}
receipt = {'schema':'mir2.native-r17-importer-root-offline-review.v1', 'passed':passed, 'command':command,
           'elapsedSeconds':time.monotonic()-clock, 'exitCode':result.returncode, **values,
           'integratedSources':expected['files'], 'stdoutSha256':hashlib.sha256(raw).hexdigest(),
           'stderrSha256':hashlib.sha256((out/'tests-01.stderr.log').read_bytes()).hexdigest(),
           'rootTestsOverlapWorker107':True, 'allInputsFake':True,
           'realCloudflareAuthorityOrR2OrPublicAcceptance':False, 'deployment':False,
           'rootReview':'Fixed native origin/closure, no arbitrary keys/URLs; native-only create/checksum/CAS, no unconditional overwrite; bounded JSON and streamed artifacts; actual account authorization denied and publication remains pending.'}
with (out / 'ROOT-REVIEW-01.json').open('x') as f:
    json.dump(receipt, f, indent=2)
    f.write('\n')
print(json.dumps(receipt), flush=True)
assert passed
