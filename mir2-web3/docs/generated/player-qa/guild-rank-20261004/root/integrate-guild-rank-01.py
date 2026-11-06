import hashlib
import json
import subprocess
from pathlib import Path

root = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
phase = Path(__file__).parent / 'guild-rank-implementation-01'
receipt = json.loads((phase / 'freeze-receipt.json').read_bytes())
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def git(*args):
    return subprocess.check_output(['git', '-c', 'gc.auto=0', *args], cwd=root).decode('utf-8').strip()

assert git('rev-parse', 'HEAD') == receipt['base_head'] == '321316b791120a6fe549e4006623e63333a5543f'
assert not git('status', '--porcelain=v1', '--untracked-files=all')
assert sha(phase / 'final.patch') == receipt['patch_sha256'] == '48fbcac187c23836634b2b0faf7bcc388a7bdee833453b9c6be56313722c711f'
assert len(receipt['source_files']) == 6
for name, expected in receipt['source_files'].items():
    frozen = phase / 'final-source' / name
    assert sha(frozen) == expected['sha256'] and frozen.stat().st_size == expected['bytes'], name
    if name in receipt['baseline_git_blobs']:
        assert sha(root / name) == receipt['baseline_git_blobs'][name]['git_blob_sha256'], name
    else:
        assert not (root / name).exists(), name
git('apply', '--check', str(phase / 'final.patch'))
git('apply', str(phase / 'final.patch'))
for name, expected in receipt['source_files'].items():
    assert sha(root / name) == expected['sha256'], name
git('diff', '--check')
report = {'sourceBase': receipt['base_head'], 'patchSha256': receipt['patch_sha256'],
          'allSixSourceBytesExact': True, 'runtimeTestsExecuted': False,
          'r18FrozenReleaseUnchanged': True, 'sourceFiles': receipt['source_files'],
          'rootStatus': git('status', '--porcelain=v1', '--untracked-files=all')}
with (Path(__file__).parent / 'guild-rank-root-integration-01.json').open('x', encoding='utf-8') as out:
    json.dump(report, out, indent=2)
print(json.dumps({k:v for k,v in report.items() if k not in ('sourceFiles','rootStatus')}))
