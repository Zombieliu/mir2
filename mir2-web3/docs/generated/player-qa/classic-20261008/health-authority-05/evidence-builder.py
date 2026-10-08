import hashlib
import json
import re
import subprocess
import tarfile
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
baseline = Path(sys.argv[2]).resolve()
out = root / 'mir2-web3/docs/generated/player-qa/classic-20261008/health-authority-05'
build = Path('C:/mir2-build')
parent = '937ac636c7118565f53e0ed90f280f166e63b12c'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write_new(path, data):
    with path.open('xb') as file:
        file.write(data)

def record(path):
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest(path)}

sources = [
    'apps/simulation/src/crystal_health.rs',
    'apps/simulation/src/lib.rs',
    'apps/simulation/src/runtime/session.rs',
    'apps/simulation/src/runtime/packets.rs',
    'apps/simulation/src/runtime/skills.rs',
    'apps/simulation/src/runtime/zone/runtime.rs',
    'apps/simulation/src/runtime/zone/runtime/special_ai.rs',
    'apps/simulation/src/runtime/zone/runtime/revival_ai.rs',
    'apps/simulation/src/runtime/zone/runtime/health_display_source_tests.rs',
    'apps/simulation/tests/shared_monster_health_lifecycle.rs',
    'apps/gateway/src/shared_groups.rs',
]

git = ['git', '-c', 'gc.auto=0', '-c', 'maintenance.auto=false']
paths = [
    'mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock', 'mir2-web3/apps/simulation',
    'mir2-web3/apps/gateway', 'mir2-web3/apps/admin-api',
    'mir2-web3/apps/dubhe-node-desktop/src-tauri', 'mir2-web3/apps/game-client',
    'mir2-web3/packages', 'mir2-web3/vendor', 'mir2-web3/config', 'mir2-web3/infra',
]
tree = subprocess.check_output(git + ['ls-tree', '-rz', parent, '--'] + paths, cwd=root)
entries = [entry for entry in tree.split(b'\0') if entry]
names = [entry.split(b'\t', 1)[1].decode() for entry in entries]
raw_attributes = subprocess.check_output(
    git + ['check-attr', '--source=' + parent, '-z', '--stdin', 'text', 'eol'],
    input=b'\0'.join(name.encode() for name in names) + b'\0', cwd=root,
).split(b'\0')
attributes = {}
for index in range(0, len(raw_attributes) - 1, 3):
    name, attribute, value = [item.decode() for item in raw_attributes[index:index + 3]]
    attributes.setdefault(name, {})[attribute] = value
archives = [tarfile.open(baseline / name) for name in ('source.tar', 'input-data.tar')]
archive_members = {}
for archive in archives:
    for member in archive.getmembers():
        if member.isfile():
            assert member.name not in archive_members, member.name
            archive_members[member.name] = (archive, member)
bound = []
for entry in entries:
    meta, name = entry.split(b'\t', 1)
    mode, kind, git_hash = meta.decode().split()
    relative = name.decode()
    path = baseline / relative
    assert mode in ('100644', '100755') and kind == 'blob', (mode, relative)
    assert path.is_file() and not path.is_symlink(), relative
    data = path.read_bytes()
    archive, member = archive_members[relative]
    archive_data = archive.extractfile(member).read()
    assert data == archive_data, ('baseline mutated after extraction', relative)
    actual = hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest()
    normalized = data
    normalization = 'none'
    if actual != git_hash:
        file_attributes = attributes[relative]
        assert file_attributes['text'] in ('set', 'auto'), (relative, file_attributes)
        assert b'\0' not in data and b'\r\n' in data, relative
        normalized = data.replace(b'\r\n', b'\n')
        normalization = 'declared-text CRLF-to-LF only'
    canonical = hashlib.sha1(
        b'blob ' + str(len(normalized)).encode() + b'\0' + normalized,
    ).hexdigest()
    assert canonical == git_hash, (relative, git_hash, actual, canonical)
    bound.append({'path': relative, 'gitBlob': git_hash, 'archiveBlob': actual,
                  'canonicalBlob': canonical, 'bytes': len(data), 'sha256': digest(path),
                  'archiveMemberBytesUnchanged': True, 'attributes': attributes[relative],
                  'normalization': normalization})
for archive in archives:
    archive.close()

