"""Create-only integration of exact frozen non-secret Python source."""
import hashlib
from pathlib import Path

root = Path('C:/Users/Administrator/.codex/worktrees/crowded-combat-control/mir2-player-journey/mir2-web3')
source = Path('C:/mir2-playtest-releases/20261004-native-r18/native-ci-auth-probe-worker-01')
expected = {
    'cloudflare_read_authority_probe.py': '1ce602ca8e1e1dc69c6745dec0ebce68237b040aa40ea11830b6aec9cdcf02dd',
    'test_cloudflare_read_authority_probe.py': 'c9f830c69b2f552e526d0f26c251027d7a443e2504423e90289a11807fc758d5',
}
for name, digest in expected.items():
    data = (source / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == digest
    target = root / 'scripts' / name
    with target.open('xb') as stream:
        stream.write(data)
    assert target.read_bytes() == data
print('Integrated two frozen read-only probe files; no credentials accessed.')
