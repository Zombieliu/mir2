import datetime
import hashlib
import json
from pathlib import Path
import subprocess

PROJECT = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
PHASE = Path('C:/mir2-playtest-releases/20261004-native-r18/cross-map-drop-implementation-01')
OUT = Path(__file__).resolve().parent
ROOT_HEAD = '1b5e64dcf43ae5b8b543cbed4becb6da285a631d'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def git(*args):
    return subprocess.run(['git', '-c', 'gc.auto=0', '-C', str(PROJECT), *args],
                          check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout

def under(base, relative):
    path = (base / relative).resolve()
    assert path.is_relative_to(base.resolve()), relative
    return path

assert git('rev-parse', 'HEAD').decode().strip() == ROOT_HEAD
assert not git('status', '--porcelain').strip(), 'root must be clean'
manifest = json.loads((PHASE / 'source-manifest.json').read_bytes())
receipt = json.loads((PHASE / 'final-receipt.json').read_bytes())
patch = PHASE / receipt['fullPatch']
assert digest(patch.read_bytes()) == receipt['fullPatchSha256']
assert len(manifest['files']) == receipt['files'] == 14
checked = []
for row in manifest['files']:
    frozen = under(PHASE / 'frozen-source', row['file']).read_bytes()
    assert len(frozen) == row['bytes'] and digest(frozen) == row['sha256'], row['file']
    current = under(PROJECT, row['file'])
    before_sha = None
    if row['new']:
        assert not current.exists(), f"new destination already exists: {row['file']}"
    else:
        baseline_blob = git('show', f"{manifest['head']}:mir2-web3/{row['file']}")
        current_blob = git('show', f"HEAD:mir2-web3/{row['file']}")
        assert baseline_blob == current_blob, f"unrelated committed change: {row['file']}"
        actual = current.read_bytes()
        assert actual.replace(b'\r\n', b'\n') == current_blob.replace(b'\r\n', b'\n'), row['file']
        before_sha = digest(actual)
    checked.append({'file': row['file'], 'new': row['new'], 'beforeSha256': before_sha,
                    'frozenSha256': row['sha256'], 'frozenBytes': row['bytes']})

inventory = json.loads((PHASE / 'artifact-inventory.json').read_bytes())
for row in inventory['files']:
    data = under(PHASE, row['path']).read_bytes()
    assert len(data) == row['bytes'] and digest(data) == row['sha256'], row['path']
originals = json.loads((PHASE / 'original-source-recheck.json').read_bytes())
for row in originals['sources']:
    original = Path(row['originalPath']).read_bytes()
    snapshot = Path(row['snapshot']).read_bytes()
    assert original == snapshot and len(original) == row['bytes'] and digest(original) == row['sha256'], row['file']

repository = git('rev-parse', '--show-toplevel').decode().strip()
forward = subprocess.run(['git', '-c', 'gc.auto=0', '-C', repository, 'apply', '--check', str(patch)],
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE)
(OUT / 'apply-check.log').write_bytes(forward.stdout + forward.stderr)
assert forward.returncode == 0, 'patch forward check failed'
result = {'schema': 'mir2.p3a.root-preflight.v1', 'createdUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'rootRevision': ROOT_HEAD, 'workerBaseline': manifest['head'], 'files': checked,
          'frozenArtifactsVerified': len(inventory['files']), 'originalSourcesVerified': len(originals['sources']),
          'patchSha256': receipt['fullPatchSha256'], 'forwardCheckPassed': True,
          'applied': False, 'productReviewPendingSameTickLifeIssue': True,
          'liveServicesChanged': False, 'playerSavesChanged': False}
(OUT / 'ROOT-PREFLIGHT-01.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(json.dumps({'sources': len(checked), 'artifacts': len(inventory['files']), 'originals': len(originals['sources']),
                  'forwardCheckPassed': True, 'applied': False}))