baseline_receipt = {
    'schema': 'mir2.health.baseline-archive-content.v1', 'commit': parent,
    'allArchiveMemberBytesUnchanged': True,
    'gitTrackedContentMatchesAfterDeclaredEol': True, 'files': bound,
    'gitArchiveMayApplyTextCheckoutEol': True,
    'archives': [record(baseline / 'source.tar'), record(baseline / 'input-data.tar')],
    'initialMissingConfigInfraCompileFailuresRetained': True,
    'sourceAndAssertionsChanged': False,
}
write_new(out / 'BASELINE-SOURCE-05.json', json.dumps(baseline_receipt, indent=2).encode() + b'\n')

inventory = []
runs = []
for log in sorted(build.glob('health-zero-authority-root-05-*.log')):
    target = out / log.name
    write_new(target, log.read_bytes())
    inventory.append({'file': target.name, 'original': str(log), 'bytes': target.stat().st_size,
                      'sha256': digest(target)})
    content = log.read_text(encoding='utf-8-sig', errors='replace')
    matches = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;.*', content)
    failures = re.findall(r'^test ([^ ]+) \.\.\. FAILED$', content, re.MULTILINE)
    runs.append({'file': target.name, 'results': matches, 'failedTests': failures,
                 'compilationFailed': 'could not compile' in content})

def run(number):
    return next(r for r in runs if r['file'].startswith(f'health-zero-authority-root-05-{number:02}-'))

comparisons = []
for current, before in [(11, 21), (12, 22), (14, 23)]:
    a, b = run(current), run(before)
    assert a['results'] == b['results'] and a['failedTests'] == b['failedTests'], (a, b)
    comparisons.append({'current': a['file'], 'baseline': b['file'],
                        'sameFailingNamesAndCounts': True, 'failedTests': a['failedTests']})

for source in [build / 'health-zero-authority-root-05-live-observation.json',
               build / 'r22-gateway-preflight-root-20261008-0839-01.log']:
    target = out / source.name
    write_new(target, source.read_bytes())
    inventory.append({'file': target.name, 'original': str(source), 'bytes': target.stat().st_size,
                      'sha256': digest(target)})

proof = {
    'schema': 'mir2.health-authority-source-arithmetic.v1', 'parent': parent,
    'sourceRoot': str(root), 'sourceFiles': [record(root / 'mir2-web3' / p) for p in sources],
    'sourceReference': [record(Path('E:/mir2/Crystal/Server/MirObjects/MapObject.cs')),
                        record(Path('E:/mir2/Crystal/Server/MirObjects/HeroObject.cs'))],
    'independentOracle': {
        'program': record(out / 'source-vectors.cs'),
        'vectors': record(out / 'source-vectors.csv'),
        'compiledExecutable': record(build / 'health-source-vectors-root-05.exe'),
        'compiler': 'Windows Framework64/v4.0.30319/csc.exe /optimize+ /platform:x64',
        'firstForwardSlashCompilerPathFailure': 'CS1504, original tool output retained in chat',
        'invalidPoolsOrUniversalZeroMaximumManaConversionProven': False,
    },
    'runs': runs, 'baselineComparisons': comparisons,
    'originalLogsInventoried': inventory,
    'zeroExecutionSelectionsAreNotPassingCoverage': [run(8)['file'], run(9)['file']],
    'trueDeathReviveHarvestGuardsRetained': True,
    'livePublicSource': 'c6c32381a646dae1067dafdeec57691f606b1779',
    'gamePackageSourceStillFrozen': '7fea5cdba8cd160d4f2c5536f14fb7ce98c6bfbc',
    'r22Promoted': False, 'gatewayRestarted': False, 'humanWheelAccepted': False,
    'remaining': [
        'Full event-driven health recipient dispatcher, HP/MP owner packets and group entry',
        'Trusted unique shared Hero identity/life/vitals admission',
        'Retained healing/deadline refresh and personal Revelation SC/RevTime state',
        'Web still derives exact HP0/death/harvest from percentage0; not covered',
        'Five adjacent baseline failures retained without skipping or relaxing assertions',
        'No-live-ack causal ordering, absent-Death native corpse animation and crowded escape',
        'Normal client exit, strict public drain, paired rollout and human acceptance',
        'Full P1-P7 and original missed 2026-10-08 03:13:35 UTC deadline',
    ],
}
write_new(out / 'ROOT-FINAL-05.json', json.dumps(proof, indent=2).encode() + b'\n')
print(json.dumps({'baselineFiles': len(bound), 'logs': len(runs), 'sourceFiles': len(sources),
                  'baselineFailureComparison': True, 'proof': record(out / 'ROOT-FINAL-05.json')}))
