"""Root integrates the reviewed literal, create-only importer; no deployment."""
import hashlib
import json
import subprocess
from pathlib import Path

worker = Path(__file__).resolve().parent
repo = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey')
git = ['git', '-c', 'gc.auto=0', '-C', str(repo)]
assert not subprocess.check_output(git + ['status', '--porcelain']).strip()
frozen = worker / 'FROZEN-DELIVERY.json'
assert hashlib.sha256(frozen.read_bytes()).hexdigest() == '2f70ff0b4a3d07792e6c7cd5e25b59921887307443a38e35e602a83b762eaa07'
receipt = json.loads(frozen.read_bytes())
assert len(receipt['approvedSourceFiles']) == 4
base = repo / 'mir2-web3/infra/cloudflare/mir2-r2-bulk-upload/src/index.ts'
assert hashlib.sha256(base.read_bytes()).hexdigest() == receipt['baseIndexSha256']
inventory = []
for row in receipt['approvedSourceFiles']:
    source = worker / row['externalPath']
    data = source.read_bytes()
    assert hashlib.sha256(data).hexdigest() == row['sha256']
    target = repo / row['repositoryPath']
    target.parent.mkdir(parents=True, exist_ok=True)
    if target == base:
        target.write_bytes(data)
    else:
        with target.open('xb') as f:
            f.write(data)
    assert target.read_bytes() == data
    inventory.append(row)
out = worker.parent / 'r17-native-r2-importer-root-01'
out.mkdir()
with (out / 'INTEGRATION-01.json').open('x') as f:
    json.dump({'schema': 'mir2.native-r17-importer-root-integration.v1', 'files': inventory,
               'allLegacySourceExceptReviewedIndexHunksPreserved': True, 'remoteR2OrServicesChanged': False,
               'actualAuthDeniedAndPublicationPending': True}, f, indent=2)
    f.write('\n')
print(json.dumps({'integratedFiles': len(inventory), 'remoteDeployment': False}))
