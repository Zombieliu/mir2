import hashlib
import json
from pathlib import Path

base = Path('C:/mir2-playtest-releases/20261004-native-r18')
worker = base / 'native-ci-auth-probe-worker-01'
review = base / 'native-ci-auth-probe-root-01'
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
target = repo / 'docs/generated/player-qa/native-delivery-20261005/ci-authority-probe-01'
target.mkdir()
inventory = []
frozen = worker / 'FROZEN-RECEIPT.json'
assert hashlib.sha256(frozen.read_bytes()).hexdigest() == 'c3252223ee9d5eca55e36ef9d6781d0fa1c3607f0e316fb0ec03e5930702cb5b'
manifest = json.loads(frozen.read_bytes())
inputs = [('worker/' + row['path'], worker / row['path'], row['sha256']) for row in manifest['evidenceFiles']]
inputs.append(('worker/FROZEN-RECEIPT.json', frozen, hashlib.sha256(frozen.read_bytes()).hexdigest()))
root_names = ['ROOT-REVIEW.json', 'root-tests-01.log', 'workflow-before.yml', 'workflow-after.yml']
root_names.extend(p.name for p in review.glob('TEST-RECEIPT-*.json'))
for name in root_names:
    path = review / name
    inputs.append(('root/' + name, path, hashlib.sha256(path.read_bytes()).hexdigest()))
for relative, source, expected in inputs:
    data = source.read_bytes()
    assert hashlib.sha256(data).hexdigest() == expected
    destination = target / relative
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open('xb') as f:
        f.write(data)
    inventory.append({'path': relative, 'size': len(data), 'sha256': expected})
with (target / 'BYTE-INVENTORY.json').open('xb') as f:
    f.write((json.dumps({'schema': 'mir2.native-ci-probe-durable-evidence.v1', 'files': inventory}, indent=2) + '\n').encode())
with (target / '.gitattributes').open('x', newline='\n') as f:
    f.write('* -text -diff\n**/* -text -diff\nREADME.md text eol=lf\n')
with (target / 'README.md').open('x', encoding='utf-8', newline='\n') as f:
    f.write('# Read-only native delivery CI authority probe\n\n')
    f.write('Root integrates the exact frozen Python helper and 31 fake-transport controls; its independent 31-case run overlaps the worker suite. The genuine close-error RED, earlier preparation boundary and raw receipts are retained. No actual credential value is read or output by these local checks.\n\n')
    f.write('The registered Web asset workflow adds explicit `native_delivery_operation=probe`; legacy job structure is unchanged except for excluding the native probe. The native job performs two fixed Cloudflare GETs with opaque Actions secrets, persists no checkout credential, and publishes only its sanitized receipt. Dispatch and actual read authority remain pending. Success never proves upload, Worker edit, route edit or CDN publication.\n\n')
    f.write('`BYTE-INVENTORY.json` binds the retained worker and root proof bytes. Worker source SHA256: `1ce602ca8e1e1dc69c6745dec0ebce68237b040aa40ea11830b6aec9cdcf02dd`. The game, updater executable, signed feed, original origin additions and Gateway are unchanged by this integration.\n')
print(json.dumps({'storedFiles': len(inventory), 'target': str(target)}))
