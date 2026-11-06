"""Read-only origin inspection for four missing, already-signed R23 resources."""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
from datetime import datetime, timezone

ROOT = Path('/srv/mir2-client-updates')
RELEASE = ROOT / 'releases/game-WN-CANDIDATE-20261009-invited-23'
FILES = [
    ('mir2-assets/generated/native-map-keyed/coverage-audit.json', 723520, '014c18e1e49666877ebcda9e21935c22b46d8950aa2bd1b35a67824946cae42b'),
    ('mir2-assets/generated/native-map-keyed/resource-input-provenance.json', 32275, 'e8ca68605b5c27e925476cae6e055cceb9552157075d6e45b8aa9d4cb76f05a8'),
    ('mir2-assets/original-ui/Prguse/1002.png', 15671, '72f73b42bc7b95a91144fe0f135ede2ad343fbfed488c75095f98ed427caf497'),
    ('mir2-assets/original-ui/Title/18.png', 697, '3b8a6097b20b32fc1bb8d371efdefffe564af7bf1c75bc098bfd10be097c02ad'),
]

def node(path):
    try:
        s = path.lstat()
    except FileNotFoundError:
        return {'path': str(path), 'exists': False}
    result = {'path': str(path), 'exists': True, 'uid': s.st_uid,
              'mode': oct(stat.S_IMODE(s.st_mode)), 'size': s.st_size,
              'directory': stat.S_ISDIR(s.st_mode), 'regular': stat.S_ISREG(s.st_mode),
              'symlink': stat.S_ISLNK(s.st_mode)}
    if stat.S_ISLNK(s.st_mode):
        result['target'] = os.readlink(path)
    return result

def pin(path, max_size=32000000):
    n = node(path)
    if not n.get('exists'):
        return n
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        s = os.fstat(fd)
        if not stat.S_ISREG(s.st_mode) or s.st_size > max_size:
            raise RuntimeError('Not a bounded ordinary file: ' + str(path))
        with os.fdopen(fd, 'rb', closefd=False) as f:
            b = f.read(max_size + 1)
        n['sha256'] = hashlib.sha256(b).hexdigest()
    finally:
        os.close(fd)
    return n

report = {'schema': 'mir2.r23-missing-assets.preflight.v1',
          'utc': datetime.now(timezone.utc).isoformat(), 'euid': os.geteuid(),
          'mutations': False, 'directories': [], 'resources': []}
all_dirs = set([Path('/'), Path('/srv'), ROOT, ROOT / 'releases', RELEASE])
for rel, length, sha in FILES:
    p = RELEASE / rel
    all_dirs.update(p.parents)
    n = pin(p, 1000000)
    n.update(expectedSize=length, expectedSha256=sha)
    report['resources'].append(n)
report['directories'] = [node(p) for p in sorted(all_dirs, key=str)]
report['packageManifest'] = pin(RELEASE / 'PACKAGE-MANIFEST.json')
report['releaseMetadata'] = [pin(RELEASE / p, 1000000) for p in ['VERSION.json', 'BUILD-ATTESTATION.json', 'README-START.txt']]
report['publisherLock'] = node(Path('/var/backups/mir2-client-updates/.publisher.lock'))
report['feedPointer'] = node(ROOT / 'feed-current')
feed = (ROOT / 'feed-current/latest.json').resolve(strict=True)
if not feed.is_relative_to(ROOT):
    raise RuntimeError('Feed resolved outside public root')
report['feedPayload'] = pin(feed, 1000000)
report['feedSequence'] = json.loads(feed.read_bytes()).get('sequence')
caddy = Path('/etc/caddy/Caddyfile')
report['caddy'] = pin(caddy, 1000000)
caddy_source = caddy.read_text()
report['caddy']['hasSiteAnchor'] = '165.154.65.136.sslip.io' in caddy_source
report['caddy']['publicRoutingLines'] = [line.strip() for line in caddy_source.splitlines()
    if any(t in line for t in ['/client-updates', '/srv/mir2-client-updates', 'Cache-Control'])]
services = subprocess.run(['systemctl', 'show', 'mir2-playtest-gateway.service', 'caddy.service',
                          '--property=Id,ActiveState,SubState,MainPID,ExecMainStartTimestampMonotonic'],
                         check=True, capture_output=True, text=True, timeout=15)
report['services'] = services.stdout
print(json.dumps(report, ensure_ascii=True, indent=2))
