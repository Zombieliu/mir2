"""Retain byte-bound fake tests and the safe actual first CI denial."""
from pathlib import Path
import hashlib
import json
import subprocess

out = Path(__file__).resolve().parent
worker = out.parent / 'native-ci-account-auth-probe-worker-01'
old = out.parent / 'native-ci-auth-probe-root-01'
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
freeze_data = (worker / 'FROZEN-RECEIPT.json').read_bytes()
assert hashlib.sha256(freeze_data).hexdigest() == '50e062ae6a94b3e78fa29d1a409f2ab8b1bc51378240811935e027afb0e09a14'
freeze = json.loads(freeze_data)
meta_path = out / 'USER-ENDPOINT-CI-JOBS.json'
with meta_path.open('xb') as log:
    result = subprocess.run(['gh', 'run', 'view', '37240009948', '--repo', 'Zombieliu/mir2', '--json', 'headSha,conclusion,jobs,url'], stdout=log, stderr=subprocess.PIPE, timeout=90)
assert result.returncode == 0
meta = json.loads(meta_path.read_bytes())
assert meta['headSha'] == '25ecb38f6ca9273304ba0f663e8533de71c51a74'
assert meta['conclusion'] == 'failure'
assert all(j['conclusion'] == 'skipped' for j in meta['jobs'] if j['name'] != 'native-download-authority-probe')
target = repo / 'mir2-web3/docs/generated/player-qa/native-delivery-20261005/ci-account-probe-01'
target.mkdir()
inventory = []
topology = []
def retain(source, relative):
    relative = Path(relative)
    assert not relative.is_absolute() and '..' not in relative.parts
    dest = target / relative
    dest.parent.mkdir(parents=True, exist_ok=True)
    data = source.read_bytes()
    with dest.open('xb') as f:
        f.write(data)
    assert dest.read_bytes() == data
    inventory.append({'path': relative.as_posix(), 'size': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
for row in freeze['evidenceFiles']:
    if row.get('type') != 'regular' or 'sha256' not in row:
        topology.append(row)
        continue
    source = worker / row['path']
    assert not source.is_symlink()
    assert hashlib.sha256(source.read_bytes()).hexdigest() == row['sha256']
    retain(source, Path('worker') / row['path'])
retain(worker / 'FROZEN-RECEIPT.json', 'worker/FROZEN-RECEIPT.json')
for source in sorted(out.iterdir()):
    if source.is_file():
        retain(source, Path('root') / source.name)
for name in ['DISPATCH-01.json', 'dispatch-01.log', 'dispatch-probe.py', 'push-01.json', 'push-01.log', 'REMOTE-CONFIRMATION-01.json', 'remote-ref-01.log']:
    retain(old / name, Path('previous-user-endpoint') / name)
retain(old / 'ci-run-37240009948/receipt.json', 'previous-user-endpoint/receipt.json')
with (target / 'BYTE-INVENTORY.json').open('x', encoding='utf-8', newline='\n') as f:
    json.dump({'schema': 'mir2.native-account-ci-probe-proof.v1', 'files': inventory, 'fixtureLinksMetadataOnly': topology}, f, indent=2)
    f.write('\n')
with (target / '.gitattributes').open('x', encoding='utf-8', newline='\n') as f:
    f.write('* -text -diff\n**/* -text -diff\nREADME.md text eol=lf diff\n')
with (target / 'README.md').open('x', encoding='utf-8', newline='\n') as f:
    f.write('# Fixed account-token authority probe\n\n')
    f.write('The actual first CI run37240009948 used the user-token verification endpoint and returned401 before any bucket call. Its safe receipt and actual job metadata are retained; both old Web jobs were skipped. This denial alone does not establish token expiration, and no write/deploy/route permission was tested.\n\n')
    f.write('The account-token variant changes only the docstring and fixed official account verification path. Worker32-case RED retains31 old passes and one new literal-path failure; GREEN32/32 and independent root32/32 pass with overlap. All fixture inputs are fake. Link topology stays inert JSON metadata. Production transport remains verified TLS, no redirects/proxies, at most two fixed GETs, normalized non-secret receipt and fail-closed write/close handling. Actual account CI is a subsequent gate; offline tests prove no live authority.\n')
print(json.dumps({'retainedFiles': len(inventory), 'fakeLinkMetadata': len(topology), 'target': str(target), 'previousActualVerifyStatus': 401, 'accountProbeDispatched': False}))
