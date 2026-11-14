import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
REPO = PROJECT.parent
PHASE = Path('C:/mir2-playtest-releases/20261004-native-r18/guild-rank-native-implementation-01')
OUT = Path(__file__).resolve().parent
BASE = '513eb83db8b919899d3044258e4b1e9a215cd9de'
ROUTING = 'apps/gateway/src/routing.rs'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def under(root, relative):
    target = (root / relative).resolve()
    target.relative_to(root.resolve())
    assert not target.is_symlink()
    return target

def git(*args):
    return subprocess.run(['git', '-c', 'gc.auto=0', '-C', str(REPO), *args],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)

assert not (OUT / 'ROOT-INTEGRATION-01.json').exists(), 'preserve finished attempt'
assert git('rev-parse', 'HEAD').stdout.decode().strip() == BASE
assert not git('status', '--porcelain').stdout.strip(), 'preserve unrelated work'
frozen_data = (PHASE / 'FROZEN-DELIVERY.json').read_bytes()
assert digest(frozen_data) == 'b43c6f12369fb9912541b8aeea83f0055defa7587c86feb829e4cf1a323e5a2f'
frozen = json.loads(frozen_data)
assert len(frozen['files']) == 11
for row in frozen['files']:
    data = under(PHASE / 'final-source/mir2-web3', row['path']).read_bytes()
    assert len(data) == row['bytes'] and digest(data) == row['sha256'], row['path']
    current = under(PROJECT, row['path'])
    if row['kind'] == 'new':
        assert not current.exists(), row['path']
    elif row['path'] != ROUTING:
        assert digest(current.read_bytes()) == row['baseline_sha256'], row['path']
assert digest((PROJECT / ROUTING).read_bytes()) == 'bb838c99429da46e9bd4e051bd86daaeb0e88bb385cc67fe0c9875c9e631d931'
for relative, expected in frozen['protected_source_and_lock_sha256'].items():
    assert digest(under(PROJECT, relative).read_bytes()) == expected, relative
for run in frozen['scoped_runs']:
    for name, expected in ((run['name'] + '.log', run['log_sha256']),
                           (run['name'] + '-input.json', run['input_sha256']),
                           (run['name'] + '-receipt.json', run['receipt_sha256'])):
        assert digest((PHASE / name).read_bytes()) == expected, name
    text = (PHASE / (run['name'] + '.log')).read_text(encoding='utf-8', errors='replace')
    assert all(result in text for result in run['test_results']), run['name']

whole = PHASE / 'guild-rank-native-scoped.patch'
routing = PHASE / 'routing-guild-rank-terminal.patch'
assert digest(whole.read_bytes()) == frozen['patch_sha256']
assert digest(routing.read_bytes()) == frozen['routing_patch_sha256']
assert routing.read_bytes().count(b'\ndiff --git ') == 0
assert routing.read_bytes().count(b'\n@@ ') == 2
for label, patch in (('whole-forward-check', whole), ('routing-forward-check', routing)):
    check = git('apply', '--check', str(patch))
    (OUT / (label + '.log')).write_bytes(check.stdout + check.stderr)
    assert check.returncode == 0, check.stderr.decode(errors='replace')

before = OUT / 'before-source' / ROUTING
before.parent.mkdir(parents=True, exist_ok=True)
shutil.copyfile(PROJECT / ROUTING, before)
applied = git('apply', str(routing))
(OUT / 'routing-apply.log').write_bytes(applied.stdout + applied.stderr)
assert applied.returncode == 0, applied.stderr.decode(errors='replace')
for row in frozen['files']:
    if row['path'] == ROUTING:
        continue
    target = under(PROJECT, row['path'])
    if target.exists():
        old = under(OUT / 'before-source', row['path'])
        old.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(target, old)
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(under(PHASE / 'final-source/mir2-web3', row['path']), target)
    assert digest(target.read_bytes()) == row['sha256'], row['path']
for relative, expected in frozen['protected_source_and_lock_sha256'].items():
    assert digest(under(PROJECT, relative).read_bytes()) == expected, relative
reverse = git('apply', '--reverse', '--check', str(whole))
(OUT / 'whole-reverse-check.log').write_bytes(reverse.stdout + reverse.stderr)
assert reverse.returncode == 0, reverse.stderr.decode(errors='replace')
diff_check = git('diff', '--check')
(OUT / 'diff-check.log').write_bytes(diff_check.stdout + diff_check.stderr)
assert diff_check.returncode == 0
receipt = {
    'schema': 'mir2.p5a.native-rank.root-integration.v1',
    'createdUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'rootBase': BASE, 'workerBase': frozen['base'],
    'frozenReceiptSha256': digest(frozen_data),
    'importedExactFiles': [row for row in frozen['files'] if row['path'] != ROUTING],
    'routingImportedByOnlyTwoForwardCheckedHunks': True,
    'routingBeforeSha256': digest(before.read_bytes()),
    'combinedRoutingSha256': digest((PROJECT / ROUTING).read_bytes()),
    'wholeForwardReverseAndDiffCheckPassed': True,
    'protectedSourcesAndLocksUnchanged': True,
    'all12RawRunsHashesVerified': True,
    'rootProductTestsRun': False,
    'nativeOrNetworkOrHumanAcceptance': False,
    'servicesOrPlayerSavesChanged': False,
    'committedOrPublished': False,
}
(OUT / 'ROOT-INTEGRATION-01.json').write_bytes((json.dumps(receipt, indent=2) + '\n').encode())
print(json.dumps(receipt), flush=True)
