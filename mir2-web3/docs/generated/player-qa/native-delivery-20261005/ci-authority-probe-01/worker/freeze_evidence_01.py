"""Create-only proof freeze; reads only the new worker files and old public pins."""
import difflib
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import time

sys.dont_write_bytecode = True
BASE = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n').encode('ascii')


def write(name, raw):
    with (BASE / name).open('xb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())


source = BASE / 'cloudflare_read_authority_probe.py'
tests = BASE / 'test_cloudflare_read_authority_probe.py'
source_sha = digest(source)
test_sha = digest(tests)
assert source_sha == '1ce602ca8e1e1dc69c6745dec0ebce68237b040aa40ea11830b6aec9cdcf02dd'
assert test_sha == 'c9f830c69b2f552e526d0f26c251027d7a443e2504423e90289a11807fc758d5'
receipts = []
for path in sorted(BASE.glob('TEST-RECEIPT-*.json')):
    value = json.loads(path.read_bytes())
    receipts.append({'path': path.name, 'sha256': digest(path), 'tests': value['tests'],
        'passed': value['passed'], 'failures': value['failures'], 'errors': value['errors'],
        'sourceSha256': value['sourceSha256'], 'testSha256': value['testSha256']})
assert len(receipts) == 3
green = json.loads((BASE / 'TEST-RECEIPT-1791151263784061600.json').read_bytes())
assert green['passed'] and green['tests'] == 31 and green['failures'] == green['errors'] == green['skipped'] == 0
assert green['sourceSha256'] == source_sha and green['testSha256'] == test_sha
red = json.loads((BASE / 'TEST-RECEIPT-1791151067545936500.json').read_bytes())
assert not red['passed'] and red['tests'] == 30 and red['failures'] == 1 and red['errors'] == 0
assert digest(BASE / 'RED02-PROBE.py') == red['sourceSha256']
assert digest(BASE / 'RED02-TESTS.py') == red['testSha256']

# Recover the exact initial test bytes from the saved RED test snapshot and the
# two known, bounded preparation hunks. Verify the historical hash before save.
initial = (BASE / 'RED02-TESTS.py').read_text(encoding='utf-8')
start = initial.index('    def test_actual_transport_close_failure_is_error_and_stops_second_request(self):\n')
end = initial.index('    def test_real_transport_refuses_any_other_path_before_connection(self):\n', start)
initial = initial[:start] + initial[end:]
new = """             patch.object(P.os, 'environ', {'HTTPS_PROXY': 'http://proxy-' + MOCK_TOKEN + '.invalid',
                                          'HTTP_PROXY': 'http://another-' + MOCK_TOKEN + '.invalid'}):"""
old = """             patch.dict(os.environ, {'HTTPS_PROXY': 'http://proxy-' + MOCK_TOKEN + '.invalid',
                                     'HTTP_PROXY': 'http://another-' + MOCK_TOKEN + '.invalid'}, clear=True):"""
assert initial.count(new) == 1
initial = initial.replace(new, old, 1)
first = json.loads((BASE / 'TEST-RECEIPT-1791150943190915400.json').read_bytes())
assert hashlib.sha256(initial.encode('utf-8')).hexdigest() == first['testSha256']
write('INITIAL01-TESTS.py', initial.encode('utf-8'))
assert first['sourceSha256'] == digest(BASE / 'RED02-PROBE.py')

patch_text = ''
for before, after in ((BASE / 'RED02-PROBE.py', source), (BASE / 'RED02-TESTS.py', tests)):
    patch_text += ''.join(difflib.unified_diff(before.read_text(encoding='utf-8').splitlines(True),
        after.read_text(encoding='utf-8').splitlines(True), fromfile=str(before), tofile=str(after)))
write('RED-TO-GREEN.patch', patch_text.encode('utf-8'))
protected = [
    (BASE.parent / 'origin-acceleration-publisher-worker-01/FROZEN-RECEIPT.json', 'adf5156678cf98831c6562d867e12f1431e8bdc2bf888fe872eba1a4593f8ba8'),
    (BASE.parent / 'origin-acceleration-publisher-worker-01/origin-r17-append.py', '10693760a24973f896caa3f5ac6b1f59e02a32f6e9977cea0dbaa65adbaa0811'),
]
for path, expected in protected:
    assert digest(path) == expected
write('SOURCE-BINDINGS.json', canonical({
    'schema': 'mir2.cloudflare.read-probe-source-bindings.v1', 'createdUnix': int(time.time()),
    'sourceSha256': source_sha, 'testSha256': test_sha,
    'docBindings': {'path': 'DOCS-BINDINGS.json', 'sha256': digest(BASE / 'DOCS-BINDINGS.json')},
    'redInputs': {'probe': {'path': 'RED02-PROBE.py', 'sha256': red['sourceSha256']},
                  'tests': {'path': 'RED02-TESTS.py', 'sha256': red['testSha256']}},
    'initialTestExactSnapshotSha256': digest(BASE / 'INITIAL01-TESTS.py'),
    'protectedOldPublicProof': [{'path': str(path), 'sha256': expected, 'readOnlyUnchanged': True} for path, expected in protected],
    'repositoryOrWorkflowChanges': False, 'gitChanges': False,
    'credentialsInspectedUsedOrLogged': False, 'authenticatedNetworkRequests': False,
    'initialEnvironmentTraversalQualification': 'patch.dict initially snapshots process mapping; not proof of no traversal; replaced by direct fake-mapping substitution in RED02 and GREEN03',
}))
write('COMMANDS.json', canonical({
    'schema': 'mir2.cloudflare.read-probe-local-commands.v1', 'createdUnix': int(time.time()),
    'commands': [{'argv': ['C:/Python313/python.exe', '-B', str(tests)], 'stdoutStderrLog': name}
                 for name in ('local-tests-01.log', 'local-tests-02-red.log', 'local-tests-03-green.log')],
    'rawReceipts': receipts,
    'probeCliWasNotInvokedWithActualEnvironment': True, 'authenticatedNetworkRequests': False,
    'officialDocumentationOnlyBrowsing': True, 'ciDispatch': False,
}))
files = []
for current, dirs, names in os.walk(BASE, followlinks=False):
    current = Path(current)
    for name in [*dirs, *names]:
        path = current / name
        info = path.lstat()
        relative = path.relative_to(BASE).as_posix()
        if stat.S_ISLNK(info.st_mode):
            files.append({'path': relative, 'type': 'symlink-fixture', 'target': os.readlink(path)})
        elif stat.S_ISREG(info.st_mode):
            files.append({'path': relative, 'type': 'regular', 'size': info.st_size,
                          'sha256': digest(path), 'nlink': info.st_nlink})
        elif not stat.S_ISDIR(info.st_mode):
            raise RuntimeError('unexpected local proof type')
files.sort(key=lambda o: o['path'])
write('FROZEN-RECEIPT.json', canonical({
    'schema': 'mir2.cloudflare.read-probe-worker-frozen.v1', 'createdUnix': int(time.time()),
    'sourceSha256': source_sha, 'testSha256': test_sha,
    'finalTests': {'tests': 31, 'passed': True, 'failures': 0, 'errors': 0, 'skipped': 0,
                   'log': 'local-tests-03-green.log', 'receipt': 'TEST-RECEIPT-1791151263784061600.json'},
    'redTests': {'tests': 30, 'failures': 1, 'errors': 0, 'log': 'local-tests-02-red.log',
                 'meaning': 'connection close error falsely allowed second GET and success'},
    'initialPreparationBoundaryRetained': True, 'codeFrozen': True,
    'credentialsUsedOrPrinted': False, 'authenticatedNetworkRequests': False,
    'officialDocumentationOnlyBrowsing': True, 'repositoryOrWorkflowChanges': False,
    'ciDispatch': False, 'serviceOrDeploymentChanges': False,
    'evidenceManifestSha256': hashlib.sha256(canonical(files)).hexdigest(), 'evidenceFiles': files,
    'openGates': ['root review and workflow integration', 'opaque actual CI credential invocation',
                  'actual token activity and known-bucket read authority',
                  'R2 object-write or Worker-edit authority is deliberately not tested'],
}))
print(json.dumps({'passed': True, 'files': len(files), 'sourceSha256': source_sha,
    'testSha256': test_sha, 'frozenReceiptSha256': digest(BASE / 'FROZEN-RECEIPT.json')}, sort_keys=True))
