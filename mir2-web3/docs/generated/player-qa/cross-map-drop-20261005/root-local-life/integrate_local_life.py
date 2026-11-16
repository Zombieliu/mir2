import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
REPO = PROJECT.parent
PHASE = Path('C:/mir2-playtest-releases/20261004-native-r18/cross-map-drop-implementation-01')
FOLLOWUP = Path('C:/mir2-playtest-releases/20261004-native-r18/p3-local-life-review-fix-01')
OUT = Path(__file__).resolve().parent
HEAD = '753ad7895e39da779b1f453ee4a09b9f0a3c011f'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def under(root, relative):
    path = (root / relative).resolve()
    path.relative_to(root.resolve())
    return path

def verify(root, row, path_key):
    data = under(root, row[path_key]).read_bytes()
    assert len(data) == row['bytes'], row[path_key]
    assert digest(data) == row['sha256'], row[path_key]

def git(*args):
    return subprocess.run(['git', '-c', 'gc.auto=0', '-C', str(REPO), *args],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)

assert not (OUT / 'ROOT-LOCAL-LIFE-INTEGRATION-01.json').exists(), 'preserve the completed attempt'
assert git('rev-parse', 'HEAD').stdout.decode().strip() == HEAD
prior = json.loads((PHASE / 'source-manifest.json').read_text(encoding='utf-8'))
followup = json.loads((FOLLOWUP / 'source-manifest.json').read_text(encoding='utf-8'))
assert len(prior['files']) == 14 and len(followup['files']) == 2
for row in prior['files']:
    verify(PROJECT, row, 'file')
for row in followup['files']:
    verify(FOLLOWUP / 'final-source', row, 'file')
    if row['new']:
        assert not under(PROJECT, row['file']).exists(), row['file']
artifact_counts = {}
for phase in (PHASE, FOLLOWUP):
    inventory = json.loads((phase / 'artifact-inventory.json').read_text(encoding='utf-8'))
    for row in inventory['files']:
        verify(phase, row, 'path')
    artifact_counts[phase.name] = len(inventory['files'])

patch = FOLLOWUP / 'p3-local-life-follow-up.patch'
assert digest(patch.read_bytes()) == '0639884c3952a5df5e51efa68d679a883893fb16bff1e4c089a935035799a888'
forward = git('apply', '--check', str(patch))
(OUT / 'forward-check.log').write_bytes(forward.stdout + forward.stderr)
assert forward.returncode == 0, forward.stderr.decode(errors='replace')

for row in followup['files']:
    target = under(PROJECT, row['file'])
    if target.exists():
        before = under(OUT / 'before-source', row['file'])
        before.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(target, before)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(under(FOLLOWUP / 'final-source', row['file']), target)
    verify(PROJECT, row, 'file')

changed = {row['file'] for row in followup['files']}
unchanged = [row for row in prior['files'] if row['file'] not in changed]
assert len(unchanged) == 13
for row in unchanged:
    verify(PROJECT, row, 'file')
reverse = git('apply', '--reverse', '--check', str(patch))
(OUT / 'reverse-check.log').write_bytes(reverse.stdout + reverse.stderr)
assert reverse.returncode == 0, reverse.stderr.decode(errors='replace')
diff_check = git('diff', '--check')
(OUT / 'diff-check.log').write_bytes(diff_check.stdout + diff_check.stderr)
assert diff_check.returncode == 0
receipt = {
    'schema': 'mir2.p3a.root-local-life-integration.v1',
    'createdUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'rootRevision': HEAD,
    'files': followup['files'],
    'prior13SourcesUnchanged': True,
    'artifactCountsVerified': artifact_counts,
    'forwardAndReverseChecksPassed': True,
    'allImportedBytesExact': True,
    'productTestsRun': False,
    'networkOrNativeAcceptance': False,
    'crossZoneSameBatchLife': 'OPEN',
    'objectIdAdmissionCollision': 'OPEN; read-only finding',
    'servicesOrPlayerSavesChanged': False,
    'committedOrPublished': False,
}
(OUT / 'ROOT-LOCAL-LIFE-INTEGRATION-01.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
print(json.dumps(receipt), flush=True)
